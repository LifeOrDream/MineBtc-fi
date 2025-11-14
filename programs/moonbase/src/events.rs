use anchor_lang::prelude::*;

// ------------------------------
// User management events
// ------------------------------
 

#[event]
pub struct ReferralRewardsAdded {
    pub referrer: Pubkey,
    pub referred_user: Pubkey,
    pub referral_rewards_account: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ReferralRewardsClaimed {
    pub owner: Pubkey,
    pub amount: u64,
}
 
  

#[event]
pub struct SolFeesWithdrawn {
    pub available_solana: u64,
    pub buyback_amount: u64,
    pub dev_earnings_amount: u64
}
 
pub struct MiningTokenVaultSet {
    /// The authority that set the token vault
    pub authority: Pubkey,
    /// The token vault address
    pub token_vault: Pubkey,
    /// The token vault authority address
    pub token_vault_authority: Pubkey,
    /// Timestamp of when mining started
    pub mining_start_timestamp: u64,
}

// Global Config events
#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub sol_claimer: Pubkey,
}

#[event]
pub struct FactionsAdded {
    pub authority: Pubkey,
    pub factions: Vec<String>,
    pub total_factions: u8, // Changed from usize to u8 for Anchor event compatibility
}

#[event]
pub struct DogeBtcTokensClaimed {
    pub owner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct MiningRewardsProcessed {
    pub owner: Pubkey,
    pub hashpower: u64,
    pub tokens_earned: u64,
}

#[event]
pub struct MiningHalveningOccurred {
    pub slot: u64,
    pub new_rate: u64,
    pub next_halvening_slot: u64,
}

#[event]
pub struct UserElectricityUpdated {
    pub user: Pubkey,
    pub previous_amount: u64,
    pub new_amount: u64,
    pub is_increase: bool,
}

// ------------------------------
// Dynamic distribution events
// ------------------------------

#[event]
pub struct RaydiumPoolSet {
    pub authority: Pubkey,
    pub pool_state: Pubkey,
}

/// Price snapshot taken every 30 minutes (1-8 snapshots per 4-hour cycle)
#[event]
pub struct PriceSnapshotTaken {
    pub snapshot_number: u8,           // 1-8 (which snapshot in the cycle)
    pub sol_swapped: u64,              // SOL amount swapped (lamports)
    pub dbtc_received: u64,           // DOGE_BTC received from swap (6 decimals)
    pub current_price: u64,           // Calculated price (9 decimals: SOL per DOGE_BTC)
    pub weighted_avg_price: u64,      // Weighted average price so far (9 decimals)
    pub sol_earnmarked_for_pol: u64,  // SOL earnmarked for POL this snapshot (lamports)
    pub total_pol_balance: u64,       // Total SOL earnmarked for POL (lamports)
    pub price_history_count: u8,      // Number of entries in price history (1-8)
    pub timestamp: i64,               // Unix timestamp
}

/// Liquidity added to Raydium pool (before burning LP tokens)
#[event]
pub struct LiquidityAdded {
    pub sol_amount: u64,              // SOL added to pool (lamports)
    pub dbtc_amount: u64,             // DOGE_BTC added to pool (6 decimals)
    pub lp_tokens_minted: u64,        // LP tokens minted (6 decimals)
    pub lp_token_price: u64,          // LP token price in SOL (9 decimals)
    pub timestamp: i64,               // Unix timestamp
}

#[event]
pub struct DistributionRateUpdated {
    pub old_rate: u64,
    pub new_rate: u64,
    pub price_change_pct: i32,
    pub current_price: u64,
    pub avg_price_4h: u64,
    pub track_price: u64,
    pub recent_price: u64,
    pub rate_changed: bool,
    pub sol_received: u64,
    pub price_history_count: u8,      // Number of price snapshots used (should be 8)
    pub sol_for_pol_used: u64,        // SOL used for POL (lamports)
    pub sol_for_pol_remaining: u64,  // SOL remaining for POL (lamports)
    pub lp_tokens_burned: u64,        // LP tokens burned (0 if no LP added)
    pub timestamp: i64,
}

#[event]
pub struct LpTokensBurned {
    pub lp_tokens_burned: u64,
    pub total_lp_burnt: u64,
    pub dbtc_amount_added: u64,
    pub sol_amount_added: u64,
    pub sol_vault_balance: u64,       // SOL vault balance after LP addition (lamports)
    pub dbtc_vault_balance: u64,     // DOGE_BTC vault balance after LP addition (6 decimals)
    pub lp_supply: u64,               // LP token supply after burn (6 decimals)
    pub lp_token_price: u64,          // LP token price in SOL (9 decimals)
    pub timestamp: i64,
}
 
 

// ========================================================================================
// =============================== DRAGON EGG NFT EVENTS =================================
// ========================================================================================

#[event]
pub struct DragonEggMinted {
    pub egg_metadata_account: Pubkey,
    pub dragon_egg_asset_signer: Pubkey,
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub dna: [u8; 32],
    pub multiplier: u32,
    pub initial_power: u32,
    pub faction_id: u8, // Faction/country the egg belongs to
}

#[event]
pub struct DragonEggCollectionCreated {
    pub collection: Pubkey,
    pub update_authority: Pubkey,
    pub name: String,
    pub uri: String,
}
 
// ========================================================================================
// =============================== STAKING EVENTS ========================================
// ========================================================================================

#[event]
pub struct DogeBtcStaked {
    pub owner: Pubkey,
    pub amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
    pub weighted_amount: u64,
    pub total_hashpower_contribution: u64,
    pub position_index: u8,
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
pub struct LiquidityStaked {
    pub owner: Pubkey,
    pub amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
    pub weighted_amount: u64,
    pub total_hashpower_contribution: u64,
    pub position_index: u8,
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
pub struct EmergencyWithdrawal {
    pub owner: Pubkey,
    pub position_index: u8,
    pub original_amount: u64,
    pub penalty_amount: u64,
    pub returned_amount: u64,
    pub penalty_tax_pct: u64,
    pub timestamp: i64,
}
