use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;

#[derive(Accounts)]
pub struct UpdateAmmConfig<'info> {
    #[account(
        mut,
        seeds = [AMM_CONFIG_SEED, &amm_config.index.to_le_bytes()],
        bump = amm_config.bump,
        constraint = amm_config.owner == owner.key() @ AmmError::Unauthorized
    )]
    pub amm_config: Account<'info, AmmConfig>,
    
    pub owner: Signer<'info>,
}

pub fn update_amm_config(
    ctx: Context<UpdateAmmConfig>,
    param: u8,
    value: u64,
) -> Result<()> {
    let amm_config = &mut ctx.accounts.amm_config;

    match param {
        CONFIG_PARAM_TRADE_FEE => {
            require!(value <= MAX_FEE_RATE, AmmError::InvalidFeeRate);
            amm_config.trade_fee_rate = value;
        }
        CONFIG_PARAM_PROTOCOL_FEE => {
            require!(value <= FEE_RATE_DENOMINATOR, AmmError::InvalidFeeRate);
            amm_config.protocol_fee_rate = value;
        }
        CONFIG_PARAM_FUND_FEE => {
            require!(value <= FEE_RATE_DENOMINATOR, AmmError::InvalidFeeRate);
            amm_config.fund_fee_rate = value;
        }
        CONFIG_PARAM_OWNER => {
            let new_owner = Pubkey::try_from(value.to_le_bytes())
                .map_err(|_| AmmError::InvalidConfigParameter)?;
            amm_config.owner = new_owner;
        }
        CONFIG_PARAM_FUND_OWNER => {
            let new_fund_owner = Pubkey::try_from(value.to_le_bytes())
                .map_err(|_| AmmError::InvalidConfigParameter)?;
            amm_config.fund_owner = new_fund_owner;
        }
        _ => return Err(AmmError::InvalidConfigParameter.into()),
    }

    emit!(AmmConfigUpdated {
        index: amm_config.index,
        param,
        value,
    });

    Ok(())
}
