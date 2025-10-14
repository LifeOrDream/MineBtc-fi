use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
// pub mod instructions;
pub mod mpl_core_helpers;
pub mod state;
// pub mod utils;


declare_id!("CG6btG2MbTDXR2Ws6Kqn24HG6VqWWFJBrfxAK7NJVyNA");

#[program]
pub mod nft_launchpad {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("NFT Launchpad initialized");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub system_program: Program<'info, System>,
}

