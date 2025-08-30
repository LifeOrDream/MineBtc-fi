use anchor_lang::prelude::*;

#[account]
pub struct AmmConfig {
    pub bump: u8,
    pub index: u16,
    pub owner: Pubkey,
    pub protocol_fee_rate: u64,
    pub trade_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub create_pool_fee: u64,
    pub fund_owner: Pubkey,
    pub padding: [u64; 16],
}

impl AmmConfig {
    pub const SPACE: usize = 8 + // discriminator
        1 +  // bump
        2 +  // index
        32 + // owner
        8 +  // protocol_fee_rate
        8 +  // trade_fee_rate
        8 +  // fund_fee_rate
        8 +  // create_pool_fee
        32 + // fund_owner
        128; // padding (16 * 8)
}

#[account]
pub struct PoolState {
    pub amm_config: Pubkey,
    pub pool_creator: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    
    pub token_0_protocol_fee: u64,
    pub token_1_protocol_fee: u64,
    pub token_0_fund_fee: u64,
    pub token_1_fund_fee: u64,
    
    pub open_time: u64,
    pub recent_epoch: u64,
    
    // Weighted pool specific fields
    pub token_0_weight: u64,
    pub token_1_weight: u64,
    pub total_weight: u64,
    
    // Pool status
    pub status: u8,
    pub bump: u8,
    
    // Padding for future upgrades
    pub padding: [u64; 32],
}

impl PoolState {
    pub const SPACE: usize = 8 + // discriminator
        32 + // amm_config
        32 + // pool_creator
        32 + // token_0_vault
        32 + // token_1_vault
        32 + // lp_mint
        32 + // token_0_mint
        32 + // token_1_mint
        8 +  // token_0_protocol_fee
        8 +  // token_1_protocol_fee
        8 +  // token_0_fund_fee
        8 +  // token_1_fund_fee
        8 +  // open_time
        8 +  // recent_epoch
        8 +  // token_0_weight
        8 +  // token_1_weight
        8 +  // total_weight
        1 +  // status
        1 +  // bump
        256; // padding (32 * 8)

    pub fn is_pool_open(&self) -> bool {
        let current_time = Clock::get().unwrap().unix_timestamp as u64;
        current_time >= self.open_time
    }

    pub fn is_pool_enabled(&self) -> bool {
        self.status == PoolStatus::Enabled as u8
    }

    pub fn get_normalized_weight_0(&self) -> u128 {
        (self.token_0_weight as u128 * PRECISION) / self.total_weight as u128
    }

    pub fn get_normalized_weight_1(&self) -> u128 {
        (self.token_1_weight as u128 * PRECISION) / self.total_weight as u128
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PoolStatus {
    Uninitialized,
    Enabled,
    Disabled,
    RemoveLiquidityOnly,
}

// Constants for calculations
pub const PRECISION: u128 = 1_000_000_000_000_000_000; // 18 decimal precision
pub const MIN_WEIGHT: u64 = 1;  // 1% minimum weight
pub const MAX_WEIGHT: u64 = 99; // 99% maximum weight
