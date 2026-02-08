// TEST FIXTURE — safe contract patterns. All detectors should return zero findings.

use cosmwasm_std::{
    entry_point, DepsMut, Deps, Env, MessageInfo, Response,
    StdResult, StdError, Uint128, Order,
};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};

const CONFIG: Item<Config> = Item::new("config");
const BALANCES: Map<&str, Uint128> = Map::new("balances");

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub owner: String,
}

#[derive(Serialize, Deserialize)]
pub enum ExecuteMsg {
    Transfer { recipient: String, amount: Uint128 },
    ListBalances { limit: u32 },
}

#[entry_point]
pub fn execute(
    deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            // SAFE: address validated
            let _validated = deps.api.addr_validate(&recipient)?;
            // SAFE: sender checked
            if info.sender != CONFIG.load(deps.storage)?.owner {
                return Err(StdError::generic_err("unauthorized"));
            }
            Ok(Response::new())
        }
        ExecuteMsg::ListBalances { limit } => {
            // SAFE: bounded iteration with .take()
            let _balances: Vec<_> = BALANCES
                .range(deps.storage, None, None, Order::Ascending)
                .take(limit as usize)
                .collect::<StdResult<Vec<_>>>()?;
            Ok(Response::new())
        }
    }
}
