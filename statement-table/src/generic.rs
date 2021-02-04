// Copyright 2017-2020 Parity Technologies (UK) Ltd.
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

//! The statement table: generic implementation.
//!
//! This stores messages other authorities issue about candidates.
//!
//! These messages are used to create a proposal submitted to a BFT consensus process.
//!
//! Each parachain is associated with a committee of authorities, who issue statements
//! indicating whether the candidate is valid or invalid. Once a threshold of the committee
//! has signed validity statements, the candidate may be marked includable.

use std::collections::hash_map::{HashMap, Entry};
use std::hash::Hash;
use std::fmt::Debug;

use primitives::v1::{ValidityAttestation as PrimitiveValidityAttestation, ValidatorSignature};

use parity_scale_codec::{Encode, Decode};

/// Context for the statement table.
pub trait Context {
	/// A authority ID
	type AuthorityId: Debug + Hash + Eq + Clone;
	/// The digest (hash or other unique attribute) of a candidate.
	type Digest: Debug + Hash + Eq + Clone;
	/// The group ID type
	type GroupId: Debug + Hash + Ord + Eq + Clone;
	/// A signature type.
	type Signature: Debug + Eq + Clone;
	/// Candidate type. In practice this will be a candidate receipt.
	type Candidate: Debug + Ord + Eq + Clone;

	/// get the digest of a candidate.
	fn candidate_digest(candidate: &Self::Candidate) -> Self::Digest;

	/// get the group of a candidate.
	fn candidate_group(candidate: &Self::Candidate) -> Self::GroupId;

	/// Whether a authority is a member of a group.
	/// Members are meant to submit candidates and vote on validity.
	fn is_member_of(&self, authority: &Self::AuthorityId, group: &Self::GroupId) -> bool;

	/// requisite number of votes for validity from a group.
	fn requisite_votes(&self, group: &Self::GroupId) -> usize;
}

/// Statements circulated among peers.
#[derive(PartialEq, Eq, Debug, Clone, Encode, Decode)]
pub enum Statement<C, D> {
	/// Broadcast by an authority to indicate that this is his candidate for
	/// inclusion.
	///
	/// Broadcasting two different candidate messages per round is not allowed.
	#[codec(index = "1")]
	Candidate(C),
	/// Broadcast by a authority to attest that the candidate with given digest
	/// is valid.
	#[codec(index = "2")]
	Valid(D),
	/// Broadcast by a authority to attest that the candidate with given digest
	/// is invalid.
	#[codec(index = "3")]
	Invalid(D),
}

/// A signed statement.
#[derive(PartialEq, Eq, Debug, Clone, Encode, Decode)]
pub struct SignedStatement<C, D, V, S> {
	/// The statement.
	pub statement: Statement<C, D>,
	/// The signature.
	pub signature: S,
	/// The sender.
	pub sender: V,
}

/// Misbehavior: voting more than one way on candidate validity.
///
/// Since there are three possible ways to vote, a double vote is possible in
/// three possible combinations (unordered)
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum ValidityDoubleVote<C, D, S> {
	/// Implicit vote by issuing and explicitly voting validity.
	IssuedAndValidity((C, S), (D, S)),
	/// Implicit vote by issuing and explicitly voting invalidity
	IssuedAndInvalidity((C, S), (D, S)),
	/// Direct votes for validity and invalidity
	ValidityAndInvalidity(C, S, S),
}

/// Misbehavior: multiple signatures on same statement.
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum DoubleSign<C, D, S> {
	/// On candidate.
	Candidate(C, S, S),
	/// On validity.
	Validity(D, S, S),
	/// On invalidity.
	Invalidity(D, S, S),
}

/// Misbehavior: declaring multiple candidates.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct MultipleCandidates<C, S> {
	/// The first candidate seen.
	pub first: (C, S),
	/// The second candidate seen.
	pub second: (C, S),
}

/// Misbehavior: submitted statement for wrong group.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct UnauthorizedStatement<C, D, V, S> {
	/// A signed statement which was submitted without proper authority.
	pub statement: SignedStatement<C, D, V, S>,
}

/// Different kinds of misbehavior. All of these kinds of malicious misbehavior
/// are easily provable and extremely disincentivized.
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Misbehavior<C, D, V, S> {
	/// Voted invalid and valid on validity.
	ValidityDoubleVote(ValidityDoubleVote<C, D, S>),
	/// Submitted multiple candidates.
	MultipleCandidates(MultipleCandidates<C, S>),
	/// Submitted a message that was unauthorized.
	UnauthorizedStatement(UnauthorizedStatement<C, D, V, S>),
	/// Submitted two valid signatures for the same message.
	DoubleSign(DoubleSign<C, D, S>),
}

/// Type alias for misbehavior corresponding to context type.
pub type MisbehaviorFor<C> = Misbehavior<<C as Context>::Candidate, <C as Context>::Digest, <C as Context>::AuthorityId, <C as Context>::Signature>;

// kinds of votes for validity
#[derive(Clone, PartialEq, Eq)]
enum ValidityVote<S: Eq + Clone> {
	// implicit validity vote by issuing
	Issued(S),
	// direct validity vote
	Valid(S),
	// direct invalidity vote
	Invalid(S),
}

/// A summary of import of a statement.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Summary<D, G> {
	/// The digest of the candidate referenced.
	pub candidate: D,
	/// The group that the candidate is in.
	pub group_id: G,
	/// How many validity votes are currently witnessed.
	pub validity_votes: usize,
	/// Whether this has been signalled bad by at least one participant.
	pub signalled_bad: bool,
}

/// A validity attestation.
#[derive(Clone, PartialEq, Decode, Encode)]
pub enum ValidityAttestation<S> {
	/// implicit validity attestation by issuing.
	/// This corresponds to issuance of a `Candidate` statement.
	Implicit(S),
	/// An explicit attestation. This corresponds to issuance of a
	/// `Valid` statement.
	Explicit(S),
}

impl Into<PrimitiveValidityAttestation> for ValidityAttestation<ValidatorSignature> {
	fn into(self) -> PrimitiveValidityAttestation {
		match self {
			Self::Implicit(s) => PrimitiveValidityAttestation::Implicit(s),
			Self::Explicit(s) => PrimitiveValidityAttestation::Explicit(s),
		}
	}
}

/// An attested-to candidate.
#[derive(Clone, PartialEq, Decode, Encode)]
pub struct AttestedCandidate<Group, Candidate, AuthorityId, Signature> {
	/// The group ID that the candidate is in.
	pub group_id: Group,
	/// The candidate data.
	pub candidate: Candidate,
	/// Validity attestations.
	pub validity_votes: Vec<(AuthorityId, ValidityAttestation<Signature>)>,
}

/// Stores votes and data about a candidate.
pub struct CandidateData<C: Context> {
	group_id: C::GroupId,
	candidate: C::Candidate,
	validity_votes: HashMap<C::AuthorityId, ValidityVote<C::Signature>>,
	indicated_bad_by: Vec<C::AuthorityId>,
}

impl<C: Context> CandidateData<C> {
	/// whether this has been indicated bad by anyone.
	pub fn indicated_bad(&self) -> bool {
		!self.indicated_bad_by.is_empty()
	}

	/// Yield a full attestation for a candidate.
	/// If the candidate can be included, it will return `Some`.
	pub fn attested(&self, validity_threshold: usize)
		-> Option<AttestedCandidate<
			C::GroupId, C::Candidate, C::AuthorityId, C::Signature,
		>>
	{
		if self.can_be_included(validity_threshold) {
			let validity_votes: Vec<_> = self.validity_votes.iter()
				.filter_map(|(a, v)| match *v {
					ValidityVote::Invalid(_) => None,

					ValidityVote::Valid(ref s) =>
						Some((a, ValidityAttestation::Explicit(s.clone()))),
					ValidityVote::Issued(ref s) =>
						Some((a, ValidityAttestation::Implicit(s.clone()))),
				})
				.take(validity_threshold)
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect();

			assert!(
				validity_votes.len() == validity_threshold,
				"candidate is includable; therefore there are enough validity votes; qed",
			);

			Some(AttestedCandidate {
				group_id: self.group_id.clone(),
				candidate: self.candidate.clone(),
				validity_votes,
			})
		} else {
			None
		}
	}

	// Candidate data can be included in a proposal
	// if it has enough validity votes
	// and no authorities have called it bad.
	fn can_be_included(&self, validity_threshold: usize) -> bool {
		self.validity_votes.len() >= validity_threshold
	}

	fn summary(&self, digest: C::Digest) -> Summary<C::Digest, C::GroupId> {
		Summary {
			candidate: digest,
			group_id: self.group_id.clone(),
			validity_votes: self.validity_votes.len(),
			signalled_bad: self.indicated_bad(),
		}
	}
}

// authority metadata
struct AuthorityData<C: Context> {
	proposal: Option<(C::Digest, C::Signature)>,
}

impl<C: Context> Default for AuthorityData<C> {
	fn default() -> Self {
		AuthorityData {
			proposal: None,
		}
	}
}

/// Type alias for the result of a statement import.
pub type ImportResult<C> = Result<
	Option<Summary<<C as Context>::Digest, <C as Context>::GroupId>>,
	MisbehaviorFor<C>
>;

/// Stores votes
pub struct Table<C: Context> {
	authority_data: HashMap<C::AuthorityId, AuthorityData<C>>,
	detected_misbehavior: HashMap<C::AuthorityId, MisbehaviorFor<C>>,
	candidate_votes: HashMap<C::Digest, CandidateData<C>>,
	includable_count: HashMap<C::GroupId, usize>,
}

impl<C: Context> Default for Table<C> {
	fn default() -> Self {
		Table {
			authority_data: HashMap::new(),
			detected_misbehavior: HashMap::new(),
			candidate_votes: HashMap::new(),
			includable_count: HashMap::new(),
		}
	}
}

impl<C: Context> Table<C> {
	/// Produce a set of proposed candidates.
	///
	/// This will be at most one per group, consisting of the
	/// best candidate for each group with requisite votes for inclusion.
	///
	/// The vector is sorted in ascending order by group id.
	pub fn proposed_candidates(&self, context: &C) -> Vec<AttestedCandidate<
		C::GroupId, C::Candidate, C::AuthorityId, C::Signature,
	>> {
		use std::collections::BTreeMap;
		use std::collections::btree_map::Entry as BTreeEntry;

		let mut best_candidates = BTreeMap::new();
		for candidate_data in self.candidate_votes.values() {
			let group_id = &candidate_data.group_id;

			if !self.includable_count.contains_key(group_id) {
				continue
			}

			let threshold = context.requisite_votes(group_id);

			if !candidate_data.can_be_included(threshold) { continue }
			match best_candidates.entry(group_id.clone()) {
				BTreeEntry::Vacant(vacant) => {
					vacant.insert((candidate_data, threshold));
				},
				BTreeEntry::Occupied(mut occ) => {
					let candidate_ref = occ.get_mut();
					if candidate_ref.0.candidate > candidate_data.candidate {
						candidate_ref.0 = candidate_data;
					}
				}
			}
		}

		best_candidates.values()
			.map(|&(candidate_data, threshold)|
				candidate_data.attested(threshold)
					.expect("candidate has been checked includable; \
						therefore an attestation can be constructed; qed")
			)
			.collect::<Vec<_>>()
	}

	/// Whether a candidate can be included.
	pub fn candidate_includable(&self, digest: &C::Digest, context: &C) -> bool {
		self.candidate_votes.get(digest).map_or(false, |data| {
			let v_threshold = context.requisite_votes(&data.group_id);
			data.can_be_included(v_threshold)
		})
	}

	/// Get the attested candidate for `digest`.
	///
	/// Returns `Some(_)` if the candidate exists and is includable.
	pub fn attested_candidate(&self, digest: &C::Digest, context: &C)
		-> Option<AttestedCandidate<
			C::GroupId, C::Candidate, C::AuthorityId, C::Signature,
		>>
	{
		self.candidate_votes.get(digest).and_then(|data| {
			let v_threshold = context.requisite_votes(&data.group_id);
			data.attested(v_threshold)
		})
	}

	/// Import a signed statement. Signatures should be checked for validity, and the
	/// sender should be checked to actually be an authority.
	///
	/// Validity and invalidity statements are only valid if the corresponding
	/// candidate has already been imported.
	///
	/// If this returns `None`, the statement was either duplicate or invalid.
	pub fn import_statement(
		&mut self,
		context: &C,
		statement: SignedStatement<C::Candidate, C::Digest, C::AuthorityId, C::Signature>,
	) -> Option<Summary<C::Digest, C::GroupId>> {
		let SignedStatement { statement, signature, sender: signer } = statement;

		let res = match statement {
			Statement::Candidate(candidate) => self.import_candidate(
				context,
				signer.clone(),
				candidate,
				signature
			),
			Statement::Valid(digest) => self.validity_vote(
				context,
				signer.clone(),
				digest,
				ValidityVote::Valid(signature),
			),
			Statement::Invalid(digest) => self.validity_vote(
				context,
				signer.clone(),
				digest,
				ValidityVote::Invalid(signature),
			),
		};

		match res {
			Ok(maybe_summary) => maybe_summary,
			Err(misbehavior) => {
				// all misbehavior in agreement is provable and actively malicious.
				// punishments are not cumulative.
				self.detected_misbehavior.insert(signer, misbehavior);
				None
			}
		}
	}

	/// Get a candidate by digest.
	pub fn get_candidate(&self, digest: &C::Digest) -> Option<&C::Candidate> {
		self.candidate_votes.get(digest).map(|d| &d.candidate)
	}

	/// Access all witnessed misbehavior.
	pub fn get_misbehavior(&self)
		-> &HashMap<C::AuthorityId, MisbehaviorFor<C>>
	{
		&self.detected_misbehavior
	}

	/// Get the current number of parachains with includable candidates.
	pub fn includable_count(&self) -> usize {
		self.includable_count.len()
	}

	fn import_candidate(
		&mut self,
		context: &C,
		from: C::AuthorityId,
		candidate: C::Candidate,
		signature: C::Signature,
	) -> ImportResult<C> {
		let group = C::candidate_group(&candidate);
		if !context.is_member_of(&from, &group) {
			return Err(Misbehavior::UnauthorizedStatement(UnauthorizedStatement {
				statement: SignedStatement {
					signature,
					statement: Statement::Candidate(candidate),
					sender: from,
				},
			}));
		}

		// check that authority hasn't already specified another candidate.
		let digest = C::candidate_digest(&candidate);

		let new_proposal = match self.authority_data.entry(from.clone()) {
			Entry::Occupied(mut occ) => {
				// if digest is different, fetch candidate and
				// note misbehavior.
				let existing = occ.get_mut();

				if let Some((ref old_digest, ref old_sig)) = existing.proposal {
					if old_digest != &digest {
						const EXISTENCE_PROOF: &str =
							"when proposal first received from authority, candidate \
							votes entry is created. proposal here is `Some`, therefore \
							candidate votes entry exists; qed";

						let old_candidate = self.candidate_votes.get(old_digest)
							.expect(EXISTENCE_PROOF)
							.candidate
							.clone();

						return Err(Misbehavior::MultipleCandidates(MultipleCandidates {
							first: (old_candidate, old_sig.clone()),
							second: (candidate, signature.clone()),
						}));
					}

					false
				} else {
					existing.proposal = Some((digest.clone(), signature.clone()));
					true
				}
			}
			Entry::Vacant(vacant) => {
				vacant.insert(AuthorityData {
					proposal: Some((digest.clone(), signature.clone())),
				});
				true
			}
		};

		// NOTE: altering this code may affect the existence proof above. ensure it remains
		// valid.
		if new_proposal {
			self.candidate_votes.entry(digest.clone()).or_insert_with(move || CandidateData {
				group_id: group,
				candidate,
				validity_votes: HashMap::new(),
				indicated_bad_by: Vec::new(),
			});
		}

		self.validity_vote(
			context,
			from,
			digest,
			ValidityVote::Issued(signature),
		)
	}

	fn validity_vote(
		&mut self,
		context: &C,
		from: C::AuthorityId,
		digest: C::Digest,
		vote: ValidityVote<C::Signature>,
	) -> ImportResult<C> {
		let votes = match self.candidate_votes.get_mut(&digest) {
			None => return Ok(None),
			Some(votes) => votes,
		};

		let v_threshold = context.requisite_votes(&votes.group_id);
		let was_includable = votes.can_be_included(v_threshold);

		// check that this authority actually can vote in this group.
		if !context.is_member_of(&from, &votes.group_id) {
			let (sig, valid) = match vote {
				ValidityVote::Valid(s) => (s, true),
				ValidityVote::Invalid(s) => (s, false),
				ValidityVote::Issued(_) =>
					panic!("implicit issuance vote only cast from `import_candidate` after \
							checking group membership of issuer; qed"),
			};

			return Err(Misbehavior::UnauthorizedStatement(UnauthorizedStatement {
				statement: SignedStatement {
					signature: sig,
					sender: from,
					statement: if valid {
						Statement::Valid(digest)
					} else {
						Statement::Invalid(digest)
					}
				}
			}));
		}

		// check for double votes.
		match votes.validity_votes.entry(from.clone()) {
			Entry::Occupied(occ) => {
				let make_vdv = |v| Misbehavior::ValidityDoubleVote(v);
				let make_ds = |ds| Misbehavior::DoubleSign(ds);
				return if occ.get() != &vote {
					Err(match (occ.get().clone(), vote) {
						// valid vote conflicting with candidate statement
						(ValidityVote::Issued(iss), ValidityVote::Valid(good)) |
						(ValidityVote::Valid(good), ValidityVote::Issued(iss)) =>
							make_vdv(ValidityDoubleVote::IssuedAndValidity((votes.candidate.clone(), iss), (digest, good))),

						// invalid vote conflicting with candidate statement
						(ValidityVote::Issued(iss), ValidityVote::Invalid(bad)) |
						(ValidityVote::Invalid(bad), ValidityVote::Issued(iss)) =>
							make_vdv(ValidityDoubleVote::IssuedAndInvalidity((votes.candidate.clone(), iss), (digest, bad))),

						// valid vote conflicting with invalid vote
						(ValidityVote::Valid(good), ValidityVote::Invalid(bad)) |
						(ValidityVote::Invalid(bad), ValidityVote::Valid(good)) =>
							make_vdv(ValidityDoubleVote::ValidityAndInvalidity(votes.candidate.clone(), good, bad)),

						// two signatures on same candidate
						(ValidityVote::Issued(a), ValidityVote::Issued(b)) =>
							make_ds(DoubleSign::Candidate(votes.candidate.clone(), a, b)),

						// two signatures on same validity vote
						(ValidityVote::Valid(a), ValidityVote::Valid(b)) =>
							make_ds(DoubleSign::Validity(digest, a, b)),

						// two signature on same invalidity vote
						(ValidityVote::Invalid(a), ValidityVote::Invalid(b)) =>
							make_ds(DoubleSign::Invalidity(digest, a, b)),
					})
				} else {
					Ok(None)
				}
			}
			Entry::Vacant(vacant) => {
				if let ValidityVote::Invalid(_) = vote {
					votes.indicated_bad_by.push(from.clone());
				}

				vacant.insert(vote);
			}
		}

		let is_includable = votes.can_be_included(v_threshold);
		update_includable_count(&mut self.includable_count, &votes.group_id, was_includable, is_includable);

		Ok(Some(votes.summary(digest)))
	}
}

fn update_includable_count<G: Hash + Eq + Clone>(
	map: &mut HashMap<G, usize>,
	group_id: &G,
	was_includable: bool,
	is_includable: bool,
) {
	if was_includable && !is_includable {
		if let Entry::Occupied(mut entry) = map.entry(group_id.clone()) {
			*entry.get_mut() -= 1;
			if *entry.get() == 0 {
				entry.remove();
			}
		}
	}

	if !was_includable && is_includable {
		*map.entry(group_id.clone()).or_insert(0) += 1;
	}
}
