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
    pub egg_treasury_amt: u64,
    pub dev_earnings_amount: u64
}
 
#[event]
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

 
#[event]
pub struct FactionsAdded {
    pub authority: Pubkey,
    pub factions: Vec<String>,
    pub total_factions: u8, // Changed from usize to u8 for Anchor event compatibility
}
 
// ------------------------------
// Dynamic distribution events
// ------------------------------
 
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
    pub player: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub dna: [u8; 32],
    pub multiplier: u32,
    pub initial_power: u32,
    pub faction_id: u8, // Faction/country the egg belongs to
    pub price: u64,
    pub ticket_tier: u64,
    pub ticket_count: u64,
}

#[event]
pub struct DragonEggCollectionCreated {
    pub collection: Pubkey,
    pub update_authority: Pubkey,
    pub name: String,
    pub uri: String,
}

/// Event emitted when a Dragon Egg is staked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct DragonEggStaked {
    /// User who staked the egg
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Egg mint address
    pub egg_mint: Pubkey,
    /// Faction ID the egg belongs to
    pub faction_id: u8,
    /// Egg metadata account address
    pub egg_metadata_account: Pubkey,
    /// Player's current multiplier after staking
    pub player_multiplier: u16,
    /// Player's current DBTC hashpower after staking
    pub dbtc_hashpower: u64,
    /// Player's current LP hashpower after staking
    pub lp_hashpower: u64,
    /// Timestamp of the staking action
    pub timestamp: i64,
}

/// Event emitted when a Dragon Egg is unstaked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct DragonEggUnstaked {
    /// User who unstaked the egg
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Egg mint address
    pub egg_mint: Pubkey,
    /// Egg metadata account address
    pub egg_metadata_account: Pubkey,
    /// Faction ID the egg belongs to
    pub faction_id: u8,
    /// Player's current multiplier after unstaking
    pub egg_multiplier: u32,
    /// Player's current DBTC hashpower after unstaking
    pub dbtc_hashpower: u64,
    /// Player's current LP hashpower after unstaking
    pub lp_hashpower: u64,
    /// Timestamp of the unstaking action
    pub timestamp: i64,
}

/// Event emitted when power is claimed and distributed to a staked egg
/// Tracks power distribution for indexing (emitted per egg)
#[event]
pub struct DragonEggPowerClaimed {
    /// Egg mint address that received power
    pub egg_mint: Pubkey,
    /// Power added to this egg
    pub to_add: u64,
    /// New total power for this egg after distribution
    pub power: u32,
    /// Timestamp of the power claim action
    pub timestamp: i64,
}
 
// ========================================================================================
// =============================== STAKING EVENTS ========================================
// ========================================================================================

#[event]
pub struct DogeBtcStaked {
    pub owner: Pubkey,
    pub faction_id: u8,
    pub amount: u64,
    pub burn_amount: u64,
    pub actual_amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
    pub hashpower_contribution: u64,
    pub position_index: u8,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
}

#[event]
pub struct DogeBtcUnstaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub weighted_amount: u64,
    pub hashpower_contribution: u64,
    pub early_withdrawal: bool,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
}

#[event]
pub struct LiquidityStaked {
    pub owner: Pubkey,
    pub faction_id: u8,
    pub amount: u64,
    pub lockup_duration: u64,
    pub multiplier: u16,
    pub weighted_amount: u64,
    pub hashpower_contribution: u64,
    pub position_index: u8,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
}

#[event]
pub struct LiquidityUnstaked {
    pub owner: Pubkey,
    pub position_index: u8,
    pub amount: u64,
    pub weighted_amount: u64,
    pub hashpower_contribution: u64,
    pub early_withdrawal: bool,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
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

// ========================================================================================
// =============================== USER PARTICIPATION EVENTS =============================
// ========================================================================================

/// Event emitted when a player initializes their account
#[event]
pub struct PlayerInitialized {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub referral_code: Option<Pubkey>,
    pub timestamp: i64,
}

/// Event emitted when a user changes their faction
#[event]
pub struct FactionChanged {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub new_faction_id: u8,
    pub timestamp: i64,
}

/// Event emitted when a user joins a round with a single bet
#[event]
pub struct RoundJoined {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub round_id: u64,
    pub target_block: u8,
    pub net_amount: u64,
    pub fee_amount: u64,
    pub points_amount: u64,
    pub used_ticket: bool,
    pub ticket_type_index: Option<u8>,
    pub timestamp: i64,
}

/// Event emitted when a user joins a round with multiple bets (batch)
#[event]
pub struct RoundJoinedBatch {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub round_id: u64,
    pub num_bets: u8,
    pub target_blocks: Vec<u8>,
    pub net_amounts: Vec<u64>,
    pub fee_amounts: Vec<u64>,
    pub points_amounts: Vec<u64>,
    pub used_ticket: bool,
    pub ticket_type_index: Option<u8>,
    pub timestamp: i64,
}

/// Event emitted when a user claims rewards for a round
#[event]
pub struct RoundRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub round_id: u64,
    pub sol_reward: u64,
    pub dbtc_reward: u64,
    pub timestamp: i64,
}

// ========================================================================================
// =============================== AUTOMINER EVENTS ======================================
// ========================================================================================

/// Event emitted when an autominer vault is initialized
#[event]
pub struct AutominerInitialized {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub autominer_vault: Pubkey,
    pub sol_per_round: u64,
    pub num_rounds: u32,
    pub bets_per_round: u64,
    pub bet_size_per_bet: u64,
    pub has_blocks_config: bool,
    pub has_factions_config: bool,
    pub timestamp: i64,
}

/// Event emitted when autominer executes bets for a round
#[event]
pub struct AutominerBetExecuted {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub autominer_vault: Pubkey,
    pub round_id: u64,
    pub target_blocks: Vec<u8>,
    pub net_amounts: Vec<u64>,
    pub fee_amounts: Vec<u64>,
    pub points_amounts: Vec<u64>,
    pub caller: Pubkey,
    pub caller_compensation: u64,
    pub rounds_remaining: u32,
    pub vault_closed: bool,
    pub timestamp: i64,
}

/// Event emitted when autominer is stopped
#[event]
pub struct AutominerStopped {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub autominer_vault: Pubkey,
    pub rounds_remaining: u32,
    pub refund_amount: u64,
    pub timestamp: i64,
}
