use anchor_lang::prelude::*;

declare_id!("CG6btG2MbTDXR2Ws6Kqn24HG6VqWWFJBrfxAK7NJVyNA");

#[program]
pub mod nft_launchpad {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("NFT Launchpad initialized");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub system_program: Program<'info, System>,
}

