use anchor_lang::prelude::*;

#[account]
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub fee_recipient: Pubkey,
    pub platform_fee_bps: u16,      // Platform fee in basis points (e.g., 100 = 1%)
    pub token_creation_fee: u64,     // Fee in lamports to create a token
    pub migration_fee_bps: u16,      // Fee in basis points for migration to AMM
    pub total_tokens_created: u64,
    pub total_volume_sol: u64,
    pub total_fees_collected: u64,
    pub bump: u8,
}

impl GlobalConfig {
    pub const SPACE: usize = 8 + // discriminator
        32 + // authority
        32 + // fee_recipient
        2 +  // platform_fee_bps
        8 +  // token_creation_fee
        2 +  // migration_fee_bps
        8 +  // total_tokens_created
        8 +  // total_volume_sol
        8 +  // total_fees_collected
        1;   // bump
}

#[account]
pub struct BondingCurve {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub total_supply: u64,
    pub complete: bool,
    pub migrated: bool,
    pub amm_pool: Option<Pubkey>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub migrated_at: Option<i64>,
    pub bump: u8,
}

impl BondingCurve {
    pub const SPACE: usize = 8 + // discriminator
        32 + // mint
        32 + // creator
        8 +  // virtual_sol_reserves
        8 +  // virtual_token_reserves
        8 +  // real_sol_reserves
        8 +  // real_token_reserves
        8 +  // total_supply
        1 +  // complete
        1 +  // migrated
        33 + // amm_pool (Option<Pubkey>)
        8 +  // created_at
        9 +  // completed_at (Option<i64>)
        9 +  // migrated_at (Option<i64>)
        1;   // bump

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn can_migrate(&self) -> bool {
        self.complete && !self.migrated
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum CurveType {
    Linear,
    Exponential,
    Logarithmic,
}

#[account]
pub struct TokenMetadata {
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub curve_type: CurveType,
    pub bump: u8,
}

impl TokenMetadata {
    pub const SPACE: usize = 8 + // discriminator
        32 + // mint
        4 + 32 + // name (String with max 32 chars)
        4 + 10 + // symbol (String with max 10 chars)
        4 + 200 + // uri (String with max 200 chars)
        1 + // curve_type
        1;  // bump
}
