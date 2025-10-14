use anchor_lang::prelude::*;

declare_id!("F6VLRcemPCd4Hm9iREcpXzvPfXNhQwxMmY5afpkH6wVZ");

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
