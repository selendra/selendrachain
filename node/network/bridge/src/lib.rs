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

//! The Network Bridge Subsystem - protocol multiplexer for Indracore.

#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]

use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::BoxStream;
use parity_scale_codec::{Decode, Encode};

use sc_network::Event as NetworkEvent;

use indracore_node_network_protocol::{
    v1 as protocol_v1, NetworkBridgeEvent, ObservedRole, PeerId, PeerSet, ReputationChange, View,
};
use indracore_primitives::v1::{AuthorityDiscoveryId, Block, Hash};
use indracore_subsystem::messages::{
    AllMessages, AvailabilityDistributionMessage, BitfieldDistributionMessage,
    CollatorProtocolMessage, NetworkBridgeMessage, PoVDistributionMessage,
    StatementDistributionMessage,
};
use indracore_subsystem::{
    ActiveLeavesUpdate, FromOverseer, OverseerSignal, SpawnedSubsystem, Subsystem,
    SubsystemContext, SubsystemError, SubsystemResult,
};

use std::collections::{hash_map, HashMap};
use std::iter::ExactSizeIterator;
use std::pin::Pin;
use std::sync::Arc;

mod validator_discovery;

/// The maximum amount of heads a peer is allowed to have in their view at any time.
///
/// We use the same limit to compute the view sent to peers locally.
const MAX_VIEW_HEADS: usize = 5;

/// The protocol name for the validation peer-set.
pub const VALIDATION_PROTOCOL_NAME: &str = "/indracore/validation/1";
/// The protocol name for the collation peer-set.
pub const COLLATION_PROTOCOL_NAME: &str = "/indracore/collation/1";

const MALFORMED_MESSAGE_COST: ReputationChange =
    ReputationChange::new(-500, "Malformed Network-bridge message");
const UNCONNECTED_PEERSET_COST: ReputationChange =
    ReputationChange::new(-50, "Message sent to un-connected peer-set");
const MALFORMED_VIEW_COST: ReputationChange = ReputationChange::new(-500, "Malformed view");

// network bridge log target
const LOG_TARGET: &str = "network_bridge";

/// Messages received on the network.
#[derive(Debug, Encode, Decode, Clone)]
pub enum WireMessage<M> {
    /// A message from a peer on a specific protocol.
    #[codec(index = "1")]
    ProtocolMessage(M),
    /// A view update from a peer.
    #[codec(index = "2")]
    ViewUpdate(View),
}

/// Information about the notifications protocol. Should be used during network configuration
/// or shortly after startup to register the protocol with the network service.
pub fn notifications_protocol_info() -> Vec<std::borrow::Cow<'static, str>> {
    vec![
        VALIDATION_PROTOCOL_NAME.into(),
        COLLATION_PROTOCOL_NAME.into(),
    ]
}

/// An action to be carried out by the network.
#[derive(Debug, PartialEq)]
pub enum NetworkAction {
    /// Note a change in reputation for a peer.
    ReputationChange(PeerId, ReputationChange),
    /// Write a notification to a given peer on the given peer-set.
    WriteNotification(PeerId, PeerSet, Vec<u8>),
}

/// An abstraction over networking for the purposes of this subsystem.
pub trait Network: Send + 'static {
    /// Get a stream of all events occurring on the network. This may include events unrelated
    /// to the Indracore protocol - the user of this function should filter only for events related
    /// to the [`VALIDATION_PROTOCOL_NAME`](VALIDATION_PROTOCOL_NAME)
    /// or [`COLLATION_PROTOCOL_NAME`](COLLATION_PROTOCOL_NAME)
    fn event_stream(&mut self) -> BoxStream<'static, NetworkEvent>;

    /// Get access to an underlying sink for all network actions.
    fn action_sink<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Sink<NetworkAction, Error = SubsystemError> + Send + 'a>>;

    /// Report a given peer as either beneficial (+) or costly (-) according to the given scalar.
    fn report_peer(
        &mut self,
        who: PeerId,
        cost_benefit: ReputationChange,
    ) -> BoxFuture<SubsystemResult<()>> {
        async move {
            self.action_sink()
                .send(NetworkAction::ReputationChange(who, cost_benefit))
                .await
        }
        .boxed()
    }

    /// Write a notification to a peer on the given peer-set's protocol.
    fn write_notification(
        &mut self,
        who: PeerId,
        peer_set: PeerSet,
        message: Vec<u8>,
    ) -> BoxFuture<SubsystemResult<()>> {
        async move {
            self.action_sink()
                .send(NetworkAction::WriteNotification(who, peer_set, message))
                .await
        }
        .boxed()
    }
}

impl Network for Arc<sc_network::NetworkService<Block, Hash>> {
    fn event_stream(&mut self) -> BoxStream<'static, NetworkEvent> {
        sc_network::NetworkService::event_stream(self, "indracore-network-bridge").boxed()
    }

    #[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
    fn action_sink<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Sink<NetworkAction, Error = SubsystemError> + Send + 'a>> {
        use futures::task::{Context, Poll};

        // wrapper around a NetworkService to make it act like a sink.
        struct ActionSink<'b>(&'b sc_network::NetworkService<Block, Hash>);

        impl<'b> Sink<NetworkAction> for ActionSink<'b> {
            type Error = SubsystemError;

            fn poll_ready(self: Pin<&mut Self>, _: &mut Context) -> Poll<SubsystemResult<()>> {
                Poll::Ready(Ok(()))
            }

            fn start_send(self: Pin<&mut Self>, action: NetworkAction) -> SubsystemResult<()> {
                match action {
                    NetworkAction::ReputationChange(peer, cost_benefit) => {
                        tracing::debug!(
                            target: LOG_TARGET,
                            "Changing reputation: {:?} for {}",
                            cost_benefit,
                            peer
                        );
                        self.0.report_peer(peer, cost_benefit)
                    }
                    NetworkAction::WriteNotification(peer, peer_set, message) => match peer_set {
                        PeerSet::Validation => self.0.write_notification(
                            peer,
                            VALIDATION_PROTOCOL_NAME.into(),
                            message,
                        ),
                        PeerSet::Collation => {
                            self.0
                                .write_notification(peer, COLLATION_PROTOCOL_NAME.into(), message)
                        }
                    },
                }

                Ok(())
            }

            fn poll_flush(self: Pin<&mut Self>, _: &mut Context) -> Poll<SubsystemResult<()>> {
                Poll::Ready(Ok(()))
            }

            fn poll_close(self: Pin<&mut Self>, _: &mut Context) -> Poll<SubsystemResult<()>> {
                Poll::Ready(Ok(()))
            }
        }

        Box::pin(ActionSink(&**self))
    }
}

/// The network bridge subsystem.
pub struct NetworkBridge<N, AD> {
    network_service: N,
    authority_discovery_service: AD,
}

impl<N, AD> NetworkBridge<N, AD> {
    /// Create a new network bridge subsystem with underlying network service and authority discovery service.
    ///
    /// This assumes that the network service has had the notifications protocol for the network
    /// bridge already registered. See [`notifications_protocol_info`](notifications_protocol_info).
    pub fn new(network_service: N, authority_discovery_service: AD) -> Self {
        NetworkBridge {
            network_service,
            authority_discovery_service,
        }
    }
}

impl<Net, AD, Context> Subsystem<Context> for NetworkBridge<Net, AD>
where
    Net: Network + validator_discovery::Network,
    AD: validator_discovery::AuthorityDiscovery,
    Context: SubsystemContext<Message = NetworkBridgeMessage>,
{
    fn start(self, ctx: Context) -> SpawnedSubsystem {
        // Swallow error because failure is fatal to the node and we log with more precision
        // within `run_network`.
        let Self {
            network_service,
            authority_discovery_service,
        } = self;
        let future = run_network(network_service, authority_discovery_service, ctx)
            .map_err(|e| SubsystemError::with_origin("network-bridge", e))
            .boxed();
        SpawnedSubsystem {
            name: "network-bridge-subsystem",
            future,
        }
    }
}

struct PeerData {
    /// Latest view sent by the peer.
    view: View,
}

#[derive(Debug)]
enum Action {
    SendValidationMessage(Vec<PeerId>, protocol_v1::ValidationProtocol),
    SendCollationMessage(Vec<PeerId>, protocol_v1::CollationProtocol),
    ConnectToValidators {
        validator_ids: Vec<AuthorityDiscoveryId>,
        connected: mpsc::Sender<(AuthorityDiscoveryId, PeerId)>,
    },
    ReportPeer(PeerId, ReputationChange),

    ActiveLeaves(ActiveLeavesUpdate),

    PeerConnected(PeerSet, PeerId, ObservedRole),
    PeerDisconnected(PeerSet, PeerId),
    PeerMessages(
        PeerId,
        Vec<WireMessage<protocol_v1::ValidationProtocol>>,
        Vec<WireMessage<protocol_v1::CollationProtocol>>,
    ),

    Abort,
    Nop,
}

#[tracing::instrument(level = "trace", fields(subsystem = LOG_TARGET))]
fn action_from_overseer_message(
    res: indracore_subsystem::SubsystemResult<FromOverseer<NetworkBridgeMessage>>,
) -> Action {
    match res {
        Ok(FromOverseer::Signal(OverseerSignal::ActiveLeaves(active_leaves))) => {
            Action::ActiveLeaves(active_leaves)
        }
        Ok(FromOverseer::Signal(OverseerSignal::Conclude)) => Action::Abort,
        Ok(FromOverseer::Communication { msg }) => match msg {
            NetworkBridgeMessage::ReportPeer(peer, rep) => Action::ReportPeer(peer, rep),
            NetworkBridgeMessage::SendValidationMessage(peers, msg) => {
                Action::SendValidationMessage(peers, msg)
            }
            NetworkBridgeMessage::SendCollationMessage(peers, msg) => {
                Action::SendCollationMessage(peers, msg)
            }
            NetworkBridgeMessage::ConnectToValidators {
                validator_ids,
                connected,
            } => Action::ConnectToValidators {
                validator_ids,
                connected,
            },
        },
        Ok(FromOverseer::Signal(OverseerSignal::BlockFinalized(_))) => Action::Nop,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, err = ?e, "Shutting down Network Bridge due to error");
            Action::Abort
        }
    }
}

#[tracing::instrument(level = "trace", fields(subsystem = LOG_TARGET))]
fn action_from_network_message(event: Option<NetworkEvent>) -> Action {
    match event {
        None => {
            tracing::info!(
                target: LOG_TARGET,
                "Shutting down Network Bridge: underlying event stream concluded"
            );
            Action::Abort
        }
        Some(NetworkEvent::Dht(_)) => Action::Nop,
        Some(NetworkEvent::NotificationStreamOpened {
            remote,
            protocol,
            role,
        }) => {
            let role = role.into();
            match protocol {
                x if x == VALIDATION_PROTOCOL_NAME => {
                    Action::PeerConnected(PeerSet::Validation, remote, role)
                }
                x if x == COLLATION_PROTOCOL_NAME => {
                    Action::PeerConnected(PeerSet::Collation, remote, role)
                }
                _ => Action::Nop,
            }
        }
        Some(NetworkEvent::NotificationStreamClosed { remote, protocol }) => match protocol {
            x if x == VALIDATION_PROTOCOL_NAME => {
                Action::PeerDisconnected(PeerSet::Validation, remote)
            }
            x if x == COLLATION_PROTOCOL_NAME => {
                Action::PeerDisconnected(PeerSet::Collation, remote)
            }
            _ => Action::Nop,
        },
        Some(NetworkEvent::NotificationsReceived { remote, messages }) => {
            let v_messages: Result<Vec<_>, _> = messages
                .iter()
                .filter(|(protocol, _)| protocol == &VALIDATION_PROTOCOL_NAME)
                .map(|(_, msg_bytes)| WireMessage::decode(&mut msg_bytes.as_ref()))
                .collect();

            let v_messages = match v_messages {
                Err(_) => return Action::ReportPeer(remote, MALFORMED_MESSAGE_COST),
                Ok(v) => v,
            };

            let c_messages: Result<Vec<_>, _> = messages
                .iter()
                .filter(|(protocol, _)| protocol == &COLLATION_PROTOCOL_NAME)
                .map(|(_, msg_bytes)| WireMessage::decode(&mut msg_bytes.as_ref()))
                .collect();

            match c_messages {
                Err(_) => Action::ReportPeer(remote, MALFORMED_MESSAGE_COST),
                Ok(c_messages) => {
                    if v_messages.is_empty() && c_messages.is_empty() {
                        Action::Nop
                    } else {
                        Action::PeerMessages(remote, v_messages, c_messages)
                    }
                }
            }
        }
    }
}

fn construct_view(live_heads: &[Hash]) -> View {
    View(
        live_heads
            .iter()
            .rev()
            .take(MAX_VIEW_HEADS)
            .cloned()
            .collect(),
    )
}

#[tracing::instrument(level = "trace", skip(net, ctx, validation_peers, collation_peers), fields(subsystem = LOG_TARGET))]
async fn update_view(
    net: &mut impl Network,
    ctx: &mut impl SubsystemContext<Message = NetworkBridgeMessage>,
    live_heads: &[Hash],
    local_view: &mut View,
    validation_peers: &HashMap<PeerId, PeerData>,
    collation_peers: &HashMap<PeerId, PeerData>,
) -> SubsystemResult<()> {
    let new_view = construct_view(live_heads);
    if *local_view == new_view {
        return Ok(());
    }

    *local_view = new_view.clone();

    send_validation_message(
        net,
        validation_peers.keys().cloned(),
        WireMessage::ViewUpdate(new_view.clone()),
    )
    .await?;

    send_collation_message(
        net,
        collation_peers.keys().cloned(),
        WireMessage::ViewUpdate(new_view.clone()),
    )
    .await?;

    dispatch_validation_event_to_all(NetworkBridgeEvent::OurViewChange(new_view.clone()), ctx)
        .await;

    dispatch_collation_event_to_all(NetworkBridgeEvent::OurViewChange(new_view.clone()), ctx).await;

    Ok(())
}

// Handle messages on a specific peer-set. The peer is expected to be connected on that
// peer-set.
#[tracing::instrument(level = "trace", skip(peers, messages, net), fields(subsystem = LOG_TARGET))]
async fn handle_peer_messages<M>(
    peer: PeerId,
    peers: &mut HashMap<PeerId, PeerData>,
    messages: Vec<WireMessage<M>>,
    net: &mut impl Network,
) -> SubsystemResult<Vec<NetworkBridgeEvent<M>>> {
    let peer_data = match peers.get_mut(&peer) {
        None => {
            net.report_peer(peer, UNCONNECTED_PEERSET_COST).await?;

            return Ok(Vec::new());
        }
        Some(d) => d,
    };

    let mut outgoing_messages = Vec::with_capacity(messages.len());
    for message in messages {
        outgoing_messages.push(match message {
            WireMessage::ViewUpdate(new_view) => {
                if new_view.0.len() > MAX_VIEW_HEADS {
                    net.report_peer(peer.clone(), MALFORMED_VIEW_COST).await?;

                    continue;
                } else if new_view == peer_data.view {
                    continue;
                } else {
                    peer_data.view = new_view;

                    NetworkBridgeEvent::PeerViewChange(peer.clone(), peer_data.view.clone())
                }
            }
            WireMessage::ProtocolMessage(message) => {
                NetworkBridgeEvent::PeerMessage(peer.clone(), message)
            }
        })
    }

    Ok(outgoing_messages)
}

#[tracing::instrument(level = "trace", skip(net, peers), fields(subsystem = LOG_TARGET))]
async fn send_validation_message<I>(
    net: &mut impl Network,
    peers: I,
    message: WireMessage<protocol_v1::ValidationProtocol>,
) -> SubsystemResult<()>
where
    I: IntoIterator<Item = PeerId>,
    I::IntoIter: ExactSizeIterator,
{
    send_message(net, peers, PeerSet::Validation, message).await
}

#[tracing::instrument(level = "trace", skip(net, peers), fields(subsystem = LOG_TARGET))]
async fn send_collation_message<I>(
    net: &mut impl Network,
    peers: I,
    message: WireMessage<protocol_v1::CollationProtocol>,
) -> SubsystemResult<()>
where
    I: IntoIterator<Item = PeerId>,
    I::IntoIter: ExactSizeIterator,
{
    send_message(net, peers, PeerSet::Collation, message).await
}

async fn send_message<M, I>(
    net: &mut impl Network,
    peers: I,
    peer_set: PeerSet,
    message: WireMessage<M>,
) -> SubsystemResult<()>
where
    M: Encode + Clone,
    I: IntoIterator<Item = PeerId>,
    I::IntoIter: ExactSizeIterator,
{
    let mut message_producer = stream::iter({
        let peers = peers.into_iter();
        let n_peers = peers.len();
        let mut message = Some(message.encode());

        peers.enumerate().map(move |(i, peer)| {
            // optimization: avoid cloning the message for the last peer in the
            // list. The message payload can be quite large. If the underlying
            // network used `Bytes` this would not be necessary.
            let message = if i == n_peers - 1 {
                message
                    .take()
                    .expect("Only taken in last iteration of loop, never afterwards; qed")
            } else {
                message
                    .as_ref()
                    .expect("Only taken in last iteration of loop, we are not there yet; qed")
                    .clone()
            };

            Ok(NetworkAction::WriteNotification(peer, peer_set, message))
        })
    });

    net.action_sink().send_all(&mut message_producer).await
}

async fn dispatch_validation_event_to_all(
    event: NetworkBridgeEvent<protocol_v1::ValidationProtocol>,
    ctx: &mut impl SubsystemContext<Message = NetworkBridgeMessage>,
) {
    dispatch_validation_events_to_all(std::iter::once(event), ctx).await
}

async fn dispatch_collation_event_to_all(
    event: NetworkBridgeEvent<protocol_v1::CollationProtocol>,
    ctx: &mut impl SubsystemContext<Message = NetworkBridgeMessage>,
) {
    dispatch_collation_events_to_all(std::iter::once(event), ctx).await
}

#[tracing::instrument(level = "trace", skip(events, ctx), fields(subsystem = LOG_TARGET))]
async fn dispatch_validation_events_to_all<I>(
    events: I,
    ctx: &mut impl SubsystemContext<Message = NetworkBridgeMessage>,
) where
    I: IntoIterator<Item = NetworkBridgeEvent<protocol_v1::ValidationProtocol>>,
    I::IntoIter: Send,
{
    let messages_for = |event: NetworkBridgeEvent<protocol_v1::ValidationProtocol>| {
        let a = std::iter::once(event.focus().ok().map(|m| {
            AllMessages::AvailabilityDistribution(
                AvailabilityDistributionMessage::NetworkBridgeUpdateV1(m),
            )
        }));

        let b = std::iter::once(event.focus().ok().map(|m| {
            AllMessages::BitfieldDistribution(BitfieldDistributionMessage::NetworkBridgeUpdateV1(m))
        }));

        let p = std::iter::once(event.focus().ok().map(|m| {
            AllMessages::PoVDistribution(PoVDistributionMessage::NetworkBridgeUpdateV1(m))
        }));

        let s = std::iter::once(event.focus().ok().map(|m| {
            AllMessages::StatementDistribution(StatementDistributionMessage::NetworkBridgeUpdateV1(
                m,
            ))
        }));

        a.chain(b).chain(p).chain(s).filter_map(|x| x)
    };

    ctx.send_messages(events.into_iter().flat_map(messages_for))
        .await
}

#[tracing::instrument(level = "trace", skip(events, ctx), fields(subsystem = LOG_TARGET))]
async fn dispatch_collation_events_to_all<I>(
    events: I,
    ctx: &mut impl SubsystemContext<Message = NetworkBridgeMessage>,
) where
    I: IntoIterator<Item = NetworkBridgeEvent<protocol_v1::CollationProtocol>>,
    I::IntoIter: Send,
{
    let messages_for = |event: NetworkBridgeEvent<protocol_v1::CollationProtocol>| {
        event.focus().ok().map(|m| {
            AllMessages::CollatorProtocol(CollatorProtocolMessage::NetworkBridgeUpdateV1(m))
        })
    };

    ctx.send_messages(events.into_iter().flat_map(messages_for))
        .await
}

#[tracing::instrument(skip(network_service, authority_discovery_service, ctx), fields(subsystem = LOG_TARGET))]
async fn run_network<N, AD>(
    mut network_service: N,
    mut authority_discovery_service: AD,
    mut ctx: impl SubsystemContext<Message = NetworkBridgeMessage>,
) -> SubsystemResult<()>
where
    N: Network + validator_discovery::Network,
    AD: validator_discovery::AuthorityDiscovery,
{
    let mut event_stream = network_service.event_stream().fuse();

    // Most recent heads are at the back.
    let mut live_heads: Vec<Hash> = Vec::with_capacity(MAX_VIEW_HEADS);
    let mut local_view = View(Vec::new());

    let mut validation_peers: HashMap<PeerId, PeerData> = HashMap::new();
    let mut collation_peers: HashMap<PeerId, PeerData> = HashMap::new();

    let mut validator_discovery = validator_discovery::Service::<N, AD>::new();

    loop {
        let action = {
            let subsystem_next = ctx.recv().fuse();
            let mut net_event_next = event_stream.next().fuse();
            futures::pin_mut!(subsystem_next);

            futures::select! {
                subsystem_msg = subsystem_next => action_from_overseer_message(subsystem_msg),
                net_event = net_event_next => action_from_network_message(net_event),
            }
        };

        match action {
            Action::Nop => {}
            Action::Abort => return Ok(()),

            Action::SendValidationMessage(peers, msg) => {
                send_message(
                    &mut network_service,
                    peers,
                    PeerSet::Validation,
                    WireMessage::ProtocolMessage(msg),
                )
                .await?
            }

            Action::SendCollationMessage(peers, msg) => {
                send_message(
                    &mut network_service,
                    peers,
                    PeerSet::Collation,
                    WireMessage::ProtocolMessage(msg),
                )
                .await?
            }

            Action::ConnectToValidators {
                validator_ids,
                connected,
            } => {
                let (ns, ads) = validator_discovery
                    .on_request(
                        validator_ids,
                        connected,
                        network_service,
                        authority_discovery_service,
                    )
                    .await;
                network_service = ns;
                authority_discovery_service = ads;
            }

            Action::ReportPeer(peer, rep) => network_service.report_peer(peer, rep).await?,

            Action::ActiveLeaves(ActiveLeavesUpdate {
                activated,
                deactivated,
            }) => {
                live_heads.extend(activated);
                live_heads.retain(|h| !deactivated.contains(h));

                update_view(
                    &mut network_service,
                    &mut ctx,
                    &live_heads,
                    &mut local_view,
                    &validation_peers,
                    &collation_peers,
                )
                .await?;
            }

            Action::PeerConnected(peer_set, peer, role) => {
                let peer_map = match peer_set {
                    PeerSet::Validation => &mut validation_peers,
                    PeerSet::Collation => &mut collation_peers,
                };

                validator_discovery
                    .on_peer_connected(&peer, &mut authority_discovery_service)
                    .await;

                match peer_map.entry(peer.clone()) {
                    hash_map::Entry::Occupied(_) => continue,
                    hash_map::Entry::Vacant(vacant) => {
                        let _ = vacant.insert(PeerData {
                            view: View(Vec::new()),
                        });

                        match peer_set {
                            PeerSet::Validation => {
                                dispatch_validation_events_to_all(
                                    vec![
                                        NetworkBridgeEvent::PeerConnected(peer.clone(), role),
                                        NetworkBridgeEvent::PeerViewChange(
                                            peer,
                                            View(Default::default()),
                                        ),
                                    ],
                                    &mut ctx,
                                )
                                .await
                            }
                            PeerSet::Collation => {
                                dispatch_collation_events_to_all(
                                    vec![
                                        NetworkBridgeEvent::PeerConnected(peer.clone(), role),
                                        NetworkBridgeEvent::PeerViewChange(
                                            peer,
                                            View(Default::default()),
                                        ),
                                    ],
                                    &mut ctx,
                                )
                                .await
                            }
                        }
                    }
                }
            }
            Action::PeerDisconnected(peer_set, peer) => {
                let peer_map = match peer_set {
                    PeerSet::Validation => &mut validation_peers,
                    PeerSet::Collation => &mut collation_peers,
                };

                validator_discovery.on_peer_disconnected(&peer);

                if peer_map.remove(&peer).is_some() {
                    match peer_set {
                        PeerSet::Validation => {
                            dispatch_validation_event_to_all(
                                NetworkBridgeEvent::PeerDisconnected(peer),
                                &mut ctx,
                            )
                            .await
                        }
                        PeerSet::Collation => {
                            dispatch_collation_event_to_all(
                                NetworkBridgeEvent::PeerDisconnected(peer),
                                &mut ctx,
                            )
                            .await
                        }
                    }
                }
            }
            Action::PeerMessages(peer, v_messages, c_messages) => {
                if !v_messages.is_empty() {
                    let events = handle_peer_messages(
                        peer.clone(),
                        &mut validation_peers,
                        v_messages,
                        &mut network_service,
                    )
                    .await?;

                    dispatch_validation_events_to_all(events, &mut ctx).await;
                }

                if !c_messages.is_empty() {
                    let events = handle_peer_messages(
                        peer.clone(),
                        &mut collation_peers,
                        c_messages,
                        &mut network_service,
                    )
                    .await?;

                    dispatch_collation_events_to_all(events, &mut ctx).await;
                }
            }
        }
    }
}
