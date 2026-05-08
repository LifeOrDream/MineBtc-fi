use anchor_lang::prelude::*;

#[event]
pub struct MarketplaceInitialized {
    pub config: Pubkey,
    pub collection_mint: Pubkey,
    pub fee_bps: u16,
}

#[event]
pub struct MarketplaceConfigUpdated {
    pub config: Pubkey,
    pub fee_bps: u16,
    pub fee_recipient: Pubkey,
    pub enabled: bool,
    pub min_price_lamports: u64,
}

#[event]
pub struct NftListed {
    pub asset: Pubkey,
    pub seller: Pubkey,
    pub price_lamports: u64,
    pub timestamp: i64,
}

#[event]
pub struct ListingCancelled {
    pub asset: Pubkey,
    pub seller: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ListingPriceUpdated {
    pub asset: Pubkey,
    pub seller: Pubkey,
    pub new_price_lamports: u64,
    pub timestamp: i64,
}

#[event]
pub struct NftSold {
    pub asset: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub price_lamports: u64,
    pub fee_lamports: u64,
    pub timestamp: i64,
}
