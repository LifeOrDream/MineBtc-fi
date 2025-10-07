use anchor_lang::prelude::*;

#[event]
pub struct AmmConfigCreated {
    pub index: u16,
    pub owner: Pubkey,
    pub trade_fee_rate: u64,
    pub protocol_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub create_pool_fee: u64,
}

#[event]
pub struct AmmConfigUpdated {
    pub index: u16,
    pub param: u8,
    pub value: u64,
}

#[event]
pub struct PoolCreated {
    pub pool_state: Pubkey,
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub token_0_weight: u64,
    pub token_1_weight: u64,
    pub init_amount_0: u64,
    pub init_amount_1: u64,
    pub lp_amount: u64,
}

#[event]
pub struct LiquidityDeposited {
    pub pool_state: Pubkey,
    pub user: Pubkey,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub lp_amount: u64,
}

#[event]
pub struct LiquidityWithdrawn {
    pub pool_state: Pubkey,
    pub user: Pubkey,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub lp_amount: u64,
}

#[event]
pub struct Swapped {
    pub pool_state: Pubkey,
    pub user: Pubkey,
    pub token_in_mint: Pubkey,
    pub token_out_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
}

#[event]
pub struct ProtocolFeeCollected {
    pub pool_state: Pubkey,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub recipient: Pubkey,
}

#[event]
pub struct FundFeeCollected {
    pub pool_state: Pubkey,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
    pub recipient: Pubkey,
}

#[event]
pub struct PoolStatusUpdated {
    pub pool_state: Pubkey,
    pub old_status: u8,
    pub new_status: u8,
}
