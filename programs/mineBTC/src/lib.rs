// # MineBTC Program
//
// The main entry point for the MineBTC program.
//
// This program implements a faction-based betting and mining game on Solana.
// Users can join factions, place bets on blocks, mine MineBTC tokens, and stake assets for rewards.
//
// ## Modules
//
// - `admin`: Administrative functions for configuration and management.
// - `economy`: Tokenomics, fee distribution, and liquidity management.
// - `user`: User interactions, betting, and account management.
// - `stake`: Staking logic for MineBTC and LP tokens.
// - `game`: Core game loop, round management, and randomness.
// - `eggs`: Egg NFT system for hashpower multipliers.
// - `tax`: Tax system for deflationary mechanics and reward distribution.
//
// ## Architecture
//
// The program uses a hub-and-spoke architecture with `GlobalConfig` and `GlobalGameState` as central
// state accounts. Users interact through `PlayerData` accounts, and factions are tracked via `FactionState`.
//

use anchor_lang::prelude::*;
mod errors;
mod events;
mod genescience;
pub mod instructions;
mod mpl_core_helpers;
pub mod state;

pub use instructions::admin::CreatorInput;
pub use instructions::admin::*;
pub use instructions::economy::*;
pub use instructions::doges::*;
pub use instructions::game::*;
pub use instructions::stake::*;
pub use instructions::tax::*;
pub use instructions::user::*;
pub use state::{
    BetType, BlocksConfig, EggConfig, FactionStrategy, FactionsConfig, MineBtcDistConfig,
    SolFeeConfig, TaxConfig, TicketTier,
};

declare_id!("4ybsV8wziB7Z4DMJjkc6x3ZhzaevRNgD4DbXNz6Ta5Ed");

#[program]
pub mod minebtc {
    use super::*;
    use instructions::admin::{self};
    use instructions::economy::{self};
    use instructions::doges::{self};
    use instructions::game::{self};
    use instructions::stake::{self};
    use instructions::tax::{self};
    use instructions::user::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, fee_recipient: Pubkey) -> Result<()> {
        admin::internal_initialize(ctx, fee_recipient)
    }

    /// Set the Raydium pool state address (admin only)
    /// Security: Prevents using malicious pools for swaps
    /// Also initializes sol_rewards_vault and sol_prize_pot_vault if not already initialized
    pub fn set_raydium_pool_state(
        ctx: Context<SetRaydiumPoolState>,
        raydium_pool_state: Pubkey,
    ) -> Result<()> {
        admin::set_raydium_pool_state_internal(ctx, raydium_pool_state)
    }

    /// Add a single faction to the global config (admin only)
    pub fn add_faction(
        ctx: Context<AddFaction>,
        faction_name: String,
        faction_id: u8,
    ) -> Result<()> {
        admin::add_faction_internal(ctx, faction_name, faction_id)
    }

    /// Initialize system referral account and buybacks system (admin only)
    pub fn initialize_system_accounts(ctx: Context<InitializeSystemAccounts>) -> Result<()> {
        admin::initialize_system_accounts_internal(ctx)
    }

    /// Update the global configuration parameters
    /// Can only be called by the current authority
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_recipient: Option<Pubkey>,
    ) -> Result<()> {
        admin::update_config_internal(ctx, new_authority, new_fee_recipient)
    }

    /// Update fee configuration (admin only)
    /// Validates that percentages sum correctly
    pub fn update_fees(
        ctx: Context<UpdateConfigAc>,
        new_protocol_fee_pct: Option<u8>,
        new_buyback_pct: Option<u8>,
        new_stakers_pct: Option<u8>,
        new_minebtc_stakers_pct: Option<u8>,
        new_minebtc_winners_pct: Option<u8>,
        new_minebtc_same_faction_pct: Option<u8>,
        new_minebtc_motherlode_pct: Option<u8>,
        new_refining_fee: Option<u8>,
        change_faction_fee: Option<u64>,
        snapshot_interval: Option<u64>,
    ) -> Result<()> {
        admin::update_fees_internal(
            ctx,
            new_protocol_fee_pct,
            new_buyback_pct,
            new_stakers_pct,
            new_minebtc_stakers_pct,
            new_minebtc_winners_pct,
            new_minebtc_same_faction_pct,
            new_minebtc_motherlode_pct,
            new_refining_fee,
            change_faction_fee,
            snapshot_interval,
        )
    }

    /// Toggle RPG progression (mutations, XP) during gameplay
    pub fn update_rpg_progression(ctx: Context<UpdateConfigAc>, enabled: bool) -> Result<()> {
        admin::update_rpg_progression_internal(ctx, enabled)
    }

    /// Update breeding configuration (admin only)
    pub fn update_breeding_config(
        ctx: Context<UpdateEggsConfig>,
        breeding_allowed: bool,
        breed_base_price: u64,
        breed_curve_a: u64,
    ) -> Result<()> {
        admin::update_breeding_config_internal(ctx, breeding_allowed, breed_base_price, breed_curve_a)
    }

    /// Update emission adjustment parameters (admin only)
    /// Allows updating price change threshold and emission increase/decrease percentages
    pub fn update_emission_params(
        ctx: Context<UpdateEmissionParams>,
        price_change_threshold: Option<u64>,
        emission_increase_pct: Option<u64>,
        emission_decrease_pct: Option<u64>,
    ) -> Result<()> {
        admin::update_emission_params_internal(
            ctx,
            price_change_threshold,
            emission_increase_pct,
            emission_decrease_pct,
        )
    }

    // ----------------------------------------------------------------------------------------
    // ------------ mine_btc_MINING (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize mining by setting the token vault and starting timestamp
    /// Can only be called once when mining_start_timestamp is 0
    pub fn initialize_mining(
        ctx: Context<InitializeMining>,
        start_timestamp: u64,
        mine_btc_per_round: u64,
        pool_state: Pubkey,
    ) -> Result<()> {
        admin::initialize_mining_internal(ctx, start_timestamp, mine_btc_per_round, pool_state)
    }

    /// Deposit MineBtc tokens to the mining vault (anyone can call)
    ///
    /// Allows anyone to deposit MineBtc tokens into the mining vault.
    /// These tokens will be distributed as rewards to stakers over time.
    pub fn deposit_mine_btc_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        admin::deposit_mine_btc_tokens_internal(ctx, amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ HASHPOWER CONFIG (ADMIN) ------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize HashpowerConfig account (admin only)
    pub fn initialize_hashpower_config(
        ctx: Context<InitializeHashpowerConfig>,
        min_lockup_days: u64,
        max_lockup_days: u64,
        base_multiplier: u16,
        max_multiplier: u16,
    ) -> Result<()> {
        admin::initialize_hashpower_config_internal(
            ctx,
            min_lockup_days,
            max_lockup_days,
            base_multiplier,
            max_multiplier,
        )
    }

    /// Initialize both custodian token accounts (admin only)
    /// Initializes:
    /// - MINEBTC custodian: Token-2022 account that holds all staked MINE_BTC tokens (global for all factions)
    /// - Liquidity custodian: Standard SPL Token account that holds all staked LP tokens (global for all factions)
    pub fn initialize_custodian_accounts(ctx: Context<InitializeCustodianAccounts>) -> Result<()> {
        admin::int_initialize_custodian_accounts(ctx)
    }

    /// Update HashpowerConfig account (admin only)
    pub fn update_hashpower_config(
        ctx: Context<UpdateHashpowerConfig>,
        min_lockup_days: u64,
        max_lockup_days: u64,
        base_multiplier: u16,
        max_multiplier: u16,
    ) -> Result<()> {
        admin::update_hashpower_config_internal(
            ctx,
            min_lockup_days,
            max_lockup_days,
            base_multiplier,
            max_multiplier,
        )
    }

    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG SYSTEM (ADMIN) ------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize EggConfig account (admin only)
    ///
    /// Creates the EggConfig account that stores Egg configuration.
    /// This must be called before creating the Egg collection.
    pub fn initialize_egg_config(
        ctx: Context<InitializeEggConfig>,
        base_price: u64,
        curve_a: u64,
        max_supply: u64,
    ) -> Result<()> {
        admin::initialize_egg_config_internal(ctx, base_price, curve_a, max_supply)
    }

    /// Update EggConfig account (admin only)
    ///
    /// Updates the EggConfig account that stores Egg collection configuration.
    ///
    /// # Parameters
    /// - `base_price`: Base price for Eggs in SOL (lamports)
    /// - `curve_a`: Bonding curve parameter (controls price growth rate)
    pub fn update_egg_config(
        ctx: Context<UpdateEggsConfig>,
        base_price: u64,
        curve_a: u64,
    ) -> Result<()> {
        admin::update_egg_config_internal(ctx, base_price, curve_a)
    }

    /// Create Egg collection with program PDA as authority (admin only)
    ///
    /// Creates a new Metaplex Core collection for Egg NFTs.
    /// The collection's update authority is set to a program-controlled PDA,
    /// allowing the program to mint NFTs from the collection.
    /// Requires EggConfig to be initialized first.
    pub fn create_egg_collection(
        ctx: Context<CreateEggCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        admin::create_egg_collection_internal(ctx, name, uri)
    }

    /// Set Egg URIs for all factions (admin only)
    /// uris: Vec of URIs, one per faction (must match number of factions)
    pub fn set_egg_uris(ctx: Context<UpdateEggsConfig>, uris: Vec<String>) -> Result<()> {
        admin::set_egg_uris_internal(ctx, uris)
    }

    /// Clear all Egg URIs (admin only)
    pub fn clear_egg_uris(ctx: Context<UpdateEggsConfig>) -> Result<()> {
        admin::clear_egg_uris_internal(ctx)
    }

    /// Initialize royalties on the Egg collection (admin only)
    pub fn init_egg_royalties(
        ctx: Context<InitEggRoyalties>,
        basis_points: u16,
        creators: Vec<CreatorInput>,
    ) -> Result<()> {
        admin::init_egg_royalties_internal(ctx, basis_points, creators)
    }

    /// Add or update ticket tier configs (admin only)
    /// Max 4 ticket tier configs can be set
    pub fn add_ticket_tier_config(
        ctx: Context<UpdateEggsConfig>,
        ticket_tier_index: u8,
        ticket_value: u64,
    ) -> Result<()> {
        admin::add_ticket_tier_config_int(ctx, ticket_tier_index, ticket_value)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ TAX SYSTEM (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize TaxConfig account and create vault token accounts (admin only)
    pub fn initialize_tax_config(
        ctx: Context<InitializeTaxConfig>,
        nft_floor_sweep_pct: u8,
        faction_treasury_pct: u8,
        burn_pct: u8,
        nft_floor_sweep_whitelisted_address: Pubkey,
    ) -> Result<()> {
        tax::internal_initialize_tax_config(ctx, nft_floor_sweep_pct, faction_treasury_pct, burn_pct, nft_floor_sweep_whitelisted_address)
    }

    /// Update tax distribution percentages (admin only)
    pub fn update_tax_config(
        ctx: Context<UpdateTaxConfig>,
        nft_floor_sweep_pct: u8,
        faction_treasury_pct: u8,
        burn_pct: u8,
    ) -> Result<()> {
        tax::internal_update_tax_config(ctx, nft_floor_sweep_pct, faction_treasury_pct, burn_pct)
    }

    /// Update NFT floor sweep whitelisted address (admin only)
    pub fn update_nft_floor_sweep_whitelist(
        ctx: Context<UpdateNftFloorSweepWhitelist>,
        new_whitelisted_address: Pubkey,
    ) -> Result<()> {
        tax::internal_update_nft_floor_sweep_whitelist(ctx, new_whitelisted_address)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ GAME STATE MANAGEMENT (ADMIN) ------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global game state for Faction Surge (admin only)
    ///
    /// Sets up the GlobalGameState account that tracks game rounds, betting, and rewards.
    /// This must be called before any rounds can be started.
    pub fn initialize_game_state(
        ctx: Context<InitializeGameState>,
        round_duration_seconds: i64,
    ) -> Result<()> {
        admin::initialize_game_state_internal(ctx, round_duration_seconds)
    }

    /// Add a cranker bot to the whitelist (admin only)
    /// Maximum MAX_CRANKER_BOTS bots can be whitelisted
    pub fn add_cranker_bot(ctx: Context<UpdateGameState>, bot_pubkey: Pubkey) -> Result<()> {
        admin::add_cranker_bot_internal(ctx, bot_pubkey)
    }

    /// Remove a cranker bot from the whitelist (admin only)
    pub fn remove_cranker_bot(ctx: Context<UpdateGameState>, bot_pubkey: Pubkey) -> Result<()> {
        admin::remove_cranker_bot_internal(ctx, bot_pubkey)
    }

    /// Switch game state (toggle is_active) (admin only)
    ///
    /// Toggles the game's active state. When paused, rounds cannot be started or ended.
    pub fn switch_game_state(ctx: Context<UpdateGameState>) -> Result<()> {
        admin::switch_game_state_internal(ctx)
    }

    /// Update round duration (admin only)
    ///
    /// Updates the duration of each game round in seconds.
    ///
    /// # Parameters
    /// - `new_round_duration_seconds`: New round duration in seconds (must be > 0)
    pub fn update_round_duration(
        ctx: Context<UpdateGameState>,
        new_round_duration_seconds: i64,
    ) -> Result<()> {
        admin::update_round_duration_internal(ctx, new_round_duration_seconds)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ PRICE ORACLE AND DISTRIBUTION RATE (ANYONE) --------------------------------
    // ----------------------------------------------------------------------------------------

    /// Withdraw collected SOL fees from the treasury (anyone can call)
    ///
    /// Withdraws SOL from the treasury and distributes it according to configured percentages:
    /// - Protocol fee percentage
    /// - Buyback percentage (for token buybacks)
    /// - Stakers percentage (distributed to stakers)
    ///
    /// The remaining amount goes to the fee recipient (dev earnings).
    pub fn distribute_sol_fees(ctx: Context<DistributeSolFees>) -> Result<()> {
        economy::distribute_sol_fees_internal(ctx)
    }

    /// INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)
    /// Performs a small SOL → MINE_BTC swap for price discovery and earnmarks SOL for POL
    /// After 8 snapshots over 4 hours, call update_rate then add_lp_and_burn to finalize
    pub fn snapshot_price(ctx: Context<SnapshotPrice>) -> Result<()> {
        economy::snapshot_price_internal(ctx)
    }

    /// INSTRUCTION 2a: Update distribution rate (can be called after 4 hours)
    /// Checks if 8 snapshots collected, updates distribution rate, sets flag for LP operation
    pub fn update_rate(ctx: Context<UpdateRate>) -> Result<()> {
        economy::update_rate_internal(ctx)
    }

    /// INSTRUCTION 2b: Add liquidity and burn LP tokens (called after update_rate)
    /// When lp_token_amount > 0: Admin override mode (requires authority signature)
    /// When lp_token_amount = 0: Automatic calculation mode (anyone can call)
    pub fn add_lp_and_burn(ctx: Context<AddLpAndBurn>, lp_token_amount: u64) -> Result<()> {
        economy::add_lp_and_burn_internal(ctx, lp_token_amount)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ TAX SYSTEM FUNCTIONS ------------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Withdraw MineBtc from NFT floor sweep vault (whitelisted address only)
    pub fn withdraw_nft_floor_sweep_funds(
        ctx: Context<WithdrawNftFloorSweepFunds>,
        amount: u64,
    ) -> Result<()> {
        tax::internal_withdraw_nft_floor_sweep_funds(ctx, amount)
    }

    /// STEP 1: Harvest fees from user token accounts to the mint
    /// Callable by anyone - keeper bot should call this in batches
    pub fn crank_harvest_fees<'info>(
        ctx: Context<'_, '_, '_, 'info, CrankHarvestFees<'info>>,
    ) -> Result<()> {
        tax::internal_crank_harvest_fees(ctx)
    }

    /// STEP 2: Withdraw total tax from mint and distribute it
    /// Callable by anyone - program-controlled withdraw authority
    pub fn crank_distribute_tax(ctx: Context<CrankDistributeTax>) -> Result<()> {
        tax::internal_crank_distribute_tax(ctx)
    }

    /// Start a new distribution round (callable by anyone after 7-day cooldown)
    pub fn start_distribution_round(ctx: Context<StartDistributionRound>) -> Result<()> {
        tax::internal_start_distribution_round(ctx)
    }

    /// Calculate leaderboard position for one faction
    /// Must be called 12 times to build complete leaderboard
    pub fn cal_faction_positions(
        ctx: Context<CalculateFactionLeaderboard>,
    ) -> Result<()> {
        tax::internal_cal_faction_positions(ctx)
    }

    /// Calculate rewards for all factions based on leaderboard
    /// Can only be called after all 12 factions are on leaderboard
    pub fn cal_faction_rewards(ctx: Context<CalculateFactionRewards>) -> Result<()> {
        tax::internal_cal_faction_rewards(ctx)
    }

    /// Claim treasury rewards for one faction
    /// Adds rewards to staking reward indexes (50% each to minebtc and lp stakers)
    pub fn claim_faction_treasury_rewards(ctx: Context<ClaimFactionTreasuryRewards>) -> Result<()> {
        tax::internal_claim_faction_treasury_rewards(ctx)
    }

    /// Finish distribution round (check all factions claimed and reset state)
    pub fn finish_distribution_round(ctx: Context<FinishDistributionRound>) -> Result<()> {
        tax::internal_finish_distribution_round(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ GAME ROUND MANAGEMENT (COMMIT-REVEAL RANDOMNESS) ------------------------
    // ----------------------------------------------------------------------------------------

    /// Start a new round by committing a hash and initializing GameSession
    /// This commits randomness hash and randomly assigns factions to blocks
    /// round_id should be current_round_id + 1 (validated in the function)
    pub fn start_round(ctx: Context<StartRound>, round_id: u64, commit: [u8; 32]) -> Result<()> {
        game::int_start_round(ctx, round_id, commit)
    }

    /// End the current round by revealing seed, selecting winner, and starting next round
    /// Verifies commit-reveal, generates final randomness, selects winning block
    pub fn end_round(ctx: Context<EndRound>, revealed_seed: [u8; 32]) -> Result<()> {
        game::int_end_round(ctx, revealed_seed)
    }

    /// End the current round by revealing seed, selecting winner, and starting next round
    /// Verifies commit-reveal, generates final randomness, selects winning block
    pub fn end_round_faction_rewards(ctx: Context<EndRoundFactionRewards>) -> Result<()> {
        game::int_end_round_faction_rewards(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ FACTION SURGE RAFFLE FUNCTIONS -------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize a player account for the Faction Surge game
    pub fn initialize_player(
        ctx: Context<InitializePlayer>,
        faction_id: u8,
        referral_code: Option<Pubkey>,
    ) -> Result<()> {
        user::internal_initialize_player(ctx, faction_id, referral_code)
    }

    /// Change user's faction
    /// Requires no staked positions (minebtc/lp hashpower = 0, no eggs staked)
    /// Charges change_faction_fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
    pub fn change_faction(ctx: Context<ChangeFaction>, new_faction_id: u8) -> Result<()> {
        user::internal_change_faction(ctx, new_faction_id)
    }

    /// Join a round by betting SOL
    /// Users can bet on either a specific block (1-24) or a faction + highest/lowest option
    pub fn join_round(
        ctx: Context<JoinRound>,
        amount: u64,
        bet_type: BetType,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        user::internal_join_round(ctx, amount, bet_type, use_ticket)
    }

    /// Join a round with multiple bets in a single transaction
    pub fn join_round_batch(
        ctx: Context<JoinRoundBatch>,
        bet_types: Vec<BetType>,
        amount_per_bet: u64,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        user::internal_join_round_batch(ctx, bet_types, amount_per_bet, use_ticket)
    }

    /// Initialize autominer vault with flexible block/faction configuration
    pub fn init_autominer(
        ctx: Context<InitAutominer>,
        blocks_config: Option<BlocksConfig>,
        factions_config: Option<FactionsConfig>,
        sol_per_round: u64,
        num_rounds: u32,
    ) -> Result<()> {
        user::internal_init_autominer(
            ctx,
            blocks_config,
            factions_config,
            sol_per_round,
            num_rounds,
        )
    }

    /// Execute autominer bet (keeper instruction)
    pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
        user::internal_execute_autominer_bet(ctx)
    }

    /// Stop autominer and refund remaining SOL
    pub fn stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
        user::internal_stop_autominer(ctx)
    }

    /// Claim rewards for a user after round ends
    pub fn claim_round_rewards(ctx: Context<ClaimRoundRewards>, round_id: u64) -> Result<()> {
        user::internal_claim_round_rewards(round_id, ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER INSTRUCTIONS :: GAMEPLAY EGGS  ------------
    // ----------------------------------------------------------------------------------------

    /// Use an egg for gameplay - deposits to custody and sets as active gameplay egg
    pub fn use_egg_for_gameplay(ctx: Context<UseEggForGameplay>) -> Result<()> {
        user::internal_use_egg_for_gameplay(ctx)
    }

    /// Withdraw egg from gameplay - returns egg to user
    pub fn withdraw_egg_from_gameplay(ctx: Context<WithdrawEggFromGameplay>) -> Result<()> {
        user::internal_withdraw_egg_from_gameplay(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER INSTRUCTIONS :: STAKE & UNSTAKE MINEBTC / LP TOKENs  ------------
    // ----------------------------------------------------------------------------------------

    /// Stake MineBtc tokens to earn SOL and minebtc rewards
    pub fn stake_minebtc(
        ctx: Context<StakeMineBtc>,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::int_stake_minebtc(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake MineBtc tokens from a position
    pub fn unstake_minebtc(ctx: Context<UnstakeMineBtc>, position_index: u8) -> Result<()> {
        stake::int_unstake_minebtc(ctx, position_index)
    }

    /// Stake LP tokens to earn SOL and minebtc rewards
    pub fn stake_lp_tokens(
        ctx: Context<StakeLpTokens>,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::int_stake_lp_tokens(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake LP tokens from a position
    pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
        stake::int_unstake_lp_tokens(ctx, position_index)
    }

    /// Claim SOL rewards from MineBtc and LP staking
    pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
        stake::int_claim_sol_rewards(ctx)
    }

    /// Claim MineBtc token rewards from staking (with refining fee redistribution)
    pub fn claim_minebtc_rewards(ctx: Context<ClaimDbtcRewards>) -> Result<()> {
        stake::int_claim_minebtc_rewards(ctx)
    }

    /// Claim referral rewards (SOL and MineBtc earned from referrals)
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
        stake::int_claim_referral_rewards(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG NFT FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------

    ///Simulate mint costs for multiple eggs accounting for bonding curve pricing
    ///
    /// # Parameters
    /// - `egg_config`: EggConfig account
    /// - `mint_count`: Number of eggs to mint
    pub fn simulate_purchase_cost(
        ctx: Context<SimulateMintCost>,
        mint_count: u64,
    ) -> Result<(u64, Vec<u64>, Vec<(u64, u64)>)> {
        doges::int_simulate_mint_cost(&ctx.accounts.egg_config, mint_count)
    }

    /// Admin function to mint a Egg NFT for free to a specified recipient (admin only)
    ///
    /// Allows the admin to mint a Egg NFT without payment.
    /// The NFT is minted directly to the specified recipient address.
    ///
    /// # Parameters
    /// - `recipient`: Address that will receive the minted NFT
    /// - `faction_id`: Faction ID the egg belongs to
    pub fn admin_mint_egg(
        ctx: Context<AdminMintEgg>,
        recipient: Pubkey,
        faction_id: u8,
        ticket_tier_index: u8,
    ) -> Result<()> {
        doges::int_admin_mint_egg(ctx, recipient, faction_id, ticket_tier_index)
    }

    /// Batch mint multiple Eggs (anyone can call, max 10 per transaction)
    ///
    /// Mints multiple Egg NFTs in a single transaction.
    /// Each egg uses bonding curve pricing based on the current supply at mint time.
    ///
    /// # Parameters
    /// - `faction_id`: Faction ID all eggs belong to
    /// - `mint_count`: Number of eggs to mint (1-10)
    /// - `ticket_tier_index`: Ticket tier index (0-2)
    pub fn batch_mint_eggs<'info>(
        ctx: Context<'_, '_, '_, 'info, BatchMintEggs<'info>>,
        faction_id: u8,
        mint_count: u8,
        ticket_tier_index: u8,
    ) -> Result<()> {
        doges::int_batch_mint_eggs(ctx, faction_id, mint_count, ticket_tier_index)
    }

    /// Stake a Egg to boost hashpower (if faction matches player's faction)
    pub fn stake_egg(ctx: Context<StakeEgg>) -> Result<()> {
        doges::int_stake_egg(ctx)
    }

    /// Unstake a Egg (remove hashpower boost)
    pub fn unstake_egg(ctx: Context<UnstakeEgg>) -> Result<()> {
        doges::int_unstake_egg(ctx)
    }

    /// Breed two eggs to create offspring
    pub fn breed_eggs(ctx: Context<BreedEggs>) -> Result<()> {
        doges::int_breed_eggs(ctx)
    }

    /// Send an egg to heaven (burn for rewards)
    pub fn send_to_heaven(ctx: Context<SendToHeaven>) -> Result<()> {
        doges::int_send_to_heaven(ctx)
    }
}
