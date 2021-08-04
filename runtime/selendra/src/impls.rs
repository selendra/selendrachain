use frame_support::transactional;
use frame_support::traits::ReservableCurrency;
use evm_accounts::account::MergeAccount;
use sp_runtime::DispatchResult;
use primitives::v1::AccountId;
use crate::Balances;

pub struct MergeAccountEvm;
impl MergeAccount<AccountId> for MergeAccountEvm {
#[transactional]
fn merge_account(source: &AccountId, dest: &AccountId) -> DispatchResult {
     // unreserve all reserved currency
     <Balances as ReservableCurrency<_>>::unreserve(source, Balances::reserved_balance(source));

     // transfer all free to dest
     match Balances::transfer(Some(source.clone()).into(), dest.clone().into(), Balances::free_balance(source)) {
       Ok(_) => Ok(()),
       Err(e) => Err(e.error),
     }
  }
}