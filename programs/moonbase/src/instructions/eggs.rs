use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::associated_token::AssociatedToken;
use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;
use crate::instructions::helper;

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

/// Simulate mint costs for multiple eggs accounting for bonding curve pricing
/// Returns (total_price, individual_prices)
pub fn simulate_mint_cost(
    egg_config: &EggConfig,
    mint_count: u64,
) -> Result<(u64, Vec<u64>)> {
    require!(mint_count > 0 && mint_count <= 10, ErrorCode::InvalidParameters);
    require!(  egg_config.eggs_minted + mint_count <= egg_config.max_supply, ErrorCode::InvalidParameters);

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
pub fn mint_dragon_egg( ctx: Context<MintDragonEgg>, faction_id: u8, ticket_tier_index: Option<u8>) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let egg_config = &mut ctx.accounts.egg_config;
    let player_data = &mut ctx.accounts.player_data;
    
    require!(   (faction_id as usize) < global_config.supported_factions.len(), ErrorCode::InvalidFactionId);
    require!(   egg_config.eggs_minted < egg_config.max_supply, ErrorCode::InvalidParameters);
    
    // Calculate cost using bonding curve
    let cost_per_egg = crate::genescience::compute_gene_price(egg_config.base_price, egg_config.curve_a, egg_config.eggs_minted)?;
    
    msg!("   Minting egg #{} at price: {} lamports", egg_config.eggs_minted + 1, cost_per_egg as f64 / 1e9);
    let treasury_amt = cost_per_egg * 20 / 100;
    let dev_amt =  cost_per_egg - treasury_amt;

    helper::transfer_to_sol_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        treasury_amt,
    )?;

    // Transfer WSOL from user to multisig account
    helper::transfer_wsol_to_multisig(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.multisig_wsol_account.to_account_info(),
        &ctx.accounts.user_wsol_account.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        dev_amt,
    )?;
    
    // Handle ticket tier selection and add free tickets
    if let Some(tier_index) = ticket_tier_index {
        require!(  (tier_index as usize) < egg_config.ticket_tiers.len(), ErrorCode::InvalidParameters );
        
        // Validate tier availability based on remaining eggs
        let eggs_remaining = egg_config.max_supply.saturating_sub(egg_config.eggs_minted);
        
        // Tier 0 and 1: always available
        // Tier 2: only available when eggs remaining < 10,000
        // Tier 3: only available when eggs remaining < 5,000
        match tier_index {
            0 | 1 => {
                // Tiers 0 and 1 are always available
            },
            2 => {
                require!(
                    eggs_remaining < 10_000,
                    ErrorCode::InvalidParameters
                );
                msg!("   Tier 2 unlocked (eggs remaining: {})", eggs_remaining);
            },
            3 => {
                require!(
                    eggs_remaining < 5_000,
                    ErrorCode::InvalidParameters
                );
                msg!("   Tier 3 unlocked (eggs remaining: {})", eggs_remaining);
            },
            _ => {
                return Err(ErrorCode::InvalidParameters.into());
            }
        }
        
        let selected_tier = &egg_config.ticket_tiers[tier_index as usize];
        let ticket_value = selected_tier.ticket_value;
        let ticket_count = selected_tier.ticket_count;
        
        msg!("   Selected ticket tier: {} tickets of {} SOL each", 
            ticket_count, 
            ticket_value as f64 / 1e9);
        
        // Add free tickets to player
        if let Some(index) = player_data.free_tickets.iter().position(|&v| v == ticket_value) {
            player_data.free_tickets_remaining[index] += ticket_count as u64;
            msg!("     Updated existing ticket type: {} tickets of {} SOL (total: {})", ticket_count, ticket_value as f64 / 1e9, player_data.free_tickets_remaining[index]);
        } else {
            require!(  player_data.free_tickets.len() < PlayerData::MAX_TICKET_TYPES, ErrorCode::InvalidParameters );
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
    let family_type = faction_id;
    let dna = crate::genescience::generate_genesis_dna(egg_config.eggs_minted + 1, &user_key, slot, family_type)?;

    // Get URI for this faction
    let uri = egg_config.dragon_egg_uris[faction_id as usize].clone();
    let name = format!("Dragon Egg #{}", egg_config.eggs_minted + 1);
    
    // Calculate progressive multiplier based on current egg count
    let multiplier = crate::genescience::calculate_progressive_multiplier(egg_config.eggs_minted, egg_config.max_supply)?;
    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[crate::state::COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];

    crate::mpl_core_helpers::create_mpl_core_asset(&ctx.accounts.dragon_egg_asset.to_account_info(), ctx.accounts.dragon_egg_collection.as_ref().map(|c| c.to_account_info()).as_ref(),
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
    
    require!(  (faction_id as usize) < global_config.supported_factions.len(), ErrorCode::InvalidFactionId );
    require!(  egg_config.eggs_minted + mint_count as u64 <= egg_config.max_supply, ErrorCode::InvalidParameters );
    
    let (total_price, _prices) = simulate_mint_cost(egg_config, mint_count as u64)?;
    msg!("   Batch minting {} eggs, total cost: {} lamports", mint_count, total_price);

    let dev_amt = total_price * 80 / 100;
    let treasury_amt = total_price - dev_amt;

    helper::transfer_to_sol_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        treasury_amt,
    )?;

    helper::transfer_wsol_to_multisig(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.multisig_wsol_account.to_account_info(),
        &ctx.accounts.user_wsol_account.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        dev_amt,
    )?;
    
    // Mint each egg
    for i in 0..mint_count {
        let current_mint_number = egg_config.eggs_minted + 1;
        
        // Calculate progressive multiplier based on current egg count (before this mint)
        let multiplier = crate::genescience::calculate_progressive_multiplier(egg_config.eggs_minted + i as u64, egg_config.max_supply)?;
        
        // Generate DNA
        let dna = crate::genescience::generate_genesis_dna(  current_mint_number,&ctx.accounts.user.key(),Clock::get()?.slot + i as u64, faction_id)?;        
        let name = format!("Dragon Egg #{}", current_mint_number);
        let uri = egg_config.dragon_egg_uris[faction_id as usize].clone();
        
        let collection_authority_bump = ctx.bumps.collection_authority;
        let collection_authority_seeds = &[crate::state::COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];
    
        crate::mpl_core_helpers::create_mpl_core_asset(&ctx.accounts.dragon_egg_asset.to_account_info(), ctx.accounts.dragon_egg_collection.as_ref().map(|c| c.to_account_info()).as_ref(),
            &ctx.accounts.collection_authority.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            name.clone(),
            uri.clone(),
            Some(&[collection_authority_seeds]),
        )?;
        
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

    // Verify recipient matches instruction parameter
    require!(  ctx.accounts.recipient.key() == recipient, ErrorCode::InvalidAccount);
    require!(  (faction_id as usize) < global_config.supported_factions.len(), ErrorCode::InvalidFactionId);
    require!(  egg_config.eggs_minted < egg_config.max_supply, ErrorCode::InvalidParameters);

    msg!("🎁 [admin_mint_dragon_egg] Admin minting free egg to recipient: {}", recipient);
    msg!("   Faction ID: {}", faction_id);
    msg!("   Egg number: {}", egg_config.eggs_minted + 1);

    // Use slot and recipient key as seed
    let recipient_key = recipient;
    
    // Generate DNA (use faction as family type)    
    let dna = crate::genescience::generate_genesis_dna(  egg_config.eggs_minted + 1,&recipient_key,Clock::get()?.slot, faction_id)?;
    let uri = egg_config.dragon_egg_uris[faction_id as usize].clone();
    let name = format!("Dragon Egg #{}", egg_config.eggs_minted + 1);
    
    // Calculate progressive multiplier based on current egg count
    let multiplier = crate::genescience::calculate_progressive_multiplier(egg_config.eggs_minted, egg_config.max_supply)?;
    
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





/// Stake a Dragon Egg to boost hashpower (multiplier applies to staked dbtc and LP)
/// Users can stake up to 5 eggs, each additional egg increases multiplier by 0.5x
/// Multipliers: 1 egg = 1.5x, 2 eggs = 2.0x, 3 eggs = 2.5x, 4 eggs = 3.0x, 5 eggs = 3.5x
pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {

    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let current_time = Clock::get()?.unix_timestamp;
    let egg_mint = egg_metadata.mint;
    let egg_multiplier = egg_metadata.multiplier;
    
    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;

    require!(nft_owner == ctx.accounts.user.key(), ErrorCode::NftNotOwnedByUser );
    require!( egg_metadata.incubated_player_data.is_none(), ErrorCode::EggAlreadyIncubated);    
    require!( egg_metadata.faction_id == player_data.faction_id && egg_metadata.faction_id == faction_state.faction_id, ErrorCode::InvalidFactionId);
    require!( player_data.staked_eggs.len() < MAX_STAKED_EGGS, ErrorCode::InvalidParameters);
        
    // Transfer NFT to custody PDA (lock it)
    msg!("🔒 Transferring NFT to custody PDA (locking)");
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts.dragon_egg_collection.as_ref().map(|c| c.to_account_info()).as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;
        
    // Process pending rewards before updating position
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = stake::update_dbtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = stake::update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

    // Add egg to player's staked eggs list
    player_data.staked_eggs.push(egg_mint);

    // Calculate new multiplier based on number of staked eggs
    let old_multiplier = player_data.egg_multiplier as u64;
    let new_multiplier = calc_player_multiplier(old_multiplier as u16, egg_multiplier as u16, true) as u64;
    player_data.egg_multiplier = new_multiplier as u16;
    msg!("⚡ Updated egg multiplier: ({})x", player_data.egg_multiplier as f64 / 100.0);    

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_dbtc_hashpower = player_data.dogebtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;
    player_data.dogebtc_hashpower = (existing_dbtc_hashpower / old_multiplier) * new_multiplier;
    player_data.lp_hashpower = (existing_lp_hashpower / old_multiplier) * new_multiplier;
    msg!("   DogeBtc hashpower: {} -> {}", existing_dbtc_hashpower as f64 / 1e6, player_data.dogebtc_hashpower as f64 / 1e6);
    msg!("   LP hashpower: {} -> {}", existing_lp_hashpower as f64 / 1e6, player_data.lp_hashpower as f64 / 1e6);

    // Update faction state totals
    faction_state.total_dbtc_hashpower = faction_state.total_dbtc_hashpower - existing_dbtc_hashpower + player_data.dogebtc_hashpower;
    faction_state.total_lp_hashpower = faction_state.total_lp_hashpower - existing_lp_hashpower + player_data.lp_hashpower;
    msg!("   Faction dbtc hashpower: {} -> {}", faction_state.total_dbtc_hashpower as f64 / 1e6, faction_state.total_dbtc_hashpower as f64 / 1e6);
    msg!("   Faction LP hashpower: {} -> {}", faction_state.total_lp_hashpower as f64 / 1e6, faction_state.total_lp_hashpower as f64 / 1e6);

    faction_state.eggs_staked += 1;
    msg!("   Faction eggs staked: {} ", faction_state.eggs_staked);
        
    // Update egg metadata
    egg_metadata.incubated_player_data = Some(player_data.owner);
    egg_metadata.last_update_ts = current_time;
    msg!("   Egg metadata updated");

    Ok(())
}




/// Unstake a Dragon Egg (reduces multiplier and recalculates hashpower)
pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {

    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let egg_mint = egg_metadata.mint;
    let incubated_by_player = egg_metadata.incubated_player_data.unwrap();
    let current_time = Clock::get()?.unix_timestamp;
    let egg_multiplier = egg_metadata.multiplier;
    
    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!( nft_owner == ctx.accounts.egg_custody_pda.key(), ErrorCode::EggNotIncubated);
    require!( egg_metadata.incubated_player_data.is_some(), ErrorCode::EggNotIncubated);
    require!( egg_metadata.faction_id == player_data.faction_id && egg_metadata.faction_id == faction_state.faction_id, ErrorCode::InvalidFactionId);
    require!( player_data.staked_eggs.contains(&egg_mint), ErrorCode::InvalidParameters);    
    require!( incubated_by_player == player_data.owner,  ErrorCode::Unauthorized);
        
    // Process pending rewards before updating position
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = stake::update_dbtc_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;
    let (new_sol_rewards, new_dbtc_rewards, accrued_dbtc_rewards) = stake::update_lp_staking_rewards(player_data, &mut ctx.accounts.unrefined_rewards, faction_state)?;

    // Remove egg from player's staked eggs list
    if let Some(index) = player_data.staked_eggs.iter().position(|&mint| mint == egg_mint) {
        player_data.staked_eggs.remove(index);
        msg!("   Removed egg from staked eggs. Remaining: {}", player_data.staked_eggs.len());
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // Calculate new multiplier based on number of staked eggs
    let old_multiplier = player_data.egg_multiplier as u64;
    let new_multiplier = calc_player_multiplier(old_multiplier as u16, egg_multiplier as u16, false) as u64;
    player_data.egg_multiplier = new_multiplier as u16;
    msg!("⚡ Updated egg multiplier: ({})x", player_data.egg_multiplier as f64 / 100.0);    

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_dbtc_hashpower = player_data.dogebtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;
    player_data.dogebtc_hashpower = (existing_dbtc_hashpower / old_multiplier) * new_multiplier;
    player_data.lp_hashpower = (existing_lp_hashpower / old_multiplier) * new_multiplier;
    msg!("   DogeBtc hashpower: {} -> {}", existing_dbtc_hashpower as f64 / 1e6, player_data.dogebtc_hashpower as f64 / 1e6);
    msg!("   LP hashpower: {} -> {}", existing_lp_hashpower as f64 / 1e6, player_data.lp_hashpower as f64 / 1e6);

    // Update faction state totals
    faction_state.total_dbtc_hashpower = faction_state.total_dbtc_hashpower - existing_dbtc_hashpower + player_data.dogebtc_hashpower;
    faction_state.total_lp_hashpower = faction_state.total_lp_hashpower - existing_lp_hashpower + player_data.lp_hashpower;
    msg!("   Faction dbtc hashpower: {} -> {}", faction_state.total_dbtc_hashpower as f64 / 1e6, faction_state.total_dbtc_hashpower as f64 / 1e6);
    msg!("   Faction LP hashpower: {} -> {}", faction_state.total_lp_hashpower as f64 / 1e6, faction_state.total_lp_hashpower as f64 / 1e6);

    faction_state.eggs_staked -= 1;
    msg!("   Faction eggs staked: {} -> {}", faction_state.eggs_staked, faction_state.eggs_staked);

    // Update egg metadata
    egg_metadata.incubated_player_data = None;
    egg_metadata.last_update_ts = current_time;
    msg!("   Egg metadata updated");
    
    // Transfer NFT back to user (unlock it)
    msg!("🔓 Transferring NFT back to user (unlocking)");
    let custody_seeds = &[DRAGON_EGG_CUSTODY_SEED, &[ctx.bumps.egg_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];
    
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts.dragon_egg_collection.as_ref().map(|c| c.to_account_info()).as_ref(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.egg_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;
    
    
    Ok(())
}





fn calc_player_multiplier(existing_multiplier: u16, egg_multiplier: u16, to_add: bool) -> u16 {

    if to_add {
        let new_multiplier = existing_multiplier + egg_multiplier;
        if new_multiplier > MAX_MULTIPLIER {
            return MAX_MULTIPLIER;
        }
        return new_multiplier;
    } else {
        let new_multiplier = existing_multiplier - egg_multiplier;
        if new_multiplier < 100 {
            return 100;
        }
        return new_multiplier;
    }
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
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    /// CHECK: Multisig WSOL token account (destination for WSOL transfers)
    #[account(mut)]
    pub multisig_wsol_account: UncheckedAccount<'info>,

    /// User's WSOL token account (for wrapping SOL to WSOL)
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = wsol_mint,
        associated_token::authority = user,
    )]
    pub user_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// CHECK: WSOL mint
    pub wsol_mint: UncheckedAccount<'info>,

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

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,

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

    /// CHECK: Multisig WSOL token account (destination for WSOL transfers)
    #[account(mut)]
    pub multisig_wsol_account: UncheckedAccount<'info>,

    /// User's WSOL token account (for wrapping SOL to WSOL)
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = wsol_mint,
        associated_token::authority = user,
    )]
    pub user_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// CHECK: WSOL mint
    pub wsol_mint: UncheckedAccount<'info>,

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

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,

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
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
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

