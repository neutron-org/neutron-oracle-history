use crate::contract::{execute, instantiate, pair_key};
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::{CONFIG, LAST_INDEX};
use crate::testing::mock_querier::mock_dependencies;
use crate::ContractError;
use cosmwasm_std::attr;
use cosmwasm_std::testing::{message_info, mock_env};
use neutron_std::types::slinky::types::v1::CurrencyPair;

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

#[test]
fn test_update_config_unauthorized() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Instantiate with owner = "owner_addr"
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
        pairs: vec![],
        update_period: 10,
        max_blocks_old: 100,
        history_size: 5,
    };
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // Attempt to update config with a non-owner
    let info = message_info(&deps.api.addr_make("non_owner"), &[]);
    let msg = ExecuteMsg::UpdateConfig {
        pairs: None,
        update_period: Some(20),
        max_blocks_old: None,
        history_size: None,
    };

    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
}

#[test]
fn test_update_config_success() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate with owner = "owner_addr"
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
        pairs: vec![CurrencyPair {
            base: "untrn".to_string(),
            quote: "usd".to_string(),
        }],
        update_period: 10,
        max_blocks_old: 100,
        history_size: 5,
    };
    let info = message_info(&deps.api.addr_make("anyone"), &[]);
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // Now update config with the correct owner
    let info = message_info(&deps.api.addr_make("owner_addr"), &[]);
    let new_pairs = vec![CurrencyPair {
        base: "uatom".to_string(),
        quote: "usd".to_string(),
    }];
    let msg = ExecuteMsg::UpdateConfig {
        pairs: Some(new_pairs.clone()),
        update_period: Some(50),
        max_blocks_old: Some(200),
        history_size: Some(10),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(res.attributes, vec![attr("action", "update_config")]);

    let config = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(config.pairs, new_pairs);
    assert_eq!(config.update_period, 50);
    assert_eq!(config.max_blocks_old, 200);
    assert_eq!(config.history_size, 10);

    // Check that ring buffer pointers are reset for new pairs
    let key = pair_key(&CurrencyPair {
        base: "uatom".to_string(),
        quote: "usd".to_string(),
    });
    let pointer = LAST_INDEX.load(&deps.storage, &key).unwrap();
    assert_eq!(pointer, 0);
}

#[test]
fn test_update_prices_unauthorized() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Instantiate
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
        pairs: vec![
            CurrencyPair {
                base: "untrn".to_string(),
                quote: "usd".to_string(),
            },
        ],
        update_period: 10,
        max_blocks_old: 100,
        history_size: 5,
    };
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // Attempt to update prices from an unauthorized address
    let info = message_info(&deps.api.addr_make("random_addr"), &[]);
    let msg = ExecuteMsg::UpdatePrices {};
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
}

#[test]
fn test_update_prices_too_soon() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Instantiate
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
        pairs: vec![],
        update_period: 10,
        max_blocks_old: 100,
        history_size: 5,
    };
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // Attempt to update prices with correct caller but not enough blocks passed
    let info = message_info(&deps.api.addr_make("caller_addr"), &[]);
    let msg = ExecuteMsg::UpdatePrices {};
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();

    // Because last_update = env.block.height, we need at least 10 more blocks
    if let ContractError::TooSoon { expected } = err {
        assert_eq!(expected, env.block.height + 10);
    } else {
        panic!("expected TooSoon error");
    }
}
