#![allow(unexpected_cfgs, deprecated)]
#![allow(
    clippy::identity_op,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::useless_asref
)]

// # MineBTC Program
//
// Degen country arena game on Solana.
//
// Players pick a country, pick a direction, bet SOL. Their Doge Operators build XP
// through gameplay and can trigger story events during rounds. Those events may
// mutate DNA on-chain, but the product-level primitive is broader: the backend can
// turn each story event into character history, artwork, reels, or social content.
// Deflationary dogeBTC economy with 0.1% transfer tax, POL burns, and faction staking rewards.
//
// ## Modules
//
// - `admin`: Configuration, factions, fee parameters.
// - `economy`: Price snapshots, emission rate adjustment, POL (LP add + burn).
// - `user`: Betting, autominers, round claims, gameplay doges, story events.
// - `stake`: dogeBTC and LP token staking.
// - `game`: 60-second round loop, slot-hash randomness, winner selection.
// - `doges`: Doge NFT minting, breeding, staking, evolution.
// - `faction_war`: Story-event-driven competitive cycles, settlement, and cycle rewards.
// - `tax`: Transfer-tax harvest, faction treasury distribution.
//

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;
use borsh::{BorshDeserialize, BorshSerialize};
mod errors;
mod events;
mod genescience;
pub mod instructions;
mod mpl_core_helpers;
pub mod state;

pub use instructions::admin::CreatorInput;
pub use instructions::admin::*;
pub use instructions::doges::*;
pub use instructions::economy::*;
pub use instructions::faction_war::*;
pub use instructions::game::*;
pub use instructions::stake::*;
pub use instructions::tax::*;
pub use instructions::user::*;
pub use state::{
    AutominerFactionPick, BetType, DogeConfig, DogeMintConfig, FactionsConfig, MineBtcDistConfig,
    PredictionDirection, SolFeeConfig, TaxConfig, TicketTier,
};

declare_id!("DPfSfuStn4cU1p4G7PTcqDiWdufGg9kpJPrsnatG6SLG");

#[macro_export]
macro_rules! log_fn {
    ($file:literal, $func:literal) => {
        msg!(concat!(":::", $file, ".", $func, ":::"));
    };
}

#[program]
pub mod minebtc {
    use super::*;
    use instructions::admin::{self};
    use instructions::doges::{self};
    use instructions::economy::{self};
    use instructions::faction_war::{self};
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
        crate::log_fn!("lib", "initialize");
        admin::internal_initialize(ctx, fee_recipient)
    }

    /// Set the Raydium pool state address (admin only)
    /// Security: Prevents using malicious pools for swaps
    /// Also initializes sol_rewards_vault and sol_prize_pot_vault if not already initialized
    pub fn set_raydium_pool_state(
        ctx: Context<SetRaydiumPoolState>,
        raydium_pool_state: Pubkey,
    ) -> Result<()> {
        crate::log_fn!("lib", "set_raydium_pool_state");
        admin::set_raydium_pool_state_internal(ctx, raydium_pool_state)
    }

    /// Add a single faction to the global config (admin only)
    pub fn add_faction(
        ctx: Context<AddFaction>,
        faction_name: String,
        faction_id: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "add_faction");
        admin::add_faction_internal(ctx, faction_name, faction_id)
    }

    /// Initialize system referral account and buybacks system (admin only)
    pub fn initialize_system_accounts(ctx: Context<InitializeSystemAccounts>) -> Result<()> {
        crate::log_fn!("lib", "initialize_system_accounts");
        admin::initialize_system_accounts_internal(ctx)
    }

    /// Propose a new authority (2-step transfer). Only current authority can call.
    /// The proposed authority must call `accept_authority` to complete the transfer.
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_recipient: Option<Pubkey>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_config");
        admin::update_config_internal(ctx, new_authority, new_fee_recipient)
    }

    /// Cancel a pending authority transfer. Only current authority can call.
    pub fn cancel_authority_transfer(ctx: Context<UpdateConfigAc>) -> Result<()> {
        crate::log_fn!("lib", "cancel_authority_transfer");
        admin::cancel_authority_transfer_internal(ctx)
    }

    /// Accept a proposed authority transfer (step 2). Only the pending authority can call.
    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        crate::log_fn!("lib", "accept_authority");
        admin::accept_authority_internal(ctx)
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
        new_minebtc_jackpot_pct: Option<u8>,
        new_hodl_tax_pct: Option<u8>,
        snapshot_interval: Option<u64>,
        new_referral_fee_pct: Option<u8>,
        new_same_faction_referral_fee_pct: Option<u8>,
        new_cycle_sol_split_pct: Option<u8>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_fees");
        admin::update_fees_internal(
            ctx,
            new_protocol_fee_pct,
            new_buyback_pct,
            new_stakers_pct,
            new_minebtc_stakers_pct,
            new_minebtc_winners_pct,
            new_minebtc_same_faction_pct,
            new_minebtc_jackpot_pct,
            new_hodl_tax_pct,
            snapshot_interval,
            new_referral_fee_pct,
            new_same_faction_referral_fee_pct,
            new_cycle_sol_split_pct,
        )
    }

    /// Toggle RPG progression (story events, XP) during gameplay
    pub fn update_rpg_progression(ctx: Context<UpdateConfigAc>, enabled: bool) -> Result<()> {
        crate::log_fn!("lib", "update_rpg_progression");
        admin::update_rpg_progression_internal(ctx, enabled)
    }

    /// Authority-only kill switch. When paused, the contract blocks new bets
    /// (manual + autominer), new round starts, and doge mints/breeds. Round
    /// settlement, claims, staking, and economy cranks remain available so
    /// players can always exit and pending rounds always finish.
    pub fn set_pause(ctx: Context<UpdateConfigAc>, paused: bool) -> Result<()> {
        crate::log_fn!("lib", "set_pause");
        admin::set_pause_internal(ctx, paused)
    }

    /// Update the highest evolution stage unlocked by admin.
    /// `0` disables evolution entirely, `1` allows stage 0 -> 1, etc.
    pub fn update_evolution_unlock_stage(
        ctx: Context<UpdateConfigAc>,
        max_stage: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_evolution_unlock_stage");
        admin::update_evolution_unlock_stage_internal(ctx, max_stage)
    }

    /// Unified admin surface for gameplay tuning and cycle-reward pacing.
    pub fn update_gameplay_tuning(
        ctx: Context<UpdateConfigAc>,
        args: GameplayTuningUpdateArgs,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_gameplay_tuning");
        admin::update_gameplay_tuning_internal(ctx, args)
    }

    /// Update breeding configuration (admin only)
    pub fn update_breeding_config(
        ctx: Context<UpdateDogeConfig>,
        breeding_allowed: bool,
        breed_base_price: u64,
        breed_curve_a: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_breeding_config");
        admin::update_breeding_config_internal(
            ctx,
            breeding_allowed,
            breed_base_price,
            breed_curve_a,
        )
    }

    /// Update emission adjustment parameters (admin only)
    /// Allows updating price change threshold and emission increase/decrease percentages
    pub fn update_emission_params(
        ctx: Context<UpdateEmissionParams>,
        price_change_threshold: Option<u64>,
        emission_increase_pct: Option<u64>,
        emission_decrease_pct: Option<u64>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_emission_params");
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
        crate::log_fn!("lib", "initialize_mining");
        admin::initialize_mining_internal(ctx, start_timestamp, mine_btc_per_round, pool_state)
    }

    /// Deposit MineBtc tokens to the mining vault (anyone can call)
    ///
    /// Allows anyone to deposit MineBtc tokens into the mining vault.
    /// These tokens will be distributed as rewards to stakers over time.
    pub fn deposit_mine_btc_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        crate::log_fn!("lib", "deposit_mine_btc_tokens");
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
        crate::log_fn!("lib", "initialize_hashpower_config");
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
        crate::log_fn!("lib", "initialize_custodian_accounts");
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
        crate::log_fn!("lib", "update_hashpower_config");
        admin::update_hashpower_config_internal(
            ctx,
            min_lockup_days,
            max_lockup_days,
            base_multiplier,
            max_multiplier,
        )
    }

    // ----------------------------------------------------------------------------------------
    // ------------  DOGE SYSTEM (ADMIN) ------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize DogeConfig account (admin only)
    ///
    /// Creates the DogeConfig account that stores collection, supply, and breeding state.
    /// This must be called before creating the Doge collection.
    pub fn initialize_doge_config(
        ctx: Context<InitializeDogeConfig>,
        max_supply: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "initialize_doge_config");
        admin::initialize_doge_config_internal(ctx, max_supply)
    }

    /// Initialize mint-only Doge config for the genesis sale.
    pub fn initialize_doge_mint_config(
        ctx: Context<InitializeDogeMintConfig>,
        base_price: u64,
        curve_a: u64,
        genesis_mint_limit: u64,
        max_genesis_mints_per_faction: u16,
    ) -> Result<()> {
        crate::log_fn!("lib", "initialize_doge_mint_config");
        admin::initialize_doge_mint_config_internal(
            ctx,
            base_price,
            curve_a,
            genesis_mint_limit,
            max_genesis_mints_per_faction,
        )
    }

    /// Update DogeConfig account (admin only)
    ///
    /// Updates the DogeConfig account that stores collection, supply, and breeding state.
    ///
    /// # Parameters
    /// - `max_supply`: Optional lifetime Doge supply cap.
    pub fn update_doge_config(
        ctx: Context<UpdateDogeConfig>,
        max_supply: Option<u64>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_doge_config");
        admin::update_doge_config_internal(ctx, max_supply)
    }

    /// Update mint-only Doge config for genesis sale pricing and caps.
    pub fn update_doge_mint_config(
        ctx: Context<UpdateDogeMintConfig>,
        base_price: Option<u64>,
        curve_a: Option<u64>,
        genesis_mint_limit: Option<u64>,
        max_genesis_mints_per_faction: Option<u16>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_doge_mint_config");
        admin::update_doge_mint_config_internal(
            ctx,
            base_price,
            curve_a,
            genesis_mint_limit,
            max_genesis_mints_per_faction,
        )
    }

    /// Toggle Doge NFT minting on/off (admin only)
    ///
    /// Flips is_active between true and false.
    pub fn switch_doge_mining(ctx: Context<SwitchDogeMiningState>) -> Result<()> {
        crate::log_fn!("lib", "switch_doge_mining");
        admin::switch_doge_mining_internal(ctx)
    }

    /// Create Doge collection with program PDA as authority (admin only)
    ///
    /// Creates a new Metaplex Core collection for Doge NFTs.
    /// The collection's update authority is set to a program-controlled PDA,
    /// allowing the program to mint NFTs from the collection.
    /// Requires DogeConfig to be initialized first.
    pub fn create_doge_collection(
        ctx: Context<CreateDogeCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        crate::log_fn!("lib", "create_doge_collection");
        admin::create_doge_collection_internal(ctx, name, uri)
    }

    /// Initialize royalties on the Doge collection (admin only)
    pub fn init_doge_royalties(
        ctx: Context<InitDogeRoyalties>,
        basis_points: u16,
        creators: Vec<CreatorInput>,
    ) -> Result<()> {
        crate::log_fn!("lib", "init_doge_royalties");
        admin::init_doge_royalties_internal(ctx, basis_points, creators)
    }

    /// Add an UpdateDelegate to the collection (admin only)
    /// Allows delegate wallet to sign for marketplace verification without
    /// transferring the update authority (which would break minting)
    pub fn add_collection_delegate(
        ctx: Context<AddCollectionDelegate>,
        delegate: Pubkey,
    ) -> Result<()> {
        crate::log_fn!("lib", "add_collection_delegate");
        admin::add_collection_delegate_internal(ctx, delegate)
    }

    /// Update collection metadata — name and/or URI (admin only)
    /// Useful for fixing dead image URLs or updating collection info
    pub fn update_collection_info(
        ctx: Context<AddCollectionDelegate>,
        new_name: Option<String>,
        new_uri: Option<String>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_collection_info");
        admin::update_collection_info_internal(ctx, new_name, new_uri)
    }

    /// Add or update ticket tier configs (admin only)
    /// Max 3 ticket tier configs can be set
    pub fn add_ticket_tier_config(
        ctx: Context<UpdateDogeMintConfig>,
        ticket_tier_index: u8,
        ticket_value: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "add_ticket_tier_config");
        admin::add_ticket_tier_config_int(ctx, ticket_tier_index, ticket_value)
    }

    /// Set or update a user's free Doge mint allowance (admin only).
    /// The user still pays transaction fees and account rent, but not the mint price.
    pub fn set_doge_free_mint_allowance(
        ctx: Context<SetDogeFreeMintAllowance>,
        user: Pubkey,
        remaining_free_mints: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "set_doge_free_mint_allowance");
        admin::set_doge_free_mint_allowance_internal(ctx, user, remaining_free_mints)
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
        crate::log_fn!("lib", "initialize_tax_config");
        tax::internal_initialize_tax_config(
            ctx,
            nft_floor_sweep_pct,
            faction_treasury_pct,
            burn_pct,
            nft_floor_sweep_whitelisted_address,
        )
    }

    /// Update tax distribution percentages (admin only)
    pub fn update_tax_config(
        ctx: Context<UpdateTaxConfig>,
        nft_floor_sweep_pct: u8,
        faction_treasury_pct: u8,
        burn_pct: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_tax_config");
        tax::internal_update_tax_config(ctx, nft_floor_sweep_pct, faction_treasury_pct, burn_pct)
    }

    /// Update NFT floor sweep whitelisted address (admin only)
    pub fn update_nft_floor_sweep_whitelist(
        ctx: Context<UpdateNftFloorSweepWhitelist>,
        new_whitelisted_address: Pubkey,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_nft_floor_sweep_whitelist");
        tax::internal_update_nft_floor_sweep_whitelist(ctx, new_whitelisted_address)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ GAME STATE MANAGEMENT (ADMIN) ------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global game state (admin only)
    ///
    /// Sets up the GlobalGameState account that tracks game rounds, betting, and rewards.
    /// This must be called before any rounds can be started.
    pub fn initialize_game_state(
        ctx: Context<InitializeGameState>,
        round_duration_seconds: i64,
    ) -> Result<()> {
        crate::log_fn!("lib", "initialize_game_state");
        admin::initialize_game_state_internal(ctx, round_duration_seconds)
    }

    /// Update game state (admin only)
    ///
    /// Optionally pause/resume the game and/or change round duration.
    pub fn update_game_state(
        ctx: Context<UpdateGameState>,
        is_active: Option<bool>,
        round_duration_seconds: Option<i64>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_game_state");
        admin::update_game_state_internal(ctx, is_active, round_duration_seconds)
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
        crate::log_fn!("lib", "distribute_sol_fees");
        economy::distribute_sol_fees_internal(ctx)
    }

    /// INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)
    /// Performs a small SOL → MINE_BTC swap for price discovery and earnmarks SOL for POL
    /// After 8 snapshots over 4 hours, call update_rate then add_lp_and_burn to finalize
    pub fn snapshot_price(ctx: Context<SnapshotPrice>) -> Result<()> {
        crate::log_fn!("lib", "snapshot_price");
        economy::snapshot_price_internal(ctx)
    }

    /// INSTRUCTION 2a: Update distribution rate (can be called after 4 hours)
    /// Checks if 8 snapshots collected, updates distribution rate, sets flag for LP operation
    pub fn update_rate(ctx: Context<UpdateRate>) -> Result<()> {
        crate::log_fn!("lib", "update_rate");
        economy::update_rate_internal(ctx)
    }

    /// INSTRUCTION 2b: Add liquidity and burn LP tokens (called after update_rate)
    /// When lp_token_amount > 0: Admin override mode (requires authority signature)
    /// When lp_token_amount = 0: Automatic calculation mode (anyone can call)
    pub fn add_lp_and_burn(ctx: Context<AddLpAndBurn>, lp_token_amount: u64) -> Result<()> {
        crate::log_fn!("lib", "add_lp_and_burn");
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
        crate::log_fn!("lib", "withdraw_nft_floor_sweep_funds");
        tax::internal_withdraw_nft_floor_sweep_funds(ctx, amount)
    }

    /// STEP 1: Harvest fees from user token accounts to the mint
    /// Callable by anyone - keeper bot should call this in batches
    pub fn crank_harvest_fees<'info>(
        ctx: Context<'_, '_, '_, 'info, CrankHarvestFees<'info>>,
    ) -> Result<()> {
        crate::log_fn!("lib", "crank_harvest_fees");
        tax::internal_crank_harvest_fees(ctx)
    }

    /// STEP 2: Withdraw total tax from mint and distribute it
    /// Callable by anyone - program-controlled withdraw authority
    pub fn crank_distribute_tax(
        ctx: Context<CrankDistributeTax>,
        faction_war_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "crank_distribute_tax");
        let bumps = ctx.bumps;
        let accounts = ctx.accounts;
        tax::internal_crank_distribute_tax(
            accounts,
            faction_war_id,
            bumps.faction_war_state,
            bumps.withdraw_withheld_authority,
        )
    }

    /// Claim faction treasury rewards for a settled faction_war.
    /// Uses the story-event leaderboard (faction_war final_ranks) -- permissionless.
    pub fn claim_faction_treasury_for_faction_war(
        ctx: Context<ClaimFactionTreasuryForFactionWar>,
        faction_war_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "claim_faction_treasury_for_faction_war");
        tax::internal_claim_faction_treasury_for_faction_war(ctx, faction_war_id)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ FACTION_WAR MINING SYSTEM -------------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize faction_war configuration (admin only).
    /// FactionWar duration is tied to the economy cycle -- one faction_war per LP burn.
    pub fn initialize_faction_war_config(ctx: Context<InitializeFactionWarConfig>) -> Result<()> {
        crate::log_fn!("lib", "initialize_faction_war_config");
        faction_war::initialize_faction_war_config_internal(ctx)
    }

    /// Update faction_war configuration (admin only)
    pub fn update_faction_war_config(
        ctx: Context<UpdateFactionWarConfig>,
        is_active: Option<bool>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_faction_war_config");
        faction_war::update_faction_war_config_internal(ctx, is_active)
    }

    /// Settle faction_war: finalize story-event-based rankings and compute reward pools.
    /// Permissionless -- anyone can call once the economy cycle's LP burn has completed.
    pub fn settle_faction_war(ctx: Context<SettleFactionWar>) -> Result<()> {
        crate::log_fn!("lib", "settle_faction_war");
        faction_war::settle_faction_war_internal(ctx)
    }

    /// User claims their faction-war rewards (closes user_faction_war_bets account).
    pub fn claim_faction_war_rewards(
        ctx: Context<ClaimFactionWarRewards>,
        faction_war_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "claim_faction_war_rewards");
        faction_war::claim_faction_war_rewards_internal(ctx, faction_war_id)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ GAME ROUND MANAGEMENT (SLOT-HASH RANDOMNESS) ------------------------
    // ----------------------------------------------------------------------------------------

    /// Start a new round and initialize its GameSession.
    /// round_id should be current_round_id + 1 (validated in the function)
    pub fn start_round(ctx: Context<StartRound>, round_id: u64) -> Result<()> {
        crate::log_fn!("lib", "start_round");
        game::int_start_round(ctx, round_id)
    }

    /// Finalize the current round using scheduled slot-hash entropy.
    pub fn end_round(ctx: Context<EndRound>) -> Result<()> {
        crate::log_fn!("lib", "end_round");
        game::int_end_round(ctx)
    }

    /// Finalize the round's faction-level staking/jackpot distribution and track faction-war mining.
    pub fn end_round_faction_rewards(
        ctx: Context<EndRoundFactionRewards>,
        faction_war_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "end_round_faction_rewards");
        let bumps = ctx.bumps;
        let accounts = ctx.accounts;
        game::int_end_round_faction_rewards(accounts, faction_war_id, bumps.faction_war_state)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ PLAYER & BETTING FUNCTIONS ------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize a player account for the MineBTC country arena
    pub fn initialize_player(
        ctx: Context<InitializePlayer>,
        faction_id: u8,
        referral_code: Option<Pubkey>,
    ) -> Result<()> {
        crate::log_fn!("lib", "initialize_player");
        user::internal_initialize_player(ctx, faction_id, referral_code)
    }

    /// Join a round by placing one or more faction-direction bets.
    pub fn join_bets(
        ctx: Context<JoinBets>,
        round_id: u64,
        faction_war_id: u64,
        bet_types: Vec<BetType>,
        amount_per_bet: u64,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        crate::log_fn!("lib", "join_bets");
        let bumps = ctx.bumps;
        let accounts = ctx.accounts;
        user::internal_join_bets(
            accounts,
            round_id,
            faction_war_id,
            bet_types,
            amount_per_bet,
            use_ticket,
            bumps.user_game_bet,
            bumps.faction_war_state,
            bumps.user_faction_war_bets,
        )
    }

    /// Initialize autominer vault with flexible faction-direction configuration
    /// use_ticket: Optional ticket tier index. If Some, autominer uses tickets instead of SOL for bets.
    pub fn init_autominer(
        ctx: Context<InitAutominer>,
        factions_config: Option<FactionsConfig>,
        sol_per_round: u64,
        num_rounds: u32,
        can_reload: bool,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        crate::log_fn!("lib", "init_autominer");
        user::internal_init_autominer(
            ctx,
            factions_config,
            sol_per_round,
            num_rounds,
            can_reload,
            use_ticket,
        )
    }

    /// Execute autominer bet (keeper instruction)
    pub fn execute_autominer_bet(
        ctx: Context<ExecuteAutominerBet>,
        current_round_id: u64,
        faction_war_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "execute_autominer_bet");
        let bumps = ctx.bumps;
        let accounts = ctx.accounts;
        user::internal_execute_autominer_bet(
            accounts,
            current_round_id,
            faction_war_id,
            bumps.user_game_bet,
            bumps.faction_war_state,
            bumps.user_faction_war_bets,
            bumps.autominer_custody,
        )
    }

    /// Update autominer configuration (sol_per_round, num_rounds, can_reload)
    pub fn update_autominer(
        ctx: Context<UpdateAutominer>,
        sol_per_round: Option<u64>,
        num_rounds: Option<u32>,
        can_reload: Option<bool>,
    ) -> Result<()> {
        crate::log_fn!("lib", "update_autominer");
        user::internal_update_autominer(ctx, sol_per_round, num_rounds, can_reload)
    }

    /// Stop autominer and refund remaining SOL
    pub fn stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
        crate::log_fn!("lib", "stop_autominer");
        user::internal_stop_autominer(ctx)
    }

    /// Claim rewards for a user after round ends
    pub fn claim_round_rewards(ctx: Context<ClaimRoundRewards>, round_id: u64) -> Result<()> {
        crate::log_fn!("lib", "claim_round_rewards");
        user::internal_claim_round_rewards(round_id, ctx)
    }

    /// Claim autominer rewards with auto-reload (keeper instruction)
    /// Uses SOL rewards to add more rounds to autominer, leftover SOL goes to owner
    pub fn claim_autominer_rewards(
        ctx: Context<ClaimAutominerRewards>,
        round_id: u64,
    ) -> Result<()> {
        crate::log_fn!("lib", "claim_autominer_rewards");
        user::internal_claim_autominer_rewards(round_id, ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------ USER INSTRUCTIONS :: GAMEPLAY DOGES  ------------
    // ----------------------------------------------------------------------------------------

    /// Use an doge for gameplay - deposits to custody and sets as active gameplay doge
    pub fn use_doge_for_gameplay(ctx: Context<UseDogeForGameplay>) -> Result<()> {
        crate::log_fn!("lib", "use_doge_for_gameplay");
        user::internal_use_doge_for_gameplay(ctx)
    }

    /// Request gameplay doge unlock. Actual withdrawal is only available in the next faction_war cycle.
    pub fn request_doge_gameplay_unlock(ctx: Context<RequestDogeGameplayUnlock>) -> Result<()> {
        crate::log_fn!("lib", "request_doge_gameplay_unlock");
        user::internal_request_doge_gameplay_unlock(ctx)
    }

    /// Withdraw doge from gameplay - returns doge to user
    pub fn withdraw_doge_from_gameplay(ctx: Context<WithdrawDogeFromGameplay>) -> Result<()> {
        crate::log_fn!("lib", "withdraw_doge_from_gameplay");
        user::internal_withdraw_doge_from_gameplay(ctx)
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
        crate::log_fn!("lib", "stake_minebtc");
        stake::int_stake_minebtc(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake MineBtc tokens from a position
    pub fn unstake_minebtc(ctx: Context<UnstakeMineBtc>, position_index: u8) -> Result<()> {
        crate::log_fn!("lib", "unstake_minebtc");
        stake::int_unstake_minebtc(ctx, position_index)
    }

    /// Stake LP tokens to earn SOL and minebtc rewards
    pub fn stake_lp_tokens(
        ctx: Context<StakeLpTokens>,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "stake_lp_tokens");
        stake::int_stake_lp_tokens(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake LP tokens from a position
    pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
        crate::log_fn!("lib", "unstake_lp_tokens");
        stake::int_unstake_lp_tokens(ctx, position_index)
    }

    /// Claim staking rewards: transfers SOL directly, accumulates MineBTC to pending
    pub fn claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
        crate::log_fn!("lib", "claim_staking_rewards");
        stake::int_claim_staking_rewards(ctx)
    }

    /// Withdraw accumulated MineBTC rewards (with HODL tax redistribution)
    pub fn withdraw_dbtc_rewards(ctx: Context<WithdrawDbtcRewards>) -> Result<()> {
        crate::log_fn!("lib", "withdraw_dbtc_rewards");
        stake::int_withdraw_dbtc_rewards(ctx)
    }

    /// Claim referral rewards (SOL and MineBtc earned from referrals)
    pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
        crate::log_fn!("lib", "claim_referral_rewards");
        stake::int_claim_referral_rewards(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------  DOGE NFT FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------

    ///Simulate mint costs for multiple doges accounting for bonding curve pricing
    ///
    /// # Parameters
    /// - `doge_config`: DogeConfig account
    /// - `doge_mint_config`: DogeMintConfig account
    /// - `mint_count`: Number of doges to mint
    pub fn simulate_purchase_cost(
        ctx: Context<SimulateMintCost>,
        mint_count: u64,
    ) -> Result<(u64, Vec<u64>, Vec<(u64, u64)>)> {
        crate::log_fn!("lib", "simulate_purchase_cost");
        doges::int_simulate_mint_cost(
            &ctx.accounts.doge_config,
            &ctx.accounts.doge_mint_config,
            mint_count,
        )
    }

    /// Admin function to mint a Doge NFT for free to a specified recipient (admin only)
    ///
    /// Allows the admin to mint a Doge NFT without payment.
    /// The NFT is minted directly to the specified recipient address.
    ///
    /// # Parameters
    /// - `recipient`: Address that will receive the minted NFT
    /// - `faction_id`: Faction ID the doge belongs to
    pub fn admin_mint_doge(
        ctx: Context<AdminMintDoge>,
        recipient: Pubkey,
        faction_id: u8,
        ticket_tier_index: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "admin_mint_doge");
        doges::int_admin_mint_doge(ctx, recipient, faction_id, ticket_tier_index)
    }

    /// Mint a single Doge for free using a per-user whitelist allowance.
    /// The caller pays transaction fees and rent, but no Doge mint price.
    pub fn whitelist_mint_doge(
        ctx: Context<WhitelistMintDoge>,
        faction_id: u8,
        ticket_tier_index: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "whitelist_mint_doge");
        doges::int_whitelist_mint_doge(ctx, faction_id, ticket_tier_index)
    }

    /// Batch mint multiple Doge (anyone can call, max 10 per transaction)
    ///
    /// Mints multiple Doge NFTs in a single transaction.
    /// Each doge uses bonding curve pricing based on the current supply at mint time.
    ///
    /// # Parameters
    /// - `faction_id`: Faction ID all doges belong to
    /// - `mint_count`: Number of doges to mint (1-10)
    /// - `ticket_tier_index`: Ticket tier index (0-2)
    pub fn batch_mint_doges<'info>(
        ctx: Context<'_, '_, '_, 'info, BatchMintDoge<'info>>,
        faction_id: u8,
        mint_count: u8,
        ticket_tier_index: u8,
    ) -> Result<()> {
        crate::log_fn!("lib", "batch_mint_doges");
        doges::int_batch_mint_doges(ctx, faction_id, mint_count, ticket_tier_index)
    }

    /// Stake a Doge to boost hashpower (if faction matches player's faction)
    pub fn stake_doge(ctx: Context<StakeDoge>) -> Result<()> {
        crate::log_fn!("lib", "stake_doge");
        doges::int_stake_doge(ctx)
    }

    /// Unstake a Doge (remove hashpower boost)
    pub fn unstake_doge(ctx: Context<UnstakeDoge>) -> Result<()> {
        crate::log_fn!("lib", "unstake_doge");
        doges::int_unstake_doge(ctx)
    }

    /// Breed two doges to create offspring
    pub fn breed_doges(ctx: Context<BreedDoge>) -> Result<()> {
        crate::log_fn!("lib", "breed_doges");
        doges::int_breed_doges(ctx)
    }

    /// Send an doge to heaven (burn for rewards)
    pub fn send_to_heaven(ctx: Context<SendToHeaven>) -> Result<()> {
        crate::log_fn!("lib", "send_to_heaven");
        doges::int_send_to_heaven(ctx)
    }

    // ----------------------------------------------------------------------------------------
    // ------------  QUERY FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct GeneBreakdown {
        pub dna: [u8; 32],
        pub family_type: u8,
        pub evolution_stage: u8,
        pub appearance_traits: Vec<u8>,
        pub dominant_appearance_traits: Vec<u8>,
        pub power_traits: Vec<u8>,
        pub dominant_power_traits: Vec<u8>,
    }

    #[derive(Accounts)]
    pub struct GetGeneBreakdown<'info> {
        /// System program (required by Anchor but not used)
        pub system_program: Program<'info, System>,
    }

    /// Query function to decode DNA and return gene breakdown
    /// This is a read-only function that can be called via simulateTransaction
    pub fn get_gene_breakdown(_ctx: Context<GetGeneBreakdown>, dna: [u8; 32]) -> Result<()> {
        crate::log_fn!("lib", "get_gene_breakdown");
        let family_type = genescience::get_family_type(&dna);
        let evolution_stage = genescience::get_evolution_stage(&dna);
        let appearance_traits = genescience::decode_appearance_traits(&dna);
        let dominant_appearance_traits = genescience::decode_dominant_appearance_traits(&dna);
        let power_traits = genescience::decode_power_traits(&dna);
        let dominant_power_traits = genescience::decode_dominant_power_traits(&dna);

        let breakdown = GeneBreakdown {
            dna,
            family_type,
            evolution_stage,
            appearance_traits,
            dominant_appearance_traits,
            power_traits,
            dominant_power_traits,
        };

        let serialized = breakdown
            .try_to_vec()
            .map_err(|_| crate::errors::ErrorCode::InvalidAccount)?;

        set_return_data(&serialized);

        Ok(())
    }
}
