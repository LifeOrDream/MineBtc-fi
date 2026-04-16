// # State Definitions
//
// This module defines all the account structures and constants used in the MineBTC program.
//
// ## Key Accounts
//
// - `GlobalConfig`: Stores global configuration parameters (fees, authorities, etc.).
// - `GlobalGameState`: Tracks the overall game state, including active rounds and pots.
// - `FactionState`: Stores statistics and reward pools for each faction.
// - `PlayerData`: Stores user-specific data, including stats, balances, and staking positions.
// - `GameSession`: Represents a single game round, tracking bets and outcomes.
// - `MineBtcMining`: Manages the mining emission and distribution logic.
// - `DogeConfig`: Configuration for the Doge NFT system.
// - `TaxConfig`: Configuration for the tax and burn system.
//

use anchor_lang::prelude::*;

pub const MINEBTC_DECIMALS: u8 = 6;

pub const BASE_MULTIPLIER: u32 = 1000; // 1.0x
pub const MAX_MULTIPLIER: u16 = 10000; // Maximum multiplier a user can have (10.0x)

/// Base mutation chance in basis points (2000 = 20%).
/// Effective chance = BASE_CHANCE × bet_strength × mult_penalty × faction_penalty / scaling.
pub const MAX_BASE_CHANCE: u64 = 2000;

/// Per-faction penalty step: each prior mutation in the round for the same faction
/// reduces the next attempt's chance.  Formula: 10000 / (10000 + count * STEP).
/// At STEP=5000: after 1 mutation → 67%, after 2 → 50%, after 3 → 40%.
pub const FACTION_MUTATION_PENALTY_STEP: u64 = 5000;

// ========== MUTATION-DRIVEN EPOCH SCORING CONSTANTS ========== //
/// Weight for an Evolution mutation when computing faction epoch score.
pub const EVOLUTION_SCORE_WEIGHT: u64 = 100;
/// Weight for a Power mutation when computing faction epoch score.
pub const POWER_SCORE_WEIGHT: u64 = 30;
/// Weight for a Trait/Visual mutation when computing faction epoch score.
pub const TRAIT_SCORE_WEIGHT: u64 = 10;
/// Normalization divisor so scores stay in a sensible range (1 SOL = 1_000_000 lamports at 6 decimals).
pub const MUTATION_SCORE_PRECISION: u64 = 1_000_000;

/// ------------ CONSTANTS ------------

pub const DAY_IN_SECONDS: u64 = 86400;
pub const BURN_TAX_PERCENTAGE: u64 = 1; // 1% burn tax on transfers

pub const MAX_ALLOWED_POSITIONS: u8 = 7;
pub const EMERGENCY_WITHDRAWAL_PENALTY_PCT: u8 = 15;
/// Whole-percent precision used by fee and reward split config fields.
/// Example: `25` means 25%, `100` means 100%.
pub const PERCENTAGE_DENOMINATOR: u64 = 100;
pub const PERCENTAGE_DENOMINATOR_U8: u8 = PERCENTAGE_DENOMINATOR as u8;
pub const PERCENTAGE_DENOMINATOR_U16: u16 = PERCENTAGE_DENOMINATOR as u16;
pub const M_HUNDRED: u64 = PERCENTAGE_DENOMINATOR;
pub const BASIS_POINTS_DENOMINATOR: u64 = 10_000;
pub const EPOCH_DOGE_REWARD_SHARE_BPS: u64 = 1_000; // 10%

// ========== DECIMAL SCALING CONSTANTS ========== //

pub const INDEX_PRECISION: u64 = 1_000_000; // 1 million
pub const DISCRIMINATOR_SIZE: usize = 8;

// ========== FACTION SURGE RAFFLE CONSTANTS ========== //

pub const MOTHERLODE_CHANCE: u64 = 625; // 1 in 625 chance (0.16%)

pub const MAX_FACTIONS: usize = 15; // Up to 15 factions for the raffle
pub const NUM_FACTIONS: usize = 15; // Same as MAX_FACTIONS, used for array sizes
pub const MAX_FACTION_NAME_LENGTH: usize = 16; // Maximum length of faction name

/// Conservative upper-bound slot estimate used to schedule round entropy at round start.
/// This keeps the entropy slot after the round closes under normal slot timing, while the
/// finalize path can still fall back to the latest available slot hash if the scheduled hash
/// ages out before anybody settles the round.
pub const ROUND_ENTROPY_SLOTS_PER_SECOND_ESTIMATE: u64 = 4;
/// Extra slot buffer added on top of the estimated end slot before sampling entropy.
pub const ROUND_PRIMARY_ENTROPY_DELAY_SLOTS: u64 = 8;

// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / MineBtcMining state
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";
pub const HASHPOWER_CONFIG_SEED: &[u8] = b"hashpower-config";
pub const MINE_BTC_MINING_SEED: &[u8] = b"mine-btc-mining";
pub const UNREFINED_REWARDS_SEED: &[u8] = b"unrefined-rewards";

// PDAs which hold SOL collected by the program
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";
pub const DOGES_TREASURY_SEED: &[u8] = b"doges-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
pub const MINE_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"minebtc-vault-authority";
pub const MINE_BTC_VAULT_SEED: &[u8] = b"minebtc_vault";

pub const REFERRAL_REWARDS_SEED: &[u8] = b"referral-rewards";
pub const COLLECTION_AUTHORITY_SEED: &[u8] = b"collection_authority";

// PDAs for Doge NFT system
pub const DOGE_METADATA_SEED: &[u8] = b"doge-metadata";
pub const DOGE_CUSTODY_SEED: &[u8] = b"doge-custody"; // PDA that holds locked NFTs
pub const DOGE_FREE_MINT_ALLOWANCE_SEED: &[u8] = b"doge-free-mint-allowance";

pub const BUYBACKS_SEED: &[u8] = b"buybacks";
pub const BUYBACKS_SOL_VAULT_SEED: &[u8] = b"buybacks-sol-vault";

// PDAs for Game system
pub const GLOBAL_GAME_STATE_SEED: &[u8] = b"global-game-state";
pub const FACTION_STATE_SEED: &[u8] = b"faction";
pub const PLAYER_DATA_SEED: &[u8] = b"player";

// PDAs for Staking system
pub const STAKED_POSITION_SEED: &[u8] = b"staked-position";
pub const LP_STAKED_POSITION_SEED: &[u8] = b"lp-staked-position";

pub const MINEBTC_CUSTODIAN_SEED: &[u8] = b"minebtc-custodian";
pub const MINEBTC_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"minebtc-custodian-authority";
pub const LIQUIDITY_CUSTODIAN_SEED: &[u8] = b"lp-custodian";
pub const LIQUIDITY_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"lp-custodian-authority";

pub const GAME_SESSION_SEED: &[u8] = b"game-session"; // Seed: [b"game-session", round_id_u64]
pub const USER_GAME_BET_SEED: &[u8] = b"user-bet"; // Seed: [b"user-bet", user_pubkey, round_id_u64]
pub const AUTOMINER_VAULT_SEED: &[u8] = b"autominer";
pub const AUTOMINER_CUSTODY_SEED: &[u8] = b"autominer-custody";
pub const SOL_PRIZE_POT_VAULT_SEED: &[u8] = b"sol-prize-pot";
pub const MOTHERLODE_POT_VAULT_SEED: &[u8] = b"motherlode-pot";

pub const STAKER_SOL_REWARD_VAULT_SEED: &[u8] = b"staker-sol-reward-vault";
pub const DOGE_CONFIG_SEED: &[u8] = b"doge-config";

// PDAs for Epoch Mining system
pub const EPOCH_CONFIG_SEED: &[u8] = b"epoch-config";
pub const EPOCH_STATE_SEED: &[u8] = b"epoch"; // Seed: [b"epoch", epoch_id_u64]
pub const USER_EPOCH_BETS_SEED: &[u8] = b"user-epoch"; // Seed: [b"user-epoch", user_pubkey, epoch_id_u64]

// PDAs for Tax system
pub const TAX_CONFIG_SEED: &[u8] = b"tax-config";
pub const WITHDRAW_WITHHELD_AUTHORITY_SEED: &[u8] = b"withdraw-withheld-authority";
pub const FACTION_TREASURY_VAULT_SEED: &[u8] = b"faction-treasury-vault";
pub const NFT_FLOOR_SWEEP_VAULT_SEED: &[u8] = b"nft-floor-sweep-vault";
pub const NFT_SALE_SOL_VAULT_SEED: &[u8] = b"nft-sale-sol-vault";

// ==========  DOGE NFT CONSTANTS ========== //
pub const MAX_STAKED_DOGES: usize = 5; // Maximum number of doges a user can stake
pub const MAX_FREE_DOGE_MINTS_PER_USER: u8 = 5;

pub const MAX_CALLER_COMPENSATION: u64 = 5_000_000; // 0.005 SOL (0.005 SOL max per round)
pub const MIN_SOL_BET_PER_POSITION: u64 = 100_000; // 0.0001 SOL minimum per country-direction bet

/// ------------ GLOBAL CONFIG ------------

/// Global configuration for the program
#[account]
pub struct GlobalConfig {
    /// total number of players in the game
    pub total_players: u64,

    /// Authority that can update config parameters
    pub ext_authority: Pubkey,
    /// Pending authority for 2-step transfer (Pubkey::default() = no pending transfer)
    pub pending_authority: Pubkey,
    /// Direct recipient for doge mints + dev earnings revenue
    pub fee_recipient: Pubkey,

    /// PDA account that holds collected SOL fees
    pub pda_sol_treasury: Pubkey,

    /// List of supported factions (e.g., "USA", "China", "Russia")
    /// Maximum 15 factions, each with max 16 characters
    pub supported_factions: Vec<String>,

    /// SOL fee distribution configuration
    pub sol_fee_config: SolFeeConfig,

    /// MineBtc distribution configuration
    pub minebtc_dist_config: MineBtcDistConfig,

    /// Authorized Raydium pool state address (security: prevents using malicious pools)
    pub raydium_pool_state: Pubkey,

    /// Fee for changing faction (in lamports)
    pub change_faction_fee: u64,

    /// Minimum time interval between price snapshots (in seconds)
    /// Default: 1800 seconds (30 minutes)
    pub snapshot_interval: u64,

    /// Enable RPG progression (mutations, XP, etc) during gameplay
    pub rpg_progression: bool,

    /// ------------------------------------------------------------           
    /// Bump for GlobalConfig PDA derivation
    pub bump: u8,
    /// Bump for SOL treasury PDA derivation
    pub treasury_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SolFeeConfig {
    /// Whole-percent share of SOL fees that goes to protocol. `100` = 100%.
    pub protocol_fee_pct: u8,
    /// Whole-percent share of SOL fees that goes to buybacks. `100` = 100%.
    pub buyback_pct: u8,
    /// Whole-percent share of SOL fees that goes to stakers. `100` = 100%.
    pub stakers_pct: u8,
}

impl SolFeeConfig {
    pub const LEN: usize = 1 + 1 + 1; // protocol_fee_pct + buyback_pct + stakers_pct
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct MineBtcDistConfig {
    /// Whole-percent share of MineBtc emission that goes to stakers. `100` = 100%.
    pub minebtc_stakers_pct: u8,
    /// Whole-percent share of MineBtc emission that goes to winning faction bettors. `100` = 100%.
    pub minebtc_winners_pct: u8,
    /// Whole-percent share of MineBtc emission that goes to each non-winning
    /// direction on the winning faction. With 3 total directions, up to two
    /// losing directions may each receive this share if they have bettors.
    pub minebtc_same_faction_pct: u8,
    /// Whole-percent share of MineBtc emission that goes to motherlode. `100` = 100%.
    pub minebtc_motherlode_pct: u8,
    /// Whole-percent refining fee applied to pending MineBtc rewards. `100` = 100%.
    pub refining_fee: u8,
}

impl MineBtcDistConfig {
    pub const LEN: usize = 1 + 1 + 1 + 1 + 1; // minebtc_stakers_pct + minebtc_winners_pct + minebtc_same_faction_pct + minebtc_motherlode_pct + refining_fee
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +                     // total_players
        32 +                    // ext_authority
        32 +                    // pending_authority
        32 +                    // fee_recipient
        32 +                    // pda_sol_treasury
        SolFeeConfig::LEN +     // sol_fee_config
        MineBtcDistConfig::LEN + // minebtc_dist_config
        32 +                    // raydium_pool_state
        8 +                     // change_faction_fee
        8 +                     // snapshot_interval
        1 +                     // rpg_progression
        1 +                     // bump
        1 +                     // treasury_bump
        4 + (MAX_FACTIONS * (4 + MAX_FACTION_NAME_LENGTH)); // supported_factions vec
}

/// ------------ DOGE-BTC MINING ------------

/// Price entry for tracking historical prices
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PriceEntry {
    /// Timestamp when this price was recorded
    pub timestamp: i64,
    /// Price in SOL per MINE_BTC (scaled by 10^9 for full precision)
    /// This matches SOL's decimal precision for accurate price tracking
    pub price: u64,
}

impl PriceEntry {
    pub const LEN: usize = 8 + 8; // timestamp + price
}

/// Protocol Owned Liquidity tracking for comprehensive POL metrics
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct ProtocolOwnedLiquidity {
    /// Total LP tokens burned (accumulated)
    pub total_lp_burnt: u64,
    /// Total SOL added to liquidity pool (accumulated)
    pub total_sol_added: u64,
    /// Total MINE_BTC added to liquidity pool (accumulated)
    pub total_minebtc_added: u64,
    /// Number of LP addition operations performed
    pub lp_operations_count: u32,
}

impl ProtocolOwnedLiquidity {
    pub const LEN: usize = 8 + 8 + 8 + 4; // 28 bytes

    /// Update POL stats after a successful LP addition and burn
    pub fn update_after_lp_operation(
        &mut self,
        lp_tokens_burnt: u64,
        sol_added: u64,
        minebtc_added: u64,
    ) {
        // Update cumulative totals
        self.total_lp_burnt = self.total_lp_burnt.saturating_add(lp_tokens_burnt);
        self.total_sol_added = self.total_sol_added.saturating_add(sol_added);
        self.total_minebtc_added = self.total_minebtc_added.saturating_add(minebtc_added);
        self.lp_operations_count = self.lp_operations_count.saturating_add(1);
    }
}

/// Doge-BTC Mining status and parameters
#[account]
pub struct MineBtcMining {
    /// Token vault that holds all pre-minted tokens
    pub minebtc_token_vault: Pubkey,
    /// Timestamp of the mining start
    pub mining_start_timestamp: u64,
    /// MineBtc mined per slot (original base rate)
    pub mine_btc_per_round: u64,

    /// Total tokens mined so far
    pub total_tokens_mined: u64,
    /// Total tokens distributed so far
    pub total_tokens_distributed: u64,

    /// Bump for PDA derivation
    pub bump: u8,
    /// Bump for vault authority PDA derivation
    pub vault_auth_bump: u8,

    // ===== DYNAMIC DISTRIBUTION FIELDS =====
    /// Raydium pool state for MINE_BTC-SOL trading
    pub raydium_pool_state: Pubkey,
    /// Last time distribution rate was updated (timestamp)
    pub last_rate_update: i64,
    /// Price history for 4-hour rolling average (8 entries, 1 per 30 mins)
    pub price_history: Vec<PriceEntry>,
    /// Recent price (last snapshot, used for comparison)
    pub recent_price: u64,
    /// Track price (price when last rate change actually happened)
    pub track_price: u64,
    /// SOL amount reserved for Protocol Owned Liquidity (tracked but stored in pda_sol_treasury)
    pub sol_for_pol: u64,
    /// Protocol Owned Liquidity tracking
    pub pol_stats: ProtocolOwnedLiquidity,
    /// LP token price in SOL (9-decimal precision, updated during oracle updates)
    pub lp_token_price_in_sol: u64,

    // ===== EMISSION ADJUSTMENT PARAMETERS =====
    /// Price change threshold percentage (e.g., 3 = 3%) - rate changes only if price moves beyond this
    pub price_change_threshold: u64,
    /// Emission increase percentage when price goes up (e.g., 1 = 1% increase)
    pub emission_increase_pct: u64,
    /// Emission decrease percentage when price goes down (e.g., 3 = 3% decrease)
    pub emission_decrease_pct: u64,

    // ===== LP OPERATION STATE =====
    /// Flag indicating LP operation is pending after rate update
    pub lp_operation_pending: bool,
}

impl MineBtcMining {
    // discriminator + minebtc_token_vault + mining_start_timestamp + mine_btc_per_round + total_tokens_mined + bump + vault_auth_bump +
    // raydium_pool_state + last_rate_update + price_history (vec) + recent_price + track_price + sol_for_pol + pol_stats + lp_token_price_in_sol
    pub const MAX_PRICE_HISTORY_ENTRIES: usize = 8; // 4-hour cycle (8 × 30min snapshots)
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 32                    // minebtc_token_vault
        + 8                     // mining_start_timestamp
        + 8                     // mine_btc_per_round
        + 8                     // total_tokens_mined
        + 8                     // total_tokens_distributed
        + 1                     // bump
        + 1                     // vault_auth_bump
        + 32                    // raydium_pool_state
        + 8                     // last_rate_update (i64)
        + (4 + Self::MAX_PRICE_HISTORY_ENTRIES * PriceEntry::LEN) // price_history Vec<PriceEntry>
        + 8                     // recent_price
        + 8                     // track_price
        + 8                     // sol_for_pol
        + ProtocolOwnedLiquidity::LEN // pol_stats
        + 8                     // lp_token_price_in_sol
        + 8                     // price_change_threshold
        + 8                     // emission_increase_pct
        + 8                     // emission_decrease_pct
        + 1; // lp_operation_pending
}

/// Buybacks account that accumulates SOL for token buybacks
#[account]
pub struct BuybacksAccount {
    /// Total SOL accumulated for buybacks (in lamports)
    pub total_sol_accumulated: u64,
    /// Total SOL used for buybacks (in lamports)
    pub total_sol_used: u64,
    /// SOL earnmarked for Protocol Owned Liquidity (in lamports)
    pub sol_for_pol: u64,
    /// Bump for PDA derivation
    pub bump: u8,
    /// Bump for SOL vault PDA derivation
    pub sol_vault_bump: u8,
}

impl BuybacksAccount {
    // discriminator + total_sol_accumulated + total_sol_used + sol_for_pol + bump + sol_vault_bump
    pub const LEN: usize = DISCRIMINATOR_SIZE + 8 + 8 + 8 + 1 + 1;
}

/// ------------ HASHPOWER CONFIG ------------

/// Hashpower configuration for the Minebtc program
#[account]
pub struct HashpowerConfig {
    /// Minimum lockup period in days
    pub min_lockup_days: u64,
    /// Maximum lockup period in days
    pub max_lockup_days: u64,

    /// Base multiplier for lockup duration (100 = 1x, separate from BASE_MULTIPLIER=1000 used for doges)
    pub base_multiplier: u16,
    /// Maximum multiplier for longest lockup (e.g., 900 = 9x for 3 years)
    pub max_multiplier: u16,

    /// Bump for PDA derivation
    pub bump: u8,
}

// For HashpowerConfig
impl HashpowerConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +     // min_lockup_days
        8 +     // max_lockup_days
        2 +     // base_multiplier (u16)
        2 +     // max_multiplier (u16)
        1; // bump
}

// ModuleInstance and ModuleRuntimeState removed - no longer needed for Faction Surge system

/// Ticket tier option for doge minting
/// When users mint doges, they choose a ticket tier which gives them free tickets
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TicketTier {
    /// Ticket value in lamports (e.g., 10_000_000 = 0.01 SOL)
    pub ticket_value: u64,
}

impl TicketTier {
    pub const LEN: usize = 8 + 2; // ticket_value
}

/// Global doge configuration
#[account]
pub struct DogeConfig {
    pub bump: u8,

    /// Whether the mining of doges is currently active
    pub is_active: bool,

    /// Doge collection address (Metaplex Core)
    pub doge_collection: Pubkey,

    /// Maximum supply of doges that can be minted
    pub max_supply: u64,

    /// Number of doges minted so far
    pub doges_minted: u64,

    /// Base price for bonding curve (in lamports)
    pub base_price: u64,

    /// Curve steepness parameter (controls price growth rate, typically >= 100)
    pub curve_a: u64,

    /// Available ticket tier configs users can choose when minting (max 4 options)
    /// Example: 0.01 SOL × 1000 tickets, 0.1 SOL × 10 tickets
    pub ticket_tiers: Vec<TicketTier>,

    /// Whether breeding is currently allowed
    pub breeding_allowed: bool,
    /// Base price for breeding cost bonding curve (in lamports)
    pub breed_base_price: u64,
    /// Curve steepness for breeding cost
    pub breed_curve_a: u64,
}

impl DogeConfig {
    pub const MAX_TICKET_TIERS: usize = 3;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        32 +    // doge_collection
        8 +     // max_supply
        8 +     // doges_minted
        8 +     // base_price
        8 +     // curve_a
        4 + (Self::MAX_TICKET_TIERS * TicketTier::LEN) + // ticket_tiers
        1 +     // breeding_allowed
        8 +     // breed_base_price
        8; // breed_curve_a
}

/// Per-user whitelist allowance for free Doge mints.
/// The whitelisted user still pays transaction/account rent, but not the mint fee.
#[account]
pub struct DogeFreeMintAllowance {
    pub user: Pubkey,
    pub remaining_free_mints: u8,
    pub bump: u8,
}

impl DogeFreeMintAllowance {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // user
        1 +     // remaining_free_mints
        1; // bump
}

// ========================================================================================
// ============================= TAX CONFIG ACCOUNT ==============================
// ========================================================================================

/// Tax Configuration PDA (Seed: `[b"tax-config"]`)
/// Manages tax distribution, faction treasury rewards, and NFT floor sweep operations
#[account]
pub struct TaxConfig {
    pub bump: u8,

    /// Percentage of withheld tax that goes to NFT floor sweep
    pub nft_floor_sweep_pct: u8,
    /// Percentage of withheld tax that goes to faction treasury
    pub faction_treasury_pct: u8,
    /// Percentage of withheld tax that gets burned (remainder goes back to vault)
    pub burn_pct: u8,

    /// Total amount of MineBtc burnt so far (cumulative)
    pub total_burnt: u64,

    /// Current distribution round state
    pub round_active: bool,
    /// Timestamp when current distribution round started
    pub start_timestamp: i64,
    /// Timestamp when last distribution round ended (for 1-day cooldown)
    pub end_timestamp: i64,

    /// Leaderboard state: faction IDs ranked by hashpower (index = rank, value = faction_id)
    /// Rank 0 = highest hashpower, Rank 11 = lowest hashpower
    pub leaderboard_faction_ids: Vec<u8>,
    /// Leaderboard hashpower values (index = rank, value = hashpower)
    pub leaderboard_hashpower: Vec<u64>,
    /// Number of factions added to leaderboard so far (0-active_faction_count)
    pub leaderboard_factions_count: u8,
    /// Snapshot of active faction count when the current distribution round started
    pub distribution_faction_count: u8,

    /// Faction rewards: MineBtc amount each faction gets (index = rank, value = minebtc_amount)
    pub faction_rewards: Vec<u64>,
    /// Whether rewards have been calculated for current round
    pub rewards_calculated: bool,

    /// Faction claim status: whether each faction has claimed rewards (index = faction_id, value = claimed)
    pub faction_claimed: Vec<bool>,
    /// Number of factions that have claimed rewards
    pub factions_claimed_count: u8,

    /// PDA addresses for tax system
    pub withdraw_withheld_authority: Pubkey,
    pub faction_treasury_vault: Pubkey,
    pub nft_floor_sweep_vault: Pubkey,
    pub nft_sale_sol_vault: Pubkey,

    /// Whitelisted address that can withdraw MineBtc from NFT floor sweep vault
    /// This address will swap MineBtc for SOL off-chain, buy NFTs, and re-list them
    pub nft_floor_sweep_whitelisted_address: Pubkey,
}

impl TaxConfig {
    pub const DISTRIBUTION_COOLDOWN_SECONDS: i64 = DAY_IN_SECONDS as i64; // 1 day

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // nft_floor_sweep_pct
        1 +     // faction_treasury_pct
        1 +     // burn_pct
        8 +     // total_burnt
        1 +     // round_active (bool)
        8 +     // start_timestamp (i64)
        8 +     // end_timestamp (i64)
        4 + (MAX_FACTIONS * 1) + // leaderboard_faction_ids Vec<u8>
        4 + (MAX_FACTIONS * 8) + // leaderboard_hashpower Vec<u64>
        1 +     // leaderboard_factions_count
        1 +     // distribution_faction_count
        4 + (MAX_FACTIONS * 8) + // faction_rewards Vec<u64>
        1 +     // rewards_calculated (bool)
        4 + (MAX_FACTIONS * 1) + // faction_claimed Vec<bool>
        1 +     // factions_claimed_count
        32 +    // withdraw_withheld_authority
        32 +    // faction_treasury_vault
        32 +    // nft_floor_sweep_vault
        32 +    // nft_sale_sol_vault
        32; // nft_floor_sweep_whitelisted_address
}

// ========================================================================================
// ========================== 1. GLOBAL & ORACLE ACCOUNTS =================================
// ========================================================================================

/// Global game state PDA (Seed: `[b"global-surge-state"]`)
/// Tracks global game statistics and the currently active round.
/// Each individual round has its own GameSession PDA.
#[account]
pub struct GlobalGameSate {
    pub bump: u8,

    /// Whether the game is currently active
    pub is_active: bool,
    pub can_begin_round: bool,

    /// Total SOL bets since start of game (cumulative across all rounds)
    pub total_sol_bets: u128,

    /// The currently active round ID (e.g., 48636).
    pub current_round_id: u64,
    /// Round duration in seconds (configurable)
    pub round_duration_seconds: i64,

    // --- Data from the *previous* round (for claiming) ---
    /// The last completed round ID
    pub last_round_id: u64,
    /// The winning faction ID from the last completed round
    pub winning_faction_id: u8,
}

impl GlobalGameSate {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        1 +     // can_begin_round
        16 +    // total_sol_bets (u128)
        8 +     // current_round_id
        8 +     // round_duration_seconds
        8 +     // last_round_id
        1; // winning_faction_id
}

#[account]
pub struct UnrefinedRewards {
    pub unrefining_index: u128,
    pub total_minebtc_claimable: u64,
}

impl UnrefinedRewards {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        16 +    // unrefining_index (u128)
        8; // total_minebtc_claimable (u64)
}

/// Faction State PDA (Seed: `[b"faction", faction_name.as_bytes()]`)
/// Tracks cumulative statistics and reward indexes for a specific faction.
/// One account per faction (up to MAX_FACTIONS factions).
/// Used for calculating staker rewards based on faction performance.
#[account]
pub struct FactionState {
    pub bump: u8,
    /// The faction ID (matching index in supported_factions)
    pub faction_id: u8,

    /// Total passive hashpower from stakers in this faction (cumulative)
    pub total_dogebtc_hashpower: u64,
    pub dogebtc_staked: u64,
    pub dogebtc_dogebtc_reward_index: u128,
    pub dogebtc_sol_reward_index: u128,

    pub total_lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_index: u128,
    pub lp_dogebtc_reward_index: u128,

    pub doges_staked: u64,
    /// Total doges currently being used in gameplay
    pub doges_playing: u64,

    /// Total SOL bet on this faction across all rounds (cumulative)
    pub total_sol_bets: u64,
    /// Total number of rounds this faction has won (cumulative)
    pub total_wins: u64,

    /// Cumulative SOL-per-share this faction has earned for stakers
    /// Used for calculating staker rewards
    pub sol_reward_index: u128,

    /// Current motherlode pot size for this faction
    pub motherlode_pot_size: u64,
}

impl FactionState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // faction_id
        8 +     // total_dogebtc_hashpower (u64)
        8 +     // dogebtc_staked (u64)
        16 +    // dogebtc_dogebtc_reward_index (u128)
        16 +    // dogebtc_sol_reward_index (u128)
        8 +     // total_lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_index (u128)
        16 +    // lp_dogebtc_reward_index (u128)
        8 +     // doges_staked (u64)
        8 +     // doges_playing (u64)
        8 +     // total_sol_bets (u64)
        8 +     // total_wins (u64)
        16 +    // sol_reward_index (u128)
        8; // motherlode_pot_size (u64)
}

// ========================================================================================
// ========================== GAME SESSION ACCOUNTS =================================
// ========================================================================================

/// Game Session PDA (Seed: `[b"game-session", round_id_u64]`)
/// Each round has its own GameSession PDA that tracks:
/// - Round timing (start/end timestamps)
/// - Total bets placed in this round
/// - Per-faction indexes for tracking individual bets
/// - Winning faction
/// - Round-specific reward pools and payout data
/// This account is created when a round starts and finalized when the round ends.
#[account]
pub struct GameSession {
    pub bump: u8,

    // 0 = Active round
    // 1 = Winning faction finalized, pending faction reward distribution
    // 2 = Faction reward distribution finalized
    pub stage: u8,

    /// The round ID this session belongs to
    pub round_id: u64,

    /// Slot when the round started.
    pub round_start_slot: u64,
    pub round_start_timestamp: i64,
    /// Timestamp after which betting is closed.
    pub round_end_timestamp: i64,
    /// Primary future slot whose hash should be used as round entropy.
    pub scheduled_entropy_slot: u64,
    /// Actual slot whose hash was used to derive the winner.
    pub entropy_slot_used: u64,
    /// Stored slot hash used for winner derivation.
    pub entropy_hash: [u8; 32],
    /// Whether the round had to fall back to latest-available slot hash instead of the scheduled one.
    pub used_entropy_fallback: bool,

    /// Total SOL bets placed in this round
    pub total_sol_bets: u64,
    /// Total points bets placed in this round
    pub total_points_bets: u64,
    /// Total weighted points bets (for dogeBTC distribution)
    pub total_wgtd_points_bets: u64,
    /// Total stakers fee paid in this round
    pub stakers_fee: u64,

    /// Number of users who bet on each faction.
    pub user_faction_indexes: [u64; NUM_FACTIONS],
    /// Net SOL bet placed on each faction.
    pub sol_bets_by_faction: [u64; NUM_FACTIONS],
    /// Points bet placed on each faction.
    pub points_bets_by_faction: [u64; NUM_FACTIONS],
    /// Weighted points per faction (points * active_multiplier / BASE_MULTIPLIER for SOL bets, else points).
    pub wgtd_points_bets_by_faction: [u64; NUM_FACTIONS],
    /// Points bet placed on each faction-direction pair.
    pub points_bets_by_faction_direction: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
    /// Weighted points bet placed on each faction-direction pair.
    pub wgtd_points_bets_by_faction_direction: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],

    /// The winning faction ID for this round.
    pub winning_faction_id: u8,
    /// The winning direction for the winning faction (0=Down, 1=Neutral, 2=Up).
    pub winning_direction: u8,

    // --- MineBtc reward pools for this round ---
    /// MineBtc allocated for exact winning faction+direction bettors in this round.
    pub minebtc_winner_pool: u64,
    /// Aggregate MineBtc allocated for the non-winning directions on the winning faction.
    pub minebtc_same_faction_pool: u64,
    /// MineBtc allocated per losing direction on the winning faction.
    /// The winning direction index remains zero in this array.
    pub minebtc_same_faction_direction_pools: [u64; PredictionDirection::COUNT],
    /// MineBtc allocated for stakers in this round
    pub faction_stakers: u64,
    /// MineBtc allocated for motherlode in this round
    pub motherlode_rewards: u64,

    /// SOL rewards index for this round's exact winning faction+direction.
    pub sol_rewards_index: u128,
    /// MineBtc rewards index for this round's exact winning faction+direction.
    pub minebtc_rewards_index: u128,
    // --- Motherlode data for this round ---
    /// Whether motherlode was hit in this round
    pub motherlode_hit: bool,
    /// Motherlode pot size when hit (if applicable)
    pub motherlode_pot_size_on_hit: u64,

    // --- Mutation tracking per round ---
    /// Highest SOL bet placed per faction this round (for mutation probability calc)
    pub highest_sol_bet_per_faction: [u64; NUM_FACTIONS],
    /// Number of mutations that have occurred per faction this round.
    /// More mutations in a faction → harder for the next one (diminishing returns).
    pub mutations_per_faction: [u8; NUM_FACTIONS],
    /// Total mutations across all factions this round.
    /// Capped at active_factions / 3 to create scarcity.
    pub total_mutations_this_round: u8,
}

impl GameSession {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // stage (u8)
        8 +     // round_id
        8 +     // round_start_slot
        8 +     // round_start_timestamp (i64)
        8 +     // round_end_timestamp (i64)
        8 +     // scheduled_entropy_slot
        8 +     // entropy_slot_used
        32 +    // entropy_hash
        1 +     // used_entropy_fallback
        8 +     // total_sol_bets
        8 +     // total_points_bets
        8 +     // total_wgtd_points_bets
        8 +     // stakers_fee
        (NUM_FACTIONS * 8) + // user_faction_indexes
        (NUM_FACTIONS * 8) + // sol_bets_by_faction
        (NUM_FACTIONS * 8) + // points_bets_by_faction
        (NUM_FACTIONS * 8) + // wgtd_points_bets_by_faction
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // points_bets_by_faction_direction
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // wgtd_points_bets_by_faction_direction
        1 +     // winning_faction_id (u8)
        1 +     // winning_direction (u8)
        8 +     // minebtc_winner_pool
        8 +     // minebtc_same_faction_pool
        (PredictionDirection::COUNT * 8) + // minebtc_same_faction_direction_pools
        8 +     // faction_stakers
        8 +     // motherlode_rewards
        16 +    // sol_rewards_index
        16 +    // minebtc_rewards_index
        1 +     // motherlode_hit
        8 +     // motherlode_pot_size_on_hit
        (NUM_FACTIONS * 8) + // highest_sol_bet_per_faction
        (NUM_FACTIONS * 1) + // mutations_per_faction
        1; // total_mutations_this_round
}

// ========================================================================================
// ============================= 2. USER-SPECIFIC ACCOUNTS ==============================
// ========================================================================================

/// Player Data PDA (Seed: `[b"player", user_pubkey]`)
/// Persistent account for each player that tracks:
/// - Player statistics (rounds played, won, total bets/winnings)
/// - List of rounds the player participated in (for tracking unclaimed rewards)
/// - Passive staking data (hashpower, reward indexes)
/// Each user bet in a round has its own UserGameBet PDA, referenced here via round IDs.
#[account]
pub struct PlayerData {
    pub bump: u8,

    /// The user's wallet address
    pub owner: Pubkey,

    /// Whether third-party bots may claim rewards on this player's behalf.
    pub allow_bots_to_claim: bool,

    /// Referral code used by this player
    pub referral_code: Pubkey,

    /// The faction this player is assigned to
    pub faction_id: u8,

    /// Cumulative statistics
    pub rounds_played: u64,

    pub total_sol_bet: u64,
    pub total_points_bet: u64,

    pub total_sol_won: u64,
    pub total_dogebtc_won: u64,

    pub dogebtc_hashpower: u64,
    pub dogebtc_staked: u64,
    pub dogebtc_dogebtc_reward_debt: u128,
    pub dogebtc_sol_reward_debt: u128,

    pub lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_debt: u128,
    pub lp_dogebtc_reward_debt: u128,

    pub pending_sol_rewards: u64,
    pub unrefining_index: u128,
    pub pending_minebtc_rewards: u64,
    pub unrefined_minebtc_rewards: u64,
    /// Number of unclaimed per-round reward accounts still outstanding.
    pub pending_round_claims: u16,
    /// Number of unclaimed per-epoch reward accounts still outstanding.
    pub pending_epoch_claims: u16,

    pub dogebtc_position_indices: Vec<u8>,
    pub lp_position_indices: Vec<u8>,

    /// Staked dragon doges (max 5 doges)
    /// Stores the mint addresses of staked doges
    pub staked_doges: Vec<Pubkey>,
    /// Current doge multiplier (1000 = 1x, 1500 = 1.5x, etc.)
    /// Effective player multiplier after applying the MAX_MULTIPLIER cap.
    pub doge_multiplier: u16,

    /// Free tickets: points size of each ticket type (max 5 ticket types)
    /// Example: [10000000, 100000000, ...] where 1 point = 1 SOL lamport
    /// So 10000000 = 0.01 SOL, 100000000 = 0.1 SOL
    pub free_tickets: Vec<u64>,
    /// Free tickets remaining: count of each ticket type remaining
    /// Index matches free_tickets (e.g., free_tickets_remaining[0] is count for free_tickets[0])
    pub free_tickets_remaining: Vec<u64>,

    /// Doge currently being used in gameplay (Pubkey::default() if none)
    pub gameplay_doge: Pubkey,
    /// Active gameplay multiplier (1000 = 1x, set from gameplay doge's multiplier, reset to BASE_MULTIPLIER on withdraw)
    pub active_multiplier: u32,
    /// Cached DNA of gameplay doge (for mutation calculations without loading DogeMetadata)
    pub gameplay_doge_dna: [u8; 32],
    /// Cached XP of gameplay doge (updated during gameplay, synced to DogeMetadata on withdraw)
    pub gameplay_doge_xp: u32,
    /// Epoch ID in which the user requested gameplay unlock.
    /// The doge can only be withdrawn once the next epoch/campaign cycle begins.
    pub gameplay_unlock_request_epoch: u64,
}

impl PlayerData {
    // Maximum number of active rounds a player can track (for Vec sizing)
    pub const MAX_ACTIVE_ROUNDS: usize = 100; // Reasonable limit for unclaimed rounds
                                              // Maximum number of ticket types (max 5 ticket types)
    pub const MAX_TICKET_TYPES: usize = 5;

    // Maximum number of staking positions per user
    pub const MAX_POSITIONS: usize = 7; // 0-6 positions

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // owner
        1 +     // allow_bots_to_claim
        32 +    // referral_code
        1 +     // faction_id
        8 +     // rounds_played
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_sol_won
        8 +     // total_dogebtc_won
        8 +     // dogebtc_hashpower (u64)
        8 +     // dogebtc_staked (u64)
        16 +    // dogebtc_dogebtc_reward_debt (u128)
        16 +    // dogebtc_sol_reward_debt (u128)
        8 +     // lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_debt (u128)
        16 +    // lp_dogebtc_reward_debt (u128)
        8 +     // pending_sol_rewards (u64)
        16 +    // unrefining_index (u128)
        8 +     // pending_minebtc_rewards (u64)
        8 +     // unrefined_minebtc_rewards (u64)
        2 +     // pending_round_claims (u16)
        2 +     // pending_epoch_claims (u16)
        4 + (Self::MAX_POSITIONS * 1) + // dogebtc_position_indices Vec<u8>
        4 + (Self::MAX_POSITIONS * 1) + // lp_position_indices Vec<u8>
        4 + (MAX_STAKED_DOGES * 32) + // staked_doges Vec<Pubkey>
        2 +     // doge_multiplier (u16)
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets Vec<u64>
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets_remaining Vec<u64>
        32 +    // gameplay_doge
        4 +     // active_multiplier (u32)
        32 +    // gameplay_doge_dna [u8; 32]
        4 +     // gameplay_doge_xp (u32)
        8; // gameplay_unlock_request_epoch (u64)
}

/// Individual MineBtc staking position
#[account]
pub struct StakedPosition {
    pub position_type: u8, // 0 = minebtc, 1 = lp

    pub position_index: u8,
    pub faction_id: u8,

    /// Staking details
    pub staked_amount: u64,
    pub weighted_amount: u64,
    pub start_timestamp: i64,
    pub lockup_end_timestamp: i64,
    pub lockup_duration: u64, // in days
    pub multiplier: u16,      // 100 = 1x
    pub bump: u8,
}

impl StakedPosition {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +  // position_type
        1 +  // position_index
        1 +  // faction_id
        8 +  // staked_amount
        8 +  // weighted_amount
        8 +  // start_timestamp
        8 +  // lockup_end_timestamp
        8 +  // lockup_duration
        2 +  // multiplier
        1; // bump
}

/// Stores referral rewards that a user has earned from referrals
#[account]
pub struct ReferralRewards {
    pub owner: Pubkey,
    pub bump: u8,
    /// Number of users who have used this user's referral code
    pub referrals_count: u16,

    /// Pending MineBtc rewards from referrals (claimable)
    pub pending_minebtc_rewards: u64,

    /// Total MineBtc earned from referrals (cumulative)
    pub total_minebtc_earned: u64,

    /// Pending SOL rewards from NFT mint/breed referral commissions (for stats tracking)
    pub pending_sol_rewards: u64,

    /// Total SOL earned from NFT mint/breed referral commissions (cumulative)
    pub total_sol_earned: u64,
}

impl ReferralRewards {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // bump
        2 +     // referrals_count
        8 +     // pending_minebtc_rewards
        8 +     // total_minebtc_earned
        8 +     // pending_sol_rewards
        8; // total_sol_earned
}

// ========================================================================================
// ===============================  DOGE NFT METADATA ===============================
// ========================================================================================

/// Doge NFT metadata (stored in minebtc program for simplicity)
#[account]
pub struct DogeMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    /// Parent 1 mint (Pubkey::default() for genesis doges)
    pub mom: Pubkey,
    /// Parent 2 mint (Pubkey::default() for genesis doges)
    pub dad: Pubkey,
    /// Number of times this doge has bred (max 5)
    pub breed_count: u8,
    /// Unix timestamp when cooldown ends (can breed again after this)
    pub cooldown_end: i64,
    /// Creation timestamp
    pub created_at: i64,
    /// Faction ID (country) that the doge belongs to (matches minebtc faction)
    pub faction_id: u8,
    /// Multiplier for this doge (1000 = 1x, same scale as BASE_MULTIPLIER)
    pub multiplier: u32,
    /// dogeBTC accumulated which can be claimed by sending this doge to heaven
    pub accumulated_val: u64,
    /// DNA data (32 bytes for breeding/evolution)
    pub dna: [u8; 32],
    /// The Player who is incubating this doge. Pubkey::default() if not incubated.
    pub incubated_player_data: Pubkey,
    /// Last power update timestamp
    pub last_update_ts: i64,
    /// Experience points, reset to 0 on evolution
    pub xp: u32,
    /// PDA bump
    pub bump: u8,
}

impl DogeMetadata {
    pub const MAX_BREED_COUNT: u8 = 5;

    /// Cooldown times in seconds: [0h, 24h, 72h, 120h, 336h]
    pub const COOLDOWNS: [i64; 5] = [0, 86400, 259200, 432000, 1209600];

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // mint
        32 +    // mom
        32 +    // dad
        1 +     // breed_count
        8 +     // cooldown_end
        8 +     // created_at
        1 +     // faction_id
        4 +     // multiplier
        8 +     // accumulated_val
        32 +    // dna
        32 +    // incubated_player_data
        8 +     // last_update_ts
        4 +     // xp
        1; // bump
}

// ========================================================================================
// ============================= BET TYPE ENUM ==============================
// ========================================================================================

/// Directional stance for faction and index prediction markets.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionDirection {
    Down,
    Neutral,
    Up,
}

impl PredictionDirection {
    pub const LEN: usize = 1;
    pub const COUNT: usize = 3;

    pub fn as_index(self) -> usize {
        match self {
            Self::Down => 0,
            Self::Neutral => 1,
            Self::Up => 2,
        }
    }
}

/// Bet type enum for user bets.
/// Each bet selects a faction and a direction for the active epoch market.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum BetType {
    FactionDirection {
        faction_id: u8,
        direction: PredictionDirection,
    },
}

impl BetType {
    // Anchor enum serialization: 1 byte discriminator + 1 byte faction_id + 1 byte direction.
    pub const LEN: usize = 3;
}

// ========================================================================================
// ============================= FACTION SURGE ACCOUNTS ==============================
// ========================================================================================

/// User Game Bet PDA (Seed: `[b"user-bet", user_pubkey, round_id_u64]`)
/// Each user bet in a round has its own PDA account.
/// Users can bet on multiple faction-direction positions in a single round,
/// including multiple directions on the same faction.
///
/// Structure:
/// - `faction_ids`: List of factions user bet on
/// - `directions`: Direction chosen for each faction (0=Down, 1=Neutral, 2=Up)
/// - `sol_bets`: SOL bets for each faction (index matches faction_ids)
/// - `points_bets`: Points bets for each faction (index matches faction_ids)
/// - `total_sol_bet`: Total SOL bet across all factions
/// - `total_points_bet`: Total points bet across all factions
/// - `total_fee`: Total fees paid
#[account]
pub struct UserGameBet {
    /// The user who placed this bet
    pub owner: Pubkey,
    /// The round ID this bet belongs to
    pub round_id: u64,

    /// List of faction IDs user bet on.
    /// Index position corresponds to the same index in directions/sol_bets/points_bets.
    pub faction_ids: Vec<u8>,
    /// Direction chosen for each faction (0=Down, 1=Neutral, 2=Up).
    pub directions: Vec<u8>,

    /// SOL bets for each faction (index matches faction_ids)
    pub sol_bets: Vec<u64>,
    /// Points bets for each faction (index matches faction_ids)
    pub points_bets: Vec<u64>,
    /// Weighted points for each faction (points * multiplier / 100 for SOL, else points) - for dogeBTC
    pub wgtd_points_bets: Vec<u64>,

    /// Total SOL amount bet across all factions (after protocol fee deduction)
    pub total_sol_bet: u64,
    /// Total points amount bet across all factions
    pub total_points_bet: u64,
    /// Total weighted points (for dogeBTC rewards)
    pub total_wgtd_points_bet: u64,

    /// Total fees paid across all bets
    pub total_fee: u64,
    pub gameplay_doge: Pubkey,

    pub bump: u8,

    // --- Instant Mutation (applied during claim_rewards) ---
    /// 0 = no mutation, 1 = Evolution, 2 = Power, 3 = Trait
    pub mutation_type: u8,
    /// Whether this bet has been accumulated into epoch bets
    pub epoch_accumulated: bool,
}

impl UserGameBet {
    // Maximum number of faction-direction positions a user can bet on in a single round.
    pub const MAX_POSITIONS_PER_BET: usize = NUM_FACTIONS * PredictionDirection::COUNT;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        8 +     // round_id
        4 + (Self::MAX_POSITIONS_PER_BET * 1) + // faction_ids Vec<u8>
        4 + (Self::MAX_POSITIONS_PER_BET * 1) + // directions Vec<u8>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // sol_bets Vec<u64>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // points_bets Vec<u64>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // wgtd_points_bets Vec<u64>
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_wgtd_points_bet
        8 +     // total_fee
        32 +     // gameplay_doge
        1 +     // bump
        1 +     // mutation_type
        1; // epoch_accumulated
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutominerFactionPick {
    pub faction_id: u8,
    pub direction: PredictionDirection,
}

/// Autominer configuration for factions
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum FactionsConfig {
    /// Specific list of faction-direction picks.
    Specific { picks: Vec<AutominerFactionPick> },
    /// Random number of factions with one shared directional stance.
    Random {
        count: u8,
        direction: PredictionDirection,
    },
}

/// Autominer Vault PDA (Seed: `[b"autominer", user_pubkey]`)
/// Stores autominer configuration for a user; funds are held in the global autominer custody PDA
/// Allows users to configure automatic faction-direction betting.
#[account]
pub struct AutominerVault {
    pub owner: Pubkey,
    /// Factions configuration (specific list or random count with direction) - optional
    pub factions_config: Option<FactionsConfig>,
    /// Total SOL budget per round in SOL mode.
    /// This includes keeper compensation plus the budget distributed across generated bets.
    /// Must be 0 in ticket mode.
    pub sol_per_round: u64,
    /// Number of rounds remaining (decremented after each round)
    pub rounds_remaining: u32,
    /// Last round ID where bets were placed (to prevent duplicate bets)
    pub last_bet_round_id: u64,
    pub vault_bump: u8,
    /// Remaining SOL balance reserved for this autominer (held in autominer custody PDA)
    pub sol_balance: u64,

    /// If set to true, SOL rewards can be used to reload Autominer and continue mining dogeBTC
    pub can_reload: bool,

    /// Optional ticket tier index. If Some, autominer uses tickets instead of SOL for bets.
    /// Ticket mode does not reserve SOL and does not pay keeper compensation.
    /// Bet amount is determined by the ticket value in player_data.free_tickets[tier].
    pub use_ticket: Option<u8>,
}

impl AutominerVault {
    pub const MAX_PICKS: usize = NUM_FACTIONS * PredictionDirection::COUNT;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        // factions_config Option<FactionsConfig>
        // Option discriminator: 1 byte
        // Max variant: Specific { picks: Vec<AutominerFactionPick> }.
        1 + (1 + 4 + (Self::MAX_PICKS * 2)) + // factions_config Option<FactionsConfig>
        8 +     // sol_per_round
        4 +     // rounds_remaining (u32)
        8 +     // last_bet_round_id
        1 +     // vault_bump
        8 +     // sol_balance
        1 +     // can_reload (bool)
        1 + 1; // use_ticket Option<u8> (1 byte discriminator + 1 byte value)
}

// ========================================================================================
// ============================= EPOCH MINING ACCOUNTS ==============================
// ========================================================================================

/// Maximum number of score update entries stored per epoch (for audit trail)
/// Epoch Configuration PDA (Seed: `[b"epoch-config"]`)
/// Epochs are tied to the economy cycle: one epoch per LP-burn cycle.
/// Settlement becomes possible once lp_operations_count reaches epoch_settle_cycle.
#[account]
pub struct EpochConfig {
    pub bump: u8,

    /// Current epoch ID (incrementing counter, starts at 1)
    pub current_epoch_id: u64,

    /// Whether epoch mining is active
    pub is_active: bool,

    /// The LP operations count that triggers settlement of the current epoch.
    /// Set to `pol_stats.lp_operations_count + 1` when the epoch starts,
    /// meaning the epoch settles after the next full economy cycle completes.
    pub epoch_settle_cycle: u32,

    /// Rankings from the previous epoch's mutation scores.
    /// Used as start_ranks when the next epoch auto-starts.
    /// Initialized to [0, 1, 2, ..., NUM_FACTIONS-1] on first setup.
    pub prev_epoch_mutation_ranks: [u8; NUM_FACTIONS],
}

impl EpochConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // current_epoch_id
        1 +     // is_active
        4 +     // epoch_settle_cycle
        (NUM_FACTIONS * 1); // prev_epoch_mutation_ranks
}

/// Epoch State PDA (Seed: `[b"epoch", epoch_id_u64_le]`)
/// Tracks a single mutation-driven prediction epoch: start/final ranks derived from
/// doge mutation scores, directional bet totals, and settlement outputs.
/// Epoch duration is tied to the economy cycle (one LP-burn cycle).
#[account]
pub struct EpochState {
    pub bump: u8,

    /// Epoch ID
    pub epoch_id: u64,
    /// Timestamp when this epoch was auto-started
    pub start_timestamp: u64,

    /// Stage: 0 = active, 1 = settled (claims open)
    pub stage: u8,
    /// Snapshot of how many factions were active when this epoch started
    pub active_faction_count: u8,

    /// Total dogeBTC mined via raffle rounds during this epoch.
    pub total_dogebtc_mined_in_epoch: u64,
    /// Epoch mining pool distributed to epoch predictors.
    pub epoch_mining_pool: u64,

    /// Rank snapshot from previous epoch (baseline for direction resolution).
    pub start_ranks: [u8; NUM_FACTIONS],
    /// Final ranks derived from faction_mutation_scores at settlement.
    pub final_ranks: [u8; NUM_FACTIONS],

    /// Rank deltas at settlement (positive = rank improved, negative = rank worsened).
    pub rank_deltas: [i8; NUM_FACTIONS],
    /// Resolved direction per faction (0=Down, 1=Neutral, 2=Up).
    pub resolved_directions: [u8; NUM_FACTIONS],

    /// Total weighted bets per faction and direction during this epoch (own-faction only).
    pub faction_direction_totals: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],

    /// Pre-computed reward pool per faction (proportional to winning-direction bet weight).
    pub faction_reward_pools: [u64; NUM_FACTIONS],
    /// 10% reward pool per faction reserved for gameplay doges that mutated during the epoch.
    pub faction_doge_reward_pools: [u64; NUM_FACTIONS],

    /// Accumulated mutation scores per faction during this epoch.
    /// Drives ranking at settlement: factions with higher mutation scores rank higher.
    /// Score = sum of (type_weight × bet_size × doge_multiplier) for every mutation that fired.
    pub faction_mutation_scores: [u64; NUM_FACTIONS],
    /// Total weighted bets per faction/direction from users whose gameplay doge mutated this epoch.
    pub eligible_doge_direction_totals: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
}

impl EpochState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // epoch_id
        8 +     // start_timestamp
        1 +     // stage
        1 +     // active_faction_count
        8 +     // total_dogebtc_mined_in_epoch
        8 +     // epoch_mining_pool
        (NUM_FACTIONS * 1) + // start_ranks
        (NUM_FACTIONS * 1) + // final_ranks
        (NUM_FACTIONS * 1) + // rank_deltas
        (NUM_FACTIONS * 1) + // resolved_directions
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // faction_direction_totals
        (NUM_FACTIONS * 8) + // faction_reward_pools
        (NUM_FACTIONS * 8) + // faction_doge_reward_pools
        (NUM_FACTIONS * 8) + // faction_mutation_scores
        (NUM_FACTIONS * PredictionDirection::COUNT * 8); // eligible_doge_direction_totals
}

/// User Epoch Bets PDA (Seed: `[b"user-epoch", user_pubkey, epoch_id_u64_le]`)
/// Tracks how much weighted stake a user bet on their own faction's direction during a specific epoch.
/// Only own-faction bets are accumulated (cross-faction bets only count for round rewards).
#[account]
pub struct UserEpochBets {
    pub bump: u8,

    /// The user who placed these bets
    pub owner: Pubkey,
    /// The epoch ID this tracks
    pub epoch_id: u64,
    /// Gameplay doge that became eligible for the epoch doge-reward pool.
    pub gameplay_doge: Pubkey,
    /// Whether this user's gameplay doge mutated/evolved during the epoch.
    pub doge_bonus_eligible: bool,

    /// Weighted bet per faction and direction during this epoch.
    pub direction_bets: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
}

impl UserEpochBets {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // owner
        8 +     // epoch_id
        32 +    // gameplay_doge
        1 +     // doge_bonus_eligible
        (NUM_FACTIONS * PredictionDirection::COUNT * 8); // direction_bets
}
