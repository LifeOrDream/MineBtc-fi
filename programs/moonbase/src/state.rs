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

// ========== FACTION SURGE RAFFLE CONSTANTS ========== //
pub const EMISSION_PER_ROUND: u64 = 10_000_000_000; // 10k DogeBtc per round
pub const ROUND_DURATION_SECONDS: i64 = 600; // 10 minutes
pub const HASHPOWER_PER_SOL_CONSTANT: u128 = 1_000_000; // 1 SOL = 1M hashpower (adjustable)
pub const MOTHERLODE_CHANCE: u64 = 625; // 1 in 625 chance (0.16%)
pub const SURGE_FEE_PERCENTAGE: u64 = 10; // 10% fee on bets
pub const SURGE_FEE_BUYBACK_PERCENTAGE: u64 = 40; // 40% of fee to buybacks
pub const SURGE_FEE_STAKER_PERCENTAGE: u64 = 40; // 40% of fee to stakers
pub const SURGE_FEE_ADMIN_PERCENTAGE: u64 = 20; // 20% of fee to admin
pub const MAX_FACTIONS: usize = 11; // 11 factions for the raffle
pub const MAX_FACTION_NAME_LENGTH: usize = 16; // Maximum length of faction name

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

// PDAs for Faction Surge raffle system
pub const GLOBAL_SURGE_STATE_SEED: &[u8] = b"global-surge-state";
pub const FACTION_STATE_SEED: &[u8] = b"faction";
pub const PLAYER_DATA_SEED: &[u8] = b"player";
pub const USER_SURGE_BET_SEED: &[u8] = b"bet";
pub const AUTOMINER_VAULT_SEED: &[u8] = b"autominer";
pub const SOL_PRIZE_POT_VAULT_SEED: &[u8] = b"sol-prize-pot";
pub const MOTHERLODE_POT_VAULT_SEED: &[u8] = b"motherlode-pot";
pub const DBTC_EMISSION_VAULT_SEED: &[u8] = b"dbtc-emission-vault";
pub const STAKER_SOL_REWARD_VAULT_SEED: &[u8] = b"staker-sol-reward-vault";

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
    // discriminator + authority + fee_collector + creation_fee_recipient + sol_treasury +  total_moonbases_created + total_sol_spent + total_referral_sol_paid + loot_percentage + buyback_percentage + is_game_active + bump + treasury_bump + supported_factions (vec) + dragon_egg_collection + total_dragon_eggs_minted + dragon_egg_uris + egg_limits + raydium_pool_state
    // Vec<String> = 4 bytes (vec length) + MAX_FACTIONS * (4 bytes string length + MAX_FACTION_NAME_LENGTH bytes)
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

// UserMoonBaseInstance and ModuleInstance removed - replaced by PlayerData and Faction Surge system

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

 
// ModuleInstance and ModuleRuntimeState removed - no longer needed for Faction Surge system

// ========== DRAGON EGG NFT CONSTANTS ========== //
pub const BASE_EGG_POWER: u32 = 100;

pub const MAX_DRAGON_EGG_URIS: usize = 20; // Max URIs in GlobalConfig
pub const MAX_URI_LENGTH: usize = 200;


 
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
// =============================== FACTION SURGE RAFFLE SYSTEM ===========================
// ========================================================================================

/// Global state for the raffle game
#[account]
pub struct GlobalSurgeState {
    pub current_round_id: u64,
    pub round_end_timestamp: i64,
    pub motherlode_pot: u64, // DogeBtc units

    // --- Data from the *previous* round (for claiming) ---
    pub last_round_id: u64,
    pub winning_faction_id: u8,
    
    // SOL Payout Data
    pub total_sol_pot_net: u64, // The 90% pot for winners
    pub total_sol_bet_on_winner: u64, // For pro-rata SOL payout
    pub total_sol_bet_on_losers: u64, // For pro-rata DogeBtc loser payout
    pub total_sol_bet_all_factions: u64, // For Motherlode payout
    
    // DogeBtc Payout Data
    pub dbtc_winner_pool: u64,   // 30% of emission
    pub dbtc_loser_pool: u64,    // 10% of emission
    
    // Motherlode Data
    pub motherlode_hit: bool,
    pub motherlode_pot_size_on_hit: u64,
    
    pub bump: u8,
}

impl GlobalSurgeState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +     // current_round_id
        8 +     // round_end_timestamp
        8 +     // motherlode_pot
        8 +     // last_round_id
        1 +     // winning_faction_id
        8 +     // total_sol_pot_net
        8 +     // total_sol_bet_on_winner
        8 +     // total_sol_bet_on_losers
        8 +     // total_sol_bet_all_factions
        8 +     // dbtc_winner_pool
        8 +     // dbtc_loser_pool
        1 +     // motherlode_hit
        8 +     // motherlode_pot_size_on_hit
        1;      // bump
}

/// State for each Faction (PDA seeded by `[b"faction", faction_id_u8]`)
#[account]
pub struct FactionState {
    pub faction_id: u8,
    pub total_passive_hashpower: u128, // Sum of all stakers' hashpower
    pub total_active_sol_bets: u64,  // Total SOL bet *this round*. Reset every round.
    pub bump: u8,
}

impl FactionState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // faction_id
        16 +    // total_passive_hashpower (u128)
        8 +     // total_active_sol_bets
        1;      // bump
}

/// Simple Player Data (replaces UserMoonBaseInstance) (PDA seeded by `[b"player", user_pubkey]`)
#[account]
pub struct PlayerData {
    pub owner: Pubkey,
    pub faction_id: u8,
    pub personal_passive_hashpower: u128, // This user's total hashpower from staking
    pub bump: u8,
}

impl PlayerData {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // faction_id
        16 +    // personal_passive_hashpower (u128)
        1;      // bump
}

/// The "Bet Slip" (PDA seeded by `[b"bet", user_pubkey, round_id_u64]`)
#[account]
pub struct UserSurgeBet {
    pub owner: Pubkey,
    pub round_id: u64,
    pub faction_id: u8,
    pub sol_bet_amount: u64, // The 90% (net) amount
    pub bump: u8,
}

impl UserSurgeBet {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        8 +     // round_id
        1 +     // faction_id
        8 +     // sol_bet_amount
        1;      // bump
}

/// Autominer Vault (PDA seeded by `[b"autominer", user_pubkey]`)
#[account]
pub struct AutominerVault {
    pub owner: Pubkey,
    pub faction_id: u8,
    pub sol_per_round: u64,
    pub rounds_remaining: u32,
    pub vault_bump: u8,
    // This PDA also acts as a SOL vault by holding lamports
}

impl AutominerVault {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // faction_id
        8 +     // sol_per_round
        4 +     // rounds_remaining
        1;      // vault_bump
}
