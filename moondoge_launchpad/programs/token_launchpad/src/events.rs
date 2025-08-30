use anchor_lang::prelude::*;

#[event]
pub struct TokenCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub initial_virtual_sol_reserves: u64,
    pub initial_virtual_token_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub timestamp: i64,
}

#[event]
pub struct TokenPurchased {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub timestamp: i64,
}

#[event]
pub struct TokenSold {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub token_amount: u64,
    pub sol_amount: u64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub timestamp: i64,
}

#[event]
pub struct BondingCurveCompleted {
    pub mint: Pubkey,
    pub final_sol_reserves: u64,
    pub final_token_reserves: u64,
    pub total_supply: u64,
    pub timestamp: i64,
}

#[event]
pub struct TokenMigrated {
    pub mint: Pubkey,
    pub amm_pool: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub weight_token: u64,
    pub weight_sol: u64,
    pub lp_tokens_burned: u64,
    pub timestamp: i64,
}

#[event]
pub struct FeesWithdrawn {
    pub recipient: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}
