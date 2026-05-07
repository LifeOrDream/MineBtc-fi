use anchor_lang::prelude::*;
use mpl_core::accounts::BaseCollectionV1;

use crate::errors::MarketError;
use crate::events::MarketplaceInitialized;
use crate::log_fn;
use crate::state::{MarketplaceConfig, MARKETPLACE_CONFIG_SEED, MAX_FEE_BPS};

#[derive(Accounts)]
pub struct InitializeMarketplace<'info> {
    /// Pays rent for the new `MarketplaceConfig`.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Recorded as `config.admin`. No state read here, just identity. Must
    /// sign so a stranger can't backdoor a config under someone else's admin key.
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = MarketplaceConfig::LEN,
        seeds = [MARKETPLACE_CONFIG_SEED, collection_mint.key().as_ref()],
        bump,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,

    /// mpl-core `CollectionV1` mint. Validated by Anchor via the `Owner` impl.
    pub collection_mint: Account<'info, BaseCollectionV1>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeMarketplace>,
    fee_bps: u16,
    fee_recipient: Pubkey,
    min_price_lamports: u64,
    mpl_core_program: Pubkey,
) -> Result<()> {
    log_fn!("market", "initialize_marketplace");

    require!(fee_bps <= MAX_FEE_BPS, MarketError::FeeTooHigh);
    require!(
        fee_recipient != Pubkey::default(),
        MarketError::InvalidFeeRecipient
    );
    // We cache the mpl-core program id and double-check in every transfer ix,
    // so a typo or zero key here would brick the entire marketplace. Reject
    // it up front.
    require!(
        mpl_core_program == mpl_core::ID,
        MarketError::InvalidMplCoreProgram
    );

    let collection_mint = ctx.accounts.collection_mint.key();

    let config = &mut ctx.accounts.marketplace_config;
    config.bump = ctx.bumps.marketplace_config;
    config.admin = ctx.accounts.admin.key();
    config.enabled = true;
    config.collection_mint = collection_mint;
    config.fee_bps = fee_bps;
    config.fee_recipient = fee_recipient;
    config.min_price_lamports = min_price_lamports;
    config.mpl_core_program = mpl_core_program;

    msg!("✅ Marketplace initialized");
    msg!("   admin: {}", config.admin);
    msg!("   collection_mint: {}", config.collection_mint);
    msg!("   fee_bps: {}", config.fee_bps);
    msg!("   fee_recipient: {}", config.fee_recipient);
    msg!("   min_price_lamports: {}", config.min_price_lamports);

    emit!(MarketplaceInitialized {
        config: config.key(),
        collection_mint,
        fee_bps,
    });

    Ok(())
}
