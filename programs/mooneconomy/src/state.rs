use anchor_lang::prelude::*;

pub const ME_CONFIG_SEED: &[u8] = b"global_config";

pub const DOGE_BTC_VAULT_SEED: &[u8] = b"dogebtc_vault";
pub const LIQUIDITY_VAULT_SEED: &[u8] = b"liquidity_vault";

pub const DBTC_SOL_VAULT_SEED: &[u8] = b"dogewifbtc-sol-vault";
pub const LP_SOL_VAULT_SEED: &[u8] = b"lp-sol-vault";
pub const DBTC_STAKER_REWARD_VAULT_SEED: &[u8] = b"dbtc-staker-reward-vault";
pub const DBTC_STAKER_REWARD_VAULT_AUTHORITY_SEED: &[u8] = b"dbtc-staker-reward-vault-authority";

pub const DBTC_CUSTODIAN_SEED: &[u8] = b"dogewifbtc-custodian";
pub const DBTC_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"dogewifbtc-custodian-authority";

pub const LIQUIDITY_CUSTODIAN_SEED: &[u8] = b"liquidity-custodian";
pub const LIQUIDITY_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"liquidity-custodian-authority";

pub const DEV_EARNINGS_SEED: &[u8] = b"dev_earnings_collector";
pub const FEE_COLLECTOR_SEED: &[u8] = b"fee_collector";

pub const USER_ELECTRICITY_SEED: &[u8] = b"user-electricity";
pub const DBTC_POSITION_SEED: &[u8] = b"dogewifbtc-position";
pub const LP_POSITION_SEED: &[u8] = b"liquidity-position";

/// ------------ CONSTANTS ------------

pub const DAY_IN_SECONDS: u64 = 86400;
pub const BURN_TAX_PERCENTAGE: u64 = 1; // 1% burn tax on transfers

pub const MAX_ALLOWED_POSITIONS: u8 = 7;
pub const EMERGENCY_WITHDRAWAL_PENALTY_PCT: u8 = 10;

pub const PRECISION_FACTOR: u128 = 1_000_000;
pub const M_HUNDRED: u64 = 100;

/// ------------ GLOBAL CONFIG ------------

/// Global configuration for the Moon Economy program
#[account]
pub struct GlobalConfig {
    /// Authority that can update config parameters
    pub authority: Pubkey,
    /// Address which can withdraw dev earnings from the program
    pub dev_address: Pubkey,

    /// PDA account that can withdraw collected SOL from moon-economy program
    pub fee_collector: Pubkey,

    /// Minimum lockup period in days
    pub min_lockup_days: u64,
    /// Maximum lockup period in days
    pub max_lockup_days: u64,

    /// Base multiplier (100 = 1x)
    pub base_multiplier: u16,
    /// Maximum multiplier for longest lockup (e.g., 900 = 9x for 3 years)
    pub max_multiplier: u16,

    /// Distribution percentages (out of 100)
    /// Percentage of SOL distributed to DogeBtc stakers
    pub dogebtc_allocation: u8,
    /// Percentage of SOL distributed to LP token stakers
    pub liquidity_allocation: u8,

    /// Last claim slot
    pub last_claim_slot: u64,

    /// Electricity per weighted SOL staked (used for both dBTC and LP staking)
    pub hashpower_per_weighted_sol: u64,

    /// Current DOGE_BTC to SOL price (9-decimal precision, same as MoonBase)
    /// Used for electricity calculations when MoonBase price is 0 or unavailable
    pub dbtc_sol_price: u64,

    /// Bump for PDA derivation
    pub bump: u8,
}

// For GlobalConfig
impl GlobalConfig {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // dev_address
        32 + // fee_collector
        8 +  // min_lockup_days
        8 +  // max_lockup_days
        2 +  // base_multiplier
        2 +  // max_multiplier
        1 +  // dogebtc_allocation
        1 +  // liquidity_allocation
        8 +  // last_claim_slot
        8 +  // hashpower_per_weighted_sol
        1 +  // sol_distribution_enabled
        8 +  // dbtc_sol_price (u64)
        1; // bump
}

/// ------------ VAULTS :: MOON-DOGE and LP TOKEN VAULTs------------

/// DogeBtc Vault configuration and state
#[account]
pub struct DogeBtcVault {
    /// Authority that can update parameters
    pub authority: Pubkey,
    /// PDA account that holds SOL to be distributed to DogeBtc stakers
    pub dbtc_sol_vault: Pubkey,
    /// PDA account that holds DogeBtc tokens to be distributed to stakers
    pub dbtc_staker_reward_vault: Pubkey,

    /// Token mint for DogeBtc
    pub dbtc_mint: Pubkey,
    /// Custodian that holds the staked tokens
    pub dbtc_custodian: Pubkey,

    /// Total DogeBtc tokens locked in the vault
    pub dbtc_locked: u64,
    /// Total weighted DogeBtc points (including time multipliers)
    pub weighted_dbtc_locked: u64,

    /// Accumulated SOL per weighted DogeBtc point (precision factor applied)
    pub accumulated_sol_per_point: u128,
    /// Accumulated DogeBtc per weighted DogeBtc point (precision factor applied)
    pub accumulated_dbtc_per_point: u128,

    /// Total SOL distributed to DogeBtc stakers
    pub total_sol_distributed: u64,
    /// Total DogeBtc distributed to stakers
    pub total_dbtc_distributed: u64,

    /// Emergency withdrawal tax percentage (0-100)
    pub emergency_tax: u8,

    /// Bump for PDA derivation
    pub bump: u8,
}

// For DogeBtcVault
impl DogeBtcVault {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // dbtc_sol_vault
        32 + // dbtc_staker_reward_vault
        32 + // dbtc_mint
        32 + // dbtc_custodian
        8 +  // dbtc_locked
        8 +  // weighted_dbtc_locked
        16 + // accumulated_sol_per_point
        16 + // accumulated_dbtc_per_point
        8 +  // total_sol_distributed
        1 +  // emergency_tax
        1; // bump
}

/// Liquidity Pool Vault configuration and state
#[account]
pub struct LiquidityVault {
    /// Authority that can update parameters
    pub authority: Pubkey,
    /// PDA account that holds SOL to be distributed to LP token stakers
    pub liquidity_sol_vault: Pubkey,

    /// Token mint for LP tokens
    pub lp_token_mint: Pubkey,
    /// Custodian that holds the staked LP tokens
    pub liquidity_custodian: Pubkey,

    /// Total LP tokens locked in the vault
    pub lp_tokens_locked: u64,
    /// Total weighted LP points (including time multipliers)
    pub weighted_lp_locked: u64,

    /// Accumulated SOL per weighted DogeBtc point (precision factor applied)
    pub accumulated_sol_per_point: u128,
    /// Accumulated DogeBtc per weighted DogeBtc point (precision factor applied)
    pub accumulated_dbtc_per_point: u128,

    /// Total SOL distributed to DogeBtc stakers
    pub total_sol_distributed: u64,
    /// Total DogeBtc distributed to stakers
    pub total_dbtc_distributed: u64,

    /// Emergency withdrawal tax percentage (0-100)
    pub emergency_tax: u8,

    /// Bump for PDA derivation
    pub bump: u8,
}

impl LiquidityVault {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // liquidity_sol_vault
        32 + // lp_token_mint
        32 + // liquidity_custodian
        8 +  // lp_tokens_locked
        8 +  // weighted_lp_locked
        16 + // accumulated_sol_per_point
        8 +  // total_sol_distributed
        1 +  // emergency_tax
        1; // bump
}

/// ------------ USER POSITIONS :: MOON-DOGE and LP TOKEN POSITIONS------------

/// User DogeBtc staking position
#[account]
pub struct UserMoonElectricity {
    /// User's wallet address
    pub owner: Pubkey,

    /// Total DogeBtc staking stats
    pub total_moondoge_staked: u64,
    pub total_weighted_dogebtc: u64,
    pub active_moondoge_positions: u8, // Max 7

    /// Total Liquidity staking stats
    pub total_lp_tokens_staked: u64,
    pub total_weighted_lp: u64,
    pub active_lp_positions: u8, // Max 7

    /// Electricity stats
    pub total_hashpower_contribution: u64,

    /// SOL rewards tracking
    pub dbtc_dbtc_reward_debt: u128, // Last checkpoint for DogeBtc SOL rewards
    pub dbtc_sol_reward_debt: u128, // Last checkpoint for DogeBtc SOL rewards
    
    pub lp_sol_reward_debt: u128, // Last checkpoint for LP SOL rewards
    pub lp_dbtc_reward_debt: u128, // Last checkpoint for LP DogeBtc rewards

    /// Position indices tracking (max 7 elements each)
    pub moondoge_position_indices: Vec<u8>, // Store actual indices
    pub lp_position_indices: Vec<u8>, // Store actual indices

    pub bump: u8,
}

impl UserMoonElectricity {
    pub const LEN: usize = 8 +  // discriminator
            32 + // owner
            8 +  // total_moondoge_staked
            8 +  // total_weighted_dogebtc
            1 +  // active_moondoge_positions
            8 +  // total_lp_tokens_staked
            8 +  // total_weighted_lp
            1 +  // active_lp_positions
            8 +  // total_hashpower_contribution
            8 +  // free_electricity
            16 + // moondoge_reward_debt
            16 + // lp_reward_debt
            8 +  // pending_dogebtc_rewards
            8 +  // pending_lp_rewards
            8 +  // total_sol_claimed
            16 + // moondoge_dbtc_reward_debt
            8 +  // pending_moondoge_dbtc_rewards
            4 + 7 +  // vec length + max 7 indices for moondoge
            4 + 7 +  // vec length + max 7 indices for lp
            1; // bump
}

/// Individual DogeBtc staking position
#[account]
pub struct DogeBtcPosition {
    pub position_index: u8,

    /// Staking details
    pub staked_amount: u64,
    pub weighted_amount: u64,
    pub start_timestamp: i64,
    pub lockup_end_timestamp: i64,
    pub lockup_duration: u64, // in days
    pub multiplier: u16,      // 100 = 1x
    pub hashpower_contribution: u64,
    pub bump: u8,
}

impl DogeBtcPosition {
    pub const LEN: usize = 8 +  // discriminator
        1 +  // position_index
        8 +  // staked_amount
        8 +  // weighted_amount
        8 +  // start_timestamp
        8 +  // lockup_end_timestamp
        8 +  // lockup_duration
        2 +  // multiplier
        8 +  // hashpower_contribution
        1; // bump
}

/// Individual Liquidity staking position
#[account]
pub struct LiquidityPosition {
    pub position_index: u8, // 0-6

    /// Staking details
    pub staked_amount: u64,
    pub weighted_amount: u64,
    pub start_timestamp: i64,
    pub lockup_end_timestamp: i64,
    pub lockup_duration: u64, // in days
    pub multiplier: u16,      // 100 = 1x
    pub hashpower_contribution: u64,
    pub bump: u8,
}

impl LiquidityPosition {
    pub const LEN: usize = DogeBtcPosition::LEN; // Same structure
}
