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

pub const DISCRIMINATOR_SIZE: usize = 8;

// ========== FACTION SURGE RAFFLE CONSTANTS ========== //

pub const ROUND_DURATION_SECONDS: i64 = 600; // 10 minutes
pub const HASHPOWER_PER_SOL_CONSTANT: u128 = 1_000_000; // 1 SOL = 1M hashpower (adjustable)

pub const MOTHERLODE_CHANCE: u64 = 625; // 1 in 625 chance (0.16%)

pub const MAX_FACTIONS: usize = 12; // 12 factions for the raffle
pub const NUM_FACTIONS: usize = 12; // Same as MAX_FACTIONS, used for array sizes
pub const MAX_FACTION_NAME_LENGTH: usize = 16; // Maximum length of faction name

// ========== BLOCK RAFFLE CONSTANTS ========== //
pub const NUM_BLOCKS: usize = 24; // 24 blocks total
pub const BLOCKS_PER_FACTION: usize = 2; // Each faction gets 2 blocks
 
// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / DogeBtcMining state
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";
pub const DOGE_BTC_MINING_SEED: &[u8] = b"moon-doge-mining";

// PDAs which hold SOL collected by the program
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
pub const DOGE_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"mdoge-vault-authority";
pub const DOGE_BTC_VAULT_SEED: &[u8] = b"dbtc_vault";
 
pub const REFERRAL_REWARDS_SEED: &[u8] = b"referral-rewards";
pub const COLLECTION_AUTHORITY_SEED: &[u8] = b"collection_authority";

// PDAs for Dragon Egg NFT system
pub const DRAGON_EGG_METADATA_SEED: &[u8] = b"dragon-egg-metadata";
pub const INCUBATION_STATE_SEED: &[u8] = b"incubation-state";
pub const DRAGON_EGG_CUSTODY_SEED: &[u8] = b"dragon-egg-custody"; // PDA that holds locked NFTs

pub const BUYBACKS_SEED: &[u8] = b"buybacks";
pub const BUYBACKS_SOL_VAULT_SEED: &[u8] = b"buybacks-sol-vault";

// PDAs for Game system
pub const GLOBAL_GAME_STATE_SEED: &[u8] = b"global-game-state";
pub const FACTION_STATE_SEED: &[u8] = b"faction";
pub const PLAYER_DATA_SEED: &[u8] = b"player";
pub const GAME_SESSION_SEED: &[u8] = b"game-session"; // Seed: [b"game-session", round_id_u64]
pub const USER_GAME_BET_SEED: &[u8] = b"user-bet"; // Seed: [b"user-bet", user_pubkey, round_id_u64]
pub const AUTOMINER_VAULT_SEED: &[u8] = b"autominer";
pub const SOL_PRIZE_POT_VAULT_SEED: &[u8] = b"sol-prize-pot";
pub const MOTHERLODE_POT_VAULT_SEED: &[u8] = b"motherlode-pot";
pub const DBTC_EMISSION_VAULT_SEED: &[u8] = b"dbtc-emission-vault";
pub const STAKER_SOL_REWARD_VAULT_SEED: &[u8] = b"staker-sol-reward-vault";

/// ------------ GLOBAL CONFIG ------------

/// Global configuration for the Moon Facility program
#[account]
pub struct GlobalConfig {

    /// Whether the game is currently active
    pub is_game_active: bool,

    /// total number of players in the game
    pub total_players: u64,

    /// Authority that can update config parameters
    pub ext_authority: Pubkey,
    /// External account that can withdraw collected SOL
    pub ext_fee_collector: Pubkey,
    /// Direct recipient for egg mints revenue
    pub creation_fee_recipient: Pubkey,
    /// PDA account that holds collected SOL fees
    pub pda_sol_treasury: Pubkey,

    /// List of supported factions (e.g., "USA", "China", "Russia")
    /// Maximum 15 factions, each with max 16 characters
    pub supported_factions: Vec<String>,

    /// SOL fee distribution configuration
    pub sol_fee_config: SolFeeConfig,

    /// DogeBtc distribution configuration
    pub dbtc_dist_config: DogeBtcDistConfig,

    /// Authorized Raydium pool state address (security: prevents using malicious pools)
    pub raydium_pool_state: Pubkey,

    /// ------------------------------------------------------------           
    /// Bump for GlobalConfig PDA derivation
    pub bump: u8,
    /// Bump for SOL treasury PDA derivation
    pub treasury_bump: u8,

    /// Dragon Egg collection address (Metaplex Core)
    pub dragon_egg_collection: Pubkey,
    /// Dragon Egg URIs organized by tier and faction
    /// Structure: [tier][faction_id] = URI
    /// 4 tiers (1, 2, 3, 4), each with URIs for each faction
    pub dragon_egg_uris: Vec<Vec<String>>, // [tier][faction_index] = URI (tier 1-4)
    /// Egg limits per tier: [tier1_limit, tier2_limit, tier3_limit, tier4_limit]
    /// All tiers: 5000 eggs each
    pub egg_limits: [u64; 4],
    /// Global total power across all Dragon Eggs (sum of all egg powers)
    pub global_dragon_egg_power: u64,
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
pub struct DogeBtcDistConfig {
    /// Percentage of DogeBtc emission that goes to stakers
    pub dbtc_stakers_pct: u8,
    /// Percentage of DogeBtc emission that goes to winning block bettors
    pub dbtc_winners_pct: u8,
    /// Percentage of DogeBtc emission that goes to losing block bettors
    pub dbtc_same_faction_pct: u8,
    /// Percentage of DogeBtc emission that goes to motherlode
    pub dbtc_motherlode_pct: u8,
    /// Refining fee
    pub refining_fee: u8,
}

impl DogeBtcDistConfig {
    pub const LEN: usize = 1 + 1 + 1 + 1; // All 4 percentages
}
 


impl GlobalConfig {
    // discriminator + is_game_active + ext_authority + ext_fee_collector + creation_fee_recipient + pda_sol_treasury + sol_fee_config + dbtc_dist_config + bump + treasury_bump + supported_factions (vec) + raydium_pool_state + dragon_egg_collection + dragon_egg_uris (vec of vec) + egg_limits + global_dragon_egg_power
    // Vec<String> = 4 bytes (vec length) + MAX_FACTIONS * (4 bytes string length + MAX_FACTION_NAME_LENGTH bytes)
    // Vec<Vec<String>> = 4 bytes (outer vec length) + 4 tiers * (4 bytes inner vec length + MAX_FACTIONS * (4 + MAX_URI_LENGTH))
    pub const LEN: usize = DISCRIMINATOR_SIZE + 
        1 +                     // is_game_active
        32 +                    // ext_authority
        32 +                    // ext_fee_collector  
        32 +                    // creation_fee_recipient
        32 +                    // pda_sol_treasury
        SolFeeConfig::LEN +     // sol_fee_config
        DogeBtcDistConfig::LEN + // dbtc_dist_config
        1 +                     // bump
        1 +                     // treasury_bump
        4 + (MAX_FACTIONS * (4 + MAX_FACTION_NAME_LENGTH)) + // supported_factions vec
        32 +                    // raydium_pool_state
        32 +                    // dragon_egg_collection
        4 + (4 * (4 + MAX_FACTIONS * (4 + MAX_URI_LENGTH))) + // dragon_egg_uris: 4 tiers × factions
        (4 * 8) +               // egg_limits [u64; 4] = 32 bytes
        8;                      // global_dragon_egg_power

    /// Get Dragon Egg URI for a specific tier and faction
    pub fn get_dragon_egg_uri(&self, tier: u8, faction_id: u8) -> Result<String> {
        require!(tier >= 1 && tier <= 4, ErrorCode::InvalidParameters);
        require!(
            (faction_id as usize) < self.supported_factions.len(),
            ErrorCode::InvalidFactionId
        );
        
        let tier_index = (tier - 1) as usize; // tier 1->0, 2->1, 3->2, 4->3
        require!(
            tier_index < self.dragon_egg_uris.len(),
            ErrorCode::InvalidMetadata
        );
        
        let faction_uris = &self.dragon_egg_uris[tier_index];
        require!(
            (faction_id as usize) < faction_uris.len(),
            ErrorCode::InvalidMetadata
        );
        
        Ok(faction_uris[faction_id as usize].clone())
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
    /// Total tokens mined so far
    pub total_tokens_mined: u64,

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

    /// Total SOL bets since start of game (cumulative across all rounds)
    pub total_sol_bets: u128,

    /// The total "shares" (hashpower) across all passive stakers.
    pub total_global_passive_hashpower: u128,

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
}

impl GlobalGameSate {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        16 +    // total_sol_bets (u128)
        16 +    // total_global_passive_hashpower (u128)
        8 +     // current_round_id
        8 +     // round_end_timestamp
        8 +     // round_duration_seconds
        8 +     // last_round_id
        1;      // winning_faction_id
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
    pub total_passive_hashpower: u128,

    /// Total SOL bet on this faction across all rounds (cumulative)
    pub total_sol_bets: u64,
    /// Total number of rounds this faction has won (cumulative)
    pub total_wins: u64,
    
    /// Cumulative SOL-per-share this faction has earned for stakers
    /// Used for calculating staker rewards
    pub sol_reward_index: u128,
    /// Cumulative DogeBtc-per-share this faction has earned for stakers
    /// Used for calculating staker rewards
    pub dbtc_reward_index: u128,

    /// Current motherlode pot size for this faction
    pub motherlode_pot_size: u64
}

impl FactionState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // faction_id
        16 +    // total_passive_hashpower (u128)
        8 +     // total_sol_bets
        8 +     // total_wins
        16 +    // sol_reward_index (u128)
        16 +    // dbtc_reward_index (u128)
        8;      // motherlode_pot_size
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

    /// The round ID this session belongs to
    pub round_id: u64,

    /// Timestamp when this round started
    pub round_start_timestamp: i64,
    /// Timestamp when this round ends
    pub round_end_timestamp: i64,

    /// Total SOL bets placed in this round
    pub total_sol_bets: u64,
    /// Total points bets placed in this round
    pub total_points_bets: u64,

    /// Indexes of UserGameBet PDAs for SOL bets in this round
    /// Used to track all bets and calculate rewards
    pub sol_bets_indexes: Vec<u64>,
    /// Indexes of UserGameBet PDAs for points bets in this round
    pub points_bets_indexes: Vec<u64>,

    /// Block assignments: [block_0, block_1, ..., block_23]
    /// Each element is the faction_id assigned to that block (0-indexed, blocks are 1-24)
    /// Set at round start when factions are randomly assigned to blocks
    pub block_assignments: [u8; NUM_BLOCKS],

    /// The winning block number for this round (1-24)
    pub winning_block: u8,
    /// The winning faction ID for this round (derived from winning_block)
    pub winning_faction_id: u8,

    // --- Round-specific payout data ---
    /// Total SOL pot for winners (net after fees)
    pub total_sol_pot_net: u64,
    /// Total SOL bet on the winning faction (for pro-rata distribution)
    pub total_sol_bet_on_winner: u64,
    /// Total SOL bet on losing factions (for pro-rata distribution)
    pub total_sol_bet_on_losers: u64,
    /// Total SOL bet across all factions (for motherlode distribution)
    pub total_sol_bet_all_factions: u64,

    // --- DogeBtc reward pools for this round ---
    /// DogeBtc allocated for winners in this round
    pub dbtc_winner_pool: u64,
    /// DogeBtc allocated for same-faction bettors in this round
    pub dbtc_loser_pool: u64,

    // --- Motherlode data for this round ---
    /// Whether motherlode was hit in this round
    pub motherlode_hit: bool,
    /// Motherlode pot size when hit (if applicable)
    pub motherlode_pot_size_on_hit: u64,
}

impl GameSession {
    // Maximum number of bets per round (for Vec sizing)
    pub const MAX_BETS_PER_ROUND: usize = 10000; // Reasonable limit per round
    
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // round_id
        8 +     // round_start_timestamp
        8 +     // round_end_timestamp
        8 +     // total_sol_bets
        8 +     // total_points_bets
        4 + (Self::MAX_BETS_PER_ROUND * 8) + // sol_bets_indexes Vec<u64>
        4 + (Self::MAX_BETS_PER_ROUND * 8) + // points_bets_indexes Vec<u64>
        (NUM_BLOCKS * 1) + // block_assignments [u8; NUM_BLOCKS]
        1 +     // winning_block (u8)
        1 +     // winning_faction_id
        8 +     // total_sol_pot_net
        8 +     // total_sol_bet_on_winner
        8 +     // total_sol_bet_on_losers
        8 +     // total_sol_bet_all_factions
        8 +     // dbtc_winner_pool
        8 +     // dbtc_loser_pool
        1 +     // motherlode_hit (bool)
        8;      // motherlode_pot_size_on_hit
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

    /// List of round IDs where the player has placed bets
    /// Used to track rounds with unclaimed rewards or unclosed sessions
    pub sol_bets_rounds: Vec<u64>,
    /// Corresponding SOL bet amounts for each round in sol_bets_rounds
    /// Index matches sol_bets_rounds (e.g., sol_bets_amounts[0] is the bet for sol_bets_rounds[0])
    pub sol_bets_amounts: Vec<u64>,

    /// Cumulative statistics
    pub rounds_played: u64,
    pub rounds_won: u64,
    pub total_sol_bet: u64,
    pub total_points_bet: u64,
    pub total_sol_won: u64,
    pub total_dbtc_won: u64,

    /// The user's "shares" (hashpower) in the passive global pool
    /// Updated by mooneconomy program via CPI
    pub personal_passive_hashpower: u128,

    /// Reward debt tracking (prevents double-claiming)
    /// The last passive_dbtc_reward_index the user claimed up to
    pub last_claimed_passive_dbtc_index: u128,
    /// The last passive_sol_reward_index the user claimed up to
    pub last_claimed_passive_sol_index: u128,    
}

impl PlayerData {
    // Maximum number of active rounds a player can track (for Vec sizing)
    pub const MAX_ACTIVE_ROUNDS: usize = 100; // Reasonable limit for unclaimed rounds
    
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // owner
        32 +    // referral_code
        1 +     // faction_id
        4 + (Self::MAX_ACTIVE_ROUNDS * 8) + // sol_bets_rounds Vec<u64>
        4 + (Self::MAX_ACTIVE_ROUNDS * 8) + // sol_bets_amounts Vec<u64>
        8 +     // rounds_played
        8 +     // rounds_won
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_sol_won
        8 +     // total_dbtc_won
        16 +    // personal_passive_hashpower (u128)
        16 +    // last_claimed_passive_dbtc_index (u128)
        16;     // last_claimed_passive_sol_index (u128)
}



/// Stores referral rewards that a user has earned from referrals
#[account]
pub struct ReferralRewards {
    pub owner: Pubkey,
    pub total_sol_earned: u64,
    pub bump: u8,
    /// Number of users who have used this user's referral code
    pub referrals_count: u16,
}

impl ReferralRewards {
    // discriminator + owner + total_sol_earned + sol_claimed_for_xp + bump + referrals_count
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 8 + 8 + 1 + 2;
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
    pub incubated_player_data: Option<Pubkey>,

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
// ============================= BET TYPE ENUM ==============================
// ========================================================================================

/// Bet type enum for user bets
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq)]
pub enum BetType {
    /// Direct block selection (block_id: 1-24)
    Block { block_id: u8 },
    /// Faction + highest/lowest selection (faction_id + is_highest)
    FactionHighestLowest { faction_id: u8, is_highest: bool },
}

impl BetType {
    // Anchor enum serialization: 1 byte discriminator + max variant size
    // Block variant: 1 byte discriminator + 1 byte block_id = 2 bytes
    // FactionHighestLowest variant: 1 byte discriminator + 1 byte faction_id + 1 byte is_highest = 3 bytes
    // Max size is 3 bytes
    pub const LEN: usize = 3;
}

// ========================================================================================
// ============================= FACTION SURGE ACCOUNTS ==============================
// ========================================================================================

/// User Game Bet PDA (Seed: `[b"user-bet", user_pubkey, round_id_u64]`)
/// Each user bet in a round has its own PDA account.
/// Users can bet on either:
/// - A specific block (1-24)
/// - A faction + highest/lowest option (which maps to one of the faction's 2 blocks)
#[account]
pub struct UserGameBet {
    /// The user who placed this bet
    pub owner: Pubkey,
    /// The round ID this bet belongs to
    pub round_id: u64,
    /// The bet type (Block or FactionHighestLowest)
    pub bet_type: BetType,
    /// The net SOL amount bet (after protocol fee deduction)
    pub sol_bet_amount: u64,
    pub bump: u8,
}

impl UserGameBet {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        8 +     // round_id
        BetType::LEN + // bet_type (enum)
        8 +     // sol_bet_amount
        1;      // bump
    
    /// Get the target block ID for this bet based on GameSession block assignments
    /// Returns None if bet is invalid or block assignments not set
    pub fn get_target_block(&self, block_assignments: &[u8; NUM_BLOCKS]) -> Option<u8> {
        match &self.bet_type {
            BetType::Block { block_id } => {
                if *block_id >= 1 && *block_id <= NUM_BLOCKS as u8 {
                    Some(*block_id)
                } else {
                    None
                }
            }
            BetType::FactionHighestLowest { faction_id, is_highest } => {
                // Find the two blocks assigned to this faction
                let mut faction_blocks: Vec<u8> = Vec::new();
                for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                    if *assigned_faction == *faction_id {
                        faction_blocks.push((block_idx + 1) as u8); // block_id is 1-indexed
                    }
                }
                
                if faction_blocks.len() == BLOCKS_PER_FACTION {
                    if *is_highest {
                        Some(*faction_blocks.iter().max().unwrap())
                    } else {
                        Some(*faction_blocks.iter().min().unwrap())
                    }
                } else {
                    None
                }
            }
        }
    }
}

/// Autominer Vault PDA (Seed: `[b"autominer", user_pubkey]`)
/// This PDA also acts as a SOL vault by holding lamports
#[account]
pub struct AutominerVault {
    pub owner: Pubkey,
    pub faction_id: u8,
    pub sol_per_round: u64,
    pub rounds_remaining: u32,
    pub vault_bump: u8,
}

impl AutominerVault {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // faction_id
        8 +     // sol_per_round
        4 +     // rounds_remaining
        1;      // vault_bump
}
