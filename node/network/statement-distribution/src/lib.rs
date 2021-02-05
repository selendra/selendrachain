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

//! The Statement Distribution Subsystem.
//!
//! This is responsible for distributing signed statements about candidate
//! validity amongst validators.

#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]

use indracore_subsystem::{
	Subsystem, SubsystemResult, SubsystemContext, SpawnedSubsystem,
	ActiveLeavesUpdate, FromOverseer, OverseerSignal,
	messages::{
		AllMessages, NetworkBridgeMessage, StatementDistributionMessage, CandidateBackingMessage,
		RuntimeApiMessage, RuntimeApiRequest,
	},
};
use indracore_node_subsystem_util::metrics::{self, prometheus};
use node_primitives::SignedFullStatement;
use indracore_primitives::v1::{
	Hash, CompactStatement, ValidatorIndex, ValidatorId, SigningContext, ValidatorSignature, CandidateHash,
};
use indracore_node_network_protocol::{
	v1 as protocol_v1, View, PeerId, ReputationChange as Rep, NetworkBridgeEvent,
};

use futures::prelude::*;
use futures::channel::{mpsc, oneshot};
use indexmap::IndexSet;

use std::collections::{HashMap, HashSet};

const COST_UNEXPECTED_STATEMENT: Rep = Rep::new(-100, "Unexpected Statement");
const COST_INVALID_SIGNATURE: Rep = Rep::new(-500, "Invalid Statement Signature");
const COST_DUPLICATE_STATEMENT: Rep = Rep::new(-250, "Statement sent more than once by peer");
const COST_APPARENT_FLOOD: Rep = Rep::new(-1000, "Peer appears to be flooding us with statements");

const BENEFIT_VALID_STATEMENT: Rep = Rep::new(5, "Peer provided a valid statement");
const BENEFIT_VALID_STATEMENT_FIRST: Rep = Rep::new(
	25,
	"Peer was the first to provide a valid statement",
);

/// The maximum amount of candidates each validator is allowed to second at any relay-parent.
/// Short for "Validator Candidate Threshold".
///
/// This is the amount of candidates we keep per validator at any relay-parent.
/// Typically we will only keep 1, but when a validator equivocates we will need to track 2.
const VC_THRESHOLD: usize = 2;

const LOG_TARGET: &str = "statement_distribution";

/// The statement distribution subsystem.
pub struct StatementDistribution {
	// Prometheus metrics
	metrics: Metrics,
}

impl<C> Subsystem<C> for StatementDistribution
	where C: SubsystemContext<Message=StatementDistributionMessage>
{
	fn start(self, ctx: C) -> SpawnedSubsystem {
		// Swallow error because failure is fatal to the node and we log with more precision
		// within `run`.
		SpawnedSubsystem {
			name: "statement-distribution-subsystem",
			future: self.run(ctx).boxed(),
		}
	}
}

impl StatementDistribution {
	/// Create a new Statement Distribution Subsystem
	pub fn new(metrics: Metrics) -> StatementDistribution {
		StatementDistribution {
			metrics,
		}
	}
}

/// Tracks our impression of a single peer's view of the candidates a validator has seconded
/// for a given relay-parent.
///
/// It is expected to receive at most `VC_THRESHOLD` from us and be aware of at most `VC_THRESHOLD`
/// via other means.
#[derive(Default)]
struct VcPerPeerTracker {
	local_observed: arrayvec::ArrayVec<[CandidateHash; VC_THRESHOLD]>,
	remote_observed: arrayvec::ArrayVec<[CandidateHash; VC_THRESHOLD]>,
}

impl VcPerPeerTracker {
	/// Note that the remote should now be aware that a validator has seconded a given candidate (by hash)
	/// based on a message that we have sent it from our local pool.
	fn note_local(&mut self, h: CandidateHash) {
		if !note_hash(&mut self.local_observed, h) {
			tracing::warn!("Statement distribution is erroneously attempting to distribute more \
				than {} candidate(s) per validator index. Ignoring", VC_THRESHOLD);
		}
	}

	/// Note that the remote should now be aware that a validator has seconded a given candidate (by hash)
	/// based on a message that it has sent us.
	///
	/// Returns `true` if the peer was allowed to send us such a message, `false` otherwise.
	fn note_remote(&mut self, h: CandidateHash) -> bool {
		note_hash(&mut self.remote_observed, h)
	}
}

fn note_hash(
	observed: &mut arrayvec::ArrayVec<[CandidateHash; VC_THRESHOLD]>,
	h: CandidateHash,
) -> bool {
	if observed.contains(&h) { return true; }

	observed.try_push(h).is_ok()
}

/// knowledge that a peer has about goings-on in a relay parent.
#[derive(Default)]
struct PeerRelayParentKnowledge {
	/// candidates that the peer is aware of. This indicates that we can
	/// send other statements pertaining to that candidate.
	known_candidates: HashSet<CandidateHash>,
	/// fingerprints of all statements a peer should be aware of: those that
	/// were sent to the peer by us.
	sent_statements: HashSet<(CompactStatement, ValidatorIndex)>,
	/// fingerprints of all statements a peer should be aware of: those that
	/// were sent to us by the peer.
	received_statements: HashSet<(CompactStatement, ValidatorIndex)>,
	/// How many candidates this peer is aware of for each given validator index.
	seconded_counts: HashMap<ValidatorIndex, VcPerPeerTracker>,
	/// How many statements we've received for each candidate that we're aware of.
	received_message_count: HashMap<CandidateHash, usize>,
}

impl PeerRelayParentKnowledge {
	/// Attempt to update our view of the peer's knowledge with this statement's fingerprint based
	/// on something that we would like to send to the peer.
	///
	/// This returns `None` if the peer cannot accept this statement, without altering internal
	/// state.
	///
	/// If the peer can accept the statement, this returns `Some` and updates the internal state.
	/// Once the knowledge has incorporated a statement, it cannot be incorporated again.
	///
	/// This returns `Some(true)` if this is the first time the peer has become aware of a
	/// candidate with the given hash.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn send(&mut self, fingerprint: &(CompactStatement, ValidatorIndex)) -> Option<bool> {
		let already_known = self.sent_statements.contains(fingerprint)
			|| self.received_statements.contains(fingerprint);

		if already_known {
			return None;
		}

		let new_known = match fingerprint.0 {
			CompactStatement::Candidate(ref h) => {
				self.seconded_counts.entry(fingerprint.1)
					.or_default()
					.note_local(h.clone());

				self.known_candidates.insert(h.clone())
			},
			CompactStatement::Valid(ref h) | CompactStatement::Invalid(ref h) => {
				// The peer can only accept Valid and Invalid statements for which it is aware
				// of the corresponding candidate.
				if !self.known_candidates.contains(h) {
					return None;
				}

				false
			}
		};

		self.sent_statements.insert(fingerprint.clone());

		Some(new_known)
	}

	/// Attempt to update our view of the peer's knowledge with this statement's fingerprint based on
	/// a message we are receiving from the peer.
	///
	/// Provide the maximum message count that we can receive per candidate. In practice we should
	/// not receive more statements for any one candidate than there are members in the group assigned
	/// to that para, but this maximum needs to be lenient to account for equivocations that may be
	/// cross-group. As such, a maximum of 2 * n_validators is recommended.
	///
	/// This returns an error if the peer should not have sent us this message according to protocol
	/// rules for flood protection.
	///
	/// If this returns `Ok`, the internal state has been altered. After `receive`ing a new
	/// candidate, we are then cleared to send the peer further statements about that candidate.
	///
	/// This returns `Ok(true)` if this is the first time the peer has become aware of a
	/// candidate with given hash.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn receive(
		&mut self,
		fingerprint: &(CompactStatement, ValidatorIndex),
		max_message_count: usize,
	) -> Result<bool, Rep> {
		// We don't check `sent_statements` because a statement could be in-flight from both
		// sides at the same time.
		if self.received_statements.contains(fingerprint) {
			return Err(COST_DUPLICATE_STATEMENT);
		}

		let candidate_hash = match fingerprint.0 {
			CompactStatement::Candidate(ref h) => {
				let allowed_remote = self.seconded_counts.entry(fingerprint.1)
					.or_insert_with(Default::default)
					.note_remote(h.clone());

				if !allowed_remote {
					return Err(COST_UNEXPECTED_STATEMENT);
				}

				h
			}
			CompactStatement::Valid(ref h)| CompactStatement::Invalid(ref h) => {
				if !self.known_candidates.contains(&h) {
					return Err(COST_UNEXPECTED_STATEMENT);
				}

				h
			}
		};

		{
			let received_per_candidate = self.received_message_count
				.entry(*candidate_hash)
				.or_insert(0);

			if *received_per_candidate >= max_message_count {
				return Err(COST_APPARENT_FLOOD);
			}

			*received_per_candidate += 1;
		}

		self.received_statements.insert(fingerprint.clone());
		Ok(self.known_candidates.insert(candidate_hash.clone()))
	}
}

struct PeerData {
	view: View,
	view_knowledge: HashMap<Hash, PeerRelayParentKnowledge>,
}

impl PeerData {
	/// Attempt to update our view of the peer's knowledge with this statement's fingerprint based
	/// on something that we would like to send to the peer.
	///
	/// This returns `None` if the peer cannot accept this statement, without altering internal
	/// state.
	///
	/// If the peer can accept the statement, this returns `Some` and updates the internal state.
	/// Once the knowledge has incorporated a statement, it cannot be incorporated again.
	///
	/// This returns `Some(true)` if this is the first time the peer has become aware of a
	/// candidate with the given hash.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn send(
		&mut self,
		relay_parent: &Hash,
		fingerprint: &(CompactStatement, ValidatorIndex),
	) -> Option<bool> {
		self.view_knowledge.get_mut(relay_parent).map_or(None, |k| k.send(fingerprint))
	}

	/// Attempt to update our view of the peer's knowledge with this statement's fingerprint based on
	/// a message we are receiving from the peer.
	///
	/// Provide the maximum message count that we can receive per candidate. In practice we should
	/// not receive more statements for any one candidate than there are members in the group assigned
	/// to that para, but this maximum needs to be lenient to account for equivocations that may be
	/// cross-group. As such, a maximum of 2 * n_validators is recommended.
	///
	/// This returns an error if the peer should not have sent us this message according to protocol
	/// rules for flood protection.
	///
	/// If this returns `Ok`, the internal state has been altered. After `receive`ing a new
	/// candidate, we are then cleared to send the peer further statements about that candidate.
	///
	/// This returns `Ok(true)` if this is the first time the peer has become aware of a
	/// candidate with given hash.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn receive(
		&mut self,
		relay_parent: &Hash,
		fingerprint: &(CompactStatement, ValidatorIndex),
		max_message_count: usize,
	) -> Result<bool, Rep> {
		self.view_knowledge
			.get_mut(relay_parent)
			.ok_or(COST_UNEXPECTED_STATEMENT)?
			.receive(fingerprint, max_message_count)
	}
}

// A statement stored while a relay chain head is active.
#[derive(Debug)]
struct StoredStatement {
	comparator: StoredStatementComparator,
	statement: SignedFullStatement,
}

// A value used for comparison of stored statements to each other.
//
// The compact version of the statement, the validator index, and the signature of the validator
// is enough to differentiate between all types of equivocations, as long as the signature is
// actually checked to be valid. The same statement with 2 signatures and 2 statements with
// different (or same) signatures wll all be correctly judged to be unequal with this comparator.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct StoredStatementComparator {
	compact: CompactStatement,
	validator_index: ValidatorIndex,
	signature: ValidatorSignature,
}

impl StoredStatement {
	fn compact(&self) -> &CompactStatement {
		&self.comparator.compact
	}

	fn fingerprint(&self) -> (CompactStatement, ValidatorIndex) {
		(self.comparator.compact.clone(), self.statement.validator_index())
	}
}

impl std::borrow::Borrow<StoredStatementComparator> for StoredStatement {
	fn borrow(&self) -> &StoredStatementComparator {
		&self.comparator
	}
}

impl std::hash::Hash for StoredStatement {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.comparator.hash(state)
	}
}

impl std::cmp::PartialEq for StoredStatement {
	fn eq(&self, other: &Self) -> bool {
		&self.comparator == &other.comparator
	}
}

impl std::cmp::Eq for StoredStatement {}

#[derive(Debug)]
enum NotedStatement<'a> {
	NotUseful,
	Fresh(&'a StoredStatement),
	UsefulButKnown
}

struct ActiveHeadData {
	/// All candidates we are aware of for this head, keyed by hash.
	candidates: HashSet<CandidateHash>,
	/// Stored statements for circulation to peers.
	///
	/// These are iterable in insertion order, and `Seconded` statements are always
	/// accepted before dependent statements.
	statements: IndexSet<StoredStatement>,
	/// The validators at this head.
	validators: Vec<ValidatorId>,
	/// The session index this head is at.
	session_index: sp_staking::SessionIndex,
	/// How many `Seconded` statements we've seen per validator.
	seconded_counts: HashMap<ValidatorIndex, usize>,
}

impl ActiveHeadData {
	fn new(validators: Vec<ValidatorId>, session_index: sp_staking::SessionIndex) -> Self {
		ActiveHeadData {
			candidates: Default::default(),
			statements: Default::default(),
			validators,
			session_index,
			seconded_counts: Default::default(),
		}
	}

	/// Note the given statement.
	///
	/// If it was not already known and can be accepted,  returns `NotedStatement::Fresh`,
	/// with a handle to the statement.
	///
	/// If it can be accepted, but we already know it, returns `NotedStatement::UsefulButKnown`.
	///
	/// We accept up to `VC_THRESHOLD` (2 at time of writing) `Seconded` statements
	/// per validator. These will be the first ones we see. The statement is assumed
	/// to have been checked, including that the validator index is not out-of-bounds and
	/// the signature is valid.
	///
	/// Any other statements or those that reference a candidate we are not aware of cannot be accepted
	/// and will return `NotedStatement::NotUseful`.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn note_statement(&mut self, statement: SignedFullStatement) -> NotedStatement {
		let validator_index = statement.validator_index();
		let comparator = StoredStatementComparator {
			compact: statement.payload().to_compact(),
			validator_index,
			signature: statement.signature().clone(),
		};

		let stored = StoredStatement {
			comparator: comparator.clone(),
			statement,
		};

		match comparator.compact {
			CompactStatement::Candidate(h) => {
				let seconded_so_far = self.seconded_counts.entry(validator_index).or_insert(0);
				if *seconded_so_far >= VC_THRESHOLD {
					return NotedStatement::NotUseful;
				}

				self.candidates.insert(h);
				if self.statements.insert(stored) {
					*seconded_so_far += 1;

					// This will always return `Some` because it was just inserted.
					NotedStatement::Fresh(self.statements.get(&comparator)
						.expect("Statement was just inserted; qed"))
				} else {
					NotedStatement::UsefulButKnown
				}
			}
			CompactStatement::Valid(h) | CompactStatement::Invalid(h) => {
				if !self.candidates.contains(&h) {
					return NotedStatement::NotUseful;
				}

				if self.statements.insert(stored) {
					// This will always return `Some` because it was just inserted.
					NotedStatement::Fresh(self.statements.get(&comparator)
						.expect("Statement was just inserted; qed"))
				} else {
					NotedStatement::UsefulButKnown
				}
			}
		}
	}

	/// Get an iterator over all statements for the active head. Seconded statements come first.
	fn statements(&self) -> impl Iterator<Item = &'_ StoredStatement> + '_ {
		self.statements.iter()
	}

	/// Get an iterator over all statements for the active head that are for a particular candidate.
	fn statements_about(&self, candidate_hash: CandidateHash)
		-> impl Iterator<Item = &'_ StoredStatement> + '_
	{
		self.statements().filter(move |s| s.compact().candidate_hash() == &candidate_hash)
	}
}

/// Check a statement signature under this parent hash.
fn check_statement_signature(
	head: &ActiveHeadData,
	relay_parent: Hash,
	statement: &SignedFullStatement,
) -> Result<(), ()> {
	let signing_context = SigningContext {
		session_index: head.session_index,
		parent_hash: relay_parent,
	};

	head.validators.get(statement.validator_index() as usize)
		.ok_or(())
		.and_then(|v| statement.check_signature(&signing_context, v))
}

type StatementListeners = Vec<mpsc::Sender<SignedFullStatement>>;

/// Informs all registered listeners about a newly received statement.
///
/// Removes all closed listeners.
#[tracing::instrument(level = "trace", skip(listeners), fields(subsystem = LOG_TARGET))]
async fn inform_statement_listeners(
	statement: &SignedFullStatement,
	listeners: &mut StatementListeners,
) {
	// Ignore the errors since these will be removed later.
	stream::iter(listeners.iter_mut()).for_each_concurrent(
		None,
		|listener| async move {
			let _ = listener.send(statement.clone()).await;
		}
	).await;
	// Remove any closed listeners.
	listeners.retain(|tx| !tx.is_closed());
}

/// Places the statement in storage if it is new, and then
/// circulates the statement to all peers who have not seen it yet, and
/// sends all statements dependent on that statement to peers who could previously not receive
/// them but now can.
#[tracing::instrument(level = "trace", skip(peers, ctx, active_heads, metrics), fields(subsystem = LOG_TARGET))]
async fn circulate_statement_and_dependents(
	peers: &mut HashMap<PeerId, PeerData>,
	active_heads: &mut HashMap<Hash, ActiveHeadData>,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	relay_parent: Hash,
	statement: SignedFullStatement,
	metrics: &Metrics,
) {
	let active_head = match active_heads.get_mut(&relay_parent) {
		Some(res) => res,
		None => return,
	};

	// First circulate the statement directly to all peers needing it.
	// The borrow of `active_head` needs to encompass only this (Rust) statement.
	let outputs: Option<(CandidateHash, Vec<PeerId>)> = {
		match active_head.note_statement(statement) {
			NotedStatement::Fresh(stored) => Some((
				*stored.compact().candidate_hash(),
				circulate_statement(peers, ctx, relay_parent, stored).await,
			)),
			_ => None,
		}
	};

	// Now send dependent statements to all peers needing them, if any.
	if let Some((candidate_hash, peers_needing_dependents)) = outputs {
		for peer in peers_needing_dependents {
			if let Some(peer_data) = peers.get_mut(&peer) {
				// defensive: the peer data should always be some because the iterator
				// of peers is derived from the set of peers.
				send_statements_about(
					peer,
					peer_data,
					ctx,
					relay_parent,
					candidate_hash,
					&*active_head,
					metrics,
				).await;
			}
		}
	}
}

fn statement_message(relay_parent: Hash, statement: SignedFullStatement)
	-> protocol_v1::ValidationProtocol
{
	protocol_v1::ValidationProtocol::StatementDistribution(
		protocol_v1::StatementDistributionMessage::Statement(relay_parent, statement)
	)
}

/// Circulates a statement to all peers who have not seen it yet, and returns
/// an iterator over peers who need to have dependent statements sent.
#[tracing::instrument(level = "trace", skip(peers, ctx), fields(subsystem = LOG_TARGET))]
async fn circulate_statement(
	peers: &mut HashMap<PeerId, PeerData>,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	relay_parent: Hash,
	stored: &StoredStatement,
) -> Vec<PeerId> {
	let fingerprint = stored.fingerprint();

	let mut peers_to_send = HashMap::new();

	for (peer, data) in peers.iter_mut() {
		if let Some(new_known) = data.send(&relay_parent, &fingerprint) {
			peers_to_send.insert(peer.clone(), new_known);
		}
	}

	// Send all these peers the initial statement.
	if !peers_to_send.is_empty() {
		let payload = statement_message(relay_parent, stored.statement.clone());
		ctx.send_message(AllMessages::NetworkBridge(NetworkBridgeMessage::SendValidationMessage(
			peers_to_send.keys().cloned().collect(),
			payload,
		))).await;
	}

	peers_to_send.into_iter().filter_map(|(peer, needs_dependent)| if needs_dependent {
		Some(peer)
	} else {
		None
	}).collect()
}

/// Send all statements about a given candidate hash to a peer.
#[tracing::instrument(level = "trace", skip(peer_data, ctx, active_head, metrics), fields(subsystem = LOG_TARGET))]
async fn send_statements_about(
	peer: PeerId,
	peer_data: &mut PeerData,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	relay_parent: Hash,
	candidate_hash: CandidateHash,
	active_head: &ActiveHeadData,
	metrics: &Metrics,
) {
	for statement in active_head.statements_about(candidate_hash) {
		if peer_data.send(&relay_parent, &statement.fingerprint()).is_some() {
			let payload = statement_message(
				relay_parent,
				statement.statement.clone(),
			);

			ctx.send_message(AllMessages::NetworkBridge(
				NetworkBridgeMessage::SendValidationMessage(vec![peer.clone()], payload)
			)).await;

			metrics.on_statement_distributed();
		}
	}
}

/// Send all statements at a given relay-parent to a peer.
#[tracing::instrument(level = "trace", skip(peer_data, ctx, active_head, metrics), fields(subsystem = LOG_TARGET))]
async fn send_statements(
	peer: PeerId,
	peer_data: &mut PeerData,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	relay_parent: Hash,
	active_head: &ActiveHeadData,
	metrics: &Metrics,
) {
	for statement in active_head.statements() {
		if peer_data.send(&relay_parent, &statement.fingerprint()).is_some() {
			let payload = statement_message(
				relay_parent,
				statement.statement.clone(),
			);

			ctx.send_message(AllMessages::NetworkBridge(
				NetworkBridgeMessage::SendValidationMessage(vec![peer.clone()], payload)
			)).await;

			metrics.on_statement_distributed();
		}
	}
}

async fn report_peer(
	ctx: &mut impl SubsystemContext,
	peer: PeerId,
	rep: Rep,
) {
	ctx.send_message(AllMessages::NetworkBridge(
		NetworkBridgeMessage::ReportPeer(peer, rep)
	)).await
}

// Handle an incoming wire message. Returns a reference to a newly-stored statement
// if we were not already aware of it, along with the corresponding relay-parent.
//
// This function checks the signature and ensures the statement is compatible with our
// view.
#[tracing::instrument(level = "trace", skip(peer_data, ctx, active_heads, metrics), fields(subsystem = LOG_TARGET))]
async fn handle_incoming_message<'a>(
	peer: PeerId,
	peer_data: &mut PeerData,
	our_view: &View,
	active_heads: &'a mut HashMap<Hash, ActiveHeadData>,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	message: protocol_v1::StatementDistributionMessage,
	metrics: &Metrics,
	statement_listeners: &mut StatementListeners,
) -> Option<(Hash, &'a StoredStatement)> {
	let (relay_parent, statement) = match message {
		protocol_v1::StatementDistributionMessage::Statement(r, s) => (r, s),
	};

	if !our_view.contains(&relay_parent) {
		report_peer(ctx, peer, COST_UNEXPECTED_STATEMENT).await;
		return None;
	}

	let active_head = match active_heads.get_mut(&relay_parent) {
		Some(h) => h,
		None => {
			// This should never be out-of-sync with our view if the view updates
			// correspond to actual `StartWork` messages. So we just log and ignore.
			tracing::warn!(
				requested_relay_parent = %relay_parent,
				"our view out-of-sync with active heads; head not found",
			);
			return None;
		}
	};

	// check the signature on the statement.
	if let Err(()) = check_statement_signature(&active_head, relay_parent, &statement) {
		report_peer(ctx, peer, COST_INVALID_SIGNATURE).await;
		return None;
	}

	// Ensure the statement is stored in the peer data.
	//
	// Note that if the peer is sending us something that is not within their view,
	// it will not be kept within their log.
	let fingerprint = (statement.payload().to_compact(), statement.validator_index());
	let max_message_count = active_head.validators.len() * 2;
	match peer_data.receive(&relay_parent, &fingerprint, max_message_count) {
		Err(rep) => {
			report_peer(ctx, peer, rep).await;
			return None;
		}
		Ok(true) => {
			// Send the peer all statements concerning the candidate that we have,
			// since it appears to have just learned about the candidate.
			send_statements_about(
				peer.clone(),
				peer_data,
				ctx,
				relay_parent,
				fingerprint.0.candidate_hash().clone(),
				&*active_head,
				metrics,
			).await;
		}
		Ok(false) => {}
	}

	inform_statement_listeners(&statement, statement_listeners).await;

	// Note: `peer_data.receive` already ensures that the statement is not an unbounded equivocation
	// or unpinned to a seconded candidate. So it is safe to place it into the storage.
	match active_head.note_statement(statement) {
		NotedStatement::NotUseful => None,
		NotedStatement::UsefulButKnown => {
			report_peer(ctx, peer, BENEFIT_VALID_STATEMENT).await;
			None
		}
		NotedStatement::Fresh(statement) => {
			report_peer(ctx, peer, BENEFIT_VALID_STATEMENT_FIRST).await;
			Some((relay_parent, statement))
		}
	}
}

/// Update a peer's view. Sends all newly unlocked statements based on the previous
#[tracing::instrument(level = "trace", skip(peer_data, ctx, active_heads, metrics), fields(subsystem = LOG_TARGET))]
async fn update_peer_view_and_send_unlocked(
	peer: PeerId,
	peer_data: &mut PeerData,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	active_heads: &HashMap<Hash, ActiveHeadData>,
	new_view: View,
	metrics: &Metrics,
) {
	let old_view = std::mem::replace(&mut peer_data.view, new_view);

	// Remove entries for all relay-parents in the old view but not the new.
	for removed in old_view.difference(&peer_data.view) {
		let _ = peer_data.view_knowledge.remove(removed);
	}

	// Add entries for all relay-parents in the new view but not the old.
	// Furthermore, send all statements we have for those relay parents.
	let new_view = peer_data.view.difference(&old_view).copied().collect::<Vec<_>>();
	for new in new_view.iter().copied() {
		peer_data.view_knowledge.insert(new, Default::default());

		if let Some(active_head) = active_heads.get(&new) {
			send_statements(
				peer.clone(),
				peer_data,
				ctx,
				new,
				active_head,
				metrics,
			).await;
		}
	}
}

#[tracing::instrument(level = "trace", skip(peers, active_heads, ctx, metrics), fields(subsystem = LOG_TARGET))]
async fn handle_network_update(
	peers: &mut HashMap<PeerId, PeerData>,
	active_heads: &mut HashMap<Hash, ActiveHeadData>,
	ctx: &mut impl SubsystemContext<Message = StatementDistributionMessage>,
	our_view: &mut View,
	update: NetworkBridgeEvent<protocol_v1::StatementDistributionMessage>,
	metrics: &Metrics,
	statement_listeners: &mut StatementListeners,
) {
	match update {
		NetworkBridgeEvent::PeerConnected(peer, _role) => {
			peers.insert(peer, PeerData {
				view: Default::default(),
				view_knowledge: Default::default(),
			});
		}
		NetworkBridgeEvent::PeerDisconnected(peer) => {
			peers.remove(&peer);
		}
		NetworkBridgeEvent::PeerMessage(peer, message) => {
			match peers.get_mut(&peer) {
				Some(data) => {
					let new_stored = handle_incoming_message(
						peer,
						data,
						&*our_view,
						active_heads,
						ctx,
						message,
						metrics,
						statement_listeners,
					).await;

					if let Some((relay_parent, new)) = new_stored {
						// When we receive a new message from a peer, we forward it to the
						// candidate backing subsystem.
						let message = AllMessages::CandidateBacking(
							CandidateBackingMessage::Statement(relay_parent, new.statement.clone())
						);
						ctx.send_message(message).await;
					}
				}
				None => (),
			}

		}
		NetworkBridgeEvent::PeerViewChange(peer, view) => {
			match peers.get_mut(&peer) {
				Some(data) => {
					update_peer_view_and_send_unlocked(
						peer,
						data,
						ctx,
						&*active_heads,
						view,
						metrics,
					).await
				}
				None => (),
			}
		}
		NetworkBridgeEvent::OurViewChange(view) => {
			let old_view = std::mem::replace(our_view, view);
			active_heads.retain(|head, _| our_view.contains(head));

			for new in our_view.difference(&old_view) {
				if !active_heads.contains_key(&new) {
					tracing::warn!(
						target: LOG_TARGET,
						unknown_hash = %new,
						"Our network bridge view update \
						inconsistent with `StartWork` messages we have received from overseer. \
						Contains unknown hash.",
					);
				}
			}
		}
	}

}

impl StatementDistribution {
	#[tracing::instrument(skip(self, ctx), fields(subsystem = LOG_TARGET))]
	async fn run(
		self,
		mut ctx: impl SubsystemContext<Message = StatementDistributionMessage>,
	) -> SubsystemResult<()> {
		let mut peers: HashMap<PeerId, PeerData> = HashMap::new();
		let mut our_view = View::default();
		let mut active_heads: HashMap<Hash, ActiveHeadData> = HashMap::new();
		let mut statement_listeners = StatementListeners::new();
		let metrics = self.metrics;

		loop {
			let message = ctx.recv().await?;
			match message {
				FromOverseer::Signal(OverseerSignal::ActiveLeaves(ActiveLeavesUpdate { activated, .. })) => {
					let _timer = metrics.time_active_leaves_update();

					for relay_parent in activated {
						let (validators, session_index) = {
							let (val_tx, val_rx) = oneshot::channel();
							let (session_tx, session_rx) = oneshot::channel();

							let val_message = AllMessages::RuntimeApi(
								RuntimeApiMessage::Request(
									relay_parent,
									RuntimeApiRequest::Validators(val_tx),
								),
							);
							let session_message = AllMessages::RuntimeApi(
								RuntimeApiMessage::Request(
									relay_parent,
									RuntimeApiRequest::SessionIndexForChild(session_tx),
								),
							);

							ctx.send_messages(
								std::iter::once(val_message).chain(std::iter::once(session_message))
							).await;

							match (val_rx.await?, session_rx.await?) {
								(Ok(v), Ok(s)) => (v, s),
								(Err(e), _) | (_, Err(e)) => {
									tracing::warn!(
										target: LOG_TARGET,
										err = ?e,
										"Failed to fetch runtime API data for active leaf",
									);

									// Lacking this bookkeeping might make us behave funny, although
									// not in any slashable way. But we shouldn't take down the node
									// on what are likely spurious runtime API errors.
									continue;
								}
							}
						};

						active_heads.entry(relay_parent)
							.or_insert(ActiveHeadData::new(validators, session_index));
					}
				}
				FromOverseer::Signal(OverseerSignal::BlockFinalized(_block_hash)) => {
					// do nothing
				}
				FromOverseer::Signal(OverseerSignal::Conclude) => break,
				FromOverseer::Communication { msg } => match msg {
					StatementDistributionMessage::Share(relay_parent, statement) => {
						let _timer = metrics.time_share();

						inform_statement_listeners(
							&statement,
							&mut statement_listeners,
						).await;
						circulate_statement_and_dependents(
							&mut peers,
							&mut active_heads,
							&mut ctx,
							relay_parent,
							statement,
							&metrics,
						).await;
					}
					StatementDistributionMessage::NetworkBridgeUpdateV1(event) => {
						let _timer = metrics.time_network_bridge_update_v1();

						handle_network_update(
							&mut peers,
							&mut active_heads,
							&mut ctx,
							&mut our_view,
							event,
							&metrics,
							&mut statement_listeners,
						).await;
					}
					StatementDistributionMessage::RegisterStatementListener(tx) => {
						statement_listeners.push(tx);
					}
				}
			}
		}
		Ok(())
	}
}

#[derive(Clone)]
struct MetricsInner {
	statements_distributed: prometheus::Counter<prometheus::U64>,
	active_leaves_update: prometheus::Histogram,
	share: prometheus::Histogram,
	network_bridge_update_v1: prometheus::Histogram,
}

/// Statement Distribution metrics.
#[derive(Default, Clone)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_statement_distributed(&self) {
		if let Some(metrics) = &self.0 {
			metrics.statements_distributed.inc();
		}
	}

	/// Provide a timer for `active_leaves_update` which observes on drop.
	fn time_active_leaves_update(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.active_leaves_update.start_timer())
	}

	/// Provide a timer for `share` which observes on drop.
	fn time_share(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.share.start_timer())
	}

	/// Provide a timer for `network_bridge_update_v1` which observes on drop.
	fn time_network_bridge_update_v1(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.network_bridge_update_v1.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> std::result::Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			statements_distributed: prometheus::register(
				prometheus::Counter::new(
					"parachain_statements_distributed_total",
					"Number of candidate validity statements distributed to other peers."
				)?,
				registry,
			)?,
			active_leaves_update: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_statement_distribution_active_leaves_update",
						"Time spent within `statement_distribution::active_leaves_update`",
					)
				)?,
				registry,
			)?,
			share: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_statement_distribution_share",
						"Time spent within `statement_distribution::share`",
					)
				)?,
				registry,
			)?,
			network_bridge_update_v1: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_statement_distribution_network_bridge_update_v1",
						"Time spent within `statement_distribution::network_bridge_update_v1`",
					)
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}