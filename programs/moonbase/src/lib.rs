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
pub use state::{SolFeeConfig, DogeBtcDistConfig, BetType, EggConfig, TicketTier, TaxConfig, BlocksConfig, FactionsConfig, FactionStrategy};
pub use instructions::admin::CreatorInput;

declare_id!("bKuXBWKQGB8nQWM9jsfPGP3BA3yDFK2jao4FmeDrvvs");

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
    /// Also initializes sol_rewards_vault and sol_prize_pot_vault if not already initialized
    pub fn set_raydium_pool_state(
        ctx: Context<SetRaydiumPoolState>,
        raydium_pool_state: Pubkey,
    ) -> Result<()> {
        admin::set_raydium_pool_state_internal(ctx, raydium_pool_state)
    }

    /// Add a single faction to the global config (admin only)
    pub fn add_faction(ctx: Context<AddFaction>, faction_name: String, faction_id: u8) -> Result<()> {
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
        change_faction_fee: Option<u64>,
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
            change_faction_fee,
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
        doge_btc_per_round: u64,
        pool_state: Pubkey,
    ) -> Result<()> {
        admin::initialize_mining_internal(ctx, start_timestamp, doge_btc_per_round, pool_state)
    }

    /// Deposit DogeBtc tokens to the mining vault (anyone can call)
    /// 
    /// Allows anyone to deposit DogeBtc tokens into the mining vault.
    /// These tokens will be distributed as rewards to stakers over time.
    pub fn deposit_doge_btc_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
        admin::deposit_doge_btc_tokens_internal(ctx, amount)
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
        admin::initialize_hashpower_config_internal(ctx, min_lockup_days, max_lockup_days, base_multiplier, max_multiplier)
    }


    /// Initialize both custodian token accounts (admin only)
    /// Initializes:
    /// - DBTC custodian: Token-2022 account that holds all staked DOGE_BTC tokens (global for all factions)
    /// - Liquidity custodian: Standard SPL Token account that holds all staked LP tokens (global for all factions)
    pub fn initialize_custodian_accounts(ctx: Context<InitializeCustodianAccounts>) -> Result<()> {
        admin::initialize_custodian_accounts(ctx)
    }

    /// Update HashpowerConfig account (admin only)
    pub fn update_hashpower_config(
        ctx: Context<UpdateHashpowerConfig>,
        min_lockup_days: u64,
        max_lockup_days: u64,
        base_multiplier: u16,
        max_multiplier: u16,
    ) -> Result<()> {
        admin::update_hashpower_config_internal(ctx, min_lockup_days, max_lockup_days, base_multiplier, max_multiplier)
    }


    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG SYSTEM (ADMIN) ------------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize EggConfig account (admin only)
    /// 
    /// Creates the EggConfig account that stores Dragon Egg configuration.
    /// This must be called before creating the Dragon Egg collection.
    pub fn initialize_egg_config(
        ctx: Context<InitializeEggConfig>,
        base_price: u64,
        curve_a: u64,
        max_supply: u64,
    ) -> Result<()> {
        admin::initialize_egg_config_internal(ctx, base_price, curve_a, max_supply)
    }

    /// Create Dragon Egg collection with program PDA as authority (admin only)
    /// 
    /// Creates a new Metaplex Core collection for Dragon Egg NFTs.
    /// The collection's update authority is set to a program-controlled PDA,
    /// allowing the program to mint NFTs from the collection.
    /// Requires EggConfig to be initialized first.
    pub fn create_dragon_egg_collection(
        ctx: Context<CreateDragonEggCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        admin::create_dragon_egg_collection_internal(ctx, name, uri)
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

    /// Initialize royalties on the Dragon Egg collection (admin only)
    pub fn init_dragon_egg_royalties(
        ctx: Context<InitDragonEggRoyalties>,
        basis_points: u16,
        creators: Vec<CreatorInput>,
    ) -> Result<()> {
        admin::init_dragon_egg_royalties(ctx, basis_points, creators)
    }

    /// Add or update ticket tier configs (admin only)
    /// Max 4 ticket tier configs can be set
    pub fn add_ticket_tier_config(
        ctx: Context<UpdateEggsConfig>,
        ticket_tier_index: u8,
        ticket_value: u64,
        ticket_count: u16,
    ) -> Result<()> {
        admin::add_ticket_tier_config(ctx, ticket_tier_index, ticket_value, ticket_count)
    }


    // ----------------------------------------------------------------------------------------
    // ------------ TAX SYSTEM (ADMIN) :: INITIALIZATION & UPDATES ------------
    // ----------------------------------------------------------------------------------------

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

    /// Switch game state (toggle is_active) (admin only)
    /// 
    /// Toggles the game's active state. When paused, rounds cannot be started or ended.
    pub fn switch_game_state(
        ctx: Context<UpdateGameState>,
    ) -> Result<()> {
        admin::switch_game_state_internal(ctx)
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
    // ------------ TAX SYSTEM FUNCTIONS ------------------------------------------------------
    // ----------------------------------------------------------------------------------------
 
    /// Withdraw DogeBtc from NFT floor sweep vault (whitelisted address only)
    pub fn withdraw_nft_floor_sweep_funds(
        ctx: Context<WithdrawNftFloorSweepFunds>,
        amount: u64,
    ) -> Result<()> {
        tax::withdraw_nft_floor_sweep_funds(ctx, amount)
    }

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
    pub fn start_round(
        ctx: Context<StartRound>,
        round_id: u64,
        commit: [u8; 32],
    ) -> Result<()> {
        game::start_round(ctx, round_id, commit)
    }

    /// End the current round by revealing seed, selecting winner, and starting next round
    /// Verifies commit-reveal, generates final randomness, selects winning block
    pub fn end_round(
        ctx: Context<EndRound>,
        revealed_seed: [u8; 32],
    ) -> Result<()> {
        game::end_round(ctx, revealed_seed)
    }
    
    /// End the current round by revealing seed, selecting winner, and starting next round
    /// Verifies commit-reveal, generates final randomness, selects winning block
    pub fn end_round_faction_rewards(
        ctx: Context<EndRoundFactionRewards>
    ) -> Result<()> {
        game::end_round_faction_rewards(ctx)
    }



    // ----------------------------------------------------------------------------------------
    // ------------ FACTION SURGE RAFFLE FUNCTIONS -------------------------------------------
    // ----------------------------------------------------------------------------------------

    /// Initialize a player account for the Faction Surge game
    pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
        user::initialize_player(ctx, faction_id, referral_code)
    }

    /// Change user's faction
    /// Requires no staked positions (dbtc/lp hashpower = 0, no eggs staked)
    /// Charges change_faction_fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
    pub fn change_faction(ctx: Context<ChangeFaction>, new_faction_id: u8) -> Result<()> {
        user::change_faction(ctx, new_faction_id)
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

    /// Join a round with multiple bets in a single transaction
    pub fn join_round_batch(
        ctx: Context<JoinRoundBatch>,
        bet_types: Vec<BetType>,
        amount_per_bet: u64,
        use_ticket: Option<u8>,
    ) -> Result<()> {
        user::join_round_batch(ctx, bet_types, amount_per_bet, use_ticket)
    }

    /// Initialize autominer vault with flexible block/faction configuration
    pub fn init_autominer(
        ctx: Context<InitAutominer>,
        blocks_config: Option<BlocksConfig>,
        factions_config: Option<FactionsConfig>,
        sol_per_round: u64,
        num_rounds: u32,
    ) -> Result<()> {
        user::init_autominer(ctx, blocks_config, factions_config, sol_per_round, num_rounds)
    }

    /// Execute autominer bet (keeper instruction)
    pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
        user::execute_autominer_bet(ctx)
    }

    /// Stop autominer and refund remaining SOL
    pub fn stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
        user::stop_autominer(ctx)
    }


    /// Claim rewards for a user after round ends
    pub fn claim_round_rewards(ctx: Context<ClaimRoundRewards>, round_id: u64) -> Result<()> {
        user::internal_claim_round_rewards(round_id, ctx)
    }

 
    // ----------------------------------------------------------------------------------------
    // ------------ USER INSTRUCTIONS :: STAKE & UNSTAKE MOONDOGE / LP TOKENs  ------------
    // ----------------------------------------------------------------------------------------

    /// Stake DogeBtc tokens to earn SOL and dbtc rewards
    pub fn stake_moondoge(
        ctx: Context<StakeDogeBtc>,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::stake_moondoge(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake DogeBtc tokens from a position
    pub fn unstake_moondoge(ctx: Context<UnstakeDogeBtc>, position_index: u8) -> Result<()> {
        stake::unstake_moondoge(ctx, position_index)
    }

    /// Stake LP tokens to earn SOL and dbtc rewards
    pub fn stake_lp_tokens(
        ctx: Context<StakeLpTokens>,
        amount: u64,
        lockup_duration: u64,
        position_index: u8,
    ) -> Result<()> {
        stake::stake_lp_tokens(ctx, amount, lockup_duration, position_index)
    }

    /// Unstake LP tokens from a position
    pub fn unstake_lp_tokens(ctx: Context<UnstakeLpTokens>, position_index: u8) -> Result<()> {
        stake::unstake_lp_tokens(ctx, position_index)
    }
    
    // /// Claim SOL rewards from DogeBtc and LP staking
    // pub fn claim_sol_rewards(ctx: Context<ClaimSolRewards>) -> Result<()> {
    //     stake::claim_sol_rewards(ctx)
    // }
    
    // /// Claim DogeBtc token rewards from staking (with refining fee redistribution)
    // pub fn claim_dbtc_rewards(ctx: Context<ClaimDbtcRewards>) -> Result<()> {
    //     stake::claim_dbtc_rewards(ctx)
    // }
    
    // /// Claim referral rewards (SOL and DogeBtc earned from referrals)
    // pub fn claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    //     stake::claim_referral_rewards(ctx)
    // }

    // ----------------------------------------------------------------------------------------
    // ------------ DRAGON EGG NFT FUNCTIONS -------------------------------------------------
    // ----------------------------------------------------------------------------------------


    /// Admin function to mint a Dragon Egg NFT for free to a specified recipient (admin only)
    /// 
    /// Allows the admin to mint a Dragon Egg NFT without payment.
    /// The NFT is minted directly to the specified recipient address.
    /// 
    /// # Parameters
    /// - `recipient`: Address that will receive the minted NFT
    /// - `faction_id`: Faction ID the egg belongs to
    pub fn admin_mint_dragon_egg(
        ctx: Context<AdminMintDragonEgg>,
        recipient: Pubkey,
        faction_id: u8,
    ) -> Result<()> {
        eggs::admin_mint_dragon_egg(ctx, recipient, faction_id)
    }


    /// Mint a single Dragon Egg NFT (anyone can call)
    /// 
    /// Mints a Dragon Egg NFT using bonding curve pricing based on current supply.
    /// Users can optionally select a ticket tier to receive free tickets when minting.
    /// 
    /// # Parameters
    /// - `faction_id`: Faction ID the egg belongs to
    /// - `ticket_tier_index`: Optional ticket tier index (0-3) to receive free tickets
    pub fn mint_dragon_egg(
        ctx: Context<MintDragonEgg>,
        faction_id: u8,
        ticket_tier_index: Option<u8>,
    ) -> Result<()> {
        eggs::mint_dragon_egg(ctx, faction_id, ticket_tier_index)
    }

    /// Batch mint multiple Dragon Eggs (anyone can call, max 10 per transaction)
    /// 
    /// Mints multiple Dragon Egg NFTs in a single transaction.
    /// Each egg uses bonding curve pricing based on the current supply at mint time.
    /// 
    /// # Parameters
    /// - `faction_id`: Faction ID all eggs belong to
    /// - `mint_count`: Number of eggs to mint (1-10)
    pub fn batch_mint_dragon_eggs(
        ctx: Context<BatchMintDragonEggs>,
        faction_id: u8,
        mint_count: u8,
        ticket_tier_index: Option<u8>,
    ) -> Result<()> {
        eggs::batch_mint_dragon_eggs(ctx, faction_id, mint_count, ticket_tier_index)
    }

    /// Stake a Dragon Egg to boost hashpower (if faction matches player's faction)
    pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
        eggs::stake_dragon_egg(ctx)
    }

    /// Unstake a Dragon Egg (remove hashpower boost)
    pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
        eggs::unstake_dragon_egg(ctx)
    }

    /// Claim power points and distribute them to staked eggs
    /// Power is accumulated when claiming dbtc rewards
    pub fn claim_power(ctx: Context<ClaimPower>) -> Result<()> {
        eggs::claim_power(ctx)
    }


   
}
