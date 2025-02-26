use neutron_std::types::slinky::oracle::v1::QuotePrice;
use cosmwasm_schema::{cw_serde, QueryResponses};
use crate::state::{Config};

#[cw_serde]
pub struct InstantiateMsg {
    /// Owner can change the configuration.
    pub owner: String,
    /// The authorized address (e.g. Cron module) that can trigger price updates.
    pub caller: String,
    /// The list of currency pairs to track.
    pub pairs: Vec<String>,
    /// Minimal period (in blocks) between updates.
    pub update_period: u64,
    /// Maximum allowed block age for a price to be "fresh".
    pub max_blocks_old: u64,
    /// How many historical records (ring buffer size) to store per pair.
    pub history_size: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Called (by the Cron module) to update prices for all configured pairs.
    UpdatePrices {},
    /// Update the configuration (only callable by the authorized address).
    UpdateConfig {
        /// Optionally update the list of currency pairs.
        pairs: Option<Vec<String>>,
        /// Optionally update the update period (in blocks).
        update_period: Option<u64>,
        /// Optionally update the maximum allowed block age.
        max_blocks_old: Option<u64>,
        /// Optionally update the ring buffer size.
        history_size: Option<u64>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the current configuration.
    #[returns(ConfigResponse)]
    Config {},
    /// Query historical price data for a set of currency pairs.
    #[returns(HistoryResponse)]
    History {
        /// The list of currency pairs to query.
        pairs: Vec<String>,
    },
}

/// The query response for history queries.
#[cw_serde]
pub struct HistoryResponse {
    /// A list of tuples containing the currency pair and its ordered list of price records.
    pub histories: Vec<(String, Vec<QuotePrice>)>,
}

/// The query response for the config query.
#[cw_serde]
pub struct ConfigResponse {
    pub config: Config,
}

#[cw_serde]
pub struct MigrateMsg {}
