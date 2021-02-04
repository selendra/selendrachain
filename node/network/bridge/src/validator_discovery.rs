// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! A validator discovery service for the Network Bridge.

use core::marker::PhantomData;
use std::collections::{HashSet, HashMap, hash_map};
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::mpsc;

use sc_network::multiaddr::{Multiaddr, Protocol};
use sc_authority_discovery::Service as AuthorityDiscoveryService;
use indracore_node_network_protocol::PeerId;
use indracore_primitives::v1::{AuthorityDiscoveryId, Block, Hash};

const PRIORITY_GROUP: &'static str = "parachain_validators";
const LOG_TARGET: &str = "validator_discovery";

/// An abstraction over networking for the purposes of validator discovery service.
#[async_trait]
pub trait Network: Send + 'static {
	/// Ask the network to connect to these nodes and not disconnect from them until removed from the priority group.
	async fn add_to_priority_group(&mut self, group_id: String, multiaddresses: HashSet<Multiaddr>) -> Result<(), String>;
	/// Remove the peers from the priority group.
	async fn remove_from_priority_group(&mut self, group_id: String, multiaddresses: HashSet<Multiaddr>) -> Result<(), String>;
}

/// An abstraction over the authority discovery service.
#[async_trait]
pub trait AuthorityDiscovery: Send + 'static {
	/// Get the addresses for the given [`AuthorityId`] from the local address cache.
	async fn get_addresses_by_authority_id(&mut self, authority: AuthorityDiscoveryId) -> Option<Vec<Multiaddr>>;
	/// Get the [`AuthorityId`] for the given [`PeerId`] from the local address cache.
	async fn get_authority_id_by_peer_id(&mut self, peer_id: PeerId) -> Option<AuthorityDiscoveryId>;
}

#[async_trait]
impl Network for Arc<sc_network::NetworkService<Block, Hash>> {
	async fn add_to_priority_group(&mut self, group_id: String, multiaddresses: HashSet<Multiaddr>) -> Result<(), String> {
		sc_network::NetworkService::add_to_priority_group(&**self, group_id, multiaddresses).await
	}

	async fn remove_from_priority_group(&mut self, group_id: String, multiaddresses: HashSet<Multiaddr>) -> Result<(), String> {
		sc_network::NetworkService::remove_from_priority_group(&**self, group_id, multiaddresses).await
	}
}

#[async_trait]
impl AuthorityDiscovery for AuthorityDiscoveryService {
	async fn get_addresses_by_authority_id(&mut self, authority: AuthorityDiscoveryId) -> Option<Vec<Multiaddr>> {
		AuthorityDiscoveryService::get_addresses_by_authority_id(self, authority).await
	}

	async fn get_authority_id_by_peer_id(&mut self, peer_id: PeerId) -> Option<AuthorityDiscoveryId> {
		AuthorityDiscoveryService::get_authority_id_by_peer_id(self, peer_id).await
	}
}

/// This struct tracks the state for one `ConnectToValidators` request.
struct NonRevokedConnectionRequestState {
	requested: Vec<AuthorityDiscoveryId>,
	pending: HashSet<AuthorityDiscoveryId>,
	sender: mpsc::Sender<(AuthorityDiscoveryId, PeerId)>,
}

impl NonRevokedConnectionRequestState {
	/// Create a new instance of `ConnectToValidatorsState`.
	pub fn new(
		requested: Vec<AuthorityDiscoveryId>,
		pending: HashSet<AuthorityDiscoveryId>,
		sender: mpsc::Sender<(AuthorityDiscoveryId, PeerId)>,
	) -> Self {
		Self {
			requested,
			pending,
			sender,
		}
	}

	pub fn on_authority_connected(&mut self, authority: &AuthorityDiscoveryId, peer_id: &PeerId) {
		if self.pending.remove(authority) {
			// an error may happen if the request was revoked or
			// the channel's buffer is full, ignoring it is fine
			let _ = self.sender.try_send((authority.clone(), peer_id.clone()));
		}
	}

	/// Returns `true` if the request is revoked.
	pub fn is_revoked(&mut self) -> bool {
		self.sender.is_closed()
	}

	pub fn requested(&self) -> &[AuthorityDiscoveryId] {
		self.requested.as_ref()
	}
}

/// Will be called by [`Service::on_request`] when a request was revoked.
///
/// Takes the `map` of requested validators and the `id` of the validator that should be revoked.
///
/// Returns `Some(id)` iff the request counter is `0`.
fn on_revoke(map: &mut HashMap<AuthorityDiscoveryId, u64>, id: AuthorityDiscoveryId) -> Option<AuthorityDiscoveryId> {
	if let hash_map::Entry::Occupied(mut entry) = map.entry(id) {
		if entry.get_mut().saturating_sub(1) == 0 {
			return Some(entry.remove_entry().0);
		}
	}

	None
}

fn peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
	addr.iter().last().and_then(|protocol| if let Protocol::P2p(multihash) = protocol {
		PeerId::from_multihash(multihash).ok()
	} else {
		None
	})
}

pub(super) struct Service<N, AD> {
	// Peers that are connected to us and authority ids associated to them.
	connected_peers: HashMap<PeerId, HashSet<AuthorityDiscoveryId>>,
	// The `u64` counts the number of pending non-revoked requests for this validator
	// note: the validators in this map are not necessarily present
	// in the `connected_validators` map.
	// Invariant: the value > 0 for non-revoked requests.
	requested_validators: HashMap<AuthorityDiscoveryId, u64>,
	non_revoked_discovery_requests: Vec<NonRevokedConnectionRequestState>,
	// PhantomData used to make the struct generic instead of having generic methods
	_phantom: PhantomData<(N, AD)>,
}

impl<N: Network, AD: AuthorityDiscovery> Service<N, AD> {
	pub fn new() -> Self {
		Self {
			connected_peers: HashMap::new(),
			requested_validators: HashMap::new(),
			non_revoked_discovery_requests: Vec::new(),
			_phantom: PhantomData,
		}
	}

	/// Find connected validators using the given `validator_ids`.
	///
	/// Returns a [`HashMap`] that contains the found [`AuthorityDiscoveryId`]'s and their associated [`PeerId`]'s.
	#[tracing::instrument(level = "trace", skip(self, authority_discovery_service), fields(subsystem = LOG_TARGET))]
	async fn find_connected_validators(
		&mut self,
		validator_ids: &[AuthorityDiscoveryId],
		authority_discovery_service: &mut AD,
	) -> HashMap<AuthorityDiscoveryId, PeerId> {
		let mut result = HashMap::new();

		for id in validator_ids {
			// First check if we already cached the validator
			if let Some(pid) = self.connected_peers
				.iter()
				.find_map(|(pid, ids)| if ids.contains(&id) { Some(pid) } else { None }) {
				result.insert(id.clone(), pid.clone());
				continue;
			}

			// If not ask the authority discovery
			if let Some(addresses) = authority_discovery_service.get_addresses_by_authority_id(id.clone()).await {
				for peer_id in addresses.iter().filter_map(peer_id_from_multiaddr) {
					if let Some(ids) = self.connected_peers.get_mut(&peer_id) {
						ids.insert(id.clone());
						result.insert(id.clone(), peer_id.clone());
					}
				}
			}
		}

		result
	}

	/// On a new connection request, a priority group update will be issued.
	/// It will ask the network to connect to the validators and not disconnect
	/// from them at least until all the pending requests containing them are revoked.
	///
	/// This method will also clean up all previously revoked requests.
	/// it takes `network_service` and `authority_discovery_service` by value
	/// and returns them as a workaround for the Future: Send requirement imposed by async fn impl.
	#[tracing::instrument(level = "trace", skip(self, connected, network_service, authority_discovery_service), fields(subsystem = LOG_TARGET))]
	pub async fn on_request(
		&mut self,
		validator_ids: Vec<AuthorityDiscoveryId>,
		mut connected: mpsc::Sender<(AuthorityDiscoveryId, PeerId)>,
		mut network_service: N,
		mut authority_discovery_service: AD,
	) -> (N, AD) {
		const MAX_ADDR_PER_PEER: usize = 3;

		// Increment the counter of how many times the validators were requested.
		validator_ids.iter().for_each(|id| *self.requested_validators.entry(id.clone()).or_default() += 1);
		let already_connected = self.find_connected_validators(&validator_ids, &mut authority_discovery_service).await;

		// try to send already connected peers
		for (id, peer) in already_connected.iter() {
			match connected.try_send((id.clone(), peer.clone())) {
				Err(e) if e.is_disconnected() => {
					// the request is already revoked
					for peer_id in validator_ids {
						let _ = on_revoke(&mut self.requested_validators, peer_id);
					}
					return (network_service, authority_discovery_service);
				}
				Err(_) => {
					// the channel's buffer is full
					// ignore the error, the receiver will miss out some peers
					// but that's fine
					break;
				}
				Ok(()) => continue,
			}
		}

		// collect multiaddress of validators
		let mut multiaddr_to_add = HashSet::new();
		for authority in validator_ids.iter() {
			let result = authority_discovery_service.get_addresses_by_authority_id(authority.clone()).await;
			if let Some(addresses) = result {
				// We might have several `PeerId`s per `AuthorityId`
				// depending on the number of sentry nodes,
				// so we limit the max number of sentries per node to connect to.
				// They are going to be removed soon though:
				// https://github.com/paritytech/substrate/issues/6845
				multiaddr_to_add.extend(addresses.into_iter().take(MAX_ADDR_PER_PEER));
			}
		}

		// clean up revoked requests
		let mut revoked_indices = Vec::new();
		let mut revoked_validators = Vec::new();
		for (i, maybe_revoked) in self.non_revoked_discovery_requests.iter_mut().enumerate() {
			if maybe_revoked.is_revoked() {
				for id in maybe_revoked.requested() {
					if let Some(id) = on_revoke(&mut self.requested_validators, id.clone()) {
						revoked_validators.push(id);
					}
				}
				revoked_indices.push(i);
			}
		}

		// clean up revoked requests states
		for to_revoke in revoked_indices.into_iter().rev() {
			drop(self.non_revoked_discovery_requests.swap_remove(to_revoke));
		}

		// multiaddresses to remove
		let mut multiaddr_to_remove = HashSet::new();
		for id in revoked_validators.into_iter() {
			let result = authority_discovery_service.get_addresses_by_authority_id(id).await;
			if let Some(addresses) = result {
				multiaddr_to_remove.extend(addresses.into_iter());
			}
		}

		// ask the network to connect to these nodes and not disconnect
		// from them until removed from the priority group
		if let Err(e) = network_service.add_to_priority_group(
			PRIORITY_GROUP.to_owned(),
			multiaddr_to_add,
		).await {
			tracing::warn!(target: LOG_TARGET, err = ?e, "AuthorityDiscoveryService returned an invalid multiaddress");
		}
		// the addresses are known to be valid
		let _ = network_service.remove_from_priority_group(PRIORITY_GROUP.to_owned(), multiaddr_to_remove).await;

		let pending = validator_ids.iter()
			.cloned()
			.filter(|id| !already_connected.contains_key(id))
			.collect::<HashSet<_>>();

		self.non_revoked_discovery_requests.push(NonRevokedConnectionRequestState::new(
			validator_ids,
			pending,
			connected,
		));

		(network_service, authority_discovery_service)
	}

	/// Should be called when a peer connected.
	#[tracing::instrument(level = "trace", skip(self, authority_discovery_service), fields(subsystem = LOG_TARGET))]
	pub async fn on_peer_connected(&mut self, peer_id: &PeerId, authority_discovery_service: &mut AD) {
		// check if it's an authority we've been waiting for
		let maybe_authority = authority_discovery_service.get_authority_id_by_peer_id(peer_id.clone()).await;
		if let Some(authority) = maybe_authority {
			for request in self.non_revoked_discovery_requests.iter_mut() {
				let _ = request.on_authority_connected(&authority, peer_id);
			}

			self.connected_peers.entry(peer_id.clone()).or_default().insert(authority);
		} else {
			self.connected_peers.insert(peer_id.clone(), Default::default());
		}
	}

	/// Should be called when a peer disconnected.
	pub fn on_peer_disconnected(&mut self, peer_id: &PeerId) {
		self.connected_peers.remove(peer_id);
	}
}
