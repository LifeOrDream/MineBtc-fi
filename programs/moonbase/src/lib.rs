use anchor_lang::prelude::*;
mod state;
mod errors;
mod events;
mod mpl_core_helpers;
pub mod instructions;

pub use instructions::admin::*;
pub use instructions::user::*;

declare_id!("54VXNmJVUjKBAtuqssBuB8hbR9CtgQJiWE3DotEgkiuJ");

#[program]
pub mod moonbase {
    use super::*;
    use crate::instructions::admin::{self};
    use crate::instructions::user::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
        admin::internal_initialize(ctx, base_creation_cost, creation_fee_recipient)         
    }
    
    /// Set the Dragon Egg collection address (admin only)
    pub fn set_dragon_egg_collection(ctx: Context<UpdateConfigAc>, dragon_egg_collection: Pubkey) -> Result<()> {
        admin::set_dragon_egg_collection_internal(ctx, dragon_egg_collection)
    }

    /// Add Dragon Egg URIs to the pool (admin only)
    pub fn add_dragon_egg_uris(ctx: Context<UpdateConfigAc>, uris: Vec<String>) -> Result<()> {
        admin::add_dragon_egg_uris_internal(ctx, uris)
    }

    /// Clear all Dragon Egg URIs (admin only)
    pub fn clear_dragon_egg_uris(ctx: Context<UpdateConfigAc>) -> Result<()> {
        admin::clear_dragon_egg_uris_internal(ctx)
    }

    /// Add factions to the global config (admin only)
    pub fn add_factions(ctx: Context<UpdateConfigAc>, factions: Vec<String>) -> Result<()> {
        admin::add_factions_internal(ctx, factions)
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
    // ------------ doge_btc_MINING (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------


    /// Initialize mining by setting the token vault and starting timestamp
    /// Can only be called once when mining_start_timestamp is 0
    pub fn initialize_mining(
        ctx: Context<InitializeMining>,
        start_timestamp: u64,
        doge_btc_per_slot: u64,
        pool_state: Pubkey
    ) -> Result<()> {
        admin::initialize_mining_internal(ctx, start_timestamp, doge_btc_per_slot, pool_state)
    }
 
    /// Update slots per hour configuration (admin only)
    pub fn update_slots_for_swap(ctx: Context<UpdateSlotsPerHour>, new_slots_for_swap: u64) -> Result<()> {
        admin::update_slots_for_swap_internal(ctx, new_slots_for_swap)
    }    

    /// Deposit moon doge tokens to the mining vault
    pub fn deposit_doge_btc_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        admin::deposit_doge_btc_tokens_internal(ctx, amount)
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
        width: u8,
        height: u8,
        mint_cost: u64,
        upgrade_cost: u64,
        upgrade_level_requirements: Vec<u8>,
    ) -> Result<()> {
        admin::add_module_to_base_internal(
            ctx, name, image_url, module_type, 
            faction_ids, min_level, width, height,
            mint_cost, upgrade_cost, upgrade_level_requirements
        )
    }

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
        admin::update_module_internal(ctx, id, image_url, faction_ids, mint_cost, upgrade_cost, upgrade_level_requirements, is_active)
    }


    /// Update module stats (required before module can be used)
    pub fn update_module_stats(
        ctx: Context<UpdateModuleStats>,
        id: u16,
        max_hp: u32,
        power_consumption: u16,
        base_hashpower: u32,
        base_xp_per_hour: u32,
    ) -> Result<()> {
        admin::update_module_stats_internal(
            ctx, id, max_hp, power_consumption, base_hashpower, base_xp_per_hour)
    }

    // ---------------------------------------------------------- 
    // ------------ WITHDRAW SOL FEES (ANYONE) ------------------ 
    // ---------------------------------------------------------- 


    /// Withdraw collected SOL fees from the treasury
    /// 
    /// Called by MoonEconomy program, withdraws SOL and splits it into 3 parts:
    /// 1. For DOGE_BTC stakers
    /// 2. For liquidity providers
    /// 3. For devs
    /// 
    /// Internally, 10% is sent to loot rewards. 
    /// 
    pub fn withdraw_sol_fees(ctx: Context<WithdrawSolFees>) -> Result<()> {
        admin::withdraw_sol_fees_internal(ctx)
    }

    /// Query function to get treasury info for external programs
    /// Returns available balance after accounting for POL reserves and loot percentage
    pub fn query_treasury_info(ctx: Context<QueryTreasuryInfo>) -> Result<TreasuryInfo> {
        admin::query_treasury_info_internal(ctx)
    }

    /// Query function to get global config values for external programs
    pub fn query_global_config(ctx: Context<QueryGlobalConfig>) -> Result<GlobalConfigInfo> {
        admin::query_global_config_internal(ctx)
    }

    /// Query function to get token prices (dBTC and LP) for external programs
    pub fn query_token_prices(ctx: Context<QueryTokenPrices>) -> Result<TokenPricesInfo> {
        admin::query_token_prices_internal(ctx)
    }


//     // ----------------------------------------------------------------------------------------
//     // ------------ UPDATE DOGE_BTC DISTRIBUTION RATE (ANYONE) --------------------------------
//     // ----------------------------------------------------------------------------------------

    /// Update DOGE_BTC distribution rate based on price oracle (can be called by anyone every hour)
    /// 
    /// We update DOGE_BTC distribution rate every 8 hrs based on price increase / decrease. 
    /// DOGE_BTC is swapped for SOL on raydium every hr for first 7 hrs and combined with DOGE_BTC from mining vault for last hr
    /// and added to the LP pool, with LP tokens being burnt.
    /// 
    /// When lp_token_amount > 0: Admin override mode (requires authority signature)
    /// When lp_token_amount = 0: Automatic calculation mode (anyone can call)
    pub fn update_dbtc_dist_per_slot(ctx: Context<UpdateMdogeDistPerSlot>, lp_token_amount: u64) -> Result<()> {
        admin::update_dbtc_dist_per_slot_internal(ctx, lp_token_amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER FUNCTIONS :: CREATE MOON-BASE, CLAIM REFERRAL REWARDS ---------------- 
    // ----------------------------------------------------------------------------------------

    /// Create a new moon base for a user
    /// pricing_tier options:
    /// - PRICE_TIER_1 (0.5 SOL): Basic moonbase, no Dragon Egg
    /// - PRICE_TIER_2 (1.42 SOL): Moonbase + Dragon Egg + 10k electricity
    /// - PRICE_TIER_3 (2.42 SOL): Moonbase + Dragon Egg + 30k electricity
    /// - PRICE_TIER_4 (4.20 SOL): Moonbase + Dragon Egg + 75k electricity
    pub fn create_user_moonbase(ctx: Context<CreateUserMoonbase>, referrer: Option<Pubkey>, faction_id: u8, pricing_tier: u64) -> Result<()> {
        user::initialize_user_moonbase(ctx, referrer, faction_id, pricing_tier)
    }

    /// Purchase a moonbase expansion (user function)
    pub fn expand_moonbase(ctx: Context<ExpandMoonbase>, expansion_id: u8) -> Result<()> {
        user::expand_moonbase_internal(ctx, expansion_id)
    }

    /// Claim referral rewards (CPI only from mooneconomy)
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>, new_electricity: u64) -> Result<()> {
        user::claim_referral_rewards_internal(ctx, new_electricity)
    }

    pub fn update_user_electricity(  ctx: Context<UpdateUserElectricity>, to_increase: bool, amount: u64) -> Result<()> {
        user::update_user_electricity_internal(ctx, to_increase, amount)
    }


    // ----------------------------------------------------------------------------------------
    // ------------ USER FUNCTIONS :: INSTALL / UPGRADE MODULEs -------------------------------
    // ----------------------------------------------------------------------------------------

    /// Buy a module without installing it
    pub fn buy_module(ctx: Context<BuyModule>, config_id: u16) -> Result<()> {
        user::buy_module(ctx, config_id)
    }
    
    /// Install/deploy an existing undeployed module
    pub fn install_module(ctx: Context<InstallModule>, module_index: u8, pos_x: u8, pos_y: u8) -> Result<()> {
        user::install_module(ctx, module_index, pos_x, pos_y)
    }

    /// Remove a module instance from the moonbase
    pub fn remove_module(ctx: Context<RemoveModuleInstance>, module_index: u8) -> Result<()> {
        user::remove_module_internal(ctx, module_index)
    }

    /// Delete a specific undeployed module permanently
    pub fn delete_module(ctx: Context<DeleteModule>, module_index: u8) -> Result<()> {
        user::delete_module(ctx, module_index)
    }

    /// Upgrade a module instance
    pub fn upgrade_module(ctx: Context<UpdateModuleInstance>, module_index: u8) -> Result<()> {
        user::upgrade_module_internal(ctx, module_index)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ MINING FUNCTIONS :: CLAIM MOONDOGE TOKENS -----------------------------------
    // ----------------------------------------------------------------------------------------
    
    /// Claim DogeBtc tokens based on user's hashpower contribution (CPI only from mooneconomy)
    pub fn claim_dbtc_tokens(ctx: Context<ClaimDogeBtc>, new_electricity: u64) -> Result<()> {
        user::claim_dbtc_tokens_internal(ctx, new_electricity)
    }

    /// Claim accumulated XP from Attraction modules (CPI only from mooneconomy)
    pub fn claim_attraction_xp(ctx: Context<ClaimAttractionXP>, module_index: u8, new_electricity: u64) -> Result<()> {
        user::claim_attraction_xp_internal(ctx, module_index, new_electricity)
    }

    /// Claim level-up rewards based on accumulated XP (with loot transfers)
    pub fn claim_level_up_rewards(ctx: Context<ClaimLevelUpRewards>) -> Result<()> {
        user::claim_level_up_rewards_internal(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG NFT FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------


    /// Incubate a Dragon Egg in the moonbase (max 1 per moonbase)
    pub fn incubate_dragon_egg(ctx: Context<IncubateDragonEgg>) -> Result<()> {
        user::incubate_dragon_egg_internal(ctx)
    }

    /// Remove Dragon Egg from moonbase incubation
    pub fn remove_dragon_egg(ctx: Context<RemoveDragonEgg>) -> Result<()> {
        user::remove_dragon_egg_internal(ctx)
    }

 
}


 