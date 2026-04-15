use anchor_lang::prelude::*;

use crate::state::{PredictionDirection, NUM_FACTIONS};

// ------------------------------
// User management events
// ------------------------------

#[event]
pub struct ReferralRewardsClaimed {
    pub referrer: Pubkey,
    pub referral_rewards_account: Pubkey,
    pub minebtc_amount: u64,
    pub sol_amount: u64,
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
    pub lp_token_price: u64, // LP token price in SOL (9 decimals)
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
    pub ticket_count: u64,
}

#[event]
pub struct DogeFreeMintAllowanceUpdated {
    pub authority: Pubkey,
    pub user: Pubkey,
    pub remaining_free_mints: u8,
}

#[event]
pub struct DogeCollectionCreated {
    pub collection: Pubkey,
    pub update_authority: Pubkey,
    pub name: String,
    pub uri: String,
}

#[event]
pub struct CollectionDelegateAdded {
    pub collection: Pubkey,
    pub delegate: Pubkey,
}

#[event]
pub struct CollectionInfoUpdated {
    pub collection: Pubkey,
    pub new_name: Option<String>,
    pub new_uri: Option<String>,
}

/// Event emitted when a Doge is staked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct DogeStaked {
    /// User who staked the doge
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Doge mint address
    pub doge_mint: Pubkey,
    /// Doge metadata account address
    pub doge_metadata_account: Pubkey,
    /// Player's current multiplier after staking
    pub player_multiplier: u16,
    /// Player's current MINEBTC hashpower after staking
    pub dogebtc_hashpower: u64,
    /// Player's current LP hashpower after staking
    pub lp_hashpower: u64,
    /// Timestamp of the staking action
    pub timestamp: i64,
}

/// Event emitted when a Doge is unstaked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct DogeUnstaked {
    /// User who unstaked the doge
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// Doge mint address
    pub doge_mint: Pubkey,
    /// Doge metadata account address
    pub doge_metadata_account: Pubkey,
    /// Player's current multiplier after unstaking
    pub player_multiplier: u16,
    /// Player's current MINEBTC hashpower after unstaking
    pub dogebtc_hashpower: u64,
    /// Player's current LP hashpower after unstaking
    pub lp_hashpower: u64,
    /// Timestamp of the unstaking action
    pub timestamp: i64,
}

/// Event emitted when an doge is sent to heaven (burnt) for rewards
#[event]
pub struct DogeSentToHeaven {
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
    pub faction_id: u8, // player's home faction (staking target)
    pub position_index: u8,
    pub position_key: Pubkey,
    pub staked_amount: u64,   // actual amount staked (after burn tax)
    pub weighted_amount: u64, // weighted amount (before doge multiplier)
    pub multiplier: u16,      // lockup multiplier (100 = 1x)
    pub lockup_duration: u64,
    pub hashpower_contribution: u64, // final hashpower (with doge multiplier)
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
    pub faction_id: u8, // player's home faction (staking target)
    pub position_index: u8,
    pub position_key: Pubkey,
    pub staked_amount: u64,   // actual amount staked
    pub weighted_amount: u64, // weighted amount (before doge multiplier)
    pub multiplier: u16,      // lockup multiplier (100 = 1x)
    pub lockup_duration: u64,
    pub hashpower_contribution: u64, // final hashpower (with doge multiplier)
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
    pub referral_bonus: u64,  // 1% bonus to user if they have referral code
    pub referral_reward: u64, // 3% reward to referrer
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

    pub gameplay_doge: Pubkey,
    pub gameplay_doge_dna: [u8; 32],
    pub active_multiplier: u32,
    pub gameplay_doge_xp: u32,

    pub round_id: u64,
    pub epoch_id: u64,
    pub index_id: u8,
    pub num_bets: u8,
    pub faction_ids: Vec<u8>,
    pub directions: Vec<u8>,
    pub net_amounts: Vec<u64>,
    pub fee_amounts: Vec<u64>,
    pub points_amounts: Vec<u64>,
    pub wgtd_points_amounts: Vec<u64>,

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

#[event]
pub struct DogeSynced {
    pub doge_mint: Pubkey,
    pub doge_metadata_account: Pubkey,
    pub dna: Vec<u8>,
    pub xp: u32,
    pub multiplier: u32,
    pub accumulated_val: u64,
    pub accum_pct: u32,
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
    pub gameplay_doge: Pubkey,
    pub autominer_vault: Pubkey,
    pub sol_per_round: u64,
    pub num_rounds: u32,
    pub bets_per_round: u64,
    pub bet_size_per_bet: u64,
    pub has_factions_config: bool,
    pub can_reload: bool,
    pub use_ticket: Option<u8>,
    pub timestamp: i64,
}

/// Event emitted when autominer is initialized
#[event]
pub struct AutominerUpdated {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub autominer_vault: Pubkey,
    pub sol_per_round: u64,
    pub rounds_remaining: u32,
    pub can_reload: bool,
    pub sol_diff: i64,
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

#[event]
pub struct AutominerReloaded {
    pub autominer_vault: Pubkey,
    pub rounds_to_add: u32,
    pub sol_for_rounds: u64,
    pub leftover_sol: u64,
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
    pub epoch_id: u64,
    pub active_index_id: u8,
    pub active_question_hash: [u8; 32],
    pub round_start_slot: u64,
    pub round_start_timestamp: i64,
    pub round_end_timestamp: i64,
    pub scheduled_entropy_slot: u64,
    pub timestamp: i64,
}

/// Event emitted when a round ends (after winner selection and reward calculations)
#[event]
pub struct RoundEnded {
    pub round_id: u64,
    pub game_session: Pubkey,
    pub winning_faction_id: u8,
    pub winning_direction: u8,
    pub entropy_slot_used: u64,
    pub used_entropy_fallback: bool,
    pub total_sol_bets: u64,
    pub total_points_bets: u64,

    pub user_bets_count: [u64; NUM_FACTIONS],
    pub faction_sol_bets: [u64; NUM_FACTIONS],
    pub faction_points: [u64; NUM_FACTIONS],
    pub faction_wgtd_points: [u64; NUM_FACTIONS],

    pub minebtc_winner_pool: u64,
    pub minebtc_same_faction_pool: u64,
    pub minebtc_same_faction_direction_pools: [u64; PredictionDirection::COUNT],
    pub minebtc_faction_stakers: u64,
    pub minebtc_motherlode: u64,
    pub motherlode_hit: bool,
    pub timestamp: i64,
}

#[event]
pub struct DogeBtcStakingRewardsDistributed {
    pub round_id: u64,
    pub faction_id: u8,
    pub minebtc_staker_rewards: u64,
    pub sol_staker_rewards: u64,
    pub dogebtc_dogebtc_reward_index: u128,
    pub dogebtc_sol_reward_index: u128,
}

#[event]
pub struct LpStakingRewardsDistributed {
    pub round_id: u64,
    pub faction_id: u8,
    pub minebtc_staker_rewards: u64,
    pub sol_staker_rewards: u64,
    pub lp_dogebtc_reward_index: u128,
    pub lp_sol_reward_index: u128,
}

#[event]
pub struct MotherlodeHit {
    pub round_id: u64,
    pub faction_id: u8,
    pub winning_direction: u8,
    pub winning_faction_rewards: u64,
    pub minebtc_rewards_index: u128,
}

#[event]
pub struct RewardsDistributedForRound {
    pub round_id: u64,
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
    pub dogebtc_hashpower: u64,
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
pub struct DogeUsedForGameplay {
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when an doge is withdrawn from gameplay
#[event]
pub struct DogeWithdrawnFromGameplay {
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when an instant mutation is triggered during betting
#[event]
pub struct MutationTriggered {
    pub round_id: u64,
    pub user: Pubkey,
    pub doge_mint: Pubkey,
    pub xp_gained: u32,
}

/// Event emitted when a doge evolves to a new stage
#[event]
pub struct DogeEvolution {
    pub round_id: u64,
    pub doge_mint: Pubkey,
    pub new_stage: u8,
    /// Visual trait mutation that happened during evolution
    pub visual_trait_index: u8,
    pub visual_old_val: u8,
    pub visual_new_val: u8,
    /// Power trait mutation that happened during evolution
    pub power_trait_index: u8,
    pub power_old_val: u8,
    pub power_new_val: u8,
}

/// Event emitted when a doge's power trait is mutated
#[event]
pub struct DogePowerMutation {
    pub round_id: u64,
    pub doge_mint: Pubkey,
    pub trait_index: u8,
    pub old_val: u8,
    pub new_val: u8,
}

/// Event emitted when a doge's visual trait is mutated
#[event]
pub struct DogeVisualMutation {
    pub round_id: u64,
    pub doge_mint: Pubkey,
    pub trait_index: u8,
    pub old_val: u8,
    pub new_val: u8,
}

// ========================================================================================
// =============================== EPOCH MINING EVENTS ====================================
// ========================================================================================

/// Event emitted when a new epoch starts
#[event]
pub struct EpochStarted {
    pub epoch_id: u64,
    pub index_id: u8,
    pub question_hash: [u8; 32],
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub risk_factor: u16,
    pub timestamp: i64,
}

/// Event emitted when an index state is initialized.
#[event]
pub struct IndexInitialized {
    pub index_id: u8,
    pub name: String,
    pub initial_scores: [i64; NUM_FACTIONS],
    pub initial_ranks: [u8; NUM_FACTIONS],
    pub timestamp: i64,
}

/// Event emitted when the oracle schedules the next epoch market.
#[event]
pub struct EpochMarketScheduled {
    pub active_index_id: u8,
    pub next_index_id: u8,
    pub next_question_hash: [u8; 32],
    pub timestamp: i64,
}

/// Event emitted when AI oracle updates index scores.
#[event]
pub struct EpochScoresUpdated {
    pub index_id: u8,
    pub score_deltas: [i64; NUM_FACTIONS],
    pub cumulative_scores: [i64; NUM_FACTIONS],
    pub ranks: [u8; NUM_FACTIONS],
    pub update_number: u32,
    pub timestamp: i64,
}

/// Event emitted when risk factor is updated
#[event]
pub struct RiskFactorUpdated {
    pub old_risk_factor: u16,
    pub new_risk_factor: u16,
    pub timestamp: i64,
}

/// Event emitted when an epoch is settled
#[event]
pub struct EpochSettled {
    pub epoch_id: u64,
    pub index_id: u8,
    pub question_hash: [u8; 32],
    pub total_dogebtc_mined: u64,
    pub risk_factor: u16,
    pub epoch_mining_pool: u64,
    pub start_scores: [i64; NUM_FACTIONS],
    pub final_scores: [i64; NUM_FACTIONS],
    pub start_ranks: [u8; NUM_FACTIONS],
    pub final_ranks: [u8; NUM_FACTIONS],
    pub rank_deltas: [i8; NUM_FACTIONS],
    pub resolved_directions: [u8; NUM_FACTIONS],
    pub faction_reward_pools: [u64; NUM_FACTIONS],
    pub timestamp: i64,
}

/// Event emitted when a user claims epoch rewards
#[event]
pub struct EpochRewardsClaimed {
    pub epoch_id: u64,
    pub index_id: u8,
    pub user: Pubkey,
    pub reward_amount: u64,
    pub timestamp: i64,
}

/// Event emitted when an epoch is auto-started inline (during join_round or end_round_faction_rewards)
#[event]
pub struct EpochAutoStarted {
    pub epoch_id: u64,
    pub index_id: u8,
    pub question_hash: [u8; 32],
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

/// Event emitted when an epoch is auto-settled inline (during end_round_faction_rewards)
#[event]
pub struct EpochAutoSettled {
    pub epoch_id: u64,
    pub index_id: u8,
    pub mining_pool: u64,
}
