use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Unauthorized")]
    Unauthorized {},
    #[error("Update called too soon. Wait until block {expected}")]
    TooSoon { expected: u64 },
    #[error("{0}")]
    Std(#[from] StdError),
}
