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

use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::task::Poll;

use futures::{
	StreamExt,
	FutureExt,
	channel::oneshot,
	future::BoxFuture,
	stream::FuturesUnordered,
};

use indracore_primitives::v1::{
	Id as ParaId, CandidateReceipt, CollatorId, Hash, PoV,
};
use indracore_subsystem::{
	FromOverseer, OverseerSignal, SubsystemContext,
	messages::{
		AllMessages, CandidateSelectionMessage, CollatorProtocolMessage, NetworkBridgeMessage,
	},
};
use indracore_node_network_protocol::{
	v1 as protocol_v1, View, PeerId, ReputationChange as Rep, RequestId,
	NetworkBridgeEvent,
};
use indracore_node_subsystem_util::{
	TimeoutExt as _,
	metrics::{self, prometheus},
};

use super::{modify_reputation, LOG_TARGET, Result};

const COST_UNEXPECTED_MESSAGE: Rep = Rep::new(-10, "An unexpected message");
const COST_REQUEST_TIMED_OUT: Rep = Rep::new(-20, "A collation request has timed out");
const COST_REPORT_BAD: Rep = Rep::new(-50, "A collator was reported by another subsystem");
const BENEFIT_NOTIFY_GOOD: Rep = Rep::new(50, "A collator was noted good by another subsystem");

#[derive(Clone, Default)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_request(&self, succeeded: std::result::Result<(), ()>) {
		if let Some(metrics) = &self.0 {
			match succeeded {
				Ok(()) => metrics.collation_requests.with_label_values(&["succeeded"]).inc(),
				Err(()) => metrics.collation_requests.with_label_values(&["failed"]).inc(),
			}
		}
	}

	/// Provide a timer for `process_msg` which observes on drop.
	fn time_process_msg(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.process_msg.start_timer())
	}

	/// Provide a timer for `handle_collation_request_result` which observes on drop.
	fn time_handle_collation_request_result(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.handle_collation_request_result.start_timer())
	}
}

#[derive(Clone)]
struct MetricsInner {
	collation_requests: prometheus::CounterVec<prometheus::U64>,
	process_msg: prometheus::Histogram,
	handle_collation_request_result: prometheus::Histogram,
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry)
		-> std::result::Result<Self, prometheus::PrometheusError>
	{
		let metrics = MetricsInner {
			collation_requests: prometheus::register(
				prometheus::CounterVec::new(
					prometheus::Opts::new(
						"parachain_collation_requests_total",
						"Number of collations requested from Collators.",
					),
					&["success"],
				)?,
				registry,
			)?,
			process_msg: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_collator_protocol_validator_process_msg",
						"Time spent within `collator_protocol_validator::process_msg`",
					)
				)?,
				registry,
			)?,
			handle_collation_request_result: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_collator_protocol_validator_handle_collation_request_result",
						"Time spent within `collator_protocol_validator::handle_collation_request_result`",
					)
				)?,
				registry,
			)?,
		};

		Ok(Metrics(Some(metrics)))
	}
}

#[derive(Debug)]
enum CollationRequestResult {
	Received(RequestId),
	Timeout(RequestId),
}

/// A Future representing an ongoing collation request.
/// It may timeout or end in a graceful fashion if a requested
/// collation has been received sucessfully or chain has moved on.
struct CollationRequest {
	// The response for this request has been received successfully or
	// chain has moved forward and this request is no longer relevant.
	received: oneshot::Receiver<()>,

	// The timeout of this request.
	timeout: Duration,

	// The id of this request.
	request_id: RequestId,
}

impl CollationRequest {
	async fn wait(self) -> CollationRequestResult {
		use CollationRequestResult::*;

		let CollationRequest {
			received,
			timeout,
			request_id,
		} = self;

		match received.timeout(timeout).await {
			None => Timeout(request_id),
			Some(_) => Received(request_id),
		}
	}
}

struct PerRequest {
	// The sender side to signal the `CollationRequest` to resolve successfully.
	received: oneshot::Sender<()>,

	// Send result here.
	result: oneshot::Sender<(CandidateReceipt, PoV)>,
}

/// All state relevant for the validator side of the protocol lives here.
#[derive(Default)]
struct State {
	/// Our own view.
	view: View,

	/// Track all active collators and their views.
	peer_views: HashMap<PeerId, View>,

	/// Peers that have declared themselves as collators.
	known_collators: HashMap<PeerId, CollatorId>,

	/// Advertisements received from collators. We accept one advertisement
	/// per collator per source per relay-parent.
	advertisements: HashMap<PeerId, HashSet<(ParaId, Hash)>>,

	/// Derive RequestIds from this.
	next_request_id: RequestId,

	/// The collations we have requested by relay parent and para id.
	///
	/// For each relay parent and para id we may be connected to a number
	/// of collators each of those may have advertised a different collation.
	/// So we group such cases here.
	requested_collations: HashMap<(Hash, ParaId, PeerId), RequestId>,

	/// Housekeeping handles we need to have per request to:
	///  - cancel ongoing requests
	///  - reply with collations to other subsystems.
	requests_info: HashMap<RequestId, PerRequest>,

	/// Collation requests that are currently in progress.
	requests_in_progress: FuturesUnordered<BoxFuture<'static, CollationRequestResult>>,

	/// Delay after which a collation request would time out.
	request_timeout: Duration,

	/// Possessed collations.
	collations: HashMap<(Hash, ParaId), Vec<(CollatorId, CandidateReceipt, PoV)>>,

	/// Leaves have recently moved out of scope.
	/// These are looked into when we receive previously requested collations that we
	/// are no longer interested in.
	recently_removed_heads: HashSet<Hash>,

	/// Metrics.
	metrics: Metrics,
}

/// Another subsystem has requested to fetch collations on a particular leaf for some para.
#[tracing::instrument(level = "trace", skip(ctx, state, tx), fields(subsystem = LOG_TARGET))]
async fn fetch_collation<Context>(
	ctx: &mut Context,
	state: &mut State,
	relay_parent: Hash,
	collator_id: CollatorId,
	para_id: ParaId,
	tx: oneshot::Sender<(CandidateReceipt, PoV)>
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	// First take a look if we have already stored some of the relevant collations.
	if let Some(collations) = state.collations.get(&(relay_parent, para_id)) {
		for collation in collations.iter() {
			if collation.0 == collator_id {
				if let Err(e) = tx.send((collation.1.clone(), collation.2.clone())) {
					// We do not want this to be fatal because the receving subsystem
					// may have closed the results channel for some reason.
					tracing::trace!(
						target: LOG_TARGET,
						err = ?e,
						"Failed to send collation",
					);
				}
				return;
			}
		}
	}

	// Dodge multiple references to `state`.
	let mut relevant_advertiser = None;

	// Has the collator in question advertised a relevant collation?
	for (k, v) in state.advertisements.iter() {
		if v.contains(&(para_id, relay_parent)) {
			if state.known_collators.get(k) == Some(&collator_id) {
				relevant_advertiser = Some(k.clone());
			}
		}
	}

	// Request the collation.
	// Assume it is `request_collation`'s job to check and ignore duplicate requests.
	if let Some(relevant_advertiser) = relevant_advertiser {
		request_collation(ctx, state, relay_parent, para_id, relevant_advertiser, tx).await;
	}
}

/// Report a collator for some malicious actions.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn report_collator<Context>(
	ctx: &mut Context,
	state: &mut State,
	id: CollatorId,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	// Since we have a one way map of PeerId -> CollatorId we have to
	// iterate here. Since a huge amount of peers is not expected this
	// is a tolerable thing to do.
	for (k, v) in state.known_collators.iter() {
		if *v == id {
			modify_reputation(ctx, k.clone(), COST_REPORT_BAD).await;
		}
	}
}

/// Some other subsystem has reported a collator as a good one, bump reputation.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn note_good_collation<Context>(
	ctx: &mut Context,
	state: &mut State,
	id: CollatorId,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	for (peer_id, collator_id) in state.known_collators.iter() {
		if id == *collator_id {
			modify_reputation(ctx, peer_id.clone(), BENEFIT_NOTIFY_GOOD).await;
		}
	}
}

/// A peer's view has changed. A number of things should be done:
///  - Ongoing collation requests have to be cancelled.
///  - Advertisements by this peer that are no longer relevant have to be removed.
#[tracing::instrument(level = "trace", skip(state), fields(subsystem = LOG_TARGET))]
async fn handle_peer_view_change(
	state: &mut State,
	peer_id: PeerId,
	view: View,
) -> Result<()> {
	let current = state.peer_views.entry(peer_id.clone()).or_default();

	let removed: Vec<_> = current.difference(&view).cloned().collect();

	*current = view;

	if let Some(advertisements) = state.advertisements.get_mut(&peer_id) {
		advertisements.retain(|(_, relay_parent)| !removed.contains(relay_parent));
	}

	let mut requests_to_cancel = Vec::new();

	for removed in removed.into_iter() {
		state.requested_collations.retain(|k, v| {
			if k.0 == removed {
				requests_to_cancel.push(*v);
				false
			} else {
				true
			}
		});
	}

	for r in requests_to_cancel.into_iter() {
		if let Some(per_request) = state.requests_info.remove(&r) {
			per_request.received.send(()).map_err(|_| oneshot::Canceled)?;
		}
	}

	Ok(())
}

/// We have received a collation.
///  - Cancel all ongoing requests
///  - Reply to interested parties if any
///  - Store collation.
#[tracing::instrument(level = "trace", skip(ctx, state, pov), fields(subsystem = LOG_TARGET))]
async fn received_collation<Context>(
	ctx: &mut Context,
	state: &mut State,
	origin: PeerId,
	request_id: RequestId,
	receipt: CandidateReceipt,
	pov: PoV,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	let relay_parent = receipt.descriptor.relay_parent;
	let para_id = receipt.descriptor.para_id;

	if let Some(id) = state.requested_collations.remove(
		&(relay_parent, para_id, origin.clone())
	) {
		if id == request_id {
			if let Some(per_request) = state.requests_info.remove(&id) {
				let _ = per_request.received.send(());
				if let Some(collator_id) = state.known_collators.get(&origin) {
					let _ = per_request.result.send((receipt.clone(), pov.clone()));
					state.metrics.on_request(Ok(()));

					state.collations
						.entry((relay_parent, para_id))
						.or_default()
						.push((collator_id.clone(), receipt, pov));
				}
			}
		}
	} else {
		// If this collation is not just a delayed one that we were expecting,
		// but our view has moved on, in that case modify peer's reputation.
		if !state.recently_removed_heads.contains(&relay_parent) {
			modify_reputation(ctx, origin, COST_UNEXPECTED_MESSAGE).await;
		}
	}
}

/// Request a collation from the network.
/// This function will
///  - Check for duplicate requests.
///  - Check if the requested collation is in our view.
///  - Update PerRequest records with the `result` field if necessary.
/// And as such invocations of this function may rely on that.
#[tracing::instrument(level = "trace", skip(ctx, state, result), fields(subsystem = LOG_TARGET))]
async fn request_collation<Context>(
	ctx: &mut Context,
	state: &mut State,
	relay_parent: Hash,
	para_id: ParaId,
	peer_id: PeerId,
	result: oneshot::Sender<(CandidateReceipt, PoV)>,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	if !state.view.contains(&relay_parent) {
		tracing::trace!(
			target: LOG_TARGET,
			peer_id = %peer_id,
			para_id = %para_id,
			relay_parent = %relay_parent,
			"collation is no longer in view",
		);
		return;
	}

	if state.requested_collations.contains_key(&(relay_parent, para_id.clone(), peer_id.clone())) {
		tracing::trace!(
			target: LOG_TARGET,
			peer_id = %peer_id,
			para_id = %para_id,
			relay_parent = %relay_parent,
			"collation has already been requested",
		);
		return;
	}

	let request_id = state.next_request_id;
	state.next_request_id += 1;

	let (tx, rx) = oneshot::channel();

	let per_request = PerRequest {
		received: tx,
		result,
	};

	let request = CollationRequest {
		received: rx,
		timeout: state.request_timeout,
		request_id,
	};

	state.requested_collations.insert((relay_parent, para_id.clone(), peer_id.clone()), request_id);

	state.requests_info.insert(request_id, per_request);

	state.requests_in_progress.push(request.wait().boxed());

	let wire_message = protocol_v1::CollatorProtocolMessage::RequestCollation(
		request_id,
		relay_parent,
		para_id,
	);

	ctx.send_message(AllMessages::NetworkBridge(
		NetworkBridgeMessage::SendCollationMessage(
			vec![peer_id],
			protocol_v1::CollationProtocol::CollatorProtocol(wire_message),
		)
	)).await;
}

/// Notify `CandidateSelectionSubsystem` that a collation has been advertised.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn notify_candidate_selection<Context>(
	ctx: &mut Context,
	collator: CollatorId,
	relay_parent: Hash,
	para_id: ParaId,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	ctx.send_message(AllMessages::CandidateSelection(
		CandidateSelectionMessage::Collation(
			relay_parent,
			para_id,
			collator,
		)
	)).await;
}

/// Networking message has been received.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn process_incoming_peer_message<Context>(
	ctx: &mut Context,
	state: &mut State,
	origin: PeerId,
	msg: protocol_v1::CollatorProtocolMessage,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	use protocol_v1::CollatorProtocolMessage::*;

	match msg {
		Declare(id) => {
			state.known_collators.insert(origin.clone(), id);
			state.peer_views.entry(origin).or_default();
		}
		AdvertiseCollation(relay_parent, para_id) => {
			state.advertisements.entry(origin.clone()).or_default().insert((para_id, relay_parent));

			if let Some(collator) = state.known_collators.get(&origin) {
				notify_candidate_selection(ctx, collator.clone(), relay_parent, para_id).await;
			}
		}
		RequestCollation(_, _, _) => {
			// This is a validator side of the protocol, collation requests are not expected here.
			modify_reputation(ctx, origin, COST_UNEXPECTED_MESSAGE).await;
		}
		Collation(request_id, receipt, pov) => {
			received_collation(ctx, state, origin, request_id, receipt, pov).await;
		}
	}
}

/// A leaf has become inactive so we want to
///   - Cancel all ongoing collation requests that are on top of that leaf.
///   - Remove all stored collations relevant to that leaf.
#[tracing::instrument(level = "trace", skip(state), fields(subsystem = LOG_TARGET))]
async fn remove_relay_parent(
	state: &mut State,
	relay_parent: Hash,
) -> Result<()> {
	let mut remove_these = Vec::new();

	state.requested_collations.retain(|k, v| {
		if k.0 == relay_parent {
			remove_these.push(*v);
		}
		k.0 != relay_parent
	});

	for id in remove_these.into_iter() {
		if let Some(info) = state.requests_info.remove(&id) {
			info.received.send(()).map_err(|_| oneshot::Canceled)?;
		}
	}

	state.collations.retain(|k, _| k.0 != relay_parent);

	Ok(())
}

/// Our view has changed.
#[tracing::instrument(level = "trace", skip(state), fields(subsystem = LOG_TARGET))]
async fn handle_our_view_change(
	state: &mut State,
	view: View,
) -> Result<()> {
	let old_view = std::mem::replace(&mut (state.view), view);

	let removed = old_view
		.difference(&state.view)
		.cloned()
		.collect::<Vec<_>>();

	// Update the set of recently removed chain heads.
	state.recently_removed_heads.clear();

	for removed in removed.into_iter() {
		state.recently_removed_heads.insert(removed.clone());
		remove_relay_parent(state, removed).await?;
	}

	Ok(())
}

/// A request has timed out.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn request_timed_out<Context>(
	ctx: &mut Context,
	state: &mut State,
	id: RequestId,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	state.metrics.on_request(Err(()));

	// We have to go backwards in the map, again.
	if let Some(key) = find_val_in_map(&state.requested_collations, &id) {
		if let Some(_) = state.requested_collations.remove(&key) {
			if let Some(_) = state.requests_info.remove(&id) {
				let peer_id = key.2;

				modify_reputation(ctx, peer_id, COST_REQUEST_TIMED_OUT).await;
			}
		}
	}
}

/// Bridge event switch.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn handle_network_msg<Context>(
	ctx: &mut Context,
	state: &mut State,
	bridge_message: NetworkBridgeEvent<protocol_v1::CollatorProtocolMessage>,
) -> Result<()>
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	use NetworkBridgeEvent::*;

	match bridge_message {
		PeerConnected(_id, _role) => {
			// A peer has connected. Until it issues a `Declare` message we do not
			// want to track it's view or take any other actions.
		},
		PeerDisconnected(peer_id) => {
			state.peer_views.remove(&peer_id);
		},
		PeerViewChange(peer_id, view) => {
			handle_peer_view_change(state, peer_id, view).await?;
		},
		OurViewChange(view) => {
			handle_our_view_change(state, view).await?;
		},
		PeerMessage(remote, msg) => {
			process_incoming_peer_message(ctx, state, remote, msg).await;
		}
	}

	Ok(())
}

/// The main message receiver switch.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn process_msg<Context>(
	ctx: &mut Context,
	msg: CollatorProtocolMessage,
	state: &mut State,
)
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	use CollatorProtocolMessage::*;

	let _timer = state.metrics.time_process_msg();

	match msg {
		CollateOn(id) => {
			tracing::warn!(
				target: LOG_TARGET,
				para_id = %id,
				"CollateOn message is not expected on the validator side of the protocol",
			);
		}
		DistributeCollation(_, _) => {
			tracing::warn!(
				target: LOG_TARGET,
				"DistributeCollation message is not expected on the validator side of the protocol",
			);
		}
		FetchCollation(relay_parent, collator_id, para_id, tx) => {
			fetch_collation(ctx, state, relay_parent, collator_id, para_id, tx).await;
		}
		ReportCollator(id) => {
			report_collator(ctx, state, id).await;
		}
		NoteGoodCollation(id) => {
			note_good_collation(ctx, state, id).await;
		}
		NetworkBridgeUpdateV1(event) => {
			if let Err(e) = handle_network_msg(
				ctx,
				state,
				event,
			).await {
				tracing::warn!(
					target: LOG_TARGET,
					err = ?e,
					"Failed to handle incoming network message",
				);
			}
		}
	}
}

/// The main run loop.
#[tracing::instrument(skip(ctx, metrics), fields(subsystem = LOG_TARGET))]
pub(crate) async fn run<Context>(
	mut ctx: Context,
	request_timeout: Duration,
	metrics: Metrics,
	) -> Result<()>
where
	Context: SubsystemContext<Message = CollatorProtocolMessage>
{
	use FromOverseer::*;
	use OverseerSignal::*;

	let mut state = State {
		request_timeout,
		metrics,
		..Default::default()
	};

	loop {
		if let Poll::Ready(msg) = futures::poll!(ctx.recv()) {
			let msg = msg?;
			tracing::trace!(target: LOG_TARGET, msg = ?msg, "received a message");

			match msg {
				Communication { msg } => process_msg(&mut ctx, msg, &mut state).await,
				Signal(BlockFinalized(_)) => {}
				Signal(ActiveLeaves(_)) => {}
				Signal(Conclude) => { break }
			}
			continue;
		}

		while let Poll::Ready(Some(request)) = futures::poll!(state.requests_in_progress.next()) {
			let _timer = state.metrics.time_handle_collation_request_result();

			// Request has timed out, we need to penalize the collator and re-send the request
			// if the chain has not moved on yet.
			match request {
				CollationRequestResult::Timeout(id) => {
					tracing::trace!(target: LOG_TARGET, id, "request timed out");
					request_timed_out(&mut ctx, &mut state, id).await;
				}
				CollationRequestResult::Received(id) => {
					state.requests_info.remove(&id);
				}
			}
		}

		futures::pending!();
	}

	Ok(())
}

fn find_val_in_map<K: Clone, V: Eq>(map: &HashMap<K, V>, val: &V) -> Option<K> {
	map
		.iter()
		.find_map(|(k, v)| if v == val { Some(k.clone()) } else { None })
}