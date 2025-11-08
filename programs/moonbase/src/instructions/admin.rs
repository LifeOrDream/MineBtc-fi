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
    pub loot_percentage: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GlobalConfigInfo {
    pub loot_percentage: u8,
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

    // Initialize Raydium pool state to default (must be set via admin function)
    global_config.raydium_pool_state = Pubkey::default();

    // Initialize global dragon egg power tracker
    global_config.global_dragon_egg_power = 0;

    global_config.bump = ctx.bumps.global_config;
    global_config.loot_percentage = 15; // Default 15% for loot rewards
    global_config.buyback_percentage = 20; // Default 20% for buybacks
    global_config.is_game_active = false; // Default to false

    // Initialize empty factions list
    global_config.supported_factions = Vec::new();

    // Optionally drop 1 lamport into the vault for future-proof rent-exempt status
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

    msg!(
        "SOL Treasury PDA created at: {} with bump: {}",
        ctx.accounts.sol_treasury.key(),
        ctx.bumps.sol_treasury
    );

    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG URI MANAGEMENT (ADMIN) ---------------------------------------
// ----------------------------------------------------------------------------------------

/// Add Dragon Egg URIs to the pool (admin only)
pub fn add_dragon_egg_uris_internal(ctx: Context<UpdateConfigAc>, uris: Vec<String>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= MAX_URI_LENGTH, ErrorCode::UriTooLong);
    }

    // Add new URIs
    global_config.dragon_egg_uris.extend(uris.clone());

    msg!("✅ Added {} Dragon Egg URIs", uris.len());
    msg!(
        "   Total Dragon Egg URIs: {}",
        global_config.dragon_egg_uris.len()
    );

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

/// Add factions to the global config (admin only)
pub fn add_factions_internal(ctx: Context<UpdateConfigAc>, factions: Vec<String>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    // Validate faction names
    for faction in &factions {
        require!(
            faction.len() > 0 && faction.len() <= 16,
            ErrorCode::InvalidFactionName
        );
    }

    // Check we don't exceed max factions
    require!(
        global_config.supported_factions.len() + factions.len() <= MAX_FACTIONS,
        ErrorCode::MaxFactionsReached
    );

    // Add new factions
    global_config.supported_factions.extend(factions.clone());

    msg!("✅ Added {} factions", factions.len());
    msg!(
        "   Total factions: {}",
        global_config.supported_factions.len()
    );

    // Emit event for off-chain indexing
    emit!(FactionsAdded {
        authority: ctx.accounts.authority.key(),
        factions: factions.clone(),
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
        msg!(
            "Updated creation fee recipient to {}",
            creation_fee_recipient
        );
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
    require!(
        global_config.expansions.len() < MAX_EXPANSIONS,
        ErrorCode::MaxExpansionsReached
    );

    // Check if expansion ID already exists
    for existing_expansion in &global_config.expansions {
        require!(
            existing_expansion.id != id,
            ErrorCode::ExpansionAlreadyExists
        );
    }

    // Validate dimensions
    require!(
        new_width >= DEFAULT_MOONBASE_WIDTH && new_height >= DEFAULT_MOONBASE_HEIGHT,
        ErrorCode::InvalidExpansionConfiguration
    );
    require!(
        new_width <= GRID_WIDTH && new_height <= GRID_HEIGHT,
        ErrorCode::InvalidExpansionConfiguration
    );

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

    msg!(
        "Added new expansion '{}' (ID: {}) requiring level {} for {} SOL",
        name,
        id,
        required_level,
        cost_sol
    );

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
    msg!(
        "   DOGE_BTC Vault Authority: {}",
        ctx.accounts.loot_dbtc_vault_authority.key()
    );

    Ok(())
}

/// Initialize buybacks account system
pub fn initialize_buybacks_internal(ctx: Context<InitializeBuybacks>) -> Result<()> {
    let buybacks_account = &mut ctx.accounts.buybacks_account;

    // Initialize buybacks state
    buybacks_account.total_sol_accumulated = 0;
    buybacks_account.total_sol_used = 0;
    buybacks_account.sol_for_pol = 0;
    buybacks_account.bump = ctx.bumps.buybacks_account;
    buybacks_account.sol_vault_bump = ctx.bumps.buybacks_sol_vault;

    msg!("💰 Buybacks system initialized");
    msg!("   Buybacks Account PDA: {}", buybacks_account.key());
    msg!(
        "   SOL Vault PDA: {}",
        ctx.accounts.buybacks_sol_vault.key()
    );

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
    msg!(
        "Module type: {:?} (from u8: {})",
        module_type_enum,
        module_type
    );

    // Build ModuleStats with placeholder values (will be updated later)
    let stats: ModuleStats = match module_type_enum {
        ModuleType::Mining => {
            ModuleStats::Mining(MiningStats {
                max_hp: 0,
                base_hashpower: 0, // Placeholder - must be updated
                power_consumption: 0,
            })
        }
        ModuleType::Attraction => {
            ModuleStats::Attraction(AttractionStats {
                max_hp: 0,
                base_xp_per_hour: 0, // Placeholder - must be updated
                power_consumption: 0,
            })
        }
    };

    msg!("Stats (placeholders): {:?}", stats);
    msg!("Faction IDs: {:?}", faction_ids);
    msg!("Min level: {}", min_level);
    msg!("Width: {}", width);

    // Validate inputs
    require!(name.len() <= 32, ErrorCode::InvalidModuleName);
    require!(image_url.len() <= 64, ErrorCode::InvalidImageUrl);
    require!(
        upgrade_level_requirements.len() <= MAX_MODULE_UPGRADES as usize,
        ErrorCode::InvalidUpgradeConfiguration
    );
    require!(
        faction_ids.len() <= MAX_FACTION_IDS_PER_MODULE,
        ErrorCode::TooManyFactionIds
    );

    // Validate that upgrade level requirements are increasing and start at or above min_level
    let mut prev_level = min_level;
    for (i, &required_level) in upgrade_level_requirements.iter().enumerate() {
        require!(
            required_level >= prev_level,
            ErrorCode::InvalidUpgradeConfiguration
        );
        prev_level = required_level;

        msg!(
            "Upgrade {} requires moonbase level {}",
            i + 1,
            required_level
        );
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
    module_config_store.next_id = module_config_store
        .next_id
        .checked_add(1)
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
    msg!(
        "Initialized new module config: {}, ID: {}, Image: {}",
        name,
        id,
        image_url
    );
    msg!(
        "Max upgrades: {}, upgrade levels: {:?}",
        upgrade_level_requirements.len(),
        &upgrade_level_requirements
    );
    msg!("⚠️ Module is INACTIVE until stats are properly configured");

    // Emit event
    emit!(NewModuleConfigCreated { id, name });

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
        }
        ModuleType::Attraction => {
            require!(base_xp_per_hour > 0, ErrorCode::InvalidModuleConfiguration);
            msg!("Attraction stats - base_xp_per_hour: {}", base_xp_per_hour);
            ModuleStats::Attraction(AttractionStats {
                max_hp,
                base_xp_per_hour,
                power_consumption,
            })
        }
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
        msg!(
            "Updated upgrade level requirements to: {:?} (max upgrades: {})",
            new_upgrade_requirements,
            new_upgrade_requirements.len()
        );
    }

    if let Some(new_faction_ids) = faction_ids {
        require!(
            new_faction_ids.len() <= MAX_FACTION_IDS_PER_MODULE,
            ErrorCode::TooManyFactionIds
        );
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

    // Calculate loot rewards amount using configurable percentage
    let loot_percentage = global_config.loot_percentage as u64;
    let sol_for_loots = available_solana
        .checked_mul(loot_percentage)
        .unwrap()
        .checked_div(100)
        .unwrap();

    // Calculate buybacks amount using configurable percentage
    let buyback_percentage = global_config.buyback_percentage as u64;
    let sol_for_buybacks = available_solana
        .checked_mul(buyback_percentage)
        .unwrap()
        .checked_div(100)
        .unwrap();

    // Remaining amount goes to fee collector (for distribution to stakers and dev)
    let fee_collector_amount = available_solana
        .checked_sub(sol_for_loots)
        .unwrap()
        .checked_sub(sol_for_buybacks)
        .unwrap();

    // Create signer seeds for sol_treasury
    let treasury_seeds = &[SOL_TREASURY_SEED.as_ref(), &[ctx.bumps.sol_treasury]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Transfer loot rewards to loot SOL vault
    if sol_for_loots > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.sol_treasury.to_account_info(),
                    to: ctx.accounts.loot_sol_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_loots,
        )?;

        // Update loot rewards tracking
        ctx.accounts.loot_rewards.total_sol_accumulated = ctx
            .accounts
            .loot_rewards
            .total_sol_accumulated
            .checked_add(sol_for_loots)
            .unwrap();

        emit!(LootRewardsAccumulated {
            dbtc_amount: 0,
            sol_amount: sol_for_loots,
            total_dbtc_accumulated: ctx.accounts.loot_rewards.total_dbtc_accumulated,
            total_sol_accumulated: ctx.accounts.loot_rewards.total_sol_accumulated,
        });

        msg!(
            "🎁 Transferred {} SOL to loot rewards vault ({}%)",
            sol_for_loots  as f64 / 1e9,
            loot_percentage
        );
    }

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
            loot_amount: sol_for_loots,
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
        loot_percentage: global_config.loot_percentage,
    })
}

/// Query global config information for external programs
pub fn query_global_config_internal(ctx: Context<QueryGlobalConfig>) -> Result<GlobalConfigInfo> {
    msg!("🔍 Querying global config");
    let global_config = &ctx.accounts.global_config;

    msg!(
        "📊 Config: loot={}%, active={}, authority={}",
        global_config.loot_percentage,
        global_config.is_game_active,
        global_config.ext_authority
    );

    Ok(GlobalConfigInfo {
        loot_percentage: global_config.loot_percentage,
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

    /// CHECK: SOL vault for loot rewards (0-byte PDA, System Account)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [LOOT_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
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

    /// CHECK: SOL treasury PDA (System Account)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump  // Let Anchor find the correct bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    #[account(mut, signer, address = global_config.ext_fee_collector)]
    pub fee_collector: Signer<'info>,

    /// CHECK: Loot SOL vault PDA (System Account)
    #[account(
        mut,
        seeds = [LOOT_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub loot_sol_vault: UncheckedAccount<'info>,

    /// Loot rewards tracking account (required)
    #[account(
        mut,
        seeds = [LOOT_REWARDS_SEED.as_ref()],
        bump,
    )]
    pub loot_rewards: Account<'info, LootRewards>,

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
