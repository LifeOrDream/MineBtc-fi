use anchor_lang::prelude::*;

use crate::errors::MarketError;
use crate::events::MarketplaceConfigUpdated;
use crate::log_fn;
use crate::state::{MarketplaceConfig, MAX_FEE_BPS};

#[derive(Accounts)]
pub struct UpdateMarketplaceConfig<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = admin @ MarketError::Unauthorized,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,
}

pub fn handler(
    ctx: Context<UpdateMarketplaceConfig>,
    fee_bps: Option<u16>,
    fee_recipient: Option<Pubkey>,
    min_price_lamports: Option<u64>,
    enabled: Option<bool>,
) -> Result<()> {
    log_fn!("market", "update_marketplace_config");

    let config = &mut ctx.accounts.marketplace_config;

    if let Some(new_fee_bps) = fee_bps {
        require!(new_fee_bps <= MAX_FEE_BPS, MarketError::FeeTooHigh);
        msg!("   fee_bps: {} -> {}", config.fee_bps, new_fee_bps);
        config.fee_bps = new_fee_bps;
    }

    if let Some(new_recipient) = fee_recipient {
        require!(
            new_recipient != Pubkey::default(),
            MarketError::InvalidFeeRecipient
        );
        msg!(
            "   fee_recipient: {} -> {}",
            config.fee_recipient,
            new_recipient
        );
        config.fee_recipient = new_recipient;
    }

    if let Some(new_min) = min_price_lamports {
        msg!(
            "   min_price_lamports: {} -> {}",
            config.min_price_lamports,
            new_min
        );
        config.min_price_lamports = new_min;
    }

    if let Some(new_enabled) = enabled {
        msg!("   enabled: {} -> {}", config.enabled, new_enabled);
        config.enabled = new_enabled;
    }

    emit!(MarketplaceConfigUpdated {
        config: config.key(),
        fee_bps: config.fee_bps,
        fee_recipient: config.fee_recipient,
        enabled: config.enabled,
        min_price_lamports: config.min_price_lamports,
    });

    Ok(())
}
