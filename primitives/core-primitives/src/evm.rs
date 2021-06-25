use crate::Balance;
use core::ops::Range;
use parity_scale_codec::{Decode, Encode};
use evm::ExitReason;
use sp_core::{H160, U256};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;
use max_encoded_len::MaxEncodedLen;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub use evm::backend::{Basic as Account, Log};
pub use evm::Config;

/// Evm Address.
pub type EvmAddress = sp_core::H160;

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
	/// Current transaction gas price.
	pub gas_price: U256,
	/// Origin of the transaction.
	pub origin: EvmAddress,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreateInfo {
	pub exit_reason: ExitReason,
	pub address: EvmAddress,
	pub output: Vec<u8>,
	pub used_gas: U256,
	pub used_storage: i32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CallInfo {
	pub exit_reason: ExitReason,
	pub output: Vec<u8>,
	pub used_gas: U256,
	pub used_storage: i32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Erc20Info {
	pub address: EvmAddress,
	pub name: Vec<u8>,
	pub symbol: Vec<u8>,
	pub decimals: u8,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EstimateResourcesRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u64>,
	/// Storage Limit
	pub storage_limit: Option<u32>,
	/// Value
	pub value: Option<Balance>,
	/// Data
	pub data: Option<Vec<u8>>,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord, MaxEncodedLen)]
pub enum ReserveIdentifier {
	CollatorSelection,
	EvmStorageDeposit,
	EvmDeveloperDeposit,
	TransactionPayment,
}

/// Ethereum precompiles
/// 0 - 0x400
/// Selendra precompiles
/// 0x400 - 0x800
pub const PRECOMPILE_ADDRESS_START: u64 = 0x400;
/// Predeployed system contracts (except Mirrored ERC20)
/// 0x800 - 0x1000
pub const PREDEPLOY_ADDRESS_START: u64 = 0x800;
/// Mirrored Tokens (ensure length <= 4 bytes, encode to u32 will take the first 4 non-zero bytes)
/// 0x1000000
pub const MIRRORED_TOKENS_ADDRESS_START: u64 = 0x1000000;
/// Mirrored NFT (ensure length <= 4 bytes, encode to u32 will take the first 4 non-zero bytes)
/// 0x2000000
pub const MIRRORED_NFT_ADDRESS_START: u64 = 0x2000000;
/// Mirrored LP Tokens
/// 0x10000000000000000
pub const MIRRORED_LP_TOKENS_ADDRESS_START: u128 = 0x10000000000000000;
/// System contract address prefix
pub const SYSTEM_CONTRACT_ADDRESS_PREFIX: [u8; 11] = [0u8; 11];

/// CurrencyId to H160([u8; 20]) bit encoding rule.
///
/// Token
/// v[16] = 1 // MIRRORED_TOKENS_ADDRESS_START
/// - v[19] = token(1 byte)
///
/// DexShare
/// v[11] = 1 // MIRRORED_LP_TOKENS_ADDRESS_START
/// - v[12..16] = dex left(4 bytes)
/// - v[16..20] = dex right(4 bytes)
///
/// Erc20
/// - v[0..20] = evm address(20 bytes)
pub const H160_TYPE_TOKEN: u8 = 1;
pub const H160_TYPE_DEXSHARE: u8 = 1;
pub const H160_POSITION_TOKEN: usize = 19;
pub const H160_POSITION_DEXSHARE_LEFT: Range<usize> = 12..16;
pub const H160_POSITION_DEXSHARE_RIGHT: Range<usize> = 16..20;
pub const H160_POSITION_ERC20: Range<usize> = 0..20;
pub const H160_PREFIX_TOKEN: [u8; 19] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0];
pub const H160_PREFIX_DEXSHARE: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
