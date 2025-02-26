#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, MigrateMsg};
use crate::state::{Config, CONFIG, LAST_INDEX};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:neutron-oracle-history";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;
    let caller = deps.api.addr_validate(&msg.caller)?;

    let config = Config {
        owner,
        caller,
        pairs: msg.pairs.clone(),
        update_period: msg.update_period,
        max_blocks_old: msg.max_blocks_old,
        history_size: msg.history_size,
        last_update: env.block.height,
    };
    CONFIG.save(deps.storage, &config)?;

    // Initialize the ring buffer pointer for each pair to 0.
    for pair in msg.pairs.iter() {
        LAST_INDEX.save(deps.storage, &pair, &0)?;
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("pairs", format!("{:?}", msg.pairs))
        .add_attribute("update_period", msg.update_period.to_string())
        .add_attribute("max_blocks_old", msg.max_blocks_old.to_string())
        .add_attribute("history_size", msg.history_size.to_string())
        .add_attribute("authorized", format!("{:?}", msg.caller))
        .add_attribute("last_update", env.block.height.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Set contract to version to latest
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, message_info},
    };

    #[test]
    fn test_instantiate_success() {
        // Set up a mock environment with default values
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&deps.api.addr_make("creator"), &[]);

        // Create a sample InstantiateMsg
        let instantiate_msg = InstantiateMsg {
            owner: deps.api.addr_make("owner_addr").into_string(),
            caller: deps.api.addr_make("caller_addr").into_string(),
            pairs: vec!["pair1".to_string(), "pair2".to_string()],
            update_period: 10,
            max_blocks_old: 100,
            history_size: 5,
        };

        // Call the instantiate function
        let res = instantiate(deps.as_mut(), env.clone(), info, instantiate_msg.clone())
            .expect("contract initialization should succeed");

        // Verify that the stored config matches the input data
        let config = CONFIG.load(&deps.storage).expect("config must be saved");
        assert_eq!(config.owner, deps.api.addr_make("owner_addr"));
        assert_eq!(config.caller, deps.api.addr_make("caller_addr"));
        assert_eq!(config.pairs, instantiate_msg.pairs);
        assert_eq!(config.update_period, instantiate_msg.update_period);
        assert_eq!(config.max_blocks_old, instantiate_msg.max_blocks_old);
        assert_eq!(config.history_size, instantiate_msg.history_size);
        assert_eq!(config.last_update, env.block.height);

        // Verify that ring buffer pointers are initialized for each pair with 0
        for pair in &instantiate_msg.pairs {
            let idx = LAST_INDEX
                .load(&deps.storage, pair)
                .expect("pair index must be saved");
            assert_eq!(idx, 0);
        }
    }
}