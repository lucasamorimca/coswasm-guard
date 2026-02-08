// TEST FIXTURE — intentional vulnerabilities for testing detectors.
// This file is NOT a real contract; it uses cosmwasm_std types syntactically.

use cosmwasm_std::{
    entry_point, DepsMut, Deps, Env, MessageInfo, Response,
    Binary, StdResult, Uint128, Order,
};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};

// State
const CONFIG: Item<Config> = Item::new("config");
const BALANCES: Map<&str, Uint128> = Map::new("balances");

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub owner: String,
}

#[derive(Serialize, Deserialize)]
pub struct InstantiateMsg {}

// Messages — BUG: String address fields without validation
#[derive(Serialize, Deserialize)]
pub enum ExecuteMsg {
    Transfer {
        recipient: String,    // BUG: String address not validated
        amount: Uint128,
    },
    UpdateConfig {
        new_owner: String,    // BUG: String address not validated
    },
    Withdraw {},              // BUG: No access control
}

// Entry points
#[entry_point]
pub fn instantiate(
    deps: DepsMut, _env: Env, info: MessageInfo, _msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config { owner: info.sender.to_string() };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new())
}

// BUG: Execute handler has no info.sender check at top level
#[entry_point]
pub fn execute(
    deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            // BUG: recipient not validated with addr_validate
            BALANCES.save(deps.storage, &recipient, &amount)?;
            Ok(Response::new())
        }
        ExecuteMsg::UpdateConfig { new_owner } => {
            // BUG: No info.sender check (missing access control)
            // BUG: new_owner not validated
            let config = Config { owner: new_owner };
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new())
        }
        ExecuteMsg::Withdraw {} => {
            // BUG: Unbounded iteration — no .take() limit
            let _all_balances: Vec<_> = BALANCES
                .range(deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<_>>>()?;
            Ok(Response::new())
        }
    }
}
