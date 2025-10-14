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
    let (mint_egg,) = determine_pricing_tier(pricing_tier)?;

    let mut dragon_egg_mint: Option<Pubkey> = None;
    
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
        dragon_egg_minted: dragon_egg_mint,
    });
    
    emit!(SOLFeesCollected {
        source: "moonbase_creation".to_string(),
        amount: pricing_tier,
        pricing_tier: Some(get_pricing_tier_name(pricing_tier).to_string()),
    });
    
    msg!("✅ NFTs minted for moonbase creation");
    msg!("   Tier: {}", get_pricing_tier_name(pricing_tier));
    if let Some(egg) = dragon_egg_mint {
        msg!("   Dragon Egg: {}", egg);
    }

    Ok(())
}
// ========================================================================================
// ========================== DRAGON EGG INCUBATION ======================================
// ========================================================================================

/// Add Dragon Egg to moonbase incubation
pub fn incubate_dragon_egg_handler(
    ctx: Context<IncubateDragonEgg>,
) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let incubation_state = &mut ctx.accounts.incubation_state;
    
    // Verify ownership from Metaplex Core asset (source of truth)
    verify_nft_ownership(&ctx.accounts.dragon_egg_asset, &ctx.accounts.user.key())?;
    
    require!(
        egg_metadata.incubated_moonbase.is_none(),
        NftLaunchpadError::EggAlreadyIncubated
    );
    require!(
        incubation_state.incubated_eggs.len() < MAX_EGGS_PER_MOONBASE as usize,
        NftLaunchpadError::MaxEggsReached
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    
    // Add egg to incubation state
    incubation_state.incubated_eggs.push(egg_metadata.mint);
    incubation_state.last_update_ts = current_time;
    
    // Update egg metadata
    egg_metadata.incubated_moonbase = Some(ctx.accounts.user.key());
    egg_metadata.last_update_ts = current_time;
    
    let total_eggs = incubation_state.incubated_eggs.len() as u8;
    
    emit!(DragonEggIncubated {
        egg_mint: egg_metadata.mint,
        moonbase_owner: ctx.accounts.user.key(),
        incubated_at: current_time,
        total_eggs_in_moonbase: total_eggs,
    });
    
    msg!("✅ Dragon Egg added to incubation");
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Total eggs in moonbase: {}", total_eggs);
    
    Ok(())
}

/// Remove Dragon Egg from moonbase incubation
pub fn remove_dragon_egg_handler(
    ctx: Context<RemoveDragonEgg>,
) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let incubation_state = &mut ctx.accounts.incubation_state;
    
    // Verify ownership from Metaplex Core asset (source of truth)
    verify_nft_ownership(&ctx.accounts.dragon_egg_asset, &ctx.accounts.user.key())?;
    
    require!(
        egg_metadata.incubated_moonbase.is_some(),
        NftLaunchpadError::EggNotIncubated
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    
    // Remove egg from incubation state
    if let Some(pos) = incubation_state.incubated_eggs.iter().position(|&x| x == egg_metadata.mint) {
        incubation_state.incubated_eggs.remove(pos);
    }
    incubation_state.last_update_ts = current_time;
    
    // Update egg metadata
    let final_power = egg_metadata.power;
    egg_metadata.incubated_moonbase = None;
    egg_metadata.last_update_ts = current_time;
    
    let remaining_eggs = incubation_state.incubated_eggs.len() as u8;
    
    emit!(DragonEggRemoved {
        egg_mint: egg_metadata.mint,
        moonbase_owner: ctx.accounts.user.key(),
        removed_at: current_time,
        final_power,
        remaining_eggs_in_moonbase: remaining_eggs,
    });
    
    msg!("✅ Dragon Egg removed from incubation");
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Final Power: {}", final_power);
    
    Ok(())
}

/// Update Dragon Egg power based on hashpower
pub fn update_dragon_egg_power_handler(
    ctx: Context<UpdateDragonEggPower>,
    total_hashpower: u64,
) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let incubation_state = &mut ctx.accounts.incubation_state;
    
    require!(
        egg_metadata.incubated_moonbase.is_some(),
        NftLaunchpadError::EggNotIncubated
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    let time_elapsed = current_time.saturating_sub(egg_metadata.last_update_ts);
    let total_eggs = incubation_state.incubated_eggs.len() as u8;
    
    let old_power = egg_metadata.power;
    let power_increase = egg_metadata.calculate_power_increase(
        total_hashpower,
        total_eggs,
        time_elapsed,
    );
    let new_power = old_power.saturating_add(power_increase).min(MAX_EGG_POWER);
    
    egg_metadata.power = new_power;
    egg_metadata.last_update_ts = current_time;
    egg_metadata.total_hashpower_accumulated = egg_metadata.total_hashpower_accumulated
        .saturating_add(total_hashpower.saturating_div(total_eggs as u64));
    
    incubation_state.total_power = incubation_state.total_power
        .saturating_add(power_increase as u64);
    incubation_state.last_update_ts = current_time;
    
    emit!(DragonEggPowerUpdated {
        egg_mint: egg_metadata.mint,
        old_power,
        new_power,
        power_increase,
        hashpower_accumulated: total_hashpower,
    });
    
    msg!("✅ Dragon Egg power updated");
    msg!("   Power: {} -> {} (+{})", old_power, new_power, power_increase);

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




#[derive(Accounts)]
pub struct IncubateDragonEgg<'info> {
    /// Metaplex Core asset (source of truth for ownership)
    /// CHECK: Verified via verify_nft_ownership helper
    pub dragon_egg_asset: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ NftLaunchpadError::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = IncubationState::LEN,
        seeds = [INCUBATION_STATE_SEED, user.key().as_ref()],
        bump
    )]
    pub incubation_state: Account<'info, IncubationState>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveDragonEgg<'info> {
    /// Metaplex Core asset (source of truth for ownership)
    /// CHECK: Verified via verify_nft_ownership helper
    pub dragon_egg_asset: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ NftLaunchpadError::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    
    #[account(
        mut,
        seeds = [INCUBATION_STATE_SEED, user.key().as_ref()],
        bump = incubation_state.bump,
    )]
    pub incubation_state: Account<'info, IncubationState>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDragonEggPower<'info> {
    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    
    #[account(
        mut,
        seeds = [INCUBATION_STATE_SEED, user.key().as_ref()],
        bump = incubation_state.bump,
    )]
    pub incubation_state: Account<'info, IncubationState>,
    
    /// CHECK: User wallet (used for PDA derivation)
    pub user: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}
