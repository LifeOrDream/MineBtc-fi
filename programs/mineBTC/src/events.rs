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
    pub referrer: Pubkey,
    pub referral_rewards_account: Pubkey,
    pub sol_amount: u64,
    pub minebtc_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SolFeesWithdrawn {
    pub available_solana: u64,
    pub buyback_amount: u64,
    pub doge_treasury_amt: u64,
    pub dev_earnings_amount: u64,
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
pub struct FactionAdded {
    pub authority: Pubkey,
    pub faction_name: String,
    pub faction_id: u8,
    pub faction_key: Pubkey,
}

// ------------------------------
// Dynamic distribution events
// ------------------------------

/// Price snapshot taken every 30 minutes (1-8 snapshots per 4-hour cycle)
#[event]
pub struct PriceSnapshotTaken {
    pub snapshot_number: u8,         // 1-8 (which snapshot in the cycle)
    pub sol_swapped: u64,            // SOL amount swapped (lamports)
    pub minebtc_received: u64,       // MINE_BTC received from swap (6 decimals)
    pub current_price: u64,          // Calculated price (9 decimals: SOL per MINE_BTC)
    pub weighted_avg_price: u64,     // Weighted average price so far (9 decimals)
    pub sol_earnmarked_for_pol: u64, // SOL earnmarked for POL this snapshot (lamports)
    pub total_pol_balance: u64,      // Total SOL earnmarked for POL (lamports)
    pub price_history_count: u8,     // Number of entries in price history (1-8)
    pub timestamp: i64,              // Unix timestamp
}

/// Liquidity added to Raydium pool (before burning LP tokens)
#[event]
pub struct LiquidityAdded {
    pub sol_amount: u64,       // SOL added to pool (lamports)
    pub minebtc_amount: u64,   // MINE_BTC added to pool (6 decimals)
    pub lp_tokens_minted: u64, // LP tokens minted (6 decimals)
    pub lp_token_price: u64,   // LP token price in SOL (9 decimals)
    pub timestamp: i64,        // Unix timestamp
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
    pub timestamp: i64,
}

#[event]
pub struct LpTokensBurned {
    pub lp_tokens_burned: u64,
    pub total_lp_burnt: u64,
    pub minebtc_amount_added: u64,
    pub sol_amount_added: u64,
    pub sol_vault_balance: u64, // SOL vault balance after LP addition (lamports)
    pub minebtc_vault_balance: u64, // MINE_BTC vault balance after LP addition (6 decimals)
    pub lp_token_price: u64,    // LP token price in SOL (9 decimals)
    pub timestamp: i64,
}

// ========================================================================================
// ===============================  DOGE NFT EVENTS =================================
// ========================================================================================

#[event]
pub struct DogeMinted {
    pub doge_metadata_account: Pubkey,
    pub doge_asset_signer: Pubkey,
    pub owner: Pubkey,
    pub player: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub dna: [u8; 32],
    pub multiplier: u32,
    pub accumulated_val: u64,
    pub faction_id: u8, // Faction/country the doge belongs to
    pub price: u64,
    pub ticket_tier: u64,
    pub ticket_count: u64
}

#[event]
pub struct EggCollectionCreated {
    pub collection: Pubkey,
    pub update_authority: Pubkey,
    pub name: String,
    pub uri: String,
}

/// Event emitted when a Doge is staked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct EggStaked {
    /// User who staked the egg
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Doge mint address
    pub doge_mint: Pubkey,
    /// Faction ID the doge belongs to
    pub faction_id: u8,
    /// Doge metadata account address
    pub doge_metadata_account: Pubkey,
    /// Player's current multiplier after staking
    pub player_multiplier: u16,
    /// Player's current MINEBTC hashpower after staking
    pub minebtc_hashpower: u64,
    /// Player's current LP hashpower after staking
    pub lp_hashpower: u64,
    /// Timestamp of the staking action
    pub timestamp: i64,
}

/// Event emitted when a Doge is unstaked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct EggUnstaked {
    /// User who unstaked the egg
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Doge mint address
    pub doge_mint: Pubkey,
    /// Doge metadata account address
    pub doge_metadata_account: Pubkey,
    /// Faction ID the doge belongs to
    pub faction_id: u8,
    /// Player's current multiplier after unstaking
    pub doge_multiplier: u32,
    /// Player's current MINEBTC hashpower after unstaking
    pub minebtc_hashpower: u64,
    /// Player's current LP hashpower after unstaking
    pub lp_hashpower: u64,
    /// Timestamp of the unstaking action
    pub timestamp: i64,
}

/// Event emitted when an doge is sent to heaven (burnt) for rewards
#[event]
pub struct EggSentToHeaven {
    /// Doge mint address that was burnt
    pub doge_mint: Pubkey,
    /// User who sent the doge to heaven
    pub user: Pubkey,
    /// Accumulated value claimed
    pub accumulated_val: u64,
    /// Timestamp of the action
    pub timestamp: i64,
}

// ========================================================================================
// =============================== STAKING EVENTS ========================================
// ========================================================================================

#[event]
pub struct MineBtcStaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub lockup_duration: u64,
    pub hashpower_contribution: u64,
    pub new_sol_rewards: u64,
    pub new_minebtc_rewards: u64,
    pub unrefined_minebtc: u64,
    pub timestamp: i64,
}

#[event]
pub struct MineBtcUnstaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub new_sol_rewards: u64,
    pub new_minebtc_rewards: u64,
    pub unrefined_minebtc: u64,
    pub original_amount: u64,
    pub returned_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityStaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub lockup_duration: u64,
    pub hashpower_contribution: u64,
    pub new_sol_rewards: u64,
    pub new_minebtc_rewards: u64,
    pub unrefined_minebtc: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityUnstaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub new_sol_rewards: u64,
    pub new_minebtc_rewards: u64,
    pub unrefined_minebtc: u64,
    pub original_amount: u64,
    pub returned_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyWithdrawal {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub original_amount: u64,
    pub penalty_amount: u64,
    pub returned_amount: u64,
    pub penalty_tax_pct: u64,
    pub timestamp: i64,
}

/// Event emitted when a user claims SOL rewards from staking
#[event]
pub struct SolRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub sol_amount: u64,
    pub referral_fee: u64,
    pub referrer: Option<Pubkey>,
    pub timestamp: i64,
}

/// Event emitted when a user claims MineBtc token rewards from staking
#[event]
pub struct DbtcRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub minebtc_amount: u64,
    pub refining_fee: u64,
    pub referral_fee: u64,
    pub referrer: Option<Pubkey>,
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

/// Event emitted when bets are placed (single, batch, or autominer)
#[event]
pub struct BetsPlaced {
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

    // Autominer specific (Optional)
    pub is_autominer: bool,
    pub autominer_vault: Option<Pubkey>,
    pub caller: Option<Pubkey>,
    pub caller_compensation: u64,
    pub rounds_remaining: Option<u32>,
    pub vault_closed: Option<bool>,

    pub timestamp: i64,
}

/// Event emitted when a user claims rewards for a round
#[event]
pub struct RoundRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub round_id: u64,
    pub sol_reward: u64,
    pub minebtc_reward: u64,
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

// ========================================================================================
// =============================== GAME ROUND EVENTS =====================================
// ========================================================================================

/// Event emitted when a new round starts
#[event]
pub struct RoundStarted {
    pub round_id: u64,
    pub game_session: Pubkey,
    pub commit_hash: [u8; 32],
    pub block_assignments: [u8; 24], // 24 blocks, each assigned to a faction (0-11)
    pub round_start_timestamp: i64,
    pub round_end_timestamp: i64,
    pub timestamp: i64,
}

/// Event emitted when a round ends (after winner selection and reward calculations)
#[event]
pub struct RoundEnded {
    pub round_id: u64,
    pub game_session: Pubkey,
    pub winning_block: u8, // 0-indexed: 0-23
    pub winning_faction_id: u8,
    pub same_faction_other_block: u8, // 0-indexed: 0-23
    pub total_sol_bets: u64,
    pub total_points_bets: u64,

    pub user_bets_count: Vec<u64>,
    pub block_bet_counts: Vec<u64>,
    pub block_points: Vec<u64>,

    pub minebtc_winner_pool: u64,
    pub minebtc_same_faction_pool: u64,
    pub minebtc_faction_stakers: u64,
    pub minebtc_motherlode: u64,
    pub motherlode_hit: bool,
    pub timestamp: i64,
}

/// Event emitted when faction rewards are distributed for a round
#[event]
pub struct RoundFactionRewardsDistributed {
    pub round_id: u64,
    pub game_session: Pubkey,
    pub winning_faction_id: u8,
    pub sol_stakers_fee: u64,
    pub motherlode_hit: bool,
    pub motherlode_pot_size_on_hit: u64,
    pub timestamp: i64,
}

// ========================================================================================
// =============================== TAX & DISTRIBUTION EVENTS =============================
// ========================================================================================

/// Event emitted when tax is distributed from mint to vaults
#[event]
pub struct TaxDistributed {
    pub total_tax_amount: u64,
    pub nft_floor_sweep_amount: u64,
    pub faction_treasury_amount: u64,
    pub burn_amount: u64,
    pub total_burnt: u64,
    pub timestamp: i64,
}

/// Event emitted when NFT floor sweep funds are withdrawn
#[event]
pub struct NftFloorSweepFundsWithdrawn {
    pub whitelisted_address: Pubkey,
    pub amount: u64,
    pub nft_floor_sweep_vault: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when a new distribution round starts
#[event]
pub struct DistributionRoundStarted {
    pub tax_config: Pubkey,
    pub faction_treasury_balance: u64,
    pub start_timestamp: i64,
    pub timestamp: i64,
}

/// Event emitted when faction leaderboard position is calculated
#[event]
pub struct FactionLeaderboardPositionCalculated {
    pub tax_config: Pubkey,
    pub faction_id: u8,
    pub faction_state: Pubkey,
    pub total_hashpower: u64,
    pub minebtc_hashpower: u64,
    pub lp_hashpower: u64,
    pub rank: u8,
    pub leaderboard_count: u8,
    pub timestamp: i64,
}

/// Event emitted when faction rewards are calculated
#[event]
pub struct FactionRewardsCalculated {
    pub tax_config: Pubkey,
    pub total_treasury: u64,
    pub first_place_faction_id: u8,
    pub first_place_reward: u64,
    pub second_place_faction_id: u8,
    pub second_place_reward: u64,
    pub third_place_faction_id: u8,
    pub third_place_reward: u64,
    pub random_winner_faction_id: u8,
    pub random_winner_reward: u64,
    pub timestamp: i64,
}

/// Event emitted when a faction claims their treasury rewards
#[event]
pub struct FactionTreasuryRewardsClaimed {
    pub tax_config: Pubkey,
    pub faction_id: u8,
    pub faction_state: Pubkey,
    pub rank: u8,
    pub total_reward: u64,
    pub minebtc_staker_reward: u64,
    pub lp_staker_reward: u64,
    pub minebtc_emission_vault: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when a distribution round finishes
#[event]
pub struct DistributionRoundFinished {
    pub tax_config: Pubkey,
    pub end_timestamp: i64,
    pub next_round_start_after: i64,
    pub timestamp: i64,
}

// ========================================================================================
// =============================== GAMEPLAY DOGE EVENTS ====================================
// ========================================================================================

/// Event emitted when an doge is used for gameplay
#[event]
pub struct EggUsedForGameplay {
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub faction_id: u8,
    pub timestamp: i64,
}

/// Event emitted when an doge is withdrawn from gameplay
#[event]
pub struct EggWithdrawnFromGameplay {
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub faction_id: u8,
    pub timestamp: i64,
}

/// Event emitted when an instant mutation is triggered during betting
#[event]
pub struct MutationTriggered {
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub faction_id: u8,
    pub round_id: u64,
    /// 0 = Evolution, 1 = Power, 2 = Trait
    pub mutation_type: u8,
    pub bet_amount: u64,
    pub highest_bet: u64,
    pub timestamp: i64,
}
