use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

/// Mint Dragon Egg NFT
/// Allows users to mint an egg with specified faction and tier
pub fn mint_dragon_egg(
    ctx: Context<MintDragonEgg>,
    faction_id: u8,
    tier: u8, // 2, 3, or 4
) -> Result<()> {
    require!(tier >= 2 && tier <= 4, ErrorCode::InvalidParameters);
    
    let global_config = &mut ctx.accounts.global_config;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Calculate cost per egg based on tier
    let cost_per_egg = match tier {
        2 => PRICE_TIER_2,
        3 => PRICE_TIER_3,
        4 => PRICE_TIER_4,
        _ => return Err(ErrorCode::InvalidParameters.into()),
    };

    // Transfer SOL from user to treasury
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        cost_per_egg,
    )?;
    
    // Use slot and user key as seed for egg count (no global counter needed)
    let clock = Clock::get()?;
    let slot = clock.slot;
    let user_key = ctx.accounts.user.key();
    let egg_seed = slot.wrapping_add(u64::from_le_bytes([
        user_key.as_ref()[0], user_key.as_ref()[1], user_key.as_ref()[2], user_key.as_ref()[3],
        user_key.as_ref()[4], user_key.as_ref()[5], user_key.as_ref()[6], user_key.as_ref()[7],
    ]));
    
    let family_type = (tier - 1) as u8; // Tier 2->Family 1, Tier 3->Family 2, Tier 4->Family 3
    
    // Generate DNA
    let dna = crate::genescience::generate_genesis_dna_with_tier(
        egg_seed,
        &user_key,
        slot,
        family_type,
    )?;

    // Get URI
    let uri = global_config
        .get_random_dragon_egg_uri(slot, egg_seed, &dna)
        .unwrap_or_else(|_| format!("https://arweave.net/dragonegg/{}", egg_seed));

    let name = format!("Dragon Egg #{}", egg_seed);
    
    // Calculate multiplier based on tier
    let multiplier = match tier {
        2 => 150, // 1.5x
        3 => 200, // 2.0x
        4 => 300, // 3.0x
        _ => 100,
    };
    
    // Get collection authority seeds
    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[
        crate::state::COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];

    // Create NFT via MPL Core CPI
    msg!("🎨 Creating Dragon Egg NFT via Metaplex Core CPI");
    msg!("   Name: {}", name);
    msg!("   URI: {}", uri);

        crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts
            .dragon_egg_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.collection_authority.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
            name.clone(),
            uri.clone(),
        Some(&[collection_authority_seeds]),
    )?;

    // Initialize Dragon Egg metadata
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    egg_metadata.mint = ctx.accounts.dragon_egg_asset.key();
        egg_metadata.power = BASE_EGG_POWER;
        egg_metadata.dna = dna;
        egg_metadata.incubated_moonbase = None;
    egg_metadata.multiplier = multiplier;
    egg_metadata.faction_id = faction_id;
        egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        egg_metadata.created_at = Clock::get()?.unix_timestamp;
    egg_metadata.bump = ctx.bumps.dragon_egg_metadata;

    // Update global dragon egg power
    global_config.global_dragon_egg_power = global_config
        .global_dragon_egg_power
        .saturating_add(BASE_EGG_POWER as u64);

        emit!(DragonEggMinted {
        egg_metadata_account: egg_metadata.key(),
        dragon_egg_asset_signer: ctx.accounts.dragon_egg_asset.key(),
        owner: ctx.accounts.user.key(),
            mint: egg_metadata.mint,
            name,
            uri,
            dna,
            initial_power: BASE_EGG_POWER,
        multiplier,
        faction_id,
    });
    
    msg!("✅ Minted Dragon Egg #{} for faction {} (Tier {})", egg_seed, faction_id, tier);
    Ok(())
}

/// Stake a Dragon Egg to boost hashpower (if faction matches)
/// Eggs belonging to the same faction as the player's passive staking can boost hashpower
pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    
    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );

    // Validation
    require!(
        egg_metadata.incubated_moonbase.is_none(),
        ErrorCode::EggAlreadyIncubated
    );
    
    // Check if egg faction matches player faction (required for boosting)
    require!(
        egg_metadata.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    
    // Transfer NFT to custody PDA (lock it)
    msg!("🔒 Transferring NFT to custody PDA (locking)");
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts
            .dragon_egg_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;
    
    // Calculate hashpower boost
    // Boost = personal_passive_hashpower * (multiplier - 100) / 100
    // Example: 1000 hashpower with 1.5x multiplier = +500 boost
    let base_hashpower = player_data.personal_passive_hashpower;
    let player_owner = player_data.owner; // Store owner before mutable borrow
    let boost_amount = if base_hashpower > 0 && egg_metadata.multiplier > 100 {
        let multiplier_excess = (egg_metadata.multiplier as u128)
            .saturating_sub(100);
        base_hashpower
            .checked_mul(multiplier_excess)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0u128
    };
    
    if boost_amount > 0 {
        msg!("⚡ Applying Dragon Egg multiplier boost");
        msg!("   Base hashpower: {}", base_hashpower);
        msg!("   Egg multiplier: {}x", egg_metadata.multiplier as f64 / 100.0);
        msg!("   Boost amount: {}", boost_amount);
        
        // Update player hashpower
        let player_data_mut = &mut ctx.accounts.player_data;
        player_data_mut.personal_passive_hashpower = player_data_mut
            .personal_passive_hashpower
            .saturating_add(boost_amount);
        
        // Update faction hashpower
        faction_state.total_passive_hashpower = faction_state
            .total_passive_hashpower
            .saturating_add(boost_amount);
        
        msg!("   New hashpower: {}", player_data_mut.personal_passive_hashpower);
        msg!("   Faction total: {}", faction_state.total_passive_hashpower);
    } else {
        msg!("⚠️ No hashpower to boost (stake tokens first)");
    }
    
    // Update egg metadata
    egg_metadata.incubated_moonbase = Some(player_owner);
    egg_metadata.last_update_ts = current_time;
    
    msg!("✅ Dragon Egg staked for player {}", player_owner);
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Faction: {}", egg_metadata.faction_id);
    
    Ok(())
}

/// Unstake a Dragon Egg (remove hashpower boost)
pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    
    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(
        nft_owner == ctx.accounts.egg_custody_pda.key(),
        ErrorCode::EggNotIncubated
    );
    
        require!(
        egg_metadata.incubated_moonbase.is_some(),
        ErrorCode::EggNotIncubated
    );
    
    require!(
        egg_metadata.incubated_moonbase.unwrap() == player_data.owner,
        ErrorCode::Unauthorized
        );
        
        let current_time = Clock::get()?.unix_timestamp;
        
    // Calculate hashpower boost to remove
    // Current hashpower includes the boost, so we need to reverse it
    // original = current / (multiplier / 100)
    // boost = current - original
    let current_hashpower = player_data.personal_passive_hashpower;
    let boost_amount = if current_hashpower > 0 && egg_metadata.multiplier > 100 {
        // Calculate original hashpower before boost
        let original_hashpower = (current_hashpower as u128)
            .checked_mul(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(egg_metadata.multiplier as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        current_hashpower.saturating_sub(original_hashpower)
        } else {
        0u128
    };
    
    if boost_amount > 0 {
        msg!("⚡ Removing Dragon Egg multiplier boost");
        msg!("   Current hashpower: {}", current_hashpower);
        msg!("   Boost to remove: {}", boost_amount);
        
        // Remove boost from player hashpower
        player_data.personal_passive_hashpower = player_data
            .personal_passive_hashpower
            .saturating_sub(boost_amount);
        
        // Remove boost from faction hashpower
        faction_state.total_passive_hashpower = faction_state
            .total_passive_hashpower
            .saturating_sub(boost_amount);
        
        msg!("   New hashpower: {}", player_data.personal_passive_hashpower);
        msg!("   Faction total: {}", faction_state.total_passive_hashpower);
    }
    
    // Transfer NFT back to user (unlock it)
    msg!("🔓 Transferring NFT back to user (unlocking)");
    let custody_seeds = &[DRAGON_EGG_CUSTODY_SEED, &[ctx.bumps.egg_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];
    
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts
            .dragon_egg_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;
    
    // Update egg metadata
    egg_metadata.incubated_moonbase = None;
    egg_metadata.last_update_ts = current_time;
    
    msg!("✅ Dragon Egg unstaked");
    msg!("   Egg: {}", egg_metadata.mint);
    
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(faction_id: u8, tier: u8)]
pub struct MintDragonEgg<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    /// CHECK: PDA that holds collected SOL fees
    pub sol_treasury: UncheckedAccount<'info>,

    /// Metaplex Core asset (will be created)
    #[account(mut)]
    /// CHECK: Will be created via MPL Core CPI
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_asset.key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED],
        bump
    )]
    /// CHECK: PDA authority for the collection
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakeDragonEgg<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (source of truth for ownership)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeDragonEgg<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (currently locked in custody PDA)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}
