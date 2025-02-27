#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

use crate::state::{Config, CONFIG, LAST_INDEX, PRICE_HISTORY};
use neutron_std::types::slinky::oracle::v1::{GetPriceResponse, OracleQuerier};
use neutron_std::types::slinky::types::v1::CurrencyPair;

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
        let key = pair_key(pair);
        LAST_INDEX.save(deps.storage, &key, &0)?;
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
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdatePrices {} => execute_update_prices(deps, env, info),
        ExecuteMsg::UpdateConfig {
            pairs,
            update_period,
            max_blocks_old,
            history_size,
        } => execute_update_config(
            deps,
            env,
            info,
            pairs,
            update_period,
            max_blocks_old,
            history_size,
        ),
    }
}

/// Triggered by the authorized Cron module. For each configured pair, this function:
/// 1. Queries the oracle price.
/// 2. Validates that the price is fresh (not too old) and not nil.
/// 3. Writes a new record to the ring buffer (only one record is read/written per pair).
pub fn execute_update_prices(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Only the authorized address (e.g. Cron module) can trigger this.
    if info.sender != config.caller {
        return Err(ContractError::Unauthorized {});
    }

    // Enforce the minimal update period.
    if env.block.height < config.last_update + config.update_period {
        return Err(ContractError::TooSoon {
            expected: config.last_update + config.update_period,
        });
    }

    let mut res = Response::new();

    // Process each pair.
    for pair in config.pairs.iter() {
        let key = pair_key(pair);
        // Query the oracle for the price.
        let oracle_resp = query_oracle_price(&deps.as_ref(), pair)?;

        if let Some(price) = oracle_resp.price {
            if !validate_price(
                oracle_resp.nonce,
                env.block.height,
                price.block_height,
                config.max_blocks_old,
            ) {
                res = res.add_attribute("skip", key.clone());
                continue;
            }

            // Update the ring buffer for this pair.
            let pointer = LAST_INDEX.may_load(deps.storage, &key)?.unwrap_or(0);

            PRICE_HISTORY.save(deps.storage, (key.as_str(), pointer), &price)?;

            // Update the pointer (wrap around using modulo arithmetic).
            let new_pointer = (pointer + 1) % config.history_size;
            LAST_INDEX.save(deps.storage, &key, &new_pointer)?;

            res = res.add_attribute("updated", key.clone());
        } else {
            // If there's no oracle response, skip to the next item in the loop.
            continue;
        }
    }

    // Update the last update block.
    config.last_update = env.block.height;
    CONFIG.save(deps.storage, &config)?;
    Ok(res)
}

/// Allows the authorized address to update configuration parameters.
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pairs: Option<Vec<CurrencyPair>>,
    update_period: Option<u64>,
    max_blocks_old: Option<u64>,
    history_size: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != &config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(new_pairs) = pairs {
        // Reset pointers for new pairs.
        for pair in new_pairs.iter() {
            let key = pair_key(pair);
            LAST_INDEX.save(deps.storage, &key, &0)?;
        }
        config.pairs = new_pairs;
    }
    if let Some(up) = update_period {
        config.update_period = up;
    }
    if let Some(mbo) = max_blocks_old {
        config.max_blocks_old = mbo;
    }
    if let Some(hs) = history_size {
        config.history_size = hs;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
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

/// ------------------------------------------------------------------------------------------------
/// HELPERS
/// ------------------------------------------------------------------------------------------------

/// Query the oracle price for a given pair.
pub fn query_oracle_price(
    deps: &Deps,
    pair: &CurrencyPair,
) -> Result<GetPriceResponse, ContractError> {
    let querier = OracleQuerier::new(&deps.querier);
    let price: GetPriceResponse = querier.get_price(Some(pair.clone()))?;
    Ok(price)
}

pub fn validate_price(
    nonce: u64,
    current_height: u64,
    price_height: u64,
    max_blocks_old: u64,
) -> bool {
    // Check that the price received more than zero updates from validators.
    // TODO(zavgorodnii): maybe introduce a parameter for the desired nonce?
    if nonce == 0 {
        return false;
    }

    // Check that the price is not stale.
    if current_height.saturating_sub(price_height) > max_blocks_old {
        return false;
    }

    true
}

/// Returns a string key for a currency pair.
fn pair_key(pair: &CurrencyPair) -> String {
    format!("{}-{}", pair.base, pair.quote)
}

/// ------------------------------------------------------------------------------------------------
/// TESTS
/// ------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};

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
            pairs: vec![
                CurrencyPair {
                    base: "untrn".to_string(),
                    quote: "usd".to_string(),
                },
                CurrencyPair {
                    base: "uatom".to_string(),
                    quote: "usd".to_string(),
                },
            ],
            update_period: 10,
            max_blocks_old: 100,
            history_size: 5,
        };

        // Call the instantiate function
        instantiate(deps.as_mut(), env.clone(), info, instantiate_msg.clone())
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
            let key = pair_key(pair);
            let idx = LAST_INDEX
                .load(&deps.storage, &key)
                .expect("pair index must be saved");
            assert_eq!(idx, 0);
        }
    }
}
