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

//! The inclusion module is responsible for inclusion and availability of scheduled parachains
//! and parathreads.
//!
//! It is responsible for carrying candidates from being backable to being backed, and then from backed
//! to included.

use bitvec::{order::Lsb0 as BitOrderLsb0, vec::BitVec};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    traits::Get, weights::Weight, IterableStorageMap,
};
use parity_scale_codec::{Decode, Encode};
use primitives::v1::{
    AvailabilityBitfield, BackedCandidate, CandidateCommitments, CandidateDescriptor,
    CandidateReceipt, CommittedCandidateReceipt, CoreIndex, GroupIndex, HeadData, Id as ParaId,
    SignedAvailabilityBitfields, SigningContext, ValidatorId, ValidatorIndex,
};
use sp_runtime::{
    traits::{One, Saturating},
    DispatchError,
};
use sp_staking::SessionIndex;
use sp_std::prelude::*;

use crate::{configuration, dmp, hrmp, paras, scheduler::CoreAssignment, ump};

/// A bitfield signed by a validator indicating that it is keeping its piece of the erasure-coding
/// for any backed candidates referred to by a `1` bit available.
///
/// The bitfield's signature should be checked at the point of submission. Afterwards it can be
/// dropped.
#[derive(Encode, Decode)]
#[cfg_attr(test, derive(Debug))]
pub struct AvailabilityBitfieldRecord<N> {
    bitfield: AvailabilityBitfield, // one bit per core.
    submitted_at: N,                // for accounting, as meaning of bits may change over time.
}

/// A backed candidate pending availability.
// TODO: split this type and change this to hold a plain `CandidateReceipt`.
#[derive(Encode, Decode, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct CandidatePendingAvailability<H, N> {
    /// The availability core this is assigned to.
    core: CoreIndex,
    /// The candidate descriptor.
    descriptor: CandidateDescriptor<H>,
    /// The received availability votes. One bit per validator.
    availability_votes: BitVec<BitOrderLsb0, u8>,
    /// The block number of the relay-parent of the receipt.
    relay_parent_number: N,
    /// The block number of the relay-chain block this was backed in.
    backed_in_number: N,
}

impl<H, N> CandidatePendingAvailability<H, N> {
    /// Get the availability votes on the candidate.
    pub(crate) fn availability_votes(&self) -> &BitVec<BitOrderLsb0, u8> {
        &self.availability_votes
    }

    /// Get the relay-chain block number this was backed in.
    pub(crate) fn backed_in_number(&self) -> &N {
        &self.backed_in_number
    }

    /// Get the core index.
    pub(crate) fn core_occupied(&self) -> CoreIndex {
        self.core
    }
}

pub trait Config:
    frame_system::Config
    + paras::Config
    + dmp::Config
    + ump::Config
    + hrmp::Config
    + configuration::Config
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
    trait Store for Module<T: Config> as ParaInclusion {
        /// The latest bitfield for each validator, referred to by their index in the validator set.
        AvailabilityBitfields: map hasher(twox_64_concat) ValidatorIndex
            => Option<AvailabilityBitfieldRecord<T::BlockNumber>>;

        /// Candidates pending availability by `ParaId`.
        PendingAvailability: map hasher(twox_64_concat) ParaId
            => Option<CandidatePendingAvailability<T::Hash, T::BlockNumber>>;

        /// The commitments of candidates pending availability, by ParaId.
        PendingAvailabilityCommitments: map hasher(twox_64_concat) ParaId
            => Option<CandidateCommitments>;

        /// The current validators, by their parachain session keys.
        Validators get(fn validators) config(validators): Vec<ValidatorId>;

        /// The current session index.
        CurrentSessionIndex get(fn session_index): SessionIndex;
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Availability bitfield has unexpected size.
        WrongBitfieldSize,
        /// Multiple bitfields submitted by same validator or validators out of order by index.
        BitfieldDuplicateOrUnordered,
        /// Validator index out of bounds.
        ValidatorIndexOutOfBounds,
        /// Invalid signature
        InvalidBitfieldSignature,
        /// Candidate submitted but para not scheduled.
        UnscheduledCandidate,
        /// Candidate scheduled despite pending candidate already existing for the para.
        CandidateScheduledBeforeParaFree,
        /// Candidate included with the wrong collator.
        WrongCollator,
        /// Scheduled cores out of order.
        ScheduledOutOfOrder,
        /// Head data exceeds the configured maximum.
        HeadDataTooLarge,
        /// Code upgrade prematurely.
        PrematureCodeUpgrade,
        /// Output code is too large
        NewCodeTooLarge,
        /// Candidate not in parent context.
        CandidateNotInParentContext,
        /// The bitfield contains a bit relating to an unassigned availability core.
        UnoccupiedBitInBitfield,
        /// Invalid group index in core assignment.
        InvalidGroupIndex,
        /// Insufficient (non-majority) backing.
        InsufficientBacking,
        /// Invalid (bad signature, unknown validator, etc.) backing.
        InvalidBacking,
        /// Collator did not sign PoV.
        NotCollatorSigned,
        /// The validation data hash does not match expected.
        ValidationDataHashMismatch,
        /// Internal error only returned when compiled with debug assertions.
        InternalError,
        /// The downward message queue is not processed correctly.
        IncorrectDownwardMessageHandling,
        /// At least one upward message sent does not pass the acceptance criteria.
        InvalidUpwardMessages,
        /// The candidate didn't follow the rules of HRMP watermark advancement.
        HrmpWatermarkMishandling,
        /// The HRMP messages sent by the candidate is not valid.
        InvalidOutboundHrmp,
    }
}

decl_event! {
    pub enum Event<T> where <T as frame_system::Config>::Hash {
        /// A candidate was backed. [candidate, head_data]
        CandidateBacked(CandidateReceipt<Hash>, HeadData),
        /// A candidate was included. [candidate, head_data]
        CandidateIncluded(CandidateReceipt<Hash>, HeadData),
        /// A candidate timed out. [candidate, head_data]
        CandidateTimedOut(CandidateReceipt<Hash>, HeadData),
    }
}

decl_module! {
    /// The parachain-candidate inclusion module.
    pub struct Module<T: Config>
        for enum Call where origin: <T as frame_system::Config>::Origin
    {
        type Error = Error<T>;

        fn deposit_event() = default;
    }
}

const LOG_TARGET: &str = "parachains_runtime_inclusion";

impl<T: Config> Module<T> {
    /// Block initialization logic, called by initializer.
    pub(crate) fn initializer_initialize(_now: T::BlockNumber) -> Weight {
        0
    }

    /// Block finalization logic, called by initializer.
    pub(crate) fn initializer_finalize() {}

    /// Handle an incoming session change.
    pub(crate) fn initializer_on_new_session(
        notification: &crate::initializer::SessionChangeNotification<T::BlockNumber>,
    ) {
        // unlike most drain methods, drained elements are not cleared on `Drop` of the iterator
        // and require consumption.
        for _ in <PendingAvailabilityCommitments>::drain() {}
        for _ in <PendingAvailability<T>>::drain() {}
        for _ in <AvailabilityBitfields<T>>::drain() {}

        Validators::set(notification.validators.clone()); // substrate forces us to clone, stupidly.
        CurrentSessionIndex::set(notification.session_index);
    }

    /// Process a set of incoming bitfields. Return a vec of cores freed by candidates
    /// becoming available.
    pub(crate) fn process_bitfields(
        signed_bitfields: SignedAvailabilityBitfields,
        core_lookup: impl Fn(CoreIndex) -> Option<ParaId>,
    ) -> Result<Vec<CoreIndex>, DispatchError> {
        let validators = Validators::get();
        let session_index = CurrentSessionIndex::get();
        let config = <configuration::Module<T>>::config();
        let parachains = <paras::Module<T>>::parachains();

        let n_bits = parachains.len() + config.parathread_cores as usize;

        let mut assigned_paras_record: Vec<_> = (0..n_bits)
            .map(|bit_index| core_lookup(CoreIndex::from(bit_index as u32)))
            .map(|core_para| core_para.map(|p| (p, PendingAvailability::<T>::get(&p))))
            .collect();

        // do sanity checks on the bitfields:
        // 1. no more than one bitfield per validator
        // 2. bitfields are ascending by validator index.
        // 3. each bitfield has exactly `n_bits`
        // 4. signature is valid.
        {
            let occupied_bitmask: BitVec<BitOrderLsb0, u8> = assigned_paras_record
                .iter()
                .map(|p| {
                    p.as_ref().map_or(false, |(_id, pending_availability)| {
                        pending_availability.is_some()
                    })
                })
                .collect();

            let mut last_index = None;

            let signing_context = SigningContext {
                parent_hash: <frame_system::Module<T>>::parent_hash(),
                session_index,
            };

            for signed_bitfield in &signed_bitfields {
                ensure!(
                    signed_bitfield.payload().0.len() == n_bits,
                    Error::<T>::WrongBitfieldSize,
                );

                ensure!(
                    last_index.map_or(true, |last| last < signed_bitfield.validator_index()),
                    Error::<T>::BitfieldDuplicateOrUnordered,
                );

                ensure!(
                    signed_bitfield.validator_index() < validators.len() as ValidatorIndex,
                    Error::<T>::ValidatorIndexOutOfBounds,
                );

                ensure!(
                    occupied_bitmask.clone() & signed_bitfield.payload().0.clone()
                        == signed_bitfield.payload().0,
                    Error::<T>::UnoccupiedBitInBitfield,
                );

                let validator_public = &validators[signed_bitfield.validator_index() as usize];

                signed_bitfield
                    .check_signature(&signing_context, validator_public)
                    .map_err(|_| Error::<T>::InvalidBitfieldSignature)?;

                last_index = Some(signed_bitfield.validator_index());
            }
        }

        let now = <frame_system::Module<T>>::block_number();
        for signed_bitfield in signed_bitfields {
            for (bit_idx, _) in signed_bitfield
                .payload()
                .0
                .iter()
                .enumerate()
                .filter(|(_, is_av)| **is_av)
            {
                let (_, pending_availability) = assigned_paras_record[bit_idx]
					.as_mut()
					.expect("validator bitfields checked not to contain bits corresponding to unoccupied cores; qed");

                // defensive check - this is constructed by loading the availability bitfield record,
                // which is always `Some` if the core is occupied - that's why we're here.
                let val_idx = signed_bitfield.validator_index() as usize;
                if let Some(mut bit) = pending_availability
                    .as_mut()
                    .and_then(|r| r.availability_votes.get_mut(val_idx))
                {
                    *bit = true;
                } else if cfg!(debug_assertions) {
                    ensure!(false, Error::<T>::InternalError);
                }
            }

            let validator_index = signed_bitfield.validator_index();
            let record = AvailabilityBitfieldRecord {
                bitfield: signed_bitfield.into_payload(),
                submitted_at: now,
            };

            <AvailabilityBitfields<T>>::insert(&validator_index, record);
        }

        let threshold = availability_threshold(validators.len());

        let mut freed_cores = Vec::with_capacity(n_bits);
        for (para_id, pending_availability) in assigned_paras_record
            .into_iter()
            .filter_map(|x| x)
            .filter_map(|(id, p)| p.map(|p| (id, p)))
        {
            if pending_availability.availability_votes.count_ones() >= threshold {
                <PendingAvailability<T>>::remove(&para_id);
                let commitments = match PendingAvailabilityCommitments::take(&para_id) {
                    Some(commitments) => commitments,
                    None => {
                        debug::warn!(
                            r#"
						Inclusion::process_bitfields:
							PendingAvailability and PendingAvailabilityCommitments
							are out of sync, did someone mess with the storage?
						"#
                        );
                        continue;
                    }
                };

                let receipt = CommittedCandidateReceipt {
                    descriptor: pending_availability.descriptor,
                    commitments,
                };
                Self::enact_candidate(pending_availability.relay_parent_number, receipt);

                freed_cores.push(pending_availability.core);
            } else {
                <PendingAvailability<T>>::insert(&para_id, &pending_availability);
            }
        }
        // TODO: pass available candidates onwards to validity module once implemented.
        Ok(freed_cores)
    }

    /// Process candidates that have been backed. Provide a set of candidates and scheduled cores.
    ///
    /// Both should be sorted ascending by core index, and the candidates should be a subset of
    /// scheduled cores. If these conditions are not met, the execution of the function fails.
    pub(crate) fn process_candidates(
        candidates: Vec<BackedCandidate<T::Hash>>,
        scheduled: Vec<CoreAssignment>,
        group_validators: impl Fn(GroupIndex) -> Option<Vec<ValidatorIndex>>,
    ) -> Result<Vec<CoreIndex>, DispatchError> {
        ensure!(
            candidates.len() <= scheduled.len(),
            Error::<T>::UnscheduledCandidate
        );

        if scheduled.is_empty() {
            return Ok(Vec::new());
        }

        let validators = Validators::get();
        let parent_hash = <frame_system::Module<T>>::parent_hash();
        let check_cx = CandidateCheckContext::<T>::new();

        // do all checks before writing storage.
        let core_indices = {
            let mut skip = 0;
            let mut core_indices = Vec::with_capacity(candidates.len());
            let mut last_core = None;

            let mut check_assignment_in_order = |assignment: &CoreAssignment| -> DispatchResult {
                ensure!(
                    last_core.map_or(true, |core| assignment.core > core),
                    Error::<T>::ScheduledOutOfOrder,
                );

                last_core = Some(assignment.core);
                Ok(())
            };

            let signing_context = SigningContext {
                parent_hash,
                session_index: CurrentSessionIndex::get(),
            };

            // We combine an outer loop over candidates with an inner loop over the scheduled,
            // where each iteration of the outer loop picks up at the position
            // in scheduled just after the past iteration left off.
            //
            // If the candidates appear in the same order as they appear in `scheduled`,
            // then they should always be found. If the end of `scheduled` is reached,
            // then the candidate was either not scheduled or out-of-order.
            //
            // In the meantime, we do certain sanity checks on the candidates and on the scheduled
            // list.
            'a: for (candidate_idx, candidate) in candidates.iter().enumerate() {
                let para_id = candidate.descriptor().para_id;

                // we require that the candidate is in the context of the parent block.
                ensure!(
                    candidate.descriptor().relay_parent == parent_hash,
                    Error::<T>::CandidateNotInParentContext,
                );
                ensure!(
                    candidate.descriptor().check_collator_signature().is_ok(),
                    Error::<T>::NotCollatorSigned,
                );

                if let Err(err) = check_cx.check_validation_outputs(
                    para_id,
                    &candidate.candidate.commitments.head_data,
                    &candidate.candidate.commitments.new_validation_code,
                    candidate.candidate.commitments.processed_downward_messages,
                    &candidate.candidate.commitments.upward_messages,
                    T::BlockNumber::from(candidate.candidate.commitments.hrmp_watermark),
                    &candidate.candidate.commitments.horizontal_messages,
                ) {
                    frame_support::debug::RuntimeLogger::init();
                    log::debug!(
						target: LOG_TARGET,
						"Validation outputs checking during inclusion of a candidate {} for parachain `{}` failed: {:?}",
						candidate_idx,
						u32::from(para_id),
						err,
					);
                    return Err(err.strip_into_dispatch_err::<T>().into());
                };

                for (i, assignment) in scheduled[skip..].iter().enumerate() {
                    check_assignment_in_order(assignment)?;

                    if para_id == assignment.para_id {
                        if let Some(required_collator) = assignment.required_collator() {
                            ensure!(
                                required_collator == &candidate.descriptor().collator,
                                Error::<T>::WrongCollator,
                            );
                        }

                        {
                            // this should never fail because the para is registered
                            let persisted_validation_data =
                                match crate::util::make_persisted_validation_data::<T>(para_id) {
                                    Some(l) => l,
                                    None => {
                                        // We don't want to error out here because it will
                                        // brick the relay-chain. So we return early without
                                        // doing anything.
                                        return Ok(Vec::new());
                                    }
                                };

                            let expected = persisted_validation_data.hash();

                            ensure!(
                                expected == candidate.descriptor().persisted_validation_data_hash,
                                Error::<T>::ValidationDataHashMismatch,
                            );
                        }

                        ensure!(
                            <PendingAvailability<T>>::get(&para_id).is_none()
                                && <PendingAvailabilityCommitments>::get(&para_id).is_none(),
                            Error::<T>::CandidateScheduledBeforeParaFree,
                        );

                        // account for already skipped, and then skip this one.
                        skip = i + skip + 1;

                        let group_vals = group_validators(assignment.group_idx)
                            .ok_or(Error::<T>::InvalidGroupIndex)?;

                        // check the signatures in the backing and that it is a majority.
                        {
                            let maybe_amount_validated = primitives::v1::check_candidate_backing(
                                &candidate,
                                &signing_context,
                                group_vals.len(),
                                |idx| {
                                    group_vals
                                        .get(idx)
                                        .and_then(|i| validators.get(*i as usize))
                                        .cloned()
                                },
                            );

                            match maybe_amount_validated {
                                Ok(amount_validated) => ensure!(
                                    amount_validated * 2 > group_vals.len(),
                                    Error::<T>::InsufficientBacking,
                                ),
                                Err(()) => {
                                    return Err(Error::<T>::InvalidBacking.into());
                                }
                            }
                        }

                        core_indices.push(assignment.core);
                        continue 'a;
                    }
                }

                // end of loop reached means that the candidate didn't appear in the non-traversed
                // section of the `scheduled` slice. either it was not scheduled or didn't appear in
                // `candidates` in the correct order.
                ensure!(false, Error::<T>::UnscheduledCandidate,);
            }

            // check remainder of scheduled cores, if any.
            for assignment in scheduled[skip..].iter() {
                check_assignment_in_order(assignment)?;
            }

            core_indices
        };

        // one more sweep for actually writing to storage.
        for (candidate, core) in candidates.into_iter().zip(core_indices.iter().cloned()) {
            let para_id = candidate.descriptor().para_id;

            // initialize all availability votes to 0.
            let availability_votes: BitVec<BitOrderLsb0, u8> =
                bitvec::bitvec![BitOrderLsb0, u8; 0; validators.len()];

            Self::deposit_event(Event::<T>::CandidateBacked(
                candidate.candidate.to_plain(),
                candidate.candidate.commitments.head_data.clone(),
            ));

            let (descriptor, commitments) = (
                candidate.candidate.descriptor,
                candidate.candidate.commitments,
            );

            <PendingAvailability<T>>::insert(
                &para_id,
                CandidatePendingAvailability {
                    core,
                    descriptor,
                    availability_votes,
                    relay_parent_number: check_cx.relay_parent_number,
                    backed_in_number: check_cx.now,
                },
            );
            <PendingAvailabilityCommitments>::insert(&para_id, commitments);
        }

        Ok(core_indices)
    }

    /// Run the acceptance criteria checks on the given candidate commitments.
    pub(crate) fn check_validation_outputs(
        para_id: ParaId,
        validation_outputs: primitives::v1::CandidateCommitments,
    ) -> bool {
        if let Err(err) = CandidateCheckContext::<T>::new().check_validation_outputs(
            para_id,
            &validation_outputs.head_data,
            &validation_outputs.new_validation_code,
            validation_outputs.processed_downward_messages,
            &validation_outputs.upward_messages,
            T::BlockNumber::from(validation_outputs.hrmp_watermark),
            &validation_outputs.horizontal_messages,
        ) {
            frame_support::debug::RuntimeLogger::init();
            log::debug!(
                target: LOG_TARGET,
                "Validation outputs checking for parachain `{}` failed: {:?}",
                u32::from(para_id),
                err,
            );
            false
        } else {
            true
        }
    }

    fn enact_candidate(
        relay_parent_number: T::BlockNumber,
        receipt: CommittedCandidateReceipt<T::Hash>,
    ) -> Weight {
        let plain = receipt.to_plain();
        let commitments = receipt.commitments;
        let config = <configuration::Module<T>>::config();

        // initial weight is config read.
        let mut weight = T::DbWeight::get().reads_writes(1, 0);
        if let Some(new_code) = commitments.new_validation_code {
            weight += <paras::Module<T>>::schedule_code_upgrade(
                receipt.descriptor.para_id,
                new_code,
                relay_parent_number + config.validation_upgrade_delay,
            );
        }

        // enact the messaging facet of the candidate.
        weight += <dmp::Module<T>>::prune_dmq(
            receipt.descriptor.para_id,
            commitments.processed_downward_messages,
        );
        weight += <ump::Module<T>>::enact_upward_messages(
            receipt.descriptor.para_id,
            commitments.upward_messages,
        );
        weight += <hrmp::Module<T>>::prune_hrmp(
            receipt.descriptor.para_id,
            T::BlockNumber::from(commitments.hrmp_watermark),
        );
        weight += <hrmp::Module<T>>::queue_outbound_hrmp(
            receipt.descriptor.para_id,
            commitments.horizontal_messages,
        );

        Self::deposit_event(Event::<T>::CandidateIncluded(
            plain,
            commitments.head_data.clone(),
        ));

        weight
            + <paras::Module<T>>::note_new_head(
                receipt.descriptor.para_id,
                commitments.head_data,
                relay_parent_number,
            )
    }

    /// Cleans up all paras pending availability that the predicate returns true for.
    ///
    /// The predicate accepts the index of the core and the block number the core has been occupied
    /// since (i.e. the block number the candidate was backed at in this fork of the relay chain).
    ///
    /// Returns a vector of cleaned-up core IDs.
    pub(crate) fn collect_pending(
        pred: impl Fn(CoreIndex, T::BlockNumber) -> bool,
    ) -> Vec<CoreIndex> {
        let mut cleaned_up_ids = Vec::new();
        let mut cleaned_up_cores = Vec::new();

        for (para_id, pending_record) in <PendingAvailability<T>>::iter() {
            if pred(pending_record.core, pending_record.backed_in_number) {
                cleaned_up_ids.push(para_id);
                cleaned_up_cores.push(pending_record.core);
            }
        }

        for para_id in cleaned_up_ids {
            let pending = <PendingAvailability<T>>::take(&para_id);
            let commitments = <PendingAvailabilityCommitments>::take(&para_id);

            if let (Some(pending), Some(commitments)) = (pending, commitments) {
                // defensive: this should always be true.
                let candidate = CandidateReceipt {
                    descriptor: pending.descriptor,
                    commitments_hash: commitments.hash(),
                };

                Self::deposit_event(Event::<T>::CandidateTimedOut(
                    candidate,
                    commitments.head_data,
                ));
            }
        }

        cleaned_up_cores
    }

    /// Forcibly enact the candidate with the given ID as though it had been deemed available
    /// by bitfields.
    ///
    /// Is a no-op if there is no candidate pending availability for this para-id.
    /// This should generally not be used but it is useful during execution of Runtime APIs,
    /// where the changes to the state are expected to be discarded directly after.
    pub(crate) fn force_enact(para: ParaId) {
        let pending = <PendingAvailability<T>>::take(&para);
        let commitments = <PendingAvailabilityCommitments>::take(&para);

        if let (Some(pending), Some(commitments)) = (pending, commitments) {
            let candidate = CommittedCandidateReceipt {
                descriptor: pending.descriptor,
                commitments,
            };

            Self::enact_candidate(pending.relay_parent_number, candidate);
        }
    }

    /// Returns the CommittedCandidateReceipt pending availability for the para provided, if any.
    pub(crate) fn candidate_pending_availability(
        para: ParaId,
    ) -> Option<CommittedCandidateReceipt<T::Hash>> {
        <PendingAvailability<T>>::get(&para)
            .map(|p| p.descriptor)
            .and_then(|d| <PendingAvailabilityCommitments>::get(&para).map(move |c| (d, c)))
            .map(|(d, c)| CommittedCandidateReceipt {
                descriptor: d,
                commitments: c,
            })
    }

    /// Returns the metadata around the candidate pending availability for the
    /// para provided, if any.
    pub(crate) fn pending_availability(
        para: ParaId,
    ) -> Option<CandidatePendingAvailability<T::Hash, T::BlockNumber>> {
        <PendingAvailability<T>>::get(&para)
    }
}

const fn availability_threshold(n_validators: usize) -> usize {
    let mut threshold = (n_validators * 2) / 3;
    threshold += (n_validators * 2) % 3;
    threshold
}

#[derive(derive_more::From, Debug)]
enum AcceptanceCheckErr<BlockNumber> {
    HeadDataTooLarge,
    PrematureCodeUpgrade,
    NewCodeTooLarge,
    ProcessedDownwardMessages(dmp::ProcessedDownwardMessagesAcceptanceErr),
    UpwardMessages(ump::AcceptanceCheckErr),
    HrmpWatermark(hrmp::HrmpWatermarkAcceptanceErr<BlockNumber>),
    OutboundHrmp(hrmp::OutboundHrmpAcceptanceErr),
}

impl<BlockNumber> AcceptanceCheckErr<BlockNumber> {
    /// Returns the same error so that it can be threaded through a needle of `DispatchError` and
    /// ultimately returned from a `Dispatchable`.
    fn strip_into_dispatch_err<T: Config>(self) -> Error<T> {
        use AcceptanceCheckErr::*;
        match self {
            HeadDataTooLarge => Error::<T>::HeadDataTooLarge,
            PrematureCodeUpgrade => Error::<T>::PrematureCodeUpgrade,
            NewCodeTooLarge => Error::<T>::NewCodeTooLarge,
            ProcessedDownwardMessages(_) => Error::<T>::IncorrectDownwardMessageHandling,
            UpwardMessages(_) => Error::<T>::InvalidUpwardMessages,
            HrmpWatermark(_) => Error::<T>::HrmpWatermarkMishandling,
            OutboundHrmp(_) => Error::<T>::InvalidOutboundHrmp,
        }
    }
}

/// A collection of data required for checking a candidate.
struct CandidateCheckContext<T: Config> {
    config: configuration::HostConfiguration<T::BlockNumber>,
    now: T::BlockNumber,
    relay_parent_number: T::BlockNumber,
}

impl<T: Config> CandidateCheckContext<T> {
    fn new() -> Self {
        let now = <frame_system::Module<T>>::block_number();
        Self {
            config: <configuration::Module<T>>::config(),
            now,
            relay_parent_number: now - One::one(),
        }
    }

    /// Check the given outputs after candidate validation on whether it passes the acceptance
    /// criteria.
    fn check_validation_outputs(
        &self,
        para_id: ParaId,
        head_data: &HeadData,
        new_validation_code: &Option<primitives::v1::ValidationCode>,
        processed_downward_messages: u32,
        upward_messages: &[primitives::v1::UpwardMessage],
        hrmp_watermark: T::BlockNumber,
        horizontal_messages: &[primitives::v1::OutboundHrmpMessage<ParaId>],
    ) -> Result<(), AcceptanceCheckErr<T::BlockNumber>> {
        ensure!(
            head_data.0.len() <= self.config.max_head_data_size as _,
            AcceptanceCheckErr::HeadDataTooLarge,
        );

        // if any, the code upgrade attempt is allowed.
        if let Some(new_validation_code) = new_validation_code {
            let valid_upgrade_attempt = <paras::Module<T>>::last_code_upgrade(para_id, true)
                .map_or(true, |last| {
                    last <= self.relay_parent_number
                        && self.relay_parent_number.saturating_sub(last)
                            >= self.config.validation_upgrade_frequency
                });
            ensure!(
                valid_upgrade_attempt,
                AcceptanceCheckErr::PrematureCodeUpgrade,
            );
            ensure!(
                new_validation_code.0.len() <= self.config.max_code_size as _,
                AcceptanceCheckErr::NewCodeTooLarge,
            );
        }

        // check if the candidate passes the messaging acceptance criteria
        <dmp::Module<T>>::check_processed_downward_messages(para_id, processed_downward_messages)?;
        <ump::Module<T>>::check_upward_messages(&self.config, para_id, upward_messages)?;
        <hrmp::Module<T>>::check_hrmp_watermark(para_id, self.relay_parent_number, hrmp_watermark)?;
        <hrmp::Module<T>>::check_outbound_hrmp(&self.config, para_id, horizontal_messages)?;

        Ok(())
    }
}
