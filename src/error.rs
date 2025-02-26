use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Unauthorized")]
    Unauthorized {},
    #[error("Price not available for {symbol}/{quote}")]
    PriceNotAvailable { symbol: String, quote: String },
    #[error("Price is too old for {symbol}/{quote}. Maximum allowed blocks: {max_blocks}")]
    PriceTooOld {
        symbol: String,
        quote: String,
        max_blocks: u64,
    },
    #[error("Price is nil for {symbol}/{quote}")]
    PriceIsNil { symbol: String, quote: String },
    #[error("Update called too soon. Wait until block {expected}")]
    TooSoon { expected: u64 },
    #[error("{0}")]
    Std(#[from] StdError),
}
