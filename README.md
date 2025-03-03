# Neutron Oracle History Contract

This CosmWasm smart contract maintains historical price data from an oracle (such as Neutron's Slinky Oracle) using an efficient ring buffer mechanism. It periodically fetches and stores price updates for specified currency pairs, ensuring data freshness and availability for queries.

## Overview

- **Periodic Updates:** Triggered by an authorized address (e.g., Cron module).
- **Ring Buffer Storage:** Efficiently stores a fixed history of recent prices for configured currency pairs.
- **Configurable:** Allows dynamic adjustment of currency pairs, update intervals, history sizes, and freshness criteria.

## Installation

Clone this repository and compile with:

```sh
cargo wasm
```

## Instantiation

Instantiate the contract with initial parameters:

```json
{
  "owner": "<owner-address>",
  "caller": "<cron-module-address>",
  "pairs": [
    { "base": "ATOM", "quote": "USD" },
    { "base": "NTRN", "quote": "USD" }
  ],
  "update_period": 10,
  "max_blocks_old": 5,
  "history_size": 50
}
```

## Execute Messages

### `UpdatePrices`

Triggered by the authorized caller to fetch current oracle prices and update the history:

```json
{
  "update_prices": {}
}
```

### `UpdateConfig`

Allows the owner to adjust contract settings:

```json
{
  "update_config": {
    "pairs": [{ "base": "BTC", "quote": "USD" }],
    "update_period": 15,
    "max_blocks_old": 10,
    "history_size": 100
  }
}
```

_All fields are optional._

## Queries

### `Config`

Returns the current contract configuration:

```json
{
  "config": {}
}
```

### `History`

Returns historical price records for requested currency pairs:

```json
{
  "history": {
    "pairs": [{ "base": "ATOM", "quote": "USD" }]
  }
}
```

## Performance Considerations

The contract intentionally reads from storage `HISTORY_SIZE` times per requested currency pair when executing the `query_history` function. This design choice optimizes performance by significantly reducing gas consumption during periodic updates (triggered by the CRON module's BeginBlocker). The approach ensures the cost-intensive write operations during periodic updates remain minimal, prioritizing contract efficiency and economic operation. The obvious downside is that reading becomes more expensive, but it doesn't seem to be possible to optimise for both reading and writing, and we chose to spend less gas in the CRON schedule.

## Migration

Supports smooth migration via the provided `MigrateMsg`. Ensure migration adheres to semantic versioning principles.

## Storage

- **Config**: Single-instance configuration.
- **Last Index**: Tracks the next write position for each currency pair.
- **Price History**: Ring-buffer storing historical price data.

## Error Handling

Errors returned:

- `Unauthorized`: Attempted by an unauthorized address.
- `TooSoon`: Updates attempted too frequently.

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE) for details.

