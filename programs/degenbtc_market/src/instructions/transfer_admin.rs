use anchor_lang::prelude::*;

use crate::errors::MarketError;
use crate::events::AdminTransferred;
use crate::log_fn;
use crate::state::MarketplaceConfig;

#[derive(Accounts)]
pub struct TransferAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = admin @ MarketError::Unauthorized,
    )]
    pub marketplace_config: Account<'info, MarketplaceConfig>,
}

pub fn handler(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
    log_fn!("market", "transfer_admin");

    require!(new_admin != Pubkey::default(), MarketError::InvalidAdmin);

    let config = &mut ctx.accounts.marketplace_config;
    let old_admin = config.admin;
    require!(new_admin != old_admin, MarketError::InvalidAdmin);

    config.admin = new_admin;
    msg!("   admin: {} -> {}", old_admin, new_admin);

    emit!(AdminTransferred {
        config: config.key(),
        old_admin,
        new_admin,
    });

    Ok(())
}
