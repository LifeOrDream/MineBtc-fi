use anchor_lang::prelude::*;

declare_id!("76bGWqGdzwR13FSd1TDwanK7GFDHcunKh6WGbzAW1PjU");

#[program]
pub mod moonbase {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("MoonBase initialized");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub system_program: Program<'info, System>,
}
