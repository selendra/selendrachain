// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Selendra-specific RPCs implementation.

// #![warn(missing_docs)]

use std::{sync::Arc, collections::BTreeMap};
use jsonrpc_pubsub::manager::SubscriptionManager;

use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{HeaderBackend, HeaderMetadata, Error as BlockChainError};
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_keystore::SyncCryptoStorePtr;
use sc_client_api::{
	AuxStore, client::BlockchainEvents, light::{Fetcher, RemoteBlockchain},
};
use sc_consensus_babe::Epoch;
use sc_finality_grandpa::FinalityProofProvider;
use sc_sync_state_rpc::{SyncStateRpcApi, SyncStateRpcHandler};
use sc_network::NetworkService;
pub use sc_rpc::{DenyUnsafe, SubscriptionTaskExecutor};

use pallet_ethereum::EthereumStorageSchema;
use fc_rpc::{OverrideHandle, RuntimeApiStorageOverride,
	SchemaV1Override, StorageOverride
};
use fc_rpc_core::types::{FilterPool, PendingTransactions};

use txpool_api::TransactionPool;
use selendra_primitives::v0::{Block, BlockNumber, AccountId, Nonce, Balance, Hash};
use selendra_client::{RuntimeApiCollection, TransactionConverters};

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Light client extra dependencies.
pub struct LightDeps<C, F, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Remote access to the blockchain (async).
	pub remote_blockchain: Arc<dyn RemoteBlockchain<Block>>,
	/// Fetcher instance.
	pub fetcher: Arc<F>,
}

/// Extra dependencies for BABE.
pub struct BabeDeps {
	/// BABE protocol config.
	pub babe_config: sc_consensus_babe::Config,
	/// BABE pending epoch changes.
	pub shared_epoch_changes: sc_consensus_epochs::SharedEpochChanges<Block, Epoch>,
	/// The keystore that manages the keys of the node.
	pub keystore: SyncCryptoStorePtr,
}

/// Dependencies for GRANDPA
pub struct GrandpaDeps<B> {
	/// Voting round info.
	pub shared_voter_state: sc_finality_grandpa::SharedVoterState,
	/// Authority set info.
	pub shared_authority_set: sc_finality_grandpa::SharedAuthoritySet<Hash, BlockNumber>,
	/// Receives notifications about justification events from Grandpa.
	pub justification_stream: sc_finality_grandpa::GrandpaJustificationStream<Block>,
	/// Executor to drive the subscription manager in the Grandpa RPC handler.
	pub subscription_executor: sc_rpc::SubscriptionTaskExecutor,
	/// Finality proof provider.
	pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Dependencies for BEEFY
pub struct BeefyDeps<BeefySignature> {
	/// Receives notifications about signed commitment events from BEEFY.
	pub beefy_commitment_stream: beefy_gadget::notification::BeefySignedCommitmentStream<Block, BeefySignature>,
	/// Executor to drive the subscription manager in the BEEFY RPC handler.
	pub subscription_executor: sc_rpc::SubscriptionTaskExecutor,
}

/// Full client dependencies
pub struct FullDeps<C, P, SC, B, BS> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// The SelectChain Strategy
	pub select_chain: SC,
	/// A copy of the chain spec.
	pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// BABE specific dependencies.
	pub babe: BabeDeps,
	/// GRANDPA specific dependencies.
	pub grandpa: GrandpaDeps<B>,
	/// BEEFY specific dependencies.
	pub beefy: BeefyDeps<BS>,
	/// The Node authority flag
	pub is_authority: bool,
	/// Network service
	pub network: Arc<NetworkService<Block, Hash>>,
	/// Ethereum pending transactions.
	pub pending_transactions: PendingTransactions,
	/// EthFilterApi pool.
	pub filter_pool: Option<FilterPool>,
	/// Backend.
	pub frontier_backend: Arc<fc_db::Backend<Block>>,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Ethereum transaction to Extrinsic converter.
	pub transaction_converter: TransactionConverters,
}

/// Instantiate all RPC extensions.
pub fn create_full<C, P, SC, B, BS>(
	deps: FullDeps<C, P, SC, B, BS>,
	subscription_task_executor: SubscriptionTaskExecutor,
) -> RpcExtension where
	C: ProvideRuntimeApi<Block> 
		+ sc_client_api::backend::StorageProvider<Block, B>
		+ HeaderBackend<Block> 
		+ AuxStore 
		+ HeaderMetadata<Block, Error=BlockChainError> 
		+ Send 
		+ Sync 
		+ 'static,
	C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_mmr_rpc::MmrRuntimeApi<Block, <Block as sp_runtime::traits::Block>::Hash>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: BabeApi<Block>,
	C::Api: BlockBuilder<Block>,
	C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
	P: TransactionPool<Block = Block> + Sync + Send  + 'static,
	SC: SelectChain<Block> + 'static,
	B: sc_client_api::Backend<Block> + Send + Sync + 'static,
	B::State: sc_client_api::StateBackend<sp_runtime::traits::HashFor<Block>>,
	BS: Clone + Send + parity_scale_codec::Encode + 'static,
	C: BlockchainEvents<Block>,
	C::Api: RuntimeApiCollection<StateBackend = B::State>,
{
	use frame_rpc_system::{FullSystem, SystemApi};
	use pallet_mmr_rpc::{MmrApi, Mmr};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use sc_consensus_babe_rpc::BabeRpcHandler;
	use sc_finality_grandpa_rpc::{GrandpaApi, GrandpaRpcHandler};
	use fc_rpc::{
		EthApi, EthApiServer, EthFilterApi, EthFilterApiServer, EthPubSubApi, EthPubSubApiServer,
		HexEncodedIdProvider, NetApi, NetApiServer, Web3Api, Web3ApiServer,
	};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		select_chain,
		chain_spec,
		deny_unsafe,
		babe,
		grandpa,
		beefy,
		is_authority,
		network,
		pending_transactions,
		filter_pool,
		frontier_backend,
		max_past_logs,
		transaction_converter,
	} = deps;
	let BabeDeps {
		keystore,
		babe_config,
		shared_epoch_changes,
	} = babe;
	let GrandpaDeps {
		shared_voter_state,
		shared_authority_set,
		justification_stream,
		subscription_executor,
		finality_provider,
	} = grandpa;

	io.extend_with(
		SystemApi::to_delegate(FullSystem::new(client.clone(), pool.clone(), deny_unsafe))
	);
	io.extend_with(
		TransactionPaymentApi::to_delegate(TransactionPayment::new(client.clone()))
	);

	let signers = Vec::new();
	let mut overrides_map = BTreeMap::new();
	overrides_map.insert(
		EthereumStorageSchema::V1,
		Box::new(SchemaV1Override::new(client.clone())) 
			as Box<dyn StorageOverride<_> + Send + Sync>,
	);
	let overrides = Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::new(client.clone())),
	});
	io.extend_with(EthApiServer::to_delegate(EthApi::new(
		client.clone(),
		pool.clone(),
		transaction_converter,
		network.clone(),
		pending_transactions,
		signers,
		overrides.clone(),
		frontier_backend,
		is_authority,
		max_past_logs,
	)));

	if let Some(filter_pool) = filter_pool {
		io.extend_with(EthFilterApiServer::to_delegate(EthFilterApi::new(
			client.clone(),
			filter_pool,
			500_usize, // max stored filters
			overrides.clone(),
			max_past_logs,
		)));
	}

	io.extend_with(NetApiServer::to_delegate(NetApi::new(
		client.clone(),
		network.clone(),
		true,
	)));

	io.extend_with(Web3ApiServer::to_delegate(Web3Api::new(client.clone())));
	io.extend_with(EthPubSubApiServer::to_delegate(EthPubSubApi::new(
		pool,
		client.clone(),
		network,
		SubscriptionManager::<HexEncodedIdProvider>::with_id_provider(
			HexEncodedIdProvider::default(),
			Arc::new(subscription_task_executor),
		),
		overrides,
	)));

	io.extend_with(
		MmrApi::to_delegate(Mmr::new(client.clone()))
	);
	io.extend_with(
		sc_consensus_babe_rpc::BabeApi::to_delegate(
			BabeRpcHandler::new(
				client.clone(),
				shared_epoch_changes.clone(),
				keystore,
				babe_config,
				select_chain,
				deny_unsafe,
			)
		)
	);
	io.extend_with(
		GrandpaApi::to_delegate(GrandpaRpcHandler::new(
			shared_authority_set.clone(),
			shared_voter_state,
			justification_stream,
			subscription_executor,
			finality_provider,
		))
	);
	io.extend_with(
		SyncStateRpcApi::to_delegate(SyncStateRpcHandler::new(
			chain_spec,
			client,
			shared_authority_set,
			shared_epoch_changes,
			deny_unsafe,
		))
	);

	io.extend_with(beefy_gadget_rpc::BeefyApi::to_delegate(
		beefy_gadget_rpc::BeefyRpcHandler::new(
			beefy.beefy_commitment_stream,
			beefy.subscription_executor,
		),
	));

	io
}

/// Instantiate all RPC extensions for light node.
pub fn create_light<C, P, F>(deps: LightDeps<C, F, P>) -> RpcExtension
	where
		C: ProvideRuntimeApi<Block>,
		C: HeaderBackend<Block>,
		C: Send + Sync + 'static,
		C::Api: frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
		C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
		P: TransactionPool + Sync + Send + 'static,
		F: Fetcher<Block> + 'static,
{
	use frame_rpc_system::{LightSystem, SystemApi};

	let LightDeps {
		client,
		pool,
		remote_blockchain,
		fetcher,
	} = deps;
	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(
		SystemApi::<Hash, AccountId, Nonce>::to_delegate(LightSystem::new(client, remote_blockchain, fetcher, pool))
	);
	io
}
