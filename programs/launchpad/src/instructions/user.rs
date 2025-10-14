use anchor_lang::prelude::*;
use crate::{constants::*, errors::NftLaunchpadError, events::*, state::*, utils::*};

// ========================================================================================
// ========================== MOONBASE CREATION WITH NFTS ================================
// ========================================================================================

/// Mint NFTs based on moonbase creation tier
/// Called by moonbase program during moonbase creation
pub fn mint_nfts_for_moonbase_handler(
    ctx: Context<MintNftsForMoonbase>,
    pricing_tier: u64, // MOONBASE_BASIC_PRICE, MOONBASE_DOGE_PRICE, or MOONBASE_FULL_PRICE
) -> Result<()> {
    // Get account info before mutable borrow
    let global_config_info = ctx.accounts.global_config.to_account_info();
    let global_config = &mut ctx.accounts.global_config;

    // Determine what NFTs to mint
    let (mint_doge, mint_egg) = determine_pricing_tier(pricing_tier)?;

    let mut moondoge_mint: Option<Pubkey> = None;
    let mut dragon_egg_mint: Option<Pubkey> = None;

    // Mint MoonDoge if tier includes it --> If
    if mint_doge && global_config.total_moondoges_minted < MAX_MOONDOGE_SUPPLY {        
        let index = global_config.total_moondoges_minted;
        let name = generate_moondoge_name(index);
        let uri = global_config.get_random_moondoge_uri(Clock::get()?.slot, index)?;
        
        // Create MoonDoge NFT with MPL Core
        crate::mpl_core_helpers::create_mpl_core_asset(
            &ctx.accounts.moondoge_asset.as_ref().unwrap(),
            Some(&ctx.accounts.moondoge_collection),
            global_config_info.as_ref(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            name.clone(),
            uri.clone(),
        )?;
        
        let doge_metadata = &mut ctx.accounts.moondoge_metadata.as_mut().unwrap();
        doge_metadata.mint = ctx.accounts.moondoge_asset.as_ref().unwrap().key();
        doge_metadata.money = BASE_DOGE_MONEY;
        doge_metadata.attached_moonbase = None;
        doge_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        doge_metadata.total_btc_mined = 0;
        doge_metadata.created_at = Clock::get()?.unix_timestamp;
        doge_metadata.bump = ctx.bumps.moondoge_metadata.unwrap();
        
        global_config.total_moondoges_minted += 1;
        moondoge_mint = Some(doge_metadata.mint);
        
        emit!(MoonDogeMinted {
            mint: doge_metadata.mint,
            name,
            uri,
            price_paid: 0, // Included in moonbase price
        });
    }
    
    // Mint Dragon Egg if tier includes it
    if mint_egg && global_config.total_dragon_eggs_minted < INITIAL_DRAGON_EGG_SUPPLY {

        let index = global_config.total_dragon_eggs_minted;
        let name = generate_dragon_egg_name(index);
        let dna = crate::state::generate_dragon_egg_dna(
            Clock::get()?.slot,
            &ctx.accounts.user.key(),
            index,
        );
        let uri = global_config.get_random_dragon_egg_uri(Clock::get()?.slot, index, &dna)?;
        
        // Create Dragon Egg NFT with MPL Core
        crate::mpl_core_helpers::create_mpl_core_asset(
            &ctx.accounts.dragon_egg_asset.as_ref().unwrap(),
            Some(&ctx.accounts.dragon_egg_collection),
            global_config_info.as_ref(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            name.clone(),
            uri.clone(),
        )?;
        
        let egg_metadata = &mut ctx.accounts.dragon_egg_metadata.as_mut().unwrap();
        egg_metadata.mint = ctx.accounts.dragon_egg_asset.as_ref().unwrap().key();
        egg_metadata.power = BASE_EGG_POWER;
        egg_metadata.dna = dna;
        egg_metadata.incubated_moonbase = None;
        egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        egg_metadata.total_hashpower_accumulated = 0;
        egg_metadata.created_at = Clock::get()?.unix_timestamp;
        egg_metadata.bump = ctx.bumps.dragon_egg_metadata.unwrap();
        
        global_config.total_dragon_eggs_minted += 1;
        dragon_egg_mint = Some(egg_metadata.mint);
        
        emit!(DragonEggMinted {
            mint: egg_metadata.mint,
            name,
            uri,
            dna,
            initial_power: BASE_EGG_POWER,
            price_paid: 0, // Included in moonbase price
        });
    }
    
    global_config.total_sol_collected += pricing_tier;
    
    emit!(MoonbaseCreatedWithNfts {
        moonbase_owner: ctx.accounts.user.key(),
        pricing_tier: get_pricing_tier_name(pricing_tier).to_string(),
        sol_paid: pricing_tier,
        moondoge_minted: moondoge_mint,
        dragon_egg_minted: dragon_egg_mint,
    });
    
    emit!(SOLFeesCollected {
        source: "moonbase_creation".to_string(),
        amount: pricing_tier,
        pricing_tier: Some(get_pricing_tier_name(pricing_tier).to_string()),
    });
    
    msg!("✅ NFTs minted for moonbase creation");
    msg!("   Tier: {}", get_pricing_tier_name(pricing_tier));
    if let Some(doge) = moondoge_mint {
        msg!("   MoonDoge: {}", doge);
    }
    if let Some(egg) = dragon_egg_mint {
        msg!("   Dragon Egg: {}", egg);
    }

    Ok(())
}



// ========================================================================================
// ================================ ACCOUNT CONTEXTS =====================================
// ========================================================================================

#[derive(Accounts)]
pub struct MintNftsForMoonbase<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// CHECK: MoonDoge asset (if applicable) - will be created via CPI
    #[account(mut)]
    pub moondoge_asset: Option<AccountInfo<'info>>,

    /// CHECK: MoonDoge collection
    #[account(mut)]
    pub moondoge_collection: UncheckedAccount<'info>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = MoonDogeMetadata::LEN,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_asset.as_ref().unwrap().key().as_ref()],
        bump
    )]
    pub moondoge_metadata: Option<Account<'info, MoonDogeMetadata>>,
    
    /// CHECK: Dragon Egg asset (if applicable) - will be created via CPI
    #[account(mut)]
    pub dragon_egg_asset: Option<AccountInfo<'info>>,

    /// CHECK: Dragon Egg collection
    #[account(mut)]
    pub dragon_egg_collection: UncheckedAccount<'info>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_asset.as_ref().unwrap().key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Option<Account<'info, DragonEggMetadata>>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    
    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,
}
