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

//! The bitfield distribution
//!
//! In case this node is a validator, gossips its own signed availability bitfield
//! for a particular relay parent.
//! Independently of that, gossips on received messages from peers to other interested peers.

#![deny(unused_crate_dependencies)]

use parity_scale_codec::{Decode, Encode};
use futures::{channel::oneshot, FutureExt};

use indracore_subsystem::messages::*;
use indracore_subsystem::{
	ActiveLeavesUpdate, FromOverseer, OverseerSignal, SpawnedSubsystem, Subsystem, SubsystemContext, SubsystemResult,
};
use indracore_node_subsystem_util::metrics::{self, prometheus};
use indracore_primitives::v1::{Hash, SignedAvailabilityBitfield, SigningContext, ValidatorId};
use indracore_node_network_protocol::{v1 as protocol_v1, PeerId, NetworkBridgeEvent, View, ReputationChange};
use std::collections::{HashMap, HashSet};

const COST_SIGNATURE_INVALID: ReputationChange =
	ReputationChange::new(-100, "Bitfield signature invalid");
const COST_VALIDATOR_INDEX_INVALID: ReputationChange =
	ReputationChange::new(-100, "Bitfield validator index invalid");
const COST_MISSING_PEER_SESSION_KEY: ReputationChange =
	ReputationChange::new(-133, "Missing peer session key");
const COST_NOT_IN_VIEW: ReputationChange =
	ReputationChange::new(-51, "Not interested in that parent hash");
const COST_PEER_DUPLICATE_MESSAGE: ReputationChange =
	ReputationChange::new(-500, "Peer sent the same message multiple times");
const BENEFIT_VALID_MESSAGE_FIRST: ReputationChange =
	ReputationChange::new(15, "Valid message with new information");
const BENEFIT_VALID_MESSAGE: ReputationChange =
	ReputationChange::new(10, "Valid message");

/// Checked signed availability bitfield that is distributed
/// to other peers.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq)]
struct BitfieldGossipMessage {
	/// The relay parent this message is relative to.
	relay_parent: Hash,
	/// The actual signed availability bitfield.
	signed_availability: SignedAvailabilityBitfield,
}

impl BitfieldGossipMessage {
	fn into_validation_protocol(self) -> protocol_v1::ValidationProtocol {
		protocol_v1::ValidationProtocol::BitfieldDistribution(
			self.into_network_message()
		)
	}

	fn into_network_message(self)
		-> protocol_v1::BitfieldDistributionMessage
	{
		protocol_v1::BitfieldDistributionMessage::Bitfield(
			self.relay_parent,
			self.signed_availability,
		)
	}
}

/// Data used to track information of peers and relay parents the
/// overseer ordered us to work on.
#[derive(Default, Clone, Debug)]
struct ProtocolState {
	/// track all active peers and their views
	/// to determine what is relevant to them.
	peer_views: HashMap<PeerId, View>,

	/// Our current view.
	view: View,

	/// Additional data particular to a relay parent.
	per_relay_parent: HashMap<Hash, PerRelayParentData>,
}

/// Data for a particular relay parent.
#[derive(Debug, Clone, Default)]
struct PerRelayParentData {
	/// Signing context for a particular relay parent.
	signing_context: SigningContext,

	/// Set of validators for a particular relay parent.
	validator_set: Vec<ValidatorId>,

	/// Set of validators for a particular relay parent for which we
	/// received a valid `BitfieldGossipMessage`.
	/// Also serves as the list of known messages for peers connecting
	/// after bitfield gossips were already received.
	one_per_validator: HashMap<ValidatorId, BitfieldGossipMessage>,

	/// Avoid duplicate message transmission to our peers.
	message_sent_to_peer: HashMap<PeerId, HashSet<ValidatorId>>,

	/// Track messages that were already received by a peer
	/// to prevent flooding.
	message_received_from_peer: HashMap<PeerId, HashSet<ValidatorId>>,
}

impl PerRelayParentData {
	/// Determines if that particular message signed by a validator is needed by the given peer.
	fn message_from_validator_needed_by_peer(
		&self,
		peer: &PeerId,
		validator: &ValidatorId,
	) -> bool {
		self.message_sent_to_peer.get(peer).map(|v| !v.contains(validator)).unwrap_or(true)
			&& self.message_received_from_peer.get(peer).map(|v| !v.contains(validator)).unwrap_or(true)
	}
}

const LOG_TARGET: &str = "bitfield_distribution";

/// The bitfield distribution subsystem.
pub struct BitfieldDistribution {
	metrics: Metrics,
}

impl BitfieldDistribution {
	/// Create a new instance of the `BitfieldDistribution` subsystem.
	pub fn new(metrics: Metrics) -> Self {
		Self { metrics }
	}

	/// Start processing work as passed on from the Overseer.
	#[tracing::instrument(skip(self, ctx), fields(subsystem = LOG_TARGET))]
	async fn run<Context>(self, mut ctx: Context)
	where
		Context: SubsystemContext<Message = BitfieldDistributionMessage>,
	{
		// work: process incoming messages from the overseer and process accordingly.
		let mut state = ProtocolState::default();
		loop {
			let message = match ctx.recv().await {
				Ok(message) => message,
				Err(e) => {
					tracing::debug!(target: LOG_TARGET, err = ?e, "Failed to receive a message from Overseer, exiting");
					return;
				},
			};
			match message {
				FromOverseer::Communication {
					msg: BitfieldDistributionMessage::DistributeBitfield(hash, signed_availability),
				} => {
					tracing::trace!(target: LOG_TARGET, "Processing DistributeBitfield");
					handle_bitfield_distribution(
						&mut ctx,
						&mut state,
						&self.metrics,
						hash,
						signed_availability,
					).await;
				}
				FromOverseer::Communication {
					msg: BitfieldDistributionMessage::NetworkBridgeUpdateV1(event),
				} => {
					tracing::trace!(target: LOG_TARGET, "Processing NetworkMessage");
					// a network message was received
					handle_network_msg(&mut ctx, &mut state, &self.metrics, event).await;
				}
				FromOverseer::Signal(OverseerSignal::ActiveLeaves(ActiveLeavesUpdate { activated, deactivated })) => {
					let _timer = self.metrics.time_active_leaves_update();

					for relay_parent in activated {
						tracing::trace!(target: LOG_TARGET, relay_parent = %relay_parent, "activated");
						// query basic system parameters once
						match query_basics(&mut ctx, relay_parent).await {
							Ok(Some((validator_set, signing_context))) => {
								// If our runtime API fails, we don't take down the node,
								// but we might alter peers' reputations erroneously as a result
								// of not having the correct bookkeeping. If we have lost a race
								// with state pruning, it is unlikely that peers will be sending
								// us anything to do with this relay-parent anyway.
								let _ = state.per_relay_parent.insert(
									relay_parent,
									PerRelayParentData {
										signing_context,
										validator_set,
										..Default::default()
									},
								);
							}
							Err(e) => {
								tracing::warn!(target: LOG_TARGET, err = ?e, "query_basics has failed");
							}
							_ => {},
						}
					}

					for relay_parent in deactivated {
						tracing::trace!(target: LOG_TARGET, relay_parent = %relay_parent, "deactivated");
						// defer the cleanup to the view change
					}
				}
				FromOverseer::Signal(OverseerSignal::BlockFinalized(hash)) => {
					tracing::trace!(target: LOG_TARGET, hash = %hash, "block finalized");
				}
				FromOverseer::Signal(OverseerSignal::Conclude) => {
					tracing::trace!(target: LOG_TARGET, "Conclude");
					return;
				}
			}
		}
	}
}

/// Modify the reputation of a peer based on its behaviour.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn modify_reputation<Context>(
	ctx: &mut Context,
	peer: PeerId,
	rep: ReputationChange,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	tracing::trace!(target: LOG_TARGET, rep = ?rep, peer_id = %peer, "reputation change");
	ctx.send_message(AllMessages::NetworkBridge(
		NetworkBridgeMessage::ReportPeer(peer, rep),
	))
	.await
}

/// Distribute a given valid and signature checked bitfield message.
///
/// For this variant the source is this node.
#[tracing::instrument(level = "trace", skip(ctx, metrics), fields(subsystem = LOG_TARGET))]
async fn handle_bitfield_distribution<Context>(
	ctx: &mut Context,
	state: &mut ProtocolState,
	metrics: &Metrics,
	relay_parent: Hash,
	signed_availability: SignedAvailabilityBitfield,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	let _timer = metrics.time_handle_bitfield_distribution();

	// Ignore anything the overseer did not tell this subsystem to work on
	let mut job_data = state.per_relay_parent.get_mut(&relay_parent);
	let job_data: &mut _ = if let Some(ref mut job_data) = job_data {
		job_data
	} else {
		tracing::trace!(
			target: LOG_TARGET,
			relay_parent = %relay_parent,
			"Not supposed to work on relay parent related data",
		);

		return;
	};
	let validator_set = &job_data.validator_set;
	if validator_set.is_empty() {
		tracing::trace!(target: LOG_TARGET, relay_parent = %relay_parent, "validator set is empty");
		return;
	}

	let validator_index = signed_availability.validator_index() as usize;
	let validator = if let Some(validator) = validator_set.get(validator_index) {
		validator.clone()
	} else {
		tracing::trace!(target: LOG_TARGET, "Could not find a validator for index {}", validator_index);
		return;
	};

	let peer_views = &mut state.peer_views;
	let msg = BitfieldGossipMessage {
		relay_parent,
		signed_availability,
	};

	relay_message(ctx, job_data, peer_views, validator, msg).await;

	metrics.on_own_bitfield_gossipped();
}

/// Distribute a given valid and signature checked bitfield message.
///
/// Can be originated by another subsystem or received via network from another peer.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn relay_message<Context>(
	ctx: &mut Context,
	job_data: &mut PerRelayParentData,
	peer_views: &mut HashMap<PeerId, View>,
	validator: ValidatorId,
	message: BitfieldGossipMessage,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	// notify the overseer about a new and valid signed bitfield
	ctx.send_message(AllMessages::Provisioner(
		ProvisionerMessage::ProvisionableData(
			message.relay_parent,
			ProvisionableData::Bitfield(
				message.relay_parent,
				message.signed_availability.clone(),
			),
		),
	))
	.await;

	// pass on the bitfield distribution to all interested peers
	let interested_peers = peer_views
		.iter()
		.filter_map(|(peer, view)| {
			// check interest in the peer in this message's relay parent
			if view.contains(&message.relay_parent) {
				let message_needed = job_data.message_from_validator_needed_by_peer(&peer, &validator);
				// track the message as sent for this peer
				job_data.message_sent_to_peer
					.entry(peer.clone())
					.or_default()
					.insert(validator.clone());

				if message_needed {
					Some(peer.clone())
				} else {
					None
				}
			} else {
				None
			}
		})
		.collect::<Vec<PeerId>>();

	if interested_peers.is_empty() {
		tracing::trace!(
			target: LOG_TARGET,
			relay_parent = %message.relay_parent,
			"no peers are interested in gossip for relay parent",
		);
	} else {
		ctx.send_message(AllMessages::NetworkBridge(
			NetworkBridgeMessage::SendValidationMessage(
				interested_peers,
				message.into_validation_protocol(),
			),
		))
		.await;
	}
}

/// Handle an incoming message from a peer.
#[tracing::instrument(level = "trace", skip(ctx, metrics), fields(subsystem = LOG_TARGET))]
async fn process_incoming_peer_message<Context>(
	ctx: &mut Context,
	state: &mut ProtocolState,
	metrics: &Metrics,
	origin: PeerId,
	message: BitfieldGossipMessage,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	// we don't care about this, not part of our view.
	if !state.view.contains(&message.relay_parent) {
		modify_reputation(ctx, origin, COST_NOT_IN_VIEW).await;
		return;
	}

	// Ignore anything the overseer did not tell this subsystem to work on.
	let mut job_data = state.per_relay_parent.get_mut(&message.relay_parent);
	let job_data: &mut _ = if let Some(ref mut job_data) = job_data {
		job_data
	} else {
		modify_reputation(ctx, origin, COST_NOT_IN_VIEW).await;
		return;
	};

	let validator_set = &job_data.validator_set;
	if validator_set.is_empty() {
		tracing::trace!(
			target: LOG_TARGET,
			relay_parent = %message.relay_parent,
			"Validator set is empty",
		);
		modify_reputation(ctx, origin, COST_MISSING_PEER_SESSION_KEY).await;
		return;
	}

	// Use the (untrusted) validator index provided by the signed payload
	// and see if that one actually signed the availability bitset.
	let signing_context = job_data.signing_context.clone();
	let validator_index = message.signed_availability.validator_index() as usize;
	let validator = if let Some(validator) = validator_set.get(validator_index) {
		validator.clone()
	} else {
		modify_reputation(ctx, origin, COST_VALIDATOR_INDEX_INVALID).await;
		return;
	};

	// Check if the peer already sent us a message for the validator denoted in the message earlier.
	// Must be done after validator index verification, in order to avoid storing an unbounded
	// number of set entries.
	let received_set = job_data
		.message_received_from_peer
		.entry(origin.clone())
		.or_default();

	if !received_set.contains(&validator) {
		received_set.insert(validator.clone());
	} else {
		modify_reputation(ctx, origin, COST_PEER_DUPLICATE_MESSAGE).await;
		return;
	};

	if message
		.signed_availability
		.check_signature(&signing_context, &validator)
		.is_ok()
	{
		metrics.on_bitfield_received();
		let one_per_validator = &mut (job_data.one_per_validator);

		// only relay_message a message of a validator once
		if one_per_validator.get(&validator).is_some() {
			tracing::trace!(
				target: LOG_TARGET,
				validator_index,
				"already received a message for validator",
			);
			modify_reputation(ctx, origin, BENEFIT_VALID_MESSAGE).await;
			return;
		}
		one_per_validator.insert(validator.clone(), message.clone());

		relay_message(ctx, job_data, &mut state.peer_views, validator, message).await;

		modify_reputation(ctx, origin, BENEFIT_VALID_MESSAGE_FIRST).await
	} else {
		modify_reputation(ctx, origin, COST_SIGNATURE_INVALID).await
	}
}

/// Deal with network bridge updates and track what needs to be tracked
/// which depends on the message type received.
#[tracing::instrument(level = "trace", skip(ctx, metrics), fields(subsystem = LOG_TARGET))]
async fn handle_network_msg<Context>(
	ctx: &mut Context,
	state: &mut ProtocolState,
	metrics: &Metrics,
	bridge_message: NetworkBridgeEvent<protocol_v1::BitfieldDistributionMessage>,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	let _timer = metrics.time_handle_network_msg();

	match bridge_message {
		NetworkBridgeEvent::PeerConnected(peerid, _role) => {
			// insert if none already present
			state.peer_views.entry(peerid).or_default();
		}
		NetworkBridgeEvent::PeerDisconnected(peerid) => {
			// get rid of superfluous data
			state.peer_views.remove(&peerid);
		}
		NetworkBridgeEvent::PeerViewChange(peerid, view) => {
			handle_peer_view_change(ctx, state, peerid, view).await;
		}
		NetworkBridgeEvent::OurViewChange(view) => {
			handle_our_view_change(state, view);
		}
		NetworkBridgeEvent::PeerMessage(remote, message) => {
			match message {
				protocol_v1::BitfieldDistributionMessage::Bitfield(relay_parent, bitfield) => {
					tracing::trace!(target: LOG_TARGET, peer_id = %remote, "received bitfield gossip from peer");
					let gossiped_bitfield = BitfieldGossipMessage {
						relay_parent,
						signed_availability: bitfield,
					};
					process_incoming_peer_message(ctx, state, metrics, remote, gossiped_bitfield).await;
				}
			}
		}
	}
}

/// Handle the changes necassary when our view changes.
#[tracing::instrument(level = "trace", fields(subsystem = LOG_TARGET))]
fn handle_our_view_change(state: &mut ProtocolState, view: View) {
	let old_view = std::mem::replace(&mut (state.view), view);

	for added in state.view.difference(&old_view) {
		if !state.per_relay_parent.contains_key(&added) {
			tracing::warn!(
				target: LOG_TARGET,
				added = %added,
				"Our view contains {} but the overseer never told use we should work on this",
				&added
			);
		}
	}
	for removed in old_view.difference(&state.view) {
		// cleanup relay parents we are not interested in any more
		let _ = state.per_relay_parent.remove(&removed);
	}
}


// Send the difference between two views which were not sent
// to that particular peer.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn handle_peer_view_change<Context>(
	ctx: &mut Context,
	state: &mut ProtocolState,
	origin: PeerId,
	view: View,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	let added = state.peer_views.entry(origin.clone()).or_default().replace_difference(view).cloned().collect::<Vec<_>>();

	// Send all messages we've seen before and the peer is now interested
	// in to that peer.

	let delta_set: Vec<(ValidatorId, BitfieldGossipMessage)> = added
		.into_iter()
		.filter_map(|new_relay_parent_interest| {
			if let Some(job_data) = (&*state).per_relay_parent.get(&new_relay_parent_interest) {
				// Send all jointly known messages for a validator (given the current relay parent)
				// to the peer `origin`...
				let one_per_validator = job_data.one_per_validator.clone();
				let origin = origin.clone();
				Some(
					one_per_validator
						.into_iter()
						.filter(move |(validator, _message)| {
							// ..except for the ones the peer already has.
							job_data.message_from_validator_needed_by_peer(&origin, validator)
						}),
				)
			} else {
				// A relay parent is in the peers view, which is not in ours, ignore those.
				None
			}
		})
		.flatten()
		.collect();

	for (validator, message) in delta_set.into_iter() {
		send_tracked_gossip_message(ctx, state, origin.clone(), validator, message).await;
	}
}

/// Send a gossip message and track it in the per relay parent data.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn send_tracked_gossip_message<Context>(
	ctx: &mut Context,
	state: &mut ProtocolState,
	dest: PeerId,
	validator: ValidatorId,
	message: BitfieldGossipMessage,
)
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	let job_data = if let Some(job_data) = state.per_relay_parent.get_mut(&message.relay_parent) {
		job_data
	} else {
		return;
	};

	job_data.message_sent_to_peer
		.entry(dest.clone())
		.or_default()
		.insert(validator.clone());

	ctx.send_message(AllMessages::NetworkBridge(
		NetworkBridgeMessage::SendValidationMessage(
			vec![dest],
			message.into_validation_protocol(),
		),
	)).await;
}

impl<C> Subsystem<C> for BitfieldDistribution
where
	C: SubsystemContext<Message = BitfieldDistributionMessage> + Sync + Send,
{
	fn start(self, ctx: C) -> SpawnedSubsystem {
		let future = self.run(ctx)
			.map(|_| Ok(()))
			.boxed();

		SpawnedSubsystem {
			name: "bitfield-distribution-subsystem",
			future,
		}
	}
}

/// Query our validator set and signing context for a particular relay parent.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn query_basics<Context>(
	ctx: &mut Context,
	relay_parent: Hash,
) -> SubsystemResult<Option<(Vec<ValidatorId>, SigningContext)>>
where
	Context: SubsystemContext<Message = BitfieldDistributionMessage>,
{
	let (validators_tx, validators_rx) = oneshot::channel();
	let (session_tx, session_rx) = oneshot::channel();

	let query_validators = AllMessages::RuntimeApi(RuntimeApiMessage::Request(
		relay_parent.clone(),
		RuntimeApiRequest::Validators(validators_tx),
	));

	let query_signing = AllMessages::RuntimeApi(RuntimeApiMessage::Request(
		relay_parent.clone(),
		RuntimeApiRequest::SessionIndexForChild(session_tx),
	));

	ctx.send_messages(std::iter::once(query_validators).chain(std::iter::once(query_signing)))
		.await;

	match (validators_rx.await?, session_rx.await?) {
		(Ok(v), Ok(s)) => Ok(Some((
			v,
			SigningContext { parent_hash: relay_parent, session_index: s },
		))),
		(Err(e), _) | (_, Err(e)) => {
			tracing::warn!(target: LOG_TARGET, err = ?e, "Failed to fetch basics from runtime API");
			Ok(None)
		}
	}
}

#[derive(Clone)]
struct MetricsInner {
	gossipped_own_availability_bitfields: prometheus::Counter<prometheus::U64>,
	received_availability_bitfields: prometheus::Counter<prometheus::U64>,
	active_leaves_update: prometheus::Histogram,
	handle_bitfield_distribution: prometheus::Histogram,
	handle_network_msg: prometheus::Histogram,
}

/// Bitfield Distribution metrics.
#[derive(Default, Clone)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_own_bitfield_gossipped(&self) {
		if let Some(metrics) = &self.0 {
			metrics.gossipped_own_availability_bitfields.inc();
		}
	}

	fn on_bitfield_received(&self) {
		if let Some(metrics) = &self.0 {
			metrics.received_availability_bitfields.inc();
		}
	}

	/// Provide a timer for `active_leaves_update` which observes on drop.
	fn time_active_leaves_update(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.active_leaves_update.start_timer())
	}

	/// Provide a timer for `handle_bitfield_distribution` which observes on drop.
	fn time_handle_bitfield_distribution(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.handle_bitfield_distribution.start_timer())
	}

	/// Provide a timer for `handle_network_msg` which observes on drop.
	fn time_handle_network_msg(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.handle_network_msg.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			gossipped_own_availability_bitfields: prometheus::register(
				prometheus::Counter::new(
					"parachain_gossipped_own_availabilty_bitfields_total",
					"Number of own availability bitfields sent to other peers."
				)?,
				registry,
			)?,
			received_availability_bitfields: prometheus::register(
				prometheus::Counter::new(
					"parachain_received_availabilty_bitfields_total",
					"Number of valid availability bitfields received from other peers."
				)?,
				registry,
			)?,
			active_leaves_update: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_bitfield_distribution_active_leaves_update",
						"Time spent within `bitfield_distribution::active_leaves_update`",
					)
				)?,
				registry,
			)?,
			handle_bitfield_distribution: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_bitfield_distribution_handle_bitfield_distribution",
						"Time spent within `bitfield_distribution::handle_bitfield_distribution`",
					)
				)?,
				registry,
			)?,
			handle_network_msg: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_bitfield_distribution_handle_network_msg",
						"Time spent within `bitfield_distribution::handle_network_msg`",
					)
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}
