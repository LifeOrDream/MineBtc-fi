use anchor_lang::prelude::*;

// ------------------------------
// Program initialization events
// ------------------------------

#[event]
pub struct ProgramInitialized {
    pub dev_address: Pubkey,
    pub dev_earnings_collector: Pubkey,
    pub fee_collector: Pubkey,
    pub dogebtc_allocation: u8,
    pub liquidity_allocation: u8,
}

#[event]
pub struct MDogeVaultsInitialized {
    pub dbtc_sol_vault: Pubkey,
    pub dbtc_mint: Pubkey,
    pub dbtc_custodian: Pubkey,
}

#[event]
pub struct LiquidityVaultsInitialized {
    pub liquidity_sol_vault: Pubkey,
    pub lp_token_mint: Pubkey,
    pub liquidity_custodian: Pubkey,
}

#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub electricity_per_weighted_sol: u64,
    pub dogebtc_allocation: u8,
    pub liquidity_allocation: u8,
}

#[event]
pub struct AdminEarningsWithdrawn {
    pub amount: u64,
}

// ------------------------------
// SOL distribution events
// ------------------------------

#[event]
pub struct SolDistributed {
    pub sol_for_distribution: u64,
    pub sol_for_dbtc_stakers: u64,
    pub sol_for_lp_stakers: u64,
    pub dev_earnings: u64,
    pub timestamp: i64,
}

#[event]
pub struct GameSolFeesWithdrawn {
    pub sol_claimer: Pubkey,
    pub amount: u64,
}

#[event]
pub struct SolFeesWithdrawnEvent {
    pub amount: u64,
    pub claimer: Pubkey,
}

// ------------------------------
// Staking position events
// ------------------------------

#[event]
pub struct DogeBtcStaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub electricity_earned: u64,
    pub weighted_amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
}

#[event]
pub struct DogeBtcStakedEvent {
    pub owner: Pubkey,
    pub amount: u64,
    pub weighted_amount: u64,
    pub lockup_period: i64,
    pub multiplier: u8,
    pub unlock_timestamp: i64,
}

#[event]
pub struct DogeBtcUnstaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub weighted_amount: u64,
    pub early_withdrawal: bool,
}

#[event]
pub struct DogeBtcUnstakedEvent {
    pub owner: Pubkey,
    pub amount: u64,
    pub weighted_amount: u64,
}

// ------------------------------
// Global position events
// ------------------------------

#[event]
pub struct GlobalPositionCreated {
    pub owner: Pubkey,
}

#[event]
pub struct GlobalPositionUpdated {
    pub owner: Pubkey,
    pub total_moondoge_staked: u64,
    pub total_moondoge_weighted: u64,
    pub total_lp_staked: u64,
    pub total_lp_weighted: u64,
}

#[event]
pub struct DogeBtcVaultSolAddedEvent {
    pub amount: u64,
    pub new_accumulated_sol_per_point: u64,
}

#[event]
pub struct LiquidityVaultSolAddedEvent {
    pub amount: u64,
    pub new_accumulated_sol_per_point: u64,
}

#[event]
pub struct EarlyUnstakePenalty {
    pub owner: Pubkey,
    pub position_index: u8,
    pub penalty_amount: u64,
    pub return_amount: u64,
}

#[event]
pub struct LiquidityStaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub electricity_earned: u64,
    pub weighted_amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
}

#[event]
pub struct LiquidityUnstaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub weighted_amount: u64,
    pub early_withdrawal: bool,
}

#[event]
pub struct EarlyLiquidityUnstakePenalty {
    pub owner: Pubkey,
    pub position_index: u8,
    pub penalty_amount: u64,
    pub return_amount: u64,
    pub penalty_tax_pct: u64,
    pub timestamp: i64,
}

#[event]
pub struct PassiveRewardsClaimed {
    pub owner: Pubkey,
    pub moondoge_sol_rewards: u64,
    pub lp_sol_rewards: u64,
    pub moondoge_dbtc_rewards: u64,
}

#[event]
pub struct TotalRewardsClaimed {
    pub owner: Pubkey,
    pub moondoge_amount: u64,
    pub liquidity_amount: u64,
    pub total_amount: u64,
    pub total_claimed: u64,
}

#[event]
pub struct EmergencyWithdrawal {
    pub owner: Pubkey,
    pub position_index: u8,
    pub original_amount: u64,
    pub penalty_amount: u64,
    pub returned_amount: u64,
    pub penalty_tax_pct: u64,
    pub timestamp: i64,
}
