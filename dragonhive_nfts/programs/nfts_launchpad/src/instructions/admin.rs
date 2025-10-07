use anchor_lang::prelude::*;
use crate::{constants::*, errors::NftLaunchpadError, events::*, state::*};

// ========================================================================================
// ================================ ADMIN FUNCTIONS ======================================
// ========================================================================================

/// Initialize the NFT Launchpad program with collections
pub fn initialize_handler(
    ctx: Context<Initialize>,
    _moondoge_collection_name: String,
    _moondoge_collection_symbol: String,
    _moondoge_collection_uri: String,
    _dragon_egg_collection_name: String,
    _dragon_egg_collection_symbol: String,
    _dragon_egg_collection_uri: String,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Initialize global config
    global_config.authority = ctx.accounts.authority.key();
    global_config.treasury = ctx.accounts.sol_treasury.key();
    global_config.moondoge_collection = ctx.accounts.moondoge_collection.key();
    global_config.dragon_egg_collection = ctx.accounts.dragon_egg_collection.key();
    global_config.total_moondoges_minted = 0;
    global_config.total_dragon_eggs_minted = 0;
    global_config.total_sol_collected = 0;
    global_config.is_paused = false;
    global_config.config_bump = ctx.bumps.global_config;
    global_config.treasury_bump = ctx.bumps.sol_treasury;
    
    emit!(ProgramInitialized {
        authority: global_config.authority,
        moondoge_collection: global_config.moondoge_collection,
        dragon_egg_collection: global_config.dragon_egg_collection,
    });
    
    msg!("✅ NFT Launchpad initialized");
    msg!("   MoonDoge Collection: {}", global_config.moondoge_collection);
    msg!("   Dragon Egg Collection: {}", global_config.dragon_egg_collection);
    
    Ok(())
}

/// Update global configuration (admin only)
pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    new_authority: Option<Pubkey>,
    new_treasury: Option<Pubkey>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    if let Some(authority) = new_authority {
        global_config.authority = authority;
    }
    
    if let Some(treasury) = new_treasury {
        global_config.treasury = treasury;
    }
    
    emit!(ConfigUpdated {
        authority: global_config.authority,
        new_authority,
        new_treasury,
    });
    
    msg!("✅ Configuration updated");
    
    Ok(())
}

/// Pause/unpause the program (admin only)
pub fn pause_program_handler(
    ctx: Context<PauseProgram>,
    is_paused: bool,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    global_config.is_paused = is_paused;
    
    msg!("✅ Program {} paused", if is_paused { "is now" } else { "is no longer" });
    
    Ok(())
}

// ========================================================================================
// ================================ ACCOUNT CONTEXTS =====================================
// ========================================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [GLOBAL_CONFIG_SEED],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// CHECK: MoonDoge collection (Metaplex Core asset)
    #[account(mut)]
    pub moondoge_collection: UncheckedAccount<'info>,
    
    /// CHECK: Dragon Egg collection (Metaplex Core asset)
    #[account(mut)]
    pub dragon_egg_collection: UncheckedAccount<'info>,
    
    /// CHECK: SOL treasury PDA (0-byte account for collecting fees)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [SOL_TREASURY_SEED],
        bump,
        owner = crate::ID
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ NftLaunchpadError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PauseProgram<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ NftLaunchpadError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
