use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::Token;
use anchor_spl::token_2022::{self, Token2022, TransferChecked};
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};
use mpl_core::ID as MPL_CORE_PROGRAM_ID;
// # Doge Instructions
//
// This module manages the Doge NFT system, which provides hashpower multipliers to players.
//
// ## Key Functions
//
// - `batch_mint_doges`: Mints new Doge NFTs using a bonding curve pricing model.
// - `stake_doge`: Stakes an doge to boost a player's hashpower.
// - `unstake_doge`: Unstakes an doge and removes the boost.
// - `claim_power`: Distributes accumulated power points to staked doges.
//
// Doge are a core mechanic for increasing mining efficiency and earning potential.
//

use crate::events::*;
use crate::instructions::helper;
use crate::instructions::stake;
use crate::state::*;

// ----------------------------------------------------------------------------------------
// --------------  DOGE NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

/// Simulate mint costs for multiple doges accounting for bonding curve pricing
/// Returns (total_price, individual_prices, ticket_amounts_per_tier)
/// ticket_amounts_per_tier: Vec of (ticket_value) for each of the 3 ticket tiers
pub fn int_simulate_mint_cost(
    doge_config: &DogeConfig,
    mint_count: u64,
) -> Result<(u64, Vec<u64>, Vec<(u64, u64)>)> {
    require!(
        mint_count > 0 && mint_count <= 10,
        ErrorCode::InvalidParameters
    );
    require!(
        doge_config.doges_minted + mint_count <= doge_config.max_supply,
        ErrorCode::InvalidParameters
    );
    require!(
        doge_config.ticket_tiers.len() == 3,
        ErrorCode::InvalidParameters
    ); // Must have exactly 3 ticket tiers

    let mut prices = Vec::new();
    let mut total_price = 0u64;
    let mut current_minted = doge_config.doges_minted;

    for _ in 0..mint_count {
        let actual_price = crate::genescience::compute_gene_price(
            doge_config.base_price,
            doge_config.curve_a,
            current_minted,
        )?;

        prices.push(actual_price);
        total_price = total_price
            .checked_add(actual_price)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        current_minted += 1;
    }

    // Calculate ticket amounts for each tier: sol_price / ticket_value * 1.5
    // This gives users tickets worth 1.5x the SOL they spent
    let mut ticket_amounts = Vec::new();
    for tier in &doge_config.ticket_tiers {
        // Calculate: (total_price / ticket_value) * 1.5
        // Using fixed-point math: multiply by 150, then divide by 100
        let ticket_count = helper::calc_tickets_count(total_price, tier.ticket_value);
        ticket_amounts.push((tier.ticket_value, ticket_count));
    }

    Ok((total_price, prices, ticket_amounts))
}

/// Batch mint multiple Doge (max 10 per transaction)
/// Uses bonding curve pricing for each doge
///
/// # Remaining Accounts
/// For each doge to mint, the client must pass 2 accounts in remaining_accounts:
/// 1. doge_asset (Signer, Writable) - The new Keypair for the doge
/// 2.doge_metadata (Writable) - The derived PDA for metadata
///
/// So for mint_count = 5, remaining_accounts will have 10 items: [asset_0, meta_0, asset_1, meta_1, ...]
pub fn int_batch_mint_doges<'info>(
    ctx: Context<'_, '_, '_, 'info, BatchMintDoge<'info>>,
    faction_id: u8,
    mint_count: u8,
    ticket_tier_index: u8,
) -> Result<()> {
    require!(
        mint_count > 0 && mint_count <= 10,
        ErrorCode::InvalidParameters
    );

    // Validate we have enough remaining accounts
    // We need 2 accounts per doge: Asset(Signer) + Metadata(PDA)
    require!(
        ctx.remaining_accounts.len() == (mint_count as usize * 2),
        ErrorCode::InvalidParameters
    );

    let global_config = &ctx.accounts.global_config;
    let doge_config = &mut ctx.accounts.doge_config;
    let player_data = &mut ctx.accounts.player_data;

    require!(doge_config.is_active, ErrorCode::MintingNotAllowed);

    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require!(
        doge_config.doges_minted + mint_count as u64 <= doge_config.max_supply,
        ErrorCode::InvalidParameters
    );

    let (total_price, prices, _ticket_amounts) =
        int_simulate_mint_cost(doge_config, mint_count as u64)?;
    msg!(
        "   Batch minting {} doges, total cost: {} lamports",
        mint_count,
        total_price
    );

    let dev_amt = total_price * 80 / 100;
    let treasury_amt = total_price - dev_amt;

    // Transfer to doges treasury
    helper::transfer_to_doges_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.doges_treasury.to_account_info(),
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

    // Handle ticket tier selection and add free tickets (using pre-calculated ticket_amounts)
    let ticket_count =
        add_tickets_to_player(player_data, doge_config, ticket_tier_index, total_price)?;

    // Mint each doge using remaining_accounts
    for i in 0..mint_count {
        let index = i as usize;

        // Get accounts from remaining_accounts
        // [asset_0, meta_0, asset_1, meta_1, ...]
        // Store keys first to avoid lifetime issues
        let doge_asset_key = ctx.remaining_accounts[index * 2].key();
        let doge_metadata_key = ctx.remaining_accounts[index * 2 + 1].key();

        // Verify Asset is a Signer
        require!(
            ctx.remaining_accounts[index * 2].is_signer,
            ErrorCode::Unauthorized
        );

        // Verify Metadata PDA derivation
        let (expected_metadata, metadata_bump) = Pubkey::find_program_address(
            &[DOGE_METADATA_SEED.as_ref(), doge_asset_key.as_ref()],
            ctx.program_id,
        );
        require!(
            doge_metadata_key == expected_metadata,
            ErrorCode::InvalidAccount
        );

        let current_mint_number = doge_config.doges_minted + 1;

        // Generate doge data (DNA, name, URI, multiplier)
        let slot = Clock::get()?.slot + i as u64;
        let (name, uri, dna, multiplier) = generate_doge_data(
            current_mint_number,
            &ctx.accounts.user.key(),
            slot,
            faction_id,
        )?;

        // Create Metaplex Core Asset
        let collection_authority_bump = ctx.bumps.collection_authority;
        let collection_authority_seeds = &[
            crate::state::COLLECTION_AUTHORITY_SEED,
            &[collection_authority_bump],
        ];

        // Get AccountInfo references for this iteration
        // Note: We must access these directly in the function call to avoid lifetime conflicts
        let doge_asset_info = &ctx.remaining_accounts[index * 2];
        let doge_metadata_info = &ctx.remaining_accounts[index * 2 + 1];

        // Prepare collection account info (if exists) - must be done inline to avoid lifetime issues
        let collection_account_info = ctx
            .accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info());

        // Call create_mpl_core_asset with all accounts accessed directly
        // This avoids storing references that mix lifetimes from remaining_accounts and ctx.accounts
        crate::mpl_core_helpers::create_mpl_core_asset(
            doge_asset_info,
            collection_account_info.as_ref(),
            &ctx.accounts.collection_authority.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            name.clone(),
            uri.clone(),
            Some(&[collection_authority_seeds]),
        )?;

        // Initialize Metadata PDA manually (since we can't use #[account(init)] with remaining_accounts)
        // Check if account already exists (shouldn't, but safety check)
        if doge_metadata_info.lamports() == 0 {
            let space = DogeMetadata::LEN;
            let rent = Rent::get()?.minimum_balance(space);

            let metadata_seeds = &[
                DOGE_METADATA_SEED.as_ref(),
                doge_asset_key.as_ref(),
                &[metadata_bump],
            ];
            let metadata_signer = &[&metadata_seeds[..]];

            // Create the account (System Program)
            anchor_lang::system_program::create_account(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::CreateAccount {
                        from: ctx.accounts.user.to_account_info(),
                        to: doge_metadata_info.to_account_info(),
                    },
                    metadata_signer, // The PDA must sign its own creation
                ),
                rent,
                space as u64,
                ctx.program_id, // Assign owner to OUR program
            )?;
        }

        // Write data to the metadata account (generation is in DNA bits 4-6)
        let metadata_data = DogeMetadata {
            mint: doge_asset_key,
            mom: Pubkey::default(),
            dad: Pubkey::default(),
            breed_count: 0,
            cooldown_end: 0,
            accumulated_val: 0,
            dna,
            incubated_player_data: Pubkey::default(),
            multiplier,
            faction_id,
            last_update_ts: Clock::get()?.unix_timestamp,
            created_at: Clock::get()?.unix_timestamp,
            xp: 0,
            bump: metadata_bump,
        };

        // Serialize into the account with Anchor discriminator
        // CRITICAL: Must write the 8-byte discriminator first, then serialize the struct
        let mut data = doge_metadata_info.try_borrow_mut_data()?;

        // Ensure the account has enough space
        require!(
            data.len() >= DogeMetadata::LEN,
            ErrorCode::InvalidParameters
        );

        // Write the 8-byte discriminator (required by Anchor for account deserialization)
        // Anchor calculates discriminator as first 8 bytes of sha256("account:DogeMetadata")
        data[..8].copy_from_slice(&<DogeMetadata as Discriminator>::DISCRIMINATOR);

        // Serialize struct data to a Vec, then copy to buffer after discriminator
        // This is more reliable than using Write trait directly on mutable slice
        let serialized = metadata_data
            .try_to_vec()
            .map_err(|_| ErrorCode::InvalidParameters)?;
        data[8..8 + serialized.len()].copy_from_slice(&serialized);

        // Emit event
        emit!(DogeMinted {
            doge_metadata_account: doge_metadata_key,
            doge_asset_signer: doge_asset_key,
            owner: ctx.accounts.user.key(),
            player: ctx.accounts.player_data.key(),
            mint: doge_asset_key,
            name: name.clone(),
            uri: uri.clone(),
            dna,
            accumulated_val: 0,
            multiplier,
            faction_id,
            price: prices[index as usize],
            ticket_tier: ticket_tier_index as u64,
            ticket_count,
        });

        doge_config.doges_minted += 1;
    }

    msg!(
        "✅ Batch minted {} Doge for faction {}",
        mint_count,
        faction_id
    );
    msg!(
        "   Total doges minted: {} / {}",
        doge_config.doges_minted,
        doge_config.max_supply
    );
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- ADMIN FREE MINT FUNCTION ------------------------------------------------
// ----------------------------------------------------------------------------------------

/// Admin function to mint a Doge NFT for free to a specified recipient
pub fn int_admin_mint_doge(
    ctx: Context<AdminMintDoge>,
    recipient: Pubkey,
    faction_id: u8,
    ticket_tier_index: u8,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let doge_config = &mut ctx.accounts.doge_config;

    // Verify recipient matches instruction parameter
    require!(
        ctx.accounts.recipient.key() == recipient,
        ErrorCode::InvalidAccount
    );
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require!(
        doge_config.doges_minted < doge_config.max_supply,
        ErrorCode::InvalidParameters
    );

    msg!(
        "🎁 [admin_mint_doge] Admin minting free doge to recipient: {}",
        recipient
    );
    msg!("   Faction ID: {}", faction_id);
    msg!("   Doge number: {}", doge_config.doges_minted + 1);

    let current_mint_number = doge_config.doges_minted + 1;

    // Generate doge data (DNA, name, URI, multiplier)
    let slot = Clock::get()?.slot;
    let (name, uri, dna, multiplier) =
        generate_doge_data(current_mint_number, &recipient, slot, faction_id)?;

    // Get collection authority seeds
    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[
        crate::state::COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];

    // Create NFT via MPL Core CPI (paid by admin, sent to recipient)
    msg!("🎨 Creating Doge NFT via Metaplex Core CPI");
    msg!("   Name: {}", name);
    msg!("   URI: {}", uri);
    msg!("   Recipient: {}", recipient);

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
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

    // Calculate actual price using bonding curve (same as regular mint)
    // This is used for ticket calculations - admin mint doesn't charge SOL but tickets are calculated based on actual price
    let cost_per_doge = crate::genescience::compute_gene_price(
        doge_config.base_price,
        doge_config.curve_a,
        doge_config.doges_minted,
    )?;

    msg!(
        "   Calculated doge price: {} lamports (for ticket calculation)",
        cost_per_doge as f64 / 1e9
    );

    // Initialize Doge metadata
    let doge_metadata = &mut ctx.accounts.doge_metadata;
    doge_metadata.mint = ctx.accounts.doge_asset.key();
    doge_metadata.mom = Pubkey::default();
    doge_metadata.dad = Pubkey::default();
    doge_metadata.breed_count = 0;
    doge_metadata.cooldown_end = 0;
    doge_metadata.accumulated_val = 0;
    doge_metadata.dna = dna;
    doge_metadata.incubated_player_data = Pubkey::default();
    doge_metadata.multiplier = multiplier;
    doge_metadata.faction_id = faction_id;
    doge_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    doge_metadata.created_at = Clock::get()?.unix_timestamp;
    doge_metadata.xp = 0;
    doge_metadata.bump = ctx.bumps.doge_metadata;

    // Handle ticket tier selection and add free tickets (using actual price)
    let ticket_count = if doge_config.ticket_tiers.len() > 0 {
        add_tickets_to_player(
            &mut ctx.accounts.player_data,
            doge_config,
            ticket_tier_index,
            cost_per_doge,
        )?
    } else {
        0
    };

    // Update doge config stats
    doge_config.doges_minted += 1;
    msg!(
        "   Total doges minted: {} / {}",
        doge_config.doges_minted,
        doge_config.max_supply
    );

    emit!(DogeMinted {
        doge_metadata_account: doge_metadata.key(),
        doge_asset_signer: ctx.accounts.doge_asset.key(),
        owner: recipient,
        player: ctx.accounts.player_data.key(),
        mint: doge_metadata.mint,
        name,
        uri,
        dna,
        accumulated_val: 0,
        multiplier,
        faction_id,
        price: cost_per_doge,
        ticket_tier: ticket_tier_index as u64,
        ticket_count,
    });

    msg!(
        "✅ Admin minted Doge #{} for faction {} to recipient {}",
        doge_config.doges_minted,
        faction_id,
        recipient
    );
    Ok(())
}

/// Stake a Doge to boost hashpower (multiplier applies to staked minebtc and LP)
/// Users can stake up to 5 doges, each additional doge increases multiplier by 0.5x
/// Multipliers: 1 doge = 1.5x, 2 doges = 2.0x, 3 doges = 2.5x, 4 doges = 3.0x, 5 doges = 3.5x
pub fn int_stake_doge(ctx: Context<StakeDoge>) -> Result<()> {
    let doge_metadata = &mut ctx.accounts.doge_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let current_time = Clock::get()?.unix_timestamp;
    let doge_mint = doge_metadata.mint;
    let doge_multiplier = doge_metadata.multiplier;

    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.doge_asset)?;

    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );
    // Check if already incubated (using Pubkey::default() instead of None)
    require!(
        doge_metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );
    require!(
        doge_metadata.faction_id == player_data.faction_id
            && doge_metadata.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.staked_doges.len() < MAX_STAKED_DOGES,
        ErrorCode::InvalidParameters
    );

    // Transfer NFT to custody PDA (lock it)
    msg!("🔒 Transferring NFT to custody PDA (locking)");
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.doge_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;

    // Process pending rewards before updating position
    let (_new_sol_rewards, _new_minebtc_rewards, _accrued_minebtc_rewards) =
        stake::int_update_minebtc_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    let (_new_sol_rewards, _new_minebtc_rewards, _accrued_minebtc_rewards) =
        stake::int_update_lp_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;

    // Add doge to player's staked doges list
    player_data.staked_doges.push(doge_mint);

    // Calculate new multiplier based on number of staked doges
    let old_multiplier = player_data.doge_multiplier as u64;
    let new_multiplier =
        calc_player_multiplier(old_multiplier as u16, doge_multiplier as u16, true) as u64;
    player_data.doge_multiplier = new_multiplier as u16;
    msg!(
        "⚡ Updated doge multiplier: ({})x",
        player_data.doge_multiplier as f64 / 100.0
    );

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_dogebtc_hashpower = player_data.dogebtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;

    // Recalculate hashpower with new multiplier (multiply first to avoid precision loss)
    // Formula: new_hashpower = (old_hashpower * new_multiplier) / old_multiplier
    if old_multiplier > 0 {
        player_data.dogebtc_hashpower = (existing_dogebtc_hashpower as u128
            * new_multiplier as u128
            / old_multiplier as u128) as u64;
        player_data.lp_hashpower = (existing_lp_hashpower as u128 * new_multiplier as u128
            / old_multiplier as u128) as u64;
    } else {
        // If old_multiplier is 0 (shouldn't happen), use new_multiplier directly
        player_data.dogebtc_hashpower = (existing_dogebtc_hashpower * new_multiplier) / M_HUNDRED;
        player_data.lp_hashpower = (existing_lp_hashpower * new_multiplier) / M_HUNDRED;
    }
    msg!(
        "   MineBtc hashpower: {} -> {}",
        existing_dogebtc_hashpower as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "   LP hashpower: {} -> {}",
        existing_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Update faction state totals
    update_faction_hashpower(
        faction_state,
        existing_dogebtc_hashpower,
        player_data.dogebtc_hashpower,
        existing_lp_hashpower,
        player_data.lp_hashpower,
    );
    msg!(
        "   Faction minebtc hashpower: {} -> {}",
        faction_state.total_dogebtc_hashpower as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "   Faction LP hashpower: {} -> {}",
        faction_state.total_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    faction_state.doges_staked += 1;
    msg!("   Faction doges staked: {} ", faction_state.doges_staked);

    // Update doge metadata
    // Set new owner (using Pubkey instead of Option)
    doge_metadata.incubated_player_data = player_data.owner;
    doge_metadata.last_update_ts = current_time;
    msg!("   Doge metadata updated");

    // Emit event for indexing
    emit!(DogeStaked {
        owner: ctx.accounts.user.key(),
        player: player_data.key(),
        doge_mint: doge_mint,
        faction_id: player_data.faction_id,
        doge_metadata_account: doge_metadata.key(),
        player_multiplier: player_data.doge_multiplier,
        dogebtc_hashpower: player_data.dogebtc_hashpower,
        lp_hashpower: player_data.lp_hashpower,
        timestamp: current_time,
    });

    Ok(())
}

/// Unstake a Doge (reduces multiplier and recalculates hashpower)
pub fn int_unstake_doge(ctx: Context<UnstakeDoge>) -> Result<()> {
    let doge_metadata = &mut ctx.accounts.doge_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let doge_mint = doge_metadata.mint;
    let incubated_by_player = doge_metadata.incubated_player_data;
    let current_time = Clock::get()?.unix_timestamp;
    let doge_multiplier = doge_metadata.multiplier;

    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.doge_asset)?;
    require!(
        nft_owner == ctx.accounts.doge_custody_pda.key(),
        ErrorCode::DogeNotAtGuard
    );
    // Verify ownership (using Pubkey::default() check instead of is_some())
    require!(
        doge_metadata.incubated_player_data != Pubkey::default(),
        ErrorCode::DogeNotAtGuard
    );
    require!(
        doge_metadata.faction_id == player_data.faction_id
            && doge_metadata.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.staked_doges.contains(&doge_mint),
        ErrorCode::InvalidParameters
    );
    require!(
        incubated_by_player == player_data.owner,
        ErrorCode::Unauthorized
    );

    // Process pending rewards before updating position
    let (_new_sol_rewards, _new_minebtc_rewards, _accrued_minebtc_rewards) =
        stake::int_update_minebtc_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;
    let (_new_sol_rewards, _new_minebtc_rewards, _accrued_minebtc_rewards) =
        stake::int_update_lp_staking_rewards(
            player_data,
            &mut ctx.accounts.unrefined_rewards,
            faction_state,
        )?;

    // Remove doge from player's staked doges list
    if let Some(index) = player_data
        .staked_doges
        .iter()
        .position(|&mint| mint == doge_mint)
    {
        player_data.staked_doges.remove(index);
        msg!(
            "   Removed doge from staked doges. Remaining: {}",
            player_data.staked_doges.len()
        );
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // Calculate new multiplier based on number of staked doges
    let old_multiplier = player_data.doge_multiplier as u64;
    let new_multiplier =
        calc_player_multiplier(old_multiplier as u16, doge_multiplier as u16, false) as u64;
    player_data.doge_multiplier = new_multiplier as u16;
    msg!(
        "⚡ Updated doge multiplier: ({})x",
        player_data.doge_multiplier as f64 / 100.0
    );

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_dogebtc_hashpower = player_data.dogebtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;

    if old_multiplier > 0 {
        player_data.dogebtc_hashpower = (existing_dogebtc_hashpower as u128
            * new_multiplier as u128
            / old_multiplier as u128) as u64;
        player_data.lp_hashpower = (existing_lp_hashpower as u128 * new_multiplier as u128
            / old_multiplier as u128) as u64;
    }
    msg!(
        "   MineBtc hashpower: {} -> {}",
        existing_dogebtc_hashpower as f64 / 1e6,
        player_data.dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "   LP hashpower: {} -> {}",
        existing_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Update faction state totals
    update_faction_hashpower(
        faction_state,
        existing_dogebtc_hashpower,
        player_data.dogebtc_hashpower,
        existing_lp_hashpower,
        player_data.lp_hashpower,
    );
    msg!(
        "   Faction minebtc hashpower: {} -> {}",
        faction_state.total_dogebtc_hashpower as f64 / 1e6,
        faction_state.total_dogebtc_hashpower as f64 / 1e6
    );
    msg!(
        "   Faction LP hashpower: {} -> {}",
        faction_state.total_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    faction_state.doges_staked -= 1;
    msg!(
        "   Faction doges staked: {} -> {}",
        faction_state.doges_staked,
        faction_state.doges_staked
    );

    // Update doge metadata
    // Clear owner (Set back to default using Pubkey::default() instead of None)
    doge_metadata.incubated_player_data = Pubkey::default();
    doge_metadata.last_update_ts = current_time;
    msg!("   Doge metadata updated");

    // Transfer NFT back to user (unlock it)
    msg!("🔓 Transferring NFT back to user (unlocking)");
    let custody_seeds = &[DOGE_CUSTODY_SEED, &[ctx.bumps.doge_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];

    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &&ctx.accounts.user.to_account_info(),
        &ctx.accounts.doge_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;

    // Emit event for indexing
    emit!(DogeUnstaked {
        owner: ctx.accounts.user.key(),
        player: player_data.key(),
        doge_mint: doge_mint,
        doge_metadata_account: doge_metadata.key(),
        faction_id: player_data.faction_id,
        player_multiplier: player_data.doge_multiplier,
        dogebtc_hashpower: player_data.dogebtc_hashpower,
        lp_hashpower: player_data.lp_hashpower,
        timestamp: current_time,
    });

    Ok(())
}

/// Send an doge to heaven (burn it) to claim accumulated rewards
pub fn int_send_to_heaven(ctx: Context<SendToHeaven>) -> Result<()> {
    let doge_config = &mut ctx.accounts.doge_config;
    let doge_metadata = &ctx.accounts.doge_metadata;
    let accumulated_val = doge_metadata.accumulated_val;
    let current_time = Clock::get()?.unix_timestamp;

    // Verify not incubated (should be default if user holds it, but double check)
    require!(
        doge_metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );

    doge_config.doges_minted -= 1;

    msg!("🔥 Burning Doge NFT to send to heaven...");
    msg!("   Accumulated Value: {}", accumulated_val);

    // Burn the NFT
    crate::mpl_core_helpers::burn_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;

    // Transfer accumulated tokens if any
    if accumulated_val > 0 {
        msg!("💸 Transferring {} MINEBTC to user...", accumulated_val);

        let seeds = &[
            MINE_BTC_VAULT_AUTHORITY_SEED,
            &[ctx.accounts.mine_btc_mining.vault_auth_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.minebtc_token_vault.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer_seeds,
            ),
            accumulated_val,
            MINEBTC_DECIMALS,
        )?;

        // Update mining stats
        let mining_state = &mut ctx.accounts.mine_btc_mining;
        mining_state.total_tokens_distributed += accumulated_val;
    }

    // Emit event
    emit!(DogeSentToHeaven {
        doge_mint: doge_metadata.mint,
        user: ctx.accounts.user.key(),
        accumulated_val,
        timestamp: current_time,
    });

    msg!("✅ [send_to_heaven] Doge sent to heaven successfully");

    Ok(())
}

/// Breed two doges to create offspring (both parents must not be incubated, same faction)
pub fn int_breed_doges(ctx: Context<BreedDoge>) -> Result<()> {
    let doge_config = &mut ctx.accounts.doge_config;
    let mom = &mut ctx.accounts.mom_metadata;
    let dad = &mut ctx.accounts.dad_metadata;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    msg!("🧬 === BREEDING DOGES ===");
    msg!("   Mom: {} (breed_count: {})", mom.mint, mom.breed_count);
    msg!("   Dad: {} (breed_count: {})", dad.mint, dad.breed_count);

    // Validate breeding is allowed
    require!(doge_config.breeding_allowed, ErrorCode::BreedingNotAllowed);
    require!(
        doge_config.doges_minted < doge_config.max_supply,
        ErrorCode::InvalidParameters
    );

    // Validate parents are not incubated
    require!(
        mom.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );
    require!(
        dad.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );

    // Validate same faction
    require!(
        mom.faction_id == dad.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Validate breed counts
    require!(
        mom.breed_count < DogeMetadata::MAX_BREED_COUNT,
        ErrorCode::MaxBreedCountReached
    );
    require!(
        dad.breed_count < DogeMetadata::MAX_BREED_COUNT,
        ErrorCode::MaxBreedCountReached
    );

    // Validate cooldowns
    require!(
        mom.cooldown_end <= current_time,
        ErrorCode::CooldownNotEnded
    );
    require!(
        dad.cooldown_end <= current_time,
        ErrorCode::CooldownNotEnded
    );

    // Verify NFT ownership
    let mom_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.mom_asset)?;
    let dad_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dad_asset)?;
    require!(
        mom_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );
    require!(
        dad_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );

    // Calculate breeding cost
    let breed_cost = crate::genescience::compute_gene_price(
        doge_config.breed_base_price,
        doge_config.breed_curve_a,
        doge_config.doges_minted,
    )?;
    msg!("   Breed cost: {} SOL", breed_cost as f64 / 1e9);

    // Transfer breeding cost
    let dev_amt = breed_cost * 50 / 100;
    let treasury_amt = breed_cost - dev_amt;

    helper::transfer_to_doges_treasury(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.doges_treasury.to_account_info(),
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

    // Generate offspring DNA
    let seed = [
        clock.slot.to_le_bytes().as_ref(),
        ctx.accounts.user.key().as_ref(),
        mom.mint.as_ref(),
        dad.mint.as_ref(),
    ]
    .concat();
    let offspring_dna = crate::genescience::breed_genes(&mom.dna, &dad.dna, &seed)?;

    // Create offspring NFT
    let current_mint_number = doge_config.doges_minted + 1;
    let name = format!("Bitcoin doges #{}", current_mint_number);
    let uri = format!("Bitcoin doges #{}", current_mint_number);

    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.offspring_asset.to_account_info(),
        ctx.accounts
            .doge_collection
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

    // Initialize offspring metadata
    let offspring = &mut ctx.accounts.offspring_metadata;
    offspring.mint = ctx.accounts.offspring_asset.key();
    offspring.mom = mom.mint;
    offspring.dad = dad.mint;
    offspring.breed_count = 0;
    offspring.cooldown_end = 0;
    offspring.accumulated_val = 0;
    offspring.dna = offspring_dna;
    offspring.incubated_player_data = Pubkey::default();
    offspring.multiplier = BASE_MULTIPLIER;
    offspring.faction_id = mom.faction_id;
    offspring.last_update_ts = current_time;
    offspring.created_at = current_time;
    offspring.xp = 0;
    offspring.bump = ctx.bumps.offspring_metadata;

    // Update parent cooldowns and breed counts
    let mom_cooldown = DogeMetadata::COOLDOWNS
        .get(mom.breed_count as usize)
        .copied()
        .unwrap_or(1209600);
    let dad_cooldown = DogeMetadata::COOLDOWNS
        .get(dad.breed_count as usize)
        .copied()
        .unwrap_or(1209600);

    mom.breed_count += 1;
    mom.cooldown_end = current_time + mom_cooldown;
    dad.breed_count += 1;
    dad.cooldown_end = current_time + dad_cooldown;

    doge_config.doges_minted += 1;

    msg!(
        "✅ Bred offspring #{} from {} x {}",
        current_mint_number,
        mom.mint,
        dad.mint
    );
    msg!(
        "   Mom next cooldown: {}s, Dad next cooldown: {}s",
        mom_cooldown,
        dad_cooldown
    );

    emit!(DogeMinted {
        doge_metadata_account: offspring.key(),
        doge_asset_signer: ctx.accounts.offspring_asset.key(),
        owner: ctx.accounts.user.key(),
        player: ctx.accounts.player_data.key(),
        mint: offspring.mint,
        name,
        uri,
        dna: offspring_dna,
        accumulated_val: 0,
        multiplier: BASE_MULTIPLIER,
        faction_id: mom.faction_id,
        price: breed_cost,
        ticket_tier: 0,
        ticket_count: 0,
    });

    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HELPER FUNCTIONS ---------------------------------------------------------
// ----------------------------------------------------------------------------------------

/// Generate doge data (DNA, name, URI, multiplier) for a new doge
pub fn generate_doge_data(
    mint_number: u64,
    user_key: &Pubkey,
    slot_offset: u64,
    faction_id: u8,
) -> Result<(String, String, [u8; 32], u32)> {
    let dna = crate::genescience::generate_genesis_dna(
        mint_number,
        user_key,
        Clock::get()?.slot + slot_offset,
        faction_id,
    )?;
    let name = format!("Bitcoin doges #{}", mint_number);
    let uri = format!("Bitcoin doges #{}", mint_number);
    let multiplier = BASE_MULTIPLIER;

    Ok((name, uri, dna, multiplier))
}

/// Add tickets to player based on price and ticket tier
fn add_tickets_to_player(
    player_data: &mut PlayerData,
    doge_config: &DogeConfig,
    ticket_tier_index: u8,
    price: u64,
) -> Result<u64> {
    require!(
        (ticket_tier_index as usize) < doge_config.ticket_tiers.len(),
        ErrorCode::InvalidParameters
    );
    require!(
        doge_config.ticket_tiers.len() == 3,
        ErrorCode::InvalidParameters
    );

    let selected_tier = &doge_config.ticket_tiers[ticket_tier_index as usize];
    let ticket_value = selected_tier.ticket_value;
    let ticket_count = helper::calc_tickets_count(price, ticket_value);

    msg!(
        "   Selected ticket tier: {} tickets of {} SOL each (calculated from {} SOL)",
        ticket_count,
        ticket_value as f64 / 1e9,
        price as f64 / 1e9
    );

    // Add free tickets to player
    if let Some(index) = player_data
        .free_tickets
        .iter()
        .position(|&v| v == ticket_value)
    {
        player_data.free_tickets_remaining[index] =
            player_data.free_tickets_remaining[index] + ticket_count;
    } else {
        require!(
            player_data.free_tickets.len() < PlayerData::MAX_TICKET_TYPES,
            ErrorCode::InvalidParameters
        );
        player_data.free_tickets.push(ticket_value);
        player_data.free_tickets_remaining.push(ticket_count);
    }
    msg!(
        "     Added new ticket type: {} tickets of {} SOL",
        ticket_count,
        ticket_value as f64 / 1e9
    );

    Ok(ticket_count)
}

/// Update faction state hashpower totals
fn update_faction_hashpower(
    faction_state: &mut FactionState,
    old_dogebtc_hashpower: u64,
    new_dogebtc_hashpower: u64,
    old_lp_hashpower: u64,
    new_lp_hashpower: u64,
) {
    faction_state.total_dogebtc_hashpower =
        faction_state.total_dogebtc_hashpower - old_dogebtc_hashpower + new_dogebtc_hashpower;
    faction_state.total_lp_hashpower =
        faction_state.total_lp_hashpower - old_lp_hashpower + new_lp_hashpower;
}

fn calc_player_multiplier(existing_multiplier: u16, doge_multiplier: u16, to_add: bool) -> u16 {
    if to_add {
        let new_multiplier = existing_multiplier + doge_multiplier;
        if new_multiplier > MAX_MULTIPLIER {
            return MAX_MULTIPLIER;
        }
        return new_multiplier;
    } else {
        if doge_multiplier > existing_multiplier {
            return BASE_MULTIPLIER as u16;
        }
        let new_multiplier = existing_multiplier - doge_multiplier;
        if new_multiplier < BASE_MULTIPLIER as u16 {
            return BASE_MULTIPLIER as u16;
        }
        return new_multiplier;
    }
}

// ----------------------------------------------------------------------------------------
// --------------  DOGE ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(mint_count: u64)]
pub struct SimulateMintCost<'info> {
    #[account(
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doge_config.bump
    )]
    pub doge_config: Account<'info, DogeConfig>,
}

#[derive(Accounts)]
#[instruction(faction_id: u8)]
pub struct MintDoge<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doge_config.bump
    )]
    pub doge_config: Account<'info, DogeConfig>,

    /// CHECK: Doge treasury PDA (for doge minting fees)
    #[account(
        mut,
        seeds = [DOGES_TREASURY_SEED.as_ref()],
        bump
    )]
    pub doges_treasury: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    /// Multisig WSOL token account (destination for WSOL transfers)
    /// MUST be owned by global_config.fee_recipient (the multisig address)
    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

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
    pub doge_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Doge
    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = DogeMetadata::LEN,
        seeds = [DOGE_METADATA_SEED.as_ref(), doge_asset.key().as_ref()],
        bump
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

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
pub struct BatchMintDoge<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doge_config.bump,
    )]
    pub doge_config: Account<'info, DogeConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    /// CHECK: Doge treasury PDA (for doge minting fees)
    #[account(
        mut,
        seeds = [DOGES_TREASURY_SEED.as_ref()],
        bump
    )]
    pub doges_treasury: UncheckedAccount<'info>,

    /// Multisig WSOL token account (destination for WSOL transfers)
    /// MUST be owned by global_config.fee_recipient (the multisig address)
    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

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

    /// CHECK: Doge collection (Metaplex Core)
    #[account(mut)]
    pub doge_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Collection authority PDA
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED.as_ref()],
        bump
    )]
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
#[instruction(recipient: Pubkey, faction_id: u8)]
pub struct AdminMintDoge<'info> {
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
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doge_config.bump,
    )]
    pub doge_config: Account<'info, DogeConfig>,

    /// CHECK: Recipient account (will receive the NFT)
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,

    /// Player data account for the recipient (for ticket distribution)
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), recipient.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == recipient.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    /// Metaplex Core asset (will be created)
    #[account(mut)]
    /// CHECK: Will be created via MPL Core CPI
    pub doge_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Doge
    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = authority,
        space = DogeMetadata::LEN,
        seeds = [DOGE_METADATA_SEED.as_ref(), doge_asset.key().as_ref()],
        bump
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

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
pub struct StakeDoge<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    /// Metaplex Core asset (source of truth for ownership)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub doge_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Doge
    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(),doge_metadata.mint.as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DOGE_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub doge_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeDoge<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    #[account(
        mut,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    /// Metaplex Core asset (currently locked in custody PDA)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub doge_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Doge
    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(),doge_metadata.mint.as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DOGE_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub doge_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SendToHeaven<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [DOGE_CONFIG_SEED.as_ref()], bump = doge_config.bump)]
    pub doge_config: Account<'info, DogeConfig>,

    #[account(
        mut,
        close = user,
        seeds = [DOGE_METADATA_SEED.as_ref(), doge_asset.key().as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// Metaplex Core asset (will be burnt)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper (implicit in burn)
    pub doge_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Doge
    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    // Mining accounts for token transfer
    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(
        mut,
        seeds = [MINE_BTC_VAULT_SEED, mine_btc_mining.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault_authority,
    )]
    pub minebtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Vault authority PDA
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount2022>,

    #[account(address = minebtc_token_vault.mint)]
    pub token_mint: InterfaceAccount<'info, Mint2022>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BreedDoge<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut, seeds = [DOGE_CONFIG_SEED.as_ref()], bump = doge_config.bump)]
    pub doge_config: Account<'info, DogeConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// CHECK: Doge treasury PDA
    #[account(mut, seeds = [DOGES_TREASURY_SEED.as_ref()], bump)]
    pub doges_treasury: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = wsol_mint,
        associated_token::authority = user,
    )]
    pub user_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// CHECK: WSOL mint
    pub wsol_mint: UncheckedAccount<'info>,

    /// CHECK: Mom NFT asset - Verified via get_mpl_core_owner
    #[account(mut)]
    pub mom_asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(), mom_asset.key().as_ref()],
        bump = mom_metadata.bump,
        constraint = mom_metadata.mint == mom_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub mom_metadata: Box<Account<'info, DogeMetadata>>,

    /// CHECK: Dad NFT asset - Verified via get_mpl_core_owner
    #[account(mut)]
    pub dad_asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(), dad_asset.key().as_ref()],
        bump = dad_metadata.bump,
        constraint = dad_metadata.mint == dad_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dad_metadata: Box<Account<'info, DogeMetadata>>,

    /// CHECK: Offspring NFT asset - Will be created via MPL Core CPI
    #[account(mut)]
    pub offspring_asset: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = DogeMetadata::LEN,
        seeds = [DOGE_METADATA_SEED.as_ref(), offspring_asset.key().as_ref()],
        bump
    )]
    pub offspring_metadata: Box<Account<'info, DogeMetadata>>,

    /// CHECK: Doge collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Collection authority PDA
    #[account(seeds = [COLLECTION_AUTHORITY_SEED.as_ref()], bump)]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
