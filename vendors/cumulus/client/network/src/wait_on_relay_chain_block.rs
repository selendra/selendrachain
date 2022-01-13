// Copyright 2020-2021 Parity Technologies (UK) Ltd.
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

//! Provides the [`WaitOnRelayChainBlock`] type.

use futures::{future::ready, Future, FutureExt, StreamExt};
use selendra_primitives::v1::{Block as PBlock, Hash as PHash};
use sc_client_api::{
	blockchain::{self, BlockStatus, HeaderBackend},
	Backend, BlockchainEvents,
};
use sp_runtime::generic::BlockId;
use std::{sync::Arc, time::Duration};

/// The timeout in seconds after that the waiting for a block should be aborted.
const TIMEOUT_IN_SECONDS: u64 = 6;

/// Custom error type used by [`WaitOnRelayChainBlock`].
#[derive(Debug, derive_more::Display)]
pub enum Error {
	#[display(fmt = "Timeout while waiting for relay-chain block `{}` to be imported.", _0)]
	Timeout(PHash),
	#[display(
		fmt = "Import listener closed while waiting for relay-chain block `{}` to be imported.",
		_0
	)]
	ImportListenerClosed(PHash),
	#[display(
		fmt = "Blockchain returned an error while waiting for relay-chain block `{}` to be imported: {:?}",
		_0,
		_1
	)]
	BlockchainError(PHash, blockchain::Error),
}

/// A helper to wait for a given relay chain block in an async way.
///
/// The caller needs to pass the hash of a block it waits for and the function will return when the
/// block is available or an error occurred.
///
/// The waiting for the block is implemented as follows:
///
/// 1. Get a read lock on the import lock from the backend.
///
/// 2. Check if the block is already imported. If yes, return from the function.
///
/// 3. If the block isn't imported yet, add an import notification listener.
///
/// 4. Poll the import notification listener until the block is imported or the timeout is fired.
///
/// The timeout is set to 6 seconds. This should be enough time to import the block in the current
/// round and if not, the new round of the relay chain already started anyway.
pub struct WaitOnRelayChainBlock<B, BCE> {
	block_chain_events: Arc<BCE>,
	backend: Arc<B>,
}

impl<B, BCE> Clone for WaitOnRelayChainBlock<B, BCE> {
	fn clone(&self) -> Self {
		Self { backend: self.backend.clone(), block_chain_events: self.block_chain_events.clone() }
	}
}

impl<B, BCE> WaitOnRelayChainBlock<B, BCE> {
	/// Creates a new instance of `Self`.
	pub fn new(backend: Arc<B>, block_chain_events: Arc<BCE>) -> Self {
		Self { backend, block_chain_events }
	}
}

impl<B, BCE> WaitOnRelayChainBlock<B, BCE>
where
	B: Backend<PBlock>,
	BCE: BlockchainEvents<PBlock>,
{
	pub fn wait_on_relay_chain_block(
		&self,
		hash: PHash,
	) -> impl Future<Output = Result<(), Error>> {
		let _lock = self.backend.get_import_lock().read();
		match self.backend.blockchain().status(BlockId::Hash(hash)) {
			Ok(BlockStatus::InChain) => return ready(Ok(())).boxed(),
			Err(err) => return ready(Err(Error::BlockchainError(hash, err))).boxed(),
			_ => {},
		}

		let mut listener = self.block_chain_events.import_notification_stream();
		// Now it is safe to drop the lock, even when the block is now imported, it should show
		// up in our registered listener.
		drop(_lock);

		let mut timeout = futures_timer::Delay::new(Duration::from_secs(TIMEOUT_IN_SECONDS)).fuse();

		async move {
			loop {
				futures::select! {
					_ = timeout => return Err(Error::Timeout(hash)),
					evt = listener.next() => match evt {
						Some(evt) if evt.hash == hash => return Ok(()),
						// Not the event we waited on.
						Some(_) => continue,
						None => return Err(Error::ImportListenerClosed(hash)),
					}
				}
			}
		}
		.boxed()
	}
}
