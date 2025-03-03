use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use neutron_std::types::slinky::oracle::v1::QuotePrice;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Contract configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Owner can change the configuration.
    pub owner: Addr,
    /// The authorized address (e.g. Cron module) allowed to trigger updates.
    pub caller: Addr,
    /// The list of currency pairs for which we want to store prices.
    pub pairs: Vec<CurrencyPair>,
    /// Minimal period (in blocks) between price updates.
    pub update_period: u64,
    /// Maximum number of blocks a price can be old to be considered fresh.
    pub max_blocks_old: u64,
    /// How many historical records to store per pair.
    pub history_size: u64,
    /// The block height when the last update was performed.
    pub last_update: u64,
}

/// Singleton config storage.
pub const CONFIG: Item<Config> = Item::new("config");

/// For each pair (keyed by a string identifier), we store the next write pointer.
pub const LAST_INDEX: Map<&str, u64> = Map::new("last_index");

/// Ring-buffer storage for price records for each pair.
/// The key is a tuple: (pair identifier, index within the ring buffer).
pub const PRICE_HISTORY: Map<(&str, u64), QuotePrice> = Map::new("price_history");
