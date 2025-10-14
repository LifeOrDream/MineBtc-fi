// Bonding curve constants
pub const BONDING_CURVE_COMPLETION_THRESHOLD: u64 = 85_000_000_000; // 85 SOL in lamports
pub const INITIAL_VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000; // 30 SOL
pub const INITIAL_VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000; // 1.073B tokens (with 6 decimals)
pub const INITIAL_REAL_TOKEN_RESERVES: u64 = 793_100_000_000_000; // 793.1M tokens

// Fee constants
pub const MAX_FEE_BPS: u16 = 1000; // 10% maximum fee
pub const DEFAULT_PLATFORM_FEE_BPS: u16 = 100; // 1% default platform fee
pub const DEFAULT_MIGRATION_FEE_BPS: u16 = 100; // 1% default migration fee

// Token constants
pub const TOKEN_DECIMALS: u8 = 6;
pub const MAX_TOKEN_NAME_LENGTH: usize = 32;
pub const MAX_TOKEN_SYMBOL_LENGTH: usize = 10;
pub const MAX_TOKEN_URI_LENGTH: usize = 200;

// Math constants
pub const PRECISION: u128 = 1_000_000_000_000; // 12 decimal precision
pub const BPS_DENOMINATOR: u64 = 10_000; // Basis points denominator

// Seeds
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";
pub const BONDING_CURVE_SEED: &[u8] = b"bonding_curve";
pub const TOKEN_METADATA_SEED: &[u8] = b"token_metadata";
pub const CURVE_TOKEN_ACCOUNT_SEED: &[u8] = b"curve_token_account";

// Minimum values
pub const MIN_SOL_AMOUNT: u64 = 1_000_000; // 0.001 SOL minimum
pub const MIN_TOKEN_AMOUNT: u64 = 1_000; // Minimum token amount
