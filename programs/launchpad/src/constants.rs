/// Program-wide constants for DragonHive NFT Launchpad

// ========================================================================================
// ================================= PDA SEEDS =========================================== 
// ========================================================================================

/// Global configuration PDA seed
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";

/// Dragon Egg collection PDA seed
pub const DRAGON_EGG_COLLECTION_SEED: &[u8] = b"dragon-egg-collection";

/// Dragon Egg metadata PDA seed
pub const DRAGON_EGG_METADATA_SEED: &[u8] = b"dragon-egg-metadata";

/// Incubation state PDA seed (per moonbase)
pub const INCUBATION_STATE_SEED: &[u8] = b"incubation-state";

/// SOL treasury PDA seed
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// ========================================================================================
// ================================ NFT SUPPLY CONSTANTS =================================
// ========================================================================================

/// Initial Dragon Egg NFT supply (can be expanded)
pub const INITIAL_DRAGON_EGG_SUPPLY: u64 = 15_000;

// ========================================================================================
// ================================ PRICING CONSTANTS ====================================
// ========================================================================================

/// SOL constants
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Moonbase creation pricing tiers
pub const MOONBASE_BASIC_PRICE: u64 = 250_000_000;     // 0.25 SOL (no NFT)
pub const MOONBASE_EGG_PRICE: u64 = 500_000_000;       // 0.5 SOL (+ Dragon Egg)
pub const MOONBASE_FULL_PRICE: u64 = 1_000_000_000;    // 1.0 SOL (Dragon Egg + extras)

/// Individual NFT prices
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

/// Update frequency (seconds between power updates)
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
