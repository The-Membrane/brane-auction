use cosmwasm_schema::cw_serde;
use cw_storage_plus::{Item, Map};
use cosmwasm_std::Addr;

use crate::msgs::Config;


#[cw_serde]
pub struct Bid {
    pub bidder: Addr,
    pub amount: u128,
}

#[cw_serde]
pub struct Votes {
    pub yes: u64,
    pub no: u64,
}

#[cw_serde]
pub struct SubmissionInfo {
    pub submitter: Addr,
    pub proceed_recipient: Addr,
    pub token_uri: String,
}
#[cw_serde]
pub struct SubmissionItem {
    pub submission: SubmissionInfo,
    pub curation_votes: Vec<Addr>,
    pub submission_end_time: u64, //in seconds
}

#[cw_serde]
pub struct Auction {
    pub submission_info: SubmissionItem,
    pub bids: Vec<Bid>,
    pub highest_bid: Bid,
    pub auction_end_time: u64, //in seconds
}


pub const CONFIG: Item<Config> = Item::new("config");
pub const SUBMISSIONS: Map<u64, SubmissionItem> = Map::new("submissions");
pub const PENDING_AUCTION: Item<Vec<Auction>> = Item::new("pending_auctions");
pub const AUCTION: Item<Auction> = Item::new("current_auction");


pub const OWNERSHIP_TRANSFER: Item<Addr> = Item::new("ownership_transfer");