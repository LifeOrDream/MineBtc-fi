use anchor_lang::prelude::*;

#[error_code]
pub enum MarketError {
    #[msg("Marketplace is disabled")]
    MarketplaceDisabled,
    #[msg("Price below minimum")]
    PriceTooLow,
    #[msg("Listing price exceeds buyer max price")]
    PriceTooHigh,
    #[msg("Fee exceeds max 10%")]
    FeeTooHigh,
    #[msg("Asset not in registered collection")]
    NotCollectionMember,
    #[msg("Seller mismatch")]
    SellerMismatch,
    #[msg("Insufficient buyer funds")]
    InsufficientFunds,
    #[msg("Invalid MPL Core program")]
    InvalidMplCoreProgram,
    #[msg("Admin only")]
    Unauthorized,
    #[msg("Asset has unsupported plugin")]
    UnsupportedPlugin,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Invalid fee recipient")]
    InvalidFeeRecipient,
    #[msg("Invalid collection")]
    InvalidCollection,
    #[msg("Asset deserialization failed")]
    InvalidAsset,
    #[msg("Listing is not stale — current owner still matches seller")]
    ListingNotStale,
}
