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

//! Indracore-specific GRANDPA integration utilities.

use std::sync::Arc;

use sp_runtime::traits::{Block as BlockT, NumberFor};
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Header as _;

#[cfg(feature = "real-overseer")]
use {
	indracore_primitives::v1::{Block as IndracoreBlock, Header as IndracoreHeader},
	indracore_subsystem::messages::ApprovalVotingMessage,
	prometheus_endpoint::{self, Registry},
	indracore_overseer::OverseerHandler,
	futures::channel::oneshot,
};

/// A custom GRANDPA voting rule that acts as a diagnostic for the approval
/// voting subsystem's desired votes.
///
/// The practical effect of this voting rule is to implement a fixed delay of
/// blocks and to issue a prometheus metric on the lag behind the head that
/// approval checking would indicate.
#[cfg(feature = "real-overseer")]
#[derive(Clone)]
pub(crate) struct ApprovalCheckingVotingRule {
	checking_lag: Option<prometheus_endpoint::Gauge<prometheus_endpoint::U64>>,
	overseer: OverseerHandler,
}

#[cfg(feature = "real-overseer")]
impl ApprovalCheckingVotingRule {
	/// Create a new approval checking diagnostic voting rule.
	pub fn new(overseer: OverseerHandler, registry: Option<&Registry>)
		-> Result<Self, prometheus_endpoint::PrometheusError>
	{
		Ok(ApprovalCheckingVotingRule {
			checking_lag: if let Some(registry) = registry {
				Some(prometheus_endpoint::register(
					prometheus_endpoint::Gauge::with_opts(
						prometheus_endpoint::Opts::new(
							"parachain_approval_checking_finality_lag",
							"How far behind the head of the chain the Approval Checking protocol wants to vote",
						)
					)?,
					registry,
				)?)
			} else {
				None
			},
			overseer,
		})
	}
}

#[cfg(feature = "real-overseer")]
impl<B> grandpa::VotingRule<IndracoreBlock, B> for ApprovalCheckingVotingRule
	where B: sp_blockchain::HeaderBackend<IndracoreBlock>
{
	fn restrict_vote(
		&self,
		_backend: Arc<B>,
		base: &IndracoreHeader,
		best_target: &IndracoreHeader,
		current_target: &IndracoreHeader,
	) -> grandpa::VotingRuleResult<IndracoreBlock> {
		// Query approval checking and issue metrics.
		let mut overseer = self.overseer.clone();
		let checking_lag = self.checking_lag.clone();

		let best_hash = best_target.hash();
		let best_number = best_target.number.clone();

		let current_hash = current_target.hash();
		let current_number = current_target.number.clone();

		let base_number = base.number;

		Box::pin(async move {
			let (tx, rx) = oneshot::channel();
			let approval_checking_subsystem_vote = {
				overseer.send_msg(ApprovalVotingMessage::ApprovedAncestor(
					best_hash,
					base_number,
					tx,
				)).await;

				rx.await.ok().and_then(|v| v)
			};

			let approval_checking_subsystem_lag = approval_checking_subsystem_vote.map_or(
				best_number - base_number,
				|(_h, n)| best_number - n,
			);

			if let Some(ref checking_lag) = checking_lag {
				checking_lag.set(approval_checking_subsystem_lag as _);
			}

			tracing::trace!(
				target: "parachain::approval-voting",
				"GRANDPA: voting on {:?}. Approval-checking lag behind best is {}",
				approval_checking_subsystem_vote,
				approval_checking_subsystem_lag,
			);

			if approval_checking_subsystem_vote.map_or(false, |v| current_number < v.1) {
				Some((current_hash, current_number))
			} else {
				approval_checking_subsystem_vote
			}
		})
	}
}

/// A custom GRANDPA voting rule that "pauses" voting (i.e. keeps voting for the
/// same last finalized block) after a given block at height `N` has been
/// finalized and for a delay of `M` blocks, i.e. until the best block reaches
/// `N` + `M`, the voter will keep voting for block `N`.
#[derive(Clone)]
pub(crate) struct PauseAfterBlockFor<N>(pub(crate) N, pub(crate) N);

impl<Block, B> grandpa::VotingRule<Block, B> for PauseAfterBlockFor<NumberFor<Block>>
where
	Block: BlockT,
	B: sp_blockchain::HeaderBackend<Block>,
{
	fn restrict_vote(
		&self,
		backend: Arc<B>,
		base: &Block::Header,
		best_target: &Block::Header,
		current_target: &Block::Header,
	) -> grandpa::VotingRuleResult<Block> {
		let aux = || {
			// walk backwards until we find the target block
			let find_target = |target_number: NumberFor<Block>, current_header: &Block::Header| {
				let mut target_hash = current_header.hash();
				let mut target_header = current_header.clone();

				loop {
					if *target_header.number() < target_number {
						unreachable!(
							"we are traversing backwards from a known block; \
							 blocks are stored contiguously; \
							 qed"
						);
					}

					if *target_header.number() == target_number {
						return Some((target_hash, target_number));
					}

					target_hash = *target_header.parent_hash();
					target_header = backend.header(BlockId::Hash(target_hash)).ok()?.expect(
						"Header known to exist due to the existence of one of its descendents; qed",
					);
				}
			};

			// only restrict votes targeting a block higher than the block
			// we've set for the pause
			if *current_target.number() > self.0 {
				// if we're past the pause period (i.e. `self.0 + self.1`)
				// then we no longer need to restrict any votes
				if *best_target.number() > self.0 + self.1 {
					return None;
				}

				// if we've finalized the pause block, just keep returning it
				// until best number increases enough to pass the condition above
				if *base.number() >= self.0 {
					return Some((base.hash(), *base.number()));
				}

				// otherwise find the target header at the pause block
				// to vote on
				return find_target(self.0, current_target);
			}

			None
		};

		let target = aux();

		Box::pin(async move { target })
	}
}

