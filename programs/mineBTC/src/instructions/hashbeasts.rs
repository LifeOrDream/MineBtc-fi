use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::Token;
use anchor_spl::token_2022::{self, Burn, Token2022, TransferChecked};
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};
use mpl_core::ID as MPL_CORE_PROGRAM_ID;
// # HashBeast Instructions
//
// HashBeasts serve three distinct roles in DegenBTC:
// - primary-market NFTs minted from the bonding-curve-like pricing path,
// - passive staking boosters that raise a player's home-faction staking hashpower,
// - gameplay avatars used in round betting / mutation progression (handled partly in `user.rs`).
//
// Important distinction:
// - `player_data.hashbeast_multiplier` is the passive staking multiplier affected by `stake_hashbeast`.
// - `player_data.active_multiplier` is the gameplay multiplier used for round participation.
//
// This file focuses on minting, passive HashBeast staking, rebirthing (`rebirth_hashbeast`),
// and breeding. Gameplay lock / unlock flows live elsewhere.
//

use crate::events::*;
use crate::instructions::helper;
use crate::instructions::stake;
use crate::state::*;

// ----------------------------------------------------------------------------------------
// --------------  HASHBEAST NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

fn load_program_account<T: AccountDeserialize>(account: &AccountInfo<'_>) -> Result<T> {
    require!(account.owner == &crate::ID, ErrorCode::InvalidAccount);
    let data = account.try_borrow_data()?;
    let mut data_slice: &[u8] = &data;
    T::try_deserialize(&mut data_slice)
}

fn ceil_mul_div_u64(a: u64, b: u64, c: u64) -> Result<u64> {
    require!(c > 0, ErrorCode::InvalidParameters);
    let numerator = (a as u128)
        .checked_mul(b as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let denominator = c as u128;
    let value = numerator
        .checked_add(denominator.saturating_sub(1))
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(denominator)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(value).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

fn metadata_rebirth_count(metadata: &HashBeastMetadata) -> u8 {
    metadata
        .rebirth_count
        .max(crate::genescience::get_rebirth_count(&metadata.dna))
}

fn assert_player_data_owner(account: &AccountInfo<'_>, expected_owner: &Pubkey) -> Result<()> {
    require!(account.owner == &crate::ID, ErrorCode::InvalidAccount);
    let data = account.try_borrow_data()?;
    require!(
        data.len() >= DISCRIMINATOR_SIZE + 1 + 32,
        ErrorCode::InvalidAccount
    );
    require!(
        data[..DISCRIMINATOR_SIZE] == <PlayerData as Discriminator>::DISCRIMINATOR[..],
        ErrorCode::InvalidAccount
    );

    let mut owner_bytes = [0u8; 32];
    owner_bytes.copy_from_slice(&data[DISCRIMINATOR_SIZE + 1..DISCRIMINATOR_SIZE + 1 + 32]);
    let actual_owner = Pubkey::new_from_array(owner_bytes);
    require_keys_eq!(actual_owner, *expected_owner, ErrorCode::Unauthorized);
    Ok(())
}

fn shares_known_parent(a: &HashBeastMetadata, b: &HashBeastMetadata) -> bool {
    let parents_a = [a.mom, a.dad];
    let parents_b = [b.mom, b.dad];
    parents_a.iter().any(|parent_a| {
        *parent_a != Pubkey::default() && parents_b.iter().any(|parent_b| parent_a == parent_b)
    })
}

fn load_staked_hashbeast_raw_multiplier(
    remaining_accounts: &[AccountInfo<'_>],
    expected_mints: &[Pubkey],
) -> Result<u64> {
    msg!(
        "🧮 [load_staked_hashbeast_raw_multiplier] expected_mints={} remaining_accounts={} expected={:?}",
        expected_mints.len(),
        remaining_accounts.len(),
        expected_mints
    );
    require!(
        remaining_accounts.len() == expected_mints.len(),
        ErrorCode::InvalidParameters
    );

    let mut seen_mints: Vec<Pubkey> = Vec::with_capacity(expected_mints.len());
    let mut raw_multiplier = BASE_MULTIPLIER as u64;

    for account in remaining_accounts {
        let hashbeast_metadata: HashBeastMetadata = load_program_account(account)?;
        let mint = hashbeast_metadata.mint;
        let (expected_pda, _) = Pubkey::find_program_address(
            &[HASHBEAST_METADATA_SEED.as_ref(), mint.as_ref()],
            &crate::ID,
        );
        require!(account.key() == expected_pda, ErrorCode::InvalidAccount);
        require!(expected_mints.contains(&mint), ErrorCode::InvalidParameters);
        require!(!seen_mints.contains(&mint), ErrorCode::InvalidParameters);
        msg!(
            "   [load_staked_hashbeast_raw_multiplier] metadata={} mint={} multiplier={} faction_id={} expected_pda_match={}",
            account.key(),
            mint,
            hashbeast_metadata.multiplier as f64 / 1000.0,
            hashbeast_metadata.faction_id,
            account.key() == expected_pda
        );

        raw_multiplier = raw_multiplier
            .checked_add(hashbeast_metadata.multiplier as u64)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        seen_mints.push(mint);
    }

    for expected_mint in expected_mints {
        require!(
            seen_mints.contains(expected_mint),
            ErrorCode::InvalidParameters
        );
    }

    msg!(
        "✅ [load_staked_hashbeast_raw_multiplier] raw_multiplier={}x loaded_mints={:?}",
        raw_multiplier as f64 / 1000.0,
        seen_mints
    );

    Ok(raw_multiplier)
}

/// Simulate mint costs for multiple hashbeasts accounting for bonding curve pricing
/// Returns (total_price, individual_prices, ticket_amounts_per_tier)
/// ticket_amounts_per_tier: Vec of (ticket_value) for each of the 3 ticket tiers
pub fn int_simulate_mint_cost(
    hashbeast_config: &HashBeastConfig,
    hashbeast_mint_config: &HashBeastMintConfig,
    mint_count: u64,
) -> Result<(u64, Vec<u64>, Vec<(u64, u64)>)> {
    crate::log_fn!("hashbeasts", "int_simulate_mint_cost");
    require!(
        mint_count > 0 && mint_count <= 10,
        ErrorCode::InvalidParameters
    );
    require!(
        hashbeast_mint_config
            .genesis_mints
            .checked_add(mint_count)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            <= hashbeast_mint_config.genesis_mint_limit,
        ErrorCode::InvalidParameters
    );
    require!(
        hashbeast_mint_config.ticket_tiers.len() == 3,
        ErrorCode::InvalidParameters
    ); // Must have exactly 3 ticket tiers
    msg!(
        "🧮 [simulate_mint_cost] mint_count={} total_minted_after={} genesis_after={} / {}",
        mint_count,
        hashbeast_config
            .total_hashbeasts_minted
            .checked_add(mint_count)
            .ok_or(ErrorCode::ArithmeticOverflow)?,
        hashbeast_mint_config
            .genesis_mints
            .checked_add(mint_count)
            .ok_or(ErrorCode::ArithmeticOverflow)?,
        hashbeast_mint_config.genesis_mint_limit
    );

    let mut prices = Vec::new();
    let mut total_price = 0u64;
    let mut current_minted = hashbeast_mint_config.genesis_mints;

    for _ in 0..mint_count {
        let actual_price = crate::genescience::compute_gene_price(
            hashbeast_mint_config.base_price,
            hashbeast_mint_config.curve_a,
            current_minted,
        )?;

        prices.push(actual_price);
        total_price = total_price
            .checked_add(actual_price)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        current_minted = current_minted
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    // Calculate ticket amounts for each tier: sol_price / ticket_value
    // Users get 100% of their mint price as game tickets
    let mut ticket_amounts = Vec::new();
    for tier in &hashbeast_mint_config.ticket_tiers {
        // Calculate: total_price / ticket_value (1.0x)
        let ticket_count = helper::calc_tickets_count(total_price, tier.ticket_value);
        ticket_amounts.push((tier.ticket_value, ticket_count));
    }

    Ok((total_price, prices, ticket_amounts))
}

fn validate_genesis_faction_cap(
    hashbeast_mint_config: &HashBeastMintConfig,
    faction_id: u8,
    mint_count: u8,
) -> Result<()> {
    let faction_index = faction_id as usize;
    require!(faction_index < NUM_FACTIONS, ErrorCode::InvalidFactionId);
    let current_faction_mints = hashbeast_mint_config.genesis_mints_by_faction[faction_index];
    let next_faction_mints = current_faction_mints
        .checked_add(mint_count as u16)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(
        next_faction_mints <= hashbeast_mint_config.max_genesis_mints_per_faction,
        ErrorCode::InvalidParameters
    );
    msg!(
        "🏁 [validate_genesis_faction_cap] faction_id={} mint_count={} faction_mints_after={} / {}",
        faction_id,
        mint_count,
        next_faction_mints,
        hashbeast_mint_config.max_genesis_mints_per_faction
    );
    Ok(())
}

/// Batch mint multiple HashBeast (max 10 per transaction)
/// Uses bonding curve pricing for each hashbeast
///
/// # Remaining Accounts
/// For each hashbeast to mint, the client must pass 2 accounts in remaining_accounts:
/// 1. hashbeast_asset (Signer, Writable) - The new Keypair for the hashbeast
/// 2. hashbeast_metadata (Writable) - The derived PDA for metadata
///
/// So for mint_count = 5, remaining_accounts will have 10 items: [asset_0, meta_0, asset_1, meta_1, ...]
pub fn int_batch_mint_hashbeasts<'info>(
    ctx: Context<'_, '_, '_, 'info, BatchMintHashBeast<'info>>,
    faction_id: u8,
    mint_count: u8,
    ticket_tier_index: u8,
) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_batch_mint_hashbeasts");
    require!(
        mint_count > 0 && mint_count <= 10,
        ErrorCode::InvalidParameters
    );

    // Validate we have enough remaining accounts
    // We need 2 accounts per hashbeast: Asset(Signer) + Metadata(PDA)
    require!(
        ctx.remaining_accounts.len() == (mint_count as usize * 2),
        ErrorCode::InvalidParameters
    );

    let global_config = &ctx.accounts.global_config;
    let hashbeast_config = &mut ctx.accounts.hashbeast_config;
    let hashbeast_mint_config = &mut ctx.accounts.hashbeast_mint_config;
    let player_data = &mut ctx.accounts.player_data;

    require!(!global_config.is_paused, ErrorCode::GamePaused);
    require!(
        hashbeast_mint_config.is_active,
        ErrorCode::MintingNotAllowed
    );

    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require!(
        hashbeast_mint_config
            .genesis_mints
            .checked_add(mint_count as u64)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            <= hashbeast_mint_config.genesis_mint_limit,
        ErrorCode::InvalidParameters
    );
    validate_genesis_faction_cap(hashbeast_mint_config, faction_id, mint_count)?;

    let (total_price, prices, _ticket_amounts) =
        int_simulate_mint_cost(hashbeast_config, hashbeast_mint_config, mint_count as u64)?;
    msg!(
        "   Batch minting {} genesis hashbeasts, total cost: {} lamports",
        mint_count,
        total_price
    );

    // --- Referral commission: tiered based on faction alignment ---
    // Same-country recruits: 1.0% of mint price. Cross-country: 0.5%.
    // Sent directly to the canonical referrer's ReferralRewards PDA.
    let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
    let (_referral_cut, remaining) = if has_referrer {
        helper::validate_referrer_rewards_account(
            &player_data.referral_code,
            ctx.accounts.referrer_rewards.as_ref(),
        )?;

        let same_faction = player_data.referrer_faction_id != u8::MAX
            && player_data.faction_id == player_data.referrer_faction_id;
        let bps = if same_faction {
            crate::state::REFERRAL_FEE_BPS_SAME_FACTION
        } else {
            crate::state::REFERRAL_FEE_BPS_CROSS_FACTION
        };
        let cut = u64::try_from(helper::mul_div(total_price, bps as u64, 10_000)?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        let referrer_rewards = ctx
            .accounts
            .referrer_rewards
            .as_mut()
            .ok_or(ErrorCode::ReferralRewardsAccountRequired)?;

        // Transfer SOL from user to referrer_rewards PDA (stored as extra lamports)
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: referrer_rewards.to_account_info(),
                },
            ),
            cut,
        )?;
        referrer_rewards.pending_sol_rewards = referrer_rewards
            .pending_sol_rewards
            .checked_add(cut)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        referrer_rewards.total_sol_earned = referrer_rewards
            .total_sol_earned
            .checked_add(cut)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Referral commission ({} bps, same_faction={}): {} lamports sent to referrer PDA",
            bps,
            same_faction,
            cut
        );
        (
            cut,
            total_price
                .checked_sub(cut)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
    } else {
        (0, total_price)
    };

    // All remaining SOL from NFT mint goes to fee_recipient via WSOL
    let dev_amt = remaining;

    helper::transfer_wsol_to_multisig(
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.multisig_wsol_account.to_account_info(),
        &ctx.accounts.user_wsol_account.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        dev_amt,
    )?;

    // Handle ticket tier selection and add free tickets (using pre-calculated ticket_amounts)
    let ticket_count = add_tickets_to_player(
        player_data,
        hashbeast_mint_config,
        ticket_tier_index,
        total_price,
    )?;

    // Mint each hashbeast using remaining_accounts
    for i in 0..mint_count {
        let index = i as usize;

        // Get accounts from remaining_accounts
        // [asset_0, meta_0, asset_1, meta_1, ...]
        // Store keys first to avoid lifetime issues
        let hashbeast_asset_key = ctx.remaining_accounts[index * 2].key();
        let hashbeast_metadata_key = ctx.remaining_accounts[index * 2 + 1].key();

        // Verify Asset is a Signer
        require!(
            ctx.remaining_accounts[index * 2].is_signer,
            ErrorCode::Unauthorized
        );

        // Verify Metadata PDA derivation
        let (expected_metadata, metadata_bump) = Pubkey::find_program_address(
            &[
                HASHBEAST_METADATA_SEED.as_ref(),
                hashbeast_asset_key.as_ref(),
            ],
            ctx.program_id,
        );
        require!(
            hashbeast_metadata_key == expected_metadata,
            ErrorCode::InvalidAccount
        );

        let current_mint_number = hashbeast_config
            .total_hashbeasts_minted
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Generate hashbeast data (DNA, name, URI, multiplier)
        let slot = Clock::get()?.slot + i as u64;
        let (name, uri, dna, multiplier) = generate_hashbeast_data(
            current_mint_number,
            &ctx.accounts.user.key(),
            slot,
            faction_id,
            &hashbeast_asset_key,
        )?;

        // Create Metaplex Core Asset
        let collection_authority_bump = ctx.bumps.collection_authority;
        let collection_authority_seeds = &[
            crate::state::COLLECTION_AUTHORITY_SEED,
            &[collection_authority_bump],
        ];

        // Get AccountInfo references for this iteration
        // Note: We must access these directly in the function call to avoid lifetime conflicts
        let hashbeast_asset_info = &ctx.remaining_accounts[index * 2];
        let hashbeast_metadata_info = &ctx.remaining_accounts[index * 2 + 1];

        // Prepare collection account info (if exists) - must be done inline to avoid lifetime issues
        let collection_account_info = ctx
            .accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info());

        // Call create_mpl_core_asset with all accounts accessed directly
        // This avoids storing references that mix lifetimes from remaining_accounts and ctx.accounts
        crate::mpl_core_helpers::create_mpl_core_asset(
            hashbeast_asset_info,
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
        if hashbeast_metadata_info.lamports() == 0 {
            let space = HashBeastMetadata::LEN;
            let rent = Rent::get()?.minimum_balance(space);

            let metadata_seeds = &[
                HASHBEAST_METADATA_SEED.as_ref(),
                hashbeast_asset_key.as_ref(),
                &[metadata_bump],
            ];
            let metadata_signer = &[&metadata_seeds[..]];

            // Create the account (System Program)
            anchor_lang::system_program::create_account(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::CreateAccount {
                        from: ctx.accounts.user.to_account_info(),
                        to: hashbeast_metadata_info.to_account_info(),
                    },
                    metadata_signer, // The PDA must sign its own creation
                ),
                rent,
                space as u64,
                ctx.program_id, // Assign owner to OUR program
            )?;
        }

        // Sanity check: now that the account exists (either freshly created
        // by us above, or pre-existing), it must be owned by this program
        // before we write to it. Prevents an attacker from passing a
        // system-owned or foreign-owned address whose data we'd otherwise
        // happily overwrite via the unchecked `try_borrow_mut_data` below.
        require_keys_eq!(
            *hashbeast_metadata_info.owner,
            crate::ID,
            ErrorCode::InvalidAccount
        );

        // Write data to the metadata account (generation is in DNA bits 4-6)
        let metadata_data = HashBeastMetadata {
            mint: hashbeast_asset_key,
            mom: Pubkey::default(),
            dad: Pubkey::default(),
            breed_count: 0,
            rebirth_count: 0,
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
        let mut data = hashbeast_metadata_info.try_borrow_mut_data()?;

        // Ensure the account has enough space
        require!(
            data.len() >= HashBeastMetadata::LEN,
            ErrorCode::InvalidParameters
        );

        // Write the 8-byte discriminator (required by Anchor for account deserialization)
        // Anchor calculates discriminator as first 8 bytes of sha256("account:HashBeastMetadata")
        data[..8].copy_from_slice(<HashBeastMetadata as Discriminator>::DISCRIMINATOR);

        // Serialize struct data to a Vec, then copy to buffer after discriminator
        // This is more reliable than using Write trait directly on mutable slice
        let serialized = metadata_data
            .try_to_vec()
            .map_err(|_| ErrorCode::InvalidParameters)?;
        data[8..8 + serialized.len()].copy_from_slice(&serialized);

        // Emit event
        emit!(HashBeastMinted {
            hashbeast_metadata_account: hashbeast_metadata_key,
            hashbeast_asset_signer: hashbeast_asset_key,
            owner: ctx.accounts.user.key(),
            player: ctx.accounts.player_data.key(),
            mint: hashbeast_asset_key,
            name: name.clone(),
            uri: uri.clone(),
            dna,
            accumulated_val: 0,
            multiplier,
            faction_id,
            price: prices[index],
            ticket_tier: ticket_tier_index as u64,
            ticket_count,
        });

        hashbeast_config.total_hashbeasts_minted = hashbeast_config
            .total_hashbeasts_minted
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        hashbeast_mint_config.genesis_mints = hashbeast_mint_config
            .genesis_mints
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let faction_index = faction_id as usize;
        hashbeast_mint_config.genesis_mints_by_faction[faction_index] = hashbeast_mint_config
            .genesis_mints_by_faction[faction_index]
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    msg!(
        "✅ Batch minted {} HashBeast for faction {}",
        mint_count,
        faction_id
    );
    msg!(
        "   Total hashbeasts minted: {}",
        hashbeast_config.total_hashbeasts_minted
    );
    msg!(
        "   Genesis mints: {} / {}, faction {}: {} / {}",
        hashbeast_mint_config.genesis_mints,
        hashbeast_mint_config.genesis_mint_limit,
        faction_id,
        hashbeast_mint_config.genesis_mints_by_faction[faction_id as usize],
        hashbeast_mint_config.max_genesis_mints_per_faction
    );
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- ADMIN FREE MINT FUNCTION ------------------------------------------------
// ----------------------------------------------------------------------------------------

/// Admin function to mint a HashBeast NFT for free to a specified recipient
pub fn int_admin_mint_hashbeast(
    ctx: Context<AdminMintHashBeast>,
    recipient: Pubkey,
    faction_id: u8,
    ticket_tier_index: u8,
) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_admin_mint_hashbeast");
    let global_config = &ctx.accounts.global_config;
    let hashbeast_config = &mut ctx.accounts.hashbeast_config;
    let hashbeast_mint_config = &mut ctx.accounts.hashbeast_mint_config;

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
        hashbeast_mint_config.genesis_mints < hashbeast_mint_config.genesis_mint_limit,
        ErrorCode::InvalidParameters
    );
    validate_genesis_faction_cap(hashbeast_mint_config, faction_id, 1)?;

    msg!(
        "🎁 [admin_mint_hashbeast] Admin minting free hashbeast to recipient: {}",
        recipient
    );
    msg!("   Faction ID: {}", faction_id);
    let current_mint_number = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   HashBeast number: {}", current_mint_number);

    // Generate hashbeast data (DNA, name, URI, multiplier)
    let slot = Clock::get()?.slot;
    let (name, uri, dna, multiplier) = generate_hashbeast_data(
        current_mint_number,
        &recipient,
        slot,
        faction_id,
        &ctx.accounts.hashbeast_asset.key(),
    )?;

    // Get collection authority seeds
    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[
        crate::state::COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];

    // Create NFT via MPL Core CPI (paid by admin, sent to recipient)
    msg!("🎨 Creating HashBeast NFT via Metaplex Core CPI");
    msg!("   Name: {}", name);
    msg!("   URI: {}", uri);
    msg!("   Recipient: {}", recipient);

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
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
    let cost_per_hashbeast = crate::genescience::compute_gene_price(
        hashbeast_mint_config.base_price,
        hashbeast_mint_config.curve_a,
        hashbeast_mint_config.genesis_mints,
    )?;

    msg!(
        "   Calculated hashbeast price: {} lamports (for ticket calculation)",
        cost_per_hashbeast as f64 / 1e9
    );

    // Initialize HashBeast metadata
    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    hashbeast_metadata.mint = ctx.accounts.hashbeast_asset.key();
    hashbeast_metadata.mom = Pubkey::default();
    hashbeast_metadata.dad = Pubkey::default();
    hashbeast_metadata.breed_count = 0;
    hashbeast_metadata.rebirth_count = 0;
    hashbeast_metadata.cooldown_end = 0;
    hashbeast_metadata.accumulated_val = 0;
    hashbeast_metadata.dna = dna;
    hashbeast_metadata.incubated_player_data = Pubkey::default();
    hashbeast_metadata.multiplier = multiplier;
    hashbeast_metadata.faction_id = faction_id;
    hashbeast_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    hashbeast_metadata.created_at = Clock::get()?.unix_timestamp;
    hashbeast_metadata.xp = 0;
    hashbeast_metadata.bump = ctx.bumps.hashbeast_metadata;

    // Handle ticket tier selection and add free tickets (using actual price)
    let ticket_count = if !hashbeast_mint_config.ticket_tiers.is_empty() {
        add_tickets_to_player(
            &mut ctx.accounts.player_data,
            hashbeast_mint_config,
            ticket_tier_index,
            cost_per_hashbeast,
        )?
    } else {
        0
    };

    // Update hashbeast config stats
    hashbeast_config.total_hashbeasts_minted = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    hashbeast_mint_config.genesis_mints = hashbeast_mint_config
        .genesis_mints
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    hashbeast_mint_config.genesis_mints_by_faction[faction_id as usize] = hashbeast_mint_config
        .genesis_mints_by_faction[faction_id as usize]
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Total hashbeasts minted: {}",
        hashbeast_config.total_hashbeasts_minted
    );

    emit!(HashBeastMinted {
        hashbeast_metadata_account: hashbeast_metadata.key(),
        hashbeast_asset_signer: ctx.accounts.hashbeast_asset.key(),
        owner: recipient,
        player: ctx.accounts.player_data.key(),
        mint: hashbeast_metadata.mint,
        name,
        uri,
        dna,
        accumulated_val: 0,
        multiplier,
        faction_id,
        price: cost_per_hashbeast,
        ticket_tier: ticket_tier_index as u64,
        ticket_count,
    });

    msg!(
        "✅ Admin minted HashBeast #{} for faction {} to recipient {}",
        hashbeast_config.total_hashbeasts_minted,
        faction_id,
        recipient
    );
    Ok(())
}

/// User-callable free mint path backed by a per-user whitelist allowance.
/// The caller pays transaction fees and account rent, but not the HashBeast mint price.
pub fn int_whitelist_mint_hashbeast(
    ctx: Context<WhitelistMintHashBeast>,
    faction_id: u8,
    ticket_tier_index: u8,
) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_whitelist_mint_hashbeast");
    let global_config = &ctx.accounts.global_config;
    let hashbeast_config = &mut ctx.accounts.hashbeast_config;
    let hashbeast_mint_config = &mut ctx.accounts.hashbeast_mint_config;
    let player_data = &mut ctx.accounts.player_data;
    let allowance = &mut ctx.accounts.hashbeast_free_mint_allowance;
    let user = ctx.accounts.user.key();

    require!(!global_config.is_paused, ErrorCode::GamePaused);
    require!(
        hashbeast_mint_config.is_active,
        ErrorCode::MintingNotAllowed
    );
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require!(
        hashbeast_mint_config.genesis_mints < hashbeast_mint_config.genesis_mint_limit,
        ErrorCode::InvalidParameters
    );
    validate_genesis_faction_cap(hashbeast_mint_config, faction_id, 1)?;
    require!(allowance.user == user, ErrorCode::Unauthorized);
    require!(
        allowance.remaining_free_mints > 0,
        ErrorCode::NoFreeHashBeastMintsRemaining
    );
    require!(
        ctx.accounts.hashbeast_asset.is_signer,
        ErrorCode::Unauthorized
    );

    msg!(
        "🎁 [whitelist_mint_hashbeast] user={} faction_id={} remaining_before={} mint_number={}",
        user,
        faction_id,
        allowance.remaining_free_mints,
        hashbeast_config
            .total_hashbeasts_minted
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    );

    let current_mint_number = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let slot = Clock::get()?.slot;
    let (name, uri, dna, multiplier) = generate_hashbeast_data(
        current_mint_number,
        &user,
        slot,
        faction_id,
        &ctx.accounts.hashbeast_asset.key(),
    )?;

    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[
        crate::state::COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
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

    let notional_price = crate::genescience::compute_gene_price(
        hashbeast_mint_config.base_price,
        hashbeast_mint_config.curve_a,
        hashbeast_mint_config.genesis_mints,
    )?;
    msg!(
        "   [whitelist_mint_hashbeast] notional_price={} SOL ticket_tier_index={}",
        notional_price as f64 / 1e9,
        ticket_tier_index
    );

    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    hashbeast_metadata.mint = ctx.accounts.hashbeast_asset.key();
    hashbeast_metadata.mom = Pubkey::default();
    hashbeast_metadata.dad = Pubkey::default();
    hashbeast_metadata.breed_count = 0;
    hashbeast_metadata.rebirth_count = 0;
    hashbeast_metadata.cooldown_end = 0;
    hashbeast_metadata.accumulated_val = 0;
    hashbeast_metadata.dna = dna;
    hashbeast_metadata.incubated_player_data = Pubkey::default();
    hashbeast_metadata.multiplier = multiplier;
    hashbeast_metadata.faction_id = faction_id;
    hashbeast_metadata.last_update_ts = Clock::get()?.unix_timestamp;
    hashbeast_metadata.created_at = Clock::get()?.unix_timestamp;
    hashbeast_metadata.xp = 0;
    hashbeast_metadata.bump = ctx.bumps.hashbeast_metadata;

    let ticket_count = if hashbeast_mint_config.ticket_tiers.is_empty() {
        0
    } else {
        add_tickets_to_player(
            player_data,
            hashbeast_mint_config,
            ticket_tier_index,
            notional_price,
        )?
    };

    hashbeast_config.total_hashbeasts_minted = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    hashbeast_mint_config.genesis_mints = hashbeast_mint_config
        .genesis_mints
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    hashbeast_mint_config.genesis_mints_by_faction[faction_id as usize] = hashbeast_mint_config
        .genesis_mints_by_faction[faction_id as usize]
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    allowance.remaining_free_mints = allowance
        .remaining_free_mints
        .checked_sub(1)
        .ok_or(ErrorCode::NoFreeHashBeastMintsRemaining)?;

    msg!(
        "✅ [whitelist_mint_hashbeast] user={} minted={} remaining_after={} total_minted={}",
        user,
        ctx.accounts.hashbeast_asset.key(),
        allowance.remaining_free_mints,
        hashbeast_config.total_hashbeasts_minted
    );

    emit!(HashBeastMinted {
        hashbeast_metadata_account: hashbeast_metadata.key(),
        hashbeast_asset_signer: ctx.accounts.hashbeast_asset.key(),
        owner: user,
        player: player_data.key(),
        mint: hashbeast_metadata.mint,
        name,
        uri,
        dna,
        accumulated_val: 0,
        multiplier,
        faction_id,
        price: notional_price,
        ticket_tier: ticket_tier_index as u64,
        ticket_count,
    });

    Ok(())
}

/// Stake a HashBeast to boost hashpower (multiplier applies to the player's home-faction degenBTC and LP stakes).
/// The HashBeast's own faction does not matter for staking boosts.
///
/// Passive staking uses three-slot smoothing:
/// - no HashBeasts: 1.0x
/// - three 1.0x HashBeasts: 2.0x
/// - three strong HashBeasts: capped at 3.0x
///
/// This keeps Genesis HashBeasts useful while avoiding the old shape where one maxed
/// HashBeast could immediately consume the full passive cap.
/// The effective multiplier is capped at PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER for reward-share math.
/// Clients must pass metadata accounts for all already-staked hashbeasts in `remaining_accounts`
/// so the program can derive the exact pre-stake multiplier without storing extra state.
pub fn int_stake_hashbeast(ctx: Context<StakeHashBeast>) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_stake_hashbeast");
    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let current_time = Clock::get()?.unix_timestamp;
    let hashbeast_mint = hashbeast_metadata.mint;
    let hashbeast_multiplier = hashbeast_metadata.multiplier;
    msg!(
        "🧭 [stake_hashbeast] user={} player={} faction_state={} player_faction_id={} hashbeast_mint={} hashbeast_faction_id={} hashbeast_multiplier={}x",
        ctx.accounts.user.key(),
        player_data.key(),
        faction_state.key(),
        player_data.faction_id,
        hashbeast_mint,
        hashbeast_metadata.faction_id,
        hashbeast_multiplier as f64 / 1000.0
    );
    msg!(
        "🧾 [stake_hashbeast] player_before staked_hashbeasts={:?} hashbeast_multiplier={}x degenbtc_hashpower={} lp_hashpower={} pending_sol={} pending_dbtc={}",
        player_data.staked_hashbeasts,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_dbtc_rewards as f64 / 1e6
    );
    let prev_faction_degenbtc_hashpower = faction_state.total_degenbtc_hashpower;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    let prev_faction_hashbeasts_staked = faction_state.hashbeasts_staked;
    msg!(
        "🧾 [stake_hashbeast] faction_before degenbtc_hashpower={} lp_hashpower={} hashbeasts_staked={}",
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        prev_faction_lp_hashpower as f64 / 1e6,
        prev_faction_hashbeasts_staked
    );

    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.hashbeast_asset)?;

    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );
    // Check if already incubated (using Pubkey::default() instead of None)
    require!(
        hashbeast_metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::HashBeastAlreadyAtGuard
    );
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.staked_hashbeasts.len() < MAX_STAKED_HASHBEASTS,
        ErrorCode::InvalidParameters
    );

    // Transfer NFT to custody PDA (lock it)
    msg!("🔒 Transferring NFT to custody PDA (locking)");
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.hashbeast_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;

    // Process pending rewards before updating position
    let (_new_sol_rewards, _new_dbtc_rewards) =
        stake::int_update_dbtc_staking_rewards(player_data, faction_state)?;
    let (_new_sol_rewards, _new_dbtc_rewards) =
        stake::int_update_lp_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [stake_hashbeast] pending_after_reward_sync sol={} staking_degenBTC={} gameplay_degenBTC={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_dbtc_rewards as f64 / 1e6,
        player_data.pending_dbtc_rewards as f64 / 1e6
    );

    // Derive the exact multiplier from currently staked hashbeasts so cap-hit flows remain reversible
    // without storing extra player state.
    let existing_staked_hashbeasts = player_data.staked_hashbeasts.clone();
    let existing_raw_multiplier =
        load_staked_hashbeast_raw_multiplier(ctx.remaining_accounts, &existing_staked_hashbeasts)?;
    let old_multiplier = capped_player_multiplier(existing_raw_multiplier) as u64;
    require!(
        player_data.hashbeast_multiplier == old_multiplier as u16,
        ErrorCode::InvalidState
    );
    let (new_raw_multiplier, new_effective_multiplier) =
        add_hashbeast_multiplier(existing_raw_multiplier, hashbeast_multiplier)?;
    msg!(
        "⚙️ [stake_hashbeast] multiplier_math existing_raw={}x old_effective={}x added={}x new_raw={}x new_effective={}x",
        existing_raw_multiplier as f64 / 1000.0,
        old_multiplier as f64 / 1000.0,
        hashbeast_multiplier as f64 / 1000.0,
        new_raw_multiplier as f64 / 1000.0,
        new_effective_multiplier as f64 / 1000.0
    );

    // Add hashbeast to player's staked hashbeasts list after validating the previous multiplier state.
    player_data.staked_hashbeasts.push(hashbeast_mint);
    player_data.hashbeast_multiplier = new_effective_multiplier;
    msg!(
        "⚡ Updated hashbeast multiplier: effective=({})x raw_total=({})x",
        player_data.hashbeast_multiplier as f64 / 1000.0,
        new_raw_multiplier as f64 / 1000.0
    );

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_degenbtc_hashpower = player_data.degenbtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;

    // Recalculate hashpower with new multiplier (multiply first to avoid precision loss)
    // Formula: new_hashpower = (old_hashpower * new_multiplier) / old_multiplier
    let new_multiplier = player_data.hashbeast_multiplier as u64;
    if old_multiplier > 0 {
        player_data.degenbtc_hashpower = scale_hashpower_by_multiplier(
            existing_degenbtc_hashpower,
            new_multiplier,
            old_multiplier,
        )?;
        player_data.lp_hashpower =
            scale_hashpower_by_multiplier(existing_lp_hashpower, new_multiplier, old_multiplier)?;
    } else {
        // If old_multiplier is 0 (shouldn't happen), use new_multiplier directly
        player_data.degenbtc_hashpower = scale_hashpower_by_multiplier(
            existing_degenbtc_hashpower,
            new_multiplier,
            BASE_MULTIPLIER as u64,
        )?;
        player_data.lp_hashpower = scale_hashpower_by_multiplier(
            existing_lp_hashpower,
            new_multiplier,
            BASE_MULTIPLIER as u64,
        )?;
    }
    msg!(
        "   degenBTC hashpower: {} -> {}",
        existing_degenbtc_hashpower as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "   LP hashpower: {} -> {}",
        existing_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Update faction state totals
    update_faction_hashpower(
        faction_state,
        existing_degenbtc_hashpower,
        player_data.degenbtc_hashpower,
        existing_lp_hashpower,
        player_data.lp_hashpower,
    )?;
    msg!(
        "   Faction degenBTC hashpower: {} -> {}",
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "   Faction LP hashpower: {} -> {}",
        prev_faction_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    faction_state.hashbeasts_staked = faction_state
        .hashbeasts_staked
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Faction hashbeasts staked: {} -> {}",
        prev_faction_hashbeasts_staked,
        faction_state.hashbeasts_staked
    );

    // Update hashbeast metadata
    // Set new owner (using Pubkey instead of Option)
    hashbeast_metadata.incubated_player_data = player_data.owner;
    hashbeast_metadata.last_update_ts = current_time;
    msg!("   HashBeast metadata updated");

    // Emit event for indexing
    emit!(HashBeastStaked {
        owner: ctx.accounts.user.key(),
        player: player_data.key(),
        hashbeast_mint,
        hashbeast_metadata_account: hashbeast_metadata.key(),
        player_multiplier: player_data.hashbeast_multiplier,
        degenbtc_hashpower: player_data.degenbtc_hashpower,
        lp_hashpower: player_data.lp_hashpower,
        timestamp: current_time,
    });

    Ok(())
}

/// Unstake a HashBeast (reduces multiplier and recalculates hashpower)
/// Clients must pass metadata accounts for all hashbeasts that remain staked after this unstake
/// in `remaining_accounts` so the program can derive the exact post-unstake multiplier.
pub fn int_unstake_hashbeast(ctx: Context<UnstakeHashBeast>) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_unstake_hashbeast");
    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let hashbeast_mint = hashbeast_metadata.mint;
    let incubated_by_player = hashbeast_metadata.incubated_player_data;
    let current_time = Clock::get()?.unix_timestamp;
    let hashbeast_multiplier = hashbeast_metadata.multiplier;
    msg!(
        "🧭 [unstake_hashbeast] user={} player={} faction_state={} player_faction_id={} hashbeast_mint={} hashbeast_faction_id={} hashbeast_multiplier={}x",
        ctx.accounts.user.key(),
        player_data.key(),
        faction_state.key(),
        player_data.faction_id,
        hashbeast_mint,
        hashbeast_metadata.faction_id,
        hashbeast_multiplier as f64 / 1000.0
    );
    msg!(
        "🧾 [unstake_hashbeast] player_before staked_hashbeasts={:?} hashbeast_multiplier={}x degenbtc_hashpower={} lp_hashpower={} pending_sol={} pending_degenBTC={}",
        player_data.staked_hashbeasts,
        player_data.hashbeast_multiplier as f64 / 1000.0,
        player_data.degenbtc_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6,
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_dbtc_rewards as f64 / 1e6
    );
    let prev_faction_degenbtc_hashpower = faction_state.total_degenbtc_hashpower;
    let prev_faction_lp_hashpower = faction_state.total_lp_hashpower;
    let prev_faction_hashbeasts_staked = faction_state.hashbeasts_staked;
    msg!(
        "🧾 [unstake_hashbeast] faction_before degenbtc_hashpower={} lp_hashpower={} hashbeasts_staked={}",
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        prev_faction_lp_hashpower as f64 / 1e6,
        prev_faction_hashbeasts_staked
    );

    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.hashbeast_asset)?;
    require!(
        nft_owner == ctx.accounts.hashbeast_custody_pda.key(),
        ErrorCode::HashBeastNotAtGuard
    );
    // Verify ownership (using Pubkey::default() check instead of is_some())
    require!(
        hashbeast_metadata.incubated_player_data != Pubkey::default(),
        ErrorCode::HashBeastNotAtGuard
    );
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.staked_hashbeasts.contains(&hashbeast_mint),
        ErrorCode::InvalidParameters
    );
    require!(
        incubated_by_player == player_data.owner,
        ErrorCode::Unauthorized
    );

    // Process pending rewards before updating position
    let (_new_sol_rewards, _new_dbtc_rewards) =
        stake::int_update_dbtc_staking_rewards(player_data, faction_state)?;
    let (_new_sol_rewards, _new_dbtc_rewards) =
        stake::int_update_lp_staking_rewards(player_data, faction_state)?;
    msg!(
        "💹 [unstake_hashbeast] pending_after_reward_sync sol={} staking_degenBTC={} gameplay_degenBTC={}",
        player_data.pending_sol_rewards as f64 / 1e9,
        player_data.pending_staking_dbtc_rewards as f64 / 1e6,
        player_data.pending_dbtc_rewards as f64 / 1e6
    );

    // Build the expected post-unstake hashbeast set before mutating state so we can validate
    // `remaining_accounts` against it.
    let mut remaining_staked_hashbeasts = player_data.staked_hashbeasts.clone();
    if let Some(index) = remaining_staked_hashbeasts
        .iter()
        .position(|&mint| mint == hashbeast_mint)
    {
        remaining_staked_hashbeasts.remove(index);
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // Derive the exact pre/post unstake multipliers from HashBeast metadata instead of stored raw state.
    let remaining_raw_multiplier =
        load_staked_hashbeast_raw_multiplier(ctx.remaining_accounts, &remaining_staked_hashbeasts)?;
    let current_raw_multiplier = remaining_raw_multiplier
        .checked_add(hashbeast_multiplier as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let old_multiplier = capped_player_multiplier(current_raw_multiplier) as u64;
    require!(
        player_data.hashbeast_multiplier == old_multiplier as u16,
        ErrorCode::InvalidState
    );

    // Remove hashbeast from player's staked hashbeasts list
    if let Some(index) = player_data
        .staked_hashbeasts
        .iter()
        .position(|&mint| mint == hashbeast_mint)
    {
        player_data.staked_hashbeasts.remove(index);
        msg!(
            "   Removed hashbeast from staked hashbeasts. Remaining: {}",
            player_data.staked_hashbeasts.len()
        );
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // Derive the new effective multiplier from the exact remaining raw sum.
    let (_, new_effective_multiplier) =
        remove_hashbeast_multiplier(current_raw_multiplier, hashbeast_multiplier)?;
    player_data.hashbeast_multiplier = new_effective_multiplier;
    msg!(
        "⚙️ [unstake_hashbeast] multiplier_math current_raw={}x old_effective={}x removed={}x remaining_raw={}x new_effective={}x",
        current_raw_multiplier as f64 / 1000.0,
        old_multiplier as f64 / 1000.0,
        hashbeast_multiplier as f64 / 1000.0,
        remaining_raw_multiplier as f64 / 1000.0,
        new_effective_multiplier as f64 / 1000.0
    );
    msg!(
        "⚡ Updated hashbeast multiplier: effective=({})x raw_total=({})x",
        player_data.hashbeast_multiplier as f64 / 1000.0,
        remaining_raw_multiplier as f64 / 1000.0
    );

    // Calculate new hashpower based on new multiplier and UPDATE
    let existing_degenbtc_hashpower = player_data.degenbtc_hashpower;
    let existing_lp_hashpower = player_data.lp_hashpower;
    let new_multiplier = player_data.hashbeast_multiplier as u64;

    if old_multiplier > 0 {
        player_data.degenbtc_hashpower = scale_hashpower_by_multiplier(
            existing_degenbtc_hashpower,
            new_multiplier,
            old_multiplier,
        )?;
        player_data.lp_hashpower =
            scale_hashpower_by_multiplier(existing_lp_hashpower, new_multiplier, old_multiplier)?;
    }
    msg!(
        "   degenBTC hashpower: {} -> {}",
        existing_degenbtc_hashpower as f64 / 1e6,
        player_data.degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "   LP hashpower: {} -> {}",
        existing_lp_hashpower as f64 / 1e6,
        player_data.lp_hashpower as f64 / 1e6
    );

    // Update faction state totals
    update_faction_hashpower(
        faction_state,
        existing_degenbtc_hashpower,
        player_data.degenbtc_hashpower,
        existing_lp_hashpower,
        player_data.lp_hashpower,
    )?;
    msg!(
        "   Faction degenBTC hashpower: {} -> {}",
        prev_faction_degenbtc_hashpower as f64 / 1e6,
        faction_state.total_degenbtc_hashpower as f64 / 1e6
    );
    msg!(
        "   Faction LP hashpower: {} -> {}",
        prev_faction_lp_hashpower as f64 / 1e6,
        faction_state.total_lp_hashpower as f64 / 1e6
    );

    faction_state.hashbeasts_staked = faction_state
        .hashbeasts_staked
        .checked_sub(1)
        .ok_or(ErrorCode::InvalidState)?;
    msg!(
        "   Faction hashbeasts staked: {} -> {}",
        prev_faction_hashbeasts_staked,
        faction_state.hashbeasts_staked
    );

    // Update hashbeast metadata
    // Clear owner (Set back to default using Pubkey::default() instead of None)
    hashbeast_metadata.incubated_player_data = Pubkey::default();
    hashbeast_metadata.last_update_ts = current_time;
    msg!("   HashBeast metadata updated");

    // Transfer NFT back to user (unlock it)
    msg!("🔓 Transferring NFT back to user (unlocking)");
    let custody_seeds = &[HASHBEAST_CUSTODY_SEED, &[ctx.bumps.hashbeast_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];

    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.hashbeast_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;

    // Emit event for indexing
    emit!(HashBeastUnstaked {
        owner: ctx.accounts.user.key(),
        player: player_data.key(),
        hashbeast_mint,
        hashbeast_metadata_account: hashbeast_metadata.key(),
        player_multiplier: player_data.hashbeast_multiplier,
        degenbtc_hashpower: player_data.degenbtc_hashpower,
        lp_hashpower: player_data.lp_hashpower,
        timestamp: current_time,
    });

    Ok(())
}

/// Rebirth a HashBeast into the program-owned lootbox inventory, or burn it when
/// the inventory path cannot accept another rebirth.
///
/// Behavior:
/// 1. Pays the user any `accumulated_val` they had earned (same as before).
/// 2. If the NFT has already hit `MAX_REBIRTH_COUNT`, burns it.
/// 3. Otherwise increments rebirth_count, rerolls fresh DNA, and resets
///    gameplay state: multiplier, xp, accumulated_val, breed_count, cooldown,
///    and parent lineage.
/// 4. Transfers the mpl-core asset from the user to `inventory_pda` (= the
///    `InventoryPool` account, which doubles as the global custody address).
/// 5. If the country's lootbox queue has room, initializes a `RebornEntry`
///    with status Lootbox and pushes the asset into that queue.
/// 6. Bumps inventory pool counters and emits `HashBeastReborn`.
pub fn int_rebirth_hashbeast(ctx: Context<RebirthHashBeast>) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_rebirth_hashbeast");
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    let current_slot = clock.slot;

    let asset_key = ctx.accounts.hashbeast_asset.key();
    let metadata = &ctx.accounts.hashbeast_metadata;
    let accumulated_val = metadata.accumulated_val;
    let multiplier_before = metadata.multiplier;
    let xp_before = metadata.xp;
    let breed_count = metadata.breed_count;
    let faction_id = metadata.faction_id;
    let previous_dna = metadata.dna;
    let rebirth_count_before = metadata
        .rebirth_count
        .max(crate::genescience::get_rebirth_count(&metadata.dna));
    let user_key = ctx.accounts.user.key();

    require!(
        metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::HashBeastAlreadyAtGuard
    );

    msg!(
        "♻️  Rebirthing HashBeast — accumulated_val={}",
        accumulated_val
    );
    msg!(
        "   Pre-rebirth stats: multiplier={} xp={} breed_count={} rebirth_count={} faction_id={}",
        multiplier_before,
        xp_before,
        breed_count,
        rebirth_count_before,
        faction_id
    );

    // 1) Always pay the user their accumulated_val first.
    if accumulated_val > 0 {
        msg!("💸 Transferring {} degenBTC to user", accumulated_val);
        let seeds = &[
            DEGEN_BTC_VAULT_AUTHORITY_SEED,
            &[ctx.accounts.dbtc_mining.vault_auth_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.dbtc_token_vault.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer_seeds,
            ),
            accumulated_val,
            DBTC_DECIMALS,
        )?;

        let mining_state = &mut ctx.accounts.dbtc_mining;
        mining_state.total_tokens_distributed = mining_state
            .total_tokens_distributed
            .checked_add(accumulated_val)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    // 2) Decide cascade: country lootbox queue first, else burn.
    let queue_has_space = (ctx.accounts.lootbox_queue.filled_count as usize) < LOOTBOX_QUEUE_SIZE;
    let inventory_has_capacity = ctx.accounts.inventory_pool.total_count < MAX_INVENTORY;
    let rebirth_cap_reached = rebirth_count_before >= MAX_REBIRTH_COUNT;

    if queue_has_space && inventory_has_capacity && !rebirth_cap_reached {
        // -------- Path A: push into country lootbox queue --------
        let quality_score = compute_quality_score(multiplier_before, xp_before, breed_count);
        let next_rebirth_count = rebirth_count_before
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let new_dna = crate::genescience::generate_reborn_dna(
            &previous_dna,
            &asset_key,
            current_slot,
            faction_id,
            next_rebirth_count,
        )?;

        // Reset HashBeastMetadata fields BEFORE the asset leaves the user. Once
        // pushed into the queue and won by another player, that player gets
        // a reborn creature: fresh DNA and default gameplay state, while the
        // same asset address carries the rebirth generation forward.
        {
            let metadata_mut = &mut ctx.accounts.hashbeast_metadata;
            metadata_mut.reset_for_rebirth(new_dna, next_rebirth_count, current_time);
        }

        // Transfer asset user → inventory_pda (mpl-core).
        crate::mpl_core_helpers::transfer_mpl_core_asset(
            &ctx.accounts.hashbeast_asset.to_account_info(),
            ctx.accounts
                .hashbeast_collection
                .as_ref()
                .map(|c| c.to_account_info())
                .as_ref(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.inventory_pda.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            None,
        )?;

        // Manually init the RebornEntry PDA (Anchor's `#[account(init)]`
        // can't be conditional). We create only on the queue path.
        {
            let entry_info = ctx.accounts.reborn_entry.to_account_info();
            let asset_key_bytes = asset_key.to_bytes();
            let bump = ctx.bumps.reborn_entry;
            let entry_seeds: &[&[u8]] = &[
                REBORN_ENTRY_SEED,
                asset_key_bytes.as_ref(),
                core::slice::from_ref(&bump),
            ];
            let blank = RebornEntry {
                bump,
                asset: asset_key,
                faction_id,
                quality_score,
                reborn_at: current_time,
                status: RebornStatus::Lootbox as u8,
                listing_price: 0,
                origin: RebornOrigin::Reborn as u8,
                original_buy_price: 0,
                expire_count: 0,
            };
            crate::instructions::helper::init_pda_account_if_needed::<RebornEntry>(
                &ctx.accounts.user.to_account_info(),
                &entry_info,
                &ctx.accounts.system_program.to_account_info(),
                entry_seeds,
                RebornEntry::LEN,
                &blank,
            )?;
        }

        // Push asset into the next slot in the country queue.
        let depth_after = {
            let queue = &mut ctx.accounts.lootbox_queue;
            let idx = queue.filled_count as usize;
            queue.slots[idx] = asset_key;
            queue.filled_count = queue
                .filled_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            queue.filled_count
        };

        // Bump pool counter (only total_count remains; per-status / lifetime
        // counters were dropped in the permissionless refactor).
        {
            let pool = &mut ctx.accounts.inventory_pool;
            pool.total_count = pool
                .total_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        emit!(HashBeastReborn {
            asset: asset_key,
            former_owner: user_key,
            accumulated_val,
            quality_score,
            rebirth_count: next_rebirth_count,
            new_dna,
            timestamp: current_time,
        });
        emit!(LootboxQueuePush {
            faction_id,
            asset: asset_key,
            queue_depth_after: depth_after,
            source: 0, // rebirth
            timestamp: current_time,
        });
        msg!(
            "✅ [rebirth_hashbeast] queued in faction {} at depth {}",
            faction_id,
            depth_after
        );
    } else {
        // -------- Path B: queue/cap full → burn the asset --------
        // No RebornEntry init, no inventory_pool counter changes.
        // User already received their accumulated_val. Asset is gone.
        let burn_reason = if rebirth_cap_reached { 1 } else { 0 };
        crate::mpl_core_helpers::burn_mpl_core_asset(
            &ctx.accounts.hashbeast_asset.to_account_info(),
            ctx.accounts
                .hashbeast_collection
                .as_ref()
                .map(|c| c.to_account_info())
                .as_ref(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.mpl_core_program.to_account_info(),
            None,
        )?;

        emit!(HashBeastRebirthBurned {
            asset: asset_key,
            former_owner: user_key,
            faction_id,
            accumulated_val,
            rebirth_count: rebirth_count_before,
            reason: burn_reason,
            timestamp: current_time,
        });
        msg!(
            "🔥 [rebirth_hashbeast] asset burned for faction {} reason={} rebirth_count={}",
            faction_id,
            burn_reason,
            rebirth_count_before
        );
    }

    Ok(())
}

/// Breed two hashbeasts to create offspring (both parents must not be incubated, same faction)
pub fn int_breed_hashbeasts(ctx: Context<BreedHashBeast>) -> Result<()> {
    crate::log_fn!("hashbeasts", "int_breed_hashbeasts");
    require!(!ctx.accounts.global_config.is_paused, ErrorCode::GamePaused);
    // Block self-breeding: passing the same asset as mom and dad would
    // increment breed_count only once (second metadata mutation overwrites
    // the first on serialize), bypassing the two-parent requirement.
    require!(
        ctx.accounts.mom_asset.key() != ctx.accounts.dad_asset.key(),
        ErrorCode::InvalidParameters
    );
    let hashbeast_config = &mut ctx.accounts.hashbeast_config;
    let mom = &mut ctx.accounts.mom_metadata;
    let dad = &mut ctx.accounts.dad_metadata;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    msg!("🧬 === BREEDING HASHBEASTS ===");
    msg!("   Mom: {} (breed_count: {})", mom.mint, mom.breed_count);
    msg!("   Dad: {} (breed_count: {})", dad.mint, dad.breed_count);

    // Validate breeding is allowed
    require!(
        hashbeast_config.breeding_allowed,
        ErrorCode::BreedingNotAllowed
    );
    let hashbeast_mint_config_info = ctx.accounts.hashbeast_mint_config.to_account_info();
    let hashbeast_mint_config: HashBeastMintConfig =
        load_program_account(&hashbeast_mint_config_info)?;
    let floor_history_info = ctx.accounts.floor_history.to_account_info();
    let floor_history: FloorHistory = load_program_account(&floor_history_info)?;
    require!(
        hashbeast_mint_config.genesis_mints >= hashbeast_mint_config.genesis_mint_limit,
        ErrorCode::GenesisNotSoldOut
    );
    assert_player_data_owner(
        &ctx.accounts.player_data.to_account_info(),
        &ctx.accounts.user.key(),
    )?;
    // Validate parents are not incubated
    require!(
        mom.incubated_player_data == Pubkey::default(),
        ErrorCode::HashBeastAlreadyAtGuard
    );
    require!(
        dad.incubated_player_data == Pubkey::default(),
        ErrorCode::HashBeastAlreadyAtGuard
    );

    // Validate same faction
    require!(
        mom.faction_id == dad.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Validate pair identity / close lineage.
    require!(mom.mint != dad.mint, ErrorCode::InvalidBreedingPair);
    require!(
        mom.mom != dad.mint && mom.dad != dad.mint && dad.mom != mom.mint && dad.dad != mom.mint,
        ErrorCode::InvalidBreedingPair
    );
    require!(
        !shares_known_parent(mom.as_ref(), dad.as_ref()),
        ErrorCode::InvalidBreedingPair
    );

    // Reborn-generation rule: a level-N reborn HashBeast can only breed with
    // another HashBeast from the same country and the same rebirth generation.
    let mom_rebirth_count = metadata_rebirth_count(mom.as_ref());
    let dad_rebirth_count = metadata_rebirth_count(dad.as_ref());
    require!(
        mom_rebirth_count == dad_rebirth_count,
        ErrorCode::RebirthLevelMismatch
    );

    // Validate breed counts
    require!(
        mom.breed_count < HashBeastMetadata::MAX_BREED_COUNT,
        ErrorCode::MaxBreedCountReached
    );
    require!(
        dad.breed_count < HashBeastMetadata::MAX_BREED_COUNT,
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

    // Calculate breeding cost. The total price is always at least 1.5x the
    // current marketplace floor anchor, so breeding cannot mint below floor.
    let curve_price = crate::genescience::compute_gene_price(
        hashbeast_config.breed_base_price,
        hashbeast_config.breed_curve_a,
        hashbeast_config.total_hashbeasts_minted,
    )?;
    let floor_anchor = floor_history.current_anchor();
    require!(
        floor_anchor >= SWEEP_MIN_ANCHOR_LAMPORTS,
        ErrorCode::BreedFloorAnchorUnavailable
    );
    let floor_min_price = ceil_mul_div_u64(
        floor_anchor,
        BREED_FLOOR_MULTIPLIER_BPS,
        BASIS_POINTS_DENOMINATOR,
    )?;
    let breed_cost = curve_price.max(floor_min_price);
    msg!(
        "   Breed cost: {} SOL curve={} floor_anchor={} floor_min={} total_minted_before={}",
        breed_cost as f64 / 1e9,
        curve_price,
        floor_anchor,
        floor_min_price,
        hashbeast_config.total_hashbeasts_minted
    );

    // Payment split: total breed price is 50% SOL and 50% dbTC by SOL value.
    // SOL leg: 25% fee_recipient, 75% SOL treasury.
    // dbTC leg: 50% burned, 50% returned to the mining emission vault.
    let sol_due = ceil_mul_div_u64(breed_cost, BREED_SOL_SHARE_BPS, BASIS_POINTS_DENOMINATOR)?;
    let dbtc_value_lamports = breed_cost
        .checked_sub(sol_due)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let dbtc_price_lamports = ctx.accounts.dbtc_mining.recent_price;
    require!(dbtc_price_lamports > 0, ErrorCode::DbtcPriceUnavailable);
    let dbtc_due = ceil_mul_div_u64(dbtc_value_lamports, DBTC_BASE_UNITS, dbtc_price_lamports)?;
    require!(dbtc_due > 0, ErrorCode::InvalidAmount);

    let sol_fee_recipient = ceil_mul_div_u64(
        sol_due,
        BREED_SOL_FEE_RECIPIENT_BPS,
        BASIS_POINTS_DENOMINATOR,
    )?;
    let sol_treasury = sol_due
        .checked_sub(sol_fee_recipient)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let dbtc_burn = ceil_mul_div_u64(dbtc_due, BREED_DBTC_BURN_BPS, BASIS_POINTS_DENOMINATOR)?;
    let dbtc_to_vault = dbtc_due
        .checked_sub(dbtc_burn)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if sol_fee_recipient > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.fee_recipient.to_account_info(),
                },
            ),
            sol_fee_recipient,
        )?;
    }
    if sol_treasury > 0 {
        helper::transfer_to_sol_treasury(
            &ctx.accounts.user.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            sol_treasury,
        )?;
    }

    if dbtc_burn > 0 {
        token_2022::burn(
            CpiContext::new(
                ctx.accounts.token_program_2022.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            dbtc_burn,
        )?;
    }
    if dbtc_to_vault > 0 {
        token_2022::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program_2022.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.dbtc_token_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            dbtc_to_vault,
            DBTC_DECIMALS,
        )?;
    }
    msg!(
        "   Breed payment: sol_due={} fee_recipient={} treasury={} dbtc_due={} burned={} vault={} dbtc_price={}",
        sol_due,
        sol_fee_recipient,
        sol_treasury,
        dbtc_due,
        dbtc_burn,
        dbtc_to_vault,
        dbtc_price_lamports
    );

    // Generate offspring DNA
    let seed = [
        clock.slot.to_le_bytes().as_ref(),
        ctx.accounts.user.key().as_ref(),
        mom.mint.as_ref(),
        dad.mint.as_ref(),
    ]
    .concat();
    let mut offspring_dna = crate::genescience::breed_genes(&mom.dna, &dad.dna, &seed)?;
    crate::genescience::set_rebirth_count(&mut offspring_dna, mom_rebirth_count)?;

    // Create offspring NFT
    let current_mint_number = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let name = format!("Bitcoin hashbeasts #{}", current_mint_number);
    let uri = format!(
        "https://assets.minebtc.fun/hashbeasts/{}.json",
        ctx.accounts.offspring_asset.key()
    );

    let collection_authority_bump = ctx.bumps.collection_authority;
    let collection_authority_seeds = &[COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];

    crate::mpl_core_helpers::create_mpl_core_asset(
        &ctx.accounts.offspring_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
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
    offspring.rebirth_count = mom_rebirth_count;
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
    let mom_cooldown = HashBeastMetadata::COOLDOWNS
        .get(mom.breed_count as usize)
        .copied()
        .unwrap_or(1209600);
    let dad_cooldown = HashBeastMetadata::COOLDOWNS
        .get(dad.breed_count as usize)
        .copied()
        .unwrap_or(1209600);

    mom.breed_count = mom
        .breed_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    mom.cooldown_end = current_time
        .checked_add(mom_cooldown)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    dad.breed_count = dad
        .breed_count
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    dad.cooldown_end = current_time
        .checked_add(dad_cooldown)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    hashbeast_config.total_hashbeasts_minted = hashbeast_config
        .total_hashbeasts_minted
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "✅ Bred offspring #{} from {} x {}",
        current_mint_number,
        mom.mint,
        dad.mint
    );
    msg!(
        "   Total hashbeasts minted after breed: {}",
        hashbeast_config.total_hashbeasts_minted
    );
    msg!(
        "   Mom next cooldown: {}s, Dad next cooldown: {}s",
        mom_cooldown,
        dad_cooldown
    );

    emit!(HashBeastMinted {
        hashbeast_metadata_account: offspring.key(),
        hashbeast_asset_signer: ctx.accounts.offspring_asset.key(),
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
    emit!(HashBeastBred {
        breeder: ctx.accounts.user.key(),
        mom: mom.mint,
        dad: dad.mint,
        offspring: offspring.mint,
        faction_id: mom.faction_id,
        rebirth_count: mom_rebirth_count,
        curve_price_lamports: curve_price,
        floor_anchor_lamports: floor_anchor,
        floor_min_price_lamports: floor_min_price,
        total_price_lamports: breed_cost,
        sol_paid_lamports: sol_due,
        sol_fee_recipient_lamports: sol_fee_recipient,
        sol_treasury_lamports: sol_treasury,
        dbtc_price_lamports,
        dbtc_paid: dbtc_due,
        dbtc_burned: dbtc_burn,
        dbtc_to_vault,
        timestamp: current_time,
    });

    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HELPER FUNCTIONS ---------------------------------------------------------
// ----------------------------------------------------------------------------------------

/// Generate hashbeast data (DNA, name, URI, multiplier) for a new hashbeast
pub fn generate_hashbeast_data(
    mint_number: u64,
    user_key: &Pubkey,
    slot_offset: u64,
    faction_id: u8,
    asset_key: &Pubkey,
) -> Result<(String, String, [u8; 32], u32)> {
    crate::log_fn!("hashbeasts", "generate_hashbeast_data");
    let dna = crate::genescience::generate_genesis_dna(
        mint_number,
        user_key,
        Clock::get()?.slot + slot_offset,
        faction_id,
    )?;
    let name = format!("Bitcoin hashbeasts #{}", mint_number);
    let uri = format!("https://assets.minebtc.fun/hashbeasts/{}.json", asset_key);
    let multiplier = BASE_MULTIPLIER;

    Ok((name, uri, dna, multiplier))
}

/// Add tickets to player based on price and ticket tier
fn add_tickets_to_player(
    player_data: &mut PlayerData,
    hashbeast_mint_config: &HashBeastMintConfig,
    ticket_tier_index: u8,
    price: u64,
) -> Result<u64> {
    require!(
        (ticket_tier_index as usize) < hashbeast_mint_config.ticket_tiers.len(),
        ErrorCode::InvalidParameters
    );
    require!(
        hashbeast_mint_config.ticket_tiers.len() == 3,
        ErrorCode::InvalidParameters
    );

    let selected_tier = &hashbeast_mint_config.ticket_tiers[ticket_tier_index as usize];
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
        player_data.free_tickets_remaining[index] += ticket_count;
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
    old_degenbtc_hashpower: u64,
    new_degenbtc_hashpower: u64,
    old_lp_hashpower: u64,
    new_lp_hashpower: u64,
) -> Result<()> {
    msg!(
        "🧮 [update_faction_hashpower] faction_id={} degenbtc_delta={} -> {} lp_delta={} -> {}",
        faction_state.faction_id,
        old_degenbtc_hashpower as f64 / 1e6,
        new_degenbtc_hashpower as f64 / 1e6,
        old_lp_hashpower as f64 / 1e6,
        new_lp_hashpower as f64 / 1e6
    );
    faction_state.total_degenbtc_hashpower = faction_state
        .total_degenbtc_hashpower
        .checked_sub(old_degenbtc_hashpower)
        .ok_or(ErrorCode::InvalidState)?
        .checked_add(new_degenbtc_hashpower)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_lp_hashpower = faction_state
        .total_lp_hashpower
        .checked_sub(old_lp_hashpower)
        .ok_or(ErrorCode::InvalidState)?
        .checked_add(new_lp_hashpower)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok(())
}

fn capped_player_multiplier(raw_multiplier: u64) -> u16 {
    let raw_hashbeast_sum = raw_multiplier.saturating_sub(BASE_MULTIPLIER as u64);
    let smoothed_multiplier = BASE_MULTIPLIER as u64
        + raw_hashbeast_sum
            .checked_div(MAX_STAKED_HASHBEASTS as u64)
            .unwrap_or(0);
    smoothed_multiplier.min(PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER as u64) as u16
}

fn scale_hashpower_by_multiplier(
    hashpower: u64,
    new_multiplier: u64,
    old_multiplier: u64,
) -> Result<u64> {
    require!(old_multiplier > 0, ErrorCode::InvalidParameters);
    let scaled = (hashpower as u128)
        .checked_mul(new_multiplier as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(old_multiplier as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    u64::try_from(scaled).map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

fn add_hashbeast_multiplier(
    existing_raw_multiplier: u64,
    hashbeast_multiplier: u32,
) -> Result<(u64, u16)> {
    let new_raw_multiplier = existing_raw_multiplier
        .checked_add(hashbeast_multiplier as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok((
        new_raw_multiplier,
        capped_player_multiplier(new_raw_multiplier),
    ))
}

fn remove_hashbeast_multiplier(
    existing_raw_multiplier: u64,
    hashbeast_multiplier: u32,
) -> Result<(u64, u16)> {
    let reduced_raw_multiplier = existing_raw_multiplier
        .checked_sub(hashbeast_multiplier as u64)
        .ok_or(ErrorCode::InvalidState)?;
    let new_raw_multiplier = reduced_raw_multiplier.max(BASE_MULTIPLIER as u64);
    Ok((
        new_raw_multiplier,
        capped_player_multiplier(new_raw_multiplier),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_hashbeast_metadata() -> HashBeastMetadata {
        HashBeastMetadata {
            mint: Pubkey::default(),
            mom: Pubkey::default(),
            dad: Pubkey::default(),
            breed_count: 0,
            rebirth_count: 0,
            cooldown_end: 0,
            created_at: 0,
            faction_id: 0,
            multiplier: BASE_MULTIPLIER,
            accumulated_val: 0,
            dna: [0u8; 32],
            incubated_player_data: Pubkey::default(),
            last_update_ts: 0,
            xp: 0,
            bump: 0,
        }
    }

    // ------------------------------------------------------------------------
    // Existing tests
    // ------------------------------------------------------------------------

    #[test]
    fn multiplier_cap_is_reversible_with_raw_sum() {
        let starting_raw = BASE_MULTIPLIER as u64 + (1_900u64 * MAX_STAKED_HASHBEASTS as u64);
        let (raw_after_stake, effective_after_stake) =
            add_hashbeast_multiplier(starting_raw, 1_000).unwrap();
        assert_eq!(
            raw_after_stake,
            BASE_MULTIPLIER as u64 + (1_900u64 * MAX_STAKED_HASHBEASTS as u64) + 1_000
        );
        assert_eq!(
            effective_after_stake,
            PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER
        );

        let (raw_after_unstake, effective_after_unstake) =
            remove_hashbeast_multiplier(raw_after_stake, 1_000).unwrap();
        assert_eq!(raw_after_unstake, starting_raw);
        assert_eq!(effective_after_unstake, 2_900);
    }

    #[test]
    fn large_hashbeast_multiplier_does_not_truncate() {
        let (raw_after_stake, effective_after_stake) =
            add_hashbeast_multiplier(BASE_MULTIPLIER as u64, 70_000).unwrap();
        assert_eq!(raw_after_stake, 71_000);
        assert_eq!(
            effective_after_stake,
            PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER
        );
    }

    #[test]
    fn passive_hashbeast_slots_are_smoothed_across_all_slots() {
        let mut raw = BASE_MULTIPLIER as u64;
        let mut effective = capped_player_multiplier(raw);
        assert_eq!(effective, BASE_MULTIPLIER as u16);

        for _ in 0..MAX_STAKED_HASHBEASTS {
            (raw, effective) = add_hashbeast_multiplier(raw, BASE_MULTIPLIER).unwrap();
        }

        assert_eq!(
            raw,
            BASE_MULTIPLIER as u64 * (MAX_STAKED_HASHBEASTS as u64 + 1)
        );
        assert_eq!(effective, 2_000);

        let mut raw = BASE_MULTIPLIER as u64;
        let mut effective = capped_player_multiplier(raw);
        for _ in 0..MAX_STAKED_HASHBEASTS {
            (raw, effective) =
                add_hashbeast_multiplier(raw, GAMEPLAY_MAX_MULTIPLIER as u32).unwrap();
        }

        assert_eq!(effective, PASSIVE_HASHBEAST_STAKING_MAX_MULTIPLIER);
    }

    #[test]
    fn genesis_mint_cap_is_enforced_per_faction() {
        let mut mint_config = HashBeastMintConfig {
            bump: 0,
            is_active: true,
            base_price: 1_000_000_000,
            curve_a: 5_263_158,
            genesis_mint_limit: 12_000,
            genesis_mints: 999,
            max_genesis_mints_per_faction: 1_000,
            genesis_mints_by_faction: [0u16; NUM_FACTIONS],
            ticket_tiers: vec![
                TicketTier {
                    ticket_value: 1_000_000,
                },
                TicketTier {
                    ticket_value: 10_000_000,
                },
                TicketTier {
                    ticket_value: 100_000_000,
                },
            ],
        };
        mint_config.genesis_mints_by_faction[3] = 999;

        assert!(validate_genesis_faction_cap(&mint_config, 3, 1).is_ok());
        assert!(validate_genesis_faction_cap(&mint_config, 3, 2).is_err());
    }

    // ------------------------------------------------------------------------
    // ceil_mul_div_u64
    // ------------------------------------------------------------------------

    #[test]
    fn ceil_mul_div_exact_division() {
        assert_eq!(ceil_mul_div_u64(10, 1, 2).unwrap(), 5);
    }

    #[test]
    fn ceil_mul_div_needs_ceiling() {
        assert_eq!(ceil_mul_div_u64(10, 1, 3).unwrap(), 4); // 10/3 = 3.33 → 4
    }

    #[test]
    fn ceil_mul_div_zero_numerator() {
        assert_eq!(ceil_mul_div_u64(0, 100, 3).unwrap(), 0);
    }

    #[test]
    fn ceil_mul_div_division_by_zero_errors() {
        assert!(ceil_mul_div_u64(10, 1, 0).is_err());
    }

    // ------------------------------------------------------------------------
    // shares_known_parent
    // ------------------------------------------------------------------------

    #[test]
    fn shares_known_parent_when_shared() {
        let mut a = blank_hashbeast_metadata();
        let mut b = blank_hashbeast_metadata();
        a.mom = Pubkey::new_from_array([1u8; 32]);
        b.dad = Pubkey::new_from_array([1u8; 32]);
        assert!(shares_known_parent(&a, &b));
    }

    #[test]
    fn shares_known_parent_when_no_shared() {
        let a = blank_hashbeast_metadata();
        let b = blank_hashbeast_metadata();
        assert!(!shares_known_parent(&a, &b));
    }

    #[test]
    fn shares_known_parent_ignores_default() {
        let mut a = blank_hashbeast_metadata();
        let mut b = blank_hashbeast_metadata();
        a.mom = Pubkey::new_from_array([1u8; 32]);
        b.mom = Pubkey::default();
        b.dad = Pubkey::default();
        assert!(!shares_known_parent(&a, &b));
    }

    // ------------------------------------------------------------------------
    // metadata_rebirth_count
    // ------------------------------------------------------------------------

    #[test]
    fn metadata_rebirth_count_prefers_field_when_higher() {
        let mut meta = blank_hashbeast_metadata();
        meta.rebirth_count = 5;
        crate::genescience::set_rebirth_count(&mut meta.dna, 3).unwrap();
        assert_eq!(metadata_rebirth_count(&meta), 5);
    }

    #[test]
    fn metadata_rebirth_count_uses_dna_when_higher() {
        let mut meta = blank_hashbeast_metadata();
        meta.rebirth_count = 2;
        crate::genescience::set_rebirth_count(&mut meta.dna, 4).unwrap();
        assert_eq!(metadata_rebirth_count(&meta), 4);
    }

    #[test]
    fn metadata_rebirth_count_equal_values() {
        let mut meta = blank_hashbeast_metadata();
        meta.rebirth_count = 3;
        crate::genescience::set_rebirth_count(&mut meta.dna, 3).unwrap();
        assert_eq!(metadata_rebirth_count(&meta), 3);
    }
}

// ----------------------------------------------------------------------------------------
// --------------  HASHBEAST ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(mint_count: u64)]
pub struct SimulateMintCost<'info> {
    #[account(
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump
    )]
    pub hashbeast_config: Account<'info, HashBeastConfig>,

    #[account(
        seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()],
        bump = hashbeast_mint_config.bump
    )]
    pub hashbeast_mint_config: Account<'info, HashBeastMintConfig>,
}

#[derive(Accounts)]
#[instruction(faction_id: u8)]
pub struct MintHashBeast<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump
    )]
    pub hashbeast_config: Box<Account<'info, HashBeastConfig>>,

    #[account(
        mut,
        seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()],
        bump = hashbeast_mint_config.bump
    )]
    pub hashbeast_mint_config: Box<Account<'info, HashBeastMintConfig>>,

    #[account(mut, seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()], bump)]
    /// CHECK: Seed-checked and owner field is verified in the handler without
    /// deserializing the full PlayerData account in the generated validator.
    pub player_data: UncheckedAccount<'info>,

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
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Optional collection account for the HashBeast
    /// CHECK: Optional collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = HashBeastMetadata::LEN,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_asset.key().as_ref()],
        bump
    )]
    pub hashbeast_metadata: Box<Account<'info, HashBeastMetadata>>,

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
pub struct BatchMintHashBeast<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump,
    )]
    pub hashbeast_config: Account<'info, HashBeastConfig>,

    #[account(
        mut,
        seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()],
        bump = hashbeast_mint_config.bump,
    )]
    pub hashbeast_mint_config: Account<'info, HashBeastMintConfig>,

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

    /// CHECK: HashBeast collection (Metaplex Core)
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

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

    /// Optional only when the minter has no referrer.
    /// Referred minters must provide the canonical referrer's ReferralRewards PDA.
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED, player_data.referral_code.as_ref()],
        bump = referrer_rewards.bump,
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(recipient: Pubkey, faction_id: u8)]
pub struct AdminMintHashBeast<'info> {
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
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump,
    )]
    pub hashbeast_config: Account<'info, HashBeastConfig>,

    #[account(
        mut,
        seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()],
        bump = hashbeast_mint_config.bump,
    )]
    pub hashbeast_mint_config: Account<'info, HashBeastMintConfig>,

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
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Optional collection account for the HashBeast
    /// CHECK: Optional collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = authority,
        space = HashBeastMetadata::LEN,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_asset.key().as_ref()],
        bump
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

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
#[instruction(faction_id: u8)]
pub struct WhitelistMintHashBeast<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [HASHBEAST_CONFIG_SEED.as_ref()],
        bump = hashbeast_config.bump,
    )]
    pub hashbeast_config: Account<'info, HashBeastConfig>,

    #[account(
        mut,
        seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()],
        bump = hashbeast_mint_config.bump,
    )]
    pub hashbeast_mint_config: Account<'info, HashBeastMintConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [HASHBEAST_FREE_MINT_ALLOWANCE_SEED.as_ref(), user.key().as_ref()],
        bump = hashbeast_free_mint_allowance.bump,
        constraint = hashbeast_free_mint_allowance.user == user.key() @ ErrorCode::Unauthorized
    )]
    pub hashbeast_free_mint_allowance: Account<'info, HashBeastFreeMintAllowance>,

    /// Metaplex Core asset (will be created)
    #[account(mut)]
    /// CHECK: Will be created via MPL Core CPI
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Optional collection account for the HashBeast
    /// CHECK: Optional collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = HashBeastMetadata::LEN,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_asset.key().as_ref()],
        bump
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED],
        bump
    )]
    /// CHECK: PDA authority for the collection
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakeHashBeast<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,

    /// Metaplex Core asset (source of truth for ownership)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Optional collection account for the HashBeast
    /// CHECK: Optional collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(),hashbeast_metadata.mint.as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [HASHBEAST_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub hashbeast_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeHashBeast<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    #[account(seeds = [TAX_CONFIG_SEED.as_ref()], bump = tax_config.bump)]
    pub tax_config: Account<'info, TaxConfig>,

    /// Metaplex Core asset (currently locked in custody PDA)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Optional collection account for the HashBeast
    /// CHECK: Optional collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(),hashbeast_metadata.mint.as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [HASHBEAST_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub hashbeast_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RebirthHashBeast<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Existing HashBeast metadata account. Mutated in-place (multiplier, xp,
    /// accumulated_val reset to fresh-start values). NOT closed — the same
    /// metadata follows the asset to its next owner.
    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_asset.key().as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub hashbeast_metadata: Box<Account<'info, HashBeastMetadata>>,

    /// CHECK: Metaplex Core asset; ownership and validity enforced by mpl-core
    /// during the TransferV1 CPI. Currently owned by `user`; becomes owned by
    /// `inventory_pda` after this instruction.
    #[account(mut)]
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: Optional HashBeast collection account. Required by mpl-core whenever
    /// the asset belongs to a collection (which all HashBeasts do post-genesis).
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    /// Global inventory pool — counters bumped here. Same PDA acts as the
    /// new owner of the reborn mpl-core asset (custody).
    #[account(
        mut,
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pool: Box<Account<'info, InventoryPool>>,

    /// CHECK: Inventory custody PDA. The mpl-core asset's `owner` field is
    /// rewritten to this address by the transfer CPI. It is the *same* PDA
    /// as `inventory_pool` (we just need a separate AccountInfo binding for
    /// mpl-core to see). Validated by seeds.
    #[account(
        seeds = [INVENTORY_POOL_SEED],
        bump = inventory_pool.bump,
    )]
    pub inventory_pda: UncheckedAccount<'info>,

    /// New per-asset entry created ONLY when the queue had space (asset was
    /// pushed in). When the queue was full, asset is burned and this PDA is
    /// not initialized. Manually init'd inside the handler via
    /// `helper::init_pda_account_if_needed`.
    /// CHECK: PDA seeds + bump validated by Anchor; payload init in handler.
    #[account(
        mut,
        seeds = [REBORN_ENTRY_SEED, hashbeast_asset.key().as_ref()],
        bump,
    )]
    pub reborn_entry: UncheckedAccount<'info>,

    /// Country lootbox queue for the hashbeast's faction. Pushed into if there's
    /// space; otherwise the asset is burned (no listing fallback).
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[hashbeast_metadata.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    // Mining accounts for token transfer
    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = dbtc_mining.bump,
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED, dbtc_mining.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault_authority,
    )]
    pub dbtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()],
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

    #[account(address = dbtc_token_vault.mint)]
    pub token_mint: InterfaceAccount<'info, Mint2022>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BreedHashBeast<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut, seeds = [HASHBEAST_CONFIG_SEED.as_ref()], bump = hashbeast_config.bump)]
    pub hashbeast_config: Box<Account<'info, HashBeastConfig>>,

    #[account(seeds = [HASHBEAST_MINT_CONFIG_SEED.as_ref()], bump)]
    /// CHECK: Seed-checked and deserialized in the handler to keep the generated
    /// account validator under the BPF stack limit.
    pub hashbeast_mint_config: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump
    )]
    /// CHECK: Seed-checked and owner field is verified in the handler without
    /// deserializing the full PlayerData account in the generated validator.
    pub player_data: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump = global_config.treasury_bump,
        constraint = sol_treasury.key() == global_config.pda_sol_treasury @ ErrorCode::InvalidAccount
    )]
    /// CHECK: SOL treasury PDA; holds native SOL for the economy/buyback loop.
    pub sol_treasury: UncheckedAccount<'info>,

    #[account(mut, address = global_config.fee_recipient)]
    /// CHECK: Native SOL fee recipient configured in GlobalConfig.
    pub fee_recipient: UncheckedAccount<'info>,

    #[account(seeds = [FLOOR_HISTORY_SEED], bump)]
    /// CHECK: Seed-checked and deserialized in the handler to keep the generated
    /// account validator under the BPF stack limit.
    pub floor_history: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = dbtc_mining.bump,
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    #[account(
        mut,
        seeds = [DEGEN_BTC_VAULT_SEED, dbtc_mining.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault_authority,
        constraint = dbtc_token_vault.key() == dbtc_mining.dbtc_token_vault @ ErrorCode::InvalidAccount,
    )]
    pub dbtc_token_vault: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        seeds = [DEGEN_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump = dbtc_mining.vault_auth_bump,
    )]
    /// CHECK: Vault authority PDA for the mining token vault.
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = user,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount2022>>,

    #[account(
        mut,
        address = dbtc_token_vault.mint,
        constraint = token_mint.decimals == DBTC_DECIMALS @ ErrorCode::InvalidMint,
    )]
    pub token_mint: Box<InterfaceAccount<'info, Mint2022>>,

    /// CHECK: Mom NFT asset - Verified via get_mpl_core_owner
    #[account(mut)]
    pub mom_asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), mom_asset.key().as_ref()],
        bump = mom_metadata.bump,
        constraint = mom_metadata.mint == mom_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub mom_metadata: Box<Account<'info, HashBeastMetadata>>,

    /// CHECK: Dad NFT asset - Verified via get_mpl_core_owner
    #[account(mut)]
    pub dad_asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), dad_asset.key().as_ref()],
        bump = dad_metadata.bump,
        constraint = dad_metadata.mint == dad_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dad_metadata: Box<Account<'info, HashBeastMetadata>>,

    /// CHECK: Offspring NFT asset - Will be created via MPL Core CPI
    #[account(mut)]
    pub offspring_asset: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = HashBeastMetadata::LEN,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), offspring_asset.key().as_ref()],
        bump
    )]
    pub offspring_metadata: Box<Account<'info, HashBeastMetadata>>,

    /// CHECK: HashBeast collection
    #[account(mut)]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    /// CHECK: Collection authority PDA
    #[account(seeds = [COLLECTION_AUTHORITY_SEED.as_ref()], bump)]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program_2022: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}
