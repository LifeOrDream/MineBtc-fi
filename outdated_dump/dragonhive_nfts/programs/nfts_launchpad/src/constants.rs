/// Program-wide constants for DragonHive NFT Launchpad

// ========================================================================================
// ================================= PDA SEEDS =========================================== 
// ========================================================================================

/// Global configuration PDA seed
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";

/// MoonDoge collection PDA seed
pub const MOONDOGE_COLLECTION_SEED: &[u8] = b"moondoge-collection";

/// Dragon Egg collection PDA seed
pub const DRAGON_EGG_COLLECTION_SEED: &[u8] = b"dragon-egg-collection";

/// MoonDoge metadata PDA seed
pub const MOONDOGE_METADATA_SEED: &[u8] = b"moondoge-metadata";

/// Dragon Egg metadata PDA seed
pub const DRAGON_EGG_METADATA_SEED: &[u8] = b"dragon-egg-metadata";

/// Incubation state PDA seed (per moonbase)
pub const INCUBATION_STATE_SEED: &[u8] = b"incubation-state";

/// Doge attachment state PDA seed (per moonbase)
pub const DOGE_ATTACHMENT_SEED: &[u8] = b"doge-attachment";

/// SOL treasury PDA seed
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// ========================================================================================
// ================================ NFT SUPPLY CONSTANTS =================================
// ========================================================================================

/// Maximum MoonDoge NFT supply (limited)
pub const MAX_MOONDOGE_SUPPLY: u64 = 10_000;

/// Initial Dragon Egg NFT supply (can be expanded)
pub const INITIAL_DRAGON_EGG_SUPPLY: u64 = 15_000;

// ========================================================================================
// ================================ PRICING CONSTANTS ====================================
// ========================================================================================

/// SOL constants
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Moonbase creation pricing tiers
pub const MOONBASE_BASIC_PRICE: u64 = 250_000_000;     // 0.25 SOL (no NFT)
pub const MOONBASE_DOGE_PRICE: u64 = 500_000_000;      // 0.5 SOL (+ MoonDoge)
pub const MOONBASE_FULL_PRICE: u64 = 1_000_000_000;    // 1.0 SOL (MoonDoge + Dragon Egg)

/// Individual NFT prices
pub const MOONDOGE_PRICE: u64 = 500_000_000;           // 0.5 SOL
pub const DRAGON_EGG_PRICE: u64 = 500_000_000;         // 0.5 SOL

// ========================================================================================
// ================================ GAMEPLAY CONSTANTS ===================================
// ========================================================================================

/// Maximum eggs that can be incubated per moonbase
pub const MAX_EGGS_PER_MOONBASE: u8 = 10;

/// Maximum doges per moonbase (1)
pub const MAX_DOGES_PER_MOONBASE: u8 = 1;

/// Power accumulation rate (per epoch/slot)
/// Formula: egg_power += (total_hashpower / total_eggs) * POWER_RATE_MULTIPLIER
pub const POWER_RATE_MULTIPLIER: u64 = 1000; // Divisor for balance

/// Money accumulation rate for doge (per mDOGE mined)
/// Formula: doge_money += mdoge_mined * MONEY_RATE_MULTIPLIER / 1e6
pub const MONEY_RATE_MULTIPLIER: u64 = 100; // 0.01% conversion rate

/// Update frequency (seconds between power/money updates)
pub const UPDATE_FREQUENCY_SECONDS: i64 = 3600; // 1 hour

// ========================================================================================
// ================================ GENETIC CONSTANTS ====================================
// ========================================================================================

/// Dragon Egg DNA size
pub const DNA_SIZE: usize = 32;

/// DNA trait categories
pub const DNA_TRAIT_GROUPS: u8 = 8;

/// Base power for new eggs
pub const BASE_EGG_POWER: u32 = 100;

/// Maximum egg power
pub const MAX_EGG_POWER: u32 = 1_000_000;

/// Base money for new doges
pub const BASE_DOGE_MONEY: u64 = 0;

/// Maximum doge money
pub const MAX_DOGE_MONEY: u64 = u64::MAX / 1000; // Leave headroom

// ========================================================================================
// ================================ SIZE CONSTANTS ======================================= 
// ========================================================================================

/// Account size constants
pub const DISCRIMINATOR_SIZE: usize = 8;

/// String size limits
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;
pub const MAX_URI_LENGTH: usize = 200;

// ========================================================================================
// ================================ METAPLEX CORE ========================================
// ========================================================================================

/// Metaplex Core program ID
pub const MPL_CORE_PROGRAM_ID: &str = "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d";
