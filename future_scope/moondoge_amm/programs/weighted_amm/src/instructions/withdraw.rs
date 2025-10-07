// Placeholder for withdraw instruction
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub placeholder: Signer<'info>,
}

pub fn withdraw(
    _ctx: Context<Withdraw>,
    _lp_token_amount: u64,
    _minimum_token_0_amount: u64,
    _minimum_token_1_amount: u64,
) -> Result<()> {
    Ok(())
}
