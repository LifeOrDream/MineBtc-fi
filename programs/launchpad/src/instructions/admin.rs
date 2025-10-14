use anchor_lang::prelude::*;
use crate::{constants::*, errors::NftLaunchpadError, events::*, state::*};

// ========================================================================================
// ================================ ADMIN FUNCTIONS ======================================
// ========================================================================================

/// Initialize the NFT Launchpad program with collections
pub fn initialize_handler(
    ctx: Context<Initialize>,
    moondoge_collection_name: String,
    _moondoge_collection_symbol: String,
    moondoge_collection_uri: String,
    dragon_egg_collection_name: String,
    _dragon_egg_collection_symbol: String,
    dragon_egg_collection_uri: String,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Create MoonDoge collection with MPL Core
    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.moondoge_collection,
        None, // No parent collection for collections
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        moondoge_collection_name,
        moondoge_collection_uri.clone(),
    )?;
    
    // Create Dragon Egg collection with MPL Core
    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.dragon_egg_collection,
        None, // No parent collection for collections
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        dragon_egg_collection_name,
        dragon_egg_collection_uri.clone(),
    )?;
    
    // Initialize global config
    global_config.authority = ctx.accounts.authority.key();
    global_config.treasury = ctx.accounts.sol_treasury.key();
    global_config.moondoge_collection = ctx.accounts.moondoge_collection.key();
    global_config.dragon_egg_collection = ctx.accounts.dragon_egg_collection.key();
    global_config.total_moondoges_minted = 0;
    global_config.total_dragon_eggs_minted = 0;
    global_config.total_sol_collected = 0;
    global_config.moondoge_uris = Vec::new();  // Initialize empty, admin adds later
    global_config.dragon_egg_uris = Vec::new();  // Initialize empty, admin adds later
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
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    if let Some(authority) = new_authority {
        global_config.authority = authority;
    }
    
    emit!(ConfigUpdated {
        authority: global_config.authority,
        new_authority,
        new_treasury: None,
    });
    
    msg!("✅ Configuration updated");
    
    Ok(())
}

 
/// Add MoonDoge URIs to the pool (admin only)
pub fn add_moondoge_uris_handler(
    ctx: Context<UpdateConfig>,
    uris: Vec<String>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= crate::constants::MAX_URI_LENGTH, crate::errors::NftLaunchpadError::UriTooLong);
    }
    
    // Add new URIs
    global_config.moondoge_uris.extend(uris.clone());
    
    msg!("✅ Added {} MoonDoge URIs", uris.len());
    msg!("   Total MoonDoge URIs: {}", global_config.moondoge_uris.len());
    
    Ok(())
}

/// Add Dragon Egg URIs to the pool (admin only)
pub fn add_dragon_egg_uris_handler(
    ctx: Context<UpdateConfig>,
    uris: Vec<String>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= crate::constants::MAX_URI_LENGTH, crate::errors::NftLaunchpadError::UriTooLong);
    }
    
    // Add new URIs
    global_config.dragon_egg_uris.extend(uris.clone());
    
    msg!("✅ Added {} Dragon Egg URIs", uris.len());
    msg!("   Total Dragon Egg URIs: {}", global_config.dragon_egg_uris.len());
    
    Ok(())
}

/// Clear all MoonDoge URIs (admin only)
pub fn clear_moondoge_uris_handler(
    ctx: Context<UpdateConfig>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    global_config.moondoge_uris.clear();
    
    msg!("✅ Cleared all MoonDoge URIs");
    
    Ok(())
}

/// Clear all Dragon Egg URIs (admin only)
pub fn clear_dragon_egg_uris_handler(
    ctx: Context<UpdateConfig>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    global_config.dragon_egg_uris.clear();
    
    msg!("✅ Cleared all Dragon Egg URIs");
    
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
    
    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,
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