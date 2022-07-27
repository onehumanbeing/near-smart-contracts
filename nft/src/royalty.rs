use crate::*;
use near_sdk::assert_one_yocto;

//defines the payout type we'll be returning as a part of the royalty standards.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Payout {
    pub payout: HashMap<AccountId, U128>,
} 

pub trait Payouts {
    /// Given a `token_id` and NEAR-denominated balance, return the `Payout`.
    /// struct for the given token. Panic if the length of the payout exceeds
    /// `max_len_payout.`
    fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: Option<u32>) -> Payout;
    /// Given a `token_id` and NEAR-denominated balance, transfer the token
    /// and return the `Payout` struct for the given token. Panic if the
    /// length of the payout exceeds `max_len_payout.`
    fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
        balance: U128,
        max_len_payout: Option<u32>,
    ) -> Payout;
}

#[near_bindgen]
impl Payouts for Contract {
    // mint royalty
    #[allow(unused_variables)]
    fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: Option<u32>) -> Payout {
        let owner_id = self.tokens.owner_by_id.get(&token_id).expect("No such token_id");
        //keep track of the total perpetual royalties
        let mut total_perpetual = 0;
        //get the u128 version of the passed in balance (which was U128 before)
        let balance_u128 = u128::from(balance);
        //keep track of the payout object to send back
        let mut payout_object = Payout {
            payout: HashMap::new()
        };
        //go through each key and value in the royalty object
        for (k, v) in self.royalties.iter() {
            //get the key
            let key = k.clone();
            //only insert into the payout if the key isn't the token owner (we add their payout at the end)
            if key != owner_id {
                //
                payout_object.payout.insert(key, util::royalty_to_payout(v, balance_u128).into());
                total_perpetual += v;
            }
        }
        // payout to previous owner who gets 100% - total perpetual royalties
        payout_object.payout.insert(owner_id, util::royalty_to_payout(10000 - total_perpetual, balance_u128).into());
        //return the payout object
        payout_object
    }

    #[payable]
    fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
        balance: U128,
        max_len_payout: Option<u32>,
    ) -> Payout {
        assert_one_yocto();
        let payout = self.nft_payout(token_id.clone(), balance, max_len_payout);
        self.nft_transfer(receiver_id, token_id, approval_id, memo);
        payout
    }
}