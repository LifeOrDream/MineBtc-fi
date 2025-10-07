// Fee constants
pub const FEE_RATE_DENOMINATOR: u64 = 1_000_000; // 1,000,000 = 100%
pub const MAX_FEE_RATE: u64 = 100_000; // 10% maximum fee
pub const MIN_FEE_RATE: u64 = 1; // 0.0001% minimum fee

// Weight constants
pub const TOTAL_WEIGHT: u64 = 100; // Total weight must equal 100
pub const MIN_WEIGHT: u64 = 1;     // 1% minimum weight
pub const MAX_WEIGHT: u64 = 99;    // 99% maximum weight

// Pool constants
pub const MIN_LIQUIDITY: u64 = 1000; // Minimum liquidity to prevent division by zero
pub const MAX_SWAP_RATIO: u128 = 300_000_000_000_000_000; // 30% of pool balance

// Math constants
pub const PRECISION: u128 = 1_000_000_000_000_000_000; // 18 decimal precision
pub const BONE: u128 = PRECISION; // Base unit for calculations
pub const MAX_ITERATIONS: u8 = 32; // Maximum iterations for power calculations

// Seeds for PDA derivation
pub const AMM_CONFIG_SEED: &[u8] = b"amm_config";
pub const POOL_SEED: &[u8] = b"pool";
pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";
pub const POOL_LP_MINT_SEED: &[u8] = b"pool_lp_mint";
pub const POOL_AUTH_SEED: &[u8] = b"vault_and_lp_mint_auth_seed";

// Pool status
pub const POOL_STATUS_UNINITIALIZED: u8 = 0;
pub const POOL_STATUS_ENABLED: u8 = 1;
pub const POOL_STATUS_DISABLED: u8 = 2;
pub const POOL_STATUS_REMOVE_LIQUIDITY_ONLY: u8 = 3;

// Configuration parameters
pub const CONFIG_PARAM_TRADE_FEE: u8 = 0;
pub const CONFIG_PARAM_PROTOCOL_FEE: u8 = 1;
pub const CONFIG_PARAM_FUND_FEE: u8 = 2;
pub const CONFIG_PARAM_OWNER: u8 = 3;
pub const CONFIG_PARAM_FUND_OWNER: u8 = 4;
