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

