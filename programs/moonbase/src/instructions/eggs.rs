use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use crate::instructions::helper;
use crate::instructions::admin::UpdateEggsConfig;
use mpl_core::{
    instructions::{
        AddCollectionPluginV1CpiBuilder,
    },
    types::{Plugin, PluginAuthority, Royalties, RuleSet, Creator},
};

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

/// Simulate mint costs for multiple eggs accounting for bonding curve pricing
/// Returns (total_price, individual_prices)
pub fn simulate_mint_cost(
    egg_config: &EggConfig,
    mint_count: u64,
) -> Result<(u64, Vec<u64>)> {
    require!(mint_count > 0, ErrorCode::InvalidParameters);
    require!(mint_count <= 10, ErrorCode::InvalidParameters); // Max 10 per batch
    require!(
        egg_config.eggs_minted + mint_count <= egg_config.max_supply,
        ErrorCode::InvalidParameters
    );

    let mut prices = Vec::new();
    let mut total_price = 0u64;
    let mut current_minted = egg_config.eggs_minted;

    for _ in 0..mint_count {
        let actual_price = crate::genescience::compute_gene_price(
            egg_config.base_price,
            egg_config.curve_a,
            current_minted,
        )?;
        
        prices.push(actual_price);
        total_price = total_price.checked_add(actual_price)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        current_minted += 1;
    }

    Ok((total_price, prices))
}

/// Mint a single Dragon Egg NFT
/// Uses bonding curve pricing based on current supply
/// Users can choose a ticket tier to receive free tickets
pub fn mint_dragon_egg(
    ctx: Context<MintDragonEgg>,
    faction_id: u8,
    ticket_tier_index: Option<u8>,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let egg_config = &mut ctx.accounts.egg_config;
    let player_data = &mut ctx.accounts.player_data;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Check supply limit
    require!(
        egg_config.eggs_minted < egg_config.max_supply,
        ErrorCode::InvalidParameters
    );
    
    // Calculate cost using bonding curve
    let cost_per_egg = crate::genescience::compute_gene_price(
        egg_config.base_price,
        egg_config.curve_a,
        egg_config.eggs_minted,
    )?;
    
    msg!("   Minting egg #{} at price: {} lamports", 
        egg_config.eggs_minted + 1, 
        cost_per_egg);

    // Transfer SOL from user to treasury
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        cost_per_egg,
    )?;
    
    // Handle ticket tier selection and add free tickets
    if let Some(tier_index) = ticket_tier_index {
        require!(
            (tier_index as usize) < egg_config.ticket_tiers.len(),
            ErrorCode::InvalidParameters
        );
        
        let selected_tier = &egg_config.ticket_tiers[tier_index as usize];
        let ticket_value = selected_tier.ticket_value;
        let ticket_count = selected_tier.ticket_count;
        
        msg!("   Selected ticket tier: {} tickets of {} SOL each", 
            ticket_count, 
            ticket_value as f64 / 1e9);
        
        // Add free tickets to player
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
    }
    
    // Use slot and user key as seed for egg count
    let clock = Clock::get()?;
    let slot = clock.slot;
    let user_key = ctx.accounts.user.key();
    
    // Generate DNA (no tier, use faction as family type)
    let family_type = faction_id.min(15); // Max 15 for family type
    let dna = crate::genescience::generate_genesis_dna(
        egg_config.eggs_minted + 1,
        &user_key,
        slot,
        family_type,
    )?;

    // Get URI for this faction
    let uri = if (faction_id as usize) < egg_config.dragon_egg_uris.len() {
        egg_config.dragon_egg_uris[faction_id as usize].clone()
    } else {
        format!("https://arweave.net/dragonegg/{}", faction_id)
    };

    let name = format!("Dragon Egg #{}", egg_config.eggs_minted + 1);
    
    // Fixed multiplier (no tiers)
    let multiplier = 100; // 1.0x base multiplier
    
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
    msg!("   Total eggs minted: {} / {}", egg_config.eggs_minted, egg_config.max_supply);

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
    
    msg!("✅ Minted Dragon Egg #{} for faction {}", egg_config.eggs_minted, faction_id);
    Ok(())
}

/// Batch mint multiple Dragon Eggs (max 10 per transaction)
/// Uses bonding curve pricing for each egg
pub fn batch_mint_dragon_eggs(
    ctx: Context<BatchMintDragonEggs>,
    faction_id: u8,
    mint_count: u8,
) -> Result<()> {
    require!(mint_count > 0 && mint_count <= 10, ErrorCode::InvalidParameters);
    
    let global_config = &ctx.accounts.global_config;
    let egg_config = &mut ctx.accounts.egg_config;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Check supply limit
    require!(
        egg_config.eggs_minted + mint_count as u64 <= egg_config.max_supply,
        ErrorCode::InvalidParameters
    );
    
    // Calculate total cost using bonding curve
    let (total_price, _prices) = simulate_mint_cost(egg_config, mint_count as u64)?;
    
    msg!("   Batch minting {} eggs, total cost: {} lamports", mint_count, total_price);

    // Transfer SOL from user to treasury
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        total_price,
    )?;
    
    let clock = Clock::get()?;
    let slot = clock.slot;
    let user_key = ctx.accounts.user.key();
    let family_type = faction_id.min(15);
    
    // Get URI for this faction
    let uri = if (faction_id as usize) < egg_config.dragon_egg_uris.len() {
        egg_config.dragon_egg_uris[faction_id as usize].clone()
    } else {
        format!("https://arweave.net/dragonegg/{}", faction_id)
    };
    
    let multiplier = 100; // Fixed multiplier
    
    // Mint each egg
    for i in 0..mint_count {
        let current_mint_number = egg_config.eggs_minted + 1;
        
        // Generate DNA
        let dna = crate::genescience::generate_genesis_dna(
            current_mint_number,
            &user_key,
            slot + i as u64,
            family_type,
        )?;
        
        let name = format!("Dragon Egg #{}", current_mint_number);
        
        // Note: In a real implementation, you'd need to create multiple NFT assets
        // For now, we'll just update the metadata account and emit events
        // The actual NFT creation would need to be done via CPI for each egg
        
        emit!(DragonEggMinted {
            egg_metadata_account: ctx.accounts.dragon_egg_metadata.key(),
            dragon_egg_asset_signer: ctx.accounts.dragon_egg_asset.key(),
            owner: ctx.accounts.user.key(),
            mint: ctx.accounts.dragon_egg_asset.key(), // In batch, this would be different per egg
            name: name.clone(),
            uri: uri.clone(),
            dna,
            initial_power: 0,
            multiplier,
            faction_id,
        });
        
        egg_config.eggs_minted += 1;
    }
    
    msg!("✅ Batch minted {} Dragon Eggs for faction {}", mint_count, faction_id);
    msg!("   Total eggs minted: {} / {}", egg_config.eggs_minted, egg_config.max_supply);
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
// -------------- TICKET TIER MANAGEMENT (ADMIN) ------------------------------------------
// ----------------------------------------------------------------------------------------

/// Add or update ticket tier configs (admin only)
/// Max 4 ticket tier configs can be set
pub fn add_ticket_tier_config(
    ctx: Context<UpdateEggsConfig>,
    ticket_tier_index: u8,
    ticket_value: u64,
    ticket_count: u16,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let eggs_config = &mut ctx.accounts.eggs_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    require!(
        ticket_tier_index < EggConfig::MAX_TICKET_TIERS as u8,
        ErrorCode::InvalidParameters
    );

    let tier_index = ticket_tier_index as usize;

    // Ensure vector is large enough
    while eggs_config.ticket_tiers.len() <= tier_index {
        eggs_config.ticket_tiers.push(TicketTier {
            ticket_value: 0,
            ticket_count: 0,
        });
    }

    // Update or add ticket tier
    eggs_config.ticket_tiers[tier_index] = TicketTier {
        ticket_value,
        ticket_count,
    };

    msg!("✅ Updated ticket tier config #{}: {} tickets of {} SOL", 
        ticket_tier_index, 
        ticket_count, 
        ticket_value as f64 / 1e9);
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- ADMIN FREE MINT FUNCTION ------------------------------------------------
// ----------------------------------------------------------------------------------------

/// Admin function to mint a Dragon Egg NFT for free to a specified recipient
pub fn admin_mint_dragon_egg(
    ctx: Context<AdminMintDragonEgg>,
    recipient: Pubkey,
    faction_id: u8,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let egg_config = &mut ctx.accounts.egg_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );

    // Check supply limit
    require!(
        egg_config.eggs_minted < egg_config.max_supply,
        ErrorCode::InvalidParameters
    );

    msg!("🎁 [admin_mint_dragon_egg] Admin minting free egg to recipient: {}", recipient);
    msg!("   Faction ID: {}", faction_id);
    msg!("   Egg number: {}", egg_config.eggs_minted + 1);

    // Use slot and recipient key as seed
    let clock = Clock::get()?;
    let slot = clock.slot;
    let recipient_key = recipient;
    
    // Generate DNA (use faction as family type)
    let family_type = faction_id.min(15); // Max 15 for family type
    let dna = crate::genescience::generate_genesis_dna(
        egg_config.eggs_minted + 1,
        &recipient_key,
        slot,
        family_type,
    )?;

    // Get URI for this faction
    let uri = if (faction_id as usize) < egg_config.dragon_egg_uris.len() {
        egg_config.dragon_egg_uris[faction_id as usize].clone()
    } else {
        format!("https://arweave.net/dragonegg/{}", faction_id)
    };

    let name = format!("Dragon Egg #{}", egg_config.eggs_minted + 1);
    
    // Fixed multiplier (no tiers)
    let multiplier = 100; // 1.0x base multiplier
    
    // Get collection authority seeds
    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[
        crate::state::COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];

    // Create NFT via MPL Core CPI (paid by admin, sent to recipient)
    msg!("🎨 Creating Dragon Egg NFT via Metaplex Core CPI");
    msg!("   Name: {}", name);
    msg!("   URI: {}", uri);
    msg!("   Recipient: {}", recipient);

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts
            .dragon_egg_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.collection_authority.to_account_info(),
        &ctx.accounts.authority.to_account_info(), // Payer is admin
        &ctx.accounts.recipient.to_account_info(), // Owner is recipient
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
    msg!("   Total eggs minted: {} / {}", egg_config.eggs_minted, egg_config.max_supply);

    emit!(DragonEggMinted {
        egg_metadata_account: egg_metadata.key(),
        dragon_egg_asset_signer: ctx.accounts.dragon_egg_asset.key(),
        owner: recipient,
        mint: egg_metadata.mint,
        name,
        uri,
        dna,
        initial_power: 0,
        multiplier,
        faction_id,
    });
    
    msg!("✅ Admin minted Dragon Egg #{} for faction {} to recipient {}", 
        egg_config.eggs_minted, faction_id, recipient);
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(faction_id: u8)]
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
#[instruction(faction_id: u8, mint_count: u8)]
pub struct BatchMintDragonEggs<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [EGG_CONFIG_SEED.as_ref()],
        bump = egg_config.bump,
    )]
    pub egg_config: Account<'info, EggConfig>,

    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    /// CHECK: SOL treasury PDA
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: Dragon Egg collection (Metaplex Core)
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Collection authority PDA
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Dragon Egg asset (will be created)
    #[account(mut, signer)]
    pub dragon_egg_asset: UncheckedAccount<'info>,

    #[account(
        init,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_asset.key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(recipient: Pubkey, faction_id: u8)]
pub struct AdminMintDragonEgg<'info> {
    #[account(mut)]
    pub authority: Signer<'info>, // Admin authority

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [EGG_CONFIG_SEED.as_ref()],
        bump = egg_config.bump,
    )]
    pub egg_config: Account<'info, EggConfig>,

    /// CHECK: Recipient account (will receive the NFT)
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,

    /// Metaplex Core asset (will be created)
    #[account(mut)]
    /// CHECK: Will be created via MPL Core CPI
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = authority,
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

    #[account(
        mut,
        seeds = [EGG_CONFIG_SEED.as_ref()],
        bump = eggs_config.bump,
    )]
    pub eggs_config: Account<'info, EggConfig>,

    /// CHECK: Dragon Egg collection (already created via MPL Core)
    #[account(
        mut,
        address = eggs_config.dragon_egg_collection @ ErrorCode::InvalidAccount
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
 