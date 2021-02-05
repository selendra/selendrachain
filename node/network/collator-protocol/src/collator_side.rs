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

use super::{Result, LOG_TARGET};

use futures::{select, FutureExt};

use indracore_node_network_protocol::{
    v1 as protocol_v1, NetworkBridgeEvent, PeerId, RequestId, View,
};
use indracore_node_subsystem_util::{
    metrics::{self, prometheus},
    request_availability_cores_ctx, request_validator_groups_ctx, request_validators_ctx,
    validator_discovery,
};
use indracore_primitives::v1::{
    CandidateReceipt, CollatorId, CoreIndex, CoreState, Hash, Id as ParaId, PoV, ValidatorId,
};
use indracore_subsystem::{
    messages::{AllMessages, CollatorProtocolMessage, NetworkBridgeMessage},
    FromOverseer, OverseerSignal, SubsystemContext,
};

#[derive(Clone, Default)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
    fn on_advertisment_made(&self) {
        if let Some(metrics) = &self.0 {
            metrics.advertisements_made.inc();
        }
    }

    fn on_collation_sent(&self) {
        if let Some(metrics) = &self.0 {
            metrics.collations_sent.inc();
        }
    }

    /// Provide a timer for handling `ConnectionRequest` which observes on drop.
    fn time_handle_connection_request(
        &self,
    ) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
        self.0
            .as_ref()
            .map(|metrics| metrics.handle_connection_request.start_timer())
    }

    /// Provide a timer for `process_msg` which observes on drop.
    fn time_process_msg(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
        self.0
            .as_ref()
            .map(|metrics| metrics.process_msg.start_timer())
    }
}

#[derive(Clone)]
struct MetricsInner {
    advertisements_made: prometheus::Counter<prometheus::U64>,
    collations_sent: prometheus::Counter<prometheus::U64>,
    handle_connection_request: prometheus::Histogram,
    process_msg: prometheus::Histogram,
}

impl metrics::Metrics for Metrics {
    fn try_register(
        registry: &prometheus::Registry,
    ) -> std::result::Result<Self, prometheus::PrometheusError> {
        let metrics = MetricsInner {
            advertisements_made: prometheus::register(
                prometheus::Counter::new(
                    "parachain_collation_advertisements_made_total",
                    "A number of collation advertisements sent to validators.",
                )?,
                registry,
            )?,
            collations_sent: prometheus::register(
                prometheus::Counter::new(
                    "parachain_collations_sent_total",
                    "A number of collations sent to validators.",
                )?,
                registry,
            )?,
            handle_connection_request: prometheus::register(
                prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                    "parachain_collator_protocol_collator_handle_connection_request",
                    "Time spent within `collator_protocol_collator::handle_connection_request`",
                ))?,
                registry,
            )?,
            process_msg: prometheus::register(
                prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                    "parachain_collator_protocol_collator_process_msg",
                    "Time spent within `collator_protocol_collator::process_msg`",
                ))?,
                registry,
            )?,
        };

        Ok(Metrics(Some(metrics)))
    }
}

/// The group of validators that is assigned to our para at a given point of time.
///
/// This structure is responsible for keeping track of which validators belong to a certain group for a para. It also
/// stores a mapping from [`PeerId`] to [`ValidatorId`] as we learn about it over the lifetime of this object. Besides
/// that it also keeps track to which validators we advertised our collation.
struct ValidatorGroup {
    /// All [`ValidatorId`]'s that are assigned to us in this group.
    validator_ids: HashSet<ValidatorId>,
    /// The mapping from [`PeerId`] to [`ValidatorId`]. This is filled over time as we learn the [`PeerId`]'s from the
    /// authority discovery. It is not ensured that this will contain *all* validators of this group.
    peer_ids: HashMap<PeerId, ValidatorId>,
    /// All [`ValidatorId`]'s of the current group to that we advertised our collation.
    advertised_to: HashSet<ValidatorId>,
}

impl ValidatorGroup {
    /// Returns `true` if we should advertise our collation to the given peer.
    fn should_advertise_to(&self, peer: &PeerId) -> bool {
        match self.peer_ids.get(peer) {
            Some(validator_id) => !self.advertised_to.contains(validator_id),
            None => false,
        }
    }

    /// Should be called after we advertised our collation to the given `peer` to keep track of it.
    fn advertised_to_peer(&mut self, peer: &PeerId) {
        if let Some(validator_id) = self.peer_ids.get(peer) {
            self.advertised_to.insert(validator_id.clone());
        }
    }

    /// Add a [`PeerId`] that belongs to the given [`ValidatorId`].
    ///
    /// This returns `true` if the given validator belongs to this group and we could insert its [`PeerId`].
    fn add_peer_id_for_validator(&mut self, peer_id: &PeerId, validator_id: &ValidatorId) -> bool {
        if !self.validator_ids.contains(validator_id) {
            false
        } else {
            self.peer_ids.insert(peer_id.clone(), validator_id.clone());
            true
        }
    }
}

impl From<HashSet<ValidatorId>> for ValidatorGroup {
    fn from(validator_ids: HashSet<ValidatorId>) -> Self {
        Self {
            validator_ids,
            peer_ids: HashMap::new(),
            advertised_to: HashSet::new(),
        }
    }
}

#[derive(Default)]
struct State {
    /// Our id.
    our_id: CollatorId,

    /// The para this collator is collating on.
    /// Starts as `None` and is updated with every `CollateOn` message.
    collating_on: Option<ParaId>,

    /// Track all active peers and their views
    /// to determine what is relevant to them.
    peer_views: HashMap<PeerId, View>,

    /// Our own view.
    view: View,

    /// Possessed collations.
    ///
    /// We will keep up to one local collation per relay-parent.
    collations: HashMap<Hash, (CandidateReceipt, PoV)>,

    /// Our validator groups per active leaf.
    our_validators_groups: HashMap<Hash, ValidatorGroup>,

    /// List of peers where we declared ourself as a collator.
    declared_at: HashSet<PeerId>,

    /// The connection requests to validators per relay parent.
    connection_requests: validator_discovery::ConnectionRequests,

    /// Metrics.
    metrics: Metrics,
}

impl State {
    /// Returns `true` if the given `peer` is interested in the leaf that is represented by `relay_parent`.
    fn peer_interested_in_leaf(&self, peer: &PeerId, relay_parent: &Hash) -> bool {
        self.peer_views
            .get(peer)
            .map(|v| v.contains(relay_parent))
            .unwrap_or(false)
    }
}

/// Distribute a collation.
///
/// Figure out the core our para is assigned to and the relevant validators.
/// Issue a connection request to these validators.
/// If the para is not scheduled or next up on any core, at the relay-parent,
/// or the relay-parent isn't in the active-leaves set, we ignore the message
/// as it must be invalid in that case - although this indicates a logic error
/// elsewhere in the node.
#[tracing::instrument(level = "trace", skip(ctx, state, pov), fields(subsystem = LOG_TARGET))]
async fn distribute_collation(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    id: ParaId,
    receipt: CandidateReceipt,
    pov: PoV,
) -> Result<()> {
    let relay_parent = receipt.descriptor.relay_parent;

    // This collation is not in the active-leaves set.
    if !state.view.contains(&relay_parent) {
        tracing::warn!(
            target: LOG_TARGET,
            relay_parent = %relay_parent,
            "distribute collation message parent is outside of our view",
        );

        return Ok(());
    }

    // We have already seen collation for this relay parent.
    if state.collations.contains_key(&relay_parent) {
        return Ok(());
    }

    // Determine which core the para collated-on is assigned to.
    // If it is not scheduled then ignore the message.
    let (our_core, num_cores) = match determine_core(ctx, id, relay_parent).await? {
        Some(core) => core,
        None => {
            tracing::warn!(
                target: LOG_TARGET,
                para_id = %id,
                relay_parent = %relay_parent,
                "looks like no core is assigned to {} at {}", id, relay_parent,
            );

            return Ok(());
        }
    };

    // Determine the group on that core and the next group on that core.
    let (current_validators, next_validators) =
        determine_our_validators(ctx, our_core, num_cores, relay_parent).await?;

    if current_validators.is_empty() && next_validators.is_empty() {
        tracing::warn!(
            target: LOG_TARGET,
            core = ?our_core,
            "there are no validators assigned to core",
        );

        return Ok(());
    }

    // Issue a discovery request for the validators of the current group and the next group.
    connect_to_validators(
        ctx,
        relay_parent,
        state,
        current_validators
            .union(&next_validators)
            .cloned()
            .collect(),
    )
    .await?;

    state
        .our_validators_groups
        .insert(relay_parent, current_validators.into());

    state.collations.insert(relay_parent, (receipt, pov));

    Ok(())
}

/// Get the Id of the Core that is assigned to the para being collated on if any
/// and the total number of cores.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn determine_core(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    para_id: ParaId,
    relay_parent: Hash,
) -> Result<Option<(CoreIndex, usize)>> {
    let cores = request_availability_cores_ctx(relay_parent, ctx)
        .await?
        .await??;

    for (idx, core) in cores.iter().enumerate() {
        if let CoreState::Scheduled(occupied) = core {
            if occupied.para_id == para_id {
                return Ok(Some(((idx as u32).into(), cores.len())));
            }
        }
    }

    Ok(None)
}

/// Figure out current and next group of validators assigned to the para being collated on.
///
/// Returns [`ValidatorId`]'s of current and next group as determined based on the `relay_parent`.
#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn determine_our_validators(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    core_index: CoreIndex,
    cores: usize,
    relay_parent: Hash,
) -> Result<(HashSet<ValidatorId>, HashSet<ValidatorId>)> {
    let groups = request_validator_groups_ctx(relay_parent, ctx).await?;

    let groups = groups.await??;

    let current_group_index = groups.1.group_for_core(core_index, cores);
    let current_validators = groups
        .0
        .get(current_group_index.0 as usize)
        .map(|v| v.as_slice())
        .unwrap_or_default();

    let next_group_idx = (current_group_index.0 as usize + 1) % groups.0.len();
    let next_validators = groups
        .0
        .get(next_group_idx)
        .map(|v| v.as_slice())
        .unwrap_or_default();

    let validators = request_validators_ctx(relay_parent, ctx).await?.await??;

    let current_validators = current_validators
        .iter()
        .map(|i| validators[*i as usize].clone())
        .collect();
    let next_validators = next_validators
        .iter()
        .map(|i| validators[*i as usize].clone())
        .collect();

    Ok((current_validators, next_validators))
}

/// Issue a `Declare` collation message to the given `peer`.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn declare(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    peer: PeerId,
) {
    let wire_message = protocol_v1::CollatorProtocolMessage::Declare(state.our_id.clone());

    ctx.send_message(AllMessages::NetworkBridge(
        NetworkBridgeMessage::SendCollationMessage(
            vec![peer],
            protocol_v1::CollationProtocol::CollatorProtocol(wire_message),
        ),
    ))
    .await;
}

/// Issue a connection request to a set of validators and
/// revoke the previous connection request.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn connect_to_validators(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    relay_parent: Hash,
    state: &mut State,
    validators: Vec<ValidatorId>,
) -> Result<()> {
    let request = validator_discovery::connect_to_validators(ctx, relay_parent, validators).await?;

    state.connection_requests.put(relay_parent, request);

    Ok(())
}

/// Advertise collation to the given `peer`.
///
/// This will only advertise a collation if there exists one for the given `relay_parent` and the given `peer` is
/// set as validator for our para at the given `relay_parent`.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn advertise_collation(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    relay_parent: Hash,
    peer: PeerId,
) {
    let collating_on = match state.collating_on {
        Some(collating_on) => collating_on,
        None => return,
    };

    let should_advertise = state
        .our_validators_groups
        .get(&relay_parent)
        .map(|g| g.should_advertise_to(&peer))
        .unwrap_or(false);

    if !state.collations.contains_key(&relay_parent) || !should_advertise {
        return;
    }

    let wire_message =
        protocol_v1::CollatorProtocolMessage::AdvertiseCollation(relay_parent, collating_on);

    ctx.send_message(AllMessages::NetworkBridge(
        NetworkBridgeMessage::SendCollationMessage(
            vec![peer.clone()],
            protocol_v1::CollationProtocol::CollatorProtocol(wire_message),
        ),
    ))
    .await;

    if let Some(validators) = state.our_validators_groups.get_mut(&relay_parent) {
        validators.advertised_to_peer(&peer);
    }

    state.metrics.on_advertisment_made();
}

/// The main incoming message dispatching switch.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn process_msg(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    msg: CollatorProtocolMessage,
) -> Result<()> {
    use CollatorProtocolMessage::*;

    let _timer = state.metrics.time_process_msg();

    match msg {
        CollateOn(id) => {
            state.collating_on = Some(id);
        }
        DistributeCollation(receipt, pov) => {
            match state.collating_on {
                Some(id) if receipt.descriptor.para_id != id => {
                    // If the ParaId of a collation requested to be distributed does not match
                    // the one we expect, we ignore the message.
                    tracing::warn!(
                        target: LOG_TARGET,
                        para_id = %receipt.descriptor.para_id,
                        collating_on = %id,
                        "DistributeCollation for unexpected para_id",
                    );
                }
                Some(id) => {
                    distribute_collation(ctx, state, id, receipt, pov).await?;
                }
                None => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        para_id = %receipt.descriptor.para_id,
                        "DistributeCollation message while not collating on any",
                    );
                }
            }
        }
        FetchCollation(_, _, _, _) => {
            tracing::warn!(
                target: LOG_TARGET,
                "FetchCollation message is not expected on the collator side of the protocol",
            );
        }
        ReportCollator(_) => {
            tracing::warn!(
                target: LOG_TARGET,
                "ReportCollator message is not expected on the collator side of the protocol",
            );
        }
        NoteGoodCollation(_) => {
            tracing::warn!(
                target: LOG_TARGET,
                "NoteGoodCollation message is not expected on the collator side of the protocol",
            );
        }
        NetworkBridgeUpdateV1(event) => {
            if let Err(e) = handle_network_msg(ctx, state, event).await {
                tracing::warn!(
                    target: LOG_TARGET,
                    err = ?e,
                    "Failed to handle incoming network message",
                );
            }
        }
    }

    Ok(())
}

/// Issue a response to a previously requested collation.
#[tracing::instrument(level = "trace", skip(ctx, state, pov), fields(subsystem = LOG_TARGET))]
async fn send_collation(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    request_id: RequestId,
    origin: PeerId,
    receipt: CandidateReceipt,
    pov: PoV,
) {
    let wire_message = protocol_v1::CollatorProtocolMessage::Collation(request_id, receipt, pov);

    ctx.send_message(AllMessages::NetworkBridge(
        NetworkBridgeMessage::SendCollationMessage(
            vec![origin],
            protocol_v1::CollationProtocol::CollatorProtocol(wire_message),
        ),
    ))
    .await;

    state.metrics.on_collation_sent();
}

/// A networking messages switch.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn handle_incoming_peer_message(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    origin: PeerId,
    msg: protocol_v1::CollatorProtocolMessage,
) -> Result<()> {
    use protocol_v1::CollatorProtocolMessage::*;

    match msg {
        Declare(_) => {
            tracing::warn!(
                target: LOG_TARGET,
                "Declare message is not expected on the collator side of the protocol",
            );
        }
        AdvertiseCollation(_, _) => {
            tracing::warn!(
                target: LOG_TARGET,
                "AdvertiseCollation message is not expected on the collator side of the protocol",
            );
        }
        RequestCollation(request_id, relay_parent, para_id) => match state.collating_on {
            Some(our_para_id) => {
                if our_para_id == para_id {
                    if let Some(collation) = state.collations.get(&relay_parent).cloned() {
                        send_collation(ctx, state, request_id, origin, collation.0, collation.1)
                            .await;
                    }
                } else {
                    tracing::warn!(
                        target: LOG_TARGET,
                        for_para_id = %para_id,
                        our_para_id = %our_para_id,
                        "received a RequestCollation for unexpected para_id",
                    );
                }
            }
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    for_para_id = %para_id,
                    "received a RequestCollation while not collating on any para",
                );
            }
        },
        Collation(_, _, _) => {
            tracing::warn!(
                target: LOG_TARGET,
                "Collation message is not expected on the collator side of the protocol",
            );
        }
    }

    Ok(())
}

/// Our view has changed.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn handle_peer_view_change(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    peer_id: PeerId,
    view: View,
) {
    let current = state.peer_views.entry(peer_id.clone()).or_default();

    let added: Vec<Hash> = view.difference(&*current).cloned().collect();

    *current = view;

    for added in added.into_iter() {
        advertise_collation(ctx, state, added, peer_id.clone()).await;
    }
}

/// A validator is connected.
///
/// `Declare` that we are a collator with a given `CollatorId`.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn handle_validator_connected(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    peer_id: PeerId,
    validator_id: ValidatorId,
    relay_parent: Hash,
) {
    let not_declared = state.declared_at.insert(peer_id.clone());

    if not_declared {
        declare(ctx, state, peer_id.clone()).await;
    }

    // Store the PeerId and find out if we should advertise to this peer.
    //
    // If this peer does not belong to the para validators, we also don't need to try to advertise our collation.
    let advertise = if let Some(validators) = state.our_validators_groups.get_mut(&relay_parent) {
        validators.add_peer_id_for_validator(&peer_id, &validator_id)
    } else {
        false
    };

    if advertise && state.peer_interested_in_leaf(&peer_id, &relay_parent) {
        advertise_collation(ctx, state, relay_parent, peer_id).await;
    }
}

/// Bridge messages switch.
#[tracing::instrument(level = "trace", skip(ctx, state), fields(subsystem = LOG_TARGET))]
async fn handle_network_msg(
    ctx: &mut impl SubsystemContext<Message = CollatorProtocolMessage>,
    state: &mut State,
    bridge_message: NetworkBridgeEvent<protocol_v1::CollatorProtocolMessage>,
) -> Result<()> {
    use NetworkBridgeEvent::*;

    match bridge_message {
        PeerConnected(_peer_id, _observed_role) => {
            // If it is possible that a disconnected validator would attempt a reconnect
            // it should be handled here.
        }
        PeerViewChange(peer_id, view) => {
            handle_peer_view_change(ctx, state, peer_id, view).await;
        }
        PeerDisconnected(peer_id) => {
            state.peer_views.remove(&peer_id);
            state.declared_at.remove(&peer_id);
        }
        OurViewChange(view) => {
            handle_our_view_change(state, view).await?;
        }
        PeerMessage(remote, msg) => {
            handle_incoming_peer_message(ctx, state, remote, msg).await?;
        }
    }

    Ok(())
}

/// Handles our view changes.
#[tracing::instrument(level = "trace", skip(state), fields(subsystem = LOG_TARGET))]
async fn handle_our_view_change(state: &mut State, view: View) -> Result<()> {
    for removed in state.view.difference(&view) {
        state.collations.remove(removed);
        state.our_validators_groups.remove(removed);
        state.connection_requests.remove(removed);
    }

    state.view = view;

    Ok(())
}

/// The collator protocol collator side main loop.
#[tracing::instrument(skip(ctx, metrics), fields(subsystem = LOG_TARGET))]
pub(crate) async fn run(
    mut ctx: impl SubsystemContext<Message = CollatorProtocolMessage>,
    our_id: CollatorId,
    metrics: Metrics,
) -> Result<()> {
    use FromOverseer::*;
    use OverseerSignal::*;

    let mut state = State {
        metrics,
        our_id,
        ..Default::default()
    };

    loop {
        select! {
            res = state.connection_requests.next().fuse() => {
                let _timer = state.metrics.time_handle_connection_request();

                handle_validator_connected(
                    &mut ctx,
                    &mut state,
                    res.peer_id,
                    res.validator_id,
                    res.relay_parent,
                ).await;
            },
            msg = ctx.recv().fuse() => match msg? {
                Communication { msg } => {
                    if let Err(e) = process_msg(&mut ctx, &mut state, msg).await {
                        tracing::warn!(target: LOG_TARGET, err = ?e, "Failed to process message");
                    }
                },
                Signal(ActiveLeaves(_update)) => {}
                Signal(BlockFinalized(_)) => {}
                Signal(Conclude) => return Ok(()),
            }
        }
    }
}
