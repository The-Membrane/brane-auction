use std::str::FromStr;

use cosmwasm_std::{
    attr, entry_point, has_coins, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Order, QueryRequest, Reply, Response, StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg, WasmQuery
};
use cw2::set_contract_version;

use url::Url;

use sg2::msg::{CollectionParams, CreateMinterMsg, Sg2ExecuteMsg};
use cw721::{TokensResponse, AllNftInfoResponse};
use sg721::{CollectionInfo, RoyaltyInfoResponse};
use sg721_base::msg::{CollectionInfoResponse, QueryMsg as Sg721QueryMsg, ExecuteMsg as Sg721ExecuteMsg};
use crate::{error::ContractError, msgs::{self, Config, ExecuteMsg, InstantiateMsg, QueryMsg}, reply::handle_collection_reply, state::{Auction, Bid, SubmissionInfo, SubmissionItem, AUCTION, CONFIG, PENDING_AUCTION, SUBMISSIONS}};


// Contract name and version used for migration.
const CONTRACT_NAME: &str = "brane_auction";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//Constants
const COLLECTION_REPLY_ID: u64 = 1u64;
const SECONDS_PER_DAY: u64 = 86400u64;
const VOTE_PERIOD: u64 = 7u64;
const AUCTION_PERIOD: u64 = 1u64;
const CURATION_THRESHOLD: Decimal = Decimal::percent(11);

//Minter costs
const MINTER_COST: u128 = 250_000_000u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    //instantiate the Collection
    let collection_msg = Sg2ExecuteMsg::CreateMinter (CreateMinterMsg::<Option<String>> {
        init_msg: None,
        collection_params: CollectionParams { 
            code_id: msg.sg721_code_id, 
            name: String::from("The Memebrane"), 
            symbol: String::from("BRANE"), 
            info: CollectionInfo { 
                creator: String::from("The Memebrane Collective"), 
                description: String::from("The Memebrane is a continuous collection created by the Memebrane Collective. It is a living, breathing, and evolving collection of digital art. The Memebrane is a place where artists can submit their braney work to append to the collection through daily auctions with all proceeds going to the submitting artist. Submissions can be new pfps, memes, portraits, etc. Let your creativity take hold of the pen!....or pencil...or stylus..you get the gist."),
                image: todo!(), //"ipfs://CREATE AN IPFS LINK".to_string(), 
                external_link: Some(String::from("https://twitter.com/insneinthebrane")),
                explicit_content: Some(false), 
                start_trading_time: None, 
                royalty_info: Some(RoyaltyInfoResponse { 
                    payment_address: env.contract.address.to_string(), 
                    share: Decimal::percent(1)
                }) 
            }
        }
    });
    let cosmos_msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_validate(&msg.base_factory_address)?.to_string(),
        msg: to_json_binary(&collection_msg)?,
        funds: vec![
            Coin {
                denom: String::from("ustars"),
                amount: Uint128::new(MINTER_COST),
            }
        ],
    });

    //Create the collection submsg
    let submsg = SubMsg::reply_on_success(cosmos_msg, COLLECTION_REPLY_ID);

    let config = Config {
        owner: info.sender.clone(),
        bid_denom: msg.bid_denom,
        memecoin_denom: msg.memecoin_denom,
        memecoin_distribution_amount: 100_000_000u128,
        current_token_id: 0,
        current_submission_id: 0,
        minter_addr: "".to_string(),
        mint_cost: msg.mint_cost,
        submission_cost: 10_000_000u128,
        submission_limit: 333u64,
        submission_total: 0u64,
        submission_vote_period: VOTE_PERIOD,
        curation_threshold: CURATION_THRESHOLD,
        auction_period: AUCTION_PERIOD,
    };

    CONFIG.save(deps.storage, &config)?;
    PENDING_AUCTION.save(deps.storage, &vec![])?;

    //Set first submission start time
    let first_submission_start_time = env.block.time.seconds() + (SECONDS_PER_DAY * VOTE_PERIOD);

    //Start first Auction
    AUCTION.save(deps.storage, &Auction {
        submission_info: msg.first_submission,
        bids: vec![],
        auction_end_time: env.block.time.seconds() + (SECONDS_PER_DAY * config.auction_period),
        highest_bid: Bid {
            bidder: Addr::unchecked(""),
            amount: 0u128,
        },
    })?;

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("method", "instantiate")
        .add_attribute("config", format!("{:?}", config))
        .add_attribute("contract_address", env.contract.address)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SubmitNFT { submitter, proceed_recipient, token_uri } => submit_nft(deps, env, info, proceed_recipient, token_uri),
        ExecuteMsg::VoteToCurate { submission_ids, vote } => curate_nft(deps, env, info, submission_ids, vote),
        ExecuteMsg::Bid {  } => bid_on_live_auction(deps, env, info),
        ExecuteMsg::ConcludeAuction {  } => conclude_auction(deps, env, info),
        ExecuteMsg::MigrateMinter { new_address } => todo!(),
        ExecuteMsg::UpdateConfig { owner, bid_denom, memecoin_denom, minter_addr } => todo!(),
    }
}

fn get_next_submission_id(
    storage: &mut dyn Storage,
    config: &mut Config
) -> Result<u64, ContractError> {
    let submission_id = config.current_submission_id;
    
    //Increment ID
    config.current_submission_id += 1;
    config.submission_total += 1;
    CONFIG.save(storage, config)?;

    Ok(submission_id)
}

fn submit_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proceed_recipient: String,
    token_uri: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut msgs: Vec<CosmosMsg> = vec![];
    
    // Token URI must be a valid URL (ipfs, https, etc.)
    Url::parse(&token_uri).map_err(|_| ContractError::InvalidTokenURI { uri: token_uri })?;

    //If submission is from a non-holder, it costs Some(bid_asset)
    if let Err(_) = check_if_collection_holder(deps.as_ref(), config.clone().minter_addr, info.clone().sender){
        if !has_coins(&info.funds, &Coin {
            denom: config.bid_denom.clone(),
            amount: Uint128::new(config.submission_cost),
        }) {
            return Err(ContractError::CustomError { val: "Submission cost not sent".to_string() });
        }

        //Send the bid asset to the owner
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: config.owner.to_string(),
            amount: vec![Coin {
                denom: config.bid_denom.clone(),
                amount: Uint128::new(config.submission_cost),
            }],
        }));
    };

    //Create a new submission
    let submission_id = get_next_submission_id(deps.storage, &mut config)?;
    let submission_info = SubmissionItem {
        submission: SubmissionInfo {            
            submitter: info.sender.clone(),
            proceed_recipient: deps.api.addr_validate(&proceed_recipient)?,
            token_uri,
        },
        curation_votes: vec![],
        submission_end_time: env.block.time.seconds() + (config.submission_vote_period * SECONDS_PER_DAY),
    };

    SUBMISSIONS.save(deps.storage, submission_id, &submission_info)?;

    Ok(Response::new()
        .add_attribute("method", "submit_nft")
        .add_attribute("submission_id", submission_id.to_string())
        .add_attribute("submitter", info.sender)
        .add_attribute("submission_info", format!("{:?}", submission_info))
    )
}

fn check_if_collection_holder(
    deps: Deps,
    minter_addr: String,
    sender: Addr,
) -> Result<(), ContractError> {  

    //Check if the sender is a collection holder
    let token_info: TokensResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: minter_addr,
        msg: to_json_binary(&Sg721QueryMsg::Tokens { owner: sender.to_string(), start_after: None, limit: None })?,
    })).map_err(|_| ContractError::CustomError { val: "Failed to query collection, sender may not hold an NFT".to_string() })?;

    if token_info.tokens.is_empty() {
        return Err(ContractError::CustomError { val: "Sender does not hold an NFT in the collection".to_string() });
    }

    Ok(())
}

fn curate_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    submission_ids: Vec<u64>,
    vote: bool,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    //Check if the submission is valid
    if config.submission_total >= config.submission_limit {
        return Err(ContractError::CustomError { val: "Exceeded submission limit".to_string() });
    }

    //Make sure the sender is a collection holder
    check_if_collection_holder(deps.as_ref(), config.clone().minter_addr, info.clone().sender)?;


    //Get total votes
    let all_token_info: TokensResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.clone().minter_addr,
        msg: to_json_binary(&Sg721QueryMsg::AllTokens { start_after: None, limit: None })?,
    }))?;
    let total_votes = all_token_info.tokens.len();
    let passing_threshold = (Uint128::new(total_votes as u128) * config.curation_threshold).u128();

    //Update the submission info
    for submission_id in submission_ids {
        //Load submission info
        let mut submission_info = SUBMISSIONS.load(deps.storage, submission_id)?;
    
        // Assert they haven't voted yet
        if submission_info.curation_votes.contains(&info.clone().sender) {
            continue;
        }
        /// Assert the submission is still in the voting period
        //If its past the submission period and the submission doesn't have enough votes, remove it
        if env.block.time.seconds() > submission_info.submission_end_time {
            if submission_info.curation_votes.len() < passing_threshold as usize {
                SUBMISSIONS.remove(deps.storage, submission_id);
                //Subtract from the submission total
                config.submission_total -= 1;
                continue;
            }
        } 
        //If still in voting period continue voting
        else {            
            //Tally the vote
            if vote {
                submission_info.curation_votes.push(info.sender.clone());
                
                //If the submission has enough votes, add it to the list of auctionables
                if submission_info.curation_votes.len() < passing_threshold as usize {
                    //Set as live auction if there is none, else add to pending auctions
                    if let Err(_) = AUCTION.load(deps.storage) {
                        AUCTION.save(deps.storage, &Auction {
                            submission_info: submission_info.clone(),
                            bids: vec![],
                            auction_end_time: env.block.time.seconds() + (SECONDS_PER_DAY * config.clone().auction_period),
                            highest_bid: Bid {
                                bidder: Addr::unchecked(""),
                                amount: 0u128,                            
                            },
                        })?;
                    } else {

                        PENDING_AUCTION.update(deps.storage, |mut auctions| -> Result<_, ContractError> {
                            auctions.push(Auction {
                                submission_info: submission_info.clone(),
                                bids: vec![],
                                auction_end_time: 0, //will set when active
                                highest_bid: Bid {
                                    bidder: Addr::unchecked(""),
                                    amount: 0u128,                            
                                },
                            });
                            Ok(auctions)
                        })?;
                    }
                    SUBMISSIONS.remove(deps.storage, submission_id);
                    //Subtract from the submission total
                    config.submission_total -= 1;
                } else {
                    //If the submission doesn't have enough votes yet, save it
                    SUBMISSIONS.save(deps.storage, submission_id, &submission_info)?;                
                }
            }
        }
    }

    //Save submission total
    CONFIG.save(deps.storage, &config)?;


    Ok(Response::new()
        .add_attribute("method", "curate_nft")
        .add_attribute("submission_ids", format!("{:?}", submission_ids))
        .add_attribute("curator", info.sender)
        .add_attribute("vote", vote.to_string())
    )
}

fn assert_bid_asset(
    deps: Deps,
    info: &MessageInfo,
    bid_denom: String,    
) -> Result<Bid, ContractError> {
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidAsset { asset: "None or more than 1 asset sent".to_string() });
    }
    //Check if the bid asset was sent
    if info.funds[0].denom != bid_denom {
        return Err(ContractError::InvalidAsset { asset: "Bid asset not sent".to_string() });
    }

    Ok(Bid {
        bidder: info.sender.clone(),
        amount: info.funds[0].amount.u128(),
    })
}

fn bid_on_live_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //Load config
    let config = CONFIG.load(deps.storage)?;
    //Assert funds are the bid asset
    let current_bid = assert_bid_asset(deps.as_ref(), &info, config.bid_denom)?;
    //Initialize msgs
    let mut msgs: Vec<CosmosMsg> = vec![];

    //This will be initiated in the instantiate function & refreshed at the end of the conclude_auction function
    let mut live_auction = AUCTION.load(deps.storage)?;

    //Check if the auction is still live
    if env.block.time.seconds() > live_auction.auction_end_time {
        return Err(ContractError::CustomError { val: "Auction has ended".to_string() });
    }

    //Check if the bid is higher than the current highest bid
    if let Some(highest_bid) = live_auction.bids.last() {
        if current_bid.amount <= highest_bid.amount {
            return Err(ContractError::CustomError { val: "Bid is lower than the current highest bid".to_string() });
        } else {
            //Add the bid to the auction's bid list
            live_auction.bids.push(current_bid);

            //Send the previous highest bid back to the bidder
            msgs.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: highest_bid.bidder.to_string(),
                    amount: vec![Coin {
                        denom: config.bid_denom,
                        amount: Uint128::new(highest_bid.amount),
                    }],
                }));

            //Set bid as highest bid
            live_auction.highest_bid = current_bid;
        }
    }
    AUCTION.save(deps.storage, &live_auction)?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("method", "bid_on_live_auction")
        .add_attribute("bidder", info.sender)
        .add_attribute("bid", current_bid.amount.to_string())
    )
}

fn get_bid_ratios(
    bids: &Vec<Bid>
) -> Vec<(Addr, Decimal)> {
    let mut bid_ratios: Vec<(Addr, Decimal)> = vec![];
    let total_bids = bids.iter().fold(0u128, |acc, bid| acc + bid.amount);

    //Aggregate bids of the same bidder
    let bids = bids.iter().fold(vec![], |mut acc, bid| {
        if let Some(bid_index) = acc.iter().position(|x: &Bid| x.bidder == bid.bidder) {
            acc[bid_index].amount += bid.amount;
        } else {
            acc.push(bid.clone());
        }
        acc
    });

    //Get ratios
    for bid in bids {
        bid_ratios.push((bid.bidder.clone(), Decimal::from_ratio(bid.amount, total_bids)));
    }

    bid_ratios
}

fn conclude_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //Load config
    let config = CONFIG.load(deps.storage)?;
    //Initialize msgs
    let mut msgs: Vec<CosmosMsg> = vec![];
    //Load live auction
    let mut live_auction = AUCTION.load(deps.storage)?;

    //Check if the auction is still live
    if env.block.time.seconds() < live_auction.auction_end_time {
        return Err(ContractError::CustomError { val: "Auction is still live".to_string() });
    }

    //Mint the NFT & send the bid to the proceed_recipient
    if live_auction.highest_bid.amount > 0 {
        //Mint the NFT to the highest bidder
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.minter_addr,
            msg: to_json_binary(&Sg721ExecuteMsg::Mint::<Option<String>, Option<String>> {
                owner: live_auction.highest_bid.bidder.to_string(),
                token_id: config.current_token_id.to_string(),
                token_uri: Some(live_auction.submission_info.submission.token_uri.clone()),
                extension: None,
            })?,
            funds: vec![
                Coin {
                    denom: String::from("ustars"),
                    amount: Uint128::new(config.mint_cost),
                }],
        }));

        //Send the highest bid to the proceed_recipient
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: live_auction.submission_info.submission.proceed_recipient.to_string(),
            amount: vec![Coin {
                denom: config.bid_denom,
                amount: Uint128::new(live_auction.highest_bid.amount),
            }],
        }));

        /////Send memecoins to Bidders & curators
        if let Some(meme_denom) = config.memecoin_denom {
            //Get memecoin distribution amount
            let memecoin_distribution_amount = match deps.querier.query_balance(env.contract.address, config.memecoin_denom){
                Ok(balance) => {
                    //We distribute the config amount or half of the balance, whichever is lower
                    if balance.amount.u128() / 2 < config.memecoin_distribution_amount {
                        balance.amount.u128() / 2
                    } else {
                        config.memecoin_distribution_amount
                    }
                },
                Err(_) => config.memecoin_distribution_amount,
            };

            //Get bidder pro_rata distribution
            let bid_ratios = get_bid_ratios(&live_auction.bids);            
            //Split total memecoins between bidders (pro_rata to bid_amount)
            let meme_to_bidders = bid_ratios.iter().map(|bidder| {
                let meme_amount = (Uint128::new(memecoin_distribution_amount) * bidder.1).u128();
                (bidder.0, Coin {
                    denom: meme_denom.clone(),
                    amount: Uint128::new(meme_amount),
                })
            }).collect::<Vec<(Addr, Coin)>>();
            //Split total memecoins between curators (1/len)
            let meme_to_curators = live_auction.submission_info.curation_votes.iter().map(|curator| {
                let meme_amount = (Uint128::new(memecoin_distribution_amount) * Decimal::from_ratio(1, live_auction.submission_info.curation_votes.len() as u128)).u128();
                (curator.clone(), Coin {
                    denom: meme_denom.clone(),
                    amount: Uint128::new(meme_amount),
                })
            }).collect::<Vec<(Addr, Coin)>>();

            //Create the memecoin distribution msgs
            for (bidder, coin) in meme_to_bidders {
                msgs.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: bidder.to_string(),
                    amount: vec![coin],
                }));
            }
            for (curator, coin) in meme_to_curators {
                msgs.push(CosmosMsg::Bank(BankMsg::Send {
                    to_address: curator.to_string(),
                    amount: vec![coin],
                }));
            }
        }
        
    }

    //Set the new auction to the next pending auction
    if let Some(mut next_auction) = PENDING_AUCTION.load(deps.storage)?.pop() {
        //set auction end time
        next_auction.auction_end_time = env.block.time.seconds() + (SECONDS_PER_DAY * config.auction_period);
        //Save as live auction
        AUCTION.save(deps.storage, &next_auction)?;
    }
    //Remove the concluded auction
    AUCTION.remove(deps.storage);    

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("method", "conclude_auction")
        .add_attribute("highest_bidder", live_auction.highest_bid.bidder)
        .add_attribute("highest_bid", live_auction.highest_bid.amount.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        COLLECTION_REPLY_ID => handle_collection_reply(deps, env, msg),
        id => Err(StdError::generic_err(format!("invalid reply id: {}", id))),
    }
}
