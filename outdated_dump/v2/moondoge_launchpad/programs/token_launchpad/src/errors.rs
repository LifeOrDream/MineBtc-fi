use anchor_lang::prelude::*;

#[error_code]
pub enum LaunchpadError {
    #[msg("Insufficient SOL amount")]
    InsufficientSolAmount,
    
    #[msg("Insufficient token amount")]
    InsufficientTokenAmount,
    
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    
    #[msg("Bonding curve is complete")]
    BondingCurveComplete,
    
    #[msg("Bonding curve is not complete")]
    BondingCurveNotComplete,
    
    #[msg("Token already migrated")]
    TokenAlreadyMigrated,
    
    #[msg("Cannot migrate incomplete bonding curve")]
    CannotMigrateIncomplete,
    
    #[msg("Invalid fee percentage")]
    InvalidFeePercentage,
    
    #[msg("Invalid reserves")]
    InvalidReserves,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Division by zero")]
    DivisionByZero,
    
    #[msg("Invalid token metadata")]
    InvalidTokenMetadata,
    
    #[msg("Unauthorized")]
    Unauthorized,
    
    #[msg("Invalid curve parameters")]
    InvalidCurveParameters,
    
    #[msg("Token creation fee not paid")]
    TokenCreationFeeNotPaid,
    
    #[msg("Invalid weight parameters")]
    InvalidWeightParameters,
}
