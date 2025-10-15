use anchor_lang::prelude::*;

declare_id!("4iiv7LNziMazs9S5gUL1XQZKEwbowDA3m2wMfZfbc8cv");

#[program]
pub mod moonbase {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("MoonBase initialized");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
