// # State Definitions
//
// This module defines all the account structures and constants used in the degenBTC program.
//
// ## Key Accounts
//
// - `GlobalConfig`: Stores global configuration parameters (fees, authorities, etc.).
// - `GlobalGameState`: Tracks the overall game state, including active rounds and pots.
// - `FactionState`: Stores statistics and reward pools for each faction.
// - `PlayerData`: Stores user-specific data, including stats, balances, and staking positions.
// - `GameSession`: Represents a single game round, tracking bets and outcomes.
// - `DegenBtcMining`: Manages the mining emission and distribution logic.
// - `HashBeastConfig`: Collection, total minted count, and breeding state for HashBeasts.
// - `HashBeastMintConfig`: Mint-only genesis sale curve, ticket tiers, and per-faction caps.
// - `TaxConfig`: Configuration for the tax and burn system.
//

use anchor_lang::prelude::*;

pub const DBTC_DECIMALS: u8 = 6;
pub const DBTC_BASE_UNITS: u64 = 1_000_000;

pub const BASE_MULTIPLIER: u32 = 1000; // 1.0x
pub const GAMEPLAY_MAX_MULTIPLIER: u16 = 4200; // Maximum gameplay HashBeast multiplier (4.2x)
pub const PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER: u16 = 3000; // Maximum passive HashBeast staking multiplier (3.0x)
pub const MAX_EVOLUTION_STAGE: u8 = 7; // Highest evolution stage encoded in hashbeast DNA
pub const MAX_REBIRTH_COUNT: u8 = 7; // Highest rebirth generation encoded in hashbeast DNA

/// Base mutation chance in basis points (2000 = 20%).
/// Effective chance = BASE_CHANCE × mult_factor × faction_penalty × volume_factor
///                  × pacing_factor × claim_boost / scaling.
pub const MAX_BASE_CHANCE: u64 = 2000;

/// Per-faction penalty step: each prior story event in the round for the same faction
/// reduces the next attempt's chance.  Formula: 10000 / (10000 + count * STEP).
/// At STEP=5000: after 1 story event -> 67%, after 2 -> 50%, after 3 -> 40%.
pub const FACTION_MUTATION_PENALTY_STEP: u64 = 5000;

// ========== STORY-EVENT-DRIVEN FACTION-WAR SCORING CONSTANTS ========== //
/// Weight for an Evolution story event when computing faction-war score.
/// Mutation-bonus weights applied to a player's `wgtd_points × multiplier`
/// contribution when their round-claim triggers a mutation. Higher tiers
/// give a bigger leaderboard kick.
pub const MUTATION_BONUS_WEIGHT_EVOLUTION: u64 = 4;
pub const MUTATION_BONUS_WEIGHT_POWER: u64 = 2;
pub const MUTATION_BONUS_WEIGHT_TRAIT: u64 = 1;

pub const STORY_EVENT_ORIGIN_ROUND: u8 = 0;
pub const STORY_EVENT_ORIGIN_FACTION_WAR: u8 = 1;

/// `GameplayScoreAccumulated.score_source` values:
/// - `ROUND_WIN`: the round-end score-add that a winning country receives.
///   Equal to `total_wgtd_points_bets[winner]` of that round. `user` field
///   on the event is unused (set to `Pubkey::default()`).
/// - `JACKPOT_HIT`: the jackpot score-add that the independently selected
///   non-winning jackpot country receives when the jackpot pot is actually
///   paid out. Equal to `total_wgtd_points_bets[jackpot_faction]` of that round.
/// - `MUTATION_BONUS`: per-user kick added when their round-claim mutation
///   roll succeeds. Computed as
///   `user_wgtd_points_on_winner × active_multiplier / BASE_MULTIPLIER × mutation_weight`.
///   Also accrues to the user's `current_war_score` for MVP tracking.
pub const GAMEPLAY_SCORE_SOURCE_ROUND_WIN: u8 = 0;
pub const GAMEPLAY_SCORE_SOURCE_MUTATION_BONUS: u8 = 1;
pub const GAMEPLAY_SCORE_SOURCE_JACKPOT_HIT: u8 = 2;
///
/// ------------ CONSTANTS ------------
pub const DAY_IN_SECONDS: u64 = 86400;

pub const MAX_ALLOWED_POSITIONS: u8 = 7;
pub const EMERGENCY_WITHDRAWAL_PENALTY_PCT: u8 = 15;
/// Whole-percent precision used by fee and reward split config fields.
/// Example: `25` means 25%, `100` means 100%.
pub const PERCENTAGE_DENOMINATOR: u64 = 100;
pub const PERCENTAGE_DENOMINATOR_U8: u8 = PERCENTAGE_DENOMINATOR as u8;
pub const PERCENTAGE_DENOMINATOR_U16: u16 = PERCENTAGE_DENOMINATOR as u16;
pub const M_HUNDRED: u64 = PERCENTAGE_DENOMINATOR;
pub const BASIS_POINTS_DENOMINATOR: u64 = 10_000;
pub const CLAIMABLE_DBTC_SOURCE_ROUND: u8 = 0;
pub const CLAIMABLE_DBTC_SOURCE_FACTION_WAR: u8 = 1;
// Source values 2 and 3 were retired when passive staking dBTC moved out of
// the HODL-tax claimable flow.
pub const CLAIMABLE_DBTC_SOURCE_REFINING_SYNC: u8 = 4;

// ========== GAMEPLAY TUNING DEFAULTS ========== //

/// Faction-war mining pool split:
/// - base rewards: anyone who predicted a country's final direction correctly
/// - mvp rewards: top contributor per faction
/// - hashbeast bonus: gameplay HashBeasts backing the resolved home-country outcome
pub const DEFAULT_FACTION_WAR_BASE_REWARD_BPS: u16 = 7500;
pub const DEFAULT_FACTION_WAR_MVP_REWARD_BPS: u16 = 500;
pub const DEFAULT_FACTION_WAR_HASHBEAST_REWARD_BPS: u16 = 2000;

/// Story-event pacing defaults stored in `GameplayTuningConfig`.
pub const DEFAULT_BASE_MUTATION_CHANCE_BPS: u16 = MAX_BASE_CHANCE as u16; // 20%
pub const DEFAULT_MUTATION_CHANCE_FLOOR_BPS: u16 = 25; // 0.25%
pub const DEFAULT_MUTATION_CHANCE_CAP_BPS: u16 = 2500; // 25%
pub const DEFAULT_FACTION_VOLUME_THRESHOLD_LAMPORTS: u64 = 85_000_000; // ~0.1 SOL gross post-fee
pub const DEFAULT_EXTRA_VOLUME_THRESHOLD_PER_MUTATION_LAMPORTS: u64 = 85_000_000;
pub const DEFAULT_TARGET_MUTATIONS_PER_CYCLE: u16 = 12;
pub const DEFAULT_TARGET_ROUNDS_PER_CYCLE: u16 = 240;
pub const DEFAULT_PACING_MAX_ADJUSTMENT_BPS: u16 = 4000; // +/-40%
pub const FACTION_WAR_RANK_WEIGHT_BPS: [u16; NUM_FACTIONS] = [
    1500, 1200, 1000, 900, 800, 700, 700, 600, 600, 500, 400, 300,
];

// ========== DEFAULT CONFIG VALUES ========== //
// All config defaults in one place. Used by initialize() in admin.rs.

/// SOL fee taken from each bet as a whole percent of the bet amount.
pub const DEFAULT_PROTOCOL_FEE_PCT: u8 = 15;
/// Minimum protocol fee percentage. Referral cuts (up to 1% of gross via
/// `REFERRAL_FEE_BPS_SAME_FACTION = 100 bps`) are deducted **from** the
/// per-bet protocol fee in `internal_process_bets`. If `protocol_fee_pct`
/// were ever set below the max referral cut, that subtraction would
/// underflow and DOS every referred user's bet. We pin the floor at 1
/// (which equals the 1% same-faction referral cap) to keep referred flows
/// solvent regardless of admin config drift.
pub const MIN_PROTOCOL_FEE_PCT: u8 = 1;
/// Percent of accumulated protocol SOL used for buybacks / POL during economy cycle.
pub const DEFAULT_BUYBACK_PCT: u8 = 70;
/// Percent of per-bet SOL fee routed to the staker reward vault.
pub const DEFAULT_STAKERS_PCT: u8 = 20;
/// Fixed referral fee: 1% of gross bet / mint / breed price goes to referrer.
pub const REFERRAL_FEE_PCT: u8 = 1;
/// Referral fee basis points for cross-faction recruits (different country from referrer)
pub const REFERRAL_FEE_BPS_CROSS_FACTION: u16 = 50; // 0.5%
/// Referral fee basis points for same-faction recruits (same country as referrer)
pub const REFERRAL_FEE_BPS_SAME_FACTION: u16 = 100; // 1.0%
/// Default cycle SOL split: 5% of user bet reserved for faction-war rewards.
pub const DEFAULT_CYCLE_SOL_SPLIT_PCT: u8 = 5;
/// Default share of `distribute_sol_fees` SOL routed to `inventory_sweep_vault`
/// to fund permissionless NFT market making (sweep buys + keeper bounties).
pub const DEFAULT_NFT_MARKET_MAKING_PCT: u8 = 3;

/// degenBTC share of round emission sent to faction stakers.
pub const DEFAULT_DBTC_STAKERS_PCT: u8 = 5;
/// degenBTC share of round emission sent to exact country+direction winners.
pub const DEFAULT_DBTC_WINNERS_PCT: u8 = 50;
/// degenBTC share of round emission sent to each non-winning direction on the winning faction.
pub const DEFAULT_DBTC_SAME_FACTION_PCT: u8 = 20;
/// degenBTC share of round emission added to the global jackpot.
pub const DEFAULT_DBTC_JACKPOT_PCT: u8 = 5;
/// Percent fee taken when claiming staking rewards (redistributed to other stakers).
pub const DEFAULT_HODL_TAX_PCT: u8 = 5;

/// Minimum seconds between price snapshots in the economy cycle.
pub const DEFAULT_SNAPSHOT_INTERVAL: u64 = 1800; // 30 minutes

/// Price change threshold for emission rate adjustment (whole percent).
pub const DEFAULT_PRICE_CHANGE_THRESHOLD: u64 = 3; // 3%
/// Emission rate increase when price rises above threshold.
pub const DEFAULT_EMISSION_INCREASE_PCT: u64 = 1; // 1%
/// Emission rate decrease when price falls below threshold.
pub const DEFAULT_EMISSION_DECREASE_PCT: u64 = 3; // 3%

/// Faction-war cycle reward multiplier bounds.
/// Stored as basis points: 1_000 = 0.1x, 10_000 = 1x, 30_000 = 3x.
/// These are hard protocol caps for `total_dbtc_mined_in_rounds * multiplier`.
pub const MIN_FACTION_WAR_MINING_MULTIPLIER_BPS: u16 = 1_000;
pub const MAX_FACTION_WAR_MINING_MULTIPLIER_BPS: u16 = 30_000;

/// Default faction-war mining multiplier (1.0x = 10_000 bps).
pub const DEFAULT_MINING_MULTIPLIER_BPS: u16 = 10_000;
/// Default multiplier increase when price goes up (+3%).
pub const DEFAULT_MULTIPLIER_INCREASE_BPS: u16 = 300;
/// Default multiplier decrease when price goes down (-10%).
pub const DEFAULT_MULTIPLIER_DECREASE_BPS: u16 = 1000;
/// Default multiplier hard floor (0.1x).
pub const DEFAULT_MULTIPLIER_MIN_BPS: u16 = MIN_FACTION_WAR_MINING_MULTIPLIER_BPS;
/// Default multiplier hard ceiling (3.0x).
pub const DEFAULT_MULTIPLIER_MAX_BPS: u16 = MAX_FACTION_WAR_MINING_MULTIPLIER_BPS;

// ========== DECIMAL SCALING CONSTANTS ========== //

pub const INDEX_PRECISION: u64 = 1_000_000; // 1 million
pub const DISCRIMINATOR_SIZE: usize = 8;

// ========== ROUND RAFFLE CONSTANTS ========== //

pub const JACKPOT_CHANCE: u64 = 625; // 1 in 625 chance (0.16%)

pub const MAX_FACTIONS: usize = 12; // Up to 12 factions for the raffle
pub const NUM_FACTIONS: usize = 12; // Same as MAX_FACTIONS, used for array sizes
pub const MAX_FACTION_NAME_LENGTH: usize = 16; // Maximum length of faction name

/// Conservative upper-bound slot estimate used to schedule round entropy at round start.
/// This keeps the entropy slot after the round closes under normal slot timing, while the
/// finalize path can still fall back to the latest available slot hash if the scheduled hash
/// ages out before anybody settles the round.
pub const ROUND_ENTROPY_SLOTS_PER_SECOND_ESTIMATE: u64 = 4;
/// Extra slot buffer added on top of the estimated end slot before sampling entropy.
pub const ROUND_PRIMARY_ENTROPY_DELAY_SLOTS: u64 = 8;

// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / DegenBtcMining state
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";
pub const HASHPOWER_CONFIG_SEED: &[u8] = b"hashpower-config";
pub const MINE_BTC_MINING_SEED: &[u8] = b"mine-btc-mining";
pub const HODL_POOL_SEED: &[u8] = b"hodl-pool";

// PDAs which hold SOL collected by the program
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// DEGEN_BTC Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies DEGEN_BTC tokens)
pub const DEGEN_BTC_VAULT_AUTHORITY_SEED: &[u8] = b"degenBTC-vault-authority";
pub const DEGEN_BTC_VAULT_SEED: &[u8] = b"dbtc_vault";

pub const REFERRAL_REWARDS_SEED: &[u8] = b"referral-rewards";
pub const COLLECTION_AUTHORITY_SEED: &[u8] = b"collection_authority";

// PDAs for HashBeast NFT system
pub const HASHBEAST_METADATA_SEED: &[u8] = b"hashbeast-metadata";
pub const HASHBEAST_CUSTODY_SEED: &[u8] = b"hashbeast-custody"; // PDA that holds locked NFTs
pub const HASHBEAST_FREE_MINT_ALLOWANCE_SEED: &[u8] = b"hashbeast-free-mint-allowance";

pub const BUYBACKS_SEED: &[u8] = b"buybacks";
pub const BUYBACKS_SOL_VAULT_SEED: &[u8] = b"buybacks-sol-vault";

// PDAs for Game system
pub const GLOBAL_GAME_STATE_SEED: &[u8] = b"global-game-state";
pub const FACTION_STATE_SEED: &[u8] = b"faction";
pub const PLAYER_DATA_SEED: &[u8] = b"player";

// PDAs for Staking system
pub const STAKED_POSITION_SEED: &[u8] = b"staked-position";
pub const LP_STAKED_POSITION_SEED: &[u8] = b"lp-staked-position";

pub const DEGENBTC_CUSTODIAN_SEED: &[u8] = b"degenBTC-custodian";
pub const DEGENBTC_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"degenBTC-custodian-authority";
pub const LIQUIDITY_CUSTODIAN_SEED: &[u8] = b"lp-custodian";
pub const LIQUIDITY_CUSTODIAN_AUTHORITY_SEED: &[u8] = b"lp-custodian-authority";

pub const GAME_SESSION_SEED: &[u8] = b"game-session"; // Seed: [b"game-session", round_id_u64]
pub const USER_GAME_BET_SEED: &[u8] = b"user-bet"; // Seed: [b"user-bet", user_pubkey, round_id_u64]
pub const AUTOMINER_VAULT_SEED: &[u8] = b"autominer";
pub const AUTOMINER_CUSTODY_SEED: &[u8] = b"autominer-custody";
pub const JACKPOT_POT_VAULT_SEED: &[u8] = b"jackpot-pot";

pub const STAKER_SOL_REWARD_VAULT_SEED: &[u8] = b"staker-sol-reward-vault";
pub const HASHBEAST_CONFIG_SEED: &[u8] = b"hashbeast-config";
pub const HASHBEAST_MINT_CONFIG_SEED: &[u8] = b"hashbeast-mint-config";

// PDAs for Faction War system
pub const FACTION_WAR_CONFIG_SEED: &[u8] = b"faction-war-config";
pub const FACTION_WAR_STATE_SEED: &[u8] = b"faction-war"; // Seed: [b"faction-war", war_id_u64]
pub const FACTION_WAR_SETTLEMENT_SEED: &[u8] = b"faction-war-settlement"; // Seed: [b"faction-war-settlement", war_id_u64]
pub const USER_FACTION_WAR_BETS_SEED: &[u8] = b"user-faction-war"; // Seed: [b"user-faction-war", user_pubkey, war_id_u64]
pub const FACTION_WAR_SOL_VAULT_SEED: &[u8] = b"faction-war-sol-vault";

// PDAs for Tax system
pub const TAX_CONFIG_SEED: &[u8] = b"tax-config";
pub const WITHDRAW_WITHHELD_AUTHORITY_SEED: &[u8] = b"withdraw-withheld-authority";
pub const FACTION_TREASURY_VAULT_SEED: &[u8] = b"faction-treasury-vault";

// PDAs for NFT rebirth inventory + lootbox + market integration
pub const INVENTORY_POOL_SEED: &[u8] = b"inventory-pool";
pub const REBORN_ENTRY_SEED: &[u8] = b"reborn-entry";
pub const INVENTORY_SWEEP_VAULT_SEED: &[u8] = b"inventory-sweep-vault";
/// Per-country lootbox queue. Seeds: `[b"lootbox-queue", &[faction_id]]`.
pub const LOOTBOX_QUEUE_SEED: &[u8] = b"lootbox-queue";
/// Per-user pending claim. Seeds: `[b"lootbox-claim", user.key().as_ref()]`.
pub const LOOTBOX_CLAIM_SEED: &[u8] = b"lootbox-claim";
/// Singleton sorted-floor queue. Seeds: `[b"floor-queue"]`.
pub const FLOOR_QUEUE_SEED: &[u8] = b"floor-queue";
/// Singleton ringbuffer of qualifying user-to-user sales. Seeds: `[b"sale-history"]`.
pub const SALE_HISTORY_SEED: &[u8] = b"sale-history";
/// Singleton 7-day rolling floor snapshot ringbuffer. Seeds: `[b"floor-history"]`.
pub const FLOOR_HISTORY_SEED: &[u8] = b"floor-history";

// ==========  HASHBEAST NFT CONSTANTS ========== //
pub const MAX_STAKED_HASHBEASTS: usize = 3; // Maximum number of hashbeasts a user can stake
pub const MAX_FREE_HASHBEAST_MINTS_PER_USER: u8 = 5;

pub const MAX_CALLER_COMPENSATION: u64 = 50_000; // 0.00005 SOL max keeper compensation per autominer round
pub const TICKET_AUTOMINER_CALLER_COMPENSATION: u64 = 50_000; // 0.00005 SOL keeper reserve for each ticket autominer round
pub const MIN_SOL_BET_PER_POSITION: u64 = 100_000; // 0.0001 SOL minimum per country-direction bet

// ==========  REBIRTH / LOOTBOX / MARKETPLACE TUNABLES ========== //
/// Maximum number of NFTs in the inventory PDA at any time.
pub const MAX_INVENTORY: u32 = 200;
/// XP value at which the quality_score's xp component saturates.
pub const MAX_XP_FOR_QUALITY: u32 = 100_000;
/// Number of per-parent price slots for breed counts 0..=4.
pub const BREED_PARENT_PRICE_COUNT: usize = 5;
/// Default per-parent breed prices used when HashBeastConfig is initialized.
pub const DEFAULT_BREED_PARENT_PRICE_LAMPORTS: [u64; BREED_PARENT_PRICE_COUNT] = [
    750_000_000,   // 0.75 SOL
    1_000_000_000, // 1.00 SOL
    1_350_000_000, // 1.35 SOL
    1_850_000_000, // 1.85 SOL
    2_500_000_000, // 2.50 SOL
];

/// Breeding price floor: total breed cost must be at least 1.5x the current
/// marketplace floor anchor.
pub const BREED_FLOOR_MULTIPLIER_BPS: u64 = 15_000;
/// Maximum age (in seconds) of the most recent floor snapshot at which the
/// breed-floor guard is still trusted. Snapshots are pushed every
/// `FLOOR_SNAPSHOT_INTERVAL_SECS` (24h), so allowing 48h gives a one-period
/// grace window. If the floor pipeline stalls beyond this, breeding is
/// blocked rather than letting it price against a stale anchor and undercut
/// the "always above floor" invariant.
pub const BREED_FLOOR_MAX_AGE_SECS: i64 = 48 * 60 * 60;
/// Breed payment split: half SOL, half dbTC by SOL value.
pub const BREED_SOL_SHARE_BPS: u64 = 5_000;
/// SOL breeding fees split: 50% of the SOL leg to fee_recipient, 50% to the
/// SOL treasury for the buybacks/economy loop.
pub const BREED_SOL_FEE_RECIPIENT_BPS: u64 = 5_000;
/// dbTC breeding fees split: 50% burned, 50% returned to the emission vault.
pub const BREED_DBTC_BURN_BPS: u64 = 5_000;

// ----- Loser-roll lootbox tunables -----
/// Number of asset slots a single country's queue holds. Rebirths fill the
/// queue first; if full, reborn assets are burned. Sweep buys also fill
/// the queue first; if full, the swept asset gets relisted at a markup
/// (or burned in deep bear). No "Pending" stranded path.
pub const LOOTBOX_QUEUE_SIZE: usize = 10;

/// Per-claim drop chance keyed by current `LootboxQueue.filled_count`.
/// Conservative fixed schedule in basis points. Full depth is still only 1.50%
/// per losing claim, so a full queue creates excitement without guaranteeing
/// a drain race.
pub const CHANCE_BPS_BY_QUEUE_DEPTH: [u16; LOOTBOX_QUEUE_SIZE + 1] =
    [0, 3, 8, 15, 25, 40, 58, 78, 100, 125, 150];

/// Inventory proceeds split: 50% sweep reserve, 50% protocol pipeline.
pub const INVENTORY_SWEEP_RESERVE_BPS: u16 = 5000;

// ----- Floor queue tunables -----
/// Number of cheapest user-listed entries tracked on-chain.
pub const FLOOR_QUEUE_SIZE: usize = 20;

// ----- Sale history tunables -----
/// Number of qualifying user-to-user sales tracked in the ringbuffer.
pub const SALE_HISTORY_SIZE: usize = 32;
/// Minimum seconds a listing must sit on the market before its sale qualifies
/// as a real-demand snapshot input (anti-snipe, anti-collusion).
pub const SALE_QUALIFY_MIN_LISTING_AGE_SECS: i64 = 5 * 60;
/// Window for "recent qualifying sales" when computing a snapshot anchor.
pub const SALE_RECENT_WINDOW_SECS: i64 = 24 * 60 * 60;
/// Minimum count of qualifying sales required before sale-median anchors;
/// below this, snapshot falls back to the registered floor queue. Set above
/// half the ringbuffer so a manipulator must control a true majority of the
/// recent sale samples before the median can move upward.
pub const MIN_SALES_FOR_ANCHOR: usize = 17;

// ----- Floor history tunables -----
/// 7-day rolling snapshot ringbuffer.
pub const FLOOR_HISTORY_SIZE: usize = 7;
/// Minimum seconds between snapshots (24h).
pub const FLOOR_SNAPSHOT_INTERVAL_SECS: i64 = 24 * 60 * 60;
/// Maximum age at which sweep/relist logic is allowed to trust the current
/// floor anchor. If snapshot cranking stalls beyond this, floor support stops
/// buying or repricing rather than spending against stale data.
pub const FLOOR_ANCHOR_MAX_AGE_SECS: i64 = 48 * 60 * 60;
/// Maximum upward move per snapshot when a previous anchor exists. Downward
/// moves are uncapped so panic-listing data can immediately make sweeps more
/// conservative; upward moves are capped to stop thin wash-trade bursts from
/// rapidly raising the vault's buy ceiling.
pub const FLOOR_ANCHOR_MAX_UPWARD_MOVE_BPS: u16 = 2_500;

// ----- Markup formula constants -----
/// Baseline markup applied to relists in flat market.
pub const RELIST_BASE_MARKUP_BPS: i32 = 1500;
/// Trend feeds into markup at this divider (half-trend).
pub const RELIST_TREND_DIVIDER: i32 = 2;
/// Lower bound on how much trend can subtract from markup (-10%).
pub const RELIST_TREND_MOD_FLOOR_BPS: i32 = -1000;
/// Upper bound on how much trend can add to markup (+30%).
pub const RELIST_TREND_MOD_CEILING_BPS: i32 = 3000;
/// Per-strike penalty applied per expire cycle (-5% / strike).
pub const RELIST_EXPIRE_PENALTY_BPS: i32 = 500;
/// Hard floor on markup; below this, deep bear forces burn.
pub const RELIST_MIN_MARKUP_BPS: i32 = -2000;
/// Hard ceiling on markup, sanity cap.
pub const RELIST_MAX_MARKUP_BPS: i32 = 6000;
/// Trend threshold for burning instead of relisting (default -30%).
pub const BURN_TREND_BPS_THRESHOLD: i32 = -3000;
/// Maximum number of expire cycles before forced burn.
pub const MAX_EXPIRES: u8 = 3;
/// Listing must be at least this old before `expire_program_listing` can fire (7d).
pub const EXPIRE_GRACE_SECS: i64 = 7 * 24 * 60 * 60;

// ----- Sweep guardrails -----
/// Sweep can buy at up to anchor × (1 + SWEEP_ATTRACTIVE_BPS/10000).
pub const SWEEP_ATTRACTIVE_BPS: u16 = 500; // +5%
/// Per-tx cap: cannot spend more than this fraction of sweep_vault on one buy (5%).
pub const SWEEP_MAX_PCT_BPS: u16 = 500;
/// Below this anchor (e.g., empty queue and history), sweep is disabled.
pub const SWEEP_MIN_ANCHOR_LAMPORTS: u64 = 10_000_000; // 0.01 SOL

// ----- Keeper rewards -----
/// Bounty paid to a keeper that purges a stale `FloorQueue` head (a queue
/// entry whose underlying marketplace listing has been canceled / mutated
/// out from under us). Deliberately much lower than `KEEPER_REWARD_LAMPORTS`
/// — high enough to compensate an honest bot's tx gas (~14k lamports) so
/// queue cleanup stays viable, low enough that an attacker can't farm it
/// by spamming `list_user_nft` → direct `marketplace::cancel_listing`
/// (bypassing the wrapper that would atomic-deregister) → `sweep_floor_lowest`
/// purge cycles. At 20k vs the ~24k-gas attack cycle cost, each spam round
/// is net-negative for the attacker, while honest bots still break even on
/// real cleanup work.
pub const STALE_PURGE_KEEPER_REWARD_LAMPORTS: u64 = 20_000;
/// Bounty paid from `inventory_sweep_vault` to caller of permissionless ix.
pub const KEEPER_REWARD_LAMPORTS: u64 = 500_000; // 0.0005 SOL

/// Minimum lamports retained in `inventory_sweep_vault` after any sweep buy.
pub const MIN_SWEEP_RESERVE_LAMPORTS: u64 = 50_000_000; // 0.05 SOL
///
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
    /// Direct recipient for hashbeast mints + dev earnings revenue
    pub fee_recipient: Pubkey,

    /// PDA account that holds collected SOL fees
    pub pda_sol_treasury: Pubkey,

    /// List of supported factions (e.g., "USA", "China", "Russia")
    /// Maximum 12 factions, each with max 16 characters
    pub supported_factions: Vec<String>,

    /// SOL fee distribution configuration
    pub sol_fee_config: SolFeeConfig,

    /// degenBTC distribution configuration
    pub dbtc_dist_config: DegenBtcDistConfig,

    /// Authorized Raydium pool state address (security: prevents using malicious pools)
    pub raydium_pool_state: Pubkey,

    /// Minimum time interval between price snapshots (in seconds)
    /// Default: 1800 seconds (30 minutes)
    pub snapshot_interval: u64,

    /// Unified gameplay and cycle-reward tuning surface.
    pub gameplay_tuning: GameplayTuningConfig,

    /// Authority-toggleable global pause. When true, blocks: new bets (manual
    /// + autominer), new round starts, hashbeast mints, and hashbeast breeds. Does NOT
    /// block: round settlement, all claims, staking/unstaking, economy cranks.
    /// Users can always exit; pending rounds always finish.
    pub is_paused: bool,

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
    /// Whole-percent share of the user's SOL bet reserved for the faction-war
    /// cycle SOL reward pool. Taken directly from the gross bet, in addition to the
    /// protocol fee. `100` = 100%.
    pub cycle_sol_split_pct: u8,
    /// Whole-percent share of `distribute_sol_fees` SOL routed to the
    /// `inventory_sweep_vault` PDA, funding permissionless NFT market making
    /// (sweep buys + keeper bounties). `100` = 100%.
    pub nft_market_making_pct: u8,
}

impl SolFeeConfig {
    pub const LEN: usize = 1 + 1 + 1 + 1 + 1; // protocol_fee_pct + buyback_pct + stakers_pct + cycle_sol_split_pct + nft_market_making_pct
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct DegenBtcDistConfig {
    /// Whole-percent share of degenBTC emission that goes to stakers. `100` = 100%.
    pub dbtc_stakers_pct: u8,
    /// Whole-percent share of degenBTC emission that goes to winning faction bettors. `100` = 100%.
    pub dbtc_winners_pct: u8,
    /// Whole-percent share of degenBTC emission that goes to each non-winning
    /// direction on the winning faction. With 3 total directions, up to two
    /// losing directions may each receive this share if they have bettors.
    pub dbtc_same_faction_pct: u8,
    /// Whole-percent share of degenBTC emission that goes to the global jackpot. `100` = 100%.
    pub dbtc_jackpot_pct: u8,
    /// Whole-percent HODL tax charged on degenBTC reward withdrawal.
    /// `100` = 100%. Paid by paper hands; redistributed to remaining diamond
    /// hands via `HodlPool::hodl_tax_index` (closed loop — no vault drain).
    pub hodl_tax_pct: u8,
}

impl DegenBtcDistConfig {
    pub const LEN: usize = 1 + 1 + 1 + 1 + 1; // dbtc_stakers_pct + dbtc_winners_pct + dbtc_same_faction_pct + dbtc_jackpot_pct + hodl_tax_pct
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +                     // total_players
        32 +                    // ext_authority
        32 +                    // pending_authority
        32 +                    // fee_recipient
        32 +                    // pda_sol_treasury
        SolFeeConfig::LEN +     // sol_fee_config
        DegenBtcDistConfig::LEN + // dbtc_dist_config
        32 +                    // raydium_pool_state
        8 +                     // snapshot_interval
        GameplayTuningConfig::LEN + // gameplay_tuning
        1 +                     // is_paused
        1 +                     // bump
        1 +                     // treasury_bump
        4 + (MAX_FACTIONS * (4 + MAX_FACTION_NAME_LENGTH)); // supported_factions vec
}

/// Unified gameplay tuning stored directly inside `GlobalConfig`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GameplayTuningConfig {
    /// Enable RPG progression (story events, XP, etc) during gameplay.
    pub rpg_progression: bool,
    /// Highest evolution stage currently unlocked by admin.
    /// `0` disables evolutions entirely, `1` allows stage 0 -> 1, etc.
    pub max_evolution_stage_unlocked: u8,

    /// Faction-war mining pool split in basis points. Must sum to 10_000.
    pub war_base_reward_bps: u16,
    pub war_mvp_reward_bps: u16,
    pub war_hashbeast_reward_bps: u16,

    /// Baseline mutation chance before runtime factors.
    pub base_mutation_chance_bps: u16,
    /// Final chance floor / cap after all runtime factors are applied.
    pub mutation_chance_floor_bps: u16,
    pub mutation_chance_cap_bps: u16,

    /// Per-faction additive volume controller. Required volume base (and
    /// per-mutation ramp) for the country's accumulated SOL bets since its
    /// last round win to fully unlock the volume_factor in the chance formula.
    pub faction_volume_threshold_lamports: u64,
    pub extra_volume_threshold_per_mutation_lamports: u64,

    /// Cycle pacing controller. Pacing factor alone regulates how many
    /// mutations land per cycle — it's a closed-loop controller comparing
    /// observed-vs-target. No separate cooldown controller needed.
    pub target_mutations_per_cycle: u16,
    pub target_rounds_per_cycle: u16,
    pub pacing_max_adjustment_bps: u16,
}

impl GameplayTuningConfig {
    pub const LEN: usize = 1 + // rpg_progression
        1 + // max_evolution_stage_unlocked
        2 + // war_base_reward_bps
        2 + // war_mvp_reward_bps
        2 + // war_hashbeast_reward_bps
        2 + // base_mutation_chance_bps
        2 + // mutation_chance_floor_bps
        2 + // mutation_chance_cap_bps
        8 + // faction_volume_threshold_lamports
        8 + // extra_volume_threshold_per_mutation_lamports
        2 + // target_mutations_per_cycle
        2 + // target_rounds_per_cycle
        2; // pacing_max_adjustment_bps

    pub fn is_uninitialized(&self) -> bool {
        !self.rpg_progression
            && self.max_evolution_stage_unlocked == 0
            && self.war_base_reward_bps == 0
            && self.war_mvp_reward_bps == 0
            && self.war_hashbeast_reward_bps == 0
    }

    pub fn apply_defaults(&mut self) {
        self.rpg_progression = false;
        self.max_evolution_stage_unlocked = 0;
        self.war_base_reward_bps = DEFAULT_FACTION_WAR_BASE_REWARD_BPS;
        self.war_mvp_reward_bps = DEFAULT_FACTION_WAR_MVP_REWARD_BPS;
        self.war_hashbeast_reward_bps = DEFAULT_FACTION_WAR_HASHBEAST_REWARD_BPS;
        self.base_mutation_chance_bps = DEFAULT_BASE_MUTATION_CHANCE_BPS;
        self.mutation_chance_floor_bps = DEFAULT_MUTATION_CHANCE_FLOOR_BPS;
        self.mutation_chance_cap_bps = DEFAULT_MUTATION_CHANCE_CAP_BPS;
        self.faction_volume_threshold_lamports = DEFAULT_FACTION_VOLUME_THRESHOLD_LAMPORTS;
        self.extra_volume_threshold_per_mutation_lamports =
            DEFAULT_EXTRA_VOLUME_THRESHOLD_PER_MUTATION_LAMPORTS;
        self.target_mutations_per_cycle = DEFAULT_TARGET_MUTATIONS_PER_CYCLE;
        self.target_rounds_per_cycle = DEFAULT_TARGET_ROUNDS_PER_CYCLE;
        self.pacing_max_adjustment_bps = DEFAULT_PACING_MAX_ADJUSTMENT_BPS;
    }
}

/// ------------ HASHBEAST-BTC MINING ------------
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
    /// Number of LP addition operations performed
    pub lp_operations_count: u32,
}

impl ProtocolOwnedLiquidity {
    pub const LEN: usize = 8 + 4; // total_lp_burnt + lp_operations_count

    /// Update POL stats after a successful LP addition and burn
    pub fn update_after_lp_operation(&mut self, lp_tokens_burnt: u64) {
        self.total_lp_burnt = self.total_lp_burnt.saturating_add(lp_tokens_burnt);
        self.lp_operations_count = self.lp_operations_count.saturating_add(1);
    }
}

/// HashBeast-BTC Mining status and parameters
#[account]
pub struct DegenBtcMining {
    /// Token vault that holds all pre-minted tokens
    pub dbtc_token_vault: Pubkey,
    /// degenBTC mined per slot (original base rate)
    pub dbtc_per_round: u64,

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

impl DegenBtcMining {
    // discriminator + dbtc_token_vault + mining_start_timestamp + dbtc_per_round + total_tokens_mined + bump + vault_auth_bump +
    // raydium_pool_state + last_rate_update + price_history (vec) + recent_price + track_price + pol_stats + lp_token_price_in_sol
    pub const MAX_PRICE_HISTORY_ENTRIES: usize = 8; // 4-hour cycle (8 × 30min snapshots)
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 32                    // dbtc_token_vault
        + 8                     // dbtc_per_round
        + 8                     // total_tokens_mined
        + 8                     // total_tokens_distributed
        + 1                     // bump
        + 1                     // vault_auth_bump
        + 32                    // raydium_pool_state
        + 8                     // last_rate_update (i64)
        + (4 + Self::MAX_PRICE_HISTORY_ENTRIES * PriceEntry::LEN) // price_history Vec<PriceEntry>
        + 8                     // recent_price
        + 8                     // track_price
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
    /// SOL earmarked for Protocol Owned Liquidity (in lamports)
    pub sol_for_pol: u64,
}

impl BuybacksAccount {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 8 + 8;
}

/// ------------ HASHPOWER CONFIG ------------
/// Hashpower configuration for the Minebtc program
#[account]
pub struct HashpowerConfig {
    /// Minimum lockup period in days
    pub min_lockup_days: u64,
    /// Maximum lockup period in days
    pub max_lockup_days: u64,

    /// Base multiplier for lockup duration (100 = 1x, separate from BASE_MULTIPLIER=1000 used for hashbeasts).
    pub base_multiplier: u16,
    /// Maximum lockup multiplier. Capped at 300 = 3x so total staking boost maxes at 9x with HashBeasts.
    pub max_multiplier: u16,
}

// For HashpowerConfig
impl HashpowerConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +     // min_lockup_days
        8 +     // max_lockup_days
        2 +     // base_multiplier (u16)
        2; // max_multiplier (u16)
}

// ModuleInstance and ModuleRuntimeState removed - no longer needed

/// Ticket tier option for hashbeast minting
/// When users mint hashbeasts, they choose a ticket tier which gives them free tickets
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TicketTier {
    /// Ticket value in lamports (e.g., 10_000_000 = 0.01 SOL)
    pub ticket_value: u64,
}

impl TicketTier {
    pub const LEN: usize = 8; // ticket_value
}

/// Global HashBeast configuration used outside the primary mint sale.
///
/// Seeds: `[b"hashbeast-config"]`. Singleton. Initialized once at deploy
/// (admin path) and mutated only via admin ix going forward.
///
/// **No lifetime supply cap.** Only the genesis sale is bounded (see
/// `HashBeastMintConfig.genesis_mint_limit`). Post-genesis, HashBeasts mint
/// via breeding without a hard ceiling; parent breed-count pricing plus the
/// floor guard make additional supply progressively expensive.
///
/// **`hashbeast_collection` is the trust anchor** for "this asset is a
/// canonical HashBeast" — every mint/breed Accounts struct address-pins the
/// `hashbeast_collection` field to this pubkey, every stake/use/withdraw
/// path resolves the same constraint. Don't add a new entry point that
/// touches a HashBeast asset without binding the collection. See
/// `instructions/hashbeasts.rs` module docs for the full mint-flow guard.
#[account]
pub struct HashBeastConfig {
    pub bump: u8,

    /// Canonical Metaplex Core collection address for HashBeasts. Set once
    /// at admin init; mint paths refuse any other collection.
    pub hashbeast_collection: Pubkey,

    /// Lifetime count of HashBeasts ever minted. Burns do NOT decrement this.
    pub total_hashbeasts_minted: u64,

    /// Admin kill-switch. When false, `breed_hashbeasts` reverts.
    pub breeding_allowed: bool,

    /// Per-parent prices by current breed_count. Valid indexes are 0..=4.
    /// Pair price = mom table price + dad table price, then the floor guard is
    /// applied. Admin-updatable so governance can retune the sink.
    pub breed_parent_prices_lamports: [u64; BREED_PARENT_PRICE_COUNT],
}

impl HashBeastConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // hashbeast_collection
        8 +     // total_hashbeasts_minted
        1 +     // breeding_allowed
        (8 * BREED_PARENT_PRICE_COUNT); // breed_parent_prices_lamports
}

/// Mint-only HashBeast configuration for the genesis sale and free/admin genesis mints.
/// Non-mint gameplay/staking/breeding instructions should not require this account.
#[account]
pub struct HashBeastMintConfig {
    pub bump: u8,

    /// Whether primary genesis minting is currently active.
    pub is_active: bool,

    /// Base price for the genesis bonding curve (in lamports).
    pub base_price: u64,

    /// Curve steepness parameter for genesis mint pricing.
    pub curve_a: u64,

    /// Total number of genesis mints allowed across all factions.
    pub genesis_mint_limit: u64,

    /// Number of genesis mints completed so far.
    pub genesis_mints: u64,

    /// Max genesis mints allowed per faction/country.
    pub max_genesis_mints_per_faction: u16,

    /// Genesis mints completed per faction/country.
    pub genesis_mints_by_faction: [u16; NUM_FACTIONS],

    /// Available ticket tier configs users can choose when minting.
    pub ticket_tiers: Vec<TicketTier>,
}

impl HashBeastMintConfig {
    pub const MAX_TICKET_TIERS: usize = 3;
    pub const DEFAULT_MAX_GENESIS_MINTS_PER_FACTION: u16 = 1_000;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        8 +     // base_price
        8 +     // curve_a
        8 +     // genesis_mint_limit
        8 +     // genesis_mints
        2 +     // max_genesis_mints_per_faction
        (NUM_FACTIONS * 2) + // genesis_mints_by_faction
        4 + (Self::MAX_TICKET_TIERS * TicketTier::LEN); // ticket_tiers
}

/// Per-user whitelist allowance for free HashBeast mints.
/// The whitelisted user still pays transaction/account rent, but not the mint fee.
#[account]
pub struct HashBeastFreeMintAllowance {
    pub user: Pubkey,
    pub remaining_free_mints: u8,
    pub bump: u8,
}

impl HashBeastFreeMintAllowance {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // user
        1 +     // remaining_free_mints
        1; // bump
}

// ========================================================================================
// ============================= TAX CONFIG ACCOUNT ==============================
// ========================================================================================

/// Tax Configuration PDA (Seed: `[b"tax-config"]`)
/// Manages degenBTC transfer-tax distribution: faction treasury + burn + the
/// residual flowing back to the mining vault. NFT market-making is funded
/// from SOL (see `SolFeeConfig::nft_market_making_pct`), not from this tax.
#[account]
pub struct TaxConfig {
    pub bump: u8,

    /// Percentage of withheld tax that goes to faction treasury
    pub treasury_pct: u8,
    /// Percentage of withheld tax that gets burned (remainder goes back to vault)
    pub burn_pct: u8,

    /// Total amount of degenBTC burnt so far (cumulative)
    pub total_burnt: u64,
    /// Treasury tax accrued while no active faction war state existed yet.
    /// This amount gets attached to the next faction war when that state is initialized.
    pub unassigned_war_treasury_amount: u64,

    /// PDA addresses for tax system
    pub withdraw_withheld_authority: Pubkey,
    pub faction_treasury_vault: Pubkey,
}

impl TaxConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // treasury_pct
        1 +     // burn_pct
        8 +     // total_burnt
        8 +     // unassigned_war_treasury_amount
        32 +    // withdraw_withheld_authority
        32; // faction_treasury_vault

    /// 80% of treasury is split by rank weight (higher rank = more reward).
    pub const RANK_WEIGHTED_BPS: u64 = 8000;
    /// 20% of treasury goes to one random faction (keeps underdogs engaged).
    pub const LUCKY_DRAW_BPS: u64 = 2000;
}

// ========================================================================================
// ========================== 1. GLOBAL & ORACLE ACCOUNTS =================================
// ========================================================================================

/// Global game state PDA (Seed: `[b"global-game-state"]`)
/// Tracks global game statistics and the currently active round.
/// Each individual round has its own GameSession PDA.
#[account]
pub struct GlobalGameSate {
    pub bump: u8,

    /// Whether the game is currently active
    pub is_active: bool,
    pub can_begin_round: bool,

    /// The currently active round ID (e.g., 48636).
    pub current_round_id: u64,
    /// Round duration in seconds (configurable)
    pub round_duration_seconds: i64,

    // --- Data from the *previous* round (for claiming) ---
    /// The last completed round ID
    pub last_round_id: u64,

    /// Global jackpot pot that accumulates across all rounds and factions.
    /// When the jackpot hits, this pot is distributed to any-direction bettors on the selected faction.
    pub jackpot_pot: u64,
}

impl GlobalGameSate {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        1 +     // is_active
        1 +     // can_begin_round
        8 +     // current_round_id
        8 +     // round_duration_seconds
        8 +     // last_round_id
        8; // jackpot_pot
}

#[account]
pub struct HodlPool {
    pub hodl_tax_index: u128,
    pub total_dbtc_claimable: u64,
}

impl HodlPool {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        16 +    // hodl_tax_index (u128)
        8; // total_dbtc_claimable (u64)
}

/// Faction State PDA (Seed: `[b"faction", faction_name.as_bytes()]`)
/// Tracks cumulative statistics and reward indexes for a specific faction.
/// One account per faction (up to MAX_FACTIONS factions).
/// Used for calculating staker rewards based on faction performance.
#[account]
pub struct FactionState {
    /// The faction ID (matching index in supported_factions)
    pub faction_id: u8,

    /// Total passive hashpower from stakers in this faction (cumulative)
    pub total_degenbtc_hashpower: u64,
    pub degenbtc_staked: u64,
    pub degenbtc_degenbtc_reward_index: u128,
    pub degenbtc_sol_reward_index: u128,

    pub total_lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_index: u128,
    pub lp_degenbtc_reward_index: u128,

    pub hashbeasts_staked: u64,
    /// Total hashbeasts currently being used in gameplay
    pub hashbeasts_playing: u64,
}

impl FactionState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // faction_id
        8 +     // total_degenbtc_hashpower (u64)
        8 +     // degenbtc_staked (u64)
        16 +    // degenbtc_degenbtc_reward_index (u128)
        16 +    // degenbtc_sol_reward_index (u128)
        8 +     // total_lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_index (u128)
        16 +    // lp_degenbtc_reward_index (u128)
        8 +     // hashbeasts_staked (u64)
        8; // hashbeasts_playing (u64)
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
    /// Total weighted points bets (for degenBTC distribution)
    pub total_wgtd_points_bets: u64,
    /// Total stakers fee paid in this round
    pub stakers_fee: u64,
    /// SOL added to the war's cycle-SOL pot this round (sum of
    /// `cycle_sol_split_per_bet × num_bets` across every bet in this round).
    /// Folded into `war_state.sol_reward_pool` at settle_round via
    /// `track_war_round_completion`. Lets us track the cycle SOL pool without
    /// loading war_state on the bet hot path.
    pub cycle_sol_pool: u64,

    /// Number of users who bet on each faction.
    pub user_faction_indexes: [u64; NUM_FACTIONS],
    /// Net SOL bet placed on each faction.
    pub sol_bets_by_faction: [u64; NUM_FACTIONS],
    /// Points bet placed on each faction-direction pair.
    pub points_bets_by_faction_direction: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
    /// Weighted points bet placed on each faction-direction pair.
    pub wgtd_points_bets_by_faction_direction: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],

    /// The winning faction ID for this round.
    pub winning_faction_id: u8,
    /// The winning direction for the winning faction (0=Down, 1=Neutral, 2=Up).
    pub winning_direction: u8,

    // --- degenBTC reward pools for this round ---
    /// degenBTC allocated for exact winning faction+direction bettors in this round.
    pub dbtc_winner_pool: u64,
    /// degenBTC allocated per losing direction on the winning faction.
    /// The winning direction index remains zero in this array.
    pub dbtc_same_faction_direction_pools: [u64; PredictionDirection::COUNT],
    /// degenBTC allocated for stakers in this round
    pub faction_stakers: u64,
    /// degenBTC allocated for the global jackpot in this round.
    pub jackpot_rewards: u64,

    /// SOL rewards index for this round's exact winning faction+direction.
    pub sol_rewards_index: u128,
    /// degenBTC rewards index for this round's exact winning faction+direction.
    pub dbtc_rewards_index: u128,
    // --- Jackpot data for this round ---
    /// Whether the global jackpot was hit in this round.
    pub jackpot_hit: bool,
    /// The faction ID that wins the global jackpot this round (if hit).
    pub jackpot_faction_id: u8,
    /// Global jackpot pot size when hit (if applicable).
    pub jackpot_pot_size_on_hit: u64,
    /// degenBTC rewards index for jackpot winners (all directions on jackpot faction).
    /// Set during `settle_round`; read by `claim_round_rewards`.
    pub jackpot_rewards_index: u128,

    // --- Mutation tracking per round ---
    /// Number of mutations that have occurred per faction this round.
    /// More mutations in a faction → harder for the next one (diminishing returns).
    pub mutations_per_faction: [u8; NUM_FACTIONS],
    /// Total mutations across all factions this round.
    /// Capped at active_factions / 3 to create scarcity.
    pub total_mutations_this_round: u8,

    /// Snapshot of `war_config.current_war_id` at round start.
    /// Used by the round-claim handler to detect late claims (cycle has settled
    /// after the round ended) so mutation-bonus score is dropped instead of
    /// being applied to a different cycle.
    pub war_id_when_played: u64,

    /// Snapshot of the winning country's `sol_volume_since_last_win`
    /// captured at round-end (in `track_war_round_completion`),
    /// BEFORE the config counter is reset to 0. Frozen value the round-claim
    /// mutation roll feeds into the volume_factor — late claims see the same
    /// number even though the config-side counter has long been reset.
    pub winning_faction_volume_at_round: u64,

    /// Accumulated cycle SOL split from bets placed during this round.
    /// Folded into `FactionWarState.sol_reward_pool` once per round at
    /// `settle_round` rather than touched per-bet — keeps JoinBets fast.
    pub sol_reward_pool_accumulated: u64,
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
        8 +     // cycle_sol_pool
        (NUM_FACTIONS * 8) + // user_faction_indexes
        (NUM_FACTIONS * 8) + // sol_bets_by_faction
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // points_bets_by_faction_direction
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // wgtd_points_bets_by_faction_direction
        1 +     // winning_faction_id (u8)
        1 +     // winning_direction (u8)
        8 +     // dbtc_winner_pool
        (PredictionDirection::COUNT * 8) + // dbtc_same_faction_direction_pools
        8 +     // faction_stakers
        8 +     // jackpot_rewards
        16 +    // sol_rewards_index
        16 +    // dbtc_rewards_index
        1 +     // jackpot_hit
        1 +     // jackpot_faction_id
        8 +     // jackpot_pot_size_on_hit
        16 +    // jackpot_rewards_index
        (NUM_FACTIONS * 1) + // mutations_per_faction
        1 + // total_mutations_this_round
        8 + // war_id_when_played
        8 + // winning_faction_volume_at_round
        8; // sol_reward_pool_accumulated
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
    /// Permanent faction chosen at signup. Country identity does not change after registration.
    pub origin_faction_id: u8,
    /// Referrer's origin faction at signup, or u8::MAX when there is no referrer.
    pub referrer_faction_id: u8,

    pub degenbtc_hashpower: u64,
    pub degenbtc_staked: u64,
    pub degenbtc_degenbtc_reward_debt: u128,
    pub degenbtc_sol_reward_debt: u128,

    pub lp_hashpower: u64,
    pub lp_staked: u64,
    pub lp_sol_reward_debt: u128,
    pub lp_degenbtc_reward_debt: u128,

    pub pending_sol_rewards: u64,
    pub hodl_tax_index: u128,
    /// Gameplay-earned degenBTC rewards pending HODL-tax withdrawal.
    pub pending_dbtc_rewards: u64,
    /// Passive staking degenBTC rewards pending direct claim with SOL staking rewards.
    pub pending_staking_dbtc_rewards: u64,
    pub unrefined_dbtc_rewards: u64,
    /// Number of unclaimed per-round reward accounts still outstanding.
    pub pending_round_claims: u16,
    /// Number of unclaimed per-faction-war reward accounts still outstanding.
    pub pending_war_claims: u16,

    pub degenbtc_position_indices: Vec<u8>,
    pub lp_position_indices: Vec<u8>,

    /// Staked dragon hashbeasts (max 3 hashbeasts)
    /// Stores the mint addresses of staked hashbeasts
    pub staked_hashbeasts: Vec<Pubkey>,
    /// Current hashbeast multiplier (1000 = 1x, 1500 = 1.5x, etc.)
    /// Effective passive staking HashBeast multiplier after applying the 3x passive cap.
    pub hashbeast_multiplier: u16,

    /// Free tickets: points size of each ticket type (max 5 ticket types)
    /// Example: [10000000, 100000000, ...] where 1 point = 1 SOL lamport
    /// So 10000000 = 0.01 SOL, 100000000 = 0.1 SOL
    pub free_tickets: Vec<u64>,
    /// Free tickets remaining: count of each ticket type remaining
    /// Index matches free_tickets (e.g., free_tickets_remaining[0] is count for free_tickets[0])
    pub free_tickets_remaining: Vec<u64>,

    /// HashBeast currently being used in gameplay (Pubkey::default() if none)
    pub gameplay_hashbeast: Pubkey,
    /// Active gameplay multiplier (1000 = 1x, set from gameplay hashbeast's multiplier, capped at 4.2x, reset to BASE_MULTIPLIER on withdraw)
    pub active_multiplier: u32,
    /// Cached DNA of gameplay hashbeast (for mutation calculations without loading HashBeastMetadata)
    pub gameplay_hashbeast_dna: [u8; 32],
    /// Cached XP of gameplay hashbeast (updated during gameplay, synced to HashBeastMetadata on withdraw)
    pub gameplay_hashbeast_xp: u32,
    /// FactionWar ID in which the user requested gameplay unlock.
    /// The hashbeast can only be withdrawn once the next faction_war cycle begins.
    pub gameplay_unlock_request_faction_war: u64,
    /// Cumulative gameplay score for the current faction war cycle.
    /// Lazy-reset to 0 the first time it's touched in a new cycle (see
    /// `current_war_score_cycle_id` below). Used for MVP tracking.
    pub current_war_score: u64,
    /// `war_id` that `current_war_score` belongs to.
    /// On the first bet of a new cycle (when `war_state.war_id`
    /// differs from this), the running score is reset to 0 and this is updated.
    /// This avoids needing a separate per-user reset instruction at cycle rollover.
    pub current_war_score_cycle_id: u64,
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
        1 +     // origin_faction_id
        1 +     // referrer_faction_id
        8 +     // degenbtc_hashpower (u64)
        8 +     // degenbtc_staked (u64)
        16 +    // degenbtc_degenbtc_reward_debt (u128)
        16 +    // degenbtc_sol_reward_debt (u128)
        8 +     // lp_hashpower (u64)
        8 +     // lp_staked (u64)
        16 +    // lp_sol_reward_debt (u128)
        16 +    // lp_degenbtc_reward_debt (u128)
        8 +     // pending_sol_rewards (u64)
        16 +    // hodl_tax_index (u128)
        8 +     // pending_dbtc_rewards (u64)
        8 +     // pending_staking_dbtc_rewards (u64)
        8 +     // unrefined_dbtc_rewards (u64)
        2 +     // pending_round_claims (u16)
        2 +     // pending_war_claims (u16)
        4 + (Self::MAX_POSITIONS * 1) + // degenbtc_position_indices Vec<u8>
        4 + (Self::MAX_POSITIONS * 1) + // lp_position_indices Vec<u8>
        4 + (MAX_STAKED_HASHBEASTS * 32) + // staked_hashbeasts Vec<Pubkey>
        2 +     // hashbeast_multiplier (u16)
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets Vec<u64>
        4 + (Self::MAX_TICKET_TYPES * 8) + // free_tickets_remaining Vec<u64>
        32 +    // gameplay_hashbeast
        4 +     // active_multiplier (u32)
        32 +    // gameplay_hashbeast_dna [u8; 32]
        4 +     // gameplay_hashbeast_xp (u32)
        8 +     // gameplay_unlock_request_faction_war (u64)
        8 +     // current_war_score (u64)
        8; // current_war_score_cycle_id (u64)
}

/// Individual degenBTC staking position
#[account]
pub struct StakedPosition {
    pub position_type: u8, // 0 = degenBTC, 1 = lp

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

/// Stores referral rewards that a user has earned from referrals.
/// Rewards accrue as a slice of SOL protocol fees the referee actually pays
/// (bets + NFT mints), capped at MAX_REFERRER_SOL_LIFETIME. After the cap,
/// new fees flow back to the normal recipients (no further accrual).
#[account]
pub struct ReferralRewards {
    pub owner: Pubkey,
    pub bump: u8,
    /// Permanent faction of the referral-code owner.
    pub owner_faction_id: u8,
    /// Number of users who have used this user's referral code.
    /// This is analytics/accounting only; registration is not capped by count.
    pub referrals_count: u64,

    /// Pending SOL rewards from referees' protocol fees (bets + NFT mints).
    /// Stored as extra lamports on this PDA; claimed via claim_referral_rewards.
    pub pending_sol_rewards: u64,

    /// Cumulative SOL earned across all referees. Capped at MAX_REFERRER_SOL_LIFETIME;
    /// once total_sol_earned >= cap, no further accrual occurs.
    pub total_sol_earned: u64,
}

impl ReferralRewards {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        1 +     // bump
        1 +     // owner_faction_id
        8 +     // referrals_count
        8 +     // pending_sol_rewards
        8; // total_sol_earned
}

/// Lifetime SOL cap per referrer code: 100,000 SOL (in lamports).
/// Once a referrer's `total_sol_earned` reaches this, additional fees
/// from their referees are routed to the normal SOL fee recipients
/// (sol_treasury / fee_recipient) and no further commission accrues.
pub const MAX_REFERRER_SOL_LIFETIME: u64 = 100_000 * 1_000_000_000;

// ========================================================================================
// ===============================  HASHBEAST NFT METADATA ===============================
// ========================================================================================

/// HashBeast NFT metadata (stored in degenBTC program for simplicity)
#[account]
pub struct HashBeastMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    /// Parent 1 mint (Pubkey::default() for genesis hashbeasts)
    pub mom: Pubkey,
    /// Parent 2 mint (Pubkey::default() for genesis hashbeasts)
    pub dad: Pubkey,
    /// Number of times this hashbeast has bred (max 5)
    pub breed_count: u8,
    /// Number of times this asset has been reborn/reborn (max 7)
    pub rebirth_count: u8,
    /// Unix timestamp when cooldown ends (can breed again after this)
    pub cooldown_end: i64,
    /// Creation timestamp
    pub created_at: i64,
    /// Faction ID (country) that the hashbeast belongs to (matches degenBTC faction)
    pub faction_id: u8,
    /// Multiplier for this hashbeast (1000 = 1x, same scale as BASE_MULTIPLIER)
    pub multiplier: u32,
    /// degenBTC accumulated which can be claimed by rebirthing this hashbeast
    pub accumulated_val: u64,
    /// DNA data (32 bytes for breeding/evolution)
    pub dna: [u8; 32],
    /// The Player who is incubating this hashbeast. Pubkey::default() if not incubated.
    pub incubated_player_data: Pubkey,
    /// Last power update timestamp
    pub last_update_ts: i64,
    /// Experience points, reset to 0 on evolution
    pub xp: u32,
    /// PDA bump
    pub bump: u8,
}

impl HashBeastMetadata {
    pub const MAX_BREED_COUNT: u8 = 5;

    /// Cooldown times in seconds: [0h, 24h, 72h, 120h, 336h]
    pub const COOLDOWNS: [i64; 5] = [0, 86400, 259200, 432000, 1209600];

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // mint
        32 +    // mom
        32 +    // dad
        1 +     // breed_count
        1 +     // rebirth_count
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

    pub fn reset_for_rebirth(&mut self, new_dna: [u8; 32], rebirth_count: u8, now: i64) {
        self.mom = Pubkey::default();
        self.dad = Pubkey::default();
        self.breed_count = 0;
        self.rebirth_count = rebirth_count;
        self.cooldown_end = 0;
        self.created_at = now;
        self.multiplier = BASE_MULTIPLIER;
        self.accumulated_val = 0;
        self.dna = new_dna;
        self.incubated_player_data = Pubkey::default();
        self.last_update_ts = now;
        self.xp = 0;
    }
}

// ========================================================================================
// ============================= BET TYPE ENUM ==============================
// ========================================================================================

/// Directional stance for country bets (rounds + cycle leaderboard).
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
/// Each bet selects a faction and a direction for the active faction_war.
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
// ============================= GAME ROUND ACCOUNTS ==============================
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
    /// Faction-war cycle active when this round bet was placed.
    pub war_id: u64,

    /// List of faction IDs user bet on.
    /// Index position corresponds to the same index in directions/sol_bets/points_bets.
    pub faction_ids: Vec<u8>,
    /// Direction chosen for each faction (0=Down, 1=Neutral, 2=Up).
    pub directions: Vec<u8>,

    /// SOL bets for each faction (index matches faction_ids)
    pub sol_bets: Vec<u64>,
    /// Points bets for each faction (index matches faction_ids)
    pub points_bets: Vec<u64>,
    /// Weighted points for each faction (points * multiplier / 100 for SOL, else points) - for degenBTC
    pub wgtd_points_bets: Vec<u64>,

    /// Total SOL amount bet across all factions (after protocol fee deduction)
    pub total_sol_bet: u64,
    /// Total points amount bet across all factions
    pub total_points_bet: u64,
    /// Total weighted points (for degenBTC rewards)
    pub total_wgtd_points_bet: u64,

    /// Total fees paid across all bets
    pub total_fee: u64,
    pub gameplay_hashbeast: Pubkey,

    pub bump: u8,

    // --- Claim-time mutation result ---
    /// 0 = no mutation, 1 = Evolution, 2 = Power, 3 = Trait
    pub mutation_type: u8,
}

impl UserGameBet {
    // Maximum number of faction-direction positions a user can bet on in a single round.
    pub const MAX_POSITIONS_PER_BET: usize = NUM_FACTIONS * PredictionDirection::COUNT;

    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // owner
        8 +     // round_id
        8 +     // war_id
        4 + (Self::MAX_POSITIONS_PER_BET * 1) + // faction_ids Vec<u8>
        4 + (Self::MAX_POSITIONS_PER_BET * 1) + // directions Vec<u8>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // sol_bets Vec<u64>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // points_bets Vec<u64>
        4 + (Self::MAX_POSITIONS_PER_BET * 8) + // wgtd_points_bets Vec<u64>
        8 +     // total_sol_bet
        8 +     // total_points_bet
        8 +     // total_wgtd_points_bet
        8 +     // total_fee
        32 +     // gameplay_hashbeast
        1 +     // bump
        1; // mutation_type
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
    /// SOL reserved per round.
    /// - SOL mode: total round budget, including keeper compensation plus generated bets.
    /// - Ticket mode: must be 0; a fixed keeper reserve is deposited per round.
    pub sol_per_round: u64,
    /// Number of rounds remaining (decremented after each round)
    pub rounds_remaining: u32,
    /// Last round ID where bets were placed (to prevent duplicate bets)
    pub last_bet_round_id: u64,
    pub vault_bump: u8,
    /// Remaining SOL balance reserved for this autominer (held in autominer custody PDA)
    pub sol_balance: u64,

    /// If set to true, SOL rewards can be used to reload Autominer and continue mining degenBTC
    pub can_reload: bool,

    /// Optional ticket tier index. If Some, autominer uses tickets for bet points.
    /// Ticket mode still reserves SOL upfront to compensate the keeper for each execution.
    /// Bet amount is determined by the ticket value in player_data.free_tickets[tier].
    pub use_ticket: Option<u8>,

    /// Autominer-placed bets that haven't been claimed yet. Incremented in
    /// `execute_autominer_bet` after a successful bet, decremented in
    /// `claim_autominer_rewards`. When this hits 0 in claim, that claim is the
    /// last unclaimed bet of the current funded cycle → bulk-reload trigger.
    pub pending_autominer_claims: u32,
    /// SOL rewards won during the current funded cycle, parked in
    /// `autominer_custody`, awaiting bulk-conversion to additional rounds on
    /// the final claim. Refunded to owner on `stop_autominer`.
    pub accrued_reload_sol: u64,
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
        1 + 1 + // use_ticket Option<u8> (1 byte discriminator + 1 byte value)
        4 +     // pending_autominer_claims (u32)
        8; // accrued_reload_sol (u64)
}

// ========================================================================================
// ============================= FACTION_WAR MINING ACCOUNTS ==============================
// ========================================================================================

/// Faction War configuration PDA (Seed: `[b"faction-war-config"]`)
/// Faction wars are tied to the economy cycle: one faction war per LP-burn cycle.
/// Settlement becomes possible once lp_operations_count reaches settle_at_lp_op_count.
#[account]
pub struct FactionWarConfig {
    pub bump: u8,

    /// Current faction-war ID (incrementing counter, starts at 1)
    pub current_war_id: u64,

    /// Cached PDA bump for `war_sol_vault`. Stored here so the hot
    /// JoinBets path can derive the vault address with `create_program_address`
    /// instead of paying `find_program_address` every bet.
    pub rewards_sol_vault_bump: u8,

    /// The LP operations count that triggers settlement of the current faction_war.
    /// Set to `pol_stats.lp_operations_count + 1` when the faction_war starts,
    /// meaning the faction_war settles after the next full economy cycle completes.
    pub settle_at_lp_op_count: u32,

    /// Rankings from the previous faction war's gameplay scores.
    /// Used as start_ranks when the next faction war auto-starts.
    /// Initialized to [0, 1, 2, ..., NUM_FACTIONS-1] on first setup.
    pub prev_ranks: [u8; NUM_FACTIONS],

    /// Last round whose round-completion side effects were applied to this cycle.
    pub last_processed_round_id: u64,

    /// Round id snapshotted by the LP-burn instruction when
    /// `lp_operations_count` first reaches `settle_at_lp_op_count`. Marks the
    /// final round of the current cycle — any round after this one belongs to
    /// the next war. `0` while the cycle is still open.
    ///
    /// Lifecycle:
    /// - LP burn captures `global_game_state.current_round_id` here once the
    ///   threshold crosses.
    /// - `start_round` is blocked once this is non-zero (war must be settled
    ///   before a new round can begin).
    /// - `settle_war` requires this to be non-zero AND
    ///   `last_processed_round_id == cycle_end_round_id` (boundary round
    ///   already folded into war_state).
    /// - Stays non-zero after `finalize_war_settlement` so `start_round`
    ///   remains blocked until `initialize_war_internal` creates the next
    ///   war PDAs, then resets to `0` there so the next war starts fresh.
    pub cycle_end_round_id: u64,

    /// Per-country additive SOL volume accumulated since each country's last
    /// round win. Resets to 0 for the winner inside
    /// `track_war_round_completion` AFTER snapshotting onto
    /// `GameSession.winning_faction_volume_at_round`. Persists across cycle
    /// boundaries — a country in a long drought builds up potential.
    pub sol_volume_since_last_win: [u64; NUM_FACTIONS],

    // ----- DYNAMIC MINING MULTIPLIER (degenBTC cycle rewards) -----
    /// Current multiplier for faction-war degenBTC rewards, in basis points.
    /// 10_000 = 1.0x. Applied to `total_dbtc_mined_in_rounds` at settlement.
    pub mining_multiplier_bps: u16,
    /// Basis-point increase applied when price goes up (e.g. 300 = +3%).
    pub multiplier_increase_bps: u16,
    /// Basis-point decrease applied when price goes down (e.g. 1000 = -10%).
    pub multiplier_decrease_bps: u16,
    /// Hard floor for the multiplier (min protocol cap: 1000 = 0.1x).
    pub multiplier_min_bps: u16,
    /// Hard ceiling for the multiplier (max protocol cap: 30000 = 3.0x).
    pub multiplier_max_bps: u16,
}

impl FactionWarConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // current_war_id
        1 +     // rewards_sol_vault_bump
        4 +     // settle_at_lp_op_count
        (NUM_FACTIONS * 1) + // prev_ranks
        8 +     // last_processed_round_id
        8 +     // cycle_end_round_id
        (NUM_FACTIONS * 8) + // sol_volume_since_last_win
        2 +     // mining_multiplier_bps
        2 +     // multiplier_increase_bps
        2 +     // multiplier_decrease_bps
        2 +     // multiplier_min_bps
        2; // multiplier_max_bps
}

impl FactionWarConfig {
    pub fn reset_cycle_round_tracking(&mut self) {
        self.last_processed_round_id = 0;
        // Note: sol_volume_since_last_win is intentionally NOT reset here
        // — it tracks across cycles; only resets per-country on round win.
    }
}

/// Faction War state PDA (Seed: `[b"faction-war", war_id_u64_le]`)
/// Tracks active gameplay data during a faction war cycle.
/// Kept small because it is loaded on every bet and every settle_round.
#[account]
pub struct FactionWarState {
    pub bump: u8,

    /// FactionWar ID
    pub war_id: u64,
    /// Timestamp when this faction_war was started
    pub start_timestamp: u64,

    /// Stage: 0 = active, 1 = settled (claims open)
    pub stage: u8,
    /// Snapshot of how many factions were active when this faction_war started
    pub faction_count: u8,

    /// Total degenBTC mined via raffle rounds during this faction_war.
    pub total_dbtc_mined_in_rounds: u64,
    /// Faction-war mining pool distributed to faction-war predictors.
    pub dbtc_mined_this_war: u64,

    /// Total weighted bets per faction and direction during this faction_war
    /// from all users. This powers the base "be right anywhere" cycle rewards.
    pub faction_direction_totals: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],

    /// Number of raffle rounds won by each faction during this faction war.
    /// Used as a tiebreak after story score.
    pub round_wins: [u16; NUM_FACTIONS],

    /// Total real SOL volume across all factions/directions for this war.
    /// Folded once per round from `game_session.total_sol_bets`. Used as the
    /// `total_sol` denominator in the claim-time mutation chance roll.
    pub total_cycle_sol: u64,

    /// Accumulated gameplay scores per faction during this faction_war.
    /// Drives ranking at settlement (round wins is the tiebreak, then faction_id).
    pub gameplay_scores: [u64; NUM_FACTIONS],

    /// Running MVP candidate per faction (user with highest cumulative gameplay score).
    pub mvp_user: [Pubkey; NUM_FACTIONS],
    /// Running MVP score per faction.
    pub mvp_score: [u64; NUM_FACTIONS],

    /// Total mutation-bonus score per faction across all users this cycle.
    /// Incremented in `apply_mutation_bonus_score` alongside per-user totals.
    /// Denominator for HB bonus claim share — `hb_share[user] =
    /// hb_pool[home] * user_mutation_score / faction_mutation_score[home]`.
    /// HB lane is now purely gameplay-driven: you must have rolled at least
    /// one successful mutation this cycle to earn HB-bonus.
    pub faction_mutation_score: [u64; NUM_FACTIONS],

    /// Accumulated SOL (from `cycle_sol_split`) reserved for this faction-war
    /// cycle's SOL jackpot. Distributed to claimants at settlement.
    pub sol_reward_pool: u64,

    /// Exact amount of faction treasury tax attributed to this faction war.
    /// Accumulated during tax distribution while the war is active, or
    /// seeded from TaxConfig.unassigned_war_treasury_amount when the
    /// war state is first initialized.
    pub treasury_reward_base_amount: u64,
}

impl FactionWarState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // war_id
        8 +     // start_timestamp
        1 +     // stage
        1 +     // faction_count
        8 +     // total_dbtc_mined_in_rounds
        8 +     // dbtc_mined_this_war
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // faction_direction_totals
        (NUM_FACTIONS * 2) + // round_wins
        8 +     // total_cycle_sol
        (NUM_FACTIONS * 8) + // gameplay_scores
        (NUM_FACTIONS * 32) + // mvp_user
        (NUM_FACTIONS * 8) + // mvp_score
        (NUM_FACTIONS * 8) + // faction_mutation_score
        8 +     // sol_reward_pool
        8; // treasury_reward_base_amount

    pub fn blank() -> Self {
        Self {
            bump: 0,
            war_id: 0,
            start_timestamp: 0,
            stage: 0,
            faction_count: 0,
            total_dbtc_mined_in_rounds: 0,
            dbtc_mined_this_war: 0,
            faction_direction_totals: [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS],
            round_wins: [0u16; NUM_FACTIONS],
            total_cycle_sol: 0,
            gameplay_scores: [0u64; NUM_FACTIONS],
            mvp_user: [Pubkey::default(); NUM_FACTIONS],
            mvp_score: [0u64; NUM_FACTIONS],
            faction_mutation_score: [0u64; NUM_FACTIONS],
            sol_reward_pool: 0,
            treasury_reward_base_amount: 0,
        }
    }

    /// Deserialize from `buf` directly into `*target` field-by-field.
    ///
    /// Avoids a large stack temporary that `<Self as AnchorDeserialize>::deserialize`
    /// would create. Each individual field deserialization uses at most ~480 bytes
    /// of stack (the `[Pubkey; NUM_FACTIONS]` field), keeping this function under
    /// BPF's 4096-byte stack budget.
    ///
    /// Field order MUST match the struct definition exactly.
    #[inline(never)]
    pub fn deserialize_into(target: &mut Self, buf: &mut &[u8]) -> Result<()> {
        target.bump = AnchorDeserialize::deserialize(buf)?;
        target.war_id = AnchorDeserialize::deserialize(buf)?;
        target.start_timestamp = AnchorDeserialize::deserialize(buf)?;
        target.stage = AnchorDeserialize::deserialize(buf)?;
        target.faction_count = AnchorDeserialize::deserialize(buf)?;
        target.total_dbtc_mined_in_rounds = AnchorDeserialize::deserialize(buf)?;
        target.dbtc_mined_this_war = AnchorDeserialize::deserialize(buf)?;
        target.faction_direction_totals = AnchorDeserialize::deserialize(buf)?;
        target.round_wins = AnchorDeserialize::deserialize(buf)?;
        target.total_cycle_sol = AnchorDeserialize::deserialize(buf)?;
        target.gameplay_scores = AnchorDeserialize::deserialize(buf)?;
        target.mvp_user = AnchorDeserialize::deserialize(buf)?;
        target.mvp_score = AnchorDeserialize::deserialize(buf)?;
        target.faction_mutation_score = AnchorDeserialize::deserialize(buf)?;
        target.sol_reward_pool = AnchorDeserialize::deserialize(buf)?;
        target.treasury_reward_base_amount = AnchorDeserialize::deserialize(buf)?;
        Ok(())
    }
}

/// Faction War settlement PDA (Seed: `[b"faction-war-settlement", war_id_u64_le]`)
/// Holds all settlement-only data computed when a faction war ends.
/// Loaded by settle_war and claim_war_rewards — NOT by join_bets or settle_round.
#[account]
pub struct FactionWarSettlement {
    pub bump: u8,

    /// FactionWar ID (must match the corresponding FactionWarState)
    pub war_id: u64,

    /// Final ranks derived from the gameplay-score array at settlement.
    pub final_ranks: [u8; NUM_FACTIONS],
    /// Rank deltas at settlement (positive = rank improved, negative = rank worsened).
    pub rank_deltas: [i8; NUM_FACTIONS],
    /// Resolved direction per faction (0=Down, 1=Neutral, 2=Up).
    pub resolved_directions: [u8; NUM_FACTIONS],

    /// Bonus amount reserved for each faction's MVP at settlement.
    pub mvp_bonus: [u64; NUM_FACTIONS],

    /// Pre-computed base reward pool per faction (rank-weighted across factions,
    /// then shared by anyone who picked that country's resolved direction correctly).
    pub base_reward_pools: [u64; NUM_FACTIONS],
    /// Reward pool per faction reserved for gameplay HashBeasts backing their home country during the faction_war.
    pub hashbeast_reward_pools: [u64; NUM_FACTIONS],

    /// SOL lane allocations (mirror of the dBTC lanes — same bps). Per-user
    /// SOL payout at claim time scales each lane's pool by the user's dBTC
    /// share of that lane:
    ///   user_sol_<lane> = sol_<lane>_pool * user_dbtc_<lane> / total_dbtc_<lane>
    ///
    /// Distribution is **absolute rank-weighted**: each active faction's slice
    /// of every lane is determined by its rank weight relative to the sum of
    /// rank weights across all active factions. Non-eligible factions' slices
    /// are NOT redistributed to other factions; they stay unallocated.
    ///
    /// `sol_base_pool + sol_hb_pool + sol_mvp_pool + undistributed_sol ==
    /// FactionWarState.sol_reward_pool` at settle time.
    pub sol_base_pool: u64,
    pub sol_hb_pool: u64,
    pub sol_mvp_pool: u64,

    /// SOL that no eligible claimant can claim (no faction met the lane's
    /// eligibility rule, or the rank-weight slot belonged to a faction with
    /// no eligibles). Transferred to `sol_treasury` at settle so it doesn't
    /// rot in the faction-war SOL vault.
    pub undistributed_sol: u64,

    /// Bitmap of factions that have already claimed treasury rewards for this
    /// faction war. Bit N = 1 means faction N has claimed.
    pub treasury_claimed_bitmap: u16,
}

impl FactionWarSettlement {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        8 +     // war_id
        (NUM_FACTIONS * 1) + // final_ranks
        (NUM_FACTIONS * 1) + // rank_deltas
        (NUM_FACTIONS * 1) + // resolved_directions
        (NUM_FACTIONS * 8) + // mvp_bonus
        (NUM_FACTIONS * 8) + // base_reward_pools
        (NUM_FACTIONS * 8) + // hashbeast_reward_pools
        8 +     // sol_base_pool
        8 +     // sol_hb_pool
        8 +     // sol_mvp_pool
        8 +     // undistributed_sol
        2; // treasury_claimed_bitmap

    pub fn blank() -> Self {
        Self {
            bump: 0,
            war_id: 0,
            final_ranks: [0u8; NUM_FACTIONS],
            rank_deltas: [0i8; NUM_FACTIONS],
            resolved_directions: [0u8; NUM_FACTIONS],
            mvp_bonus: [0u64; NUM_FACTIONS],
            base_reward_pools: [0u64; NUM_FACTIONS],
            hashbeast_reward_pools: [0u64; NUM_FACTIONS],
            sol_base_pool: 0,
            sol_hb_pool: 0,
            sol_mvp_pool: 0,
            undistributed_sol: 0,
            treasury_claimed_bitmap: 0,
        }
    }

    /// Field-order deserializer to stay under BPF stack limit.
    #[inline(never)]
    pub fn deserialize_into(target: &mut Self, buf: &mut &[u8]) -> Result<()> {
        target.bump = AnchorDeserialize::deserialize(buf)?;
        target.war_id = AnchorDeserialize::deserialize(buf)?;
        target.final_ranks = AnchorDeserialize::deserialize(buf)?;
        target.rank_deltas = AnchorDeserialize::deserialize(buf)?;
        target.resolved_directions = AnchorDeserialize::deserialize(buf)?;
        target.mvp_bonus = AnchorDeserialize::deserialize(buf)?;
        target.base_reward_pools = AnchorDeserialize::deserialize(buf)?;
        target.hashbeast_reward_pools = AnchorDeserialize::deserialize(buf)?;
        target.sol_base_pool = AnchorDeserialize::deserialize(buf)?;
        target.sol_hb_pool = AnchorDeserialize::deserialize(buf)?;
        target.sol_mvp_pool = AnchorDeserialize::deserialize(buf)?;
        target.undistributed_sol = AnchorDeserialize::deserialize(buf)?;
        target.treasury_claimed_bitmap = AnchorDeserialize::deserialize(buf)?;
        Ok(())
    }
}

/// User FactionWar Bets PDA (Seed: `[b"user-faction-war", user_pubkey, war_id_u64_le]`)
/// Tracks how much weighted stake a user bet on each faction's direction during a
/// specific faction_war. These weights power the global base cycle rewards.
#[account]
pub struct UserFactionWarBets {
    pub bump: u8,

    /// The user who placed these bets
    pub owner: Pubkey,
    /// The faction-war ID this tracks
    pub war_id: u64,
    /// Gameplay hashbeast that backed home country during the faction_war.
    /// Set on the user's first home-faction bet while an HB is deployed;
    /// validated to stay the same for subsequent home bets in the cycle.
    pub gameplay_hashbeast: Pubkey,

    /// Cumulative mutation-bonus score this user contributed to their home
    /// country during the cycle. Incremented in `apply_mutation_bonus_score`
    /// on each successful round-claim mutation roll. Used as the HB-bonus
    /// numerator at war claim (`hb_share = pool * mutation_score / faction_mutation_score`).
    pub mutation_score: u64,

    /// Weighted bet per faction and direction during this faction_war.
    pub direction_bets: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
    /// Real SOL bet per faction and direction during this faction_war. Ticket bets stay zero here.
    pub sol_direction_bets: [[u64; PredictionDirection::COUNT]; NUM_FACTIONS],
}

impl UserFactionWarBets {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // bump
        32 +    // owner
        8 +     // war_id
        32 +    // gameplay_hashbeast
        8 +     // mutation_score
        (NUM_FACTIONS * PredictionDirection::COUNT * 8) + // direction_bets
        (NUM_FACTIONS * PredictionDirection::COUNT * 8); // sol_direction_bets

    pub fn blank() -> Self {
        Self {
            bump: 0,
            owner: Pubkey::default(),
            war_id: 0,
            gameplay_hashbeast: Pubkey::default(),
            mutation_score: 0,
            direction_bets: [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS],
            sol_direction_bets: [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS],
        }
    }
}

// ========================================================================================
// ============================== REBIRTH / LOOTBOX / MARKET ==============================
// ========================================================================================

/// Status flags for `RebornEntry`. Closed accounts are terminal.
/// Only Lootbox and Listed ever persist; the previous Pending state is gone
/// in the permissionless model — every swept asset goes queue-or-list-or-burn
/// in the same tx.
#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebornStatus {
    Lootbox = 0,
    Listed = 1,
}

impl RebornStatus {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Lootbox),
            1 => Some(Self::Listed),
            _ => None,
        }
    }
}

/// Origin of an inventory entry. Reborn = recycled from a player through rebirth.
/// Swept = bought from another player's listing on the open market.
#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebornOrigin {
    Reborn = 0,
    Swept = 1,
}

/// Singleton inventory pool PDA. Seeds: `[b"inventory-pool"]`.
///
/// **Dual role:** the PDA at this address simultaneously serves as
///   1. the typed `Account<InventoryPool>` (this struct — holds counters
///      and cached marketplace identifiers), and
///   2. the on-chain custody account: every HashBeast asset the protocol
///      acquires via `sweep_floor_lowest` or holds for the lootbox queue
///      has its mpl-core `owner` field set to this PDA.
///
/// Most marketplace ix in `marketplace_cpi.rs` therefore pull this PDA
/// twice — once as `inventory_pool` (typed view, for counter mutation) and
/// once as `inventory_pda` (raw view, for asset transfer signer). Same
/// pubkey, same bump. The PDA signs all asset moves out of inventory
/// (transfer to user on lootbox claim, list/cancel CPI to the marketplace,
/// burn CPI to mpl-core) using `[INVENTORY_POOL_SEED, bump]`.
///
/// Sale proceeds: when the marketplace fills one of our inventory listings,
/// the SOL lands on this PDA as raw lamports above the rent floor.
/// `handle_inventory_proceeds` routes that surplus 50/50 to
/// `inventory_sweep_vault` and `sol_treasury`.
///
/// `marketplace_program` / `marketplace_config` are cached at init to avoid
/// passing them as args; every CPI wrapper validates the caller-supplied
/// account against the cached pubkey.
#[account]
pub struct InventoryPool {
    pub bump: u8,
    /// Cached pubkey of the standalone `degenbtc_market` program. CPI
    /// wrappers `require_keys_eq!(...)` against this.
    pub marketplace_program: Pubkey,
    /// Cached marketplace `MarketplaceConfig` PDA inside that program.
    /// Also `require_keys_eq!`'d in every wrapper.
    pub marketplace_config: Pubkey,

    /// Live count of NFTs in inventory custody (status: Lootbox or Listed).
    /// Bumped on intake (sweep buy success), decremented on outflow
    /// (`claim_lootbox_nft`, `inventory_finalize_sale`, burn paths in
    /// `expire_program_listing` / `sweep_floor_lowest`). Capped at
    /// `MAX_INVENTORY`. Per-status counts are NOT tracked here — indexers
    /// reconstruct them from `LootboxQueuePush` / `InventoryAssetRelisted`
    /// / `InventoryAssetBurned` / `InventorySaleFinalized` events.
    pub total_count: u32,
}

impl InventoryPool {
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 1   // bump
        + 32  // marketplace_program
        + 32  // marketplace_config
        + 4; // total_count
}

/// One per HashBeast currently held by `inventory_pda`.
/// Seeds: `[b"reborn-entry", asset]`.
///
/// **Lifecycle:**
///
/// ```text
///   ┌───────────────────────────────────────────────────────────────────┐
///   │ INTAKE (creates RebornEntry, +1 InventoryPool.total_count)        │
///   │   sweep_floor_lowest → status = Lootbox  OR  Listed  OR  (burn,   │
///   │                        no entry)                                   │
///   │   rebirth_hashbeast   → status = Lootbox (no relist path)         │
///   └───────────────────────────────────────────────────────────────────┘
///                                  │
///                                  ▼
///   ┌───────────────────────────────────────────────────────────────────┐
///   │ ACTIVE                                                             │
///   │   Lootbox: sits in `LootboxQueue[faction_id]`; awaits loser-roll. │
///   │   Listed:  live program-owned listing on marketplace.             │
///   └───────────────────────────────────────────────────────────────────┘
///                                  │
///                ┌─────────────────┼──────────────────┐
///                ▼                 ▼                  ▼
///   ┌────────────────────┐ ┌────────────────┐ ┌──────────────────────┐
///   │ claim_lootbox_nft  │ │ inventory_     │ │ expire_program_      │
///   │ (Lootbox → user)   │ │ finalize_sale  │ │ listing              │
///   │ closes RebornEntry │ │ (Listed sold)  │ │ (Listed unsold @ 7d) │
///   │ -1 total_count     │ │ closes entry   │ │ cancel + cascade:    │
///   └────────────────────┘ │ -1 total_count │ │  - relist (++strike) │
///                          └────────────────┘ │  - lootbox push      │
///                                             │  - burn @ MAX_EXPIRES│
///                                             └──────────────────────┘
/// ```
///
/// **`original_buy_price`** is the immutable anchor for relist markup math.
/// Across multiple expire/relist cycles, each new list price is computed as
/// `apply_markup(original_buy_price, markup_bps)` where `markup_bps` depends
/// on the floor trend and `expire_count`. This keeps the protocol's effective
/// resale "cost basis" stable even as the asset is repriced over time.
///
/// **Quality score** is fixed at intake from
/// `compute_quality_score(multiplier, xp, breed_count)`. Indexers use it for
/// "rare drop" UX.
#[account]
pub struct RebornEntry {
    pub bump: u8,
    pub asset: Pubkey,
    pub faction_id: u8,
    /// 0..=10_000. Snapshot at intake; never updated.
    pub quality_score: u16,
    pub reborn_at: i64,
    /// `RebornStatus` enum value (Lootbox | Listed).
    pub status: u8,
    /// Current live listing price (lamports); 0 if status != Listed.
    pub listing_price: u64,
    /// `RebornOrigin` enum value (Reborn | Swept).
    pub origin: u8,
    /// Immutable cost basis: the price the protocol paid for the asset
    /// (sweep buy amount, or 0 for rebirth-origin entries). Used as the
    /// base of the relist markup formula across expire cycles, so the
    /// protocol's effective floor for resale doesn't drift downward as
    /// strikes accumulate.
    pub original_buy_price: u64,
    /// Number of times `expire_program_listing` has fired for this entry.
    /// Each strike subtracts `RELIST_EXPIRE_PENALTY_BPS` from the markup
    /// formula. Forced burn at `MAX_EXPIRES`.
    pub expire_count: u8,
}

impl RebornEntry {
    pub const LEN: usize = DISCRIMINATOR_SIZE
        + 1   // bump
        + 32  // asset
        + 1   // faction_id
        + 2   // quality_score
        + 8   // reborn_at
        + 1   // status
        + 8   // listing_price
        + 1   // origin
        + 8   // original_buy_price
        + 1; // expire_count
}

// ========================================================================================
// =========================== FLOOR QUEUE / SALE HISTORY / FLOOR HISTORY ==================
// ========================================================================================
//
// These three accounts form the on-chain price oracle + buy-target index
// for the permissionless market maker. They are all singletons (single PDA
// each, no per-user variants) and all initialized once in
// `init_inventory_pool` (admin path).
//
//   FloorQueue   — currently registered cheapest user listings, sorted
//                  ascending by price. The `sweep_floor_lowest` ix buys
//                  the head. Spam-resistant: capped at FLOOR_QUEUE_SIZE,
//                  cheaper-than-worst eviction, dedup on asset.
//
//   SaleHistory  — ringbuffer of recent qualifying user-to-user sales
//                  (≥5min listing age, neither party is `inventory_pda`).
//                  Median of the qualifying window seeds the floor anchor.
//
//   FloorHistory — 7-day rolling ringbuffer of (timestamp, anchor_price)
//                  snapshots. `current_anchor()` is the head; `compute_
//                  trend_bps()` is the (clamped) head-vs-oldest delta.
//                  Read by `breed_hashbeasts` for the 1.5× floor min,
//                  and by `sweep_floor_lowest` for the 1.05× price ceiling.
//
// See `instructions/marketplace_cpi.rs` for the manipulation-resistance
// analysis (5min listing-age qualifier × 17-sale minimum × queue/prior-anchor
// caps × 24h interval = brute anchor manipulation is loss-leading).

/// One entry in the on-chain sorted-floor queue. Tracks a user-listed asset
/// (program-owned listings are explicitly excluded — sweep buying the
/// protocol's own listings would be circular). Stale entries (listing was
/// canceled directly via the marketplace, bypassing our `cancel_user_listing`
/// wrapper that would have atomic-deregistered) are popped one at a time
/// by `sweep_floor_lowest`; the keeper bounty for that purge is the
/// `STALE_PURGE_KEEPER_REWARD_LAMPORTS` constant, set deliberately low to
/// defuse list→raw-cancel→purge spam attacks. See that constant's docs.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloorEntry {
    pub listing: Pubkey,
    pub asset: Pubkey,
    pub seller: Pubkey,
    pub price: u64,
    pub registered_at: i64,
}

impl FloorEntry {
    pub const SIZE: usize = 32 + 32 + 32 + 8 + 8;
}

/// Singleton sorted-ascending queue of the cheapest user listings.
/// Invariants: `entries[..entries_count]` is sorted ascending by `price`.
/// Seeds: `[b"floor-queue"]`.
#[account]
pub struct FloorQueue {
    pub bump: u8,
    pub entries_count: u8,
    pub entries: [FloorEntry; FLOOR_QUEUE_SIZE],
}

impl FloorQueue {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 1 + 1 + (FloorEntry::SIZE * FLOOR_QUEUE_SIZE);
}

/// One entry in the user-to-user sale history ringbuffer. Recorded by
/// `buy_user_listing` when both buyer and seller are non-protocol and the
/// listing has been live for at least `SALE_QUALIFY_MIN_LISTING_AGE_SECS`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaleEntry {
    pub asset: Pubkey,
    pub price: u64,
    pub listed_at: i64,
    pub sold_at: i64,
    pub buyer: Pubkey,
    pub seller: Pubkey,
}

impl SaleEntry {
    pub const SIZE: usize = 32 + 8 + 8 + 8 + 32 + 32;
}

/// Singleton ringbuffer of qualifying user-to-user sales — feeds the floor
/// snapshot's anchor when there's enough volume.
/// Seeds: `[b"sale-history"]`.
#[account]
pub struct SaleHistory {
    /// Index of the next slot to write (head moves forward, wraps around).
    pub head: u8,
    pub entries: [SaleEntry; SALE_HISTORY_SIZE],
}

impl SaleHistory {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 1 + (SaleEntry::SIZE * SALE_HISTORY_SIZE);
}

/// One entry in the 7-day rolling floor snapshot ringbuffer.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloorSnapshot {
    pub timestamp: i64,
    pub anchor_price: u64,
}

impl FloorSnapshot {
    pub const SIZE: usize = 8 + 8;
}

/// Singleton 7-entry rolling buffer of daily floor anchors.
/// `compute_trend_bps` reads the head vs. the (head+1) wrap-around to get a
/// 7-day delta in basis points.
/// Seeds: `[b"floor-history"]`.
#[account]
pub struct FloorHistory {
    pub bump: u8,
    pub head: u8,
    pub last_snapshot_at: i64,
    pub snapshots: [FloorSnapshot; FLOOR_HISTORY_SIZE],
}

impl FloorHistory {
    pub const LEN: usize =
        DISCRIMINATOR_SIZE + 1 + 1 + 8 + (FloorSnapshot::SIZE * FLOOR_HISTORY_SIZE);

    /// 7-day floor delta in basis points relative to the oldest valid snapshot.
    /// Positive = bull, negative = bear, 0 if insufficient history.
    pub fn compute_trend_bps(&self) -> i32 {
        // Find oldest non-zero snapshot; ringbuffer slot just past head.
        let mut oldest_idx = ((self.head as usize) + 1) % FLOOR_HISTORY_SIZE;
        let mut found_oldest = false;
        for _ in 0..FLOOR_HISTORY_SIZE {
            if self.snapshots[oldest_idx].anchor_price > 0 {
                found_oldest = true;
                break;
            }
            oldest_idx = (oldest_idx + 1) % FLOOR_HISTORY_SIZE;
        }
        let head_idx = self.head as usize % FLOOR_HISTORY_SIZE;
        if !found_oldest || oldest_idx == head_idx {
            return 0;
        }
        let oldest = self.snapshots[oldest_idx].anchor_price as i128;
        let newest = self.snapshots[head_idx].anchor_price as i128;
        if oldest == 0 {
            return 0;
        }
        let bps = ((newest - oldest) * 10_000) / oldest;
        bps.clamp(-10_000, 10_000) as i32
    }

    /// Returns the most recent anchor price, or 0 if no snapshots.
    pub fn current_anchor(&self) -> u64 {
        self.snapshots[self.head as usize % FLOOR_HISTORY_SIZE].anchor_price
    }
}

/// Compute the relist markup in basis points (signed). May be negative when
/// the asset has been struck or trend is bearish — caller decides whether
/// `apply_markup` produces a price below `buy_price`.
pub fn compute_relist_markup_bps(trend_bps: i32, expire_count: u8) -> i32 {
    let trend_mod = (trend_bps / RELIST_TREND_DIVIDER)
        .clamp(RELIST_TREND_MOD_FLOOR_BPS, RELIST_TREND_MOD_CEILING_BPS);
    let expire_penalty = (expire_count as i32) * RELIST_EXPIRE_PENALTY_BPS;
    (RELIST_BASE_MARKUP_BPS + trend_mod - expire_penalty)
        .clamp(RELIST_MIN_MARKUP_BPS, RELIST_MAX_MARKUP_BPS)
}

/// Apply a signed markup to a base price. Negative markup reduces.
pub fn apply_markup(base_price: u64, markup_bps: i32) -> u64 {
    if markup_bps >= 0 {
        let bps = markup_bps as u64;
        base_price.saturating_add(((base_price as u128 * bps as u128) / 10_000) as u64)
    } else {
        let cut_bps = (-markup_bps) as u64;
        base_price.saturating_sub(((base_price as u128 * cut_bps as u128) / 10_000) as u64)
    }
}

/// Compute a deterministic 0..=10_000 quality score for a HashBeast being reborn
/// or swept into inventory. Used by the off-chain disposition cranker for
/// listing-price modeling and by the lootbox cranker for candidate weighting.
///
/// Components:
/// - multiplier above base, scaled to 6_000
/// - xp, scaled to 3_000 (saturates at MAX_XP_FOR_QUALITY)
/// - breed_count, up to 1_000
pub fn compute_quality_score(multiplier: u32, xp: u32, breed_count: u8) -> u16 {
    let base = BASE_MULTIPLIER as u64;
    let mult_above_base = (multiplier as u64).saturating_sub(base);
    let mult_cap = base.saturating_mul(4); // up to 4x above base ≈ 5x total
    let mult_component = (mult_above_base.min(mult_cap)).saturating_mul(6_000) / mult_cap.max(1);

    let xp_component = ((xp as u64).min(MAX_XP_FOR_QUALITY as u64)).saturating_mul(3_000)
        / (MAX_XP_FOR_QUALITY as u64).max(1);

    let remaining_breeds = (5u8).saturating_sub(breed_count);
    let breed_component = (remaining_breeds.min(5) as u64) * 200;

    let total = mult_component + xp_component + breed_component;
    total.min(10_000) as u16
}

/// Loser-roll drop chance in basis points, keyed by the country queue's
/// current `filled_count`. The table stays intentionally conservative:
/// full depth is 1.50%, and near-empty queues are low enough to keep the last
/// few NFTs visible instead of creating an instant drain race.
/// `filled_count == 0` returns 0 — eligibility upstream prevents that case
/// from reaching here, but defensive zero is correct.
pub fn compute_loser_drop_chance_bps(filled_count: u8) -> u16 {
    let idx = filled_count as usize;
    if idx >= CHANCE_BPS_BY_QUEUE_DEPTH.len() {
        // Out-of-range (shouldn't happen if queue invariants hold) — clamp
        // to the highest configured chance.
        return CHANCE_BPS_BY_QUEUE_DEPTH[CHANCE_BPS_BY_QUEUE_DEPTH.len() - 1];
    }
    CHANCE_BPS_BY_QUEUE_DEPTH[idx]
}

#[cfg(test)]
mod lootbox_probability_tests {
    use super::*;

    #[test]
    fn loser_drop_chance_matches_configured_depth_schedule() {
        let expected: [u16; LOOTBOX_QUEUE_SIZE + 1] = [0, 3, 8, 15, 25, 40, 58, 78, 100, 125, 150];

        for (depth, expected_bps) in expected.iter().copied().enumerate() {
            assert_eq!(
                compute_loser_drop_chance_bps(depth as u8),
                expected_bps,
                "wrong loser lootbox chance at depth {depth}"
            );
        }
    }

    #[test]
    fn loser_drop_chance_clamps_above_queue_capacity() {
        assert_eq!(compute_loser_drop_chance_bps(u8::MAX), 150);
    }
}

// ========================================================================================
// ============================== LOOTBOX QUEUES + RESERVATIONS ===========================
// ========================================================================================

/// Per-country lootbox queue. One PDA per faction. Rebirth and sweep-buy
/// flows push assets into `slots[..filled_count]` (always packed). Loser-roll
/// pops a random index out and shifts left.
///
/// Seeds: `[b"lootbox-queue", &[faction_id]]`.
#[account]
pub struct LootboxQueue {
    pub bump: u8,
    pub faction_id: u8,
    /// Packed asset addresses. `slots[..filled_count]` is the live window.
    pub slots: [Pubkey; LOOTBOX_QUEUE_SIZE],
    pub filled_count: u8,
}

impl LootboxQueue {
    /// 8 (disc) + 1 + 1 + LOOTBOX_QUEUE_SIZE*32 + 1.
    pub const LEN: usize = DISCRIMINATOR_SIZE + 1 + 1 + (LOOTBOX_QUEUE_SIZE * 32) + 1;
}

/// Per-user winning-loser reservation. Created lazily inside the claim-rewards
/// ix only when a loser-roll wins; closed by `claim_lootbox_nft` after a user
/// or cranker delivers the NFT to the recorded winner.
///
/// `asset == Pubkey::default()` is treated as "no active reservation" by
/// the eligibility check in claim-rewards.
///
/// Seeds: `[b"lootbox-claim", user.key().as_ref()]`.
#[account]
pub struct LootboxClaim {
    pub bump: u8,
    pub user: Pubkey,
    pub asset: Pubkey,
    pub faction_id: u8,
}

impl LootboxClaim {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 1 + 32 + 32 + 1;
}
