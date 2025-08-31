/// Program-wide constants for DragonHive NFTs

// ========================================================================================
// ================================= PDA SEEDS =========================================== 
// ========================================================================================

/// Global configuration PDA seed
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global-config";

/// HONEY token vault PDA seed  
pub const HONEY_VAULT_SEED: &[u8] = b"honey-vault";

/// HONEY vault authority PDA seed
pub const HONEY_VAULT_AUTHORITY_SEED: &[u8] = b"honey-vault-authority";

/// DragonBee collection PDA seed
pub const DRAGONBEE_COLLECTION_SEED: &[u8] = b"dragonbee-collection";

/// DragonBee metadata PDA seed
pub const DRAGONBEE_METADATA_SEED: &[u8] = b"dragonbee-metadata";

/// User profile PDA seed
pub const USER_PROFILE_SEED: &[u8] = b"user-profile";

/// Queen auction manager PDA seed
pub const QUEEN_AUCTION_MANAGER_SEED: &[u8] = b"queen-auction-manager";

/// Leading DragonBee PDA seed (per family type)
pub const LEADING_DRAGONBEE_SEED: &[u8] = b"leading-dragonbee";

/// Auction participation PDA seed
pub const AUCTION_PARTICIPATION_SEED: &[u8] = b"auction-participation";

/// Auction bid pool PDA seed
pub const AUCTION_BID_POOL_SEED: &[u8] = b"auction-bid-pool";

/// Breeding cooldown PDA seed
pub const BREEDING_COOLDOWN_SEED: &[u8] = b"breeding-cooldown";

/// Kill rewards pool PDA seed
pub const KILL_REWARDS_POOL_SEED: &[u8] = b"kill-rewards-pool";

/// SOL treasury PDA seed
pub const SOL_TREASURY_SEED: &[u8] = b"sol-treasury";

// ========================================================================================
// ================================ DRAGONBEE GENETICS ==================================
// ========================================================================================

/// DragonBee type constants (4 bits - 0 to 15)
pub const BEE_TYPE_SOLAR: u8 = 1;     // Fire
pub const BEE_TYPE_AQUA: u8 = 2;      // Water
pub const BEE_TYPE_THUNDER: u8 = 3;   // Electric
pub const BEE_TYPE_TERRA: u8 = 4;     // Earth
pub const BEE_TYPE_WIND: u8 = 5;      // Air
pub const BEE_TYPE_VENOM: u8 = 6;     // Poison
pub const BEE_TYPE_FROST: u8 = 7;     // Ice
pub const BEE_TYPE_MYSTIC: u8 = 8;    // Psychic

/// Evolution stages (3 bits - 0 to 7)
pub const EVOLUTION_LARVA: u8 = 0;
pub const EVOLUTION_PUPAE: u8 = 1;
pub const EVOLUTION_WORKER: u8 = 2;
pub const EVOLUTION_SOLDIER: u8 = 3;
pub const EVOLUTION_ELITE: u8 = 4;
pub const EVOLUTION_ROYAL: u8 = 5;
pub const EVOLUTION_QUEEN: u8 = 6;
pub const EVOLUTION_DRAGON: u8 = 7;

/// Maximum values for genetic traits
pub const MAX_APPEARANCE_TRAIT: u8 = 31; // 5 bits
pub const MAX_POWER_TRAIT: u8 = 15;      // 4 bits

/// Number of trait groups
pub const APPEARANCE_TRAIT_GROUPS: u8 = 7;  // Wings, Antennae, Eyes, Stinger, Legs, Fur, Mouth
pub const POWER_TRAIT_GROUPS: u8 = 7;       // Health, Energy, StingStrike, HornCharge, Wingslash, BuzzBlast, HiveSurge

/// Traits per group
pub const APPEARANCE_TRAITS_PER_GROUP: u8 = 4; // 1 dominant + 3 recessive
pub const POWER_TRAITS_PER_GROUP: u8 = 3;      // 3 genes per power trait

// ========================================================================================
// ================================ ECONOMIC CONSTANTS ===================================
// ========================================================================================

/// NFT pricing (in lamports)
pub const DRAGONBEE_PRICE: u64 = 1_000_000_000; // 1 SOL

/// Breeding fees
pub const BASE_BREEDING_FEE: u64 = 100_000_000; // 0.1 SOL
pub const BREEDING_FEE_PERCENTAGE: u8 = 10; // 10% of breeding cost

/// Team fee percentages
pub const TEAM_FEE_PERCENTAGE: u8 = 30; // 30% to team
pub const BUYBACK_PERCENTAGE: u8 = 70;  // 70% for DRAGON buyback

/// Kill rewards pool percentage
pub const KILL_POOL_PERCENTAGE: u8 = 10; // 10% of buybacks go to kill pool

/// SOL constants
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// HONEY token constants
pub const HONEY_DECIMALS: u8 = 9;
pub const HONEY_TOTAL_SUPPLY: u64 = 100_000_000_000 * 10_u64.pow(HONEY_DECIMALS as u32); // 100B tokens

/// Maximum DragonBee supply
pub const MAX_DRAGONBEE_SUPPLY: u64 = 15_000; // Initial genesis sale

/// Cooldown periods (in seconds)
pub const BREEDING_COOLDOWN_BASE: i64 = 300;      // 5 minutes
pub const BREEDING_COOLDOWN_NORMAL: i64 = 1800;   // 30 minutes  
pub const BREEDING_COOLDOWN_SLOW: i64 = 21600;    // 6 hours
pub const BREEDING_COOLDOWN_SLOWER: i64 = 604800; // 7 days
pub const BREEDING_COOLDOWN_SLOWEST: i64 = 2592000; // 1 month

/// Power scaling for kill rewards
pub const MIN_KILL_POWER_THRESHOLD: u32 = 100;
pub const POWER_SCALING_FACTOR: u64 = 1000; // For calculating kill rewards

// ========================================================================================
// ================================ SIZE CONSTANTS ======================================= 
// ========================================================================================

/// Account size constants
pub const DISCRIMINATOR_SIZE: usize = 8;

/// String size limits
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;
pub const MAX_URI_LENGTH: usize = 200;

/// Vector size limits
pub const MAX_USER_DRAGONBEES: usize = 100; // Max DragonBees per user for storage

// ========================================================================================
// ================================= TIME CONSTANTS ======================================
// ========================================================================================

/// Seconds per day
pub const SECONDS_PER_DAY: i64 = 86400;

/// Default auction duration
pub const DEFAULT_AUCTION_DURATION: i64 = 7 * SECONDS_PER_DAY; // 7 days

// ========================================================================================
// =============================== GENETIC BIT OFFSETS ===================================
// ========================================================================================

/// Bit offsets for genetic data parsing
pub const TYPE_OFFSET: u8 = 0;                    // Bits 0-3: DragonBee type
pub const EVOLUTION_OFFSET: u8 = 4;               // Bits 4-6: Evolution stage
pub const APPEARANCE_OFFSET: u8 = 7;              // Bits 7-146: Appearance traits (140 bits)
pub const POWER_OFFSET: u8 = 147;                 // Bits 147-230: Power traits (84 bits)
pub const RESERVED_OFFSET: u8 = 231;              // Bits 231-255: Reserved (25 bits)

/// Bit sizes for genetic components
pub const TYPE_BITS: u8 = 4;
pub const EVOLUTION_BITS: u8 = 3;
pub const APPEARANCE_TRAIT_BITS: u8 = 5;
pub const POWER_TRAIT_BITS: u8 = 4;

// ========================================================================================
// =============================== QUEEN AUCTION MECHANICS ===============================
// ========================================================================================

/// Auction phase constants
pub const AUCTION_PHASE_OPEN: u8 = 1;                // Open bidding for all users
pub const AUCTION_PHASE_LIMITED: u8 = 2;             // Limited bidding for existing participants
pub const AUCTION_PHASE_COOLDOWN: u8 = 3;            // Auction ended, cooldown period

/// Auction timing (in epochs/slots)
pub const UNLIMITED_DEPOSIT_WINDOW: u64 = 3;         // 3 epochs for open bidding
pub const LIMITED_DEPOSIT_WINDOW: u64 = 1;           // 1 epoch for limited bidding
pub const AUCTION_COOLDOWN_PERIOD: u64 = 1;          // 1 epoch cooldown

/// Bid adjustment percentages
pub const MIN_BID_INCREASE_PCT: u64 = 5;             // Minimum 5% increase per auction
pub const MAX_BID_INCREASE_PCT: u64 = 50;            // Maximum 50% increase per auction
pub const MAX_BID_DECREASE_PCT: u64 = 11;            // Maximum 11% decrease during limited phase

/// Queen auction pricing
pub const BASE_QUEEN_PRICE: u64 = 50 * LAMPORTS_PER_SOL; // 50 SOL base price
pub const MIN_QUEEN_BID: u64 = 1 * LAMPORTS_PER_SOL;     // 1 SOL minimum bid

/// Queen breeding benefits
pub const QUEEN_POWER_MULTIPLIER: u32 = 150;         // 50% power bonus for queen offspring
pub const MAX_EGGS_PER_QUEEN: u64 = 100;             // Maximum eggs a queen can lay

/// Auction tax and fees
pub const MIN_AUCTION_TAX: u64 = 1;                  // 1% minimum tax
pub const MAX_AUCTION_TAX: u64 = 15;                 // 15% maximum tax
pub const DEFAULT_AUCTION_TAX: u64 = 10;             // 10% default tax

// ========================================================================================
// ================================ VALIDATION LIMITS ====================================
// ========================================================================================

/// Input validation limits
pub const MIN_BID_AMOUNT: u64 = 10_000_000; // 0.01 SOL minimum bid
pub const MAX_BREEDING_PRICE: u64 = 100_000_000_000; // 100 SOL max breeding price
pub const MAX_POWER_INCREASE_PER_UPDATE: u32 = 1000; // Max power increase per game interaction

/// Rate limiting
pub const MAX_EVOLUTIONS_PER_DAY: u8 = 3;
pub const MAX_BREEDINGS_PER_DAY: u8 = 5;
