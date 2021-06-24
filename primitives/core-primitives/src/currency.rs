#![allow(clippy::from_over_into)]

use crate::evm::EvmAddress;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{
	convert::{Into, TryFrom, TryInto},
	prelude::*,
};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TokenSymbol {
	SEL = 0,
	SUSD = 1,
}

impl TryFrom<u8> for TokenSymbol {
	type Error = ();

	fn try_from(v: u8) -> Result<Self, Self::Error> {
		match v {
			0 => Ok(TokenSymbol::SEL),
			1 => Ok(TokenSymbol::SUSD),
			_ => Err(()),
		}
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Token(TokenSymbol),
	DEXShare(TokenSymbol, TokenSymbol),
	ERC20(EvmAddress),
}

impl CurrencyId {
	pub fn is_token_currency_id(&self) -> bool {
		matches!(self, CurrencyId::Token(_))
	}

	pub fn is_dex_share_currency_id(&self) -> bool {
		matches!(self, CurrencyId::DEXShare(_, _))
	}

	pub fn split_dex_share_currency_id(&self) -> Option<(Self, Self)> {
		match self {
			CurrencyId::DEXShare(token_symbol_0, token_symbol_1) => {
				Some((CurrencyId::Token(*token_symbol_0), CurrencyId::Token(*token_symbol_1)))
			}
			_ => None,
		}
	}

	pub fn join_dex_share_currency_id(currency_id_0: Self, currency_id_1: Self) -> Option<Self> {
		match (currency_id_0, currency_id_1) {
			(CurrencyId::Token(token_symbol_0), CurrencyId::Token(token_symbol_1)) => {
				Some(CurrencyId::DEXShare(token_symbol_0, token_symbol_1))
			}
			_ => None,
		}
	}
}

/// Note the pre-deployed ERC20 contracts depend on `CurrencyId` implementation,
/// and need to be updated if any change.
impl TryFrom<[u8; 32]> for CurrencyId {
	type Error = ();

	fn try_from(v: [u8; 32]) -> Result<Self, Self::Error> {
		if !v.starts_with(&[0u8; 29][..]) {
			return Err(());
		}

		// token
		if v[29] == 0 && v[31] == 0 {
			return v[30].try_into().map(CurrencyId::Token);
		}

		// DEX share
		if v[29] == 1 {
			let left = v[30].try_into()?;
			let right = v[31].try_into()?;
			return Ok(CurrencyId::DEXShare(left, right));
		}

		Err(())
	}
}

/// Note the pre-deployed ERC20 contracts depend on `CurrencyId` implementation,
/// and need to be updated if any change.
impl From<CurrencyId> for [u8; 32] {
	fn from(val: CurrencyId) -> Self {
		let mut bytes = [0u8; 32];
		match val {
			CurrencyId::Token(token) => {
				bytes[30] = token as u8;
			}
			CurrencyId::DEXShare(left, right) => {
				bytes[29] = 1;
				bytes[30] = left as u8;
				bytes[31] = right as u8;
			}
			_ => {}
		}
		bytes
	}
}


#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AuthoritysOriginId {
	Root,
}
