use cosmwasm_std::{DepsMut, Env, Reply, Response, StdError, StdResult};

use crate::state::CONFIG;



pub fn handle_collection_reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.result.into_result() {
        Ok(result) => {
            
            // let mint_event = result
            //     .events
            //     .iter()
            //     .find(|e| {
            //         e.attributes
            //             .iter()
            //             .any(|attr| attr.key == "_contract_address")
            //     })
            //     .ok_or_else(|| {
            //         StdError::GenericErr { msg: String::from("unable to find mint event") }
            //     })?;

            panic!("Events: {:?}", result.events);

            //Load config
            let mut config = CONFIG.load(deps.storage)?;

            //Save mint address
            // config.minter_addr = mint_event
            //     .attributes
            //     .iter()
            //     .find(|attr| attr.key == "_contract_address")
            //     .ok_or_else(|| {
            //         StdError::GenericErr { msg: String::from("unable to find mint address") }
            //     })?
            //     .value
            //     .clone();
            todo!("Save the minter address to storage on success");

            Ok(Response::new()
            .add_attribute("mint_addr", config.clone().minter_addr))
        },
        
        Err(err) => {
            //Its reply on success only
            Ok(Response::new().add_attribute("error", err))
        }
    }    

}