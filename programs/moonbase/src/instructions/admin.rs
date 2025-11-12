use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use mpl_core::{instructions::CreateCollectionV1CpiBuilder, ID as MPL_CORE_PROGRAM_ID};

use crate::errors::ErrorCode;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{
    self as token_if, // gives you CPI helpers such as `token_if::transfer`
    Mint as Mint2022,
    TokenAccount as TokenAccount2022,
}; // ← the PROGRAM-ID wrapper (implements Id)

// Import Raydium CP-Swap for CPI calls (actual CPI calls are in economy.rs)

// constants
pub const MAX_MODULE_CONFIGS: usize = 50; // ≈ 8.2 kB

// Query data structures for external programs
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TreasuryInfo {
    pub total_balance: u64,
    pub pol_reserves: u64,
    pub rent_exempt_amount: u64,
    pub available_for_withdrawal: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GlobalConfigInfo {
    pub is_game_active: bool,
    pub ext_authority: Pubkey,
    pub ext_fee_collector: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TokenPricesInfo {
    pub dbtc_price_in_sol: u64,
    pub lp_token_price_in_sol: u64,
}

// --------------------------------------------------------------------------------
// ------------ GLOBAL_CONFIG :: UPDATES, ADDING EXPANSIONS ------------
// --------------------------------------------------------------------------------

pub fn internal_initialize(ctx: Context<Initialize>, creation_fee_recipient: Pubkey) -> Result<()> {
    msg!("🔧 [internal_initialize] Initializing global config");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Creation fee recipient: {}", creation_fee_recipient);
    
    let global_config = &mut ctx.accounts.global_config;
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;

    // Initialize GlobalConfig
    msg!("   Initializing GlobalConfig...");
    global_config.ext_authority = ctx.accounts.authority.key();
    global_config.ext_fee_collector = ctx.accounts.authority.key(); // Initially set to authority, can be updated later
    global_config.creation_fee_recipient = creation_fee_recipient;
    msg!("     Authority: {}", global_config.ext_authority);
    msg!("     Fee collector: {}", global_config.ext_fee_collector);
    msg!("     Creation fee recipient: {}", global_config.creation_fee_recipient);

    // Store both PDA bumps for future derivation
    global_config.pda_sol_treasury = ctx.accounts.sol_treasury.key();
    global_config.treasury_bump = ctx.bumps.sol_treasury;
    msg!("     SOL treasury PDA: {} (bump: {})", global_config.pda_sol_treasury, global_config.treasury_bump);

    // Initialize SOL fee config with defaults
    msg!("   Initializing SOL fee config...");
    global_config.sol_fee_config = SolFeeConfig {
        protocol_fee_pct: 10,
        buyback_pct: 40,
        stakers_pct: 40,
    };
    msg!("     Protocol fee: {}%", global_config.sol_fee_config.protocol_fee_pct);
    msg!("     Buyback: {}%", global_config.sol_fee_config.buyback_pct);
    msg!("     Stakers: {}%", global_config.sol_fee_config.stakers_pct);

    // Initialize DogeBtc distribution config with defaults
    msg!("   Initializing DogeBtc distribution config...");
    global_config.dbtc_dist_config = DogeBtcDistConfig {
        dbtc_stakers_pct: 50,
        dbtc_winners_pct: 30,
        dbtc_same_faction_pct: 10,
        dbtc_motherlode_pct: 10,
        refining_fee: 5,
    };
    msg!("     Stakers: {}%", global_config.dbtc_dist_config.dbtc_stakers_pct);
    msg!("     Winners: {}%", global_config.dbtc_dist_config.dbtc_winners_pct);
    msg!("     Same-faction: {}%", global_config.dbtc_dist_config.dbtc_same_faction_pct);
    msg!("     Motherlode: {}%", global_config.dbtc_dist_config.dbtc_motherlode_pct);
    msg!("     Refining fee: {}%", global_config.dbtc_dist_config.refining_fee);

    // Initialize egg limits: [tier1_limit, tier2_limit, tier3_limit, tier4_limit]
    global_config.egg_limits = [5000, 5000, 5000, 5000];
    msg!("   Egg limits: tier1={}, tier2={}, tier3={}, tier4={}", 
        global_config.egg_limits[0], 
        global_config.egg_limits[1], 
        global_config.egg_limits[2], 
        global_config.egg_limits[3]
    );

    // Initialize Raydium pool state to default (must be set via admin function)
    global_config.raydium_pool_state = Pubkey::default();
    msg!("   Raydium pool state: {} (default, must be set via admin)", global_config.raydium_pool_state);

    // Initialize global dragon egg power tracker
    global_config.global_dragon_egg_power = 0;
    msg!("   Global dragon egg power: {}", global_config.global_dragon_egg_power);

    global_config.bump = ctx.bumps.global_config;
    global_config.is_game_active = false; // Game starts inactive until initialized
    msg!("   Global config bump: {}", global_config.bump);
    msg!("   Game active: {} (starts inactive)", global_config.is_game_active);

    // Initialize empty factions list
    global_config.supported_factions = Vec::new();
    msg!("   Supported factions: {} (empty, must be added via admin)", global_config.supported_factions.len());

    // Initialize dragon egg URIs as empty vec of vecs (4 tiers, will be populated per faction)
    global_config.dragon_egg_uris = Vec::new();
    msg!("   Dragon egg URIs: {} (empty, must be set via admin)", global_config.dragon_egg_uris.len());

    // Optionally drop 1 lamport into the vault for future-proof rent-exempt status
    msg!("   Transferring 1 lamport to SOL treasury for rent-exempt status...");
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        1,
    )?;
    msg!("     ✓ 1 lamport transferred");

    // Initialize DogeBtcMining
    msg!("   Initializing DogeBtcMining...");
    doge_btc_mining.dbtc_token_vault = Pubkey::default(); // Will be set during initialize_mining
    doge_btc_mining.mining_start_timestamp = 0; // Set to 0 to indicate mining not started
    doge_btc_mining.doge_btc_per_slot = 0;
    doge_btc_mining.last_slot = 0;
    doge_btc_mining.total_tokens_mined = 0;
    doge_btc_mining.bump = ctx.bumps.doge_btc_mining;
    doge_btc_mining.vault_auth_bump = 0; // Will be set during initialize_mining
    msg!("     DogeBtc token vault: {} (default, will be set during initialize_mining)", doge_btc_mining.dbtc_token_vault);
    msg!("     Mining start timestamp: {} (0 = not started)", doge_btc_mining.mining_start_timestamp);
    msg!("     DogeBtc per slot: {}", doge_btc_mining.doge_btc_per_slot);
    msg!("     Bump: {}", doge_btc_mining.bump);

    // Initialize dynamic distribution fields with defaults
    msg!("   Initializing dynamic distribution fields...");
    doge_btc_mining.raydium_pool_state = Pubkey::default();
    doge_btc_mining.last_rate_update = 0;
    doge_btc_mining.current_dist_rate = 0;
    doge_btc_mining.price_history = Vec::new();
    doge_btc_mining.recent_price = 0; // Default: 0.001 SOL/DBTC
    doge_btc_mining.track_price = 0;
    doge_btc_mining.sol_for_pol = 0;
    msg!("     Raydium pool state: {} (default)", doge_btc_mining.raydium_pool_state);
    msg!("     Current dist rate: {}", doge_btc_mining.current_dist_rate);
    msg!("     Recent price: {}", doge_btc_mining.recent_price);

    //--------------------------------- Global Game State ---------------------------------
    // Note: GlobalGameSate should be initialized separately via initialize_game_state function
    // We don't initialize it here to keep Initialize simple
    msg!("   ⚠️ Note: GlobalGameSate must be initialized separately via initialize_game_state");

    msg!("✅ [internal_initialize] Global config initialized successfully");
    msg!("   SOL Treasury PDA: {} (bump: {})",
        ctx.accounts.sol_treasury.key(),
        ctx.bumps.sol_treasury
    );

    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG URI MANAGEMENT (ADMIN) ---------------------------------------
// ----------------------------------------------------------------------------------------

/// Set Dragon Egg URIs for a specific tier and all factions (admin only)
/// uris: Vec of URIs, one per faction (must match number of factions)
/// tier: 1, 2, 3, or 4
pub fn set_dragon_egg_uris_for_tier_internal(
    ctx: Context<UpdateConfigAc>,
    tier: u8,
    uris: Vec<String>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    require!(tier >= 1 && tier <= 4, ErrorCode::InvalidParameters);
    require!(
        uris.len() == global_config.supported_factions.len(),
        ErrorCode::InvalidParameters
    );

    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= MAX_URI_LENGTH, ErrorCode::UriTooLong);
    }

    let tier_index = (tier - 1) as usize; // tier 1->0, 2->1, 3->2, 4->3

    // Ensure we have 4 tiers initialized
    while global_config.dragon_egg_uris.len() <= tier_index {
        global_config.dragon_egg_uris.push(Vec::new());
    }

    // Set URIs for this tier
    global_config.dragon_egg_uris[tier_index] = uris.clone();

    msg!("✅ Set {} Dragon Egg URIs for tier {}", uris.len(), tier);
    msg!("   Factions: {}", global_config.supported_factions.len());

    Ok(())
}

/// Clear all Dragon Egg URIs (admin only)
pub fn clear_dragon_egg_uris_internal(ctx: Context<UpdateConfigAc>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    global_config.dragon_egg_uris.clear();

    msg!("✅ Cleared all Dragon Egg URIs");

    Ok(())
}

/// Set the Raydium pool state address (admin only)
/// This is a security measure to prevent using malicious pools
pub fn set_raydium_pool_state_internal(
    ctx: Context<UpdateConfigAc>,
    raydium_pool_state: Pubkey,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    require!(
        raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );

    global_config.raydium_pool_state = raydium_pool_state;

    msg!("✅ Set Raydium pool state: {}", raydium_pool_state);

    Ok(())
}

/// Set the Dragon Egg collection address (admin only, should be called during initialize)
pub fn set_dragon_egg_collection_internal(
    ctx: Context<UpdateConfigAc>,
    dragon_egg_collection: Pubkey,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    global_config.dragon_egg_collection = dragon_egg_collection;

    msg!("✅ Set Dragon Egg collection: {}", dragon_egg_collection);

    Ok(())
}

/// Add a single faction to the global config (admin only)
/// Also initializes the faction state account for the new faction
pub fn add_faction_internal(ctx: Context<AddFaction>, faction_name: String) -> Result<()> {
    msg!("🏛️ [add_faction_internal] Adding faction");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Faction name: {}", faction_name);
    
    let global_config = &mut ctx.accounts.global_config;

    msg!("   Current factions count: {}", global_config.supported_factions.len());
    msg!("   Validating faction name...");
    // Validate faction name
    require!(
        faction_name.len() > 0 && faction_name.len() <= MAX_FACTION_NAME_LENGTH,
        ErrorCode::InvalidFactionName
    );
    msg!("     ✓ Faction name length valid ({} chars, max: {})", faction_name.len(), MAX_FACTION_NAME_LENGTH);

    // Check we don't exceed max factions
    let current_faction_count = global_config.supported_factions.len();
    require!(
        current_faction_count < MAX_FACTIONS,
        ErrorCode::MaxFactionsReached
    );
    msg!("     ✓ Factions count < MAX_FACTIONS ({})", MAX_FACTIONS);

    let faction_id = current_faction_count as u8;
    msg!("   Faction ID will be: {}", faction_id);

    // Derive expected PDA and verify
    msg!("   Deriving faction state PDA...");
    let (expected_pda, bump) = Pubkey::find_program_address(
        &[FACTION_STATE_SEED.as_ref(), &[faction_id]],
        ctx.program_id,
    );
    msg!("     Expected PDA: {} (bump: {})", expected_pda, bump);
    require!(
        ctx.accounts.faction_state.key() == expected_pda,
        ErrorCode::InvalidAccount
    );
    msg!("     ✓ Faction state PDA matches");

    // Initialize faction state account if needed
    msg!("   Checking if faction state account exists...");
    let rent = Rent::get()?;
    let rent_lamports = rent.minimum_balance(FactionState::LEN);
    msg!("     Rent required: {} lamports", rent_lamports);
    
    if ctx.accounts.faction_state.lamports() == 0 {
        msg!("     Account doesn't exist - creating...");
        // Create account
        anchor_lang::system_program::create_account(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::CreateAccount {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.faction_state.to_account_info(),
                },
                &[&[FACTION_STATE_SEED.as_ref(), &[faction_id], &[bump]]],
            ),
            rent_lamports,
            FactionState::LEN as u64,
            ctx.program_id,
        )?;
        msg!("     ✓ Faction state account created");
    } else {
        msg!("     Account already exists");
    }

    // Initialize faction state data
    msg!("   Initializing faction state data...");
    let mut faction_state_data = ctx.accounts.faction_state.try_borrow_mut_data()?;
    let faction_state = FactionState {
        bump,
        faction_id,
        total_passive_hashpower: 0,
        total_sol_bets: 0,
        total_wins: 0,
        sol_reward_index: 0,
        dbtc_reward_index: 0,
        motherlode_pot_size: 0,
    };
    faction_state.try_serialize(&mut &mut faction_state_data[..])?;
    msg!("     Faction state initialized:");
    msg!("       Bump: {}", bump);
    msg!("       Total passive hashpower: 0");
    msg!("       Total SOL bets: 0");
    msg!("       Total wins: 0");
    msg!("       SOL reward index: 0");
    msg!("       DogeBtc reward index: 0");
    msg!("       Motherlode pot size: 0");

    // Add faction to config
    msg!("   Adding faction to supported_factions list...");
    global_config.supported_factions.push(faction_name.clone());
    msg!("     New factions count: {}", global_config.supported_factions.len());

    msg!("✅ [add_faction_internal] Faction added successfully");
    msg!("   Faction: {} (ID: {})", faction_name, faction_id);
    msg!("   Total factions: {}", global_config.supported_factions.len());

    // Emit event for off-chain indexing
    emit!(FactionsAdded {
        authority: ctx.accounts.authority.key(),
        factions: vec![faction_name.clone()],
        total_factions: global_config.supported_factions.len() as u8,
    });

    Ok(())
}

/// Update the global configuration parameters
/// Can only be called by the current authority
pub fn update_config_internal(
    ctx: Context<UpdateConfigAc>,
    new_authority: Option<Pubkey>,
    new_fee_collector: Option<Pubkey>,
    new_creation_fee_recipient: Option<Pubkey>,
) -> Result<()> {
    msg!("🔧 [update_config_internal] Updating global config");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_config = &mut ctx.accounts.global_config;

    msg!("   Current config:");
    msg!("     Authority: {}", global_config.ext_authority);
    msg!("     Fee collector: {}", global_config.ext_fee_collector);
    msg!("     Creation fee recipient: {}", global_config.creation_fee_recipient);

    // Update fields if provided
    if let Some(authority) = new_authority {
        let old_authority = global_config.ext_authority;
        global_config.ext_authority = authority;
        msg!("   Updated authority: {} -> {}", old_authority, authority);
    } else {
        msg!("   Authority: not updated");
    }

    // Update SOL claimer if provided
    if let Some(fee_collector) = new_fee_collector {
        let old_collector = global_config.ext_fee_collector;
        global_config.ext_fee_collector = fee_collector;
        msg!("   Updated fee collector: {} -> {}", old_collector, fee_collector);
    } else {
        msg!("   Fee collector: not updated");
    }

    // Update creation fee recipient if provided
    if let Some(creation_fee_recipient) = new_creation_fee_recipient {
        let old_recipient = global_config.creation_fee_recipient;
        global_config.creation_fee_recipient = creation_fee_recipient;
        msg!("   Updated creation fee recipient: {} -> {}", old_recipient, creation_fee_recipient);
    } else {
        msg!("   Creation fee recipient: not updated");
    }

    msg!("✅ [update_config_internal] Config updated successfully");
    Ok(())
}

/// Update fee configuration (admin only)
/// Validates that percentages sum correctly
pub fn update_fees_internal(
    ctx: Context<UpdateConfigAc>,
    new_protocol_fee_pct: Option<u8>,
    new_buyback_pct: Option<u8>,
    new_stakers_pct: Option<u8>,
    new_dbtc_stakers_pct: Option<u8>,
    new_dbtc_winners_pct: Option<u8>,
    new_dbtc_same_faction_pct: Option<u8>,
    new_dbtc_motherlode_pct: Option<u8>,
    new_refining_fee: Option<u8>,
) -> Result<()> {
    msg!("💰 [update_fees_internal] Updating fee configuration");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_config = &mut ctx.accounts.global_config;
    
    msg!("   Current SOL fee config:");
    msg!("     Protocol fee: {}%", global_config.sol_fee_config.protocol_fee_pct);
    msg!("     Buyback: {}%", global_config.sol_fee_config.buyback_pct);
    msg!("     Stakers: {}%", global_config.sol_fee_config.stakers_pct);
    msg!("   Current DogeBtc dist config:");
    msg!("     Stakers: {}%", global_config.dbtc_dist_config.dbtc_stakers_pct);
    msg!("     Winners: {}%", global_config.dbtc_dist_config.dbtc_winners_pct);
    msg!("     Same-faction: {}%", global_config.dbtc_dist_config.dbtc_same_faction_pct);
    msg!("     Motherlode: {}%", global_config.dbtc_dist_config.dbtc_motherlode_pct);
    msg!("     Refining fee: {}%", global_config.dbtc_dist_config.refining_fee);

    // Update SOL fee config if any values provided
    if new_protocol_fee_pct.is_some() || new_buyback_pct.is_some() || new_stakers_pct.is_some() {
        msg!("   Updating SOL fee config...");
        let protocol_fee_pct = new_protocol_fee_pct.unwrap_or(global_config.sol_fee_config.protocol_fee_pct);
        let buyback_pct = new_buyback_pct.unwrap_or(global_config.sol_fee_config.buyback_pct);
        let stakers_pct = new_stakers_pct.unwrap_or(global_config.sol_fee_config.stakers_pct);

        msg!("     New values: protocol={}%, buyback={}%, stakers={}%", protocol_fee_pct, buyback_pct, stakers_pct);
        let total = protocol_fee_pct as u16 + buyback_pct as u16 + stakers_pct as u16;
        msg!("     Total: {}%", total);
        
        require!(
            total == 100,
            ErrorCode::InvalidParameters
        );
        msg!("     ✓ Percentages sum to 100%");

        let old_config = global_config.sol_fee_config.clone();
        global_config.sol_fee_config = SolFeeConfig {
            protocol_fee_pct,
            buyback_pct,
            stakers_pct,
        };
        msg!("     Updated: protocol {}% -> {}%, buyback {}% -> {}%, stakers {}% -> {}%",
            old_config.protocol_fee_pct, protocol_fee_pct,
            old_config.buyback_pct, buyback_pct,
            old_config.stakers_pct, stakers_pct
        );
    } else {
        msg!("   SOL fee config: not updated");
    }

    // Update DogeBtc distribution config if any values provided
    if new_dbtc_stakers_pct.is_some() 
        || new_dbtc_winners_pct.is_some() 
        || new_dbtc_same_faction_pct.is_some() 
        || new_dbtc_motherlode_pct.is_some() 
    {
        msg!("   Updating DogeBtc distribution config...");
        let dbtc_stakers_pct = new_dbtc_stakers_pct.unwrap_or(global_config.dbtc_dist_config.dbtc_stakers_pct);
        let dbtc_winners_pct = new_dbtc_winners_pct.unwrap_or(global_config.dbtc_dist_config.dbtc_winners_pct);
        let dbtc_same_faction_pct = new_dbtc_same_faction_pct.unwrap_or(global_config.dbtc_dist_config.dbtc_same_faction_pct);
        let dbtc_motherlode_pct = new_dbtc_motherlode_pct.unwrap_or(global_config.dbtc_dist_config.dbtc_motherlode_pct);

        msg!("     New values: stakers={}%, winners={}%, same_faction={}%, motherlode={}%",
            dbtc_stakers_pct, dbtc_winners_pct, dbtc_same_faction_pct, dbtc_motherlode_pct);
        let total = dbtc_stakers_pct as u16
            + dbtc_winners_pct as u16
            + dbtc_same_faction_pct as u16
            + dbtc_motherlode_pct as u16;
        msg!("     Total: {}%", total);
        
        require!(
            total == 100,
            ErrorCode::InvalidParameters
        );
        msg!("     ✓ Percentages sum to 100%");

        // Get current refining_fee to preserve it
        let current_refining_fee = global_config.dbtc_dist_config.refining_fee;
        msg!("     Preserving refining_fee: {}%", current_refining_fee);
        
        let old_config = global_config.dbtc_dist_config.clone();
        global_config.dbtc_dist_config = DogeBtcDistConfig {
            dbtc_stakers_pct,
            dbtc_winners_pct,
            dbtc_same_faction_pct,
            dbtc_motherlode_pct,
            refining_fee: current_refining_fee,
        };
        msg!("     Updated: stakers {}% -> {}%, winners {}% -> {}%, same_faction {}% -> {}%, motherlode {}% -> {}%",
            old_config.dbtc_stakers_pct, dbtc_stakers_pct,
            old_config.dbtc_winners_pct, dbtc_winners_pct,
            old_config.dbtc_same_faction_pct, dbtc_same_faction_pct,
            old_config.dbtc_motherlode_pct, dbtc_motherlode_pct
        );
    } else {
        msg!("   DogeBtc dist config: not updated");
    }
    
    // Update refining fee if provided
    if let Some(refining_fee) = new_refining_fee {
        let old_refining_fee = global_config.dbtc_dist_config.refining_fee;
        global_config.dbtc_dist_config.refining_fee = refining_fee;
        msg!("   Updated refining fee: {}% -> {}%", old_refining_fee, refining_fee);
    } else {
        msg!("   Refining fee: {}% (not updated)", global_config.dbtc_dist_config.refining_fee);
    }

    msg!("✅ [update_fees_internal] Fee configuration updated successfully");
    Ok(())
}

/// Update egg limits for tiers (admin only)
pub fn update_egg_limits_internal(
    ctx: Context<UpdateConfigAc>,
    tier1_limit: Option<u64>,
    tier2_limit: Option<u64>,
    tier3_limit: Option<u64>,
    tier4_limit: Option<u64>,
) -> Result<()> {
    msg!("🥚 [update_egg_limits_internal] Updating egg limits");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_config = &mut ctx.accounts.global_config;

    msg!("   Current egg limits:");
    msg!("     Tier 1: {}", global_config.egg_limits[0]);
    msg!("     Tier 2: {}", global_config.egg_limits[1]);
    msg!("     Tier 3: {}", global_config.egg_limits[2]);
    msg!("     Tier 4: {}", global_config.egg_limits[3]);

    if let Some(limit) = tier1_limit {
        let old_limit = global_config.egg_limits[0];
        global_config.egg_limits[0] = limit;
        msg!("   Updated Tier 1: {} -> {}", old_limit, limit);
    } else {
        msg!("   Tier 1: not updated");
    }

    if let Some(limit) = tier2_limit {
        let old_limit = global_config.egg_limits[1];
        global_config.egg_limits[1] = limit;
        msg!("   Updated Tier 2: {} -> {}", old_limit, limit);
    } else {
        msg!("   Tier 2: not updated");
    }

    if let Some(limit) = tier3_limit {
        let old_limit = global_config.egg_limits[2];
        global_config.egg_limits[2] = limit;
        msg!("   Updated Tier 3: {} -> {}", old_limit, limit);
    } else {
        msg!("   Tier 3: not updated");
    }

    if let Some(limit) = tier4_limit {
        let old_limit = global_config.egg_limits[3];
        global_config.egg_limits[3] = limit;
        msg!("   Updated Tier 4: {} -> {}", old_limit, limit);
    } else {
        msg!("   Tier 4: not updated");
    }

    msg!("✅ [update_egg_limits_internal] Egg limits updated successfully");
    Ok(())
}

// --------------------------------------------------------------------------------
// ------------ FACTION SURGE GAME STATE INITIALIZATION ---------------------------
// --------------------------------------------------------------------------------

/// Initialize the global game state for Faction Surge
pub fn initialize_game_state_internal(
    ctx: Context<InitializeGameState>,
    round_duration_seconds: i64,
) -> Result<()> {
    msg!("🎮 [initialize_game_state_internal] Initializing global game state");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Round duration: {} seconds", round_duration_seconds);
    
    let global_game_state = &mut ctx.accounts.global_game_state;
    let clock = Clock::get()?;

    msg!("   Current timestamp: {}", clock.unix_timestamp);

    // Initialize game state
    msg!("   Initializing game state fields...");
    global_game_state.bump = ctx.bumps.global_game_state;
    global_game_state.is_active = true;
    global_game_state.current_round_id = 0; // Will be incremented to 1 in start_round
    global_game_state.round_end_timestamp = clock.unix_timestamp + round_duration_seconds;
    global_game_state.round_duration_seconds = round_duration_seconds;
    msg!("     Bump: {}", global_game_state.bump);
    msg!("     Is active: {}", global_game_state.is_active);
    msg!("     Current round ID: {} (will be incremented to 1 in start_round)", global_game_state.current_round_id);
    msg!("     Round duration: {} seconds", global_game_state.round_duration_seconds);
    msg!("     Round end timestamp: {}", global_game_state.round_end_timestamp);
    
    // Initialize previous round data
    global_game_state.last_round_id = 0;
    global_game_state.winning_faction_id = 0;
    msg!("     Last round ID: {}", global_game_state.last_round_id);
    msg!("     Winning faction ID: {} (no rounds completed yet)", global_game_state.winning_faction_id);
    
    // Initialize commit-reveal randomness fields
    global_game_state.current_round_commit = [0u8; 32]; // Will be set in start_round
    global_game_state.current_round_seed = None;
    global_game_state.next_round_commit = [0u8; 32]; // Should be set before first start_round
    msg!("     Current round commit: {:?} (will be set in start_round)", global_game_state.current_round_commit);
    msg!("     Current round seed: {:?} (will be set in end_round)", global_game_state.current_round_seed);
    msg!("     Next round commit: {:?} (should be set before first start_round)", global_game_state.next_round_commit);
    
    // Initialize cumulative stats
    global_game_state.total_sol_bets = 0;
    global_game_state.total_global_passive_hashpower = 0;
    msg!("     Total SOL bets: {}", global_game_state.total_sol_bets);
    msg!("     Total global passive hashpower: {}", global_game_state.total_global_passive_hashpower);
    
    // Initialize empty cranker bots whitelist
    global_game_state.cranker_bots = Vec::new();
    msg!("     Cranker bots whitelist: {} (empty, must be added via admin)", global_game_state.cranker_bots.len());

    msg!("✅ [initialize_game_state_internal] Global game state initialized successfully");
    msg!("   Round duration: {} seconds", round_duration_seconds);
    msg!("   First round ends at: {}", global_game_state.round_end_timestamp);

    Ok(())
}

/// Initialize a faction state account
pub fn initialize_faction_state_internal(
    ctx: Context<InitializeFactionState>,
    faction_id: u8,
) -> Result<()> {
    msg!("🏛️ [initialize_faction_state_internal] Initializing faction state");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Faction ID: {}", faction_id);
    
    let global_config = &ctx.accounts.global_config;
    let faction_state = &mut ctx.accounts.faction_state;
    
    msg!("   Validating faction ID...");

    // Validate faction_id exists
    msg!("     Supported factions count: {}", global_config.supported_factions.len());
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    msg!("     ✓ Faction ID {} is valid", faction_id);

    // Initialize faction state
    msg!("   Initializing faction state fields...");
    faction_state.bump = ctx.bumps.faction_state;
    faction_state.faction_id = faction_id;
    faction_state.total_passive_hashpower = 0;
    faction_state.total_sol_bets = 0;
    faction_state.total_wins = 0;
    faction_state.sol_reward_index = 0;
    faction_state.dbtc_reward_index = 0;
    faction_state.motherlode_pot_size = 0;
    msg!("     Bump: {}", faction_state.bump);
    msg!("     Total passive hashpower: {}", faction_state.total_passive_hashpower);
    msg!("     Total SOL bets: {}", faction_state.total_sol_bets);
    msg!("     Total wins: {}", faction_state.total_wins);
    msg!("     SOL reward index: {}", faction_state.sol_reward_index);
    msg!("     DogeBtc reward index: {}", faction_state.dbtc_reward_index);
    msg!("     Motherlode pot size: {}", faction_state.motherlode_pot_size);

    msg!("✅ [initialize_faction_state_internal] Faction state initialized successfully");
    msg!("   Faction ID: {}", faction_id);
    msg!("   Faction name: {}", global_config.supported_factions[faction_id as usize]);

    Ok(())
}

/// Add a cranker bot to the whitelist (admin only)
/// Maximum MAX_CRANKER_BOTS bots can be whitelisted
pub fn add_cranker_bot_internal(
    ctx: Context<UpdateGameState>,
    bot_pubkey: Pubkey,
) -> Result<()> {
    msg!("🤖 [add_cranker_bot_internal] Adding cranker bot to whitelist");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Bot pubkey: {}", bot_pubkey);
    
    let global_game_state = &mut ctx.accounts.global_game_state;
    
    msg!("   Current cranker bots count: {}", global_game_state.cranker_bots.len());
    msg!("   Maximum allowed: {}", MAX_CRANKER_BOTS);
    
    // Check if bot is already whitelisted
    require!(
        !global_game_state.cranker_bots.contains(&bot_pubkey),
        ErrorCode::InvalidParameters // Bot already whitelisted
    );
    msg!("     ✓ Bot not already whitelisted");
    
    // Check if we've reached the maximum
    require!(
        global_game_state.cranker_bots.len() < MAX_CRANKER_BOTS,
        ErrorCode::InvalidParameters // Max cranker bots reached
    );
    msg!("     ✓ Under maximum limit");
    
    // Add bot to whitelist
    global_game_state.cranker_bots.push(bot_pubkey);
    msg!("   Added bot to whitelist");
    msg!("     New cranker bots count: {}", global_game_state.cranker_bots.len());
    
    msg!("✅ [add_cranker_bot_internal] Cranker bot added successfully");
    msg!("   Bot: {}", bot_pubkey);
    msg!("   Total whitelisted bots: {}", global_game_state.cranker_bots.len());
    
    Ok(())
}

/// Remove a cranker bot from the whitelist (admin only)
pub fn remove_cranker_bot_internal(
    ctx: Context<UpdateGameState>,
    bot_pubkey: Pubkey,
) -> Result<()> {
    msg!("🤖 [remove_cranker_bot_internal] Removing cranker bot from whitelist");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Bot pubkey: {}", bot_pubkey);
    
    let global_game_state = &mut ctx.accounts.global_game_state;
    
    msg!("   Current cranker bots count: {}", global_game_state.cranker_bots.len());
    
    // Find and remove bot
    let initial_count = global_game_state.cranker_bots.len();
    global_game_state.cranker_bots.retain(|&bot| bot != bot_pubkey);
    
    require!(
        global_game_state.cranker_bots.len() < initial_count,
        ErrorCode::InvalidParameters // Bot not found in whitelist
    );
    msg!("     ✓ Bot removed from whitelist");
    msg!("     New cranker bots count: {}", global_game_state.cranker_bots.len());
    
    msg!("✅ [remove_cranker_bot_internal] Cranker bot removed successfully");
    msg!("   Bot: {}", bot_pubkey);
    msg!("   Remaining whitelisted bots: {}", global_game_state.cranker_bots.len());
    
    Ok(())
}

// --------------------------------------------------------------------------------
// --------------------------------------------------------------------------------
// ------------ doge_btc_MINING :: INITIALIZATION & UPDATES ------------
// --------------------------------------------------------------------------------
// --------------------------------------------------------------------------------

/// Initialize mining by setting the token vault and starting timestamp
/// Can only be called once when mining_start_timestamp is 0
pub fn initialize_mining_internal(
    ctx: Context<InitializeMining>,
    start_timestamp: u64,
    doge_btc_per_slot: u64,
    pool_state: Pubkey,
) -> Result<()> {
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;

    // Check mining hasn't been initialized yet
    require!(
        doge_btc_mining.mining_start_timestamp == 0,
        ErrorCode::MiningAlreadyInitialized
    );

    let cur_slot = Clock::get()?.slot;

    // ───── persist vault + bump(s) ─────
    doge_btc_mining.dbtc_token_vault = ctx.accounts.token_vault.key();
    doge_btc_mining.vault_auth_bump = ctx.bumps.vault_authority;

    // Initialize mining parameters
    doge_btc_mining.mining_start_timestamp = start_timestamp;
    doge_btc_mining.doge_btc_per_slot = doge_btc_per_slot;
    doge_btc_mining.last_slot = cur_slot;

    // Initialize dynamic distribution fields
    doge_btc_mining.raydium_pool_state = pool_state;
    doge_btc_mining.last_rate_update = Clock::get()?.unix_timestamp;
    doge_btc_mining.current_dist_rate = doge_btc_per_slot;

    doge_btc_mining.price_history = Vec::with_capacity(8);
    doge_btc_mining.recent_price = 0; // Default: 0.001 SOL/DBTC
    doge_btc_mining.track_price = 0; // Initialize with same default

    doge_btc_mining.sol_for_pol = 0; // Initialize POL tracking
    doge_btc_mining.pol_stats = ProtocolOwnedLiquidity::default(); // Initialize POL stats tracking

    msg!("Initialized dynamic distribution system (30min snapshots, 4hr cycles) with Raydium pool: {}", pool_state);

    // Emit event
    emit!(MiningTokenVaultSet {
        authority: ctx.accounts.authority.key(),
        token_vault: ctx.accounts.token_vault.key(),
        token_vault_authority: ctx.accounts.vault_authority.key(),
        mining_start_timestamp: start_timestamp,
    });

    msg!(
        "Mining initialized with token vault: {}",
        ctx.accounts.token_vault.key()
    );

    Ok(())
}

/// Deposit moon doge tokens to the mining vault
pub fn deposit_doge_btc_tokens_internal(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
    token_if::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(), // TOKEN_2022_PROGRAM_ID
            token_if::TransferChecked {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.dbtc_token_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
        DBTC_DECIMALS,
    )?;

    msg!("Deposited {} MDOGE into mining vault", amount);
    Ok(())
}

 

// ----------------------------------------------------------------------------------------
// ------------ WITHDRAW SOL FEES ----------------------------------
// ----------------------------------------------------------------------------------------

/// Withdraw SOL fees from the treasury (excluding POL reserves)
/// This function allows the authorized fee_collector to withdraw SOL fees
/// but respects the sol_for_pol reserve for Protocol Owned Liquidity
pub fn withdraw_sol_fees_internal(ctx: Context<WithdrawSolFees>) -> Result<()> {
    let sol_treasury = &ctx.accounts.sol_treasury;
    let fee_collector = &ctx.accounts.fee_collector;
    let global_config = &ctx.accounts.global_config;

    msg!("Withdrawing SOL from treasury");
    msg!("SOL Treasury: {}", sol_treasury.key());
    msg!("Treasury balance: {} SOL", sol_treasury.lamports() as f64 / 1e9);
    msg!("Fee collector: {}", fee_collector.key());

    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let current_balance = sol_treasury.lamports();

    // Calculate available balance (total - rent)
    let reserved_amount = rent_exempt_amount;
    let available_solana = current_balance.saturating_sub(reserved_amount);

    // Check if we have enough available balance
    if available_solana == 0 {
        msg!(
            "⚠️ No SOL balance to withdraw. Available: {} SOL",
            available_solana as f64 / 1e9
        );        
        return Ok(());
    }
    msg!(
        "   Total balance: {} SOL, Rent: {} SOL",
        current_balance as f64 / 1e9,
        rent_exempt_amount as f64 / 1e9
    );

    // Calculate buybacks amount using configurable percentage
    let buyback_percentage = global_config.sol_fee_config.buyback_pct as u64;
    let sol_for_buybacks = available_solana
        .checked_mul(buyback_percentage)
        .unwrap()
        .checked_div(100)
        .unwrap();

    // Remaining amount goes to fee collector (for distribution to stakers and dev)
    let fee_collector_amount = available_solana
        .checked_sub(sol_for_buybacks)
        .unwrap();

    // Create signer seeds for sol_treasury
    let treasury_seeds = &[SOL_TREASURY_SEED.as_ref(), &[ctx.bumps.sol_treasury]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Transfer buybacks amount to buybacks SOL vault
    if sol_for_buybacks > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.buybacks_sol_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_buybacks,
        )?;

        // Update buybacks tracking
        ctx.accounts.buybacks_account.total_sol_accumulated = ctx
            .accounts
            .buybacks_account
            .total_sol_accumulated
            .checked_add(sol_for_buybacks)
            .unwrap();

        msg!(
            "💰 Transferred {} SOL to buybacks vault ({}%)",
            sol_for_buybacks as f64 / 1e9,
            buyback_percentage
        );
    }

    // Transfer remaining amount to fee collector
    if fee_collector_amount > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.fee_collector.to_account_info(),
                },
                signer_seeds,
            ),
            fee_collector_amount,
        )?;

        // Emit event
        emit!(SolFeesWithdrawn {
            fee_collector: fee_collector.key(),
            economy_program_amount: fee_collector_amount,
            buyback_amount: sol_for_buybacks,
        });
    }

    msg!("Withdrew {} SOL from treasury", fee_collector_amount as f64 / 1e9);
    Ok(())
}
// ----------------------------------------------------------------------------------------
// ------------ QUERY FUNCTIONS FOR EXTERNAL PROGRAMS ------------
// ----------------------------------------------------------------------------------------

/// Query treasury information for external programs
pub fn query_treasury_info_internal(ctx: Context<QueryTreasuryInfo>) -> Result<TreasuryInfo> {
    msg!("🔍 Querying treasury info");
    let sol_treasury = &ctx.accounts.sol_treasury;
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;
    let global_config = &ctx.accounts.global_config;

    let total_balance = sol_treasury.lamports();
    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let pol_reserves = doge_btc_mining.sol_for_pol;

    // Calculate available balance (total - POL reserve - rent)
    let reserved_amount = rent_exempt_amount
        .checked_add(pol_reserves)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let available_for_withdrawal = total_balance.saturating_sub(reserved_amount);

    msg!(
        "📊 Treasury: total={}, POL={}, rent={}, available={}",
        total_balance,
        pol_reserves,
        rent_exempt_amount,
        available_for_withdrawal
    );

    Ok(TreasuryInfo {
        total_balance,
        pol_reserves,
        rent_exempt_amount,
        available_for_withdrawal,
    })
}

/// Query global config information for external programs
pub fn query_global_config_internal(ctx: Context<QueryGlobalConfig>) -> Result<GlobalConfigInfo> {
    msg!("🔍 Querying global config");
    let global_config = &ctx.accounts.global_config;

    msg!(
        "📊 Config: active={}, authority={}",
        global_config.is_game_active,
        global_config.ext_authority
    );

    Ok(GlobalConfigInfo {
        is_game_active: global_config.is_game_active,
        ext_authority: global_config.ext_authority,
        ext_fee_collector: global_config.ext_fee_collector,
    })
}

/// Query token prices (dBTC and LP) for external programs
pub fn query_token_prices_internal(ctx: Context<QueryTokenPrices>) -> Result<TokenPricesInfo> {
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;

    Ok(TokenPricesInfo {
        dbtc_price_in_sol: doge_btc_mining.recent_price,
        lp_token_price_in_sol: doge_btc_mining.lp_token_price_in_sol,
    })
}

// --------------------------------------------------------------------------------
// ------------ GLOBAL_CONFIG :: INITIALIZE ------------
// --------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
        space = DogeBtcMining::LEN,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    /// CHECK: 0-byte PDA that only stores lamports (System Account)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfigAc<'info> {
    #[account(
        mut, 
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump,
    )]
    pub doge_btc_mining: Option<Account<'info, DogeBtcMining>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeMining<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    //  Vault authority PDA (0-byte, signer only)
    #[account(
        seeds = [DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required
    pub vault_authority: UncheckedAccount<'info>,

    // ─────────────────── token-2022 vault account ────────────────────
    #[account(
        init,
        payer  = authority,
        owner  = token_program.key(),
        seeds  = [DOGE_BTC_VAULT_SEED, doge_btc_mining.key().as_ref()],
        token::mint      = token_mint,
        token::authority = vault_authority,
        bump
    )]
    pub token_vault: InterfaceAccount<'info, TokenAccount2022>,

    // Mint created under Token-2022
    #[account(mut, owner = token_program.key())]
    pub token_mint: InterfaceAccount<'info, Mint2022>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateSlotsPerHour<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct DepositTokens<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        owner       = token_program.key(),                     // interface account check
        constraint  = depositor_token_account.owner == depositor.key() @ ErrorCode::Unauthorized,
        constraint  = depositor_token_account.mint  == dbtc_token_vault.mint @ ErrorCode::InvalidMint
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount2022>,

    // ─── mining token vault ───
    #[account(
        mut,
        seeds  = [DOGE_BTC_VAULT_SEED, doge_btc_mining.key().as_ref()],
        bump,
        owner  = token_program.key(),
    )]
    pub dbtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    #[account(owner = token_program.key())]
    pub token_mint: InterfaceAccount<'info, Mint2022>,

    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct CreateSystemReferralAccount<'info> {
    #[account(
        init,
        payer = user,
        space = ReferralRewards::LEN,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), system_program.key().as_ref()],
        bump,
    )]
    pub referrer_rewards: Account<'info, ReferralRewards>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}
 

/// Account struct for initializing buybacks system
#[derive(Accounts)]
pub struct InitializeBuybacks<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
        space = BuybacksAccount::LEN,
        seeds = [BUYBACKS_SEED.as_ref()],
        bump
    )]
    pub buybacks_account: Account<'info, BuybacksAccount>,

    /// CHECK: SOL vault for buybacks (0-byte PDA, System Account)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawSolFees<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    /// CHECK: SOL treasury PDA (System Account)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump  // Let Anchor find the correct bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    #[account(mut, signer, address = global_config.ext_fee_collector)]
    pub fee_collector: Signer<'info>,

    /// CHECK: Buybacks SOL vault PDA (System Account)
    #[account(
        mut,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    /// Buybacks tracking account (required)
    #[account(
        mut,
        seeds = [BUYBACKS_SEED.as_ref()],
        bump,
    )]
    pub buybacks_account: Account<'info, BuybacksAccount>,

    pub system_program: Program<'info, System>,
}

// ----------------------------------------------------------------------------------------
// ------------ QUERY ACCOUNT STRUCTS ------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct QueryTreasuryInfo<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    /// CHECK: SOL treasury PDA (System Account)
    #[account(
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
}
 

 











#[derive(Accounts)]
pub struct QueryGlobalConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
}

#[derive(Accounts)]
pub struct QueryTokenPrices<'info> {
    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
}

/// Create Dragon Egg collection with program PDA as authority
pub fn create_dragon_egg_collection_internal(
    ctx: Context<CreateDragonEggCollection>,
    name: String,
    uri: String,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let authority = &ctx.accounts.authority;

    // Verify authority
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    msg!("Creating Dragon Egg collection with program PDA as update authority");
    msg!("Collection: {}", ctx.accounts.collection.key());
    msg!(
        "Collection Authority PDA: {}",
        ctx.accounts.collection_authority.key()
    );

    // Get the collection authority bump for signing
    let collection_authority_bump = ctx.bumps.collection_authority;
    let _collection_authority_seeds = &[COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];

    // Create the collection using CPI
    let mpl_core_program = &ctx.accounts.mpl_core_program.to_account_info();
    let mut cpi_builder = CreateCollectionV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.authority.to_account_info())
        .update_authority(Some(&ctx.accounts.collection_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name.clone())
        .uri(uri.clone())
        .invoke()?;

    // Store the collection address in global config
    global_config.dragon_egg_collection = ctx.accounts.collection.key();

    emit!(DragonEggCollectionCreated {
        collection: ctx.accounts.collection.key(),
        update_authority: ctx.accounts.collection_authority.key(),
        name,
        uri,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct CreateDragonEggCollection<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Dragon Egg collection account (will be created by MPL Core)
    #[account(mut, signer)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: Collection authority PDA that will be the update authority
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddFaction<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Faction state PDA (validated in instruction)
    #[account(mut)]
    pub faction_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeGameState<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalGameSate::LEN,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateGameState<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(faction_id: u8)]
pub struct InitializeFactionState<'info> {
    #[account(
        init,
        payer = authority,
        space = FactionState::LEN,
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_id]],
        bump
    )]
    pub faction_state: Account<'info, FactionState>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
