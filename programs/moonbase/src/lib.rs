use anchor_lang::prelude::*;
mod errors;
mod events;
mod genescience;
pub mod instructions;
mod mpl_core_helpers;
pub mod state;

pub use instructions::admin::*;
pub use instructions::economy::*;
pub use instructions::game::*;
pub use instructions::user::*;
pub use instructions::stake::*;
pub use instructions::eggs::*;
pub use state::{SolFeeConfig, DogeBtcDistConfig, BetType, EggConfig, TicketTier};

declare_id!("35isCtM4mT84BFPQazwuu7PmN6hzwHVUZHkYeDqzLzTc");

#[program]
pub mod moonbase {
    use super::*;
    use crate::instructions::admin::{self, CreateDragonEggCollection};
    use crate::instructions::economy::{self};
    use crate::instructions::game::{self};
    use crate::instructions::user::{self};
    use crate::instructions::stake::{self};
    use crate::instructions::eggs::{self};

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
    ) -> Result<()> {
        admin::update_config_internal(
            ctx,
            new_authority,
            new_fee_collector,
            new_creation_fee_recipient,
        )
    }

    /// Update fee configuration (admin only)
    /// Validates that percentages sum correctly
    pub fn update_fees(
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
        admin::update_fees_internal(
            ctx,
            new_protocol_fee_pct,
            new_buyback_pct,
            new_stakers_pct,
            new_dbtc_stakers_pct,
            new_dbtc_winners_pct,
            new_dbtc_same_faction_pct,
            new_dbtc_motherlode_pct,
            new_refining_fee,
        )
    }

    /// Update egg limits for tiers (admin only)
    pub fn update_egg_limits(
        ctx: Context<UpdateConfigAc>,
        tier1_limit: Option<u64>,
        tier2_limit: Option<u64>,
        tier3_limit: Option<u64>,
        tier4_limit: Option<u64>,
    ) -> Result<()> {
        admin::update_egg_limits_internal(ctx, tier1_limit, tier2_limit, tier3_limit, tier4_limit)
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
        admin::distribute_sol_fees_internal(ctx)
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

    /// Add a cranker bot to the whitelist (admin only)
    /// Maximum MAX_CRANKER_BOTS bots can be whitelisted
    pub fn add_cranker_bot(
        ctx: Context<UpdateGameState>,
        bot_pubkey: Pubkey,
    ) -> Result<()> {
        admin::add_cranker_bot_internal(ctx, bot_pubkey)
    }

    /// Remove a cranker bot from the whitelist (admin only)
    pub fn remove_cranker_bot(
        ctx: Context<UpdateGameState>,
        bot_pubkey: Pubkey,
    ) -> Result<()> {
        admin::remove_cranker_bot_internal(ctx, bot_pubkey)
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
    pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
        user::initialize_player(ctx, faction_id, referral_code)
    }

    // Note: update_personal_hashpower function removed - no longer needed

    /// Join a round by betting SOL
    /// Users can bet on either a specific block (1-24) or a faction + highest/lowest option
    pub fn join_round(
        ctx: Context<JoinRound>, 
        amount: u64,
        bet_type: BetType,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        user::join_round(ctx, amount, bet_type, use_ticket)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ GAME ROUND MANAGEMENT (COMMIT-REVEAL RANDOMNESS) ------------------------
    // ----------------------------------------------------------------------------------------

    /// Start a new round by committing a hash and initializing GameSession
    /// This commits randomness hash and randomly assigns factions to blocks
    /// If commit_hash is None, uses next_round_commit from global_state
    pub fn start_round(
        ctx: Context<StartRound>,
        commit_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        game::start_round(ctx, commit_hash)
    }

    /// End the current round by revealing seed, selecting winner, and starting next round
    /// Verifies commit-reveal, generates final randomness, selects winning block
    pub fn end_round(
        ctx: Context<EndRound>,
        revealed_seed: [u8; 32],
        next_round_commit: [u8; 32],
    ) -> Result<()> {
        game::end_round(ctx, revealed_seed, next_round_commit)
    }

    /// Claim rewards for a user after round ends
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        user::claim_rewards(ctx)
    }

    /// Initialize autominer vault with bet types and amounts
    pub fn init_autominer(
        ctx: Context<InitAutominer>,
        bet_types: Vec<BetType>,
        bet_amount_per_bet: u64,
        num_rounds: u32,
    ) -> Result<()> {
        user::init_autominer(ctx, bet_types, bet_amount_per_bet, num_rounds)
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
    // ------------ USER INSTRUCTIONS :: STAKE & UNSTAKE MOONDOGE / LP TOKENs  ------------
    // ----------------------------------------------------------------------------------------

    /// Stake DogeBtc tokens to earn SOL and dbtc rewards
    pub fn stake_moondoge(
        ctx: Context<StakeDogeBtc>,
        faction_id: u8,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::stake_moondoge(ctx, faction_id, amount, lockup_duration, position_index)
    }

    /// Unstake DogeBtc tokens from a position
    pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
        stake::unstake_moondoge(ctx, position_index)
    }

    /// Stake LP tokens to earn SOL and dbtc rewards
    pub fn stake_lp_tokens(
        ctx: Context<StakeLpTokens>,
        faction_id: u8,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::stake_lp_tokens(ctx, faction_id, amount, lockup_duration, position_index)
    }

    /// Unstake LP tokens from a position
    pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
        stake::unstake_lp_tokens(ctx, position_index)
    }
    
    /// Claim SOL rewards from DogeBtc and LP staking
    pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>, faction_id: u8) -> Result<()> {
        stake::claim_sol_rewards(ctx, faction_id)
    }
    
    /// Claim DogeBtc token rewards from staking (with refining fee redistribution)
    pub fn claim_dbtc_rewards(ctx: Context<ClaimDbtcRewards>, faction_id: u8) -> Result<()> {
        stake::claim_dbtc_rewards(ctx, faction_id)
    }
    
    /// Claim referral rewards (SOL and DogeBtc earned from referrals)
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
        stake::claim_referral_rewards(ctx)
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
        eggs::mint_dragon_egg(ctx, faction_id, tier)
    }

    /// Stake a Dragon Egg to boost hashpower (if faction matches player's faction)
    pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
        eggs::stake_dragon_egg(ctx)
    }

    /// Unstake a Dragon Egg (remove hashpower boost)
    pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
        eggs::unstake_dragon_egg(ctx)
    }
}
