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

//! Implements a `CandidateBackingSubsystem`.

#![deny(unused_crate_dependencies)]

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::pin::Pin;
use std::sync::Arc;

use bitvec::vec::BitVec;
use futures::{channel::{mpsc, oneshot}, Future, FutureExt, SinkExt, StreamExt};

use sp_keystore::SyncCryptoStorePtr;
use indracore_primitives::v1::{
	CommittedCandidateReceipt, BackedCandidate, Id as ParaId, ValidatorId,
	ValidatorIndex, SigningContext, PoV, CandidateHash,
	CandidateDescriptor, AvailableData, ValidatorSignature, Hash, CandidateReceipt,
	CoreState, CoreIndex, CollatorId, ValidityAttestation, CandidateCommitments,
};
use indracore_node_primitives::{
	FromTableMisbehavior, Statement, SignedFullStatement, MisbehaviorReport, ValidationResult,
};
use indracore_subsystem::{
	messages::{
		AllMessages, AvailabilityStoreMessage, CandidateBackingMessage, CandidateSelectionMessage,
		CandidateValidationMessage, PoVDistributionMessage, ProvisionableData,
		ProvisionerMessage, StatementDistributionMessage, ValidationFailed, RuntimeApiRequest,
	},
};
use indracore_node_subsystem_util::{
	self as util,
	request_session_index_for_child,
	request_validator_groups,
	request_validators,
	request_from_runtime,
	Validator,
	delegated_subsystem,
	FromJobCommand,
	metrics::{self, prometheus},
};
use statement_table::{
	generic::AttestedCandidate as TableAttestedCandidate,
	Context as TableContextTrait,
	Table,
	v1::{
		Statement as TableStatement,
		SignedStatement as TableSignedStatement, Summary as TableSummary,
	},
};
use thiserror::Error;

const LOG_TARGET: &str = "candidate_backing";

#[derive(Debug, Error)]
enum Error {
	#[error("Candidate is not found")]
	CandidateNotFound,
	#[error("Signature is invalid")]
	InvalidSignature,
	#[error("Failed to send candidates {0:?}")]
	Send(Vec<BackedCandidate>),
	#[error("FetchPoV channel closed before receipt")]
	FetchPoV(#[source] oneshot::Canceled),
	#[error("ValidateFromChainState channel closed before receipt")]
	ValidateFromChainState(#[source] oneshot::Canceled),
	#[error("StoreAvailableData channel closed before receipt")]
	StoreAvailableData(#[source] oneshot::Canceled),
	#[error("a channel was closed before receipt in try_join!")]
	JoinMultiple(#[source] oneshot::Canceled),
	#[error("Obtaining erasure chunks failed")]
	ObtainErasureChunks(#[from] erasure_coding::Error),
	#[error(transparent)]
	ValidationFailed(#[from] ValidationFailed),
	#[error(transparent)]
	Mpsc(#[from] mpsc::SendError),
	#[error(transparent)]
	UtilError(#[from] util::Error),
}

enum ValidatedCandidateCommand {
	// We were instructed to second the candidate.
	Second(BackgroundValidationResult),
	// We were instructed to validate the candidate.
	Attest(BackgroundValidationResult),
}

impl std::fmt::Debug for ValidatedCandidateCommand {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let candidate_hash = self.candidate_hash();
		match *self {
			ValidatedCandidateCommand::Second(_) =>
				write!(f, "Second({})", candidate_hash),
			ValidatedCandidateCommand::Attest(_) =>
				write!(f, "Attest({})", candidate_hash),
		}
	}
}

impl ValidatedCandidateCommand {
	fn candidate_hash(&self) -> CandidateHash {
		match *self {
			ValidatedCandidateCommand::Second(Ok((ref candidate, _, _))) => candidate.hash(),
			ValidatedCandidateCommand::Second(Err(ref candidate)) => candidate.hash(),
			ValidatedCandidateCommand::Attest(Ok((ref candidate, _, _))) => candidate.hash(),
			ValidatedCandidateCommand::Attest(Err(ref candidate)) => candidate.hash(),
		}
	}
}

/// Holds all data needed for candidate backing job operation.
struct CandidateBackingJob {
	/// The hash of the relay parent on top of which this job is doing it's work.
	parent: Hash,
	/// Outbound message channel sending part.
	tx_from: mpsc::Sender<FromJobCommand>,
	/// The `ParaId` assigned to this validator
	assignment: Option<ParaId>,
	/// The collator required to author the candidate, if any.
	required_collator: Option<CollatorId>,
	/// We issued `Seconded`, `Valid` or `Invalid` statements on about these candidates.
	issued_statements: HashSet<CandidateHash>,
	/// These candidates are undergoing validation in the background.
	awaiting_validation: HashSet<CandidateHash>,
	/// `Some(h)` if this job has already issues `Seconded` statemt for some candidate with `h` hash.
	seconded: Option<CandidateHash>,
	/// The candidates that are includable, by hash. Each entry here indicates
	/// that we've sent the provisioner the backed candidate.
	backed: HashSet<CandidateHash>,
	/// We have already reported misbehaviors for these validators.
	reported_misbehavior_for: HashSet<ValidatorIndex>,
	keystore: SyncCryptoStorePtr,
	table: Table<TableContext>,
	table_context: TableContext,
	background_validation: mpsc::Receiver<ValidatedCandidateCommand>,
	background_validation_tx: mpsc::Sender<ValidatedCandidateCommand>,
	metrics: Metrics,
}

const fn group_quorum(n_validators: usize) -> usize {
	(n_validators / 2) + 1
}

#[derive(Default)]
struct TableContext {
	signing_context: SigningContext,
	validator: Option<Validator>,
	groups: HashMap<ParaId, Vec<ValidatorIndex>>,
	validators: Vec<ValidatorId>,
}

impl TableContextTrait for TableContext {
	type AuthorityId = ValidatorIndex;
	type Digest = CandidateHash;
	type GroupId = ParaId;
	type Signature = ValidatorSignature;
	type Candidate = CommittedCandidateReceipt;

	fn candidate_digest(candidate: &CommittedCandidateReceipt) -> CandidateHash {
		candidate.hash()
	}

	fn candidate_group(candidate: &CommittedCandidateReceipt) -> ParaId {
		candidate.descriptor().para_id
	}

	fn is_member_of(&self, authority: &ValidatorIndex, group: &ParaId) -> bool {
		self.groups.get(group).map_or(false, |g| g.iter().position(|a| a == authority).is_some())
	}

	fn requisite_votes(&self, group: &ParaId) -> usize {
		self.groups.get(group).map_or(usize::max_value(), |g| group_quorum(g.len()))
	}
}

struct InvalidErasureRoot;

// It looks like it's not possible to do an `impl From` given the current state of
// the code. So this does the necessary conversion.
fn primitive_statement_to_table(s: &SignedFullStatement) -> TableSignedStatement {
	let statement = match s.payload() {
		Statement::Seconded(c) => TableStatement::Candidate(c.clone()),
		Statement::Valid(h) => TableStatement::Valid(h.clone()),
		Statement::Invalid(h) => TableStatement::Invalid(h.clone()),
	};

	TableSignedStatement {
		statement,
		signature: s.signature().clone(),
		sender: s.validator_index(),
	}
}

#[tracing::instrument(level = "trace", skip(attested, table_context), fields(subsystem = LOG_TARGET))]
fn table_attested_to_backed(
	attested: TableAttestedCandidate<
		ParaId,
		CommittedCandidateReceipt,
		ValidatorIndex,
		ValidatorSignature,
	>,
	table_context: &TableContext,
) -> Option<BackedCandidate> {
	let TableAttestedCandidate { candidate, validity_votes, group_id: para_id } = attested;

	let (ids, validity_votes): (Vec<_>, Vec<ValidityAttestation>) = validity_votes
		.into_iter()
		.map(|(id, vote)| (id, vote.into()))
		.unzip();

	let group = table_context.groups.get(&para_id)?;

	let mut validator_indices = BitVec::with_capacity(group.len());

	validator_indices.resize(group.len(), false);

	// The order of the validity votes in the backed candidate must match
	// the order of bits set in the bitfield, which is not necessarily
	// the order of the `validity_votes` we got from the table.
	let mut vote_positions = Vec::with_capacity(validity_votes.len());
	for (orig_idx, id) in ids.iter().enumerate() {
		if let Some(position) = group.iter().position(|x| x == id) {
			validator_indices.set(position, true);
			vote_positions.push((orig_idx, position));
		} else {
			tracing::warn!(
				target: LOG_TARGET,
				"Logic error: Validity vote from table does not correspond to group",
			);

			return None;
		}
	}
	vote_positions.sort_by_key(|(_orig, pos_in_group)| *pos_in_group);

	Some(BackedCandidate {
		candidate,
		validity_votes: vote_positions.into_iter()
			.map(|(pos_in_votes, _pos_in_group)| validity_votes[pos_in_votes].clone())
			.collect(),
		validator_indices,
	})
}

async fn store_available_data(
	tx_from: &mut mpsc::Sender<FromJobCommand>,
	id: Option<ValidatorIndex>,
	n_validators: u32,
	candidate_hash: CandidateHash,
	available_data: AvailableData,
) -> Result<(), Error> {
	let (tx, rx) = oneshot::channel();
	tx_from.send(AllMessages::AvailabilityStore(
			AvailabilityStoreMessage::StoreAvailableData(
				candidate_hash,
				id,
				n_validators,
				available_data,
				tx,
			)
		).into()
	).await?;

	let _ = rx.await.map_err(Error::StoreAvailableData)?;

	Ok(())
}

// Make a `PoV` available.
//
// This will compute the erasure root internally and compare it to the expected erasure root.
// This returns `Err()` iff there is an internal error. Otherwise, it returns either `Ok(Ok(()))` or `Ok(Err(_))`.
#[tracing::instrument(level = "trace", skip(tx_from, pov), fields(subsystem = LOG_TARGET))]
async fn make_pov_available(
	tx_from: &mut mpsc::Sender<FromJobCommand>,
	validator_index: Option<ValidatorIndex>,
	n_validators: usize,
	pov: Arc<PoV>,
	candidate_hash: CandidateHash,
	validation_data: indracore_primitives::v1::PersistedValidationData,
	expected_erasure_root: Hash,
) -> Result<Result<(), InvalidErasureRoot>, Error> {
	let available_data = AvailableData {
		pov,
		validation_data,
	};

	let chunks = erasure_coding::obtain_chunks_v1(
		n_validators,
		&available_data,
	)?;

	let branches = erasure_coding::branches(chunks.as_ref());
	let erasure_root = branches.root();

	if erasure_root != expected_erasure_root {
		return Ok(Err(InvalidErasureRoot));
	}

	store_available_data(
		tx_from,
		validator_index,
		n_validators as u32,
		candidate_hash,
		available_data,
	).await?;

	Ok(Ok(()))
}

async fn request_pov_from_distribution(
	tx_from: &mut mpsc::Sender<FromJobCommand>,
	parent: Hash,
	descriptor: CandidateDescriptor,
) -> Result<Arc<PoV>, Error> {
	let (tx, rx) = oneshot::channel();

	tx_from.send(AllMessages::PoVDistribution(
		PoVDistributionMessage::FetchPoV(parent, descriptor, tx)
	).into()).await?;

	rx.await.map_err(Error::FetchPoV)
}

async fn request_candidate_validation(
	tx_from: &mut mpsc::Sender<FromJobCommand>,
	candidate: CandidateDescriptor,
	pov: Arc<PoV>,
) -> Result<ValidationResult, Error> {
	let (tx, rx) = oneshot::channel();

	tx_from.send(AllMessages::CandidateValidation(
			CandidateValidationMessage::ValidateFromChainState(
				candidate,
				pov,
				tx,
			)
		).into()
	).await?;

	match rx.await {
		Ok(Ok(validation_result)) => Ok(validation_result),
		Ok(Err(err)) => Err(Error::ValidationFailed(err)),
		Err(err) => Err(Error::ValidateFromChainState(err)),
	}
}

type BackgroundValidationResult = Result<(CandidateReceipt, CandidateCommitments, Arc<PoV>), CandidateReceipt>;

struct BackgroundValidationParams<F> {
	tx_from: mpsc::Sender<FromJobCommand>,
	tx_command: mpsc::Sender<ValidatedCandidateCommand>,
	candidate: CandidateReceipt,
	relay_parent: Hash,
	pov: Option<Arc<PoV>>,
	validator_index: Option<ValidatorIndex>,
	n_validators: usize,
	make_command: F,
}

async fn validate_and_make_available(
	params: BackgroundValidationParams<impl Fn(BackgroundValidationResult) -> ValidatedCandidateCommand>,
) -> Result<(), Error> {
	let BackgroundValidationParams {
		mut tx_from,
		mut tx_command,
		candidate,
		relay_parent,
		pov,
		validator_index,
		n_validators,
		make_command,
	} = params;

	let pov = match pov {
		Some(pov) => pov,
		None => request_pov_from_distribution(
			&mut tx_from,
			relay_parent,
			candidate.descriptor.clone(),
		).await?,
	};

	let v = request_candidate_validation(&mut tx_from, candidate.descriptor.clone(), pov.clone()).await?;

	let expected_commitments_hash = candidate.commitments_hash;

	let res = match v {
		ValidationResult::Valid(commitments, validation_data) => {
			// If validation produces a new set of commitments, we vote the candidate as invalid.
			if commitments.hash() != expected_commitments_hash {
				tracing::trace!(
					target: LOG_TARGET,
					candidate_receipt = ?candidate,
					actual_commitments = ?commitments,
					"Commitments obtained with validation don't match the announced by the candidate receipt",
				);
				Err(candidate)
			} else {
				let erasure_valid = make_pov_available(
					&mut tx_from,
					validator_index,
					n_validators,
					pov.clone(),
					candidate.hash(),
					validation_data,
					candidate.descriptor.erasure_root,
				).await?;

				match erasure_valid {
					Ok(()) => Ok((candidate, commitments, pov.clone())),
					Err(InvalidErasureRoot) => {
						tracing::trace!(
							target: LOG_TARGET,
							candidate_receipt = ?candidate,
							actual_commitments = ?commitments,
							"Erasure root doesn't match the announced by the candidate receipt",
						);
						Err(candidate)
					},
				}
			}
		}
		ValidationResult::Invalid(reason) => {
			tracing::trace!(
				target: LOG_TARGET,
				candidate_receipt = ?candidate,
				reason = ?reason,
				"Validation yielded an invalid candidate",
			);
			Err(candidate)
		}
	};

	let command = make_command(res);
	tx_command.send(command).await?;
	Ok(())
}

impl CandidateBackingJob {
	/// Run asynchronously.
	async fn run_loop(
		mut self,
		mut rx_to: mpsc::Receiver<CandidateBackingMessage>,
	) -> Result<(), Error> {
		loop {
			futures::select! {
				validated_command = self.background_validation.next() => {
					if let Some(c) = validated_command {
						self.handle_validated_candidate_command(c).await?;
					} else {
						panic!("`self` hasn't dropped and `self` holds a reference to this sender; qed");
					}
				}
				to_job = rx_to.next() => match to_job {
					None => break,
					Some(msg) => {
						self.process_msg(msg).await?;
					}
				}
			}
		}

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn handle_validated_candidate_command(
		&mut self,
		command: ValidatedCandidateCommand,
	) -> Result<(), Error> {
		let candidate_hash = command.candidate_hash();
		self.awaiting_validation.remove(&candidate_hash);

		match command {
			ValidatedCandidateCommand::Second(res) => {
				match res {
					Ok((candidate, commitments, pov)) => {
						// sanity check.
						if self.seconded.is_none() && !self.issued_statements.contains(&candidate_hash) {
							self.seconded = Some(candidate_hash);
							self.issued_statements.insert(candidate_hash);
							self.metrics.on_candidate_seconded();

							let statement = Statement::Seconded(CommittedCandidateReceipt {
								descriptor: candidate.descriptor.clone(),
								commitments,
							});
							self.sign_import_and_distribute_statement(statement).await?;
							self.distribute_pov(candidate.descriptor, pov).await?;
						}
					}
					Err(candidate) => {
						self.issue_candidate_invalid_message(candidate).await?;
					}
				}
			}
			ValidatedCandidateCommand::Attest(res) => {
				// sanity check.
				if !self.issued_statements.contains(&candidate_hash) {
					let statement = if res.is_ok() {
						Statement::Valid(candidate_hash)
					} else {
						Statement::Invalid(candidate_hash)
					};

					self.issued_statements.insert(candidate_hash);
					self.sign_import_and_distribute_statement(statement).await?;
				}
			}
		}

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self, params), fields(subsystem = LOG_TARGET))]
	async fn background_validate_and_make_available(
		&mut self,
		params: BackgroundValidationParams<
			impl Fn(BackgroundValidationResult) -> ValidatedCandidateCommand + Send + 'static
		>,
	) -> Result<(), Error> {
		let candidate_hash = params.candidate.hash();

		if self.awaiting_validation.insert(candidate_hash) {
			// spawn background task.
			let bg = async move {
				if let Err(e) = validate_and_make_available(params).await {
					tracing::error!("Failed to validate and make available: {:?}", e);
				}
			};
			self.tx_from.send(FromJobCommand::Spawn("Backing Validation", bg.boxed())).await?;
		}

		Ok(())
	}

	async fn issue_candidate_invalid_message(
		&mut self,
		candidate: CandidateReceipt,
	) -> Result<(), Error> {
		self.tx_from.send(AllMessages::from(CandidateSelectionMessage::Invalid(self.parent, candidate)).into()).await?;

		Ok(())
	}

	/// Kick off background validation with intent to second.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn validate_and_second(
		&mut self,
		candidate: &CandidateReceipt,
		pov: Arc<PoV>,
	) -> Result<(), Error> {
		// Check that candidate is collated by the right collator.
		if self.required_collator.as_ref()
			.map_or(false, |c| c != &candidate.descriptor().collator)
		{
			self.issue_candidate_invalid_message(candidate.clone()).await?;
			return Ok(());
		}

		self.background_validate_and_make_available(BackgroundValidationParams {
			tx_from: self.tx_from.clone(),
			tx_command: self.background_validation_tx.clone(),
			candidate: candidate.clone(),
			relay_parent: self.parent,
			pov: Some(pov),
			validator_index: self.table_context.validator.as_ref().map(|v| v.index()),
			n_validators: self.table_context.validators.len(),
			make_command: ValidatedCandidateCommand::Second,
		}).await?;

		Ok(())
	}

	async fn sign_import_and_distribute_statement(&mut self, statement: Statement) -> Result<(), Error> {
		if let Some(signed_statement) = self.sign_statement(statement).await {
			self.import_statement(&signed_statement).await?;
			self.distribute_signed_statement(signed_statement).await?;
		}

		Ok(())
	}

	/// Check if there have happened any new misbehaviors and issue necessary messages.
	/// TODO: Report multiple misbehaviors 
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn issue_new_misbehaviors(&mut self) -> Result<(), Error> {
		let mut reports = Vec::new();

		for (k, v) in self.table.get_misbehavior().iter() {
			if !self.reported_misbehavior_for.contains(k) {
				self.reported_misbehavior_for.insert(*k);

				let f = FromTableMisbehavior {
					id: *k,
					report: v.clone(),
					signing_context: self.table_context.signing_context.clone(),
					key: self.table_context.validators[*k as usize].clone(),
				};

				if let Ok(report) = MisbehaviorReport::try_from(f) {
					let message = ProvisionerMessage::ProvisionableData(
						self.parent,
						ProvisionableData::MisbehaviorReport(self.parent, report),
					);

					reports.push(message);
				}
			}
		}

		for report in reports.drain(..) {
			self.send_to_provisioner(report).await?
		}

		Ok(())
	}

	/// Import a statement into the statement table and return the summary of the import.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn import_statement(
		&mut self,
		statement: &SignedFullStatement,
	) -> Result<Option<TableSummary>, Error> {
		let stmt = primitive_statement_to_table(statement);

		let summary = self.table.import_statement(&self.table_context, stmt);

		if let Some(ref summary) = summary {
			if let Some(attested) = self.table.attested_candidate(
				&summary.candidate,
				&self.table_context,
			) {
				// `HashSet::insert` returns true if the thing wasn't in there already.
				// one of the few places the Rust-std folks did a bad job with API
				if self.backed.insert(summary.candidate) {
					if let Some(backed) =
						table_attested_to_backed(attested, &self.table_context)
					{
						let message = ProvisionerMessage::ProvisionableData(
							self.parent,
							ProvisionableData::BackedCandidate(backed.receipt()),
						);
						self.send_to_provisioner(message).await?;
					}
				}
			}
		}

		self.issue_new_misbehaviors().await?;

		Ok(summary)
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn process_msg(&mut self, msg: CandidateBackingMessage) -> Result<(), Error> {
		match msg {
			CandidateBackingMessage::Second(_, candidate, pov) => {
				let _timer = self.metrics.time_process_second();

				// Sanity check that candidate is from our assignment.
				if Some(candidate.descriptor().para_id) != self.assignment {
					return Ok(());
				}

				// If the message is a `CandidateBackingMessage::Second`, sign and dispatch a
				// Seconded statement only if we have not seconded any other candidate and
				// have not signed a Valid statement for the requested candidate.
				if self.seconded.is_none() {
					// This job has not seconded a candidate yet.
					let candidate_hash = candidate.hash();
					let pov = Arc::new(pov);

					if !self.issued_statements.contains(&candidate_hash) {
						self.validate_and_second(&candidate, pov.clone()).await?;
					}
				}
			}
			CandidateBackingMessage::Statement(_, statement) => {
				let _timer = self.metrics.time_process_statement();

				self.check_statement_signature(&statement)?;
				match self.maybe_validate_and_import(statement).await {
					Err(Error::ValidationFailed(_)) => return Ok(()),
					Err(e) => return Err(e),
					Ok(()) => (),
				}
			}
			CandidateBackingMessage::GetBackedCandidates(_, requested_candidates, tx) => {
				let _timer = self.metrics.time_get_backed_candidates();

				let backed = requested_candidates
					.into_iter()
					.filter_map(|hash| {
						self.table.attested_candidate(&hash, &self.table_context)
							.and_then(|attested| table_attested_to_backed(attested, &self.table_context))
					})
					.collect();

				tx.send(backed).map_err(|data| Error::Send(data))?;
			}
		}

		Ok(())
	}

	/// Kick off validation work and distribute the result as a signed statement.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn kick_off_validation_work(
		&mut self,
		summary: TableSummary,
	) -> Result<(), Error> {
		let candidate_hash = summary.candidate;

		if self.issued_statements.contains(&candidate_hash) {
			return Ok(())
		}

		// We clone the commitments here because there are borrowck
		// errors relating to this being a struct and methods borrowing the entirety of self
		// and not just those things that the function uses.
		let candidate = self.table.get_candidate(&candidate_hash).ok_or(Error::CandidateNotFound)?.to_plain();
		let descriptor = candidate.descriptor().clone();

		// Check that candidate is collated by the right collator.
		if self.required_collator.as_ref()
			.map_or(false, |c| c != &descriptor.collator)
		{
			// If not, we've got the statement in the table but we will
			// not issue validation work for it.
			//
			// Act as though we've issued a statement.
			self.issued_statements.insert(candidate_hash);
			return Ok(());
		}

		self.background_validate_and_make_available(BackgroundValidationParams {
			tx_from: self.tx_from.clone(),
			tx_command: self.background_validation_tx.clone(),
			candidate,
			relay_parent: self.parent,
			pov: None,
			validator_index: self.table_context.validator.as_ref().map(|v| v.index()),
			n_validators: self.table_context.validators.len(),
			make_command: ValidatedCandidateCommand::Attest,
		}).await
	}

	/// Import the statement and kick off validation work if it is a part of our assignment.
	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn maybe_validate_and_import(
		&mut self,
		statement: SignedFullStatement,
	) -> Result<(), Error> {
		if let Some(summary) = self.import_statement(&statement).await? {
			if let Statement::Seconded(_) = statement.payload() {
				if Some(summary.group_id) == self.assignment {
					self.kick_off_validation_work(summary).await?;
				}
			}
		}

		Ok(())
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	async fn sign_statement(&self, statement: Statement) -> Option<SignedFullStatement> {
		let signed = self.table_context
			.validator
			.as_ref()?
			.sign(self.keystore.clone(), statement)
			.await
			.ok()?;
		self.metrics.on_statement_signed();
		Some(signed)
	}

	#[tracing::instrument(level = "trace", skip(self), fields(subsystem = LOG_TARGET))]
	fn check_statement_signature(&self, statement: &SignedFullStatement) -> Result<(), Error> {
		let idx = statement.validator_index() as usize;

		if self.table_context.validators.len() > idx {
			statement.check_signature(
				&self.table_context.signing_context,
				&self.table_context.validators[idx],
			).map_err(|_| Error::InvalidSignature)?;
		} else {
			return Err(Error::InvalidSignature);
		}

		Ok(())
	}

	async fn send_to_provisioner(&mut self, msg: ProvisionerMessage) -> Result<(), Error> {
		self.tx_from.send(AllMessages::from(msg).into()).await?;

		Ok(())
	}

	async fn distribute_pov(
		&mut self,
		descriptor: CandidateDescriptor,
		pov: Arc<PoV>,
	) -> Result<(), Error> {
		self.tx_from.send(AllMessages::from(
			PoVDistributionMessage::DistributePoV(self.parent, descriptor, pov),
		).into()).await.map_err(Into::into)
	}

	async fn distribute_signed_statement(&mut self, s: SignedFullStatement) -> Result<(), Error> {
		let smsg = StatementDistributionMessage::Share(self.parent, s);

		self.tx_from.send(AllMessages::from(smsg).into()).await?;

		Ok(())
	}
}

impl util::JobTrait for CandidateBackingJob {
	type ToJob = CandidateBackingMessage;
	type Error = Error;
	type RunArgs = SyncCryptoStorePtr;
	type Metrics = Metrics;

	const NAME: &'static str = "CandidateBackingJob";

	#[tracing::instrument(skip(keystore, metrics, rx_to, tx_from), fields(subsystem = LOG_TARGET))]
	fn run(
		parent: Hash,
		keystore: SyncCryptoStorePtr,
		metrics: Metrics,
		rx_to: mpsc::Receiver<Self::ToJob>,
		mut tx_from: mpsc::Sender<FromJobCommand>,
	) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send>> {
		async move {
			macro_rules! try_runtime_api {
				($x: expr) => {
					match $x {
						Ok(x) => x,
						Err(e) => {
							tracing::warn!(
								target: LOG_TARGET,
								err = ?e,
								"Failed to fetch runtime API data for job",
							);

							// We can't do candidate validation work if we don't have the
							// requisite runtime API data. But these errors should not take
							// down the node.
							return Ok(());
						}
					}
				}
			}

			let (validators, groups, session_index, cores) = futures::try_join!(
				try_runtime_api!(request_validators(parent, &mut tx_from).await),
				try_runtime_api!(request_validator_groups(parent, &mut tx_from).await),
				try_runtime_api!(request_session_index_for_child(parent, &mut tx_from).await),
				try_runtime_api!(request_from_runtime(
					parent,
					&mut tx_from,
					|tx| RuntimeApiRequest::AvailabilityCores(tx),
				).await),
			).map_err(Error::JoinMultiple)?;

			let validators = try_runtime_api!(validators);
			let (validator_groups, group_rotation_info) = try_runtime_api!(groups);
			let session_index = try_runtime_api!(session_index);
			let cores = try_runtime_api!(cores);

			let signing_context = SigningContext { parent_hash: parent, session_index };
			let validator = match Validator::construct(
				&validators,
				signing_context,
				keystore.clone(),
			).await {
				Ok(v) => v,
				Err(util::Error::NotAValidator) => { return Ok(()) },
				Err(e) => {
					tracing::warn!(
						target: LOG_TARGET,
						err = ?e,
						"Cannot participate in candidate backing",
					);

					return Ok(())
				}
			};

			let mut groups = HashMap::new();

			let n_cores = cores.len();

			let mut assignment = None;
			for (idx, core) in cores.into_iter().enumerate() {
				// Ignore prospective assignments on occupied cores for the time being.
				if let CoreState::Scheduled(scheduled) = core {
					let core_index = CoreIndex(idx as _);
					let group_index = group_rotation_info.group_for_core(core_index, n_cores);
					if let Some(g) = validator_groups.get(group_index.0 as usize) {
						if g.contains(&validator.index()) {
							assignment = Some((scheduled.para_id, scheduled.collator));
						}
						groups.insert(scheduled.para_id, g.clone());
					}
				}
			}

			let table_context = TableContext {
				groups,
				validators,
				signing_context: validator.signing_context().clone(),
				validator: Some(validator),
			};

			let (assignment, required_collator) = match assignment {
				None => (None, None),
				Some((assignment, required_collator)) => (Some(assignment), required_collator),
			};

			let (background_tx, background_rx) = mpsc::channel(16);
			let job = CandidateBackingJob {
				parent,
				tx_from,
				assignment,
				required_collator,
				issued_statements: HashSet::new(),
				awaiting_validation: HashSet::new(),
				seconded: None,
				backed: HashSet::new(),
				reported_misbehavior_for: HashSet::new(),
				keystore,
				table: Table::default(),
				table_context,
				background_validation: background_rx,
				background_validation_tx: background_tx,
				metrics,
			};

			job.run_loop(rx_to).await
		}
		.boxed()
	}
}

#[derive(Clone)]
struct MetricsInner {
	signed_statements_total: prometheus::Counter<prometheus::U64>,
	candidates_seconded_total: prometheus::Counter<prometheus::U64>,
	process_second: prometheus::Histogram,
	process_statement: prometheus::Histogram,
	get_backed_candidates: prometheus::Histogram,
}

/// Candidate backing metrics.
#[derive(Default, Clone)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_statement_signed(&self) {
		if let Some(metrics) = &self.0 {
			metrics.signed_statements_total.inc();
		}
	}

	fn on_candidate_seconded(&self) {
		if let Some(metrics) = &self.0 {
			metrics.candidates_seconded_total.inc();
		}
	}

	/// Provide a timer for handling `CandidateBackingMessage:Second` which observes on drop.
	fn time_process_second(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.process_second.start_timer())
	}

	/// Provide a timer for handling `CandidateBackingMessage::Statement` which observes on drop.
	fn time_process_statement(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.process_statement.start_timer())
	}

	/// Provide a timer for handling `CandidateBackingMessage::GetBackedCandidates` which observes on drop.
	fn time_get_backed_candidates(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.get_backed_candidates.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			signed_statements_total: prometheus::register(
				prometheus::Counter::new(
					"parachain_candidate_backing_signed_statements_total",
					"Number of statements signed.",
				)?,
				registry,
			)?,
			candidates_seconded_total: prometheus::register(
				prometheus::Counter::new(
					"parachain_candidate_backing_candidates_seconded_total",
					"Number of candidates seconded.",
				)?,
				registry,
			)?,
			process_second: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_backing_process_second",
						"Time spent within `candidate_backing::process_second`",
					)
				)?,
				registry,
			)?,
			process_statement: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_backing_process_statement",
						"Time spent within `candidate_backing::process_statement`",
					)
				)?,
				registry,
			)?,
			get_backed_candidates: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_backing_get_backed_candidates",
						"Time spent within `candidate_backing::get_backed_candidates`",
					)
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}

delegated_subsystem!(CandidateBackingJob(SyncCryptoStorePtr, Metrics) <- CandidateBackingMessage as CandidateBackingSubsystem);
