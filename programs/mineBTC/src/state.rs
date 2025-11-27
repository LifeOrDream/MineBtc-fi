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
// - `EggConfig`: Configuration for the Doge NFT system.
// - `TaxConfig`: Configuration for the tax and burn system.
//

use anchor_lang::prelude::*;

pub const MINEBTC_DECIMALS: u8 = 6;
pub const THIRTY_MINS: u64 = 5; //  1800; // 30 minutes in seconds
pub const FOUR_HOURS: u64 = 90; //  14400; // 4 hours in seconds

pub const BASE_MULTIPLIER: u32 = 1000; // 1.0x
pub const MAX_BASE_CHANCE: u64 = 3000; // 30%

/// ------------ CONSTANTS ------------

pub const DAY_IN_SECONDS: u64 = 86400;
pub const BURN_TAX_PERCENTAGE: u64 = 1; // 1% burn tax on transfers

pub const MAX_ALLOWED_POSITIONS: u8 = 7;
pub const EMERGENCY_WITHDRAWAL_PENALTY_PCT: u8 = 15;
pub const M_HUNDRED: u64 = 100;

// ========== DECIMAL SCALING CONSTANTS ========== //

pub const INDEX_PRECISION: u64 = 1_000_000; // 1 million
pub const DISCRIMINATOR_SIZE: usize = 8;

// ========== FACTION SURGE RAFFLE CONSTANTS ========== //

pub const MOTHERLODE_CHANCE: u64 = 625; // 1 in 625 chance (0.16%)

pub const MAX_FACTIONS: usize = 12; // 12 factions for the raffle
pub const NUM_FACTIONS: usize = 12; // Same as MAX_FACTIONS, used for array sizes
pub const MAX_FACTION_NAME_LENGTH: usize = 16; // Maximum length of faction name

// ========== BLOCK RAFFLE CONSTANTS ========== //
pub const NUM_BLOCKS: usize = 24; // 24 blocks total
pub const BLOCKS_PER_FACTION: usize = 2; // Each faction gets 2 blocks
pub const MAX_CRANKER_BOTS: usize = 3; // Maximum number of whitelisted cranker bots

// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / MineBtcMining state
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";
pub const HASHPOWER_CONFIG_SEED: &[u8] = b"hashpower-config";
pub const MINE_BTC_MINING_SEED: &[u8] = b"mine-btc-mining";
pub const UNREFINED_REWARDS_SEED: &[u8] = b"unrefined-rewards";

// PDAs which hold SOL collected by the program
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";
pub const DOGES_TREASURY_SEED: &[u8] = b"eggs-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
pub const MINE_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"minebtc-vault-authority";
pub const MINE_BTC_VAULT_SEED: &[u8] = b"minebtc_vault";

pub const REFERRAL_REWARDS_SEED: &[u8] = b"referral-rewards";
pub const COLLECTION_AUTHORITY_SEED: &[u8] = b"collection_authority";

// PDAs for Doge NFT system
pub const DOGE_METADATA_SEED: &[u8] = b"doge-metadata";
pub const DOGE_CUSTODY_SEED: &[u8] = b"doge-custody"; // PDA that holds locked NFTs

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
pub const DOGE_CONFIG_SEED: &[u8] = b"egg-config";

// PDAs for Tax system
pub const TAX_CONFIG_SEED: &[u8] = b"tax-config";
pub const WITHDRAW_WITHHELD_AUTHORITY_SEED: &[u8] = b"withdraw-withheld-authority";
pub const FACTION_TREASURY_VAULT_SEED: &[u8] = b"faction-treasury-vault";
pub const NFT_FLOOR_SWEEP_VAULT_SEED: &[u8] = b"nft-floor-sweep-vault";
pub const NFT_SALE_SOL_VAULT_SEED: &[u8] = b"nft-sale-sol-vault";

// ==========  DOGE NFT CONSTANTS ========== //
pub const MAX_STAKED_DOGES: usize = 5; // Maximum number of eggs a user can stake
pub const MAX_MULTIPLIER: u16 = 690; // Maximum multiplier a user can have (6.9x)

pub const MAX_DOGE_URIS: usize = 20; // Max URIs in GlobalConfig
pub const MAX_URI_LENGTH: usize = 200;

pub const MAX_CALLER_COMPENSATION: u64 = 5_000_000; // 0.005 SOL (0.005 SOL max per round)

/// ------------ GLOBAL CONFIG ------------

/// Global configuration for the program
#[account]
pub struct GlobalConfig {
    /// total number of players in the game
    pub total_players: u64,

    /// Authority that can update config parameters
    pub ext_authority: Pubkey,
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
    /// Percentage of SOL fees that go to protocol  
    pub protocol_fee_pct: u8,
    /// Percentage of SOL fees that go to buybacks
    pub buyback_pct: u8,
    /// Percentage of SOL fees that go to stakers
    pub stakers_pct: u8,
}

impl SolFeeConfig {
    pub const LEN: usize = 1 + 1 + 1; // protocol_fee_pct + buyback_pct + stakers_pct
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct MineBtcDistConfig {
    /// Percentage of MineBtc emission that goes to stakers
    pub minebtc_stakers_pct: u8,
    /// Percentage of MineBtc emission that goes to winning block bettors
    pub minebtc_winners_pct: u8,
    /// Percentage of MineBtc emission that goes to losing block bettors
    pub minebtc_same_faction_pct: u8,
    /// Percentage of MineBtc emission that goes to motherlode
    pub minebtc_motherlode_pct: u8,
    /// Refining fee
    pub refining_fee: u8,
}

impl MineBtcDistConfig {
    pub const LEN: usize = 1 + 1 + 1 + 1 + 1; // minebtc_stakers_pct + minebtc_winners_pct + minebtc_same_faction_pct + minebtc_motherlode_pct + refining_fee
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +                     // total_players
        32 +                    // ext_authority
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

    /// Base multiplier (100 = 1x)
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
/// When users mint eggs, they choose a ticket tier which gives them free tickets
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
pub struct EggConfig {
    pub bump: u8,

    /// Doge collection address (Metaplex Core)
    pub doge_collection: Pubkey,

    /// Doge URIs organized by faction
    /// Structure: [faction_id] = URI
    pub doge_uris: Vec<String>, // [faction_index] = URI

    /// Maximum supply of eggs that can be minted
    pub max_supply: u64,

    /// Number of eggs minted so far
    pub eggs_minted: u64,

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

impl EggConfig {
    pub const MAX_TICKET_TIERS: usize = 3;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // doge_collection
        4 + (MAX_FACTIONS * (4 + MAX_URI_LENGTH)) + // doge_uris Vec<String>
        8 +     // max_supply
        8 +     // eggs_minted
        8 +     // base_price
        8 +     // curve_a
        4 + (Self::MAX_TICKET_TIERS * TicketTier::LEN) + // ticket_tiers
        1 +     // breeding_allowed
        8 +     // breed_base_price
        8;      // breed_curve_a
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
    /// Timestamp when last distribution round ended (for 7-day cooldown)
    pub end_timestamp: i64,

    /// Leaderboard state: faction IDs ranked by hashpower (index = rank, value = faction_id)
    /// Rank 0 = highest hashpower, Rank 11 = lowest hashpower
    pub leaderboard_faction_ids: Vec<u8>,
    /// Leaderboard hashpower values (index = rank, value = hashpower)
    pub leaderboard_hashpower: Vec<u64>,
    /// Number of factions added to leaderboard so far (0-12)
    pub leaderboard_factions_count: u8,

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
    pub const DISTRIBUTION_COOLDOWN_SECONDS: i64 = 7 * DAY_IN_SECONDS as i64; // 7 days

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
    /// The timestamp when the current round ends.
    pub round_end_timestamp: i64,
    /// Round duration in seconds (configurable)
    pub round_duration_seconds: i64,

    // --- Data from the *previous* round (for claiming) ---
    /// The last completed round ID
    pub last_round_id: u64,
    /// The winning faction ID from the last completed round
    pub winning_faction_id: u8,

    // --- Commit-Reveal Randomness (ORE-style) ---
    /// Committed hash for the current round (set before round starts)
    /// This is hash(secret_seed) - the secret is revealed after betting closes
    pub current_round_commit: [u8; 32],
    /// Revealed seed for the current round (set after betting closes)
    /// Must verify: hash(revealed_seed) == current_round_commit
    pub current_round_seed: Option<[u8; 32]>,

    /// Whitelisted cranker bots that can call start_round and end_round
    /// Maximum MAX_CRANKER_BOTS bots
    pub cranker_bots: Vec<Pubkey>,
}

impl GlobalGameSate {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        1 +     // can_begin_round
        16 +    // total_sol_bets (u128)
        8 +     // current_round_id
        8 +     // round_end_timestamp
        8 +     // round_duration_seconds
        8 +     // last_round_id
        1 +     // winning_faction_id
        32 +    // current_round_commit [u8; 32]
        33 +    // current_round_seed Option<[u8; 32]> (1 byte discriminator + 32 bytes)
        4 + (MAX_CRANKER_BOTS * 32); // cranker_bots Vec<Pubkey> (4 bytes length + MAX_CRANKER_BOTS * 32 bytes)
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

/// Faction State PDA (Seed: `[b"faction", faction_id_u8]`)
/// Tracks cumulative statistics and reward indexes for a specific faction.
/// One account per faction (up to MAX_FACTIONS factions).
/// Used for calculating staker rewards based on faction performance.
#[account]
pub struct FactionState {
    pub bump: u8,
    /// The faction ID (0-10, matching index in supported_factions)
    pub faction_id: u8,

    /// Total passive hashpower from stakers in this faction (cumulative)
    pub total_minebtc_hashpower: u64,
    pub minebtc_staked: u64,
    pub minebtc_minebtc_reward_index: u128,
    pub minebtc_sol_reward_index: u128,

    pub total_lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_index: u128,
    pub lp_minebtc_reward_index: u128,

    pub eggs_staked: u64,
    /// Total eggs currently being used in gameplay
    pub eggs_playing: u64,

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
        8 +     // total_minebtc_hashpower (u64)
        8 +     // minebtc_staked (u64)
        16 +    // minebtc_minebtc_reward_index (u128)
        16 +    // minebtc_sol_reward_index (u128)
        8 +     // total_lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_index (u128)
        16 +    // lp_minebtc_reward_index (u128)
        8 +     // eggs_staked (u64)
        8 +     // eggs_playing (u64)
        8 +     // total_sol_bets (u64)
        8 +     // total_wins (u64)
        16 +    // sol_reward_index (u128)
        8; // motherlode_pot_size (u64)
}

// ========================================================================================
// ========================== GAME SESSION ACCOUNTS =================================
// ========================================================================================

/// Game Session PDA (Seed: `[b"game-session", round_id_u64]`)
/// Each raffle round has its own GameSession PDA that tracks:
/// - Round timing (start/end timestamps)
/// - Total bets placed in this round
/// - Bet indexes for tracking individual bets
/// - Winning block and faction
/// - Round-specific reward pools and payout data
/// This account is created when a round starts and finalized when the round ends.
#[account]
pub struct GameSession {
    pub bump: u8,

    // 0 = Not ended yet
    // 1 = ended and winning block finalized, need to claim faction rewards
    // 2 = Faction rewards also finalized
    pub stage: u8,

    /// The round ID this session belongs to
    pub round_id: u64,

    pub round_start_timestamp: i64,
    pub round_end_timestamp: i64,

    /// Total SOL bets placed in this round
    pub total_sol_bets: u64,
    /// Total points bets placed in this round
    pub total_points_bets: u64,
    /// Total weighted points bets (for dogeBTC distribution)
    pub total_wgtd_points_bets: u64,
    /// Total stakers fee paid in this round
    pub stakers_fee: u64,

    /// Block assignments: [block_0, block_1, ..., block_23]
    /// Each element is the faction_id assigned to that block (0-indexed: blocks 0-23)
    /// Set at round start when factions are randomly assigned to blocks
    pub block_assignments: [u8; NUM_BLOCKS],

    /// Number of users who bet on each block, SOL bet placed on that block and points bet placed on that block
    /// Used to track all bets and calculate rewards
    pub user_block_indexes: Vec<u64>,
    pub sol_bets_indexes: Vec<u64>,
    pub points_bets_indexes: Vec<u64>,
    /// Weighted points per block (points * active_multiplier / 100 for SOL bets, else points) - for dogeBTC rewards
    pub wgtd_points_bets_indexes: Vec<u64>,

    /// The winning block and faction ID for this round (0-indexed: 0-23), and the 2nd block with same faction ID
    pub winning_block: u8,
    pub winning_faction_id: u8,
    pub same_faction_other_block: u8,

    // --- MineBtc reward pools for this round ---
    /// MineBtc allocated for winners in this round
    pub minebtc_winner_pool: u64,
    /// MineBtc allocated for same-faction bettors in this round
    pub minebtc_loser_pool: u64,
    /// MineBtc allocated for stakers in this round
    pub faction_stakers: u64,
    /// MineBtc allocated for motherlode in this round
    pub motherlode_rewards: u64,

    /// SOL rewards index for this round
    pub sol_rewards_index: u128,
    /// MineBtc rewards index for this round
    pub minebtc_rewards_index: u128,
    /// MineBtc rewards index for same-faction bettors in this round
    pub same_faction_minebtc_rewards_index: u128,

    // --- Motherlode data for this round ---
    /// Whether motherlode was hit in this round
    pub motherlode_hit: bool,
    /// Motherlode pot size when hit (if applicable)
    pub motherlode_pot_size_on_hit: u64,

    // --- Instant Mutation tracking per faction ---
    /// Highest SOL bet placed per faction this round (for mutation probability calc)
    pub highest_sol_bet_per_faction: [u64; NUM_FACTIONS],
    /// Whether mutation has occurred for each faction this round (1 per faction per round)
    pub mutation_occurred_per_faction: [bool; NUM_FACTIONS],
}

impl GameSession {
    // Maximum number of bets per round (for Vec sizing)
    pub const MAX_BETS_PER_ROUND: usize = 10000; // Reasonable limit per round

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // stage (u8)
        8 +     // round_id
        8 +     // round_start_timestamp (i64)
        8 +     // round_end_timestamp (i64)
        8 +     // total_sol_bets
        8 +     // total_points_bets
        8 +     // total_wgtd_points_bets
        8 +     // stakers_fee
        4 + (NUM_BLOCKS * 8) + // user_block_indexes Vec<u64>
        4 + (NUM_BLOCKS * 8) + // sol_bets_indexes Vec<u64>
        4 + (NUM_BLOCKS * 8) + // points_bets_indexes Vec<u64>
        4 + (NUM_BLOCKS * 8) + // wgtd_points_bets_indexes Vec<u64>
        (NUM_BLOCKS * 1) + // block_assignments [u8; NUM_BLOCKS]
        1 +     // winning_block (u8)
        1 +     // winning_faction_id (u8)
        1 +     // same_faction_other_block (u8)
        8 +     // minebtc_winner_pool
        8 +     // minebtc_loser_pool
        8 +     // faction_stakers (u64)
        8 +     // motherlode_rewards (u64)
        16 +    // sol_rewards_index (u128)
        16 +    // minebtc_rewards_index (u128)
        16 +    // same_faction_minebtc_rewards_index (u128)
        1 +     // motherlode_hit (bool)
        8 +     // motherlode_pot_size_on_hit
        (NUM_FACTIONS * 8) + // highest_sol_bet_per_faction [u64; 12]
        (NUM_FACTIONS * 1); // mutation_occurred_per_faction [bool; 12]
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

    /// Referral code used by this player
    pub referral_code: Pubkey,

    /// The faction this player is assigned to
    pub faction_id: u8,

    /// Cumulative statistics
    pub rounds_played: u64,

    pub total_sol_bet: u64,
    pub total_points_bet: u64,

    pub total_sol_won: u64,
    pub total_minebtc_won: u64,

    pub minebtc_hashpower: u64,
    pub minebtc_staked: u64,
    pub minebtc_minebtc_reward_debt: u128,
    pub minebtc_sol_reward_debt: u128,

    pub lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_debt: u128,
    pub lp_minebtc_reward_debt: u128,

    pub pending_sol_rewards: u64,
    pub unrefining_index: u128,
    pub pending_minebtc_rewards: u64,
    pub unrefined_minebtc_rewards: u64,

    pub minebtc_position_indices: Vec<u8>,
    pub lp_position_indices: Vec<u8>,

    /// Staked dragon eggs (max 5 eggs)
    /// Stores the mint addresses of staked eggs
    pub staked_eggs: Vec<Pubkey>,
    /// Current doge multiplier (100 = 1x, 150 = 1.5x, etc.)
    /// Calculated based on number of staked eggs
    pub doge_multiplier: u16,

    /// Free tickets: points size of each ticket type (max 5 ticket types)
    /// Example: [10000000, 100000000, ...] where 1 point = 1 SOL lamport
    /// So 10000000 = 0.01 SOL, 100000000 = 0.1 SOL
    pub free_tickets: Vec<u64>,
    /// Free tickets remaining: count of each ticket type remaining
    /// Index matches free_tickets (e.g., free_tickets_remaining[0] is count for free_tickets[0])
    pub free_tickets_remaining: Vec<u64>,

    /// Doge currently being used in gameplay (Pubkey::default() if none)
    pub gameplay_egg: Pubkey,
    /// Active gameplay multiplier (100 = 1x, set from gameplay egg's multiplier, reset to 100 on withdraw)
    pub active_multiplier: u32,
    /// Cached DNA of gameplay doge (for mutation calculations without loading EggMetadata)
    pub gameplay_doge_dna: [u8; 32],
    /// Cached XP of gameplay doge (updated during gameplay, synced to EggMetadata on withdraw)
    pub gameplay_doge_xp: u32,
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
        32 +    // referral_code
        1 +     // faction_id
        8 +     // rounds_played
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_sol_won
        8 +     // total_minebtc_won
        8 +     // minebtc_hashpower (u64)
        8 +     // minebtc_staked (u64)
        16 +    // minebtc_minebtc_reward_debt (u128)
        16 +    // minebtc_sol_reward_debt (u128)
        8 +     // lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_debt (u128)
        16 +    // lp_minebtc_reward_debt (u128)
        8 +     // pending_sol_rewards (u64)
        16 +    // unrefining_index (u128)
        8 +     // pending_minebtc_rewards (u64)
        8 +     // unrefined_minebtc_rewards (u64)
        4 + (Self::MAX_POSITIONS * 1) + // minebtc_position_indices Vec<u8>
        4 + (Self::MAX_POSITIONS * 1) + // lp_position_indices Vec<u8>
        4 + (MAX_STAKED_DOGES * 32) + // staked_eggs Vec<Pubkey>
        2 +     // doge_multiplier (u16)
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets Vec<u64>
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets_remaining Vec<u64>
        32 +    // gameplay_egg
        4 +     // active_multiplier (u32)
        32 +    // gameplay_doge_dna [u8; 32]
        4; // gameplay_doge_xp (u32)
}

/// Individual MineBtc staking position
#[account]
pub struct StakedPosition {
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

    /// Pending SOL rewards from referrals (claimable)
    pub pending_sol_rewards: u64,
    /// Pending MineBtc rewards from referrals (claimable)
    pub pending_minebtc_rewards: u64,

    /// Total SOL earned from referrals (cumulative)
    pub total_sol_earned: u64,
    /// Total MineBtc earned from referrals (cumulative)
    pub total_minebtc_earned: u64,
}

impl ReferralRewards {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // bump
        2 +     // referrals_count
        8 +     // pending_sol_rewards
        8 +     // pending_minebtc_rewards
        8 +     // total_sol_earned
        8; // total_minebtc_earned
}

// ========================================================================================
// ===============================  DOGE NFT METADATA ===============================
// ========================================================================================

/// Doge NFT metadata (stored in minebtc program for simplicity)
#[account]
pub struct EggMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    /// Parent 1 mint (Pubkey::default() for genesis eggs)
    pub mom: Pubkey,
    /// Parent 2 mint (Pubkey::default() for genesis eggs)
    pub dad: Pubkey,
    /// Number of times this doge has bred (max 5)
    pub breed_count: u8,
    /// Unix timestamp when cooldown ends (can breed again after this)
    pub cooldown_end: i64,
    /// Creation timestamp
    pub created_at: i64,
    /// Faction ID (country) that the doge belongs to (matches minebtc faction)
    pub faction_id: u8,
    /// Multiplier for this doge (basis points, e.g., 150 = 1.5x)
    pub multiplier: u32,
    /// dogeBTC accumulated which can be claimed by sending this doge to heaven
    pub accumulated_val: u64,
    /// DNA data (32 bytes for breeding/evolution)
    pub dna: [u8; 32],
    /// The Player who is incubating this egg. Pubkey::default() if not incubated.
    pub incubated_player_data: Pubkey,
    /// Last power update timestamp
    pub last_update_ts: i64,
    /// Experience points, reset to 0 on evolution
    pub xp: u32,
    /// PDA bump
    pub bump: u8,
}

impl EggMetadata {
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
        1;      // bump
}

// ========================================================================================
// ============================= BET TYPE ENUM ==============================
// ========================================================================================

/// Bet type enum for user bets
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum BetType {
    /// Direct block selection (block_id: 1-24)
    Block { block_id: u8 },
    /// Faction + highest/lowest selection (faction_id + is_highest)
    /// is_highest: true = highest block, false = lowest block
    FactionHighestLowest { faction_id: u8, is_highest: bool },
    /// Faction with "both" option - bets on both blocks assigned to the faction
    /// This creates 2 separate bets internally
    FactionBoth { faction_id: u8 },
    /// Random block selection (block_id will be randomly selected at bet time)
    /// Note: For autominer use, this will be resolved when placing bets
    RandomBlock,
}

impl BetType {
    // Anchor enum serialization: 1 byte discriminator + max variant size
    // Block variant: 1 byte discriminator + 1 byte block_id = 2 bytes
    // FactionHighestLowest variant: 1 byte discriminator + 1 byte faction_id + 1 byte is_highest = 3 bytes
    // FactionBoth variant: 1 byte discriminator + 1 byte faction_id = 2 bytes
    // RandomBlock variant: 1 byte discriminator = 1 byte
    // Max size is 3 bytes
    pub const LEN: usize = 3;
}

// ========================================================================================
// ============================= FACTION SURGE ACCOUNTS ==============================
// ========================================================================================

/// User Game Bet PDA (Seed: `[b"user-bet", user_pubkey, round_id_u64]`)
/// Each user bet in a round has its own PDA account.
/// Users can bet on multiple blocks in a single round.
///
/// Structure:
/// - `block_ids`: List of blocks user bet on (0-indexed: 0-23)
/// - `sol_bets`: SOL bets for each block (index matches block_ids)
/// - `points_bets`: Points bets for each block (index matches block_ids)
/// - `total_sol_bet`: Total SOL bet across all blocks
/// - `total_points_bet`: Total points bet across all blocks
/// - `total_fee`: Total fees paid
#[account]
pub struct UserGameBet {
    /// The user who placed this bet
    pub owner: Pubkey,
    /// The round ID this bet belongs to
    pub round_id: u64,

    /// List of block IDs user bet on (0-indexed: 0-23)
    /// Index position corresponds to same index in sol_bets and points_bets
    pub block_ids: Vec<u8>,

    /// SOL bets for each block (index matches block_ids)
    pub sol_bets: Vec<u64>,
    /// Points bets for each block (index matches block_ids)
    pub points_bets: Vec<u64>,
    /// Weighted points for each block (points * multiplier / 100 for SOL, else points) - for dogeBTC
    pub wgtd_points_bets: Vec<u64>,

    /// Total SOL amount bet across all blocks (after protocol fee deduction)
    pub total_sol_bet: u64,
    /// Total points amount bet across all blocks
    pub total_points_bet: u64,
    /// Total weighted points (for dogeBTC rewards)
    pub total_wgtd_points_bet: u64,

    /// Total fees paid across all bets
    pub total_fee: u64,
    pub bump: u8,

    // --- Instant Mutation (applied during claim_rewards) ---
    /// 0 = no mutation, 1 = Evolution, 2 = Power, 3 = Trait
    pub mutation_type: u8,
}

impl UserGameBet {
    // Maximum number of blocks a user can bet on in a single round
    pub const MAX_BLOCKS_PER_BET: usize = 24;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        8 +     // round_id
        4 + (Self::MAX_BLOCKS_PER_BET * 1) + // block_ids Vec<u8>
        4 + (Self::MAX_BLOCKS_PER_BET * 8) + // sol_bets Vec<u64>
        4 + (Self::MAX_BLOCKS_PER_BET * 8) + // points_bets Vec<u64>
        4 + (Self::MAX_BLOCKS_PER_BET * 8) + // wgtd_points_bets Vec<u64>
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_wgtd_points_bet
        8 +     // total_fee
        1 +     // bump
        1;      // mutation_type
}

/// Autominer configuration for blocks
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum BlocksConfig {
    /// Specific list of blocks to bet on (0-indexed: 0-23)
    Specific { blocks: Vec<u8> },
    /// Random number of blocks to bet on
    Random { count: u8 },
}

/// Autominer configuration for factions
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum FactionsConfig {
    /// Specific list of factions with strategy
    Specific {
        factions: Vec<u8>,
        strategy: FactionStrategy, // highest, lowest, or both
    },
    /// Random number of factions with strategy
    Random {
        count: u8,
        strategy: FactionStrategy,
    },
}

/// Strategy for faction betting
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum FactionStrategy {
    Highest,
    Lowest,
    Both,
}

/// Autominer Vault PDA (Seed: `[b"autominer", user_pubkey]`)
/// Stores autominer configuration for a user; funds are held in the global autominer custody PDA
/// Allows users to configure automatic betting with flexible block/faction selection
#[account]
pub struct AutominerVault {
    pub owner: Pubkey,
    /// Blocks configuration (specific list or random count) - optional
    pub blocks_config: Option<BlocksConfig>,
    /// Factions configuration (specific list or random count with strategy) - optional
    pub factions_config: Option<FactionsConfig>,
    /// SOL amount to bet per round (distributed across all blocks)
    pub sol_per_round: u64,
    /// Number of rounds remaining (decremented after each round)
    pub rounds_remaining: u32,
    /// Last round ID where bets were placed (to prevent duplicate bets)
    pub last_bet_round_id: u64,
    pub vault_bump: u8,
    /// Remaining SOL balance reserved for this autominer (held in autominer custody PDA)
    pub sol_balance: u64,
}

impl AutominerVault {
    pub const MAX_BLOCKS: usize = 24; // Max 24 blocks
    pub const MAX_FACTIONS: usize = 12; // Max 12 factions

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        // blocks_config Option<BlocksConfig>
        // Option discriminator: 1 byte
        // Max variant: Specific { blocks: Vec<u8> } = 1 byte enum discriminator + 4 bytes Vec length + MAX_BLOCKS * 1 byte
        1 + (1 + 4 + (Self::MAX_BLOCKS * 1)) + // blocks_config Option<BlocksConfig>
        // factions_config Option<FactionsConfig>
        // Option discriminator: 1 byte
        // Max variant: Specific { factions: Vec<u8>, strategy: FactionStrategy } = 1 byte enum discriminator + 4 bytes Vec length + MAX_FACTIONS * 1 byte + 1 byte strategy
        1 + (1 + 4 + (Self::MAX_FACTIONS * 1) + 1) + // factions_config Option<FactionsConfig>
        8 +     // sol_per_round
        4 +     // rounds_remaining (u32)
        8 +     // last_bet_round_id
        1 +     // vault_bump
        8; // sol_balance
}
