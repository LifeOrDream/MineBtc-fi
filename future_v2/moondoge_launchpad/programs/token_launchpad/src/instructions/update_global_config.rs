use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;

#[derive(Accounts)]
pub struct UpdateGlobalConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.authority == authority.key() @ LaunchpadError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub authority: Signer<'info>,
}

pub fn update_global_config(
    ctx: Context<UpdateGlobalConfig>,
    new_authority: Option<Pubkey>,
    new_fee_recipient: Option<Pubkey>,
    new_platform_fee_bps: Option<u16>,
    new_token_creation_fee: Option<u64>,
    new_migration_fee_bps: Option<u16>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    if let Some(authority) = new_authority {
        global_config.authority = authority;
    }

    if let Some(fee_recipient) = new_fee_recipient {
        global_config.fee_recipient = fee_recipient;
    }

    if let Some(platform_fee_bps) = new_platform_fee_bps {
        require!(
            platform_fee_bps <= MAX_FEE_BPS,
            LaunchpadError::InvalidFeePercentage
        );
        global_config.platform_fee_bps = platform_fee_bps;
    }

    if let Some(token_creation_fee) = new_token_creation_fee {
        global_config.token_creation_fee = token_creation_fee;
    }

    if let Some(migration_fee_bps) = new_migration_fee_bps {
        require!(
            migration_fee_bps <= MAX_FEE_BPS,
            LaunchpadError::InvalidFeePercentage
        );
        global_config.migration_fee_bps = migration_fee_bps;
    }

    msg!(
        "Global config updated: authority={}, fee_recipient={}, platform_fee_bps={}, token_creation_fee={}, migration_fee_bps={}",
        global_config.authority,
        global_config.fee_recipient,
        global_config.platform_fee_bps,
        global_config.token_creation_fee,
        global_config.migration_fee_bps
    );

    Ok(())
}
