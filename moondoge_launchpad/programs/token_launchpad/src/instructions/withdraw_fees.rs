use anchor_lang::prelude::*;
use crate::state::*;
use crate::constants::*;
use crate::errors::*;
use crate::events::*;

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.authority == authority.key() @ LaunchpadError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub authority: Signer<'info>,

    #[account(
        mut,
        address = global_config.fee_recipient
    )]
    pub fee_recipient: SystemAccount<'info>,
}

pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
    let fee_recipient = &ctx.accounts.fee_recipient;
    let amount = fee_recipient.lamports();

    if amount > 0 {
        // Transfer all lamports from fee_recipient to authority
        **fee_recipient.to_account_info().try_borrow_mut_lamports()? = 0;
        **ctx.accounts.authority.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .authority
            .to_account_info()
            .lamports()
            .checked_add(amount)
            .ok_or(LaunchpadError::MathOverflow)?;

        emit!(FeesWithdrawn {
            recipient: ctx.accounts.authority.key(),
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!(
            "Fees withdrawn: recipient={}, amount={}",
            ctx.accounts.authority.key(),
            amount
        );
    }

    Ok(())
}
