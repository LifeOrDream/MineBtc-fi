use anchor_lang::prelude::*;

use crate::errors::ErrorCode;

// Moonbase pricing tiers
pub const PRICE_TIER_1: u64 = 500_000_000; // 0.5 SOL (no egg)
pub const PRICE_TIER_2: u64 = 2_420_000_000; // 2.42 SOL (has egg)
pub const PRICE_TIER_3: u64 = 4_200_000_000; // 4.20 SOL (has egg)
pub const PRICE_TIER_4: u64 = 6_900_000_000; // 6.9 SOL (has egg)

pub const DBTC_DECIMALS: u8 = 6;
pub const THIRTY_MINS: u64 = 5; //  1800; // 30 minutes in seconds
pub const FOUR_HOURS: u64 = 90; //  14400; // 4 hours in seconds
pub const PRICE_CHANGE_THRESHOLD: u64 = 3; // 3% threshold for rate changes

// ========== DECIMAL SCALING CONSTANTS ========== //

pub const INDEX_PRECISION: u64 = 1_000_000; // 1 million

// ========== GLOBAL CONSTANTS ========== //
pub const REFERRAL_FEE: u64 = 10; // 10%
pub const REFERRAL_DISCOUNT: u64 = 5; // 5% discount for users who use a referral code
pub const LOOT_REWARDS_PERCENTAGE: u64 = 15; // 15% of distributions/collections go to loot rewards (increased for sustainability)
pub const DISCRIMINATOR_SIZE: usize = 8;

// ========== LOOT SYSTEM CONSTANTS ========== //
pub const LOOT_TARGET_SOL_VAULT: u64 = 1_000_000_000_000; // 1,000 SOL target for healthy vault
pub const LOOT_TARGET_DBTC_VAULT: u64 = 100_000_000_000; // 100,000 DBTC target for healthy vault

// ========== FACTION CONSTANTS ========== //
pub const MAX_FACTIONS: usize = 15; // Maximum number of supported factions
pub const MAX_FACTION_NAME_LENGTH: usize = 32; // Maximum characters in faction name
pub const MAX_FACTION_IDS_PER_MODULE: usize = 8; // Maximum faction restrictions per module
pub const MAX_EXPANSIONS: usize = 20; // Maximum number of expansion configs
pub const MAX_EXPANSION_NAME_LENGTH: usize = 32; // Maximum characters in expansion name

// ========== XP SYSTEM CONSTANTS ========== //
pub const XP_DAILY_LOGIN: u32 = 10;
pub const XP_MINING_1000_MDOGE: u32 = 15;

// New exponential XP curve: required_xp = 120 × (1.35^level)
pub const XP_CURVE_NUM: u64 = 135; // 1.35 in Q0.2 fixed-point
pub const XP_CURVE_DEN: u64 = 100;
pub const XP_BASE: u64 = 120; // Base XP for level 1

// ========== UPGRADE SCALING CONSTANTS ========== //
// Using fixed-point math: 115 = +15% per upgrade step (Power Curve)
pub const GROWTH_NUM: u64 = 115;
pub const GROWTH_DEN: u64 = 100;

// ========== UPGRADE COST SCALING CONSTANTS ========== //
// Using moderate curve for upgrade costs: 125 = +25% per upgrade level (Cost Curve)
// This gives reasonable progression: 1x -> 1.25x -> 1.56x -> 1.95x -> 2.44x -> 3.05x -> 3.81x -> 4.77x -> 5.96x -> 7.45x at level 10
pub const UPGRADE_COST_NUM: u64 = 125;
pub const UPGRADE_COST_DEN: u64 = 100;

/// Calculate growth factor for power scaling (damage, hashpower, shield HP)
/// Returns Q32 fixed-point representation of (1.15)^level
fn growth_factor(level: u8) -> u64 {
    let mut num: u64 = 1;
    let mut den: u64 = 1;
    for _ in 0..level {
        num = num.saturating_mul(GROWTH_NUM);
        den = den.saturating_mul(GROWTH_DEN);
    }
    // Return Q32 fixed-point: (numerator << 32) / denominator
    (num << 32) / den.max(1) // Prevent division by zero
}

// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / DogeBtcMining state
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";
pub const DOGE_BTC_MINING_SEED: &[u8] = b"moon-doge-mining";

// PDAs which hold SOL collected by the program
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
pub const DOGE_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"mdoge-vault-authority";
pub const DOGE_BTC_VAULT_SEED: &[u8] = b"dbtc_vault";

// PDAs which hold ModuleConfigStore  state
pub const MODULE_CONFIG_STORE_SEED: &[u8] = b"module-config-store";
pub const MODULE_CONFIG_SEED: &[u8] = b"module-config"; // For individual module config PDAs

// PDAs which hold UserMoonBaseInstance / ReferralRewards state
pub const USER_MOONBASE_SEED: &[u8] = b"user-moonbase";
pub const REFERRAL_REWARDS_SEED: &[u8] = b"referral-rewards";
pub const COLLECTION_AUTHORITY_SEED: &[u8] = b"collection_authority";

// PDAs which hold GearInstance / ModuleInstance state
pub const MODULE_INSTANCE_SEED: &[u8] = b"module-instance";

// PDAs for Dragon Egg NFT system
pub const DRAGON_EGG_METADATA_SEED: &[u8] = b"dragon-egg-metadata";
pub const INCUBATION_STATE_SEED: &[u8] = b"incubation-state";
pub const DRAGON_EGG_CUSTODY_SEED: &[u8] = b"dragon-egg-custody"; // PDA that holds locked NFTs

// PDAs for loot rewards system
pub const LOOT_REWARDS_SEED: &[u8] = b"loot-rewards";
pub const LOOT_SOL_VAULT_SEED: &[u8] = b"loot-sol-vault";
pub const LOOT_DOGE_BTC_VAULT_SEED: &[u8] = b"loot-mdoge-vault";
pub const LOOT_DOGE_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"loot-mdoge-vault-authority";
pub const BUYBACKS_SEED: &[u8] = b"buybacks";
pub const BUYBACKS_SOL_VAULT_SEED: &[u8] = b"buybacks-sol-vault";
pub const LEVEL_STATS_SEED: &[u8] = b"level-stats";

// ========== MODULE SYSTEM CONSTANTS ========== //
pub const MAX_MODULE_UPGRADES: u8 = 10; // Maximum upgrade level for any module

pub const MAX_BOUGHT_MODULES: usize = 100; // Maximum modules that can be bought but not installed

// Command Center config IDs (one per init_type tier)
pub const COMMAND_CENTER_TIER_1_CONFIG_ID: u16 = 1; // Tier 1 (no egg)
pub const COMMAND_CENTER_TIER_2_CONFIG_ID: u16 = 2; // Tier 2
pub const COMMAND_CENTER_TIER_3_CONFIG_ID: u16 = 3; // Tier 3
pub const COMMAND_CENTER_TIER_4_CONFIG_ID: u16 = 4; // Tier 4

// ========== GRID SYSTEM CONSTANTS ========== //
pub const GRID_WIDTH: u8 = 20; // 20 tiles wide
pub const GRID_HEIGHT: u8 = 15; // 15 tiles tall
pub const TOTAL_TILES: usize = (GRID_WIDTH as usize) * (GRID_HEIGHT as usize); // 300 tiles
pub const BITMAP_SIZE: usize = (TOTAL_TILES + 7) / 8; // 38 bytes (300 bits rounded up to bytes)

// ========== MOONBASE EXPANSION CONSTANTS ========== //
pub const DEFAULT_MOONBASE_WIDTH: u8 = 10; // Starting moonbase is 10x8 (80 tiles)
pub const DEFAULT_MOONBASE_HEIGHT: u8 = 8;

/// ------------ EXPANSION SYSTEM ------------

/// Configuration for a moonbase expansion
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ExpansionConfig {
    /// Unique expansion ID
    pub id: u8,
    /// Display name for the expansion (e.g., "Mining Sector", "Research Wing")
    pub name: String,
    /// Minimum level required to unlock this expansion
    pub required_level: u8,
    /// Cost in SOL to purchase this expansion (in lamports)
    pub cost_sol: u64,
    /// New width after this expansion
    pub new_width: u8,
    /// New height after this expansion
    pub new_height: u8,
    /// Whether this expansion is currently available for purchase
    pub is_active: bool,
}

impl ExpansionConfig {
    // id + name + required_level + cost_sol + new_width + new_height + is_active
    pub const LEN: usize = 1 + (4 + MAX_EXPANSION_NAME_LENGTH) + 1 + 8 + 1 + 1 + 1;
}

/// ------------ GLOBAL CONFIG ------------

/// Global configuration for the Moon Facility program
#[account]
pub struct GlobalConfig {
    /// Authority that can update config parameters
    pub ext_authority: Pubkey,
    /// External account that can withdraw collected SOL
    pub ext_fee_collector: Pubkey,
    /// Direct recipient for 50% of creation fees
    pub creation_fee_recipient: Pubkey,
    /// PDA account that holds collected SOL fees
    pub pda_sol_treasury: Pubkey,
    /// ------------------------------------------------------------           
    /// Total number of moonbases that have been created
    pub total_moonbases_created: u64,
    /// Total SOL spent by users in the game
    pub total_sol_spent: u64,
    /// Total SOL paid out as referrals
    pub total_referral_sol_paid: u64,
    /// Percentage of distributions/fees that go to loot rewards (default 10%)
    pub loot_percentage: u8,
    /// Percentage of SOL fees that go to buybacks (default 20%)
    pub buyback_percentage: u8,
    /// Whether PvP games are currently active and can be created
    pub is_game_active: bool,
    /// ------------------------------------------------------------           
    /// Bump for GlobalConfig PDA derivation
    pub bump: u8,
    /// Bump for SOL treasury PDA derivation
    pub treasury_bump: u8,
    /// List of supported factions (e.g., "USA", "China", "Russia")
    /// Maximum 15 factions, each with max 16 characters
    pub supported_factions: Vec<String>,
    /// Available moonbase expansions (level requirements and costs)
    pub expansions: Vec<ExpansionConfig>,
    /// Dragon Egg collection address (Metaplex Core)
    pub dragon_egg_collection: Pubkey,
    /// Total Dragon Eggs minted
    pub total_dragon_eggs_minted: u64,
    /// Available Dragon Egg URIs (randomly selected on mint)
    pub dragon_egg_uris: Vec<String>,
    /// Egg limits per tier: [unused, tier2_limit, tier3_limit, tier4_limit]
    /// Tier 2: 5000 eggs, Tier 3: 5000 eggs, Tier 4: 5000 eggs
    pub egg_limits: [u64; 4],
    /// Authorized Raydium pool state address (security: prevents using malicious pools)
    pub raydium_pool_state: Pubkey,
    /// Global total power across all Dragon Eggs (sum of all egg powers)
    pub global_dragon_egg_power: u64,
}

impl GlobalConfig {
    // discriminator + authority + fee_collector + creation_fee_recipient + sol_treasury +  total_moonbases_created + total_sol_spent + total_referral_sol_paid + loot_percentage + buyback_percentage + is_game_active + bump + treasury_bump + supported_factions (vec) + expansions (vec) + dragon_egg_collection + total_dragon_eggs_minted + dragon_egg_uris + egg_limits + raydium_pool_state
    // Vec<String> = 4 bytes (vec length) + MAX_FACTIONS * (4 bytes string length + MAX_FACTION_NAME_LENGTH bytes)
    // Vec<ExpansionConfig> = 4 bytes (vec length) + MAX_EXPANSIONS * ExpansionConfig::LEN
    pub const LEN: usize = DISCRIMINATOR_SIZE + 
        32 +                    // ext_authority
        32 +                    // ext_fee_collector  
        32 +                    // creation_fee_recipient
        32 +                    // pda_sol_treasury
        8 +                     // total_moonbases_created
        8 +                     // total_sol_spent
        8 +                     // total_referral_sol_paid
        1 +                     // loot_percentage
        1 +                     // buyback_percentage
        1 +                     // is_game_active
        1 +                     // bump
        1 +                     // treasury_bump
        4 + (MAX_FACTIONS * (4 + MAX_FACTION_NAME_LENGTH)) + // supported_factions vec
        4 + (MAX_EXPANSIONS * ExpansionConfig::LEN) +         // expansions vec
        32 +                    // dragon_egg_collection
        8 +                     // total_dragon_eggs_minted
        4 + (MAX_DRAGON_EGG_URIS * (4 + MAX_URI_LENGTH)) +    // dragon_egg_uris vec
        (4 * 8) +               // egg_limits [u64; 4] = 32 bytes
        32 +                    // raydium_pool_state
        8; // global_dragon_egg_power

    /// Select random Dragon Egg URI based on slot, index, and DNA
    pub fn get_random_dragon_egg_uri(
        &self,
        slot: u64,
        index: u64,
        dna: &[u8; 32],
    ) -> Result<String> {
        require!(!self.dragon_egg_uris.is_empty(), ErrorCode::InvalidMetadata);

        let dna_seed = u64::from_le_bytes([
            dna[0], dna[1], dna[2], dna[3], dna[4], dna[5], dna[6], dna[7],
        ]);
        let random_index =
            (slot.wrapping_add(index).wrapping_add(dna_seed)) as usize % self.dragon_egg_uris.len();
        Ok(self.dragon_egg_uris[random_index].clone())
    }
}

/// ------------ MOON DOGE MINING ------------

/// Price entry for tracking historical prices
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PriceEntry {
    /// Timestamp when this price was recorded
    pub timestamp: i64,
    /// Price in SOL per DOGE_BTC (scaled by 10^9 for full precision)
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
    /// Total DOGE_BTC added to liquidity pool (accumulated)
    pub total_dbtc_added: u64,
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
        dbtc_added: u64,
    ) {
        // Update cumulative totals
        self.total_lp_burnt = self.total_lp_burnt.saturating_add(lp_tokens_burnt);
        self.total_sol_added = self.total_sol_added.saturating_add(sol_added);
        self.total_dbtc_added = self.total_dbtc_added.saturating_add(dbtc_added);
        self.lp_operations_count = self.lp_operations_count.saturating_add(1);
    }
}

/// Moon Doge Mining status and parameters
#[account]
pub struct DogeBtcMining {
    /// Token vault that holds all pre-minted tokens
    pub dbtc_token_vault: Pubkey,
    /// Timestamp of the mining start
    pub mining_start_timestamp: u64,
    /// DogeBtc mined per slot (original base rate)
    pub doge_btc_per_slot: u64,
    /// Last slot when moondoge were mined
    pub last_slot: u64,
    /// Total active hashpower across all facilities
    pub total_active_hashpower: u64,
    /// Total active electricity across all facilities
    pub total_active_electricity: u64,
    /// Total tokens mined so far
    pub total_tokens_mined: u64,

    /// dBTC tokens minted per hashpower (index for tracking distirbution)
    pub dbtc_tokens_minted_per_hashpower: u128,

    /// Bump for PDA derivation
    pub bump: u8,
    /// Bump for vault authority PDA derivation
    pub vault_auth_bump: u8,

    // ===== DYNAMIC DISTRIBUTION FIELDS =====
    /// Raydium pool state for DOGE_BTC-SOL trading
    pub raydium_pool_state: Pubkey,
    /// Last time distribution rate was updated (timestamp)
    pub last_rate_update: i64,
    /// Current distribution rate (starts at doge_btc_per_slot)
    pub current_dist_rate: u64,
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
}

impl DogeBtcMining {
    // discriminator + dbtc_token_vault + mining_start_timestamp + doge_btc_per_slot + last_slot + total_active_hashpower + total_active_electricity + total_tokens_mined + dbtc_tokens_minted_per_hashpower + bump + vault_auth_bump +
    // raydium_pool_state + last_rate_update + current_dist_rate + price_history (vec) + recent_price + track_price + sol_for_pol + pol_stats + lp_token_price_in_sol
    pub const MAX_PRICE_HISTORY_ENTRIES: usize = 8; // 4-hour cycle (8 × 30min snapshots)
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 32
        + 8
        + 8
        + 8
        + 8
        + 8
        + 8
        + 16
        + 1
        + 1
        + 32
        + 8
        + 8
        + (4 + Self::MAX_PRICE_HISTORY_ENTRIES * PriceEntry::LEN)
        + 8
        + 8
        + 8
        + ProtocolOwnedLiquidity::LEN
        + 8;
}

/// ------------ USER MOON-BASE INSTANCES ------------

/// Entry for tracking available modules by config ID
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AvailableModuleEntry {
    /// Configuration ID of the module type
    pub config_id: u16,
    pub count: u8,
}

impl AvailableModuleEntry {
    pub const LEN: usize = 2 + 1; // config_id + module
}

/// User Moon-Base instance
#[account]
pub struct UserMoonBaseInstance {
    /// Module instances
    pub owner: Pubkey,
    pub referral: Pubkey,
    pub modules_count: u8,
    pub active_hashpower: u64,
    pub available_electricity: u64,
    pub used_electricity: u64,

    pub dbtc_claim_index: u128,
    pub claimable_dbtc: u64,

    pub bump: u8,
    /// Faction ID (0-based index into GlobalConfig.supported_factions)
    pub faction_id: u8,
    /// Player level (starts at 0)
    pub level: u8,
    /// Initialization tier (1, 2, 3, or 4) - determines initial bonuses
    pub init_type: u8,
    /// Current XP points
    pub xp: u32,
    /// Last login timestamp for daily login tracking
    pub last_login_ts: i64,
    /// Daily login streak counter
    pub daily_login_streak: u16,
    /// Current moonbase width (starts at DEFAULT_MOONBASE_WIDTH)
    pub current_width: u8,
    /// Current moonbase height (starts at DEFAULT_MOONBASE_HEIGHT)
    pub current_height: u8,
    /// List of expansion IDs that have been purchased
    pub purchased_expansions: Vec<u8>,
    /// Grid occupation bitmap (300 tiles = 38 bytes)
    pub occupied_bitmap: [u8; BITMAP_SIZE],
    /// List of modules available to the user (config_id -> count) - includes deployed and undeployed
    pub available_modules: Vec<AvailableModuleEntry>,

    // ========== PVP SUPPORT ========== //
    pub pvp_hp: u32,

    /// Pubkey of currently active PvP game (None if not in a match)
    pub active_game: Option<Pubkey>,
    /// Timestamp when the last PvP game ended (for cooldown / repair)
    pub last_game_end_ts: i64,
    /// Flag indicating whether modules were repaired since last game
    pub modules_repaired_since_last_game: bool,

    // ========== DRAGON EGG INTEGRATION ========== //
    /// Incubated Dragon Egg metadata address (max 1 per moonbase)
    pub incubated_dragon_egg: Option<Pubkey>,

    // ========== COMMAND CENTER MODULE ========== //
    /// Command center module instance (always index 0, non-removable)
    pub command_center_module: Option<Pubkey>,
}

// UserMoonBaseInstance
impl UserMoonBaseInstance {
    // discriminator + owner + referral + modules_count + active_hashpower + available_electricity + used_electricity + dbtc_claim_index + claimable_dbtc + bump + faction_id + level + init_type + xp + last_login_ts + daily_login_streak + current_width + current_height + purchased_expansions + occupied_bitmap + available_modules + pvp_hp + active_game + last_game_end_ts + modules_repaired_since_last_game + incubated_dragon_egg + command_center_module
    // purchased_expansions = 4 bytes (vec length) + MAX_EXPANSIONS * 1 byte per expansion ID
    // available_modules = 4 bytes (vec length) + MAX_BOUGHT_MODULES * AvailableModuleEntry::LEN
    // active_game = Option<Pubkey> = 1 byte flag + 32 bytes pubkey = 33 bytes
    // incubated_dragon_egg = Option<Pubkey> = 1 byte flag + 32 bytes pubkey = 33 bytes
    // command_center_module = Option<Pubkey> = 1 byte flag + 32 bytes pubkey = 33 bytes
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 32
        + 32
        + 1
        + 8
        + 8
        + 8
        + 16
        + 8
        + 1
        + 1
        + 1
        + 1
        + 4
        + 8
        + 2
        + 1
        + 1
        + (4 + MAX_EXPANSIONS)
        + BITMAP_SIZE
        + (4 + MAX_BOUGHT_MODULES * AvailableModuleEntry::LEN)
        + 4
        + 33
        + 8
        + 1
        + 33
        + 33;
}

/// Stores referral rewards that a user has earned from referrals
#[account]
pub struct ReferralRewards {
    pub owner: Pubkey,
    pub total_sol_earned: u64,
    /// Amount of SOL that has already been used for XP calculation
    pub sol_claimed_for_xp: u64,
    pub bump: u8,
    /// Number of users who have used this user's referral code
    pub referrals_count: u16,
}

impl ReferralRewards {
    // discriminator + owner + total_sol_earned + sol_claimed_for_xp + bump + referrals_count
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 8 + 8 + 1 + 2;
}

/// Loot rewards system that accumulates 10% of DOGE_BTC distributions and SOL collections
#[account]
pub struct LootRewards {
    /// Total DOGE_BTC tokens accumulated for loot rewards
    pub total_dbtc_accumulated: u64,
    /// Total SOL accumulated for loot rewards (in lamports)
    pub total_sol_accumulated: u64,
    /// Total DOGE_BTC tokens distributed as loot rewards
    pub total_dbtc_distributed: u64,
    /// Total SOL distributed as loot rewards (in lamports)
    pub total_sol_distributed: u64,
    /// Bump for PDA derivation
    pub bump: u8,
    /// Bump for SOL vault PDA derivation
    pub sol_vault_bump: u8,
    /// Bump for DOGE_BTC vault PDA derivation
    pub dbtc_vault_bump: u8,
    /// Bump for DOGE_BTC vault authority PDA derivation
    pub dbtc_vault_authority_bump: u8,
}

impl LootRewards {
    // discriminator + total_dbtc_accumulated + total_sol_accumulated + total_dbtc_distributed + total_sol_distributed + bump + sol_vault_bump + dbtc_vault_bump + dbtc_vault_authority_bump
    pub const LEN: usize = DISCRIMINATOR_SIZE + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1;
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

/// Level statistics for tracking user distribution across top levels only
#[account]
pub struct LevelStats {
    /// Tracked levels with their user counts (sorted by level descending)
    /// Each entry contains (level, user_count) pairs for the top levels
    pub tracked_levels: Vec<LevelEntry>, // [(level_50, count), (level_49, count), ...]
    /// Total number of users in the system
    pub total_users: u32,
    /// Highest level achieved by any user
    pub max_level_achieved: u8,
    /// Minimum level currently being tracked (lowest in our top levels)
    pub min_tracked_level: u8,
    /// Last time stats were updated
    pub last_update_timestamp: i64,
    /// Bump for PDA derivation
    pub bump: u8,
}

/// Entry for tracking a specific level and its user count
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct LevelEntry {
    pub level: u8,
    pub user_count: u32,
}

impl LevelEntry {
    pub const LEN: usize = 1 + 4; // level + user_count
}

impl LevelStats {
    // We track top 25 levels dynamically
    pub const MAX_TRACKED_LEVELS: usize = 10;
    // discriminator + tracked_levels (vec) + total_users + max_level_achieved + min_tracked_level + last_update_timestamp + bump
    pub const LEN: usize =
        DISCRIMINATOR_SIZE + 4 + (Self::MAX_TRACKED_LEVELS * LevelEntry::LEN) + 4 + 1 + 1 + 8 + 1;
}

/// ------------ MODULE CONFIGS ------------

/// Module type enumeration for different gameplay mechanics
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum ModuleType {
    Mining,     // Generates hashpower → DOGE_BTC
    Attraction, // Grants passive XP / social score
}

impl ModuleType {
    pub const LEN: usize = 1; // enum discriminant
}

/// Mining module statistics - affects mining operations
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct MiningStats {
    pub max_hp: u32,
    pub base_hashpower: u32,    // Base hashpower at level 0
    pub power_consumption: u16, // Electricity consumed per hour
}

impl MiningStats {
    pub const LEN: usize = 4 + 4 + 2; // 10 bytes

    /// Calculate current hashpower using exponential growth curve (Power Curve)
    /// Each upgrade provides ~15% multiplicative increase
    pub fn current_hashpower(&self, upgrade_level: u8) -> u32 {
        let q32 = growth_factor(upgrade_level);
        ((self.base_hashpower as u64 * q32) >> 32).min(u32::MAX as u64) as u32
    }
}

/// Attraction module statistics - generates XP for moonbase
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub struct AttractionStats {
    pub max_hp: u32,
    pub base_xp_per_hour: u32,  // Base XP generation at level 0
    pub power_consumption: u16, // Electricity consumed per hour
}

impl AttractionStats {
    pub const LEN: usize = 4 + 4 + 2; // 10 bytes

    /// Calculate current XP generation using exponential growth curve (Power Curve)
    /// Each upgrade provides ~15% multiplicative increase
    pub fn current_xp_per_hour(&self, upgrade_level: u8) -> u32 {
        let q32 = growth_factor(upgrade_level);
        ((self.base_xp_per_hour as u64 * q32) >> 32).min(u32::MAX as u64) as u32
    }
}

/// Type-safe stats union for different module types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum ModuleStats {
    Mining(MiningStats),
    Attraction(AttractionStats),
}

impl ModuleStats {
    // Use the largest variant for size calculation (both are 10 bytes)
    pub const LEN: usize = 1 + MiningStats::LEN; // enum discriminant + largest variant
}

/// Configuration for a type of module that can be built
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ModuleConfig {
    pub id: u16,
    pub name: String,      // Max 32 chars
    pub image_url: String, // Max 64 chars
    pub module_type: ModuleType,
    pub stats: ModuleStats,
    pub faction_ids: Vec<u8>, // Which factions can use this module (empty = all)
    pub min_level: u8,        // Minimum moonbase level required to build
    pub width: u8,            // Grid width
    pub height: u8,           // Grid height
    pub mint_cost: u64,       // Base SOL cost to mint
    pub upgrade_cost: u64,    // Base SOL cost per upgrade
    pub upgrade_level_requirements: Vec<u8>, // Moonbase levels required for each upgrade [level_for_upgrade_1, level_for_upgrade_2, ...]
    pub is_active: bool,                     // Whether this config is currently available
}

impl ModuleConfig {
    pub const LEN: usize = 2 +                         // id
        4 + 32 +                    // name (String with length prefix)
        4 + 64 +                    // image_url (String with length prefix)
        1 +                         // module_type
        ModuleStats::LEN +          // stats (largest variant)
        4 + (1 * MAX_FACTION_IDS_PER_MODULE) + // faction_ids (Vec with length prefix) - each faction_id is u8 (1 byte)
        1 +                         // min_level
        1 +                         // width
        1 +                         // height
        8 +                         // mint_cost
        8 +                         // upgrade_cost
        4 + (MAX_MODULE_UPGRADES as usize * 1) + // upgrade_level_requirements (Vec with length prefix)
        1; // is_active

    /// Get maximum upgrade level (derived from upgrade_level_requirements length)
    pub fn max_upgrades(&self) -> u8 {
        self.upgrade_level_requirements.len() as u8
    }

    /// Check if upgrade is available at the given moonbase level
    pub fn is_upgrade_available(&self, upgrade_level: u8, moonbase_level: u8) -> bool {
        if upgrade_level == 0 {
            return true; // Base level always available if module can be built
        }

        let upgrade_index = (upgrade_level - 1) as usize;
        if upgrade_index >= self.upgrade_level_requirements.len() {
            return false; // Upgrade doesn't exist
        }

        moonbase_level >= self.upgrade_level_requirements[upgrade_index]
    }

    /// Get the moonbase level required for a specific upgrade
    pub fn get_upgrade_level_requirement(&self, upgrade_level: u8) -> Option<u8> {
        if upgrade_level == 0 {
            return Some(self.min_level); // Base level requirement
        }

        let upgrade_index = (upgrade_level - 1) as usize;
        self.upgrade_level_requirements.get(upgrade_index).copied()
    }

    /// Calculate total upgrade cost up to a specific level using progressive pricing
    /// Uses the moderate 1.25x multiplier per level for balanced upgrade progression
    pub fn total_upgrade_cost(&self, target_upgrade_level: u8) -> u64 {
        if target_upgrade_level == 0 {
            return 0;
        }

        let mut total_cost = 0u64;

        // Sum up the cost of each upgrade level (1.25x progression)
        for level in 1..=target_upgrade_level {
            let mut num: u64 = 1;
            let mut den: u64 = 1;

            // Calculate (1.25)^level
            for _ in 0..level {
                num = num.saturating_mul(UPGRADE_COST_NUM);
                den = den.saturating_mul(UPGRADE_COST_DEN);
            }

            // Apply scaling to base cost
            let level_cost = (self.upgrade_cost as u128 * num as u128) / den as u128;
            total_cost = total_cost.saturating_add(level_cost.min(u64::MAX as u128) as u64);
        }

        total_cost
    }

    /// Calculate the cost for upgrading from current level to next level
    /// This is what the user pays for a single upgrade step
    pub fn next_upgrade_cost(&self, current_upgrade_level: u8) -> u64 {
        let next_level = current_upgrade_level + 1;

        if next_level > self.max_upgrades() {
            return 0; // Can't upgrade beyond max
        }

        let mut num: u64 = 1;
        let mut den: u64 = 1;

        // Calculate (1.25)^next_level
        for _ in 0..next_level {
            num = num.saturating_mul(UPGRADE_COST_NUM);
            den = den.saturating_mul(UPGRADE_COST_DEN);
        }

        // Apply scaling to base cost
        let scaled_cost = (self.upgrade_cost as u128 * num as u128) / den as u128;
        scaled_cost.min(u64::MAX as u128) as u64
    }
}

/// Storage for all module configs - now just an index
#[account]
pub struct ModuleConfigStore {
    /// Next ID to assign
    pub next_id: u16,
    /// List of active config IDs (optional: for quick enumeration)
    pub active_ids: Vec<u16>,
    /// Bump for PDA derivation
    pub bump: u8,
}

impl ModuleConfigStore {
    // discriminator + next_id + vec len + (100 active ids) + bump
    pub const LEN: usize = DISCRIMINATOR_SIZE + 2 + 4 + (100 * 2) + 1; // ~309 bytes for 100 configs
}

/// Individual module config account (one PDA per config)
#[account]
pub struct ModuleConfigAccount {
    /// The actual module configuration data
    pub data: ModuleConfig,
    /// Bump for PDA derivation
    pub bump: u8,
}

impl ModuleConfigAccount {
    pub const LEN: usize = DISCRIMINATOR_SIZE + ModuleConfig::LEN + 1; // ~300-400 bytes per config
}

/// ------------ USER INSTANCES ------------

/// Runtime state for different module types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum ModuleRuntimeState {
    Mining {
        current_hp: u32,
        total_mined: u64,
    },
    Attraction {
        current_hp: u32,
        total_xp_generated: u64,
        last_xp_claim: i64,
    },
}

impl ModuleRuntimeState {
    // Use largest variant for size calculation
    // Attraction variant: discriminant(1) + current_hp(4) + total_xp_generated(8) + last_xp_claim(8)
    pub const LEN: usize = 1 + 4 + 8 + 8; // 21 bytes total
}

/// Module instance owned by a user with enhanced type safety
#[account]
pub struct ModuleInstance {
    /// Module config ID this is an instance of
    pub config_id: u16,
    /// Current upgrade level (0-10)
    pub upgrade_level: u8,
    /// Instance index within the moonbase
    pub index: u8,
    /// Module type (cached from config for efficiency)
    pub module_type: ModuleType,

    /// Position on the grid
    pub pos_x: u8, // left-most tile (0..GRID_WIDTH-1)
    pub pos_y: u8,  // top-most tile (0..GRID_HEIGHT-1)
    pub width: u8,  // tiles wide
    pub height: u8, // tiles tall
    /// Runtime state specific to module type
    pub runtime_state: ModuleRuntimeState,

    /// Current electricity cost (calculated from base + upgrades)
    pub electricity_cost: u32,
    /// Whether this module is currently active
    pub is_active: bool,

    /// Creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub last_updated: i64,

    /// Whether this module can be removed (command center is non-removable)
    pub is_removable: bool,

    /// Bump for PDA derivation
    pub bump: u8,
}

impl ModuleInstance {
    // discriminator + config_id + upgrade_level + index + module_type + pos_x + pos_y + width + height + runtime_state +
    // electricity_cost + is_active + created_at + last_updated + is_removable + bump
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 2
        + 1
        + 1
        + ModuleType::LEN
        + 1
        + 1
        + 1
        + 1
        + ModuleRuntimeState::LEN
        + 4
        + 1
        + 8
        + 8
        + 1
        + 1;

    /// Calculate current HP from runtime state
    pub fn current_hp(&self) -> u32 {
        match &self.runtime_state {
            ModuleRuntimeState::Mining { current_hp, .. } => *current_hp,
            ModuleRuntimeState::Attraction { current_hp, .. } => *current_hp,
        }
    }

    /// Calculate efficiency multiplier based on HP (damaged modules work worse)
    /// Returns a value between 0.1 and 1.0 (10% to 100% efficiency)
    pub fn hp_efficiency_multiplier(&self, max_hp: u32) -> f64 {
        let current_hp = self.current_hp();
        if max_hp == 0 {
            return 1.0;
        }

        let efficiency = (current_hp as f64) / (max_hp as f64);
        efficiency.max(0.1).min(1.0) // Minimum 10% efficiency even when heavily damaged
    }

    /// Calculate effective hashpower for mining modules
    pub fn effective_hashpower(&self, stats: &MiningStats) -> u32 {
        let base_hashpower = stats.current_hashpower(self.upgrade_level);
        let efficiency = self.hp_efficiency_multiplier(stats.max_hp);
        (base_hashpower as f64 * efficiency) as u32
    }

    /// Calculate effective XP generation for attraction modules
    pub fn effective_xp_per_hour(&self, stats: &AttractionStats) -> u32 {
        let base_xp = stats.current_xp_per_hour(self.upgrade_level);
        let efficiency = self.hp_efficiency_multiplier(stats.max_hp);
        (base_xp as f64 * efficiency) as u32
    }

    /// Check if this module can be upgraded to the next level
    pub fn can_upgrade(&self, config: &ModuleConfig, moonbase_level: u8) -> bool {
        if self.upgrade_level >= config.max_upgrades() {
            return false; // Already at max level
        }

        let next_level = self.upgrade_level + 1;
        config.is_upgrade_available(next_level, moonbase_level)
    }

    /// Calculate cost to upgrade to the next level using progressive pricing
    pub fn upgrade_cost(&self, config: &ModuleConfig) -> Option<u64> {
        if self.upgrade_level >= config.max_upgrades() {
            return None; // Already at max level
        }

        // Use the new progressive cost calculation
        Some(config.next_upgrade_cost(self.upgrade_level))
    }
}

// ========== DRAGON EGG NFT CONSTANTS ========== //
pub const BASE_EGG_POWER: u32 = 100;
pub const POWER_RATE_MULTIPLIER: u64 = 1000; // Divisor for balance

pub const MAX_DRAGON_EGG_URIS: usize = 20; // Max URIs in GlobalConfig
pub const MAX_URI_LENGTH: usize = 200;

// ========== LOOT SYSTEM CONSTANTS ========== //
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

// Safety rails for loot payouts
pub const MIN_SOL_PAYOUT_LAMPORTS: u64 = 10_000_000; // 0.01 SOL
pub const MAX_SOL_PAYOUT_LAMPORTS: u64 = 100 * LAMPORTS_PER_SOL; // 100 SOL

// Jackpot wheel pots (fixed SOL amounts)
pub const JACKPOT_POTS_SOL: [u64; 5] = [
    1_000 * LAMPORTS_PER_SOL, // 1,000 SOL
    750 * LAMPORTS_PER_SOL,   // 750 SOL
    690 * LAMPORTS_PER_SOL,   // 690 SOL
    510 * LAMPORTS_PER_SOL,   // 510 SOL
    420 * LAMPORTS_PER_SOL,   // 420 SOL
];

// Jackpot probability (0.20% = 20 out of 10,000)
pub const JACKPOT_CHANCE_BP: u16 = 20;

// Exclusivity bonus multipliers (in percentage) - DEGEN EDITION ✦✦✦
pub const LOOT_FIRST_CHANCE_MULT: u32 = 150; // 1.5x chance (global max level)
pub const LOOT_FIRST_VAULT_MULT: u64 = 300; // 3.0x vault (global max level)

pub const LOOT_TOP10_CHANCE_MULT: u32 = 120; // 1.2x chance (≤10 users at level)
pub const LOOT_TOP10_VAULT_MULT: u64 = 150; // 1.5x vault (≤10 users at level)
pub const LOOT_TOP25_CHANCE_MULT: u32 = 110; // 1.1x chance (≤25 users at level)
pub const LOOT_TOP25_VAULT_MULT: u64 = 120; // 1.2x vault (≤25 users at level)

pub const MAX_VAULT_SLICE_BP: u64 = 1_000; // 10 %

// ========================================================================================
// =============================== DRAGON EGG DNA UTILITIES ===============================
// ========================================================================================

/// Generate random DNA for new dragon eggs
pub fn generate_dragon_egg_dna(slot: u64, owner: &Pubkey, index: u64) -> [u8; 32] {
    let mut dna = [0u8; 32];

    // Use slot, owner, and index as entropy sources
    let seed = slot
        .wrapping_add(index)
        .wrapping_mul(1103515245)
        .wrapping_add(12345);

    for i in 0..32 {
        let random_value = seed
            .wrapping_mul(owner.to_bytes()[i % 32] as u64)
            .wrapping_add(i as u64 * 31)
            .wrapping_mul(1103515245);

        dna[i] = (random_value & 0xFF) as u8;
    }

    dna
}

// ========================================================================================
// =============================== DRAGON EGG NFT METADATA ===============================
// ========================================================================================

/// Dragon Egg NFT metadata (stored in moonbase program for simplicity)
#[account]
pub struct DragonEggMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,

    /// Current power level
    pub power: u32,

    /// DNA data (32 bytes for breeding/evolution)
    pub dna: [u8; 32],

    /// Moonbase this egg is incubated in (if any)
    pub incubated_moonbase: Option<Pubkey>,

    /// Multiplier for this egg based on pricing tier (basis points, e.g., 150 = 1.5x, 200 = 2.0x, 300 = 3.0x)
    pub multiplier: u32,

    /// Faction ID (country) that the egg belongs to (matches moonbase faction)
    pub faction_id: u8,

    /// Last power update timestamp
    pub last_update_ts: i64,

    /// Creation timestamp
    pub created_at: i64,

    /// PDA bump
    pub bump: u8,
}

impl DragonEggMetadata {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // mint
        4 +     // power
        32 +    // dna
        33 +    // incubated_moonbase (Option<Pubkey>)
        4 +     // multiplier
        1 +     // faction_id
        8 +     // last_update_ts
        8 +     // created_at
        1; // bump
}

// ========================================================================================
// =============================== MOONBASE INCUBATION STATE =============================
// ========================================================================================

/// Incubation state for a specific moonbase (tracks incubated egg)
#[account]
pub struct IncubationState {
    /// Moonbase owner
    pub moonbase_owner: Pubkey,

    /// Incubated egg mint (Option for max 1 egg)
    pub incubated_egg: Option<Pubkey>,

    /// Last update timestamp
    pub last_update_ts: i64,

    /// Total power accumulated
    pub total_power: u64,

    /// PDA bump
    pub bump: u8,
}

impl IncubationState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // moonbase_owner
        33 +    // incubated_egg (Option<Pubkey>)
        8 +     // last_update_ts
        8 +     // total_power
        1; // bump
}
