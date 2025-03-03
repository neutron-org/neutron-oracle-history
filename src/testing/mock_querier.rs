use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_json, Binary, ContractResult, Empty, GrpcQuery, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult,
};
use neutron_std::types::slinky::oracle::v1::{GetPriceRequest, GetPriceResponse, QuotePrice};
use prost::Message;
use std::collections::HashMap;
use std::marker::PhantomData;

// A convenience struct to store mock price data for each pair.
#[derive(Clone, Debug)]
pub struct MockOraclePriceData {
    pub price: Option<QuotePrice>,
    pub nonce: u64,
    pub decimals: u64,
    pub id: u64,
}

impl Default for MockOraclePriceData {
    fn default() -> Self {
        Self {
            price: None,
            nonce: 0,
            decimals: 0,
            id: 0,
        }
    }
}

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_storage = MockStorage::default();
    let custom_querier = WasmMockQuerier::new(MockQuerier::new(&[]));

    OwnedDeps {
        storage: custom_storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

/// A custom mock querier that can store and return per-pair oracle data.
pub struct WasmMockQuerier {
    base: MockQuerier,
    // Map from "pair_key" => MockOraclePriceData
    oracle_data: HashMap<String, MockOraclePriceData>,
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier {
            base,
            oracle_data: HashMap::new(),
        }
    }

    /// Allows tests to insert mock price data for a given pair key
    pub fn update_oracle_data(&mut self, key: &str, data: MockOraclePriceData) {
        self.oracle_data.insert(key.to_string(), data);
    }

    fn handle_grpc_query(&self, path: &str, data: &[u8]) -> SystemResult<ContractResult<Binary>> {
        match path {
            "/slinky.oracle.v1.Query/GetPrice" => {
                // Decode the request so we can check which pair is queried
                let req = match GetPriceRequest::decode(data) {
                    Ok(r) => r,
                    Err(e) => {
                        return SystemResult::Err(SystemError::InvalidRequest {
                            error: format!("Failed to decode GetPriceRequest: {}", e),
                            request: data.into(),
                        });
                    }
                };

                // The request should contain an optional currency pair
                if let Some(pair) = req.currency_pair {
                    let key = format!("{}/{}", pair.base, pair.quote);
                    // Lookup mock data from the HashMap
                    let mock_data = self.oracle_data.get(&key).cloned().unwrap_or_default();

                    let resp = GetPriceResponse {
                        price: mock_data.price,
                        nonce: mock_data.nonce,
                        decimals: mock_data.decimals,
                        id: mock_data.id,
                    };
                    SystemResult::Ok(ContractResult::Ok(Binary::from(resp.to_proto_bytes())))
                } else {
                    // If no pair is provided, return default empty
                    let resp = GetPriceResponse::default();
                    SystemResult::Ok(ContractResult::Ok(Binary::from(resp.to_proto_bytes())))
                }
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: format!("Unhandled path in mock_querier: {}", path),
            }),
        }
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Grpc(GrpcQuery { data, path }) => self.handle_grpc_query(path, data),
            _ => self.base.handle_query(request),
        }
    }
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return QuerierResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                });
            }
        };
        self.handle_query(&request)
    }
}
