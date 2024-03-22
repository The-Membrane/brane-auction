use cosmwasm_std::{Addr, Decimal, Uint128};
use cosmwasm_schema::cw_serde;

use crate::state::SubmissionItem;

#[cw_serde]
pub struct InstantiateMsg {
    pub sg721_code_id: u64, //testnet: 2595, mainnet: 180
    pub base_factory_address: String, //testnet: stars1a45hcxty3spnmm2f0papl8v4dk5ew29s4syhn4efte8u5haex99qlkrtnx, mainnet: stars1klnzgwfvca8dnjeasx00v00f49l6nplnvnsxyc080ph2h8qxe4wss4d3ga
    /// Bid denom
    pub bid_denom: String,
    /// Memecoin denom
    pub memecoin_denom: Option<String>,
    /// First submission for the first NFT auction of the collection
    pub first_submission: SubmissionItem,
    ///Mint cost
    pub mint_cost: u128,
}

#[cw_serde]
pub enum ExecuteMsg {
    SubmitNFT { 
        submitter: String,
        proceed_recipient: String,
        token_uri: String,
    },
    /// Submissions have 7 days to get votes, after 7 days any votes will delete the submission
    VoteToCurate { submission_ids: Vec<u64>, vote: bool },
    Bid { },
    /// Transfer NFT to highest bidder & handle memecoin distributions
    ConcludeAuction { },
    ////These are all controlled by the owner who will be a DAODAO NFT staking contract
    MigrateMinter { new_address: String },
    // MigrateContract { new_code_id: u64 },
    UpdateConfig {
        owner: Option<Addr>,
        bid_denom: Option<String>,
        memecoin_denom: Option<String>,
        minter_addr: Option<String>, //do we need this and the migrate minter?
    },
    //////
}

#[cw_serde]
pub enum QueryMsg {
    /// Return contract config
    Config {},
    /// Return list of submissions
    Submissions { limit: Option<u32>, start_after: Option<u64> },
}

#[cw_serde]
pub struct Config {
    /// Contract owner
    pub owner: Addr,
    /// Bid denom
    pub bid_denom: String,
    /// Memecoin denom
    pub memecoin_denom: Option<String>,
    /// Memecoin distribution amount
    pub memecoin_distribution_amount: u128,
    /// Current token ID
    pub current_token_id: u64,
    /// Current submission ID
    pub current_submission_id: u64,
    /// Minter address
    pub minter_addr: String,
    /// Stargaze Mint cost 
    /// Testnet: 50_000_000u128
    /// Mainnet: 5_000_000_000u128
    pub mint_cost: u128,
    /// Submission cost for non-holders in the bid_denom
    pub submission_cost: u128,
    /// Submission limit
    pub submission_limit: u64,
    /// Current submission total
    pub submission_total: u64,
    /// Submission vote period (in days)
    pub submission_vote_period: u64,
    /// Curation threshold (i.e. % of Yes votes)
    pub curation_threshold: Decimal,
    /// Auction period (in days)
    pub auction_period: u64, 
}
