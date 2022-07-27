use near_sdk::{Promise, AccountId};
use near_contract_standards::non_fungible_token::{Token, events::NftMint};


pub fn transfer(account_id: &AccountId, amount: u128) -> Option<Promise> {
    if amount > 0 {
        return Some(Promise::new(account_id.clone()).transfer(amount));
    };
    None
}

pub fn log_mint(owner_id: &AccountId, tokens: &[Token]) {
    let token_ids = &tokens
        .iter()
        .map(|t| t.token_id.as_str())
        .collect::<Vec<&str>>();
    NftMint {
        owner_id,
        token_ids,
        memo: None,
    }
    .emit()
}

pub fn royalty_to_payout(a: u32, b: u128) -> u128 {
    a as u128 * b / 10_000u128
}