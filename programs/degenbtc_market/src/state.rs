use anchor_lang::prelude::*;

// ------------------------------
// Seed constants
// ------------------------------

pub const MARKETPLACE_CONFIG_SEED: &[u8] = b"marketplace-config";
pub const LISTING_SEED: &[u8] = b"listing";
pub const ESCROW_SEED: &[u8] = b"escrow";

// ------------------------------
// Config bounds
// ------------------------------

/// Hard ceiling on the marketplace fee (10.00%).
pub const MAX_FEE_BPS: u16 = 1000;

/// Default fee on `initialize` if the caller passes 0 or > `MAX_FEE_BPS`.
/// Spec calls for 3.00% as the protocol default.
pub const DEFAULT_FEE_BPS: u16 = 300;

/// Floor on listing prices when the caller passes 0 at init time. 0.01 SOL.
pub const DEFAULT_MIN_PRICE_LAMPORTS: u64 = 10_000_000;

/// Basis-point denominator for fee math.
pub const BPS_DENOMINATOR: u64 = 10_000;

// ------------------------------
// MarketplaceConfig PDA
// ------------------------------

/// PDA: `[b"marketplace-config", collection_mint]`
///
/// Per-collection configuration for the marketplace. The cached
/// `mpl_core_program` is checked on every transfer-bearing ix so a malicious
/// caller can't slip in a fake mpl-core program account.
#[account]
pub struct MarketplaceConfig {
    pub bump: u8,
    pub admin: Pubkey,
    pub enabled: bool,
    /// Verified mpl-core `CollectionV1` mint.
    pub collection_mint: Pubkey,
    /// 300 = 3.00%
    pub fee_bps: u16,
    /// SOL recipient for marketplace fees (mineBTC `fee_recipient`).
    pub fee_recipient: Pubkey,
    /// Hard floor on listing prices in lamports.
    pub min_price_lamports: u64,
    /// Cached MPL Core program id used in transfer CPIs.
    pub mpl_core_program: Pubkey,
}

impl MarketplaceConfig {
    /// 8 (anchor disc) + 1 + 32 + 1 + 32 + 2 + 32 + 8 + 32 = 148 bytes
    pub const LEN: usize = 8 + 1 + 32 + 1 + 32 + 2 + 32 + 8 + 32;
}

// ------------------------------
// Listing PDA
// ------------------------------

/// PDA: `[b"listing", marketplace_config, asset]`
///
/// Existence of this account ⇔ listing is active. Cancel and buy both close
/// the account (rent refund flows to the seller in both cases).
#[account]
pub struct Listing {
    pub bump: u8,
    pub seller: Pubkey,
    pub asset: Pubkey,
    pub price_lamports: u64,
    pub created_at: i64,
}

impl Listing {
    /// 8 (anchor disc) + 1 + 32 + 32 + 8 + 8 = 89 bytes
    pub const LEN: usize = 8 + 1 + 32 + 32 + 8 + 8;
}
