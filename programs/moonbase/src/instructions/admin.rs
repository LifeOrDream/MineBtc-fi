use anchor_lang::prelude::*;
use crate::state::*;
use crate::events::*;
use anchor_lang::system_program::{self, Transfer}; // <- the CPI struct

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

// -------------------------------------------------------------------------------- 
// ------------ GLOBAL_CONFIG :: UPDATES, ADDING EXPANSIONS ------------
// -------------------------------------------------------------------------------- 


pub fn internal_initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let moon_doge_mining = &mut ctx.accounts.moon_doge_mining;

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

    // Initialize MoonDogeMining
    moon_doge_mining.mdoge_token_vault = Pubkey::default(); // Will be set during initialize_mining
    moon_doge_mining.mining_start_timestamp = 0; // Set to 0 to indicate mining not started
    moon_doge_mining.total_active_hashpower = 0;
    moon_doge_mining.total_active_electricity = 0;
    moon_doge_mining.moon_doge_per_slot = 0;
    moon_doge_mining.last_slot = 0;
    moon_doge_mining.total_tokens_mined = 0;
    moon_doge_mining.bump = ctx.bumps.moon_doge_mining;
    moon_doge_mining.vault_auth_bump = 0; // Will be set during initialize_mining
    
    // Initialize dynamic distribution fields with defaults
    moon_doge_mining.raydium_pool_state = Pubkey::default();
    moon_doge_mining.last_rate_update = 0;
    moon_doge_mining.current_dist_rate = 0;
    moon_doge_mining.price_history = Vec::new();
    moon_doge_mining.avg_price_8h = 0;
    moon_doge_mining.prev_avg_price_8h = 0;
    moon_doge_mining.sol_for_pol = 0;
    moon_doge_mining.slots_for_swap = 9000;
    
    msg!("Program initialized with creation cost: {}", base_creation_cost);
    msg!("SOL Treasury PDA created at: {} with bump: {}", ctx.accounts.sol_treasury.key(), ctx.bumps.sol_treasury);
    
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
// ------------ MOON_DOGE_MINING :: INITIALIZATION & UPDATES ------------
// -------------------------------------------------------------------------------- 
// -------------------------------------------------------------------------------- 



/// Initialize mining by setting the token vault and starting timestamp
/// Can only be called once when mining_start_timestamp is 0
pub fn initialize_mining_internal(  ctx: Context<InitializeMining>, start_timestamp: u64, 
    moon_doge_per_slot: u64, pool_state: Pubkey) -> Result<()> {
let moon_doge_mining = &mut ctx.accounts.moon_doge_mining;

// Check mining hasn't been initialized yet
require!(
moon_doge_mining.mining_start_timestamp == 0,
ErrorCode::MiningAlreadyInitialized
);

let cur_slot = Clock::get()?.slot;

// ───── persist vault + bump(s) ─────
moon_doge_mining.mdoge_token_vault = ctx.accounts.token_vault.key();
moon_doge_mining.vault_auth_bump = ctx.bumps.vault_authority;

// Initialize mining parameters
moon_doge_mining.mining_start_timestamp = start_timestamp;
moon_doge_mining.moon_doge_per_slot = moon_doge_per_slot;
moon_doge_mining.last_slot = cur_slot;

// Initialize dynamic distribution fields  
moon_doge_mining.raydium_pool_state = pool_state;
moon_doge_mining.last_rate_update = Clock::get()?.unix_timestamp;
moon_doge_mining.current_dist_rate = moon_doge_per_slot;
moon_doge_mining.price_history = Vec::with_capacity(8);
moon_doge_mining.avg_price_8h = 0;
moon_doge_mining.prev_avg_price_8h = 0;
moon_doge_mining.sol_for_pol = 0; // Initialize POL tracking
moon_doge_mining.slots_for_swap = 9000; // Default: ~2.5 slots/second * 3600 seconds
moon_doge_mining.pol_stats = ProtocolOwnedLiquidity::default(); // Initialize POL stats tracking

msg!("Initialized dynamic distribution system with Raydium pool: {}", pool_state);

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
let moon_doge_mining = &mut ctx.accounts.moon_doge_mining;

require!(new_slots_for_swap > 0, ErrorCode::InvalidParameters);

let old_slots_for_swap = moon_doge_mining.slots_for_swap;
moon_doge_mining.slots_for_swap = new_slots_for_swap;

msg!("Updated slots per hour from {} to {}", old_slots_for_swap, new_slots_for_swap);

emit!(SlotsPerHourUpdated {
authority: ctx.accounts.authority.key(),
old_slots_for_swap,
new_slots_for_swap,
});

Ok(())
}

/// Deposit moon doge tokens to the mining vault
pub fn deposit_moon_doge_tokens_internal(  ctx: Context<DepositTokens>,  amount: u64) -> Result<()> {
token_if::transfer_checked(
CpiContext::new(
ctx.accounts.token_program.to_account_info(),      // TOKEN_2022_PROGRAM_ID
token_if::TransferChecked {
from:      ctx.accounts.depositor_token_account.to_account_info(),
mint:      ctx.accounts.token_mint.to_account_info(),
to:        ctx.accounts.mdoge_token_vault.to_account_info(),
authority: ctx.accounts.depositor.to_account_info(),
},
),
amount,
MDOGE_DECIMALS,     // decimals
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
    loot_rewards.total_mdoge_accumulated = 0;
    loot_rewards.total_sol_accumulated = 0;
    loot_rewards.total_mdoge_distributed = 0;
    loot_rewards.total_sol_distributed = 0;
    loot_rewards.bump = ctx.bumps.loot_rewards;
    loot_rewards.sol_vault_bump = ctx.bumps.loot_sol_vault;
    loot_rewards.mdoge_vault_bump = ctx.bumps.loot_mdoge_vault;
    loot_rewards.mdoge_vault_authority_bump = ctx.bumps.loot_mdoge_vault_authority;
    
    emit!(LootRewardsInitialized {
        loot_rewards_pda: loot_rewards.key(),
        sol_vault_pda: ctx.accounts.loot_sol_vault.key(),
        mdoge_vault_pda: ctx.accounts.loot_mdoge_vault.key(),
    });
    
    msg!("🎁 Loot rewards system initialized");
    msg!("   Loot Rewards PDA: {}", loot_rewards.key());
    msg!("   SOL Vault: {}", ctx.accounts.loot_sol_vault.key());
    msg!("   mDOGE Vault: {}", ctx.accounts.loot_mdoge_vault.key());
    msg!("   mDOGE Vault Authority: {}", ctx.accounts.loot_mdoge_vault_authority.key());
    
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
    module_type: ModuleType,
    faction_ids: Vec<u8>,
    min_level: u8,
    max_per_base: u8,
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

    msg!("Adding module: {}", name);
    msg!("Image URL: {}", image_url);
    msg!("Module type: {:?}", module_type);

    // Build ModuleStats with placeholder values (will be updated later)
    let stats: ModuleStats = match module_type {
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
    msg!("Max per base: {}", max_per_base);
    msg!("Width: {}", width);
    
    // Validate inputs
    require!(name.len() <= 32, ErrorCode::InvalidModuleName);
    require!(image_url.len() <= 64, ErrorCode::InvalidImageUrl);
    require!(upgrade_level_requirements.len() <= MAX_MODULE_UPGRADES as usize, ErrorCode::InvalidUpgradeConfiguration);
    require!(max_per_base > 0 && max_per_base <= MAX_MODULES_PER_BASE, ErrorCode::InvalidModuleConfiguration);
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
        module_type,
        stats,
        faction_ids,
        min_level,
        max_per_base,
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
    base_damage: u32,
    base_missiles_per_load: u8,
    reload_time_seconds: u32,
    cooldown_sec: u32,
    max_reward: u64,
    probability: u16,
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
 

// /// Update an existing module config
// pub fn update_module_internal(
//     ctx: Context<UpdateModuleConfig>,
//     id: u16,
//     image_url: Option<String>,
//     faction_ids: Option<Vec<u8>>,
//     max_per_base: Option<u8>,
//     mint_cost: Option<u64>,
//     upgrade_cost: Option<u64>,
//     upgrade_level_requirements: Option<Vec<u8>>,
//     is_active: Option<bool>,
// ) -> Result<()> {
//     let module_config_account = &mut ctx.accounts.module_config_account;
//     let config = &mut module_config_account.data;
    
//     // Verify this is the correct config ID
//     require!(config.id == id, ErrorCode::ConfigNotFound);
    
//     // Update fields if provided
//     if let Some(new_url) = image_url {
//         config.image_url = new_url.clone();
//         msg!("Updated module image URL to: {}", new_url);
//     }

//     if let Some(new_mint_cost) = mint_cost {
//         config.mint_cost = new_mint_cost;
//         msg!("Updated mint cost to: {}", new_mint_cost);
//     }
    
//     if let Some(new_upgrade_cost) = upgrade_cost {
//         config.upgrade_cost = new_upgrade_cost;
//         msg!("Updated upgrade cost to: {}", new_upgrade_cost);
//     }
    
//     if let Some(new_is_active) = is_active {
//         config.is_active = new_is_active;
//         msg!("Updated active status to: {}", new_is_active);
//     }
    
//     // Handle upgrade_level_requirements
//     if let Some(new_upgrade_requirements) = upgrade_level_requirements {
//         // Validate that the number of requirements doesn't exceed max
//         require!(
//             new_upgrade_requirements.len() <= MAX_MODULE_UPGRADES as usize,
//             ErrorCode::InvalidUpgradeConfiguration
//         );
        
//         // Validate that upgrade level requirements are increasing and start at or above min_level
//         let mut prev_level = config.min_level;
//         for (_i, &required_level) in new_upgrade_requirements.iter().enumerate() {
//             require!(
//                 required_level >= prev_level,
//                 ErrorCode::InvalidUpgradeConfiguration
//             );
//             prev_level = required_level;
//         }
        
//         config.upgrade_level_requirements = new_upgrade_requirements.clone();
//         msg!("Updated upgrade level requirements to: {:?} (max upgrades: {})", new_upgrade_requirements, new_upgrade_requirements.len());
//     }
    
//     if let Some(new_faction_ids) = faction_ids {
//         require!(new_faction_ids.len() <= MAX_FACTION_IDS_PER_MODULE, ErrorCode::TooManyFactionIds);
//         config.faction_ids = new_faction_ids;
//         msg!("Updated faction IDs");
//     }
    
//     if let Some(new_max_per_base) = max_per_base {
//         require!(new_max_per_base > 0 && new_max_per_base <= MAX_MODULES_PER_BASE, ErrorCode::InvalidModuleConfiguration);
//         config.max_per_base = new_max_per_base;
//         msg!("Updated max per base to: {}", new_max_per_base);
//     }
    
//     // Emit event
//     emit!(ModuleConfigUpdated {
//         id,
//         name: config.name.clone(),
//     });
    
//     Ok(())
// }


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
        space = MoonDogeMining::LEN,
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,

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
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump,
    )]
    pub moon_doge_mining: Option<Account<'info, MoonDogeMining>>,
    
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
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump,
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,

    //  Vault authority PDA (0-byte, signer only)
    #[account(
        seeds = [MDOGE_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required
    pub vault_authority: UncheckedAccount<'info>,

    // ─────────────────── token-2022 vault account ────────────────────
    #[account(
        init,
        payer  = authority,
        owner  = token_program.key(),
        seeds  = [MDOGE_VAULT_SEED, moon_doge_mining.key().as_ref()],
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
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump = moon_doge_mining.bump,
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,
    
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
        constraint  = depositor_token_account.mint  == mdoge_token_vault.mint @ ErrorCode::InvalidMint
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount2022>,

    // ─── mining token vault ───
    #[account(
        mut,
        seeds  = [MDOGE_VAULT_SEED, moon_doge_mining.key().as_ref()],
        bump,
        owner  = token_program.key(),
    )]
    pub mdoge_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump
    )]
    pub moon_doge_mining: Account<'info, MoonDogeMining>,
    
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
    
    /// mDOGE vault for loot rewards
    #[account(
        init,
        payer = authority,
        owner = token_program.key(),
        seeds = [LOOT_MDOGE_VAULT_SEED.as_ref()],
        token::mint = mdoge_mint,
        token::authority = loot_mdoge_vault_authority,
        bump
    )]
    pub loot_mdoge_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    /// CHECK: Authority for loot mDOGE vault (0-byte PDA)
    #[account(
        seeds = [LOOT_MDOGE_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub loot_mdoge_vault_authority: UncheckedAccount<'info>,
    
    /// mDOGE mint (Token-2022)
    #[account(owner = token_program.key())]
    pub mdoge_mint: InterfaceAccount<'info, Mint2022>,
    
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
    module_type: ModuleType,
    faction_ids: Vec<u8>,
    min_level: u8,
    max_per_base: u8,
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
    base_damage: u32,
    base_missiles_per_load: u8,
    reload_time_seconds: u32,
    cooldown_sec: u32,
    max_reward: u64,
    probability: u16,
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

