use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The authority is invalid")]
    InvalidAuthority,

    #[msg("User not authorized to perform this action")]
    Unauthorized,
    
    #[msg("Invalid allocation percentages - must add up to 100%")]
    InvalidAllocationPercentages,
    
    #[msg("Invalid lockup period specified")]
    InvalidLockupPeriod,
    
    #[msg("Invalid multiplier specified")]
    InvalidMultiplier,

    #[msg("Maximum positions (7) reached")]
    MaxPositionsReached,

    #[msg("Invalid program owner")]
    InvalidProgramOwner,
    
    #[msg("Invalid distribution period")]
    InvalidDistributionPeriod,
    
    #[msg("Distribution period has not elapsed yet")]
    DistributionPeriodNotElapsed,
    
    #[msg("Insufficient funds in treasury")]
    InsufficientTreasuryFunds,
    
    #[msg("Amount overflow")]
    AmountOverflow,
    
    #[msg("Invalid staking amount")]
    InvalidStakingAmount,
    
    #[msg("Lockup period not ended yet")]
    LockupPeriodNotEnded,
    
    #[msg("Position not found")]
    PositionNotFound,
    
    #[msg("Invalid position index")]
    InvalidPositionIndex,
    
    #[msg("User has no global position")]
    NoGlobalPosition,
    
    #[msg("Token account does not have enough tokens")]
    InsufficientTokenBalance,
    
    #[msg("Invalid owner")]
    InvalidOwner,
    
    #[msg("Staking position already exists at this index")]
    PositionAlreadyExists,
    
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    
    #[msg("Position still locked")]
    PositionStillLocked,

    #[msg("No rewards available to claim")]
    NoRewardsToClaim,

    #[msg("Insufficient SOL balance in reward vault")]
    InsufficientVaultBalance,
    
    #[msg("Invalid token owner")]
    InvalidTokenOwner,
    
    #[msg("Insufficient funds")]
    InsufficientFunds,
    
    #[msg("Invalid moonbase init type")]
    InvalidInitType,
    
    #[msg("Invalid moonbase account")]
    InvalidMoonbaseAccount,
    
    #[msg("SOL distribution is currently disabled - will be enabled by admin after launch period")]
    SolDistributionDisabled,
}