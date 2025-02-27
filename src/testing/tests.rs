use crate::contract::{execute, instantiate, pair_key};
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::{CONFIG, LAST_INDEX, PRICE_HISTORY};
use crate::testing::mock_querier::{mock_dependencies, MockOraclePriceData};
use crate::ContractError;
use cosmwasm_std::attr;
use cosmwasm_std::testing::{message_info, mock_env};
use neutron_std::types::slinky::oracle::v1::QuotePrice;
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

#[test]
fn test_update_prices_success_and_skip_logic() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Instantiate with 2 pairs
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
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
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // Manually move block height forward to allow updates
    env.block.height += 20;

    // Insert some mock oracle data:
    // 1) untrn-usd -> valid data (nonce=2, price.block_height=env.block.height - 5)
    // 2) uatom-usd -> stale data (nonce=3, but block_height=0 or something older than max_blocks_old)
    let key1 = "untrn-usd";
    deps.querier.update_oracle_data(
        key1,
        MockOraclePriceData {
            price: Some(QuotePrice {
                price: "1000".to_string(),
                block_timestamp: None,
                block_height: env.block.height - 5,
            }),
            nonce: 2,
            decimals: 6,
            id: 42,
        },
    );
    let key2 = "uatom-usd";
    deps.querier.update_oracle_data(
        key2,
        MockOraclePriceData {
            price: Some(QuotePrice {
                price: "1000".to_string(),
                block_timestamp: None,
                block_height: env.block.height - 200, // definitely older than max_blocks_old=100
            }),
            nonce: 3,
            decimals: 6,
            id: 123,
        },
    );

    // Now call update_prices
    let info = message_info(&deps.api.addr_make("caller_addr"), &[]);
    let msg = ExecuteMsg::UpdatePrices {};
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // We expect one pair to be updated, the other to be skipped
    // The "skip" attribute is for the stale pair (uatom-usd)
    // The "updated" attribute is for the fresh pair (untrn-usd)
    assert_eq!(res.attributes.len(), 2);
    assert_eq!(res.attributes[0], attr("updated", key1));
    assert_eq!(res.attributes[1], attr("skip", key2));

    // Check that ring buffer has a record for the updated pair
    let pointer = LAST_INDEX.load(&deps.storage, key1).unwrap();
    // Because we started from 0 and wrote one record
    assert_eq!(pointer, 1);

    // The actual price stored:
    let stored_price = PRICE_HISTORY
        .load(&deps.storage, (key1, 0))
        .expect("should store price at index 0");
    assert_eq!(stored_price.block_height, env.block.height - 5);

    // The stale pair should still have pointer=0 (no writes)
    let pointer2 = LAST_INDEX.load(&deps.storage, key2).unwrap();
    assert_eq!(pointer2, 0);

    // Confirm that config.last_update was updated to the new block height
    let config = CONFIG.load(&deps.storage).unwrap();
    assert_eq!(config.last_update, env.block.height);
}

#[test]
fn test_update_prices_ring_buffer_wrap() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // We'll set history_size to 2 to force wrap
    let instantiate_msg = InstantiateMsg {
        owner: deps.api.addr_make("owner_addr").to_string(),
        caller: deps.api.addr_make("caller_addr").to_string(),
        pairs: vec![CurrencyPair {
            base: "untrn".to_string(),
            quote: "usd".to_string(),
        }],
        update_period: 1,
        max_blocks_old: 100,
        history_size: 2,
    };
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg).unwrap();

    // We'll do multiple update calls to force the ring buffer to wrap
    let key = "untrn-usd";

    for i in 0..5 {
        env.block.height += 2; // enough blocks so we don't get "TooSoon"

        deps.querier.update_oracle_data(
            key,
            MockOraclePriceData {
                price: Some(QuotePrice {
                    price: format!("{}", i),
                    block_timestamp: None,
                    block_height: env.block.height,
                }),
                nonce: 1,
                decimals: 6,
                id: 0,
            },
        );

        let info = message_info(&deps.api.addr_make("caller_addr"), &[]);
        let msg = ExecuteMsg::UpdatePrices {};
        let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    }

    // Because history_size=2, pointer should always cycle among 0,1
    // after 5 updates, pointer = 5 mod 2 = 1
    let pointer = LAST_INDEX.load(&deps.storage, key).unwrap();
    assert_eq!(pointer, 5 % 2);

    // Let's see what's stored at index 0,1 in the ring buffer
    // The final pointer is 1, meaning the last write was to index 0 (because we do pointer,
    // then pointer+1 mod size). However, let's just check what's there.

    let price0 = PRICE_HISTORY.load(&deps.storage, (key, 0)).unwrap();
    let price1 = PRICE_HISTORY.load(&deps.storage, (key, 1)).unwrap();

    // Because the final update had price=4 (0-based), that means the final stored price is 4,
    // so presumably the ring buffer alternated writes:
    //  - 1st update => writes to index 0 (pointer was 0, then pointer=1)
    //  - 2nd update => writes to index 1 (pointer was 1, then pointer=0)
    //  - 3rd update => writes to index 0 (pointer was 0, then pointer=1)
    //  - 4th update => writes to index 1 (pointer was 1, then pointer=0)
    //  - 5th update => writes to index 0 (pointer was 0, then pointer=1)
    //
    // So after the 5th update, index 0 = i=4, index 1 = i=3.
    assert_eq!(price0.price, "4");
    assert_eq!(price1.price, "3");
}
