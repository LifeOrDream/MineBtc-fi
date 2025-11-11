use anchor_lang::prelude::*;
mod errors;
mod events;
mod genescience;
pub mod instructions;
mod mpl_core_helpers;
pub mod state;

pub use instructions::admin::*;
pub use instructions::economy::*;
pub use instructions::user::*;
pub use state::{SolFeeConfig, DogeBtcDistConfig};

declare_id!("35isCtM4mT84BFPQazwuu7PmN6hzwHVUZHkYeDqzLzTc");

#[program]
pub mod moonbase {
    use super::*;
    use crate::instructions::admin::{self, CreateDragonEggCollection};
    use crate::instructions::economy::{self};
    use crate::instructions::user::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, creation_fee_recipient: Pubkey) -> Result<()> {
        admin::internal_initialize(ctx, creation_fee_recipient)
    }

    /// Set the Dragon Egg collection address (admin only)
    pub fn set_dragon_egg_collection(
        ctx: Context<UpdateConfigAc>,
        dragon_egg_collection: Pubkey,
    ) -> Result<()> {
        admin::set_dragon_egg_collection_internal(ctx, dragon_egg_collection)
    }

    /// Set the Raydium pool state address (admin only)
    /// Security: Prevents using malicious pools for swaps
    pub fn set_raydium_pool_state(
        ctx: Context<UpdateConfigAc>,
        raydium_pool_state: Pubkey,
    ) -> Result<()> {
        admin::set_raydium_pool_state_internal(ctx, raydium_pool_state)
    }

    /// Set Dragon Egg URIs for a specific tier (admin only)
    /// uris: Vec of URIs, one per faction (must match number of factions)
    pub fn set_dragon_egg_uris_for_tier(
        ctx: Context<UpdateConfigAc>,
        tier: u8,
        uris: Vec<String>,
    ) -> Result<()> {
        admin::set_dragon_egg_uris_for_tier_internal(ctx, tier, uris)
    }

    /// Clear all Dragon Egg URIs (admin only)
    pub fn clear_dragon_egg_uris(ctx: Context<UpdateConfigAc>) -> Result<()> {
        admin::clear_dragon_egg_uris_internal(ctx)
    }

    /// Add a single faction to the global config (admin only)
    pub fn add_faction(ctx: Context<AddFaction>, faction_name: String) -> Result<()> {
        admin::add_faction_internal(ctx, faction_name)
    }

    /// Create Dragon Egg collection with program PDA as authority
    /// This allows the program to mint NFTs from the collection
    pub fn create_dragon_egg_collection(
        ctx: Context<CreateDragonEggCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        admin::create_dragon_egg_collection_internal(ctx, name, uri)
    }

    /// Update the global configuration parameters
    /// Can only be called by the current authority
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_collector: Option<Pubkey>,
        new_creation_fee_recipient: Option<Pubkey>,
        new_emission_per_round: Option<u64>,
    ) -> Result<()> {
        admin::update_config_internal(
            ctx,
            new_authority,
            new_fee_collector,
            new_creation_fee_recipient,
            new_emission_per_round,
        )
    }

    /// Update fee configuration (admin only)
    /// Validates that percentages sum correctly
    pub fn update_fees(
        ctx: Context<UpdateConfigAc>,
        new_sol_fee_config: Option<SolFeeConfig>,
        new_dbtc_dist_config: Option<DogeBtcDistConfig>,
    ) -> Result<()> {
        admin::update_fees_internal(ctx, new_sol_fee_config, new_dbtc_dist_config)
    }

    /// Update egg limits for tiers (admin only)
    pub fn update_egg_limits(
        ctx: Context<UpdateConfigAc>,
        tier2_limit: Option<u64>,
        tier3_limit: Option<u64>,
        tier4_limit: Option<u64>,
    ) -> Result<()> {
        admin::update_egg_limits_internal(ctx, tier2_limit, tier3_limit, tier4_limit)
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
        pool_state: Pubkey,
    ) -> Result<()> {
        admin::initialize_mining_internal(ctx, start_timestamp, doge_btc_per_slot, pool_state)
    }

    /// Deposit moon doge tokens to the mining vault
    pub fn deposit_doge_btc_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        admin::deposit_doge_btc_tokens_internal(ctx, amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ SYSTEM_REFERRAL_ACCOUNT (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------

 
    /// Initialize the loot rewards system 
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

    /// Initialize the global game state for Faction Surge
    pub fn initialize_game_state(
        ctx: Context<InitializeGameState>,
        round_duration_seconds: i64,
    ) -> Result<()> {
        admin::initialize_game_state_internal(ctx, round_duration_seconds)
    }

    /// Initialize a faction state account
    pub fn initialize_faction_state(
        ctx: Context<InitializeFactionState>,
        faction_id: u8,
    ) -> Result<()> {
        admin::initialize_faction_state_internal(ctx, faction_id)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ PRICE ORACLE AND DISTRIBUTION RATE (ANYONE) --------------------------------
    // ----------------------------------------------------------------------------------------

    /// INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)
    /// Performs a small SOL → DOGE_BTC swap for price discovery and earnmarks SOL for POL
    /// After 8 snapshots over 4 hours, call update_rate_and_add_lp to finalize
    pub fn snapshot_price(ctx: Context<SnapshotPrice>) -> Result<()> {
        economy::snapshot_price_internal(ctx)
    }

    /// INSTRUCTION 2: Update distribution rate and add liquidity (can be called after 4 hours)
    /// Checks if 8 snapshots collected, updates distribution rate, and adds liquidity to pool
    ///
    /// When lp_token_amount > 0: Admin override mode (requires authority signature)
    /// When lp_token_amount = 0: Automatic calculation mode (anyone can call)
    pub fn update_rate_and_add_lp(
        ctx: Context<UpdateRateAndAddLp>,
        lp_token_amount: u64,
    ) -> Result<()> {
        economy::update_rate_and_add_lp_internal(ctx, lp_token_amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER FUNCTIONS :: OLD MOONBASE BUILDER (REMOVED) -------------------------
    // ----------------------------------------------------------------------------------------
    // All old moonbase builder functions have been removed as part of the Faction Surge pivot.
    // The new system uses PlayerData instead of UserMoonBaseInstance.

    // ----------------------------------------------------------------------------------------
    // ------------ FACTION SURGE RAFFLE FUNCTIONS -------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize a player account for the Faction Surge game
    pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8) -> Result<()> {
        user::initialize_player(ctx, faction_id)
    }

    /// Update personal hashpower (CPI-only, called by mooneconomy program)
    pub fn update_personal_hashpower(
        ctx: Context<UpdatePersonalHashpower>,
        amount: i128,
        user_pubkey: Pubkey,
    ) -> Result<()> {
        user::update_personal_hashpower(ctx, amount, user_pubkey)
    }

    /// Join a surge round by betting SOL
    pub fn join_surge(ctx: Context<JoinSurge>, amount: u64) -> Result<()> {
        user::join_surge(ctx, amount)
    }

    /// Crank end surge - determines winner and distributes rewards
    pub fn crank_end_surge(ctx: Context<CrankEndSurge>) -> Result<()> {
        user::crank_end_surge(ctx)
    }

    /// Claim surge rewards for a user
    pub fn claim_surge_rewards(ctx: Context<ClaimSurgeRewards>) -> Result<()> {
        user::claim_surge_rewards(ctx)
    }

    /// Initialize autominer vault
    pub fn init_autominer(
        ctx: Context<InitAutominer>,
        sol_per_round: u64,
        num_rounds: u32,
    ) -> Result<()> {
        user::init_autominer(ctx, sol_per_round, num_rounds)
    }

    /// Execute autominer bet (keeper instruction)
    pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
        user::execute_autominer_bet(ctx)
    }

    /// Cancel autominer vault
    pub fn cancel_autominer(ctx: Context<CancelAutominer>) -> Result<()> {
        user::cancel_autominer(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG NFT FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Mint a Dragon Egg NFT with specified faction and tier
    pub fn mint_dragon_egg(
        ctx: Context<MintDragonEgg>,
        faction_id: u8,
        tier: u8,
    ) -> Result<()> {
        user::mint_dragon_egg(ctx, faction_id, tier)
    }

    /// Stake a Dragon Egg to boost hashpower (if faction matches player's faction)
    pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
        user::stake_dragon_egg(ctx)
    }

    /// Unstake a Dragon Egg (remove hashpower boost)
    pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
        user::unstake_dragon_egg(ctx)
    }
}
