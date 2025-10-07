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
    let global_config = &mut ctx.accounts.global_config;
    
    // Check if program is paused
    require!(!global_config.is_paused, NftLaunchpadError::ProgramPaused);
    
    // Determine what NFTs to mint
    let (mint_doge, mint_egg) = determine_pricing_tier(pricing_tier)?;
    
    let mut moondoge_mint: Option<Pubkey> = None;
    let mut dragon_egg_mint: Option<Pubkey> = None;
    
    // Mint MoonDoge if tier includes it
    if mint_doge {
        require!(
            global_config.total_moondoges_minted < MAX_MOONDOGE_SUPPLY,
            NftLaunchpadError::MaxMoonDogeSupplyReached
        );
        
        let index = global_config.total_moondoges_minted;
        let name = generate_moondoge_name(index);
        let uri = generate_moondoge_uri(index);
        
        // In production: Create Metaplex Core asset via CPI
        // For now, we'll track in metadata account
        
        let doge_metadata = &mut ctx.accounts.moondoge_metadata.as_mut().unwrap();
        doge_metadata.mint = ctx.accounts.moondoge_mint.as_ref().unwrap().key();
        doge_metadata.owner = ctx.accounts.user.key();
        doge_metadata.money = BASE_DOGE_MONEY;
        doge_metadata.attached_moonbase = None;
        doge_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        doge_metadata.total_mdoge_mined = 0;
        doge_metadata.created_at = Clock::get()?.unix_timestamp;
        doge_metadata.bump = ctx.bumps.moondoge_metadata.unwrap();
        
        global_config.total_moondoges_minted += 1;
        moondoge_mint = Some(doge_metadata.mint);
        
        emit!(MoonDogeMinted {
            mint: doge_metadata.mint,
            owner: doge_metadata.owner,
            name,
            uri,
            price_paid: 0, // Included in moonbase price
        });
    }
    
    // Mint Dragon Egg if tier includes it
    if mint_egg {
        let index = global_config.total_dragon_eggs_minted;
        let name = generate_dragon_egg_name(index);
        let dna = crate::state::generate_dragon_egg_dna(
            Clock::get()?.slot,
            &ctx.accounts.user.key(),
            index,
        );
        let uri = generate_dragon_egg_uri(index, &dna);
        
        let egg_metadata = &mut ctx.accounts.dragon_egg_metadata.as_mut().unwrap();
        egg_metadata.mint = ctx.accounts.dragon_egg_mint.as_ref().unwrap().key();
        egg_metadata.owner = ctx.accounts.user.key();
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
            owner: egg_metadata.owner,
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
// ========================== INDIVIDUAL NFT PURCHASES ===================================
// ========================================================================================

/// Purchase a MoonDoge NFT (0.5 SOL)
pub fn purchase_moondoge_handler(
    ctx: Context<PurchaseMoonDoge>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    require!(!global_config.is_paused, NftLaunchpadError::ProgramPaused);
    require!(
        global_config.total_moondoges_minted < MAX_MOONDOGE_SUPPLY,
        NftLaunchpadError::MaxMoonDogeSupplyReached
    );
    
    // Transfer SOL to treasury
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        MOONDOGE_PRICE,
    )?;
    
    let index = global_config.total_moondoges_minted;
    let name = generate_moondoge_name(index);
    let uri = generate_moondoge_uri(index);
    
    // Initialize metadata
    let doge_metadata = &mut ctx.accounts.moondoge_metadata;
    doge_metadata.mint = ctx.accounts.moondoge_mint.key();
    doge_metadata.owner = ctx.accounts.user.key();
    doge_metadata.money = BASE_DOGE_MONEY;
    doge_metadata.attached_moonbase = None;
    doge_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    doge_metadata.total_mdoge_mined = 0;
    doge_metadata.created_at = Clock::get()?.unix_timestamp;
    doge_metadata.bump = ctx.bumps.moondoge_metadata;
    
    global_config.total_moondoges_minted += 1;
    global_config.total_sol_collected += MOONDOGE_PRICE;
    
    emit!(MoonDogeMinted {
        mint: doge_metadata.mint,
        owner: doge_metadata.owner,
        name,
        uri,
        price_paid: MOONDOGE_PRICE,
    });
    
    emit!(SOLFeesCollected {
        source: "nft_purchase".to_string(),
        amount: MOONDOGE_PRICE,
        pricing_tier: None,
    });
    
    msg!("✅ MoonDoge purchased");
    msg!("   Mint: {}", doge_metadata.mint);
    msg!("   Price: {} SOL", MOONDOGE_PRICE as f64 / LAMPORTS_PER_SOL as f64);
    
    Ok(())
}

/// Purchase a Dragon Egg NFT (0.5 SOL)
pub fn purchase_dragon_egg_handler(
    ctx: Context<PurchaseDragonEgg>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    require!(!global_config.is_paused, NftLaunchpadError::ProgramPaused);
    
    // Transfer SOL to treasury
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        DRAGON_EGG_PRICE,
    )?;
    
    let index = global_config.total_dragon_eggs_minted;
    let name = generate_dragon_egg_name(index);
    let dna = crate::state::generate_dragon_egg_dna(
        Clock::get()?.slot,
        &ctx.accounts.user.key(),
        index,
    );
    let uri = generate_dragon_egg_uri(index, &dna);
    
    // Initialize metadata
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    egg_metadata.mint = ctx.accounts.dragon_egg_mint.key();
    egg_metadata.owner = ctx.accounts.user.key();
    egg_metadata.power = BASE_EGG_POWER;
    egg_metadata.dna = dna;
    egg_metadata.incubated_moonbase = None;
    egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    egg_metadata.total_hashpower_accumulated = 0;
    egg_metadata.created_at = Clock::get()?.unix_timestamp;
    egg_metadata.bump = ctx.bumps.dragon_egg_metadata;
    
    global_config.total_dragon_eggs_minted += 1;
    global_config.total_sol_collected += DRAGON_EGG_PRICE;
    
    emit!(DragonEggMinted {
        mint: egg_metadata.mint,
        owner: egg_metadata.owner,
        name,
        uri,
        dna,
        initial_power: BASE_EGG_POWER,
        price_paid: DRAGON_EGG_PRICE,
    });
    
    emit!(SOLFeesCollected {
        source: "nft_purchase".to_string(),
        amount: DRAGON_EGG_PRICE,
        pricing_tier: None,
    });
    
    msg!("✅ Dragon Egg purchased");
    msg!("   Mint: {}", egg_metadata.mint);
    msg!("   Price: {} SOL", DRAGON_EGG_PRICE as f64 / LAMPORTS_PER_SOL as f64);
    
    Ok(())
}

// ========================================================================================
// ========================== MOONDOGE ATTACHMENT ========================================
// ========================================================================================

/// Attach MoonDoge to moonbase (1 per moonbase max)
pub fn attach_moondoge_handler(
    ctx: Context<AttachMoonDoge>,
) -> Result<()> {
    let doge_metadata = &mut ctx.accounts.moondoge_metadata;
    let doge_attachment = &mut ctx.accounts.doge_attachment;
    
    // Validation
    require!(
        doge_metadata.owner == ctx.accounts.user.key(),
        NftLaunchpadError::NftNotOwnedByUser
    );
    require!(
        doge_metadata.attached_moonbase.is_none(),
        NftLaunchpadError::DogeAlreadyAttached
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    
    // Initialize attachment state
    doge_attachment.moonbase_owner = ctx.accounts.user.key();
    doge_attachment.doge_mint = doge_metadata.mint;
    doge_attachment.last_update_ts = current_time;
    doge_attachment.last_mdoge_balance = 0; // Will be updated on first update call
    doge_attachment.bump = ctx.bumps.doge_attachment;
    
    // Update metadata
    doge_metadata.attached_moonbase = Some(ctx.accounts.user.key());
    doge_metadata.last_update_ts = current_time;
    
    emit!(MoonDogeAttached {
        doge_mint: doge_metadata.mint,
        moonbase_owner: ctx.accounts.user.key(),
        attached_at: current_time,
    });
    
    msg!("✅ MoonDoge attached to moonbase");
    msg!("   Doge: {}", doge_metadata.mint);
    msg!("   Moonbase: {}", ctx.accounts.user.key());
    
    Ok(())
}

/// Detach MoonDoge from moonbase
pub fn detach_moondoge_handler(
    ctx: Context<DetachMoonDoge>,
) -> Result<()> {
    let doge_metadata = &mut ctx.accounts.moondoge_metadata;
    
    require!(
        doge_metadata.owner == ctx.accounts.user.key(),
        NftLaunchpadError::NftNotOwnedByUser
    );
    require!(
        doge_metadata.attached_moonbase.is_some(),
        NftLaunchpadError::DogeNotAttached
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    let final_money = doge_metadata.money;
    
    // Update metadata
    doge_metadata.attached_moonbase = None;
    doge_metadata.last_update_ts = current_time;
    
    emit!(MoonDogeDetached {
        doge_mint: doge_metadata.mint,
        moonbase_owner: ctx.accounts.user.key(),
        detached_at: current_time,
        final_money,
    });
    
    msg!("✅ MoonDoge detached from moonbase");
    msg!("   Doge: {}", doge_metadata.mint);
    msg!("   Final Money: {}", final_money);
    
    Ok(())
}

/// Update MoonDoge money based on mDOGE mined
pub fn update_moondoge_money_handler(
    ctx: Context<UpdateMoonDogeMoney>,
    mdoge_mined: u64,
) -> Result<()> {
    let doge_metadata = &mut ctx.accounts.moondoge_metadata;
    let doge_attachment = &mut ctx.accounts.doge_attachment;
    
    require!(
        doge_metadata.attached_moonbase.is_some(),
        NftLaunchpadError::DogeNotAttached
    );
    
    let old_money = doge_metadata.money;
    let money_increase = calculate_doge_money_increase(mdoge_mined)?;
    let new_money = old_money.saturating_add(money_increase).min(MAX_DOGE_MONEY);
    
    doge_metadata.money = new_money;
    doge_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    doge_metadata.total_mdoge_mined = doge_metadata.total_mdoge_mined.saturating_add(mdoge_mined);
    
    doge_attachment.last_update_ts = Clock::get()?.unix_timestamp;
    doge_attachment.last_mdoge_balance = doge_attachment.last_mdoge_balance.saturating_add(mdoge_mined);
    
    emit!(MoonDogeMoneyUpdated {
        doge_mint: doge_metadata.mint,
        owner: doge_metadata.owner,
        old_money,
        new_money,
        money_increase,
        mdoge_mined,
    });
    
    msg!("✅ MoonDoge money updated");
    msg!("   Money: {} -> {} (+{})", old_money, new_money, money_increase);
    
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
    
    require!(
        egg_metadata.owner == ctx.accounts.user.key(),
        NftLaunchpadError::NftNotOwnedByUser
    );
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
    
    require!(
        egg_metadata.owner == ctx.accounts.user.key(),
        NftLaunchpadError::NftNotOwnedByUser
    );
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
        owner: egg_metadata.owner,
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
    
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump = global_config.treasury_bump,
    )]
    /// CHECK: SOL treasury PDA
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: MoonDoge mint (if applicable)
    #[account(mut)]
    pub moondoge_mint: Option<UncheckedAccount<'info>>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = MoonDogeMetadata::LEN,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_mint.as_ref().unwrap().key().as_ref()],
        bump
    )]
    pub moondoge_metadata: Option<Account<'info, MoonDogeMetadata>>,
    
    /// CHECK: Dragon Egg mint (if applicable)
    #[account(mut)]
    pub dragon_egg_mint: Option<UncheckedAccount<'info>>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_mint.as_ref().unwrap().key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Option<Account<'info, DragonEggMetadata>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseMoonDoge<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump = global_config.treasury_bump,
    )]
    /// CHECK: SOL treasury PDA
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: MoonDoge mint (Metaplex Core asset)
    #[account(mut)]
    pub moondoge_mint: UncheckedAccount<'info>,
    
    #[account(
        init,
        payer = user,
        space = MoonDogeMetadata::LEN,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_mint.key().as_ref()],
        bump
    )]
    pub moondoge_metadata: Account<'info, MoonDogeMetadata>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchaseDragonEgg<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump = global_config.treasury_bump,
    )]
    /// CHECK: SOL treasury PDA
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: Dragon Egg mint (Metaplex Core asset)
    #[account(mut)]
    pub dragon_egg_mint: UncheckedAccount<'info>,
    
    #[account(
        init,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_mint.key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AttachMoonDoge<'info> {
    #[account(
        mut,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_metadata.mint.as_ref()],
        bump = moondoge_metadata.bump,
    )]
    pub moondoge_metadata: Account<'info, MoonDogeMetadata>,
    
    #[account(
        init,
        payer = user,
        space = DogeAttachment::LEN,
        seeds = [DOGE_ATTACHMENT_SEED, user.key().as_ref()],
        bump
    )]
    pub doge_attachment: Account<'info, DogeAttachment>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DetachMoonDoge<'info> {
    #[account(
        mut,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_metadata.mint.as_ref()],
        bump = moondoge_metadata.bump,
    )]
    pub moondoge_metadata: Account<'info, MoonDogeMetadata>,
    
    #[account(
        mut,
        seeds = [DOGE_ATTACHMENT_SEED, user.key().as_ref()],
        bump = doge_attachment.bump,
        close = user
    )]
    pub doge_attachment: Account<'info, DogeAttachment>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateMoonDogeMoney<'info> {
    #[account(
        mut,
        seeds = [MOONDOGE_METADATA_SEED, moondoge_metadata.mint.as_ref()],
        bump = moondoge_metadata.bump,
    )]
    pub moondoge_metadata: Account<'info, MoonDogeMetadata>,
    
    #[account(
        mut,
        seeds = [DOGE_ATTACHMENT_SEED, user.key().as_ref()],
        bump = doge_attachment.bump,
    )]
    pub doge_attachment: Account<'info, DogeAttachment>,
    
    /// CHECK: User wallet (used for PDA derivation)
    pub user: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct IncubateDragonEgg<'info> {
    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED, dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
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
