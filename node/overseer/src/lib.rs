
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

//! # Overseer
//!
//! `overseer` implements the Overseer architecture described in the
//! For the motivations behind implementing the overseer itself you should
//! check out that guide, documentation in this crate will be mostly discussing
//! technical stuff.
//!
//! An `Overseer` is something that allows spawning/stopping and overseing
//! asynchronous tasks as well as establishing a well-defined and easy to use
//! protocol that the tasks can use to communicate with each other. It is desired
//! that this protocol is the only way tasks communicate with each other, however
//! at this moment there are no foolproof guards against other ways of communication.
//!
//! The `Overseer` is instantiated with a pre-defined set of `Subsystems` that
//! share the same behavior from `Overseer`'s point of view.
//!
//! ```text
//!                              +-----------------------------+
//!                              |         Overseer            |
//!                              +-----------------------------+
//!
//!             ................|  Overseer "holds" these and uses |..............
//!             .                  them to (re)start things                      .
//!             .                                                                .
//!             .  +-------------------+                +---------------------+  .
//!             .  |   Subsystem1      |                |   Subsystem2        |  .
//!             .  +-------------------+                +---------------------+  .
//!             .           |                                       |            .
//!             ..................................................................
//!                         |                                       |
//!                       start()                                 start()
//!                         V                                       V
//!             ..................| Overseer "runs" these |.......................
//!             .  +--------------------+               +---------------------+  .
//!             .  | SubsystemInstance1 |               | SubsystemInstance2  |  .
//!             .  +--------------------+               +---------------------+  .
//!             ..................................................................
//! ```

// #![deny(unused_results)]
// unused dependencies can not work for test and examples at the same time
// yielding false positives
#![warn(missing_docs)]

use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use std::collections::{hash_map, HashMap};

use futures::channel::{mpsc, oneshot};
use futures::{
	poll, select,
	future::BoxFuture,
	stream::{self, FuturesUnordered},
	Future, FutureExt, SinkExt, StreamExt,
};
use futures_timer::Delay;
use streamunordered::{StreamYield, StreamUnordered};

use indracore_primitives::v1::{Block, BlockNumber, Hash};
use client::{BlockImportNotification, BlockchainEvents, FinalityNotification};

use indracore_subsystem::messages::{
	CandidateValidationMessage, CandidateBackingMessage,
	CandidateSelectionMessage, ChainApiMessage, StatementDistributionMessage,
	AvailabilityDistributionMessage, BitfieldSigningMessage, BitfieldDistributionMessage,
	ProvisionerMessage, PoVDistributionMessage, RuntimeApiMessage,
	AvailabilityStoreMessage, NetworkBridgeMessage, AllMessages, CollationGenerationMessage, CollatorProtocolMessage,
};
pub use indracore_subsystem::{
	Subsystem, SubsystemContext, OverseerSignal, FromOverseer, SubsystemError, SubsystemResult,
	SpawnedSubsystem, ActiveLeavesUpdate, DummySubsystem,
};
use indracore_node_subsystem_util::metrics::{self, prometheus};
use indracore_node_primitives::SpawnNamed;


// A capacity of bounded channels inside the overseer.
const CHANNEL_CAPACITY: usize = 1024;
// A graceful `Overseer` teardown time delay.
const STOP_DELAY: u64 = 1;
// Target for logs.
const LOG_TARGET: &'static str = "overseer";

/// A type of messages that are sent from [`Subsystem`] to [`Overseer`].
///
/// It wraps a system-wide [`AllMessages`] type that represents all possible
/// messages in the system.
///
/// [`AllMessages`]: enum.AllMessages.html
/// [`Subsystem`]: trait.Subsystem.html
/// [`Overseer`]: struct.Overseer.html
enum ToOverseer {
	/// This is a message sent by a `Subsystem`.
	SubsystemMessage(AllMessages),

	/// A message that wraps something the `Subsystem` is desiring to
	/// spawn on the overseer and a `oneshot::Sender` to signal the result
	/// of the spawn.
	SpawnJob {
		name: &'static str,
		s: BoxFuture<'static, ()>,
	},

	/// Same as `SpawnJob` but for blocking tasks to be executed on a
	/// dedicated thread pool.
	SpawnBlockingJob {
		name: &'static str,
		s: BoxFuture<'static, ()>,
	},
}

/// An event telling the `Overseer` on the particular block
/// that has been imported or finalized.
///
/// This structure exists solely for the purposes of decoupling
/// `Overseer` code from the client code and the necessity to call
/// `HeaderBackend::block_number_from_id()`.
#[derive(Debug)]
pub struct BlockInfo {
	/// hash of the block.
	pub hash: Hash,
	/// hash of the parent block.
	pub parent_hash: Hash,
	/// block's number.
	pub number: BlockNumber,
}

impl From<BlockImportNotification<Block>> for BlockInfo {
	fn from(n: BlockImportNotification<Block>) -> Self {
		BlockInfo {
			hash: n.hash,
			parent_hash: n.header.parent_hash,
			number: n.header.number,
		}
	}
}

impl From<FinalityNotification<Block>> for BlockInfo {
	fn from(n: FinalityNotification<Block>) -> Self {
		BlockInfo {
			hash: n.hash,
			parent_hash: n.header.parent_hash,
			number: n.header.number,
		}
	}
}

/// Some event from the outer world.
enum Event {
	BlockImported(BlockInfo),
	BlockFinalized(BlockInfo),
	MsgToSubsystem(AllMessages),
	ExternalRequest(ExternalRequest),
	Stop,
}

/// Some request from outer world.
enum ExternalRequest {
	WaitForActivation {
		hash: Hash,
		response_channel: oneshot::Sender<SubsystemResult<()>>,
	},
}

/// A handler used to communicate with the [`Overseer`].
///
/// [`Overseer`]: struct.Overseer.html
#[derive(Clone)]
pub struct OverseerHandler {
	events_tx: mpsc::Sender<Event>,
}

impl OverseerHandler {
	/// Inform the `Overseer` that that some block was imported.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	pub async fn block_imported(&mut self, block: BlockInfo) {
		self.send_and_log_error(Event::BlockImported(block)).await
	}

	/// Send some message to one of the `Subsystem`s.
	#[tracing::instrument(level = "trace", skip(self, msg), fields(subsystem = LOG_TARGET))]
	pub async fn send_msg(&mut self, msg: impl Into<AllMessages>) {
		self.send_and_log_error(Event::MsgToSubsystem(msg.into())).await
	}

	/// Inform the `Overseer` that that some block was finalized.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	pub async fn block_finalized(&mut self, block: BlockInfo) {
		self.send_and_log_error(Event::BlockFinalized(block)).await
	}

	/// Wait for a block with the given hash to be in the active-leaves set.
	/// This method is used for external code like `Proposer` that doesn't subscribe to Overseer's signals.
	///
	/// The response channel responds if the hash was activated and is closed if the hash was deactivated.
	/// Note that due the fact the overseer doesn't store the whole active-leaves set, only deltas,
	/// the response channel may never return if the hash was deactivated before this call.
	/// In this case, it's the caller's responsibility to ensure a timeout is set.
	#[tracing::instrument(level = "trace", skip(self, response_channel), fields(subsystem = LOG_TARGET))]
	pub async fn wait_for_activation(&mut self, hash: Hash, response_channel: oneshot::Sender<SubsystemResult<()>>) {
		self.send_and_log_error(Event::ExternalRequest(ExternalRequest::WaitForActivation {
			hash,
			response_channel
		})).await
	}

	/// Tell `Overseer` to shutdown.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	pub async fn stop(&mut self) {
		self.send_and_log_error(Event::Stop).await
	}

	async fn send_and_log_error(&mut self, event: Event) {
		if self.events_tx.send(event).await.is_err() {
			tracing::info!(target: LOG_TARGET, "Failed to send an event to Overseer");
		}
	}
}

/// Glues together the [`Overseer`] and `BlockchainEvents` by forwarding
/// import and finality notifications into the [`OverseerHandler`].
///
/// [`Overseer`]: struct.Overseer.html
/// [`OverseerHandler`]: struct.OverseerHandler.html
pub async fn forward_events<P: BlockchainEvents<Block>>(
	client: Arc<P>,
	mut handler: OverseerHandler,
) {
	let mut finality = client.finality_notification_stream();
	let mut imports = client.import_notification_stream();

	loop {
		select! {
			f = finality.next() => {
				match f {
					Some(block) => {
						handler.block_finalized(block.into()).await;
					}
					None => break,
				}
			},
			i = imports.next() => {
				match i {
					Some(block) => {
						handler.block_imported(block.into()).await;
					}
					None => break,
				}
			},
			complete => break,
		}
	}
}

impl Debug for ToOverseer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ToOverseer::SubsystemMessage(msg) => {
				write!(f, "OverseerMessage::SubsystemMessage({:?})", msg)
			}
			ToOverseer::SpawnJob { .. } => write!(f, "OverseerMessage::Spawn(..)"),
			ToOverseer::SpawnBlockingJob { .. } => write!(f, "OverseerMessage::SpawnBlocking(..)")
		}
	}
}

/// A running instance of some [`Subsystem`].
///
/// [`Subsystem`]: trait.Subsystem.html
struct SubsystemInstance<M> {
	tx: mpsc::Sender<FromOverseer<M>>,
}

/// A context type that is given to the [`Subsystem`] upon spawning.
/// It can be used by [`Subsystem`] to communicate with other [`Subsystem`]s
/// or to spawn it's [`SubsystemJob`]s.
///
/// [`Overseer`]: struct.Overseer.html
/// [`Subsystem`]: trait.Subsystem.html
/// [`SubsystemJob`]: trait.SubsystemJob.html
#[derive(Debug)]
pub struct OverseerSubsystemContext<M>{
	rx: mpsc::Receiver<FromOverseer<M>>,
	tx: mpsc::Sender<ToOverseer>,
}

#[async_trait::async_trait]
impl<M: Send + 'static> SubsystemContext for OverseerSubsystemContext<M> {
	type Message = M;

	async fn try_recv(&mut self) -> Result<Option<FromOverseer<M>>, ()> {
		match poll!(self.rx.next()) {
			Poll::Ready(Some(msg)) => Ok(Some(msg)),
			Poll::Ready(None) => Err(()),
			Poll::Pending => Ok(None),
		}
	}

	async fn recv(&mut self) -> SubsystemResult<FromOverseer<M>> {
		self.rx.next().await
			.ok_or(SubsystemError::Context(
				"No more messages in rx queue to process"
				.to_owned()
			))
	}

	async fn spawn(&mut self, name: &'static str, s: Pin<Box<dyn Future<Output = ()> + Send>>)
		-> SubsystemResult<()>
	{
		self.tx.send(ToOverseer::SpawnJob {
			name,
			s,
		}).await.map_err(Into::into)
	}

	async fn spawn_blocking(&mut self, name: &'static str, s: Pin<Box<dyn Future<Output = ()> + Send>>)
		-> SubsystemResult<()>
	{
		self.tx.send(ToOverseer::SpawnBlockingJob {
			name,
			s,
		}).await.map_err(Into::into)
	}

	async fn send_message(&mut self, msg: AllMessages) {
		self.send_and_log_error(ToOverseer::SubsystemMessage(msg)).await
	}

	async fn send_messages<T>(&mut self, msgs: T)
		where T: IntoIterator<Item = AllMessages> + Send, T::IntoIter: Send
	{
		let mut msgs = stream::iter(msgs.into_iter().map(ToOverseer::SubsystemMessage).map(Ok));
		if self.tx.send_all(&mut msgs).await.is_err() {
			tracing::debug!(
				target: LOG_TARGET,
				msg_type = std::any::type_name::<M>(),
				"Failed to send messages to Overseer",
			);

		}
	}
}

impl<M> OverseerSubsystemContext<M> {
	async fn send_and_log_error(&mut self, msg: ToOverseer) {
		if self.tx.send(msg).await.is_err() {
			tracing::debug!(
				target: LOG_TARGET,
				msg_type = std::any::type_name::<M>(),
				"Failed to send a message to Overseer",
			);
		}
	}
}

/// A subsystem that we oversee.
///
/// Ties together the [`Subsystem`] itself and it's running instance
/// (which may be missing if the [`Subsystem`] is not running at the moment
/// for whatever reason).
///
/// [`Subsystem`]: trait.Subsystem.html
struct OverseenSubsystem<M> {
	instance: Option<SubsystemInstance<M>>,
}

impl<M> OverseenSubsystem<M> {
	/// Send a message to the wrapped subsystem.
	///
	/// If the inner `instance` is `None`, nothing is happening.
	async fn send_message(&mut self, msg: M) -> SubsystemResult<()> {
		if let Some(ref mut instance) = self.instance {
			instance.tx.send(FromOverseer::Communication { msg }).await?;
		}

		Ok(())
	}

	/// Send a signal to the wrapped subsystem.
	///
	/// If the inner `instance` is `None`, nothing is happening.
	async fn send_signal(&mut self, signal: OverseerSignal) -> SubsystemResult<()> {
		if let Some(ref mut instance) = self.instance {
			instance.tx.send(FromOverseer::Signal(signal)).await?;
		}

		Ok(())
	}
}

/// The `Overseer` itself.
pub struct Overseer<S> {
	/// A candidate validation subsystem.
	candidate_validation_subsystem: OverseenSubsystem<CandidateValidationMessage>,

	/// A candidate backing subsystem.
	candidate_backing_subsystem: OverseenSubsystem<CandidateBackingMessage>,

	/// A candidate selection subsystem.
	candidate_selection_subsystem: OverseenSubsystem<CandidateSelectionMessage>,

	/// A statement distribution subsystem.
	statement_distribution_subsystem: OverseenSubsystem<StatementDistributionMessage>,

	/// An availability distribution subsystem.
	availability_distribution_subsystem: OverseenSubsystem<AvailabilityDistributionMessage>,

	/// A bitfield signing subsystem.
	bitfield_signing_subsystem: OverseenSubsystem<BitfieldSigningMessage>,

	/// A bitfield distribution subsystem.
	bitfield_distribution_subsystem: OverseenSubsystem<BitfieldDistributionMessage>,

	/// A provisioner subsystem.
	provisioner_subsystem: OverseenSubsystem<ProvisionerMessage>,

	/// A PoV distribution subsystem.
	pov_distribution_subsystem: OverseenSubsystem<PoVDistributionMessage>,

	/// A runtime API subsystem.
	runtime_api_subsystem: OverseenSubsystem<RuntimeApiMessage>,

	/// An availability store subsystem.
	availability_store_subsystem: OverseenSubsystem<AvailabilityStoreMessage>,

	/// A network bridge subsystem.
	network_bridge_subsystem: OverseenSubsystem<NetworkBridgeMessage>,

	/// A Chain API subsystem.
	chain_api_subsystem: OverseenSubsystem<ChainApiMessage>,

	/// A Collation Generation subsystem.
	collation_generation_subsystem: OverseenSubsystem<CollationGenerationMessage>,

	/// A Collator Protocol subsystem.
	collator_protocol_subsystem: OverseenSubsystem<CollatorProtocolMessage>,

	/// Spawner to spawn tasks to.
	s: S,

	/// Here we keep handles to spawned subsystems to be notified when they terminate.
	running_subsystems: FuturesUnordered<BoxFuture<'static, SubsystemResult<()>>>,

	/// Gather running subsystms' outbound streams into one.
	running_subsystems_rx: StreamUnordered<mpsc::Receiver<ToOverseer>>,

	/// Events that are sent to the overseer from the outside world
	events_rx: mpsc::Receiver<Event>,

	/// External listeners waiting for a hash to be in the active-leave set.
	activation_external_listeners: HashMap<Hash, Vec<oneshot::Sender<SubsystemResult<()>>>>,

	/// A set of leaves that `Overseer` starts working with.
	///
	/// Drained at the beginning of `run` and never used again.
	leaves: Vec<(Hash, BlockNumber)>,

	/// The set of the "active leaves".
	active_leaves: HashMap<Hash, BlockNumber>,

	/// Various Prometheus metrics.
	metrics: Metrics,
}

/// This struct is passed as an argument to create a new instance of an [`Overseer`].
///
/// As any entity that satisfies the interface may act as a [`Subsystem`] this allows
/// mocking in the test code:
///
/// Each [`Subsystem`] is supposed to implement some interface that is generic over
/// message type that is specific to this [`Subsystem`]. At the moment not all
/// subsystems are implemented and the rest can be mocked with the [`DummySubsystem`].
pub struct AllSubsystems<
	CV = (), CB = (), CS = (), SD = (), AD = (), BS = (), BD = (), P = (),
	PoVD = (), RA = (), AS = (), NB = (), CA = (), CG = (), CP = ()
> {
	/// A candidate validation subsystem.
	pub candidate_validation: CV,
	/// A candidate backing subsystem.
	pub candidate_backing: CB,
	/// A candidate selection subsystem.
	pub candidate_selection: CS,
	/// A statement distribution subsystem.
	pub statement_distribution: SD,
	/// An availability distribution subsystem.
	pub availability_distribution: AD,
	/// A bitfield signing subsystem.
	pub bitfield_signing: BS,
	/// A bitfield distribution subsystem.
	pub bitfield_distribution: BD,
	/// A provisioner subsystem.
	pub provisioner: P,
	/// A PoV distribution subsystem.
	pub pov_distribution: PoVD,
	/// A runtime API subsystem.
	pub runtime_api: RA,
	/// An availability store subsystem.
	pub availability_store: AS,
	/// A network bridge subsystem.
	pub network_bridge: NB,
	/// A Chain API subsystem.
	pub chain_api: CA,
	/// A Collation Generation subsystem.
	pub collation_generation: CG,
	/// A Collator Protocol subsystem.
	pub collator_protocol: CP,
}

impl<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP>
	AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP>
{
	/// Create a new instance of [`AllSubsystems`].
	///
	/// Each subsystem is set to [`DummySystem`].
	///
	///# Note
	///
	/// Because of a bug in rustc it is required that when calling this function,
	/// you provide a "random" type for the first generic parameter:
	///
	/// ```
	/// indracore_overseer::AllSubsystems::<()>::dummy();
	/// ```
	pub fn dummy() -> AllSubsystems<
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem,
		DummySubsystem
	> {
		AllSubsystems {
			candidate_validation: DummySubsystem,
			candidate_backing: DummySubsystem,
			candidate_selection: DummySubsystem,
			statement_distribution: DummySubsystem,
			availability_distribution: DummySubsystem,
			bitfield_signing: DummySubsystem,
			bitfield_distribution: DummySubsystem,
			provisioner: DummySubsystem,
			pov_distribution: DummySubsystem,
			runtime_api: DummySubsystem,
			availability_store: DummySubsystem,
			network_bridge: DummySubsystem,
			chain_api: DummySubsystem,
			collation_generation: DummySubsystem,
			collator_protocol: DummySubsystem,
		}
	}

	/// Replace the `candidate_validation` instance in `self`.
	pub fn replace_candidate_validation<NEW>(
		self,
		candidate_validation: NEW,
	) -> AllSubsystems<NEW, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `candidate_backing` instance in `self`.
	pub fn replace_candidate_backing<NEW>(
		self,
		candidate_backing: NEW,
	) -> AllSubsystems<CV, NEW, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `candidate_selection` instance in `self`.
	pub fn replace_candidate_selection<NEW>(
		self,
		candidate_selection: NEW,
	) -> AllSubsystems<CV, CB, NEW, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `statement_distribution` instance in `self`.
	pub fn replace_statement_distribution<NEW>(
		self,
		statement_distribution: NEW,
	) -> AllSubsystems<CV, CB, CS, NEW, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `availability_distribution` instance in `self`.
	pub fn replace_availability_distribution<NEW>(
		self,
		availability_distribution: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, NEW, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `bitfield_signing` instance in `self`.
	pub fn replace_bitfield_signing<NEW>(
		self,
		bitfield_signing: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, NEW, BD, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `bitfield_distribution` instance in `self`.
	pub fn replace_bitfield_distribution<NEW>(
		self,
		bitfield_distribution: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, NEW, P, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `provisioner` instance in `self`.
	pub fn replace_provisioner<NEW>(
		self,
		provisioner: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, NEW, PoVD, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `pov_distribution` instance in `self`.
	pub fn replace_pov_distribution<NEW>(
		self,
		pov_distribution: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, NEW, RA, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `runtime_api` instance in `self`.
	pub fn replace_runtime_api<NEW>(
		self,
		runtime_api: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, NEW, AS, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `availability_store` instance in `self`.
	pub fn replace_availability_store<NEW>(
		self,
		availability_store: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, NEW, NB, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `network_bridge` instance in `self`.
	pub fn replace_network_bridge<NEW>(
		self,
		network_bridge: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NEW, CA, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `chain_api` instance in `self`.
	pub fn replace_chain_api<NEW>(
		self,
		chain_api: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, NEW, CG, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api,
			collation_generation: self.collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `collation_generation` instance in `self`.
	pub fn replace_collation_generation<NEW>(
		self,
		collation_generation: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, NEW, CP> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation,
			collator_protocol: self.collator_protocol,
		}
	}

	/// Replace the `collator_protocol` instance in `self`.
	pub fn replace_collator_protocol<NEW>(
		self,
		collator_protocol: NEW,
	) -> AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, NEW> {
		AllSubsystems {
			candidate_validation: self.candidate_validation,
			candidate_backing: self.candidate_backing,
			candidate_selection: self.candidate_selection,
			statement_distribution: self.statement_distribution,
			availability_distribution: self.availability_distribution,
			bitfield_signing: self.bitfield_signing,
			bitfield_distribution: self.bitfield_distribution,
			provisioner: self.provisioner,
			pov_distribution: self.pov_distribution,
			runtime_api: self.runtime_api,
			availability_store: self.availability_store,
			network_bridge: self.network_bridge,
			chain_api: self.chain_api,
			collation_generation: self.collation_generation,
			collator_protocol,
		}
	}
}

/// Overseer Prometheus metrics.
#[derive(Clone)]
struct MetricsInner {
	activated_heads_total: prometheus::Counter<prometheus::U64>,
	deactivated_heads_total: prometheus::Counter<prometheus::U64>,
	messages_relayed_total: prometheus::Counter<prometheus::U64>,
}

#[derive(Default, Clone)]
struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_head_activated(&self) {
		if let Some(metrics) = &self.0 {
			metrics.activated_heads_total.inc();
		}
	}

	fn on_head_deactivated(&self) {
		if let Some(metrics) = &self.0 {
			metrics.deactivated_heads_total.inc();
		}
	}

	fn on_message_relayed(&self) {
		if let Some(metrics) = &self.0 {
			metrics.messages_relayed_total.inc();
		}
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			activated_heads_total: prometheus::register(
				prometheus::Counter::new(
					"parachain_activated_heads_total",
					"Number of activated heads."
				)?,
				registry,
			)?,
			deactivated_heads_total: prometheus::register(
				prometheus::Counter::new(
					"parachain_deactivated_heads_total",
					"Number of deactivated heads."
				)?,
				registry,
			)?,
			messages_relayed_total: prometheus::register(
				prometheus::Counter::new(
					"parachain_messages_relayed_total",
					"Number of messages relayed by Overseer."
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}

impl<S> Overseer<S>
where
	S: SpawnNamed,
{
	/// Create a new intance of the `Overseer` with a fixed set of [`Subsystem`]s.
	///
	/// ```text
	///                  +------------------------------------+
	///                  |            Overseer                |
	///                  +------------------------------------+
	///                    /            |             |      \
	///      ................. subsystems...................................
	///      . +-----------+    +-----------+   +----------+   +---------+ .
	///      . |           |    |           |   |          |   |         | .
	///      . +-----------+    +-----------+   +----------+   +---------+ .
	///      ...............................................................
	///                              |
	///                        probably `spawn`
	///                            a `job`
	///                              |
	///                              V
	///                         +-----------+
	///                         |           |
	///                         +-----------+
	///
	/// ```
	///
	/// [`Subsystem`]: trait.Subsystem.html
	///
	/// # Example
	///
	/// The [`Subsystems`] may be any type as long as they implement an expected interface.
	/// Here, we create a mock validation subsystem and a few dummy ones and start the `Overseer` with them.
	/// For the sake of simplicity the termination of the example is done with a timeout.
	/// ```
	/// # use std::time::Duration;
	/// # use futures::{executor, pin_mut, select, FutureExt};
	/// # use futures_timer::Delay;
	/// # use indracore_overseer::{Overseer, AllSubsystems};
	/// # use indracore_subsystem::{
	/// #     Subsystem, DummySubsystem, SpawnedSubsystem, SubsystemContext,
	/// #     messages::CandidateValidationMessage,
	/// # };
	///
	/// struct ValidationSubsystem;
	///
	/// impl<C> Subsystem<C> for ValidationSubsystem
	///     where C: SubsystemContext<Message=CandidateValidationMessage>
	/// {
	///     fn start(
	///         self,
	///         mut ctx: C,
	///     ) -> SpawnedSubsystem {
	///         SpawnedSubsystem {
	///             name: "validation-subsystem",
	///             future: Box::pin(async move {
	///                 loop {
	///                     Delay::new(Duration::from_secs(1)).await;
	///                 }
	///             }),
	///         }
	///     }
	/// }
	///
	/// # fn main() { executor::block_on(async move {
	/// let spawner = sp_core::testing::TaskExecutor::new();
	/// let all_subsystems = AllSubsystems::<()>::dummy().replace_candidate_validation(ValidationSubsystem);
	/// let (overseer, _handler) = Overseer::new(
	///     vec![],
	///     all_subsystems,
	///     None,
	///     spawner,
	/// ).unwrap();
	///
	/// let timer = Delay::new(Duration::from_millis(50)).fuse();
	///
	/// let overseer_fut = overseer.run().fuse();
	/// pin_mut!(timer);
	/// pin_mut!(overseer_fut);
	///
	/// select! {
	///     _ = overseer_fut => (),
	///     _ = timer => (),
	/// }
	/// #
	/// # }); }
	/// ```
	pub fn new<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP>(
		leaves: impl IntoIterator<Item = BlockInfo>,
		all_subsystems: AllSubsystems<CV, CB, CS, SD, AD, BS, BD, P, PoVD, RA, AS, NB, CA, CG, CP>,
		prometheus_registry: Option<&prometheus::Registry>,
		mut s: S,
	) -> SubsystemResult<(Self, OverseerHandler)>
	where
		CV: Subsystem<OverseerSubsystemContext<CandidateValidationMessage>> + Send,
		CB: Subsystem<OverseerSubsystemContext<CandidateBackingMessage>> + Send,
		CS: Subsystem<OverseerSubsystemContext<CandidateSelectionMessage>> + Send,
		SD: Subsystem<OverseerSubsystemContext<StatementDistributionMessage>> + Send,
		AD: Subsystem<OverseerSubsystemContext<AvailabilityDistributionMessage>> + Send,
		BS: Subsystem<OverseerSubsystemContext<BitfieldSigningMessage>> + Send,
		BD: Subsystem<OverseerSubsystemContext<BitfieldDistributionMessage>> + Send,
		P: Subsystem<OverseerSubsystemContext<ProvisionerMessage>> + Send,
		PoVD: Subsystem<OverseerSubsystemContext<PoVDistributionMessage>> + Send,
		RA: Subsystem<OverseerSubsystemContext<RuntimeApiMessage>> + Send,
		AS: Subsystem<OverseerSubsystemContext<AvailabilityStoreMessage>> + Send,
		NB: Subsystem<OverseerSubsystemContext<NetworkBridgeMessage>> + Send,
		CA: Subsystem<OverseerSubsystemContext<ChainApiMessage>> + Send,
		CG: Subsystem<OverseerSubsystemContext<CollationGenerationMessage>> + Send,
		CP: Subsystem<OverseerSubsystemContext<CollatorProtocolMessage>> + Send,
	{
		let (events_tx, events_rx) = mpsc::channel(CHANNEL_CAPACITY);

		let handler = OverseerHandler {
			events_tx: events_tx.clone(),
		};

		let mut running_subsystems_rx = StreamUnordered::new();
		let mut running_subsystems = FuturesUnordered::new();

		let candidate_validation_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.candidate_validation,
		)?;

		let candidate_backing_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.candidate_backing,
		)?;

		let candidate_selection_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.candidate_selection,
		)?;

		let statement_distribution_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.statement_distribution,
		)?;

		let availability_distribution_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.availability_distribution,
		)?;

		let bitfield_signing_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.bitfield_signing,
		)?;

		let bitfield_distribution_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.bitfield_distribution,
		)?;

		let provisioner_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.provisioner,
		)?;

		let pov_distribution_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.pov_distribution,
		)?;

		let runtime_api_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.runtime_api,
		)?;

		let availability_store_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.availability_store,
		)?;

		let network_bridge_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.network_bridge,
		)?;

		let chain_api_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.chain_api,
		)?;

		let collation_generation_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.collation_generation,
		)?;


		let collator_protocol_subsystem = spawn(
			&mut s,
			&mut running_subsystems,
			&mut running_subsystems_rx,
			all_subsystems.collator_protocol,
		)?;

		let leaves = leaves
			.into_iter()
			.map(|BlockInfo { hash, parent_hash: _, number }| (hash, number))
			.collect();

		let active_leaves = HashMap::new();

		let metrics = <Metrics as metrics::Metrics>::register(prometheus_registry)?;
		let activation_external_listeners = HashMap::new();

		let this = Self {
			candidate_validation_subsystem,
			candidate_backing_subsystem,
			candidate_selection_subsystem,
			statement_distribution_subsystem,
			availability_distribution_subsystem,
			bitfield_signing_subsystem,
			bitfield_distribution_subsystem,
			provisioner_subsystem,
			pov_distribution_subsystem,
			runtime_api_subsystem,
			availability_store_subsystem,
			network_bridge_subsystem,
			chain_api_subsystem,
			collation_generation_subsystem,
			collator_protocol_subsystem,
			s,
			running_subsystems,
			running_subsystems_rx,
			events_rx,
			activation_external_listeners,
			leaves,
			active_leaves,
			metrics,
		};

		Ok((this, handler))
	}

	// Stop the overseer.
	async fn stop(mut self) {
		let _ = self.candidate_validation_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.candidate_backing_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.candidate_selection_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.statement_distribution_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.availability_distribution_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.bitfield_signing_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.bitfield_distribution_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.provisioner_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.pov_distribution_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.runtime_api_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.availability_store_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.network_bridge_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.chain_api_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.collator_protocol_subsystem.send_signal(OverseerSignal::Conclude).await;
		let _ = self.collation_generation_subsystem.send_signal(OverseerSignal::Conclude).await;

		let mut stop_delay = Delay::new(Duration::from_secs(STOP_DELAY)).fuse();

		loop {
			select! {
				_ = self.running_subsystems.next() => {
					if self.running_subsystems.is_empty() {
						break;
					}
				},
				_ = stop_delay => break,
				complete => break,
			}
		}
	}

	/// Run the `Overseer`.
	#[tracing::instrument(skip(self), fields(subsystem = LOG_TARGET))]
	pub async fn run(mut self) -> SubsystemResult<()> {
		let mut update = ActiveLeavesUpdate::default();

		for (hash, number) in std::mem::take(&mut self.leaves) {
			update.activated.push(hash);
			let _ = self.active_leaves.insert(hash, number);
			self.on_head_activated(&hash);
		}

		self.broadcast_signal(OverseerSignal::ActiveLeaves(update)).await?;

		loop {
			select! {
				msg = self.events_rx.next().fuse() => {
					let msg = if let Some(msg) = msg {
						msg
					} else {
						continue
					};

					match msg {
						Event::MsgToSubsystem(msg) => {
							self.route_message(msg).await;
						}
						Event::Stop => {
							self.stop().await;
							return Ok(());
						}
						Event::BlockImported(block) => {
							self.block_imported(block).await?;
						}
						Event::BlockFinalized(block) => {
							self.block_finalized(block).await?;
						}
						Event::ExternalRequest(request) => {
							self.handle_external_request(request);
						}
					}
				},
				msg = self.running_subsystems_rx.next().fuse() => {
					let msg = if let Some((StreamYield::Item(msg), _)) = msg {
						msg
					} else {
						continue
					};

					match msg {
						ToOverseer::SubsystemMessage(msg) => self.route_message(msg).await,
						ToOverseer::SpawnJob { name, s } => {
							self.spawn_job(name, s);
						}
						ToOverseer::SpawnBlockingJob { name, s } => {
							self.spawn_blocking_job(name, s);
						}
					}
				},
				res = self.running_subsystems.next().fuse() => {
					let finished = if let Some(finished) = res {
						finished
					} else {
						continue
					};

					tracing::error!(target: LOG_TARGET, subsystem = ?finished, "subsystem finished unexpectedly");
					self.stop().await;
					return finished;
				},
			}
		}
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn block_imported(&mut self, block: BlockInfo) -> SubsystemResult<()> {
		let mut update = ActiveLeavesUpdate::default();

		if let Some(number) = self.active_leaves.remove(&block.parent_hash) {
			if let Some(expected_parent_number) = block.number.checked_sub(1) {
				debug_assert_eq!(expected_parent_number, number);
			}
			update.deactivated.push(block.parent_hash);
			self.on_head_deactivated(&block.parent_hash);
		}

		match self.active_leaves.entry(block.hash) {
			hash_map::Entry::Vacant(entry) => {
				update.activated.push(block.hash);
				let _ = entry.insert(block.number);
				self.on_head_activated(&block.hash);
			},
			hash_map::Entry::Occupied(entry) => {
				debug_assert_eq!(*entry.get(), block.number);
			}
		}

		self.clean_up_external_listeners();

		self.broadcast_signal(OverseerSignal::ActiveLeaves(update)).await?;

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn block_finalized(&mut self, block: BlockInfo) -> SubsystemResult<()> {
		let mut update = ActiveLeavesUpdate::default();

		self.active_leaves.retain(|h, n| {
			if *n <= block.number {
				update.deactivated.push(*h);
				false
			} else {
				true
			}
		});

		for deactivated in &update.deactivated {
			self.on_head_deactivated(deactivated)
		}

		// Most of the time we have a leave already closed when it is finalized, so we check here if there are actually
		// any updates before sending it to the subsystems.
		if !update.is_empty() {
			self.broadcast_signal(OverseerSignal::ActiveLeaves(update)).await?;
		}

		self.broadcast_signal(OverseerSignal::BlockFinalized(block.hash)).await?;

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn broadcast_signal(&mut self, signal: OverseerSignal) -> SubsystemResult<()> {
		self.candidate_validation_subsystem.send_signal(signal.clone()).await?;
		self.candidate_backing_subsystem.send_signal(signal.clone()).await?;
		self.candidate_selection_subsystem.send_signal(signal.clone()).await?;
		self.statement_distribution_subsystem.send_signal(signal.clone()).await?;
		self.availability_distribution_subsystem.send_signal(signal.clone()).await?;
		self.bitfield_signing_subsystem.send_signal(signal.clone()).await?;
		self.bitfield_distribution_subsystem.send_signal(signal.clone()).await?;
		self.provisioner_subsystem.send_signal(signal.clone()).await?;
		self.pov_distribution_subsystem.send_signal(signal.clone()).await?;
		self.runtime_api_subsystem.send_signal(signal.clone()).await?;
		self.availability_store_subsystem.send_signal(signal.clone()).await?;
		self.network_bridge_subsystem.send_signal(signal.clone()).await?;
		self.chain_api_subsystem.send_signal(signal.clone()).await?;
		self.collator_protocol_subsystem.send_signal(signal.clone()).await?;
		self.collation_generation_subsystem.send_signal(signal).await?;

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn route_message(&mut self, msg: AllMessages) {
		self.metrics.on_message_relayed();
		match msg {
			AllMessages::CandidateValidation(msg) => {
				let _ = self.candidate_validation_subsystem.send_message(msg).await;
			},
			AllMessages::CandidateBacking(msg) => {
				let _ = self.candidate_backing_subsystem.send_message(msg).await;
			},
			AllMessages::CandidateSelection(msg) => {
				let _ = self.candidate_selection_subsystem.send_message(msg).await;
			},
			AllMessages::StatementDistribution(msg) => {
				let _ = self.statement_distribution_subsystem.send_message(msg).await;
			},
			AllMessages::AvailabilityDistribution(msg) => {
				let _ = self.availability_distribution_subsystem.send_message(msg).await;
			},
			AllMessages::BitfieldDistribution(msg) => {
				let _ = self.bitfield_distribution_subsystem.send_message(msg).await;
			},
			AllMessages::BitfieldSigning(msg) => {
				let _ = self.bitfield_signing_subsystem.send_message(msg).await;
			},
			AllMessages::Provisioner(msg) => {
				let _ = self.provisioner_subsystem.send_message(msg).await;
			},
			AllMessages::PoVDistribution(msg) => {
				let _ = self.pov_distribution_subsystem.send_message(msg).await;
			},
			AllMessages::RuntimeApi(msg) => {
				let _ = self.runtime_api_subsystem.send_message(msg).await;
			},
			AllMessages::AvailabilityStore(msg) => {
				let _ = self.availability_store_subsystem.send_message(msg).await;
			},
			AllMessages::NetworkBridge(msg) => {
				let _ = self.network_bridge_subsystem.send_message(msg).await;
			},
			AllMessages::ChainApi(msg) => {
				let _ = self.chain_api_subsystem.send_message(msg).await;
			},
			AllMessages::CollationGeneration(msg) => {
				let _ = self.collation_generation_subsystem.send_message(msg).await;
			},
			AllMessages::CollatorProtocol(msg) => {
				let _ = self.collator_protocol_subsystem.send_message(msg).await;
			},
		}
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn on_head_activated(&mut self, hash: &Hash) {
		self.metrics.on_head_activated();
		if let Some(listeners) = self.activation_external_listeners.remove(hash) {
			for listener in listeners {
				// it's fine if the listener is no longer interested
				let _ = listener.send(Ok(()));
			}
		}
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn on_head_deactivated(&mut self, hash: &Hash) {
		self.metrics.on_head_deactivated();
		if let Some(listeners) = self.activation_external_listeners.remove(hash) {
			// clean up and signal to listeners the block is deactivated
			drop(listeners);
		}
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn clean_up_external_listeners(&mut self) {
		self.activation_external_listeners.retain(|_, v| {
			// remove dead listeners
			v.retain(|c| !c.is_canceled());
			!v.is_empty()
		})
	}

	#[tracing::instrument(level = "trace", skip(self, request), fields(subsystem = LOG_TARGET))]
	fn handle_external_request(&mut self, request: ExternalRequest) {
		match request {
			ExternalRequest::WaitForActivation { hash, response_channel } => {
				if self.active_leaves.get(&hash).is_some() {
					// it's fine if the listener is no longer interested
					let _ = response_channel.send(Ok(()));
				} else {
					self.activation_external_listeners.entry(hash).or_default().push(response_channel);
				}
			}
		}
	}

	fn spawn_job(&mut self, name: &'static str, j: BoxFuture<'static, ()>) {
		self.s.spawn(name, j);
	}

	fn spawn_blocking_job(&mut self, name: &'static str, j: BoxFuture<'static, ()>) {
		self.s.spawn_blocking(name, j);
	}
}

fn spawn<S: SpawnNamed, M: Send + 'static>(
	spawner: &mut S,
	futures: &mut FuturesUnordered<BoxFuture<'static, SubsystemResult<()>>>,
	streams: &mut StreamUnordered<mpsc::Receiver<ToOverseer>>,
	s: impl Subsystem<OverseerSubsystemContext<M>>,
) -> SubsystemResult<OverseenSubsystem<M>> {
	let (to_tx, to_rx) = mpsc::channel(CHANNEL_CAPACITY);
	let (from_tx, from_rx) = mpsc::channel(CHANNEL_CAPACITY);
	let ctx = OverseerSubsystemContext { rx: to_rx, tx: from_tx };
	let SpawnedSubsystem { future, name } = s.start(ctx);

	let (tx, rx) = oneshot::channel();

	let fut = Box::pin(async move {
		if let Err(e) = future.await {
			tracing::error!(subsystem=name, err = ?e, "subsystem exited with error");
		} else {
			tracing::debug!(subsystem=name, "subsystem exited without an error");
		}
		let _ = tx.send(());
	});

	spawner.spawn(name, fut);

	let _ = streams.insert(from_rx);
	futures.push(Box::pin(rx.map(|e| { tracing::warn!(err = ?e, "dropping error"); Ok(()) })));

	let instance = Some(SubsystemInstance {
		tx: to_tx,
	});

	Ok(OverseenSubsystem {
		instance,
	})
}
