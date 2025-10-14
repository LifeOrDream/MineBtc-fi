use anchor_lang::prelude::*;
use anchor_lang::system_program;
mod state;
mod errors;
mod events;
pub mod instructions;

pub use instructions::admin::*;
// pub use instructions::user::*;
// pub use instructions::game::*;

declare_id!("76bGWqGdzwR13FSd1TDwanK7GFDHcunKh6WGbzAW1PjU");

#[program]
pub mod moonbase {
    use super::*;
    use crate::instructions::admin::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
        admin::internal_initialize(ctx, base_creation_cost, creation_fee_recipient)         
    }
    
    /// Update the global configuration parameters
    /// Can only be called by the current authority
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_collector: Option<Pubkey>,
        new_creation_fee_recipient: Option<Pubkey>,
        new_base_creation_cost: Option<u64>,
        new_loot_percentage: Option<u8>,
    ) -> Result<()> {
        admin::update_config_internal(
            ctx,
            new_authority,
            new_fee_collector,
            new_creation_fee_recipient,
            new_base_creation_cost,
            new_loot_percentage,
        )
    }

    /// Add a new expansion configuration (admin only)
    pub fn add_expansion(
        ctx: Context<AddExpansion>,
        id: u8,
        name: String,
        required_level: u8,
        cost_sol: u64,
        new_width: u8,
        new_height: u8,
    ) -> Result<()> {
        admin::add_expansion_internal(ctx, id, name, required_level, cost_sol, new_width, new_height)
    }


    // ----------------------------------------------------------------------------------------
    // ------------ MOON_DOGE_MINING (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------


    /// Initialize mining by setting the token vault and starting timestamp
    /// Can only be called once when mining_start_timestamp is 0
    pub fn initialize_mining(
        ctx: Context<InitializeMining>,
        start_timestamp: u64,
        moon_doge_per_slot: u64,
        pool_state: Pubkey
    ) -> Result<()> {
        admin::initialize_mining_internal(ctx, start_timestamp, moon_doge_per_slot, pool_state)
    }
 
    /// Update slots per hour configuration (admin only)
    pub fn update_slots_for_swap(ctx: Context<UpdateSlotsPerHour>, new_slots_for_swap: u64) -> Result<()> {
        admin::update_slots_for_swap_internal(ctx, new_slots_for_swap)
    }    

    /// Deposit moon doge tokens to the mining vault
    pub fn deposit_moon_doge_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        admin::deposit_moon_doge_tokens_internal(ctx, amount)
    }


    // ----------------------------------------------------------------------------------------
    // ------------ SYSTEM_REFERRAL_ACCOUNT (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------

   /// Create a new moon base for a user
   pub fn create_system_referral_account(ctx: Context<CreateSystemReferralAccount>) -> Result<()> {
        let rewards_acct = &mut ctx.accounts.referrer_rewards;
        // 1) Set the owner field to the system program key
        rewards_acct.owner = ctx.accounts.system_program.key();
        rewards_acct.total_sol_earned = 0;
        rewards_acct.sol_claimed_for_xp = 0;
        // 2) Set the bump field to the bump of the account
        rewards_acct.bump = ctx.bumps.referrer_rewards;
        rewards_acct.referrals_count = 0;

        Ok(())
    }    


    // ---------------------------------------------------------- 
    // ------------ LOOT REWARDS (ADMIN) --------------------------------
    // ---------------------------------------------------------- 


    /// Initialize the loot rewards system
    pub fn initialize_loot_rewards(ctx: Context<InitializeLootRewards>) -> Result<()> {
        admin::initialize_loot_rewards_internal(ctx)
    }

    /// Initialize level statistics tracking (admin only)
    pub fn initialize_level_stats(ctx: Context<InitializeLevelStats>) -> Result<()> {
        admin::initialize_level_stats_internal(ctx)
    }
    

    // ----------------------------------------------------------------------------------------
    // ------------ MOONBASE EXPANSION FUNCTIONS :: MANAGE EXPANSIONS & PURCHASE -----------
    // ----------------------------------------------------------------------------------------

    /// Initialize config stores for modules and gears
    /// Can only be called by the authority
    pub fn initialize_config_stores(ctx: Context<InitializeConfigStore>) -> Result<()> {
        admin::initialize_config_stores_internal(ctx)
    }
    
    /// Initialize a new module configuration (basic info only)
    pub fn add_module_to_base(
        ctx: Context<AddModuleToConfigStore>,
        name: String,
        image_url: String,
        module_type: state::ModuleType,
        faction_ids: Vec<u8>,
        min_level: u8,
        max_per_base: u8,
        width: u8,
        height: u8,
        mint_cost: u64,
        upgrade_cost: u64,
        upgrade_level_requirements: Vec<u8>,
    ) -> Result<()> {
        admin::add_module_to_base_internal(
            ctx, name, image_url, module_type, 
            faction_ids, min_level, max_per_base, width, height,
            mint_cost, upgrade_cost, upgrade_level_requirements
        )
    }

    /// Update module stats (required before module can be used)
    pub fn update_module_stats(
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
        admin::update_module_stats_internal(
            ctx, id, max_hp, power_consumption, base_hashpower, base_xp_per_hour, base_damage,
            base_missiles_per_load, reload_time_seconds, cooldown_sec,
            max_reward, probability
        )
    }



}

 