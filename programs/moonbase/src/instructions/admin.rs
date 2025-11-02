use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use anchor_lang::system_program;
use mpl_core::{
    instructions::CreateCollectionV1CpiBuilder,
    ID as MPL_CORE_PROGRAM_ID,
};

use crate::errors::ErrorCode;


use anchor_spl::token_interface::{
    self as token_if,           // gives you CPI helpers such as `token_if::transfer`
    Mint as Mint2022,
    TokenAccount as TokenAccount2022,
};
use anchor_spl::token_2022::Token2022;          // ← the PROGRAM-ID wrapper (implements Id)

// Import Raydium CP-Swap for CPI calls
use raydium_cp_swap;

// constants
pub const MAX_MODULE_CONFIGS: usize = 50; // ≈ 8.2 kB

// Query data structures for external programs
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TreasuryInfo {
    pub total_balance: u64,
    pub pol_reserves: u64,
    pub rent_exempt_amount: u64,
    pub available_for_withdrawal: u64,
    pub loot_percentage: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GlobalConfigInfo {
    pub loot_percentage: u8,
    pub is_game_active: bool,
    pub base_creation_cost: u64,
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


pub fn internal_initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;

    // Initialize GlobalConfig
    global_config.ext_authority = ctx.accounts.authority.key();
    global_config.ext_fee_collector = ctx.accounts.authority.key(); // Initially set to authority, can be updated later
    global_config.creation_fee_recipient = creation_fee_recipient;

    // Store both PDA bumps for future derivation
    global_config.pda_sol_treasury = ctx.accounts.sol_treasury.key();
    global_config.treasury_bump = ctx.bumps.sol_treasury;

    global_config.total_moonbases_created = 0;
    global_config.total_sol_spent = 0;
    global_config.total_referral_sol_paid = 0;
    global_config.total_dragon_eggs_minted = 0;
    
    // Initialize egg limits: [unused, tier2_limit, tier3_limit, tier4_limit]
    global_config.egg_limits = [0, 5000, 5000, 5000];

    global_config.bump = ctx.bumps.global_config;
    global_config.base_creation_cost = base_creation_cost;
    global_config.loot_percentage = 10; // Default 10% for loot rewards
    global_config.is_game_active = false; // Default to false 
    
    // Initialize empty factions list
    global_config.supported_factions = Vec::new();

    // Optionally drop 1 lamport into the vault for future-proof rent-exempt status
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to:   ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        1,
    )?;

    // Initialize DogeBtcMining
    doge_btc_mining.dbtc_token_vault = Pubkey::default(); // Will be set during initialize_mining
    doge_btc_mining.mining_start_timestamp = 0; // Set to 0 to indicate mining not started
    doge_btc_mining.total_active_hashpower = 0;
    doge_btc_mining.total_active_electricity = 0;
    doge_btc_mining.doge_btc_per_slot = 0;
    doge_btc_mining.last_slot = 0;
    doge_btc_mining.total_tokens_mined = 0;
    doge_btc_mining.bump = ctx.bumps.doge_btc_mining;
    doge_btc_mining.vault_auth_bump = 0; // Will be set during initialize_mining
    
    // Initialize dynamic distribution fields with defaults
    doge_btc_mining.raydium_pool_state = Pubkey::default();
    doge_btc_mining.last_rate_update = 0;
    doge_btc_mining.current_dist_rate = 0;
    doge_btc_mining.price_history = Vec::new();
    doge_btc_mining.recent_price = 0; // Default: 0.001 SOL/DBTC
    doge_btc_mining.track_price = 0;
    doge_btc_mining.sol_for_pol = 0;
    doge_btc_mining.slots_for_swap = 450; // 3-minute periods
    
    msg!("Program initialized with creation cost: {}", base_creation_cost);
    msg!("SOL Treasury PDA created at: {} with bump: {}", ctx.accounts.sol_treasury.key(), ctx.bumps.sol_treasury);
    
    Ok(())
}


// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG URI MANAGEMENT (ADMIN) ---------------------------------------
// ----------------------------------------------------------------------------------------

/// Add Dragon Egg URIs to the pool (admin only)
pub fn add_dragon_egg_uris_internal(
    ctx: Context<UpdateConfigAc>,
    uris: Vec<String>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= MAX_URI_LENGTH, ErrorCode::UriTooLong);
    }

    // Add new URIs
    global_config.dragon_egg_uris.extend(uris.clone());

    msg!("✅ Added {} Dragon Egg URIs", uris.len());
    msg!("   Total Dragon Egg URIs: {}", global_config.dragon_egg_uris.len());

    Ok(())
}

/// Clear all Dragon Egg URIs (admin only)
pub fn clear_dragon_egg_uris_internal(
    ctx: Context<UpdateConfigAc>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    global_config.dragon_egg_uris.clear();

    msg!("✅ Cleared all Dragon Egg URIs");

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

/// Add factions to the global config (admin only)
pub fn add_factions_internal(
    ctx: Context<UpdateConfigAc>,
    factions: Vec<String>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    // Validate faction names
    for faction in &factions {
        require!(faction.len() > 0 && faction.len() <= 16, ErrorCode::InvalidFactionName);
    }

    // Check we don't exceed max factions (10)
    require!(
        global_config.supported_factions.len() + factions.len() <= 10,
        ErrorCode::MaxFactionsReached
    );

    // Add new factions
    global_config.supported_factions.extend(factions.clone());

    msg!("✅ Added {} factions", factions.len());
    msg!("   Total factions: {}", global_config.supported_factions.len());

    Ok(())
}


/// Update the global configuration parameters
/// Can only be called by the current authority
pub fn update_config_internal(
    ctx: Context<UpdateConfigAc>,
    new_authority: Option<Pubkey>,
    new_fee_collector: Option<Pubkey>,
    new_creation_fee_recipient: Option<Pubkey>,
    new_base_creation_cost: Option<u64>,
    new_loot_percentage: Option<u8>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Update fields if provided
    if let Some(authority) = new_authority {
        global_config.ext_authority = authority;
        msg!("Updated authority to {}", authority);
    }
    
    // Update SOL claimer if provided
    if let Some(fee_collector) = new_fee_collector {
        global_config.ext_fee_collector = fee_collector;
        msg!("Updated SOL claimer to {}", fee_collector);
    }
        
    // Update creation fee recipient if provided
    if let Some(creation_fee_recipient) = new_creation_fee_recipient {
        global_config.creation_fee_recipient = creation_fee_recipient;
        msg!("Updated creation fee recipient to {}", creation_fee_recipient);
    }
    
    // Update facility creation cost if provided
    if let Some(cost) = new_base_creation_cost {
        global_config.base_creation_cost = cost;
        msg!("Updated facility creation cost to {}", cost);
    }
    
    // Update loot percentage if provided
    if let Some(loot_percentage) = new_loot_percentage {
        require!(loot_percentage <= 100, ErrorCode::InvalidParameters);
        global_config.loot_percentage = loot_percentage;
        msg!("Updated loot percentage to {}%", loot_percentage);
    }
    
    Ok(())
}

/// Update egg limits for tiers (admin only)
pub fn update_egg_limits_internal(
    ctx: Context<UpdateConfigAc>,
    tier2_limit: Option<u64>,
    tier3_limit: Option<u64>,
    tier4_limit: Option<u64>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    if let Some(limit) = tier2_limit {
        global_config.egg_limits[1] = limit; // Tier 2 is at index 1
        msg!("Updated Tier 2 egg limit to {}", limit);
    }
    
    if let Some(limit) = tier3_limit {
        global_config.egg_limits[2] = limit; // Tier 3 is at index 2
        msg!("Updated Tier 3 egg limit to {}", limit);
    }
    
    if let Some(limit) = tier4_limit {
        global_config.egg_limits[3] = limit; // Tier 4 is at index 3
        msg!("Updated Tier 4 egg limit to {}", limit);
    }
    
    Ok(())
}


/// Add a new expansion configuration (admin only)
pub fn add_expansion_internal(
    ctx: Context<AddExpansion>,
    id: u8,
    name: String,
    required_level: u8,
    cost_sol: u64,
    new_width: u8,
    new_height: u8,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Validate expansion name length
    require!(
        name.len() > 0 && name.len() <= MAX_EXPANSION_NAME_LENGTH,
        ErrorCode::InvalidExpansionConfiguration
    );
    
    // Check if we've reached the maximum number of expansions
    require!(  global_config.expansions.len() < MAX_EXPANSIONS, ErrorCode::MaxExpansionsReached);
    
    // Check if expansion ID already exists
    for existing_expansion in &global_config.expansions {
        require!( existing_expansion.id != id, ErrorCode::ExpansionAlreadyExists );
    }
    
    // Validate dimensions
    require!(  new_width >= DEFAULT_MOONBASE_WIDTH && new_height >= DEFAULT_MOONBASE_HEIGHT,  ErrorCode::InvalidExpansionConfiguration);
    require!( new_width <= GRID_WIDTH && new_height <= GRID_HEIGHT,  ErrorCode::InvalidExpansionConfiguration);
    
    // Create the new expansion
    let expansion = ExpansionConfig {
        id,
        name: name.clone(),
        required_level,
        cost_sol,
        new_width,
        new_height,
        is_active: true,
    };
    
    // Add the expansion to the list
    global_config.expansions.push(expansion);
    
    msg!("Added new expansion '{}' (ID: {}) requiring level {} for {} SOL", 
         name, id, required_level, cost_sol);
    
    emit!(ExpansionAdded {
        authority: ctx.accounts.authority.key(),
        expansion_id: id,
        expansion_name: name,
        required_level,
        cost_sol,
        new_width,
        new_height,
    });
    
    Ok(())
}


// -------------------------------------------------------------------------------- 
// -------------------------------------------------------------------------------- 
// ------------ doge_btc_MINING :: INITIALIZATION & UPDATES ------------
// -------------------------------------------------------------------------------- 
// -------------------------------------------------------------------------------- 



/// Initialize mining by setting the token vault and starting timestamp
/// Can only be called once when mining_start_timestamp is 0
pub fn initialize_mining_internal(  ctx: Context<InitializeMining>, start_timestamp: u64, 
    doge_btc_per_slot: u64, pool_state: Pubkey) -> Result<()> {
    
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;

    // Check mining hasn't been initialized yet
    require!( doge_btc_mining.mining_start_timestamp == 0,  ErrorCode::MiningAlreadyInitialized);

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
    doge_btc_mining.slots_for_swap = 450; // Default: ~2.5 slots/second * 1800 seconds (3 mins)
    doge_btc_mining.pol_stats = ProtocolOwnedLiquidity::default(); // Initialize POL stats tracking

    msg!("Initialized dynamic distribution system (30min snapshots, 4hr cycles) with Raydium pool: {}", pool_state);

    // Emit event
    emit!(MiningTokenVaultSet {
            authority: ctx.accounts.authority.key(),
            token_vault: ctx.accounts.token_vault.key(),
            token_vault_authority: ctx.accounts.vault_authority.key(),
            mining_start_timestamp: start_timestamp,
    });

    msg!("Mining initialized with token vault: {}", 
    ctx.accounts.token_vault.key());

    Ok(())
}

/// Update slots per hour configuration (admin only)
pub fn update_slots_for_swap_internal(ctx: Context<UpdateSlotsPerHour>, new_slots_for_swap: u64) -> Result<()> {
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;

    require!(new_slots_for_swap > 0, ErrorCode::InvalidParameters);

    let old_slots_for_swap = doge_btc_mining.slots_for_swap;
    doge_btc_mining.slots_for_swap = new_slots_for_swap;

    msg!("Updated slots per hour from {} to {}", old_slots_for_swap, new_slots_for_swap);

    emit!(SlotsPerHourUpdated {
            authority: ctx.accounts.authority.key(),
            old_slots_for_swap,
            new_slots_for_swap,
    });

    Ok(())
}

/// Deposit moon doge tokens to the mining vault
pub fn deposit_doge_btc_tokens_internal(  ctx: Context<DepositTokens>,  amount: u64) -> Result<()> {
    token_if::transfer_checked(  CpiContext::new(  ctx.accounts.token_program.to_account_info(),      // TOKEN_2022_PROGRAM_ID
                                                    token_if::TransferChecked {
                                                        from:      ctx.accounts.depositor_token_account.to_account_info(),
                                                        mint:      ctx.accounts.token_mint.to_account_info(),
                                                        to:        ctx.accounts.dbtc_token_vault.to_account_info(),
                                                        authority: ctx.accounts.depositor.to_account_info(),
                                                    },
                                                ),
                                amount,  DBTC_DECIMALS)?;

    msg!("Deposited {} MDOGE into mining vault", amount);
    Ok(())
}


// ----------------------------------------------------------------------------------------
// ------------  LOOT REWARDS --------------------------------
// ----------------------------------------------------------------------------------------


/// Initialize the loot rewards system
pub fn initialize_loot_rewards_internal(ctx: Context<InitializeLootRewards>) -> Result<()> {
    let loot_rewards = &mut ctx.accounts.loot_rewards;
    let _clock = Clock::get()?;
    
    // Initialize loot rewards state
    loot_rewards.total_dbtc_accumulated = 0;
    loot_rewards.total_sol_accumulated = 0;
    loot_rewards.total_dbtc_distributed = 0;
    loot_rewards.total_sol_distributed = 0;
    loot_rewards.bump = ctx.bumps.loot_rewards;
    loot_rewards.sol_vault_bump = ctx.bumps.loot_sol_vault;
    loot_rewards.dbtc_vault_bump = ctx.bumps.loot_dbtc_vault;
    loot_rewards.dbtc_vault_authority_bump = ctx.bumps.loot_dbtc_vault_authority;
    
    emit!(LootRewardsInitialized {
        loot_rewards_pda: loot_rewards.key(),
        sol_vault_pda: ctx.accounts.loot_sol_vault.key(),
        dbtc_vault_pda: ctx.accounts.loot_dbtc_vault.key(),
    });
    
    msg!("🎁 Loot rewards system initialized");
    msg!("   Loot Rewards PDA: {}", loot_rewards.key());
    msg!("   SOL Vault: {}", ctx.accounts.loot_sol_vault.key());
    msg!("   DOGE_BTC Vault: {}", ctx.accounts.loot_dbtc_vault.key());
    msg!("   DOGE_BTC Vault Authority: {}", ctx.accounts.loot_dbtc_vault_authority.key());
    
    Ok(())
}

/// Initialize level statistics tracking (admin only)
pub fn initialize_level_stats_internal(ctx: Context<InitializeLevelStats>) -> Result<()> {
    msg!("🔒 Initializing level statistics tracking");
    
    let level_stats = &mut ctx.accounts.level_stats;
    
    // Initialize empty tracking for top levels
    level_stats.tracked_levels = Vec::with_capacity(LevelStats::MAX_TRACKED_LEVELS);
    level_stats.total_users = 0;
    level_stats.max_level_achieved = 0;
    level_stats.min_tracked_level = 0;
    level_stats.last_update_timestamp = Clock::get()?.unix_timestamp;
    level_stats.bump = ctx.bumps.level_stats;
    
    emit!(LevelStatsInitialized {
        level_stats_pda: ctx.accounts.level_stats.key(),
        tracked_levels: LevelStats::MAX_TRACKED_LEVELS as u8,
    });
    
    msg!("✅ Level statistics tracking initialized");
    Ok(())
}


// ----------------------------------------------------------------------------------------
// ------------ INITIALIZATION & UPDATES :: CONFIG-STOREs, MODULEs (stuff which can be installed in a moon-base) CONFIGS --------------------------------
// ----------------------------------------------------------------------------------------


/// Initialize store accounts for config types
pub fn initialize_config_stores_internal(ctx: Context<InitializeConfigStore>) -> Result<()> {
    let module_store = &mut ctx.accounts.module_config_store;
    
    // Initialize empty config stores with starting IDs
    module_store.next_id = 1; // IDs start at 1
    module_store.active_ids = Vec::new();
    module_store.bump = ctx.bumps.module_config_store;
    
    msg!("Initialized config stores for modules");
    
    Ok(())
}

/// Initialize a new module config that users can mint
pub fn add_module_to_base_internal(
    ctx: Context<AddModuleToConfigStore>,
    name: String,
    image_url: String,
    module_type: u8,
    faction_ids: Vec<u8>,
    min_level: u8,
    width: u8,
    height: u8,
    mint_cost: u64,
    upgrade_cost: u64,
    upgrade_level_requirements: Vec<u8>,
) -> Result<()> {
    let module_config_store = &mut ctx.accounts.module_config_store;
    let module_config_account = &mut ctx.accounts.module_config_account;
    let global_config = &ctx.accounts.global_config;
    
    // Validate authority
    require!(
        global_config.ext_authority == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );

    // Convert u8 to ModuleType enum
    // 1 = Mining, 2 = Attraction
    let module_type_enum = match module_type {
        1 => ModuleType::Mining,
        2 => ModuleType::Attraction,
        _ => return Err(ErrorCode::InvalidModuleType.into()),
    };

    msg!("Adding module: {}", name);
    msg!("Image URL: {}", image_url);
    msg!("Module type: {:?} (from u8: {})", module_type_enum, module_type);

    // Build ModuleStats with placeholder values (will be updated later)
    let stats: ModuleStats = match module_type_enum {
        ModuleType::Mining => {
            ModuleStats::Mining(MiningStats {
                max_hp: 0,
                base_hashpower: 0, // Placeholder - must be updated
                power_consumption: 0,
            })
        },
        ModuleType::Attraction => {
            ModuleStats::Attraction(AttractionStats {
                max_hp: 0,
                base_xp_per_hour: 0, // Placeholder - must be updated
                power_consumption: 0,
            })
        },
    };

    msg!("Stats (placeholders): {:?}", stats);
    msg!("Faction IDs: {:?}", faction_ids);
    msg!("Min level: {}", min_level);
    msg!("Width: {}", width);
    
    // Validate inputs
    require!(name.len() <= 32, ErrorCode::InvalidModuleName);
    require!(image_url.len() <= 64, ErrorCode::InvalidImageUrl);
    require!(upgrade_level_requirements.len() <= MAX_MODULE_UPGRADES as usize, ErrorCode::InvalidUpgradeConfiguration);
    require!(faction_ids.len() <= MAX_FACTION_IDS_PER_MODULE, ErrorCode::TooManyFactionIds);
    
    // Validate that upgrade level requirements are increasing and start at or above min_level
    let mut prev_level = min_level;
    for (i, &required_level) in upgrade_level_requirements.iter().enumerate() {
        require!(
            required_level >= prev_level,
            ErrorCode::InvalidUpgradeConfiguration
        );
        prev_level = required_level;
        
        msg!("Upgrade {} requires moonbase level {}", i + 1, required_level);
    }
    
    // Validate faction IDs exist in global config (if any specified)
    if !faction_ids.is_empty() {
        for faction_id in &faction_ids {
            require!(
                (*faction_id as usize) < global_config.supported_factions.len(),
                ErrorCode::InvalidFactionId
            );
        }
    }
    
    // Get the next ID and increment it
    let id = module_config_store.next_id;
    module_config_store.next_id = module_config_store.next_id.checked_add(1)
        .ok_or(ErrorCode::IdOverflow)?;
    
    // Create the new module config in the individual PDA account
    module_config_account.data = ModuleConfig {
        id,
        name: name.clone(),
        image_url: image_url.clone(),
        module_type: module_type_enum,
        stats,
        faction_ids,
        min_level,
        width,
        height,
        mint_cost,
        upgrade_cost,
        upgrade_level_requirements: upgrade_level_requirements.clone(),
        is_active: false, // Inactive until stats are properly set
    };
    
    // Set the bump for the individual config account
    module_config_account.bump = ctx.bumps.module_config_account;
    
    // Add ID to active_ids list for enumeration
    module_config_store.active_ids.push(id);
    
    // Log the creation
    msg!("Initialized new module config: {}, ID: {}, Image: {}", name, id, image_url);
    msg!("Max upgrades: {}, upgrade levels: {:?}", upgrade_level_requirements.len(), &upgrade_level_requirements);
    msg!("⚠️ Module is INACTIVE until stats are properly configured");
    
    // Emit event
    emit!(NewModuleConfigCreated {
        id,
        name,
    });
    
    Ok(())
}

/// Update module stats and activate the module
pub fn update_module_stats_internal(
    ctx: Context<UpdateModuleStats>,
    id: u16,
    max_hp: u32,
    power_consumption: u16,
    base_hashpower: u32,
    base_xp_per_hour: u32,
) -> Result<()> {
    let module_config_account = &mut ctx.accounts.module_config_account;
    let config = &mut module_config_account.data;
    
    // Verify this is the correct config ID
    require!(config.id == id, ErrorCode::ConfigNotFound);

    msg!("Updating stats for module: {} (ID: {})", config.name, id);
    msg!("Module type: {:?}", config.module_type);

    // Build new ModuleStats and validate required fields are not 0
    let new_stats: ModuleStats = match config.module_type {
        ModuleType::Mining => {
            require!(base_hashpower > 0, ErrorCode::InvalidModuleConfiguration);
            msg!("Mining stats - base_hashpower: {}", base_hashpower);
            ModuleStats::Mining(MiningStats {
                max_hp,
                base_hashpower,
                power_consumption,
            })
        },
        ModuleType::Attraction => {
            require!(base_xp_per_hour > 0, ErrorCode::InvalidModuleConfiguration);
            msg!("Attraction stats - base_xp_per_hour: {}", base_xp_per_hour);
            ModuleStats::Attraction(AttractionStats {
                max_hp,
                base_xp_per_hour,
                power_consumption,
            })
        },
    };

    // Update the stats
    config.stats = new_stats;
    
    // Activate the module now that stats are properly set
    config.is_active = true;
    
    msg!("✅ Module stats updated and module activated");
    msg!("New stats: {:?}", config.stats);
    
    // Emit event
    emit!(ModuleConfigUpdated {
        id,
        name: config.name.clone(),
    });
    
    Ok(())
}
 

/// Update an existing module config
pub fn update_module_internal(
    ctx: Context<UpdateModuleConfig>,
    id: u16,
    image_url: Option<String>,
    faction_ids: Option<Vec<u8>>,
    mint_cost: Option<u64>,
    upgrade_cost: Option<u64>,
    upgrade_level_requirements: Option<Vec<u8>>,
    is_active: Option<bool>,
) -> Result<()> {
    let module_config_account = &mut ctx.accounts.module_config_account;
    let config = &mut module_config_account.data;
    
    // Verify this is the correct config ID
    require!(config.id == id, ErrorCode::ConfigNotFound);
    
    // Update fields if provided
    if let Some(new_url) = image_url {
        config.image_url = new_url.clone();
        msg!("Updated module image URL to: {}", new_url);
    }

    if let Some(new_mint_cost) = mint_cost {
        config.mint_cost = new_mint_cost;
        msg!("Updated mint cost to: {}", new_mint_cost);
    }
    
    if let Some(new_upgrade_cost) = upgrade_cost {
        config.upgrade_cost = new_upgrade_cost;
        msg!("Updated upgrade cost to: {}", new_upgrade_cost);
    }
    
    if let Some(new_is_active) = is_active {
        config.is_active = new_is_active;
        msg!("Updated active status to: {}", new_is_active);
    }
    
    // Handle upgrade_level_requirements
    if let Some(new_upgrade_requirements) = upgrade_level_requirements {
        // Validate that the number of requirements doesn't exceed max
        require!(
            new_upgrade_requirements.len() <= MAX_MODULE_UPGRADES as usize,
            ErrorCode::InvalidUpgradeConfiguration
        );
        
        // Validate that upgrade level requirements are increasing and start at or above min_level
        let mut prev_level = config.min_level;
        for (_i, &required_level) in new_upgrade_requirements.iter().enumerate() {
            require!(
                required_level >= prev_level,
                ErrorCode::InvalidUpgradeConfiguration
            );
            prev_level = required_level;
        }
        
        config.upgrade_level_requirements = new_upgrade_requirements.clone();
        msg!("Updated upgrade level requirements to: {:?} (max upgrades: {})", new_upgrade_requirements, new_upgrade_requirements.len());
    }
    
    if let Some(new_faction_ids) = faction_ids {
        require!(new_faction_ids.len() <= MAX_FACTION_IDS_PER_MODULE, ErrorCode::TooManyFactionIds);
        config.faction_ids = new_faction_ids;
        msg!("Updated faction IDs");
    }
    
    // Emit event
    emit!(ModuleConfigUpdated {
        id,
        name: config.name.clone(),
    });
    
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
    msg!("Treasury balance: {}", sol_treasury.lamports());
    msg!("Fee collector: {}", fee_collector.key());
 
    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let current_balance = sol_treasury.lamports();
    
    // Calculate available balance (total - rent)
    let reserved_amount = rent_exempt_amount;    
    let available_balance = current_balance.saturating_sub(reserved_amount);
    
    // Check if we have enough available balance
    if available_balance == 0 {
        msg!("⚠️ No available balance to withdraw. Available: {}", available_balance);
        msg!("   Total balance: {}, Rent: {}", current_balance, rent_exempt_amount);
        return Err(ErrorCode::InsufficientTreasuryFunds.into());
    }

    // Calculate loot rewards amount using configurable percentage
    let loot_percentage = global_config.loot_percentage as u64;
    let loot_amount = available_balance.checked_mul(loot_percentage).unwrap().checked_div(100).unwrap();
    let fee_collector_amount = available_balance.checked_sub(loot_amount).unwrap();

    // Transfer loot rewards to loot SOL vault (required)
    // Transfer loot amount to loot SOL vault
    **sol_treasury.try_borrow_mut_lamports()? = current_balance.checked_sub(loot_amount).unwrap();
    **ctx.accounts.loot_sol_vault.try_borrow_mut_lamports()? += loot_amount;

    // Update loot rewards tracking
    ctx.accounts.loot_rewards.total_sol_accumulated = ctx.accounts.loot_rewards.total_sol_accumulated.checked_add(loot_amount).unwrap();
    
    emit!(LootRewardsAccumulated {
        dbtc_amount: 0,
        sol_amount: loot_amount,
        total_dbtc_accumulated: ctx.accounts.loot_rewards.total_dbtc_accumulated,
        total_sol_accumulated: ctx.accounts.loot_rewards.total_sol_accumulated,
    });

    msg!("🎁 Transferred {} SOL to loot rewards vault ({}%)", loot_amount, loot_percentage);
    
    // Transfer remaining amount to fee collector
    let remaining_balance = current_balance.saturating_sub(available_balance);
    **sol_treasury.try_borrow_mut_lamports()? = remaining_balance;
    **fee_collector.try_borrow_mut_lamports()? += fee_collector_amount;

    // Emit event
    emit!(SolFeesWithdrawn {
        fee_collector: fee_collector.key(),
        amount: fee_collector_amount,
        loot_amount,
    });

    msg!("Withdrew {} lamports from treasury (Available balance now: {})", fee_collector_amount, remaining_balance.saturating_sub(reserved_amount));

    Ok(())
}

 
// ----------------------------------------------------------------------------------------
// ------------ QUERY FUNCTIONS FOR EXTERNAL PROGRAMS ------------
// ----------------------------------------------------------------------------------------

/// Query treasury information for external programs
pub fn query_treasury_info_internal(ctx: Context<QueryTreasuryInfo>) -> Result<TreasuryInfo> {
    let sol_treasury = &ctx.accounts.sol_treasury;
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;
    let global_config = &ctx.accounts.global_config;

    let total_balance = sol_treasury.lamports();
    let rent_exempt_amount = Rent::get()?.minimum_balance(sol_treasury.data_len());
    let pol_reserves = doge_btc_mining.sol_for_pol;
    
    // Calculate available balance (total - POL reserve - rent)
    let reserved_amount = rent_exempt_amount.checked_add(pol_reserves)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let available_for_withdrawal = total_balance.saturating_sub(reserved_amount);

    Ok(TreasuryInfo {
        total_balance,
        pol_reserves,
        rent_exempt_amount,
        available_for_withdrawal,
        loot_percentage: global_config.loot_percentage,
    })
}

/// Query global config information for external programs
pub fn query_global_config_internal(ctx: Context<QueryGlobalConfig>) -> Result<GlobalConfigInfo> {
    let global_config = &ctx.accounts.global_config;

    Ok(GlobalConfigInfo {
        loot_percentage: global_config.loot_percentage,
        is_game_active: global_config.is_game_active,
        base_creation_cost: global_config.base_creation_cost,
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



// ----------------------------------------------------------------------------------------
// ------------ DYNAMIC DISTRIBUTION FUNCTIONS :: ORACLE & RATE UPDATES -----------------
// ----------------------------------------------------------------------------------------
 

/// Calculate price change percentage between old and new price
/// Returns (change_pct, direction) where direction: 1=increase, -1=decrease, 0=same
fn calculate_price_change_pct(old_price: u64, new_price: u64) -> (i64, i64) {
    if old_price == 0 || new_price == 0 {
        return (0, 0);
    }
    
    let old = old_price as i128;
    let new = new_price as i128;
    
    // Calculate percentage change: ((new - old) / old) × 100
    let diff = new - old;
    let change_pct = (diff * 100) / old;
    
    let direction = if new > old {
        1
    } else if new < old {
        -1
    } else {
        0
    };
    
    (change_pct as i64, direction)
}

/// Update DBTC distribution rate based on price oracle
/// This function can be called by anyone every 30 minutes
/// Distribution rate updated every 4 hours with 3% deadband
pub fn update_dbtc_dist_per_slot_internal(ctx: Context<UpdateMdogeDistPerSlot>, lp_token_amount: u64) -> Result<()> {
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Check if admin override is being used (when lp_token_amount > 0)
    if lp_token_amount > 0 {
        // Verify that the authority is provided and matches the global config
        require!(ctx.accounts.authority.is_some(), ErrorCode::Unauthorized);
        let authority = ctx.accounts.authority.as_ref().unwrap();
        require!(
            ctx.accounts.global_config.ext_authority == authority.key(),
            ErrorCode::Unauthorized
        );
        msg!("🔧 Admin override: Using LP token amount {}", lp_token_amount);
    } else {
        msg!("🔄 Using automatic LP calculation");
    }
    
    // Check if at least 30 minutes has passed since last update
    let thirty_mins = THIRTY_MINS as i64;
    if current_time < doge_btc_mining.last_rate_update + thirty_mins {
        msg!("⏰ Update too early - must wait at least 30 minutes between updates");
        msg!("   Current time: {}, Next update allowed: {}, remaining seconds: {}", current_time, doge_btc_mining.last_rate_update + thirty_mins, (doge_btc_mining.last_rate_update + thirty_mins - current_time));
        return Ok(());
    }
    
    msg!("🔄 Starting DOGE_BTC distribution rate update");
    msg!("   Current time: {}", current_time);
    msg!("   Last update: {}", doge_btc_mining.last_rate_update);
    msg!("   Current dist rate: {}", doge_btc_mining.current_dist_rate);
    
    // Calculate DBTC for liquidity based on current distribution rate and slots
    let dbtc_for_liquidity = doge_btc_mining.current_dist_rate.checked_mul(doge_btc_mining.slots_for_swap).ok_or(ErrorCode::ArithmeticOverflow)?;
        
    msg!("   Price snapshot {}/8: Swapping {} DOGE_BTC for SOL", 
         doge_btc_mining.price_history.len() + 1, dbtc_for_liquidity);
    
    // Perform swap via Raydium CPI to get current exchange rate
    let sol_received = perform_dbtc_to_sol_swap(
        &ctx.accounts.raydium_program,
        &ctx.accounts.pool_state,
        &ctx.accounts.amm_config,
        &ctx.accounts.authority_pda,
        &ctx.accounts.raydium_authority,
        &ctx.accounts.dbtc_vault,
        &ctx.accounts.sol_vault,
        &ctx.accounts.dbtc_token_account,
        &ctx.accounts.sol_token_account,
        &ctx.accounts.dbtc_mint,
        &ctx.accounts.sol_mint,
        &ctx.accounts.observation_state,
        &ctx.accounts.token_program_2022,
        &ctx.accounts.token_program,
        dbtc_for_liquidity,
        doge_btc_mining.vault_auth_bump,
    )?;
    
    // Calculate current price (SOL per DOGE_BTC) with proper decimal handling
    // sol_received is in WSOL base units (9 decimals), dbtc_for_liquidity is in DOGE_BTC base units (6 decimals)
    // 
    // Formula: Price = (sol_received / 10^9) / (dbtc_for_liquidity / 10^6)
    // Simplified: Price = (sol_received * 10^6) / (dbtc_for_liquidity * 10^9)
    // To store with 9-decimal precision: multiply by 10^9
    // Final: Price = (sol_received * 10^6 * 10^9) / (dbtc_for_liquidity * 10^9) = (sol_received * 10^6) / dbtc_for_liquidity
    let current_price = if dbtc_for_liquidity > 0 {
        // Prevent overflow by checking limits
        if sol_received > crate::state::MAX_SAFE_U64 || dbtc_for_liquidity > crate::state::MAX_SAFE_U64 {
            msg!("⚠️ Price calculation values too large, using fallback");
            0
        } else {
            // Calculate: (sol_received * 10^9) / dbtc_for_liquidity
            // This gives us SOL per DOGE_BTC stored with 9-decimal precision
            (sol_received as u128)
                .checked_mul(1_000_000_000) // Scale by 10^9 for full precision
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(dbtc_for_liquidity as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .min(u64::MAX as u128) as u64
        }
    } else {
        0
    };
    
    // Calculate human-readable price for logging
    // Convert back to actual SOL per DOGE_BTC
    let actual_price = current_price as f64 / 1_000_000_000.0;
    msg!("   Swap details: {} DOGE_BTC base units → {} WSOL base units", dbtc_for_liquidity, sol_received);
    msg!("   Human readable: {} DOGE_BTC → {:.9} SOL", dbtc_for_liquidity / 1_000_000, sol_received as f64 / 1_000_000_000.0);
    msg!("   Current price: {} (9-decimal precision), Actual: {:.9} SOL per DOGE_BTC", 
         current_price, actual_price);
    
    // Add current price to history
    let price_entry = PriceEntry {
        timestamp: current_time,
        price: current_price,
    };
    
    // Add price entry to history
    doge_btc_mining.price_history.push(price_entry);
    
    // Accumulate SOL for POL
    doge_btc_mining.sol_for_pol = doge_btc_mining.sol_for_pol.checked_add(sol_received).unwrap();
    
    // Calculate ongoing weighted average (even before 4 hours)
    let mut weighted_sum: u128 = 0;
    let mut total_weights: u128 = 0;
    
    for (i, entry) in doge_btc_mining.price_history.iter().enumerate() {
        let weight = (i + 1) as u128; // Weight from 1 to 8
        
        let price_contribution = (entry.price as u128)
            .checked_mul(weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        weighted_sum = weighted_sum
            .checked_add(price_contribution)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        total_weights = total_weights
            .checked_add(weight)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    let current_weighted_avg = if total_weights > 0 {
        (weighted_sum / total_weights).min(u64::MAX as u128) as u64
    } else {
        current_price
    };
    
    // Update recent price with current weighted average
    doge_btc_mining.recent_price = current_weighted_avg;
    
    msg!("   💰 Accumulated {} WSOL for POL, total reserve: {}", sol_received, doge_btc_mining.sol_for_pol);
    msg!("   📊 Ongoing weighted average: {} (from {} snapshots)", current_weighted_avg, doge_btc_mining.price_history.len());
    msg!("   🎯 Track price (last rate change): {}", doge_btc_mining.track_price);
    
    // Update timestamp for next snapshot
    doge_btc_mining.last_rate_update = current_time;
        
    // ----------------------------------------------------
    // Check if 4 hours have passed AND we have 8 price entries
    // Only then check if distribution rate should change
    // ----------------------------------------------------
    let four_hours = FOUR_HOURS as i64;
    let time_since_last = doge_btc_mining.price_history.first()
        .map(|e| current_time - e.timestamp)
        .unwrap_or(0);
    
    if doge_btc_mining.price_history.len() < 8 || time_since_last < four_hours {
        msg!("   ⏰ Not ready for rate update: {} snapshots, {} seconds elapsed (need 8 snapshots over 4 hours)", 
             doge_btc_mining.price_history.len(), time_since_last);
        return Ok(());
    }
    
    // ----------------------------------------------------
    // 4 hours completed - Check if rate should change
    // ----------------------------------------------------
    msg!("   ✅ 4-hour cycle complete with {} snapshots", doge_btc_mining.price_history.len());
    
    let new_avg_price = current_weighted_avg;
    
    // Initialize track_price if first time
    if doge_btc_mining.track_price == 0 {
        doge_btc_mining.track_price = new_avg_price;
        msg!("   🎯 Initialized track_price: {}", doge_btc_mining.track_price);
    }
    
    // Calculate price change percentage from BOTH recent and track prices
    // Use the LARGER change to determine if we should update
    let change_from_track = calculate_price_change_pct(doge_btc_mining.track_price, new_avg_price);
    
    // For recent_price, use the oldest entry in history (4 hours ago)
    let recent_comparison_price = doge_btc_mining.price_history.first()
        .map(|e| e.price)
        .unwrap_or(new_avg_price);
    let change_from_recent = calculate_price_change_pct(recent_comparison_price, new_avg_price);
    
    msg!("   📊 Price changes: from track_price ({}): {}%, from 4h ago ({}): {}%", 
         doge_btc_mining.track_price, change_from_track.0,
         recent_comparison_price, change_from_recent.0);
    
    // Pick the larger change (by absolute value)
    let (price_change_pct, direction) = if change_from_track.0.abs() > change_from_recent.0.abs() {
        msg!("   🎯 Using change from track_price (larger movement)");
        change_from_track
    } else {
        msg!("   🎯 Using change from 4h ago price (larger movement)");
        change_from_recent
    };
    
    msg!("   📈 Selected price change: {}% (direction: {})", price_change_pct, direction);
    
    // Check if change exceeds 3% threshold
    let old_rate = doge_btc_mining.current_dist_rate;
    let mut rate_changed = false;
    
    if price_change_pct.abs() < PRICE_CHANGE_THRESHOLD as i64 {
        msg!("   ➡️ Price change {}% within ±3% deadband, keeping same distribution rate", price_change_pct);
        // Don't update track_price, keep monitoring
    } else if direction > 0 {
        // Price increased by >3% - increase distribution by 1%
        doge_btc_mining.current_dist_rate = doge_btc_mining.current_dist_rate
            .checked_mul(101)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("   📈 Price increased {}%! Increasing distribution rate by 1%", price_change_pct);
        rate_changed = true;
    } else {
        // Price decreased by >3% - decrease distribution by 3%
        doge_btc_mining.current_dist_rate = doge_btc_mining.current_dist_rate
            .checked_mul(97)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("   📉 Price decreased {}%! Decreasing distribution rate by 3%", price_change_pct);
        rate_changed = true;
    }
    
    // Update track_price only if rate actually changed
    if rate_changed {
        doge_btc_mining.track_price = new_avg_price;
        msg!("   🎯 Updated track_price to: {}", doge_btc_mining.track_price);
    }
    
    // Calculate amounts for LP addition using UPDATED distribution rate
    // Use accumulated SOL (already includes current swap)
    let total_sol_for_lp = doge_btc_mining.sol_for_pol;
    
    msg!("   🏦 Adding liquidity: {} WSOL (accumulated over 4 hours)", total_sol_for_lp);
    
    // Note: WSOL is already in our sol_token_account from swaps, no need to withdraw from treasury
    
    // Perform actual LP addition and burn
    perform_lp_addition_and_burn(
        &ctx.accounts.raydium_program,
        &ctx.accounts.pool_state,
        &ctx.accounts.authority_pda,
        &ctx.accounts.raydium_authority,
        &ctx.accounts.dbtc_vault,
        &ctx.accounts.sol_vault,
        &ctx.accounts.dbtc_token_account,
        &ctx.accounts.sol_token_account,
        &ctx.accounts.lp_token_account,
        &ctx.accounts.lp_mint,
        &ctx.accounts.dbtc_mint,
        &ctx.accounts.sol_mint,
        &ctx.accounts.token_program_2022,
        &ctx.accounts.token_program,
        total_sol_for_lp,
        doge_btc_mining.vault_auth_bump,
        doge_btc_mining,
        lp_token_amount, // Pass the LP token amount
    )?;
    
    // Check actual WSOL balance after LP addition to see how much was consumed
    let wsol_balance_after_lp = {
        let sol_account_data = ctx.accounts.sol_token_account.try_borrow_data()?;
        let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
        sol_token_data.amount
    };
    
    // Calculate how much SOL was actually consumed for LP
    let sol_consumed_for_lp = total_sol_for_lp.saturating_sub(wsol_balance_after_lp);
    
    // Update POL tracking - subtract only the amount actually used
    doge_btc_mining.sol_for_pol = wsol_balance_after_lp; // Keep any leftover WSOL for next cycle
    
    msg!("   💰 SOL consumption: {} total available, {} consumed for LP, {} remaining", 
         total_sol_for_lp, sol_consumed_for_lp, doge_btc_mining.sol_for_pol);
    
    // Clear price history to restart the 4-hour cycle
    doge_btc_mining.price_history.clear();
    
    // Update state
    doge_btc_mining.recent_price = new_avg_price; // Store as recent for next cycle
    doge_btc_mining.last_rate_update = current_time;
    
    msg!("   🔄 Price history cleared - restarting 4-hour accumulation cycle");
    msg!("   🎯 Distribution rate: {} -> {} ({})", 
         old_rate, doge_btc_mining.current_dist_rate,
         if rate_changed { "CHANGED" } else { "unchanged" });
    
    emit!(DistributionRateUpdated {
        old_rate,
        new_rate: doge_btc_mining.current_dist_rate,
        price_change_pct: price_change_pct as i32,
        current_price,
        avg_price_4h: new_avg_price,
        track_price: doge_btc_mining.track_price,
        recent_price: doge_btc_mining.recent_price,
        rate_changed,
        sol_received,
        timestamp: current_time,
    });
    
    Ok(())
}

/// Helper function to perform DOGE_BTC to SOL swap via Raydium CPI
fn perform_dbtc_to_sol_swap<'info>(
    raydium_program: &AccountInfo<'info>,
    pool_state: &AccountInfo<'info>,
    amm_config: &AccountInfo<'info>,
    authority_pda: &AccountInfo<'info>,
    raydium_authority: &AccountInfo<'info>,
    dbtc_vault: &AccountInfo<'info>,
    sol_vault: &AccountInfo<'info>,
    dbtc_token_account: &AccountInfo<'info>,
    sol_token_account: &AccountInfo<'info>,
    dbtc_mint: &AccountInfo<'info>,
    sol_mint: &AccountInfo<'info>,
    observation_state: &AccountInfo<'info>,
    token_program_2022: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount_in: u64,
    vault_auth_bump: u8,
) -> Result<u64> {
    use raydium_cp_swap::cpi;
    
    msg!("🔄 Performing real Raydium swap: {} DOGE_BTC for WSOL", amount_in);
    
    // Get WSOL token balance before swap by deserializing account data
    let sol_balance_before = {
        let sol_account_data = sol_token_account.try_borrow_data()?;
        let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
        sol_token_data.amount
    }; // Borrow is dropped here
    
    // Create signer seeds for vault authority
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    
    // Create CPI context for Raydium swap
    let cpi_accounts = cpi::accounts::Swap {
        payer: authority_pda.to_account_info(),         // Our PDA as the payer/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        amm_config: amm_config.to_account_info(),
        pool_state: pool_state.to_account_info(),
        input_token_account: dbtc_token_account.to_account_info(),  // Our token account (authority = our PDA)
        output_token_account: sol_token_account.to_account_info(),   // Our token account (authority = our PDA)
        input_vault: dbtc_vault.to_account_info(),     // Raydium's DOGE_BTC vault
        output_vault: sol_vault.to_account_info(),      // Raydium's SOL vault  
        input_token_program: token_program_2022.to_account_info(),   // Token-2022 for DOGE_BTC
        output_token_program: token_program.to_account_info(),       // Standard token for SOL
        input_token_mint: dbtc_mint.to_account_info(),
        output_token_mint: sol_mint.to_account_info(),
        observation_state: observation_state.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        raydium_program.to_account_info(),
        cpi_accounts,
        signer_seeds,
    );
    
    // Accept any amount out since we're just getting current market price
    let min_amount_out = 0;
    
    // Perform the actual swap
    cpi::swap_base_input(cpi_ctx, amount_in, min_amount_out)?;
    
    // Calculate actual WSOL received by checking token account balance again
    let sol_received = {
        let sol_account_data_after = sol_token_account.try_borrow_data()?;
        let sol_token_data_after = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data_after[..])?;
        let sol_balance_after = sol_token_data_after.amount;
        sol_balance_after.saturating_sub(sol_balance_before)
    }; // Borrow is dropped here
    
    msg!("✅ Swap completed: received {} WSOL tokens", sol_received);
    
    Ok(sol_received)
}

/// Helper function to add liquidity to Raydium pool and burn LP tokens
fn perform_lp_addition_and_burn<'info>(
    raydium_program: &AccountInfo<'info>,
    pool_state: &AccountInfo<'info>,
    authority_pda: &AccountInfo<'info>,
    raydium_authority: &AccountInfo<'info>,
    dbtc_vault: &AccountInfo<'info>,
    sol_vault: &AccountInfo<'info>,
    dbtc_token_account: &AccountInfo<'info>,
    sol_token_account: &AccountInfo<'info>,
    lp_token_account: &AccountInfo<'info>,
    lp_mint: &AccountInfo<'info>,
    dbtc_mint: &AccountInfo<'info>,
    sol_mint: &AccountInfo<'info>,
    token_program_2022: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    sol_amount: u64,
    vault_auth_bump: u8,
    doge_btc_mining: &mut Account<DogeBtcMining>,
    admin_lp_override: u64,
) -> Result<()> {

    
    msg!("🏦 Starting LP addition: {} SOL", sol_amount);
    
    // Create signer seeds for vault authority
    let authority_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[vault_auth_bump],
    ];
    let signer_seeds = &[&authority_seeds[..]];
    
    // Step 1: Get LP token balance before deposit to calculate actual minted amount
    let lp_balance_before = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let lp_account_data = lp_token_account.try_borrow_data()?;
        let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
        lp_account.amount
    };
    
    msg!("💰 LP token balance before deposit: {}", lp_balance_before);
    
    // Step 2: Use actual Raydium CPI for deposit
    msg!("🏦 Creating CPI context for Raydium deposit");
    
    let cpi_accounts = raydium_cp_swap::cpi::accounts::Deposit {
        owner: authority_pda.to_account_info(),        // Our vault authority as the owner/signer
        authority: raydium_authority.to_account_info(), // Raydium's pool authority PDA
        pool_state: pool_state.to_account_info(),
        owner_lp_token: lp_token_account.to_account_info(), // LP token account (authority = our PDA)
        token_0_account: sol_token_account.to_account_info(),   // Our SOL account (authority = our PDA) - token0 is WSOL
        token_1_account: dbtc_token_account.to_account_info(), // Our DOGE_BTC account (authority = our PDA) - token1 is DOGE_BTC
        token_0_vault: sol_vault.to_account_info(),      // Raydium's SOL vault - token0 vault
        token_1_vault: dbtc_vault.to_account_info(),    // Raydium's DOGE_BTC vault - token1 vault
        token_program: token_program.to_account_info(),  // Standard token program
        token_program_2022: token_program_2022.to_account_info(), // Token-2022 program
        vault_0_mint: sol_mint.to_account_info(),        // SOL mint - token0 mint
        vault_1_mint: dbtc_mint.to_account_info(),      // DOGE_BTC mint - token1 mint
        lp_mint: lp_mint.to_account_info(),              // Raydium's LP mint
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        raydium_program.to_account_info(),
        cpi_accounts,
        signer_seeds, // Use vault authority signer seeds for all operations
    );
    
    // Read token vault balances directly from the token accounts
    let sol_vault_balance = {
        let account_data = sol_vault.try_borrow_data()?;
        let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let dbtc_vault_balance = {
        let account_data = dbtc_vault.try_borrow_data()?;
        let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    // Read LP supply from pool state (this is what Raydium uses internally)
    let lp_supply = {
        let pool_data = pool_state.try_borrow_data()?;
        // Skip discriminator (8 bytes) and read the lp_supply field directly
        // Based on PoolState struct: lp_supply is at offset after all the Pubkeys and small fields
        // From pool.rs: 10 Pubkeys (320 bytes) + 5 u8s (5 bytes) = 325 bytes from start + discriminator (8) = 333
        let lp_supply_offset = 8 + 10 * 32 + 5; // discriminator + 10 pubkeys + 5 u8 fields
        if pool_data.len() >= lp_supply_offset + 8 {
            u64::from_le_bytes([
                pool_data[lp_supply_offset],
                pool_data[lp_supply_offset + 1],
                pool_data[lp_supply_offset + 2],
                pool_data[lp_supply_offset + 3],
                pool_data[lp_supply_offset + 4],
                pool_data[lp_supply_offset + 5],
                pool_data[lp_supply_offset + 6],
                pool_data[lp_supply_offset + 7],
            ])
        } else {
            0 // Fallback if we can't read the data
        }
    };
    
    msg!("📊 Pool balances - SOL vault: {}, DOGE_BTC vault: {}, LP supply: {}", 
         sol_vault_balance, dbtc_vault_balance, lp_supply);
    
    // Reserve buffer upfront to account for transfer fees and rounding
    // This ensures our calculations are based on what we can actually use
    let sol_buffer = sol_amount / 50; // 2% buffer for transfer fees and rounding
    let available_sol = sol_amount.saturating_sub(sol_buffer);
    
    msg!("🛡️ Reserved {} SOL as buffer, available for LP: {} SOL", sol_buffer, available_sol);
    
        // Calculate LP tokens and adjusted amounts to maximize token usage
    let (estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount) = if admin_lp_override > 0 {
        // Admin override: Calculate required token amounts for the specified LP amount
        let required_sol = if lp_supply > 0 && sol_vault_balance > 0 {
            (admin_lp_override as u128 * sol_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            available_sol // Fallback to available amount (after buffer)
        };
        
        let required_mdoge = if lp_supply > 0 && dbtc_vault_balance > 0 {
            (admin_lp_override as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64
        } else {
            0 // No DOGE_BTC needed if pool is empty
        };
        
        // Use available SOL (already has buffer applied)
        let final_sol = required_sol.min(available_sol);
        let final_mdoge = required_mdoge + 100;
        
        msg!("🔧 Admin override LP calculation: {} LP tokens (needs {} SOL, {} DOGE_BTC)", 
             admin_lp_override, final_sol, final_mdoge);
             
        (admin_lp_override, final_sol, final_mdoge)
    } else {
        // Normal automatic calculation using available SOL (after buffer)
        let lp_from_sol = (available_sol as u128 * lp_supply as u128 / sol_vault_balance as u128) as u64;
        let required_mdoge = (lp_from_sol as u128 * dbtc_vault_balance as u128 / lp_supply as u128) as u64;
        msg!("💰 SOL-limited LP calculation: {} LP tokens (needs {} SOL, {} DOGE_BTC)",  
             lp_from_sol, available_sol, required_mdoge);
        (lp_from_sol, available_sol, required_mdoge + 100) // Add small buffer for ceiling rounding
    };
    
    msg!("🎯 Final LP token amount: {} for deposits of {} SOL, {} DOGE_BTC", 
         estimated_lp_amount, adjusted_sol_amount, adjusted_dbtc_amount);
    
    // Add small additional buffer to DOGE_BTC for transfer fees (1% burn tax)
    // SOL already has buffer applied upfront, but DOGE_BTC needs extra for burn tax
    let max_dbtc_with_buffer = adjusted_dbtc_amount.saturating_add(adjusted_dbtc_amount / 50); // +2% buffer for burn tax
    
    msg!("🛡️ Maximum amounts: {} SOL (buffered upfront), {} DOGE_BTC (with burn tax buffer)", 
         adjusted_sol_amount, max_dbtc_with_buffer);
    
    // Perform the actual deposit with calculated LP amount and proper maximums
    // Parameters: (lp_token_amount, maximum_token_0_amount, maximum_token_1_amount)
    // token0 = WSOL, token1 = DOGE_BTC
    // SOL amount is already buffered, DOGE_BTC has burn tax buffer
    raydium_cp_swap::cpi::deposit(cpi_ctx, estimated_lp_amount, adjusted_sol_amount, max_dbtc_with_buffer)?;
    
    // Calculate actual LP tokens minted by checking balance difference
    let lp_balance_after = {
        use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
        let lp_account_data = lp_token_account.try_borrow_data()?;
        let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
        lp_account.amount
    };
    
    let lp_tokens_minted = lp_balance_after.saturating_sub(lp_balance_before);
    
    msg!("💰 LP token balance after deposit: {}", lp_balance_after);
    msg!("✅ LP tokens minted: {}", lp_tokens_minted);
    
    // Step 3: Burn the LP tokens immediately using SPL token burn
    if lp_tokens_minted > 0 {
        msg!("🔥 Burning {} LP tokens", lp_tokens_minted);
        
        use anchor_spl::token;
        
        // Use vault authority to burn LP tokens (same authority that owns the LP token account)
        let burn_ctx = CpiContext::new_with_signer(
            token_program.to_account_info(),
            token::Burn {
                mint: lp_mint.to_account_info(),
                from: lp_token_account.to_account_info(),
                authority: authority_pda.to_account_info(),
            },
            signer_seeds, // Use vault authority signer seeds (same as deposit)
        );
        
        token::burn(burn_ctx, lp_tokens_minted)?;
        
        // Get actual amounts consumed by checking token account balances before/after
        let sol_consumed = {
            let sol_account_data = sol_token_account.try_borrow_data()?;
            let sol_token_data = anchor_spl::token::TokenAccount::try_deserialize(&mut &sol_account_data[..])?;
            // Calculate how much SOL was actually consumed (we started with sol_amount, now have this much left)
            sol_amount.saturating_sub(sol_token_data.amount)
        };
            
        // Update POL stats with actual consumed amounts
        doge_btc_mining.pol_stats.update_after_lp_operation(
            lp_tokens_minted,
            sol_consumed,
            adjusted_dbtc_amount
        );
        
        msg!("📊 POL Stats Updated:");
        msg!("   Total LP Burnt: {}", doge_btc_mining.pol_stats.total_lp_burnt);
        msg!("   Total SOL Added: {}", doge_btc_mining.pol_stats.total_sol_added);
        msg!("   Total DOGE_BTC Added: {}", doge_btc_mining.pol_stats.total_dbtc_added);
        msg!("   LP Operations: {}", doge_btc_mining.pol_stats.lp_operations_count);
        
        // Emit LP burn tracking event
        emit!(LpTokensBurned {
            lp_tokens_burned: lp_tokens_minted,
            total_lp_burnt: doge_btc_mining.pol_stats.total_lp_burnt,
            dbtc_amount_added: adjusted_dbtc_amount,
            sol_amount_added: sol_consumed,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        // Verify burn was successful by checking final balance
        let lp_balance_final = {
            use anchor_spl::token_interface::TokenAccount as TokenAccountInterface;
            let lp_account_data = lp_token_account.try_borrow_data()?;
            let lp_account = TokenAccountInterface::try_deserialize(&mut &lp_account_data[..])?;
            lp_account.amount
        };
        
        msg!("🔥 LP tokens burned: {} (Total burnt: {})", lp_tokens_minted, doge_btc_mining.pol_stats.total_lp_burnt);
        msg!("💰 Final LP token balance: {} (should equal initial: {})", lp_balance_final, lp_balance_before);
        
        // Ensure all LP tokens were properly burned
        require_eq!(lp_balance_final, lp_balance_before, ErrorCode::IncompleteTokenBurn);
    } else {
        msg!("⚠️ No LP tokens were minted, skipping burn");
    }
    
    // Calculate and store LP token price in SOL terms
    // LP price = (SOL_in_pool + DBTC_in_pool * DBTC_price) / LP_supply
    let sol_vault_balance_final = {
        let account_data = sol_vault.try_borrow_data()?;
        let token_account = anchor_spl::token::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let dbtc_vault_balance_final = {
        let account_data = dbtc_vault.try_borrow_data()?;
        let token_account = anchor_spl::token_interface::TokenAccount::try_deserialize(&mut &account_data[..])?;
        token_account.amount
    };
    
    let lp_supply_final = {
        let pool_data = pool_state.try_borrow_data()?;
        let lp_supply_offset = 8 + 10 * 32 + 5;
        if pool_data.len() >= lp_supply_offset + 8 {
            u64::from_le_bytes([
                pool_data[lp_supply_offset],
                pool_data[lp_supply_offset + 1],
                pool_data[lp_supply_offset + 2],
                pool_data[lp_supply_offset + 3],
                pool_data[lp_supply_offset + 4],
                pool_data[lp_supply_offset + 5],
                pool_data[lp_supply_offset + 6],
                pool_data[lp_supply_offset + 7],
            ])
        } else {
            0
        }
    };
    
    // Calculate LP token price if we have valid data
    if lp_supply_final > 0 {
        // Get dBTC price from the recent_price (already in 9-decimal precision)
        let dbtc_price = doge_btc_mining.recent_price;
        
        // Calculate total value in pool in SOL terms (9-decimal precision)
        // SOL value = sol_vault_balance (9 decimals)
        // DBTC value in SOL = dbtc_vault_balance (6 decimals) * dbtc_price (9 decimals) / 10^6
        let sol_value = sol_vault_balance_final; // Already in 9-decimal precision (lamports)
        
        let dbtc_value_in_sol = if dbtc_price > 0 {
            // (dbtc_vault * dbtc_price) / 10^6
            // This gives us SOL value with 9-decimal precision
            (dbtc_vault_balance_final as u128)
                .checked_mul(dbtc_price as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(1_000_000) // DBTC has 6 decimals
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .min(u64::MAX as u128) as u64
        } else {
            0
        };
        
        let total_pool_value_sol = sol_value
            .checked_add(dbtc_value_in_sol)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        // LP token price = total_pool_value / lp_supply
        // Result in 9-decimal precision (SOL per LP token)
        doge_btc_mining.lp_token_price_in_sol = (total_pool_value_sol as u128)
            .checked_mul(1_000_000_000) // Scale to 9 decimals
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(lp_supply_final as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .min(u64::MAX as u128) as u64;
        
        let lp_price_actual = doge_btc_mining.lp_token_price_in_sol as f64 / 1_000_000_000.0;
        msg!("💎 LP token price updated: {} (9-decimal precision), Actual: {:.9} SOL per LP token", 
             doge_btc_mining.lp_token_price_in_sol, lp_price_actual);
        msg!("   Pool composition: {} SOL + {} DBTC (worth {} SOL) = {} SOL total value",
             sol_vault_balance_final as f64 / 1_000_000_000.0,
             dbtc_vault_balance_final as f64 / 1_000_000.0,
             dbtc_value_in_sol as f64 / 1_000_000_000.0,
             total_pool_value_sol as f64 / 1_000_000_000.0);
    } else {
        msg!("⚠️ LP supply is 0, cannot calculate LP token price");
    }
    
    msg!("✅ LP addition and burn completed successfully");
    
    Ok(())
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

    /// CHECK: 0-byte PDA that only stores lamports
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
        owner   = crate::ID 
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
        seeds = [MODULE_CONFIG_STORE_SEED.as_ref()],
        bump = module_config_store.bump,
    )]
    pub module_config_store: Option<Account<'info, ModuleConfigStore>>,
    
    
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


/// Account struct for adding a new expansion
#[derive(Accounts)]
pub struct AddExpansion<'info> {
    #[account(
        mut,
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


/// Account struct for initializing loot rewards system
#[derive(Accounts)]
pub struct InitializeLootRewards<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        init,
        payer = authority,
        space = LootRewards::LEN,
        seeds = [LOOT_REWARDS_SEED.as_ref()],
        bump
    )]
    pub loot_rewards: Account<'info, LootRewards>,
    
    /// CHECK: SOL vault for loot rewards (0-byte PDA)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [LOOT_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = crate::ID
    )]
    pub loot_sol_vault: UncheckedAccount<'info>,
    
    /// DOGE_BTC vault for loot rewards
    #[account(
        init,
        payer = authority,
        owner = token_program.key(),
        seeds = [LOOT_DOGE_BTC_VAULT_SEED.as_ref()],
        token::mint = dbtc_mint,
        token::authority = loot_dbtc_vault_authority,
        bump
    )]
    pub loot_dbtc_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: Authority for loot DOGE_BTC vault (0-byte PDA)
    #[account(
        seeds = [LOOT_DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub loot_dbtc_vault_authority: UncheckedAccount<'info>,
    
    /// DOGE_BTC mint (Token-2022)
    #[account(owner = token_program.key())]
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub rent: Sysvar<'info, Rent>,
}



/// Account struct for initializing level statistics
#[derive(Accounts)]
pub struct InitializeLevelStats<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        init,
        payer = authority,
        space = LevelStats::LEN,
        seeds = [LEVEL_STATS_SEED.as_ref()],
        bump
    )]
    pub level_stats: Account<'info, LevelStats>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct InitializeConfigStore<'info> {
    #[account(
        init,
        payer = authority,
        space = ModuleConfigStore::LEN,
        seeds = [MODULE_CONFIG_STORE_SEED.as_ref()],
        bump
    )]
    pub module_config_store: Account<'info, ModuleConfigStore>,
        
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
 



#[derive(Accounts)]
#[instruction(
    name: String,
    image_url: String,
    module_type: u8,
    faction_ids: Vec<u8>,
    min_level: u8,
    width: u8,
    height: u8,
    mint_cost: u64,
    upgrade_cost: u64,
    upgrade_level_requirements: Vec<u8>,
)]
pub struct AddModuleToConfigStore<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [MODULE_CONFIG_STORE_SEED.as_ref()],
        bump = module_config_store.bump,
    )]
    pub module_config_store: Account<'info, ModuleConfigStore>,
    
    #[account(
        init,
        payer = authority,
        space = ModuleConfigAccount::LEN,
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_config_store.next_id.to_le_bytes().as_ref()],
        bump
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
            
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    id: u16,
    max_hp: u32,
    power_consumption: u16,
    base_hashpower: u32,
    base_xp_per_hour: u32,
)]
pub struct UpdateModuleStats<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [MODULE_CONFIG_SEED.as_ref(), id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
#[instruction(id: u16)]
pub struct UpdateModuleConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [MODULE_CONFIG_SEED.as_ref(), id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
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

    /// CHECK: safe PDA used as vault authority
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,  // Let Anchor find the correct bump
        owner = crate::ID  // Use program_id for clarity
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    #[account(mut, signer, address = global_config.ext_fee_collector)]
    pub fee_collector: Signer<'info>,

    /// CHECK: Loot SOL vault PDA (required)
    #[account(
        mut,
        seeds = [LOOT_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = crate::ID
    )]
    pub loot_sol_vault: UncheckedAccount<'info>,

    /// Loot rewards tracking account (required)
    #[account(
        mut,
        seeds = [LOOT_REWARDS_SEED.as_ref()],
        bump,
    )]
    pub loot_rewards: Account<'info, LootRewards>,

    pub system_program: Program<'info, System>,
}
 


// ----------------------------------------------------------------------------------------
// ------------ DYNAMIC DISTRIBUTION ACCOUNT STRUCTS ------------------------------------
// ----------------------------------------------------------------------------------------

 

#[derive(Accounts)]
pub struct UpdateMdogeDistPerSlot<'info> {
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// GlobalConfig for admin authority verification
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// Authority (optional - only required when lp_token_amount > 0)
    pub authority: Option<Signer<'info>>,
    
    /// CHECK: Raydium CP-Swap program
    pub raydium_program: UncheckedAccount<'info>,
    
    /// CHECK: Raydium pool state
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,
    
    /// CHECK: Raydium AMM config
    pub amm_config: UncheckedAccount<'info>,
    
    /// CHECK: Vault authority PDA (our program's authority for token accounts)
    #[account(
        seeds = [DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub authority_pda: UncheckedAccount<'info>,
    
    /// CHECK: Raydium's pool authority PDA (from Raydium program)
    pub raydium_authority: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC vault in Raydium pool
    #[account(mut)]
    pub dbtc_vault: UncheckedAccount<'info>,
    
    /// CHECK: SOL vault in Raydium pool
    #[account(mut)]
    pub sol_vault: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC token account for swapping
    #[account(mut)]
    pub dbtc_token_account: UncheckedAccount<'info>,
    
    /// CHECK: SOL token account for receiving
    #[account(mut)]
    pub sol_token_account: UncheckedAccount<'info>,
    
    /// CHECK: DOGE_BTC mint
    pub dbtc_mint: UncheckedAccount<'info>,
    
    /// CHECK: SOL mint (WSOL)
    pub sol_mint: UncheckedAccount<'info>,
    
    /// CHECK: Raydium observation state
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,
    
    /// CHECK: SOL treasury to receive swapped SOL
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: LP token account for receiving and burning LP tokens (can be any valid token account)
    #[account(mut)]
    pub lp_token_account: UncheckedAccount<'info>,
        
    /// CHECK: LP mint from Raydium pool (must be writable for minting)
    #[account(mut)]
    pub lp_mint: UncheckedAccount<'info>,
    
    /// Token-2022 program for DOGE_BTC
    pub token_program_2022: Program<'info, Token2022>,
    
    /// Standard token program for SOL
    pub token_program: Program<'info, anchor_spl::token::Token>,
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

    /// CHECK: SOL treasury PDA
    #[account(
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
        owner = crate::ID
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
    msg!("Collection Authority PDA: {}", ctx.accounts.collection_authority.key());
    
    // Get the collection authority bump for signing
    let collection_authority_bump = ctx.bumps.collection_authority;
    let _collection_authority_seeds = &[
        COLLECTION_AUTHORITY_SEED,
        &[collection_authority_bump],
    ];
    
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
    #[account(
        mut,
        signer
    )]
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



