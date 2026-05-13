use anchor_lang::prelude::*;

use crate::state::{PredictionDirection, NUM_FACTIONS};

// ------------------------------
// User management events
// ------------------------------

#[event]
pub struct ReferralRewardsClaimed {
    pub referrer: Pubkey,
    pub referral_rewards_account: Pubkey,
    pub dbtc_amount: u64,
    pub sol_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SolFeesWithdrawn {
    pub available_solana: u64,
    pub buyback_amount: u64,
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
    pub dbtc_received: u64,       // MINE_BTC received from swap (6 decimals)
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
    pub dbtc_amount: u64,   // MINE_BTC added to pool (6 decimals)
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
    pub new_mining_multiplier: u16,
    pub timestamp: i64,
}

#[event]
pub struct FactionWarMultiplierUpdated {
    pub old_multiplier_bps: u16,
    pub new_multiplier_bps: u16,
    pub direction: i8,
    pub timestamp: i64,
}

#[event]
pub struct LpTokensBurned {
    pub lp_tokens_burned: u64,
    pub total_lp_burnt: u64,
    pub dbtc_amount_added: u64,
    pub sol_amount_added: u64,
    pub lp_token_price: u64, // LP token price in SOL (9 decimals)
    pub timestamp: i64,
}

// ========================================================================================
// ===============================  HASHBEAST NFT EVENTS =================================
// ========================================================================================

#[event]
pub struct HashBeastMinted {
    pub hashbeast_metadata_account: Pubkey,
    pub hashbeast_asset_signer: Pubkey,
    pub owner: Pubkey,
    pub player: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub dna: [u8; 32],
    pub multiplier: u32,
    pub accumulated_val: u64,
    pub faction_id: u8, // Faction/country the hashbeast belongs to
    pub price: u64,
    pub ticket_tier: u64,
    pub ticket_count: u64,
}

#[event]
pub struct HashBeastBred {
    pub breeder: Pubkey,
    pub mom: Pubkey,
    pub dad: Pubkey,
    pub offspring: Pubkey,
    pub faction_id: u8,
    pub rebirth_count: u8,
    pub curve_price_lamports: u64,
    pub floor_anchor_lamports: u64,
    pub floor_min_price_lamports: u64,
    pub total_price_lamports: u64,
    pub sol_paid_lamports: u64,
    pub sol_fee_recipient_lamports: u64,
    pub sol_treasury_lamports: u64,
    pub dbtc_price_lamports: u64,
    pub dbtc_paid: u64,
    pub dbtc_burned: u64,
    pub dbtc_to_vault: u64,
    pub timestamp: i64,
}

#[event]
pub struct HashBeastFreeMintAllowanceUpdated {
    pub authority: Pubkey,
    pub user: Pubkey,
    pub remaining_free_mints: u8,
}

#[event]
pub struct HashBeastCollectionCreated {
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

#[event]
pub struct EvolutionUnlockStageUpdated {
    pub authority: Pubkey,
    pub max_evolution_stage_unlocked: u8,
}

#[event]
pub struct GameplayTuningUpdated {
    pub authority: Pubkey,
    pub rpg_progression: bool,
    pub max_evolution_stage_unlocked: u8,
    pub war_base_reward_bps: u16,
    pub war_mvp_reward_bps: u16,
    pub war_hashbeast_reward_bps: u16,
    pub base_mutation_chance_bps: u16,
    pub mutation_chance_floor_bps: u16,
    pub mutation_chance_cap_bps: u16,
    pub faction_volume_threshold_lamports: u64,
    pub extra_volume_threshold_per_mutation_lamports: u64,
    pub target_mutations_per_cycle: u16,
    pub target_rounds_per_cycle: u16,
    pub pacing_max_adjustment_bps: u16,
}

/// Event emitted when a HashBeast is staked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct HashBeastStaked {
    /// User who staked the hashbeast
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// HashBeast mint address
    pub hashbeast_mint: Pubkey,
    /// HashBeast metadata account address
    pub hashbeast_metadata_account: Pubkey,
    /// Player's current multiplier after staking
    pub player_multiplier: u16,
    /// Player's current MINEBTC hashpower after staking
    pub degenbtc_hashpower: u64,
    /// Player's current LP hashpower after staking
    pub lp_hashpower: u64,
    /// Timestamp of the staking action
    pub timestamp: i64,
}

/// Event emitted when a HashBeast is unstaked
/// Tracks multiplier changes and hashpower updates for indexing
#[event]
pub struct HashBeastUnstaked {
    /// User who unstaked the hashbeast
    pub owner: Pubkey,
    /// Player data account address
    pub player: Pubkey,
    /// HashBeast mint address
    pub hashbeast_mint: Pubkey,
    /// HashBeast metadata account address
    pub hashbeast_metadata_account: Pubkey,
    /// Player's current multiplier after unstaking
    pub player_multiplier: u16,
    /// Player's current MINEBTC hashpower after unstaking
    pub degenbtc_hashpower: u64,
    /// Player's current LP hashpower after unstaking
    pub lp_hashpower: u64,
    /// Timestamp of the unstaking action
    pub timestamp: i64,
}

/// Event emitted when a HashBeast is reborn. The user
/// receives any accumulated_val, then the same asset is reborn into inventory
/// with fresh DNA and default gameplay state.
#[event]
pub struct HashBeastReborn {
    pub asset: Pubkey,
    pub former_owner: Pubkey,
    pub accumulated_val: u64,
    pub quality_score: u16,
    pub rebirth_count: u8,
    pub new_dna: [u8; 32],
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
    pub weighted_amount: u64, // weighted amount (before hashbeast multiplier)
    pub multiplier: u16,      // lockup multiplier (100 = 1x, max 300 = 3x)
    pub lockup_duration: u64,
    pub hashpower_contribution: u64, // final hashpower (with hashbeast multiplier)
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
    pub timestamp: i64,
}

#[event]
pub struct MineBtcUnstaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
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
    pub weighted_amount: u64, // weighted amount (before hashbeast multiplier)
    pub multiplier: u16,      // lockup multiplier (100 = 1x, max 300 = 3x)
    pub lockup_duration: u64,
    pub hashpower_contribution: u64, // final hashpower (with hashbeast multiplier)
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityUnstaked {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub new_sol_rewards: u64,
    pub new_dbtc_rewards: u64,
    pub unrefined_dbtc: u64,
    pub original_amount: u64,
    pub returned_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct PaperHandBurned {
    pub owner: Pubkey,
    pub player_data: Pubkey,
    pub position_index: u8,
    pub position_key: Pubkey,
    pub staked_token_type: u8, // 0 = degenBTC, 1 = LP
    pub original_amount: u64,
    pub penalty_amount: u64,
    pub returned_amount: u64,
    pub penalty_tax_pct: u64,
    pub days_remaining: u64,
    pub timestamp: i64,
}

/// Event emitted when a user claims passive staking rewards.
#[event]
pub struct SolRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub sol_amount: u64,
    pub dbtc_amount: u64,
    pub timestamp: i64,
}

/// Event emitted when a user withdraws gameplay-earned degenBTC token rewards.
#[event]
pub struct DbtcRewardsClaimed {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub faction_id: u8,
    pub dbtc_amount: u64,
    pub hodl_tax: u64,
    pub referral_bonus: u64,  // 1% bonus to user if they have referral code
    pub referral_reward: u64, // 3% reward to referrer
    pub referrer: Option<Pubkey>,
    pub timestamp: i64,
}

/// Event emitted whenever pending degenBTC claimable balance is increased.
/// `source_amount` is the new reward from the triggering action, while
/// `unrefined_bonus_amount` is previously deferred hodl-tax yield realized at the same time.
#[event]
pub struct MinebtcClaimableAccrued {
    pub user: Pubkey,
    pub player_data: Pubkey,
    pub source: u8,
    pub reference_id: u64,
    pub source_amount: u64,
    pub unrefined_bonus_amount: u64,
    pub total_added: u64,
    pub pending_dbtc_after: u64,
    pub total_claimable_after: u64,
    pub timestamp: i64,
}

/// Event emitted when a degenBTC HODL tax is redistributed through the HODL tax index.
/// Event emitted when a user pays the HODL tax ("HODL Tax") and it gets
/// redistributed to other users with unclaimed gameplay rewards.
#[event]
pub struct HodlTaxRedistributed {
    pub paper_hand: Pubkey, // user who paid the tax (unstaked early / claimed rewards)
    pub player_data: Pubkey,
    pub tax_amount: u64, // total HODL tax paid
    pub redistributed_amount: u64,
    pub redistributed_index_increment: u128,
    pub remaining_total_claimable: u64, // remaining HODL-eligible gameplay rewards
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
    pub origin_faction_id: u8,
    pub referral_code: Option<Pubkey>,
    pub referrer_faction_id: Option<u8>,
    pub timestamp: i64,
}

/// Event emitted when a player joins through the country referral loop.
#[event]
pub struct PlayerRecruited {
    pub player: Pubkey,
    pub referrer: Pubkey,
    pub player_origin_faction_id: u8,
    pub referrer_origin_faction_id: u8,
    pub referrer_total_recruits: u64,
    pub timestamp: i64,
}

/// Event emitted when bets are placed (single, batch, or autominer)
#[event]
pub struct BetsPlaced {
    pub user: Pubkey,
    pub player_data: Pubkey,

    pub gameplay_hashbeast: Pubkey,
    pub gameplay_hashbeast_dna: [u8; 32],
    pub active_multiplier: u32,
    pub gameplay_hashbeast_xp: u32,

    pub round_id: u64,
    pub war_id: u64,
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

    /// Total SOL deducted from this batch for the cycle SOL split (faction war vault).
    pub total_cycle_sol_split: u64,

    pub timestamp: i64,
}

#[event]
pub struct HashBeastSynced {
    pub hashbeast_mint: Pubkey,
    pub hashbeast_metadata_account: Pubkey,
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
    pub gameplay_hashbeast: Pubkey,
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
    pub war_id: u64,
    pub round_start_slot: u64,
    pub round_start_timestamp: i64,
    pub round_end_timestamp: i64,
    pub scheduled_entropy_slot: u64,
    pub timestamp: i64,
}

/// Event emitted when a round ends (after winner selection and reward calculations).
///
/// Carries everything the off-chain indexer needs to render `latest_result`
/// WITHOUT having to fetch the `GameSession` PDA. All these fields are already
/// populated on the PDA by the time `emit_round_ended` is called (game.rs:559).
/// `winning_faction_volume_at_round` is the one exception — it lands later in
/// `track_war_round_completion` and ships on `RewardsDistributedForRound`.
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
    pub total_wgtd_points_bets: u64,

    pub user_bets_count: [u64; NUM_FACTIONS],
    pub faction_sol_bets: [u64; NUM_FACTIONS],

    pub dbtc_winner_pool: u64,
    pub dbtc_same_faction_direction_pools: [u64; PredictionDirection::COUNT],
    pub dbtc_stakers: u64,
    pub dbtc_jackpot: u64,
    pub jackpot_hit: bool,
    pub jackpot_faction_id: u8,

    /// Σ stakers_fee_per_bet accumulated by internal_process_bets (user.rs:2544).
    /// Indexer reverses this to derive effective_fee = stakers_fee × 100 / stakers_pct,
    /// then splits the residual into buybacks / nft_mm / dev_fee per economy.rs.
    pub stakers_fee: u64,
    /// Reward indexes finalized inside end_round. SOL paid by raw points,
    /// dBTC paid by weighted points (hashbeast multiplier applies to dBTC only).
    pub sol_rewards_index: u128,
    pub dbtc_rewards_index: u128,
    /// Mutation tally for this round (state.rs:1037-1040).
    pub mutations_per_faction: [u8; NUM_FACTIONS],
    pub total_mutations_this_round: u8,
    /// Cycle ID snapshot at round-start, frozen onto the GameSession so late
    /// claims hit the right FactionWarState PDA even after the cycle settles
    /// (state.rs:1042-1046).
    pub war_id_when_played: u64,

    pub timestamp: i64,
}

#[event]
pub struct DegenBtcStakingRewardsDistributed {
    pub round_id: u64,
    pub faction_id: u8,
    pub dbtc_staker_rewards: u64,
    pub sol_staker_rewards: u64,
    pub degenbtc_degenbtc_reward_index: u128,
    pub degenbtc_sol_reward_index: u128,
}

#[event]
pub struct LpStakingRewardsDistributed {
    pub round_id: u64,
    pub faction_id: u8,
    pub dbtc_staker_rewards: u64,
    pub sol_staker_rewards: u64,
    pub lp_degenbtc_reward_index: u128,
    pub lp_sol_reward_index: u128,
}

/// Event emitted by `distribute_jackpot_rewards` when the jackpot pot was
/// successfully paid out to bettors on `faction_id`. Note that `faction_id`
/// here is the JACKPOT faction (selected by inverse-volume weighting), NOT
/// the round winner — see game.rs:777-783.
#[event]
pub struct JackpotHit {
    pub round_id: u64,
    pub faction_id: u8,
    /// Size of the global jackpot pot at the moment of the hit, snapshotted
    /// onto GameSession.jackpot_pot_size_on_hit before the pot was drained.
    pub jackpot_pot_size_on_hit: u64,
    /// dBTC reward index for any-direction bettors on the jackpot faction
    /// (state.rs:1029). Claim payout = wgtd_points × jackpot_rewards_index / INDEX_PRECISION.
    pub jackpot_rewards_index: u128,
}

/// Event emitted when the jackpot roll was close to hitting (within top 10 closest rolls).
/// Used by the frontend to hook users with near-miss notifications.
#[event]
pub struct JackpotNearMiss {
    pub round_id: u64,
    pub roll: u64,
    pub threshold: u64,
    pub pot_size: u64,
    pub timestamp: i64,
}

/// Event emitted when the jackpot hits but there are no eligible winners
/// to receive it. The pot rolls over and keeps accumulating.
#[event]
pub struct JackpotRolledOver {
    pub round_id: u64,
    pub faction_id: u8,
    pub pot_size: u64,
    pub reason: u8, // 0 = no exact winners, 1+ reserved for future
    pub timestamp: i64,
}

/// Event emitted by `settle_round` after `track_war_round_completion`
/// runs. Carries the drought-volume snapshot that fed into the mutation roll for
/// this round's claimers (state.rs:1048-1053, used at user.rs:1689).
#[event]
pub struct RewardsDistributedForRound {
    pub round_id: u64,
    pub winning_faction_id: u8,
    pub winning_direction: u8,
    /// Frozen value of the winning faction's `sol_volume_since_last_win` at
    /// round-end, BEFORE the counter was reset to 0. Late claims hours later
    /// will still see this same number when computing volume_factor.
    pub winning_faction_volume_at_round: u64,
    pub timestamp: i64,
}

// ========================================================================================
// =============================== TAX & DISTRIBUTION EVENTS =============================
// ========================================================================================

/// Event emitted when tax is distributed from mint to vaults
#[event]
pub struct TaxDistributed {
    pub total_tax_amount: u64,
    pub faction_treasury_amount: u64,
    pub burn_amount: u64,
    pub total_burnt: u64,
    pub timestamp: i64,
}

/// Event emitted when a faction claims treasury rewards for a settled faction_war.
#[event]
pub struct FactionTreasuryRewardsClaimed {
    pub war_id: u64,
    pub faction_id: u8,
    pub rank: u8,
    pub reward_amount: u64,
    pub dbtc_share: u64,
    pub lp_share: u64,
    pub reborn_amount: u64,
    pub timestamp: i64,
}

// ========================================================================================
// =============================== GAMEPLAY HASHBEAST EVENTS ====================================
// ========================================================================================

/// Event emitted when a HashBeast is used for gameplay
#[event]
pub struct HashBeastUsedForGameplay {
    pub user: Pubkey,
    pub hashbeast_mint: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when a HashBeast is withdrawn from gameplay
#[event]
pub struct HashBeastWithdrawnFromGameplay {
    pub user: Pubkey,
    pub hashbeast_mint: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when a user requests gameplay unlock for the next faction_war cycle.
#[event]
pub struct HashBeastGameplayUnlockRequested {
    pub user: Pubkey,
    pub hashbeast_mint: Pubkey,
    pub requested_during_war_id: u64,
    pub unlock_available_after_war_id: u64,
    pub timestamp: i64,
}

/// Event emitted when gameplay creates a story-worthy HashBeast event.
///
/// The contract may still mutate DNA / XP / multiplier as part of the event,
/// but off-chain systems should treat this as a flexible story hook. A backend
/// can turn it into artwork, reels, character history, or a simple indexed beat.
#[event]
pub struct StoryEventTriggered {
    /// 0 = round claim, 1 = faction-war claim.
    pub origin: u8,
    pub origin_id: u64,
    pub user: Pubkey,
    pub hashbeast_mint: Pubkey,
    pub story_event_type: u8,
    pub xp_gained: u32,
    pub multiplier_after: u32,
}

/// Event emitted when a hashbeast evolves to a new stage
#[event]
pub struct HashBeastEvolution {
    /// 0 = round claim, 1 = faction-war claim.
    pub origin: u8,
    pub origin_id: u64,
    pub hashbeast_mint: Pubkey,
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

/// Event emitted when a hashbeast's power trait is mutated
#[event]
pub struct HashBeastPowerMutation {
    /// 0 = round claim, 1 = faction-war claim.
    pub origin: u8,
    pub origin_id: u64,
    pub hashbeast_mint: Pubkey,
    pub trait_index: u8,
    pub old_val: u8,
    pub new_val: u8,
}

/// Event emitted when a hashbeast's visual trait is mutated
#[event]
pub struct HashBeastVisualMutation {
    /// 0 = round claim, 1 = faction-war claim.
    pub origin: u8,
    pub origin_id: u64,
    pub hashbeast_mint: Pubkey,
    pub trait_index: u8,
    pub old_val: u8,
    pub new_val: u8,
}

// ========================================================================================
// ============================ FACTION WAR EVENTS ==========================================
// ========================================================================================

/// Event emitted when a faction_war is settled.
/// Rankings driven by on-chain gameplay scores accumulated during the faction_war.
#[event]
pub struct FactionWarSettled {
    pub war_id: u64,
    pub total_degenbtc_mined: u64,
    pub dbtc_mined_this_war: u64,
    pub final_ranks: [u8; NUM_FACTIONS],
    pub rank_deltas: [i8; NUM_FACTIONS],
    pub resolved_directions: [u8; NUM_FACTIONS],
    pub base_reward_pools: [u64; NUM_FACTIONS],
    pub hashbeast_reward_pools: [u64; NUM_FACTIONS],
    /// SOL lane allocations (sum across eligibles). Per-user SOL share at
    /// claim time scales each by `user_dbtc_lane / total_dbtc_lane`.
    pub sol_base_pool: u64,
    pub sol_hb_pool: u64,
    pub sol_mvp_pool: u64,
    /// SOL drained to sol_treasury because no eligible claimant existed for
    /// that rank/lane slot (e.g. faction had no winners on resolved direction,
    /// no mutations, or no MVP).
    pub undistributed_sol: u64,
    /// Per-faction MVP dBTC bonus (zero where mvp_user[fid] == default).
    pub mvp_bonus: [u64; NUM_FACTIONS],
    /// Per-faction MVP user (default pubkey = no MVP this cycle).
    pub mvp_user: [Pubkey; NUM_FACTIONS],
    pub round_wins: [u16; NUM_FACTIONS],
    pub gameplay_scores: [u64; NUM_FACTIONS],
    pub timestamp: i64,
}

/// Emitted by `initialize_war_internal` when a new cycle's FactionWarState
/// PDA is created. Lets indexers detect cycle starts without scanning ix data.
#[event]
pub struct FactionWarStarted {
    pub war_id: u64,
    pub faction_count: u8,
    pub start_timestamp: u64,
    pub prev_ranks: [u8; NUM_FACTIONS],
    /// LP-operations count threshold; once `pol_stats.lp_operations_count`
    /// reaches this, the LP-burn ix snapshots the cycle's final round.
    pub settle_at_lp_op_count: u32,
    /// Treasury-tax SOL seeded at war start (rolled forward from
    /// `tax_config.unassigned_war_treasury_amount`).
    pub treasury_reward_base_amount: u64,
}

/// Emitted by `add_lp_and_burn` when an LP operation pushes
/// `lp_operations_count` past `settle_at_lp_op_count` and the current cycle's
/// final round_id is snapshotted onto `FactionWarConfig.cycle_end_round_id`.
/// Lets indexers know the active cycle is now in its final round — no new
/// rounds will start until settle_war runs.
#[event]
pub struct CycleEndRoundSnapshotted {
    pub war_id: u64,
    pub cycle_end_round_id: u64,
    pub lp_operations_count: u32,
    pub timestamp: i64,
}

/// Cycle leaderboard score-add event. Emitted from two sites:
///
/// - `score_source = GAMEPLAY_SCORE_SOURCE_ROUND_WIN (0)`: end-of-round
///   accumulation when a country wins. `score_added` equals the round's
///   total weighted points bet on that country (any direction). `user` is
///   `Pubkey::default()` — no specific user owns this contribution.
///
/// - `score_source = GAMEPLAY_SCORE_SOURCE_MUTATION_BONUS (1)`: per-claim
///   bonus when a player's round-claim mutation roll succeeds and the
///   round's cycle is still active. `score_added` equals
///   `user_wgtd_points_on_winner × active_multiplier / BASE_MULTIPLIER × mutation_weight`
///   where `mutation_weight` is 4/2/1 for Evolution/Power/Trait. `user` is
///   the claimant — also drives MVP candidacy for the winning country.
#[event]
pub struct GameplayScoreAccumulated {
    pub war_id: u64,
    pub faction_id: u8,
    pub score_source: u8,
    pub score_added: u64,
    pub faction_total_score: u64,
    pub user: Pubkey,
}

/// Event emitted when a faction war MVP is determined at settlement.
/// The #1 ranked faction's top contributor receives a bonus.
#[event]
pub struct FactionWarMvp {
    pub war_id: u64,
    pub faction_id: u8,
    pub user: Pubkey,
    pub mvp_score: u64,
    pub bonus_amount: u64,
    pub timestamp: i64,
}

/// Event emitted when a user claims faction_war rewards. dBTC amounts are
/// per-lane (base + mvp + hb = reward_amount). SOL is broken out the same way
/// so the indexer can attribute earnings to predict/perform/mvp activity.
#[event]
pub struct FactionWarRewardsClaimed {
    pub war_id: u64,
    pub user: Pubkey,
    pub reward_amount: u64,
    pub base_reward_amount: u64,
    pub mvp_bonus_amount: u64,
    pub hashbeast_bonus_amount: u64,
    pub sol_reward_amount: u64,
    pub sol_base_amount: u64,
    pub sol_hb_amount: u64,
    pub sol_mvp_amount: u64,
    pub hashbeast_mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when the authority toggles the global pause flag.
/// Indexers should propagate `is_paused` to the frontend so the UI can
/// disable bet/mint actions and show a clear "paused" banner to users.
#[event]
pub struct GamePauseToggled {
    pub is_paused: bool,
    pub authority: Pubkey,
    pub timestamp: i64,
}

// ========================================================================================
// ============================ INVENTORY / LOOTBOX / MARKET ==============================
// ========================================================================================

/// One-time emit when the inventory pool is initialized.
#[event]
pub struct InventoryPoolInitialized {
    pub marketplace_program: Pubkey,
    pub marketplace_config: Pubkey,
}

/// Inventory PDA bought the cheapest user-listed NFT via `sweep_floor_lowest`.
/// Disposition is reflected in a follow-up event (LootboxQueuePush,
/// InventoryAssetRelisted, or InventoryAssetBurned).
#[event]
pub struct FloorSweepExecuted {
    pub asset: Pubkey,
    pub buy_price: u64,
    pub seller: Pubkey,
    pub anchor_price: u64,
    pub trend_bps: i32,
    pub stale_skipped: u8,
    pub keeper: Pubkey,
    pub timestamp: i64,
}

/// An inventory asset was relisted at a formula-driven price after either a
/// fresh sweep or an `expire_program_listing` strike.
#[event]
pub struct InventoryAssetRelisted {
    pub asset: Pubkey,
    pub original_buy_price: u64,
    pub new_list_price: u64,
    pub markup_bps: i32,
    pub trend_bps: i32,
    pub expire_count: u8,
    pub timestamp: i64,
}

/// An inventory asset was burned because either the trend crashed below
/// the burn threshold or the entry hit MAX_EXPIRES.
#[event]
pub struct InventoryAssetBurned {
    pub asset: Pubkey,
    /// 0 = trend crash, 1 = max expires, 2 = rebirth queue full
    pub reason: u8,
    pub trend_bps: i32,
    pub expire_count: u8,
    pub timestamp: i64,
}

/// A user (or keeper) registered a marketplace listing into the floor queue.
#[event]
pub struct FloorEntryRegistered {
    pub listing: Pubkey,
    pub asset: Pubkey,
    pub seller: Pubkey,
    pub price: u64,
    pub queue_index: u8,
    pub queue_size_after: u8,
    pub timestamp: i64,
}

/// A floor queue entry was removed (sale, cancel, price-update reorder, or stale).
#[event]
pub struct FloorEntryRemoved {
    pub listing: Pubkey,
    pub asset: Pubkey,
    pub queue_index: u8,
    pub reason: u8, // 0=sweep, 1=cancel, 2=price-update, 3=stale-popped
    pub timestamp: i64,
}

/// A user-to-user marketplace sale qualified as a real-demand snapshot input.
#[event]
pub struct UserSaleRecorded {
    pub asset: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub price: u64,
    pub listing_age_secs: i64,
    pub timestamp: i64,
}

/// A daily floor snapshot was committed.
#[event]
pub struct FloorSnapshotRecorded {
    pub anchor_price: u64,
    pub source: u8, // 0 = sale-median, 1 = queue-median fallback
    pub samples: u32,
    pub timestamp: i64,
}

/// An inventory listing that sat unsold for `EXPIRE_GRACE_SECS` was expired.
/// Disposition cascade follows in a separate event.
#[event]
pub struct ProgramListingExpired {
    pub asset: Pubkey,
    pub previous_list_price: u64,
    pub expire_count_after: u8,
    pub keeper: Pubkey,
    pub timestamp: i64,
}

/// One-time emit when a country's lootbox queue PDA is created at admin setup.
#[event]
pub struct LootboxQueueInitialized {
    pub faction_id: u8,
    pub queue_pda: Pubkey,
    pub timestamp: i64,
}

/// An asset was pushed into a country lootbox queue (from `rebirth_hashbeast`,
/// `sweep_floor_lowest`, or `expire_program_listing`). `queue_depth_after`
/// reflects post-push state.
#[event]
pub struct LootboxQueuePush {
    pub faction_id: u8,
    pub asset: Pubkey,
    pub queue_depth_after: u8,
    pub source: u8, // 0 = rebirth, 1 = sweep_buy
    pub timestamp: i64,
}

/// A losing player's claim ix triggered a roll that WON. Asset is reserved
/// for them via `LootboxClaim` PDA until a user or cranker delivers it with
/// `claim_lootbox_nft`.
#[event]
pub struct LootboxRollWon {
    pub user: Pubkey,
    pub faction_id: u8,
    pub asset: Pubkey,
    pub queue_depth_before: u8,
    pub roll_value: u16,
    pub threshold_bps: u16,
    pub timestamp: i64,
}

/// A losing player's claim ix triggered a roll that MISSED. Queue unchanged.
#[event]
pub struct LootboxRollMissed {
    pub user: Pubkey,
    pub faction_id: u8,
    pub queue_depth: u8,
    pub roll_value: u16,
    pub threshold_bps: u16,
    pub timestamp: i64,
}

/// Reserved hashbeast was delivered to the recorded user. `cranker` is the signer
/// that paid the delivery transaction; it may be the user or a bot.
#[event]
pub struct LootboxNftClaimed {
    pub user: Pubkey,
    pub cranker: Pubkey,
    pub faction_id: u8,
    pub asset: Pubkey,
    pub rebirth_count: u8,
    pub timestamp: i64,
}

/// A `rebirth_hashbeast` call burned the asset because the country queue was full,
/// inventory was full, or the asset had already reached MAX_REBIRTH_COUNT.
/// User still received their `accumulated_val` payout; the asset is gone.
#[event]
pub struct HashBeastRebirthBurned {
    pub asset: Pubkey,
    pub former_owner: Pubkey,
    pub faction_id: u8,
    pub accumulated_val: u64,
    pub rebirth_count: u8,
    /// 0 = queue/inventory full, 1 = max rebirth count reached
    pub reason: u8,
    pub timestamp: i64,
}

/// `handle_inventory_proceeds` split accumulated inventory SOL into the sweep
/// reserve and the protocol fee pipeline.
#[event]
pub struct InventoryProceedsRouted {
    pub to_sweep: u64,
    pub to_protocol: u64,
    pub timestamp: i64,
}

/// Permissionless `inventory_finalize_sale` cleaned up the RebornEntry
/// after detecting that an inventory listing's asset is no longer owned by
/// `inventory_pda` (i.e., it sold to a real buyer).
#[event]
pub struct InventorySaleFinalized {
    pub asset: Pubkey,
    pub keeper: Pubkey,
    pub timestamp: i64,
}

/// `distribute_sol_fees` peeled off `nft_market_making_pct` of available SOL
/// and routed it directly to `inventory_sweep_vault` to fund permissionless
/// NFT market-making (sweep buys + keeper bounties). Replaces the old
/// dbtc-tax → Raydium swap → SOL refill flow.
#[event]
pub struct NftMarketMakingFunded {
    pub sol_amount: u64,
    pub timestamp: i64,
}
