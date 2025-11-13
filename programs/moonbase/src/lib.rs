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
pub use instructions::stake::*;
pub use instructions::game::*;
pub use instructions::eggs::*;
pub use instructions::tax::*;
pub use state::{SolFeeConfig, DogeBtcDistConfig, BetType, EggConfig, TicketTier, TaxConfig};
pub use instructions::eggs::CreatorInput;

declare_id!("9xwvYvnjA3TVRpPVUonvDQcchTxgo7dRi7zwi2zvoSAG");

#[program]
pub mod moonbase {
    use super::*;
    use instructions::admin::{self};
    use instructions::economy::{self};
    use instructions::user::{self};
    use instructions::stake::{self};
    use instructions::game::{self};
    use instructions::eggs::{self};
    use instructions::tax::{self};

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
    pub fn set_raydium_pool_state(
        ctx: Context<UpdateConfigAc>,
        raydium_pool_state: Pubkey,
    ) -> Result<()> {
        admin::set_raydium_pool_state_internal(ctx, raydium_pool_state)
    }

    /// Set Dragon Egg URIs for all factions (admin only)
    /// uris: Vec of URIs, one per faction (must match number of factions)
    pub fn set_dragon_egg_uris(
        ctx: Context<UpdateEggsConfig>,
        uris: Vec<String>,
    ) -> Result<()> {
        admin::set_dragon_egg_uris_internal(ctx, uris)
    }

    /// Clear all Dragon Egg URIs (admin only)
    pub fn clear_dragon_egg_uris(ctx: Context<UpdateEggsConfig>) -> Result<()> {
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


    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG ROYALTY MANAGEMENT (ADMIN) ------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize royalties on the Dragon Egg collection (admin only)
    pub fn init_dragon_egg_royalties(
        ctx: Context<InitDragonEggRoyalties>,
        basis_points: u16,
        creators: Vec<CreatorInput>,
    ) -> Result<()> {
        eggs::init_dragon_egg_royalties(ctx, basis_points, creators)
    }

    /// Add or update ticket tier configs (admin only)
    /// Max 4 ticket tier configs can be set
    pub fn add_ticket_tier_config(
        ctx: Context<UpdateEggsConfig>,
        ticket_tier_index: u8,
        ticket_value: u64,
        ticket_count: u16,
    ) -> Result<()> {
        eggs::add_ticket_tier_config(ctx, ticket_tier_index, ticket_value, ticket_count)
    }

    /// Admin function to mint a Dragon Egg NFT for free to a specified recipient (admin only)
    pub fn admin_mint_dragon_egg(
        ctx: Context<AdminMintDragonEgg>,
        recipient: Pubkey,
        faction_id: u8,
    ) -> Result<()> {
        eggs::admin_mint_dragon_egg(ctx, recipient, faction_id)
    }




















    /// Update the global configuration parameters
    /// Can only be called by the current authority
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_recipient: Option<Pubkey>,
    ) -> Result<()> {
        admin::update_config_internal(
            ctx,
            new_authority,
            new_fee_recipient,
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


    /// Initialize system referral account and buybacks system (admin only)
    pub fn initialize_system_accounts(ctx: Context<InitializeSystemAccounts>) -> Result<()> {
        admin::initialize_system_accounts_internal(ctx)
    }

    /// Initialize TaxConfig account and create vault token accounts (admin only)
    pub fn initialize_tax_config(
        ctx: Context<InitializeTaxConfig>,
        nft_floor_sweep_pct: u8,
        faction_treasury_pct: u8,
        nft_floor_sweep_whitelisted_address: Pubkey,
    ) -> Result<()> {
        tax::initialize_tax_config(ctx, nft_floor_sweep_pct, faction_treasury_pct, nft_floor_sweep_whitelisted_address)
    }

    /// Update tax distribution percentages (admin only)
    pub fn update_tax_config(
        ctx: Context<UpdateTaxConfig>,
        nft_floor_sweep_pct: u8,
        faction_treasury_pct: u8,
    ) -> Result<()> {
        tax::update_tax_config(ctx, nft_floor_sweep_pct, faction_treasury_pct)
    }

    /// Update NFT floor sweep whitelisted address (admin only)
    pub fn update_nft_floor_sweep_whitelist(
        ctx: Context<UpdateNftFloorSweepWhitelist>,
        new_whitelisted_address: Pubkey,
    ) -> Result<()> {
        tax::update_nft_floor_sweep_whitelist(ctx, new_whitelisted_address)
    }
 
    // ----------------------------------------------------------
    // ------------ WITHDRAW SOL FEES (ANYONE) ------------------
    // ----------------------------------------------------------



    /// Withdraw DogeBtc from NFT floor sweep vault (whitelisted address only)
    pub fn withdraw_nft_floor_sweep_funds(
        ctx: Context<WithdrawNftFloorSweepFunds>,
        amount: u64,
    ) -> Result<()> {
        tax::withdraw_nft_floor_sweep_funds(ctx, amount)
    }



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
    // ------------ TAX SYSTEM FUNCTIONS ------------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// STEP 1: Harvest fees from user token accounts to the mint
    /// Callable by anyone - keeper bot should call this in batches
    pub fn crank_harvest_fees<'info>(ctx: Context<'_, '_, '_, 'info, CrankHarvestFees<'info>>) -> Result<()> {
        tax::crank_harvest_fees(ctx)
    }

    /// STEP 2: Withdraw total tax from mint and distribute it
    /// Callable by anyone - program-controlled withdraw authority
    pub fn crank_distribute_tax(ctx: Context<CrankDistributeTax>) -> Result<()> {
        tax::crank_distribute_tax(ctx)
    }

    /// Start a new distribution round (callable by anyone after 7-day cooldown)
    pub fn start_distribution_round(ctx: Context<StartDistributionRound>) -> Result<()> {
        tax::start_distribution_round(ctx)
    }

    /// Calculate leaderboard position for one faction
    /// Must be called 12 times to build complete leaderboard
    pub fn calculate_faction_leaderboard_position(ctx: Context<CalculateFactionLeaderboard>) -> Result<()> {
        tax::calculate_faction_leaderboard_position(ctx)
    }

    /// Calculate rewards for all factions based on leaderboard
    /// Can only be called after all 12 factions are on leaderboard
    pub fn calculate_faction_rewards(ctx: Context<CalculateFactionRewards>) -> Result<()> {
        tax::calculate_faction_rewards(ctx)
    }

    /// Claim treasury rewards for one faction
    /// Adds rewards to staking reward indexes (50% each to dbtc and lp stakers)
    pub fn claim_faction_treasury_rewards(ctx: Context<ClaimFactionTreasuryRewards>) -> Result<()> {
        tax::claim_faction_treasury_rewards(ctx)
    }

    /// Finish distribution round (check all factions claimed and reset state)
    pub fn finish_distribution_round(ctx: Context<FinishDistributionRound>) -> Result<()> {
        tax::finish_distribution_round(ctx)
    }



    // ----------------------------------------------------------------------------------------
    // ------------ GAME ROUND MANAGEMENT (COMMIT-REVEAL RANDOMNESS) ------------------------
    // ----------------------------------------------------------------------------------------

    /// Start a new round by committing a hash and initializing GameSession
    /// This commits randomness hash and randomly assigns factions to blocks
    /// round_id should be current_round_id + 1 (validated in the function)
    /// If commit_hash is None, uses next_round_commit from global_state
    pub fn start_round(
        ctx: Context<StartRound>,
        round_id: u64,
        commit_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        game::start_round(ctx, round_id, commit_hash)
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



    // ----------------------------------------------------------------------------------------
    // ------------ FACTION SURGE RAFFLE FUNCTIONS -------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize a player account for the Faction Surge game
    pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
        user::initialize_player(ctx, faction_id, referral_code)
    }

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

    /// Mint a single Dragon Egg NFT
    /// Uses bonding curve pricing based on current supply
    /// Users can optionally choose a ticket tier to receive free tickets
    pub fn mint_dragon_egg(
        ctx: Context<MintDragonEgg>,
        faction_id: u8,
        ticket_tier_index: Option<u8>,
    ) -> Result<()> {
        eggs::mint_dragon_egg(ctx, faction_id, ticket_tier_index)
    }

    /// Batch mint multiple Dragon Eggs (max 10 per transaction)
    /// Uses bonding curve pricing for each egg
    pub fn batch_mint_dragon_eggs(
        ctx: Context<BatchMintDragonEggs>,
        faction_id: u8,
        mint_count: u8,
    ) -> Result<()> {
        eggs::batch_mint_dragon_eggs(ctx, faction_id, mint_count)
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
