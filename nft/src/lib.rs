use std::collections::HashMap;
use near_contract_standards::non_fungible_token::{
    metadata::{NFTContractMetadata, TokenMetadata},
    NonFungibleToken, Token, TokenId
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, AccountId, PanicOnDefault, Promise, PromiseOrValue,
    BorshStorageKey, Gas, Balance
};
use near_units::{parse_near, parse_gas};
pub use crate::util::*;
pub const GAS_FOR_NFT_TRANSFER_CALL: Gas = Gas(parse_gas!("25 Tgas") as u64);

mod util;
mod royalty;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub(crate) tokens: NonFungibleToken,
    pub metadata: LazyOption<NFTContractMetadata>,
    pub royalties: UnorderedMap<AccountId, u32>,
    index: u64,
    mint_price: u128
}

#[derive(BorshSerialize, BorshStorageKey)]
pub enum StorageKey {
    NFTContractMetadata,
    NonFungibleToken,
    Enumeration,
    Approval,
    TokenMetadata,
    Royalties
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        metadata: NFTContractMetadata
    ) -> Self {
        let this = Self {
            tokens: NonFungibleToken::new(
                StorageKey::NonFungibleToken,
                env::current_account_id(),
                Some(StorageKey::TokenMetadata),
                Some(StorageKey::Enumeration),
                Some(StorageKey::Approval),
            ),
            metadata: LazyOption::new(
                StorageKey::NFTContractMetadata.try_to_vec().unwrap(),
                Some(&metadata),
            ),
            mint_price: parse_near!("1 N"),
            royalties: UnorderedMap::new(StorageKey::Royalties.try_to_vec().unwrap()),
            index: 1
        };
        this
    }

    fn token_storage_cost(&self) -> u128 {
        env::storage_byte_cost() * self.tokens.extra_storage_in_bytes_per_token as u128
    }

    pub fn get_token_storage_cost(&self) -> U128 {
        self.token_storage_cost().into()
    }

    fn create_metadata(&mut self, title: String, media: String, reference: String) -> TokenMetadata {
        TokenMetadata {
            title: Some(title), // ex. "Arch Nemesis: Mail Carrier" or "Parcel #5055"
            media: Some(media), // URL to associated media, preferably to decentralized, content-addressed storage
            issued_at: Some(env::block_timestamp().to_string()), // ISO 8601 datetime when token was issued or minted
            reference: Some(reference),            // URL to an off-chain JSON file with more info.
            description: None,    // free-form description
            media_hash: None, // Base64-encoded sha256 hash of content referenced by the `media` field. Required if `media` is included.
            copies: None, // number of copies of this set of metadata in existence when token was minted.
            expires_at: None, // ISO 8601 datetime when token expires
            starts_at: None, // ISO 8601 datetime when token starts being valid
            updated_at: None, // ISO 8601 datetime when token was last updated
            extra: None, // anything extra the NFT wants to store on-chain. Can be stringified JSON.
            reference_hash: None, // Base64-encoded sha256 hash of JSON from reference field. Required if `reference` is included.
        }
    }

    pub fn update_metadata(
        &mut self, icon: Option<String>, name: Option<String>, symbol: Option<String>,
        base_uri: Option<String>,
        mint_price: Option<u128>,
        royalties: Option<HashMap<AccountId, u32>>
    ) {
        if env::signer_account_id() != env::current_account_id() {
            env::panic_str("Forbin");
        }
        if icon.is_some() || name.is_some() || symbol.is_some() || base_uri.is_some() {
            let mut metadata = self.metadata.get().unwrap();
            if icon.is_some() {
                metadata.icon = icon;
            }
            if name.is_some() {
                metadata.name = name.unwrap();
            }
            if symbol.is_some() {
                metadata.symbol = symbol.unwrap();
            }
            if base_uri.is_some() {
                metadata.base_uri = base_uri;
            }
            self.metadata = LazyOption::new(
                StorageKey::NFTContractMetadata.try_to_vec().unwrap(),
                Some(&metadata),
            );
        }
        if mint_price.is_some() {
            let update_price = mint_price.unwrap() * parse_near!("0.00001 N");
            self.mint_price = update_price;
        }
        if royalties.is_some() {
            self.royalties.clear();
            let mut amount = 0;
            let royalty = royalties.unwrap();
            if royalty.len() > 7 {
                env::panic_str("Royalty wallets cannot exceed 6");
            }
            for (k, v) in royalty.iter() {
                let key = k.clone();
                amount += v;
                if key != env::current_account_id() {
                    self.royalties.insert(&key, v);
                }
                if amount > 2000 {
                    env::panic_str("Royalty can't be bigger than 20%");
                }
            }
        }
    }

    #[payable]
    pub fn nft_mint(
        &mut self, 
        receiver_id: AccountId,
        title: String,
        media: String,
        reference: String
    ) -> Token {
        if GAS_FOR_NFT_TRANSFER_CALL > env::prepaid_gas() {
            env::panic_str(&format!("Insufficient gas, prepaid gas: {}, safe gas: {}", u64::from(env::prepaid_gas()), u64::from(GAS_FOR_NFT_TRANSFER_CALL)));
        }
        let amount = env::attached_deposit();
        let predict_price = self.mint_price + self.token_storage_cost();
        if predict_price > amount {
            env::panic_str(&format!("Insufficient deposited amount, {} $yoctoNEAR needed", predict_price));
        }
        let initial_storage_usage = env::storage_usage();
        let token_id = self.index.to_string();
        self.index += 1;
        let token = self.internal_mint(token_id.clone(), receiver_id.clone(), title, media, reference);
        util::log_mint(&receiver_id, &[token.clone()]);
        let storage_used = env::storage_usage() - initial_storage_usage;
        let required_cost = env::storage_byte_cost() * Balance::from(storage_used);
        let deposit_used = self.mint_price + required_cost;
        if deposit_used > amount {
            util::transfer(&env::signer_account_id(), deposit_used - amount);
        }
        token
    }

    fn internal_mint(
        &mut self,
        token_id: String,
        token_owner_id: AccountId,
        title: String,
        media: String,
        reference: String
    ) -> Token {
        let token_metadata = Some(self.create_metadata(title, media, reference));
        self.tokens
            .internal_mint_with_refund(token_id, token_owner_id, token_metadata, None)
    }

    pub fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata.get().unwrap()
    }
}

near_contract_standards::impl_non_fungible_token_core!(Contract, tokens);
near_contract_standards::impl_non_fungible_token_approval!(Contract, tokens);
near_contract_standards::impl_non_fungible_token_enumeration!(Contract, tokens);