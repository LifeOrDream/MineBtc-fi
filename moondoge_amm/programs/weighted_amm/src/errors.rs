use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Invalid fee rate")]
    InvalidFeeRate,
    
    #[msg("Invalid weight configuration")]
    InvalidWeights,
    
    #[msg("Pool not open yet")]
    PoolNotOpen,
    
    #[msg("Pool is disabled")]
    PoolDisabled,
    
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Division by zero")]
    DivisionByZero,
    
    #[msg("Invalid token amount")]
    InvalidTokenAmount,
    
    #[msg("Unauthorized")]
    Unauthorized,
    
    #[msg("Invalid pool status")]
    InvalidPoolStatus,
    
    #[msg("Pool already initialized")]
    PoolAlreadyInitialized,
    
    #[msg("Invalid configuration parameter")]
    InvalidConfigParameter,
    
    #[msg("Insufficient LP tokens")]
    InsufficientLpTokens,
    
    #[msg("Maximum tokens exceeded")]
    MaximumTokensExceeded,
    
    #[msg("Minimum tokens not met")]
    MinimumTokensNotMet,
    
    #[msg("Invalid swap ratio")]
    InvalidSwapRatio,
    
    #[msg("Pool creation fee not paid")]
    PoolCreationFeeNotPaid,
}
