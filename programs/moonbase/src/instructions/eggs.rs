use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use crate::instructions::helper;
use mpl_core::{
    instructions::{
        AddCollectionPluginV1CpiBuilder,
        UpdateCollectionPluginV1CpiBuilder,
    },
    types::{Plugin, PluginAuthority, PluginType, Royalties, RuleSet, Creator},
    RoyaltiesPlugin,
};
use borsh::BorshDeserialize;

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

/// Helper type for passing creators from client
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreatorInput {
    pub address: Pubkey,
    /// Percentage share (0–100). Sum must be exactly 100.
    pub percentage: u8,
}

/// Mint Dragon Egg NFT
/// Allows users to mint an egg with specified faction and tier
pub fn mint_dragon_egg(
    ctx: Context<MintDragonEgg>,
    faction_id: u8,
    tier: u8, // 1, 2, 3, or 4
) -> Result<()> {
    require!(tier >= 1 && tier <= 4, ErrorCode::InvalidParameters);
    
    let global_config = &mut ctx.accounts.global_config;
    let egg_config = &mut ctx.accounts.egg_config;
    let player_data = &mut ctx.accounts.player_data;
    
    // Derive ticket tier index from tier (tier 1 -> index 0, tier 2 -> index 1, etc.)
    let ticket_tier_index = (tier - 1) as usize;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Validate ticket tier index
    require!(
        ticket_tier_index < egg_config.ticket_tiers.len(),
        ErrorCode::InvalidParameters
    );
    
    // Get selected ticket tier (extract before mutating egg_config)
    let selected_ticket_tier = &egg_config.ticket_tiers[ticket_tier_index];
    let ticket_value = selected_ticket_tier.ticket_value;
    let ticket_count = selected_ticket_tier.ticket_count;
    
    // Calculate cost per egg based on tier from EggConfig
    let cost_per_egg = egg_config.prices[ticket_tier_index];
    msg!("   Selected ticket tier: {} tickets of {} SOL each", 
        selected_ticket_tier.ticket_count, 
        selected_ticket_tier.ticket_value as f64 / 1e9);

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

    // Get URI for this tier and faction
    let uri = global_config
        .get_dragon_egg_uri(tier, faction_id)
        .unwrap_or_else(|_| format!("https://arweave.net/dragonegg/{}/{}", tier, faction_id));

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
        egg_metadata.power = 0;
        egg_metadata.dna = dna;
        egg_metadata.incubated_player_data = None;
    egg_metadata.multiplier = multiplier;
    egg_metadata.faction_id = faction_id;
        egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        egg_metadata.created_at = Clock::get()?.unix_timestamp;
    egg_metadata.bump = ctx.bumps.dragon_egg_metadata;
    
    // Update egg config stats
    egg_config.eggs_minted += 1;
    msg!("   Total eggs minted: {} / {}", egg_config.eggs_minted, egg_config.total_supply);
    
    // Add free tickets to player based on selected tier
    msg!("   Adding free tickets to player...");
    
    // Check if this ticket value already exists in player's free_tickets
    if let Some(index) = player_data.free_tickets.iter().position(|&v| v == ticket_value) {
        // Ticket type exists, increment count
        player_data.free_tickets_remaining[index] += ticket_count as u64;
        msg!("     Updated existing ticket type: {} tickets of {} SOL (total: {})", 
            ticket_count, ticket_value as f64 / 1e9, player_data.free_tickets_remaining[index]);
    } else {
        // New ticket type, add to vectors
        require!(
            player_data.free_tickets.len() < PlayerData::MAX_TICKET_TYPES,
            ErrorCode::InvalidParameters
        );
        player_data.free_tickets.push(ticket_value);
        player_data.free_tickets_remaining.push(ticket_count as u64);
        msg!("     Added new ticket type: {} tickets of {} SOL", ticket_count, ticket_value as f64 / 1e9);
    }

        emit!(DragonEggMinted {
        egg_metadata_account: egg_metadata.key(),
        dragon_egg_asset_signer: ctx.accounts.dragon_egg_asset.key(),
        owner: ctx.accounts.user.key(),
            mint: egg_metadata.mint,
            name,
            uri,
            dna,
            initial_power: 0,
        multiplier,
        faction_id,
    });
    
    msg!("✅ Minted Dragon Egg #{} for faction {} (Tier {})", egg_seed, faction_id, tier);
    Ok(())
}
 
/// Stake a Dragon Egg to boost hashpower (multiplier applies to staked dbtc and LP)
/// Users can stake up to 5 eggs, each additional egg increases multiplier by 0.5x
/// Multipliers: 1 egg = 1.5x, 2 eggs = 2.0x, 3 eggs = 2.5x, 4 eggs = 3.0x, 5 eggs = 3.5x
pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    
    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );

    // Validation
    require!(
        egg_metadata.incubated_player_data.is_none(),
        ErrorCode::EggAlreadyIncubated
    );
    
    // Check if egg faction matches player faction (required for boosting)
    require!(
        egg_metadata.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );
    
    // Check user hasn't reached max staked eggs
    require!(
        player_data.staked_eggs.len() < MAX_STAKED_EGGS,
        ErrorCode::InvalidParameters
    );
    
    let current_time = Clock::get()?.unix_timestamp;
    let egg_mint = egg_metadata.mint;
    
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
    
    // Add egg to player's staked eggs list
    player_data.staked_eggs.push(egg_mint);
    let num_staked_eggs = player_data.staked_eggs.len();
    msg!("   Added egg to staked eggs. Total staked: {}", num_staked_eggs);
    
    // Calculate new multiplier based on number of staked eggs
    // 1 egg = 150 (1.5x), 2 eggs = 200 (2.0x), 3 eggs = 250 (2.5x), 4 eggs = 300 (3.0x), 5 eggs = 350 (3.5x)
    let old_multiplier = player_data.egg_multiplier;
    let new_multiplier = 100 + (num_staked_eggs as u16 * 50); // Base 100 + 50 per egg
    player_data.egg_multiplier = new_multiplier;
    msg!("⚡ Updated egg multiplier: {} ({}x) -> {} ({}x)", 
        old_multiplier, old_multiplier as f64 / 100.0,
        new_multiplier, new_multiplier as f64 / 100.0);
    
    // Recalculate total hashpower with new multiplier
    // hashpower = (dogebtc_staked * lockup_multiplier / 100) * egg_multiplier / 100
    // Since positions already have weighted_amount, we need to recalculate player totals
    let old_dbtc_hashpower = player_data.dogebtc_hashpower;
    let old_lp_hashpower = player_data.lp_hashpower;
    
    // Apply multiplier difference
    let multiplier_increase = new_multiplier - old_multiplier;
    if multiplier_increase > 0 && (old_dbtc_hashpower > 0 || old_lp_hashpower > 0) {
        let dbtc_boost = old_dbtc_hashpower * multiplier_increase as u64 / 100;
        let lp_boost = old_lp_hashpower * multiplier_increase as u64 / 100;
        
        player_data.dogebtc_hashpower += dbtc_boost;
        player_data.lp_hashpower += lp_boost;
        
        faction_state.total_dbtc_hashpower += dbtc_boost;
        faction_state.total_lp_hashpower += lp_boost;
        
        msg!("   DogeBtc hashpower boost: {} (+{})", player_data.dogebtc_hashpower, dbtc_boost);
        msg!("   LP hashpower boost: {} (+{})", player_data.lp_hashpower, lp_boost);
        msg!("   Faction dbtc hashpower: {}", faction_state.total_dbtc_hashpower);
        msg!("   Faction LP hashpower: {}", faction_state.total_lp_hashpower);
    }
    
    // Update egg metadata
    egg_metadata.incubated_player_data = Some(player_data.owner);
    egg_metadata.last_update_ts = current_time;
    
    msg!("✅ Dragon Egg staked for player {}", player_data.owner);
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Faction: {}", egg_metadata.faction_id);
    msg!("   Total eggs staked: {}/{}", num_staked_eggs, MAX_STAKED_EGGS);
    
    Ok(())
}

/// Unstake a Dragon Egg (reduces multiplier and recalculates hashpower)
pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let egg_mint = egg_metadata.mint;
    
    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(
        nft_owner == ctx.accounts.egg_custody_pda.key(),
        ErrorCode::EggNotIncubated
    );
    
    require!(
        egg_metadata.incubated_player_data.is_some(),
        ErrorCode::EggNotIncubated
    );
    
    require!(
        egg_metadata.incubated_player_data.unwrap() == player_data.owner,
        ErrorCode::Unauthorized
    );
        
    let current_time = Clock::get()?.unix_timestamp;
    
    // Remove egg from player's staked eggs list
    if let Some(index) = player_data.staked_eggs.iter().position(|&mint| mint == egg_mint) {
        player_data.staked_eggs.remove(index);
        msg!("   Removed egg from staked eggs. Remaining: {}", player_data.staked_eggs.len());
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }
    
    let num_staked_eggs = player_data.staked_eggs.len();
    
    // Calculate new multiplier based on remaining staked eggs
    let old_multiplier = player_data.egg_multiplier;
    let new_multiplier = 100 + (num_staked_eggs as u16 * 50); // Base 100 + 50 per egg
    player_data.egg_multiplier = new_multiplier;
    msg!("⚡ Updated egg multiplier: {} ({}x) -> {} ({}x)", 
        old_multiplier, old_multiplier as f64 / 100.0,
        new_multiplier, new_multiplier as f64 / 100.0);
    
    // Recalculate total hashpower with new multiplier
    let old_dbtc_hashpower = player_data.dogebtc_hashpower;
    let old_lp_hashpower = player_data.lp_hashpower;
    
    // Apply multiplier decrease
    let multiplier_decrease = old_multiplier - new_multiplier;
    if multiplier_decrease > 0 && (old_dbtc_hashpower > 0 || old_lp_hashpower > 0) {
        let dbtc_reduction = old_dbtc_hashpower * multiplier_decrease as u64 / 100;
        let lp_reduction = old_lp_hashpower * multiplier_decrease as u64 / 100;
        
        player_data.dogebtc_hashpower -= dbtc_reduction;
        player_data.lp_hashpower -= lp_reduction;
        
        faction_state.total_dbtc_hashpower -= dbtc_reduction;
        faction_state.total_lp_hashpower -= lp_reduction;
        
        msg!("   DogeBtc hashpower reduction: {} (-{})", player_data.dogebtc_hashpower, dbtc_reduction);
        msg!("   LP hashpower reduction: {} (-{})", player_data.lp_hashpower, lp_reduction);
        msg!("   Faction dbtc hashpower: {}", faction_state.total_dbtc_hashpower);
        msg!("   Faction LP hashpower: {}", faction_state.total_lp_hashpower);
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
    egg_metadata.incubated_player_data = None;
    egg_metadata.last_update_ts = current_time;
    
    msg!("✅ Dragon Egg unstaked");
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Remaining eggs staked: {}/{}", num_staked_eggs, MAX_STAKED_EGGS);
    
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- ROYALTY MANAGEMENT (ADMIN) ---------------------------------------------
// ----------------------------------------------------------------------------------------

/// Initialize royalties on the Dragon Egg collection.
/// - basis_points: 500 = 5%
/// - creators: list of (address, percentage) where sum(percentage) == 100
pub fn init_dragon_egg_royalties(
    ctx: Context<InitDragonEggRoyalties>,
    basis_points: u16,
    creators: Vec<CreatorInput>,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    // Basic creator validation
    require!(!creators.is_empty(), ErrorCode::NoCreators);
    let total_pct: u16 = creators.iter().map(|c| c.percentage as u16).sum();
    require!(total_pct == 100, ErrorCode::InvalidCreatorShare);

    // Convert to mpl-core creators
    let creators_mpl: Vec<Creator> = creators
        .into_iter()
        .map(|c| Creator {
            address: c.address,
            percentage: c.percentage,
        })
        .collect();

    // Royalties plugin data
    let royalties = Royalties {
        basis_points,
        creators: creators_mpl,
        // Start with an EMPTY ProgramDenyList so you can add later.
        rule_set: RuleSet::ProgramDenyList(vec![]),
    };

    // PDA signer for collection authority (same PDA you used as update_authority)
    let bump = ctx.bumps.collection_authority;
    let seeds: &[&[u8]] = &[COLLECTION_AUTHORITY_SEED, &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[&seeds];

    let mpl_core_program = &ctx.accounts.mpl_core_program.to_account_info();
    let mut cpi = AddCollectionPluginV1CpiBuilder::new(mpl_core_program);

    cpi.collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.authority.to_account_info())
        // The authority that initializes the plugin is the collection update authority PDA.
        .authority(Some(&ctx.accounts.collection_authority.to_account_info()))
        .plugin(Plugin::Royalties(royalties))
        // Plugin authority is "UpdateAuthority", i.e. the collection update authority PDA.
        .init_authority(PluginAuthority::UpdateAuthority)
        .system_program(&ctx.accounts.system_program.to_account_info())
        // No log_wrapper needed; pass no extra accounts.
        .invoke_signed(signer_seeds)?;

    msg!("✅ Initialized Dragon Egg royalties: {} basis points", basis_points);
    Ok(())
}

 

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(faction_id: u8, tier: u8, ticket_tier_index: u8)]
pub struct MintDragonEgg<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [EGG_CONFIG_SEED.as_ref()],
        bump = egg_config.bump
    )]
    pub egg_config: Account<'info, EggConfig>,
    
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

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

#[derive(Accounts)]
pub struct InitDragonEggRoyalties<'info> {
    #[account(mut)]
    pub authority: Signer<'info>, // ext authority EOA

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Dragon Egg collection (already created via MPL Core)
    #[account(
        mut,
        address = global_config.dragon_egg_collection @ ErrorCode::InvalidAccount
    )]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that is update_authority for the collection
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
 