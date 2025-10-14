use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;

#[derive(Accounts)]
#[instruction(index: u16)]
pub struct InitializeAmmConfig<'info> {
    #[account(
        init,
        payer = owner,
        space = AmmConfig::SPACE,
        seeds = [AMM_CONFIG_SEED, &index.to_le_bytes()],
        bump
    )]
    pub amm_config: Account<'info, AmmConfig>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_amm_config(
    ctx: Context<InitializeAmmConfig>,
    index: u16,
    trade_fee_rate: u64,
    protocol_fee_rate: u64,
    fund_fee_rate: u64,
    create_pool_fee: u64,
) -> Result<()> {
    require!(
        trade_fee_rate <= MAX_FEE_RATE,
        AmmError::InvalidFeeRate
    );
    require!(
        protocol_fee_rate <= FEE_RATE_DENOMINATOR,
        AmmError::InvalidFeeRate
    );
    require!(
        fund_fee_rate <= FEE_RATE_DENOMINATOR,
        AmmError::InvalidFeeRate
    );
    require!(
        fund_fee_rate + protocol_fee_rate <= FEE_RATE_DENOMINATOR,
        AmmError::InvalidFeeRate
    );

    let amm_config = &mut ctx.accounts.amm_config;
    
    amm_config.bump = ctx.bumps.amm_config;
    amm_config.index = index;
    amm_config.owner = ctx.accounts.owner.key();
    amm_config.protocol_fee_rate = protocol_fee_rate;
    amm_config.trade_fee_rate = trade_fee_rate;
    amm_config.fund_fee_rate = fund_fee_rate;
    amm_config.create_pool_fee = create_pool_fee;
    amm_config.fund_owner = ctx.accounts.owner.key();
    amm_config.padding = [0; 16];

    emit!(AmmConfigCreated {
        index,
        owner: ctx.accounts.owner.key(),
        trade_fee_rate,
        protocol_fee_rate,
        fund_fee_rate,
        create_pool_fee,
    });

    Ok(())
}
