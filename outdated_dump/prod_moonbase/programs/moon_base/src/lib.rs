use anchor_lang::prelude::*;
use anchor_lang::system_program;
mod state;
mod errors;
mod events;
pub mod instructions;

// Remove ambiguous glob re-exports
pub use instructions::admin::*;
pub use instructions::user::*;
pub use instructions::game::*;

declare_id!("AKDm1gXxpnJE23aYTtmNE8E9CagM7gkvjKGegDZ9qTPD");

#[program]
pub mod moon_base {
    use super::*;
    use crate::instructions::admin::{self};
    use crate::instructions::user::{self};
    use crate::instructions::game::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
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

    // /// Toggle the PvP game active state (admin only)
    // pub fn toggle_game_active(ctx: Context<ToggleGameActive>) -> Result<()> {
    //     admin::toggle_game_active_internal(ctx)
    // }
    

    /// Add a new faction to the supported factions list (admin only)
    /// Factions cannot be removed once added to maintain data integrity
    pub fn add_faction(ctx: Context<AddFaction>, name: String) -> Result<()> {
        admin::add_faction_internal(ctx, name)
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
    // ------------ PVP MATCHMAKER (ADMIN) :: INITIALIZATION ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the PvP matchmaker (admin only)
    pub fn initialize_pvp_matchmaker(ctx: Context<InitializePvPMatchmaker>) -> Result<()> {
        let matchmaker = &mut ctx.accounts.pvp_matchmaker;
        
        // Initialize empty game lists for all ticket tiers
        matchmaker.index_1_games = Vec::new();
        matchmaker.index_2_games = Vec::new();
        matchmaker.index_3_games = Vec::new();
        matchmaker.index_4_games = Vec::new();
        matchmaker.index_5_games = Vec::new();
        matchmaker.index_6_games = Vec::new();
        matchmaker.index_7_games = Vec::new();
        matchmaker.index_8_games = Vec::new();
        matchmaker.index_9_games = Vec::new();
        matchmaker.index_10_games = Vec::new();
        matchmaker.index_11_games = Vec::new();
        matchmaker.index_12_games = Vec::new();
        matchmaker.index_13_games = Vec::new();
        matchmaker.bump = ctx.bumps.pvp_matchmaker;
        
        msg!("PvP Matchmaker initialized with {} ticket tiers", state::PvPMatchmaker::NUM_INDICES);
        
        Ok(())
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
     
    // ---------------------------------------------------------- 
    // ------------ WITHDRAW SOL FEES (ANYONE) --------------------------------
    // ---------------------------------------------------------- 


    /// Withdraw collected SOL fees from the treasury
    /// 
    /// Called by MoonEconomy program, withdraws SOL and splits it into 3 parts:
    /// 1. For mDOGE stakers
    /// 2. For liquidity providers
    /// 3. For game development
    /// 4. For devs
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

    // ----------------------------------------------------------------------------------------
    // ------------ UPDATE mDOGE DISTRIBUTION RATE (ANYONE) --------------------------------
    // ----------------------------------------------------------------------------------------

    /// Update mDOGE distribution rate based on price oracle (can be called by anyone every hour)
    /// 
    /// We update mDOGE distribution rate every 8 hrs based on price increase / decrease. 
    /// mDOGE is swapped for SOL on raydium every hr for first 7 hrs and combined with mDOGE from mining vault for last hr
    /// and added to the LP pool, with LP tokens being burnt.
    /// 
    /// When lp_token_amount > 0: Admin override mode (requires authority signature)
    /// When lp_token_amount = 0: Automatic calculation mode (anyone can call)
    pub fn update_mdoge_dist_per_slot(ctx: Context<UpdateMdogeDistPerSlot>, lp_token_amount: u64) -> Result<()> {
        admin::update_mdoge_dist_per_slot_internal(ctx, lp_token_amount)
    }


 
    // ----------------------------------------------------------------------------------------
    // ------------ USER FUNCTIONS :: CREATE MOON-BASE, CLAIM REFERRAL REWARDS ---------------- 
    // ----------------------------------------------------------------------------------------

    /// Create a new moon base for a user
    pub fn create_user_moonbase(ctx: Context<CreateUserMoonbase>, referrer: Option<Pubkey>, faction_id: u8) -> Result<()> {
        user::initialize_user_moonbase(ctx, referrer, faction_id)
    }


    /// Purchase a moonbase expansion (user function)
    pub fn expand_moonbase(ctx: Context<ExpandMoonbase>, expansion_id: u8) -> Result<()> {
        user::expand_moonbase_internal(ctx, expansion_id)
    }

    /// Claim referral rewards
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
        user::claim_referral_rewards_internal(ctx)
    }



    pub fn update_user_electricity(  ctx: Context<UpdateUserElectricity>, to_increase: bool, amount: u64) -> Result<()> {
        user::update_user_electricity_internal(ctx, to_increase, amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER FUNCTIONS :: PVP GAME --------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Create a new PvP game
    pub fn create_pvp_game(ctx: Context<CreatePvPGame>, ticket_index: u8) -> Result<()> {
        game::create_pvp_game_internal(ctx, ticket_index)
    }

    /// Join a PvP game
    pub fn join_pvp_game(ctx: Context<JoinPvPGame>, player_a: Pubkey) -> Result<()> {
        game::join_pvp_game_internal(ctx, player_a)
    }

    /// Cancel a PvP game
    pub fn cancel_pvp_game(ctx: Context<CancelPvPGame>) -> Result<()> {
        game::cancel_pvp_game_internal(ctx)
    }

    /// Cancel an expired PvP game (anyone can call after 30 minutes)
    pub fn cancel_expired_pvp_game(ctx: Context<CancelExpiredPvPGame>) -> Result<()> {
        game::cancel_expired_pvp_game_internal(ctx)
    }

    /// PvP attack turn - execute an attack against another player's module
    pub fn pvp_attack_turn(ctx: Context<PvPAttack>, target_module_type: state::ModuleType) -> Result<()> {
        game::pvp_attack_turn_internal(ctx, target_module_type)
    }

    /// Repair moonbase after PvP damage (free after cooldown or paid instantly)
    pub fn repair_moonbase(ctx: Context<RepairMoonbase>, pay_instantly: bool) -> Result<()> {
        game::repair_moonbase_internal(ctx, pay_instantly)
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

    /// Claim accumulated XP from Attraction modules
    pub fn claim_attraction_xp(ctx: Context<ClaimAttractionXP>, module_index: u8) -> Result<()> {
        user::claim_attraction_xp_internal(ctx, module_index)
    }

    /// Claim research rewards from Research modules (casino-style probability system)
    pub fn claim_research_rewards(ctx: Context<ClaimResearchRewards>, module_index: u8) -> Result<()> {
        user::claim_research_rewards_internal(ctx, module_index)
    }

    /// Claim level-up rewards based on accumulated XP (with loot transfers)
    pub fn claim_level_up_rewards(ctx: Context<ClaimLevelUpRewards>) -> Result<()> {
        user::claim_level_up_rewards_internal(ctx)
    }

 
    // ----------------------------------------------------------------------------------------
    // ------------ MINING FUNCTIONS :: CLAIM MOONDOGE TOKENS -----------------------------------
    // ----------------------------------------------------------------------------------------
    
    /// Claim MoonDoge tokens based on user's hashpower contribution
    pub fn claim_mdoge_tokens(ctx: Context<ClaimMoonDoge>) -> Result<()> {
        user::claim_mdoge_tokens_internal(ctx)
    }



}

#[cfg(test)]
mod tests {
    use super::*;
    use instructions::helper;

    #[test]
    fn test_moderate_upgrade_cost_progression() {
        println!("\n🔢 Testing Moderate Upgrade Cost Progression (1.25x multiplier)");
        println!("Base cost: 1.0 SOL (1_000_000_000 lamports)");
        
        let base_cost = 1_000_000_000u64; // 1 SOL
        
        for level in 0..=10 {
            let cost = helper::calculate_upgrade_cost(base_cost, level);
            let multiplier = if level == 0 { 1.0 } else { cost as f64 / base_cost as f64 };
            println!("   Level {}: {:.3} SOL ({:.2}x)", level, cost as f64 / 1e9, multiplier);
        }
        
        // Verify some specific values
        assert_eq!(helper::calculate_upgrade_cost(base_cost, 0), 0); // Level 0 should be free
        
        let level_1_cost = helper::calculate_upgrade_cost(base_cost, 1);
        let expected_level_1 = (base_cost as f64 * 1.25) as u64; // Level 1 should be 1.25x base cost
        assert_eq!(level_1_cost, expected_level_1);
        
        let level_5_cost = helper::calculate_upgrade_cost(base_cost, 5);
        let expected_multiplier_5 = 1.25f64.powi(5); // 1.25^5 ≈ 3.05
        let expected_cost_5 = (base_cost as f64 * expected_multiplier_5) as u64;
        assert!((level_5_cost as f64 - expected_cost_5 as f64).abs() < 1000.0); // Within 1000 lamports
        
        let level_10_cost = helper::calculate_upgrade_cost(base_cost, 10);
        let expected_multiplier_10 = 1.25f64.powi(10); // 1.25^10 ≈ 9.31
        println!("\nLevel 10 analysis:");
        println!("   Actual cost: {:.3} SOL", level_10_cost as f64 / 1e9);
        println!("   Expected multiplier: {:.2}x", expected_multiplier_10);
        println!("   Much more reasonable than the previous 57x!");
    }

    #[test]
    fn test_sol_based_xp_progression() {
        println!("\n🌟 Testing SOL-based XP Progression (500 XP per SOL with sqrt scaling)");
        
        let test_amounts = [
            100_000_000,   // 0.1 SOL
            500_000_000,   // 0.5 SOL
            1_000_000_000, // 1.0 SOL
            2_000_000_000, // 2.0 SOL
            5_000_000_000, // 5.0 SOL
            10_000_000_000, // 10.0 SOL
            20_000_000_000, // 20.0 SOL
        ];
        
        for amount in test_amounts.iter() {
            let xp = helper::calculate_sol_based_xp(*amount);
            let sol_value = *amount as f64 / 1e9;
            let xp_per_sol = xp as f64 / sol_value;
            println!("   {:.1} SOL → {} XP ({:.1} XP/SOL)", sol_value, xp, xp_per_sol);
        }
        
        // Verify some specific values
        let xp_0_1_sol = helper::calculate_sol_based_xp(100_000_000);
        let xp_1_sol = helper::calculate_sol_based_xp(1_000_000_000);
        let xp_10_sol = helper::calculate_sol_based_xp(10_000_000_000);
        
        // Check that 1 SOL gives approximately 500 XP (with sqrt scaling)
        // sqrt(1e9) * 500 / sqrt(1e9) = 500
        assert!(xp_1_sol >= 450 && xp_1_sol <= 550, "1 SOL should give ~500 XP, got {}", xp_1_sol);
        
        // Check that scaling is sublinear (sqrt scaling means diminishing returns)
        assert!(xp_10_sol < xp_1_sol * 10, "10 SOL should give less than 10x the XP of 1 SOL due to sqrt scaling");
        
        println!("\n✅ SOL-based XP system shows proper diminishing returns with sqrt scaling");
        println!("   Old fixed system: 50 XP for any module install");
        println!("   New dynamic system: {} XP for 1 SOL spend", xp_1_sol);
    }
}
