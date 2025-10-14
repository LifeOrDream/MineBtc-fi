use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;

#[derive(Accounts)]
pub struct InitializeGlobalConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::SPACE,
        seeds = [GLOBAL_CONFIG_SEED],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_global_config(
    ctx: Context<InitializeGlobalConfig>,
    fee_recipient: Pubkey,
    platform_fee_bps: u16,
    token_creation_fee: u64,
    migration_fee_bps: u16,
) -> Result<()> {
    require!(
        platform_fee_bps <= MAX_FEE_BPS,
        LaunchpadError::InvalidFeePercentage
    );
    require!(
        migration_fee_bps <= MAX_FEE_BPS,
        LaunchpadError::InvalidFeePercentage
    );

    let global_config = &mut ctx.accounts.global_config;
    
    global_config.authority = ctx.accounts.authority.key();
    global_config.fee_recipient = fee_recipient;
    global_config.platform_fee_bps = platform_fee_bps;
    global_config.token_creation_fee = token_creation_fee;
    global_config.migration_fee_bps = migration_fee_bps;
    global_config.total_tokens_created = 0;
    global_config.total_volume_sol = 0;
    global_config.total_fees_collected = 0;
    global_config.bump = ctx.bumps.global_config;

    msg!(
        "Global config initialized with authority: {}, fee_recipient: {}, platform_fee_bps: {}, token_creation_fee: {}, migration_fee_bps: {}",
        global_config.authority,
        global_config.fee_recipient,
        global_config.platform_fee_bps,
        global_config.token_creation_fee,
        global_config.migration_fee_bps
    );

    Ok(())
}
