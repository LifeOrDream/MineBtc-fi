// # Admin Instructions
//
// This module contains administrative functions for configuring and managing the MineBTC program.
//
// ## Key Functions
//
// - `initialize`: Sets up the initial global configuration.
// - `update_config`: Updates global parameters like authorities and fees.
// - `add_faction`: Registers new factions in the game.
// - `initialize_mining`: Starts the token mining process.
// - `initialize_doge_config`: Sets up the Doge NFT system.
// - `initialize_tax_config`: Configures the tax and burn mechanisms.
// - `initialize_game_state`: Prepares the game state for the first round.
//
// Only authorized administrators (or the program authority) can call these functions.
//

use crate::events::*;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use mpl_core::{
    instructions::{CreateCollectionV1CpiBuilder, UpdateCollectionV1CpiBuilder},
    ID as MPL_CORE_PROGRAM_ID,
};

use crate::errors::ErrorCode;

use anchor_spl::token::{self, Token};
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{
    self as token_if, Mint as Mint2022, TokenAccount as TokenAccount2022,
};

use mpl_core::{
    instructions::AddCollectionPluginV1CpiBuilder,
    types::{Creator, Plugin, PluginAuthority, Royalties, RuleSet, UpdateDelegate},
};

/// Helper type for passing creators from client
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreatorInput {
    pub address: Pubkey,
    /// Whole-percent share (`100` = 100%). Sum must equal the percentage denominator.
    pub percentage: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct GameplayTuningUpdateArgs {
    pub enable_rpg_progression: Option<bool>,
    pub max_evolution_stage_unlocked: Option<u8>,

    pub faction_war_base_reward_bps: Option<u16>,
    pub faction_war_loyalty_reward_bps: Option<u16>,
    pub faction_war_mvp_reward_bps: Option<u16>,
    pub faction_war_doge_reward_bps: Option<u16>,

    pub base_mutation_chance_bps: Option<u16>,
    pub mutation_chance_floor_bps: Option<u16>,
    pub mutation_chance_cap_bps: Option<u16>,
    pub faction_volume_threshold_lamports: Option<u64>,
    pub extra_volume_threshold_per_mutation_lamports: Option<u64>,
    pub global_mutation_pressure_decay_bps: Option<u16>,
    pub global_mutation_pressure_per_mutation_bps: Option<u16>,
    pub target_mutations_per_cycle: Option<u16>,
    pub target_rounds_per_cycle: Option<u16>,
    pub pacing_max_adjustment_bps: Option<u16>,
}

// --------------------------------------------------------------------------------
// ------------ GLOBAL_CONFIG :: UPDATES, ADDING EXPANSIONS ------------
// --------------------------------------------------------------------------------

/// Initialize the global program configuration (admin only)
///
/// Creates the GlobalConfig and MineBtcMining accounts and initializes default values.
/// This function can only be called once during program deployment.
///
/// # Parameters
/// - `fee_recipient`: Address that receives creation fees and dev earnings
///
/// # Initializes
/// - GlobalConfig with default fee distributions
/// - MineBtcMining account
/// - SOL treasury PDA
pub fn internal_initialize(ctx: Context<Initialize>, fee_recipient: Pubkey) -> Result<()> {
    crate::log_fn!("admin", "internal_initialize");
    let global_config = &mut ctx.accounts.global_config;
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    // Initialize GlobalConfig
    global_config.ext_authority = ctx.accounts.authority.key();
    global_config.pending_authority = Pubkey::default();
    global_config.fee_recipient = fee_recipient;

    // Store both PDA bumps for future derivation
    global_config.pda_sol_treasury = ctx.accounts.sol_treasury.key();
    global_config.treasury_bump = ctx.bumps.sol_treasury;

    // Initialize SOL fee config (defaults defined in state.rs)
    global_config.sol_fee_config = SolFeeConfig {
        protocol_fee_pct: DEFAULT_PROTOCOL_FEE_PCT,
        buyback_pct: DEFAULT_BUYBACK_PCT,
        stakers_pct: DEFAULT_STAKERS_PCT,
        referral_fee_pct: DEFAULT_REFERRAL_FEE_PCT,
        same_faction_referral_fee_pct: DEFAULT_SAME_FACTION_REFERRAL_FEE_PCT,
        cycle_sol_split_pct: DEFAULT_CYCLE_SOL_SPLIT_PCT,
    };

    // Initialize dogeBTC round distribution config (defaults defined in state.rs)
    // Invariant: stakers + winners + 2 * same_faction + jackpot = 100
    global_config.minebtc_dist_config = MineBtcDistConfig {
        minebtc_stakers_pct: DEFAULT_MINEBTC_STAKERS_PCT,
        minebtc_winners_pct: DEFAULT_MINEBTC_WINNERS_PCT,
        minebtc_same_faction_pct: DEFAULT_MINEBTC_SAME_FACTION_PCT,
        minebtc_jackpot_pct: DEFAULT_MINEBTC_JACKPOT_PCT,
        hodl_tax_pct: DEFAULT_HODL_TAX_PCT,
    };

    global_config.snapshot_interval = DEFAULT_SNAPSHOT_INTERVAL;
    global_config.gameplay_tuning.apply_defaults();
    global_config.is_paused = false;

    // Initialize Raydium pool state to default (must be set via admin function)
    global_config.raydium_pool_state = Pubkey::default();

    global_config.bump = ctx.bumps.global_config;

    // Initialize empty factions list
    global_config.supported_factions = Vec::new();

    // Optionally drop 1 lamport into the vaults for future-proof rent-exempt status
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        1,
    )?;

    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.autominer_custody.to_account_info(),
            },
        ),
        1,
    )?;

    // Initialize MineBtcMining
    mine_btc_mining.minebtc_token_vault = Pubkey::default(); // Will be set during initialize_mining
    mine_btc_mining.mining_start_timestamp = 0; // Set to 0 to indicate mining not started
    mine_btc_mining.mine_btc_per_round = 0;

    mine_btc_mining.total_tokens_mined = 0;
    mine_btc_mining.bump = ctx.bumps.mine_btc_mining;
    mine_btc_mining.vault_auth_bump = 0; // Will be set during initialize_mining

    // Initialize dynamic distribution fields with defaults
    mine_btc_mining.raydium_pool_state = Pubkey::default();
    mine_btc_mining.last_rate_update = 0;
    mine_btc_mining.price_history = Vec::new();
    mine_btc_mining.recent_price = 0; // Default: 0.001 SOL/MINEBTC
    mine_btc_mining.track_price = 0;
    mine_btc_mining.sol_for_pol = 0;

    // Initialize emission adjustment parameters (defaults defined in state.rs)
    mine_btc_mining.price_change_threshold = DEFAULT_PRICE_CHANGE_THRESHOLD;
    mine_btc_mining.emission_increase_pct = DEFAULT_EMISSION_INCREASE_PCT;
    mine_btc_mining.emission_decrease_pct = DEFAULT_EMISSION_DECREASE_PCT;

    // ---------------------------- Unrefined Rewards ---------------------------------
    let hodl_pool = &mut ctx.accounts.hodl_pool;
    hodl_pool.hodl_tax_index = INDEX_PRECISION as u128;
    hodl_pool.total_minebtc_claimable = 0;

    Ok(())
}

/// Set the Raydium pool state address (admin only)
/// Also initializes SOL vault PDAs if not already initialized.
///
/// Security measure to prevent using malicious pools for swaps.
/// Only the authorized Raydium pool can be used for price discovery and liquidity operations.
///
/// # Parameters
/// - `raydium_pool_state`: The authorized Raydium pool state address
pub fn set_raydium_pool_state_internal(
    ctx: Context<SetRaydiumPoolState>,
    raydium_pool_state: Pubkey,
) -> Result<()> {
    crate::log_fn!("admin", "set_raydium_pool_state_internal");
    let global_config = &mut ctx.accounts.global_config;

    require!(
        raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );

    global_config.raydium_pool_state = raydium_pool_state;

    // Initialize sol_rewards_vault if not already initialized
    let sol_rewards_vault_lamports = ctx.accounts.sol_rewards_vault.lamports();
    if sol_rewards_vault_lamports == 0 {
        // Transfer 1 lamport to make it rent-exempt
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.sol_rewards_vault.to_account_info(),
                },
            ),
            1,
        )?;
    }

    // Initialize sol_prize_pot_vault if not already initialized
    let sol_prize_pot_vault_lamports = ctx.accounts.sol_prize_pot_vault.lamports();
    if sol_prize_pot_vault_lamports == 0 {
        // Transfer 1 lamport to make it rent-exempt
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.sol_prize_pot_vault.to_account_info(),
                },
            ),
            1,
        )?;
    }

    // Initialize faction_war_sol_vault if not already initialized. User bets
    // transfer tiny cycle splits here, so the PDA must exist rent-exempt before
    // the first split arrives.
    let faction_war_sol_vault_lamports = ctx.accounts.faction_war_sol_vault.lamports();
    if faction_war_sol_vault_lamports == 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.faction_war_sol_vault.to_account_info(),
                },
            ),
            1,
        )?;
    }

    Ok(())
}

/// Add a single faction to the global config (admin only)
///
/// Adds a new faction to the supported factions list and initializes its FactionState account.
/// Maximum of MAX_FACTIONS (15) factions can be added.
///
/// # Parameters
/// - `faction_name`: Name of the faction (max MAX_FACTION_NAME_LENGTH characters)
///
/// # Effects
/// - Adds faction to `supported_factions` list
/// - Creates and initializes FactionState PDA for the new faction
/// - Faction ID is assigned based on current count (0-indexed)
pub fn add_faction_internal(
    ctx: Context<AddFaction>,
    faction_name: String,
    faction_id: u8,
) -> Result<()> {
    crate::log_fn!("admin", "add_faction_internal");
    let global_config = &mut ctx.accounts.global_config;
    let faction_state = &mut ctx.accounts.faction_state;

    // Validate faction name
    require!(
        !faction_name.is_empty() && faction_name.len() <= MAX_FACTION_NAME_LENGTH,
        ErrorCode::InvalidFactionName
    );
    require!(
        !global_config
            .supported_factions
            .iter()
            .any(|existing_name| existing_name == &faction_name),
        ErrorCode::FactionAlreadyExists
    );

    // Check we don't exceed max factions
    let current_faction_count = global_config.supported_factions.len();
    require!(
        current_faction_count < MAX_FACTIONS,
        ErrorCode::MaxFactionsReached
    );

    require!(
        faction_id == current_faction_count as u8,
        ErrorCode::InvalidFactionId
    );

    // Initialize faction state data
    faction_state.bump = ctx.bumps.faction_state;
    faction_state.faction_id = faction_id;
    faction_state.total_dogebtc_hashpower = 0;
    faction_state.dogebtc_staked = 0;
    faction_state.dogebtc_dogebtc_reward_index = 0;
    faction_state.dogebtc_sol_reward_index = 0;
    faction_state.total_lp_hashpower = 0;
    faction_state.lp_sol_reward_index = 0;
    faction_state.lp_dogebtc_reward_index = 0;
    faction_state.sol_reward_index = 0;
    // Add faction to config
    global_config.supported_factions.push(faction_name.clone());

    // Emit event for off-chain indexing
    emit!(FactionAdded {
        authority: ctx.accounts.authority.key(),
        faction_name: faction_name.clone(),
        faction_id,
        faction_key: faction_state.key(),
    });

    Ok(())
}

/// Update the global configuration parameters (admin only)
///
/// Proposes a new program authority (2-step transfer).
/// Only the current `ext_authority` can call this function.
/// The new authority must call `accept_authority` to complete the transfer.
///
/// # Parameters
/// - `new_authority`: Optional new program authority to propose (if None, cancels pending transfer)
/// - `new_fee_recipient`: Optional new fee recipient (if None, fee recipient unchanged)
pub fn update_config_internal(
    ctx: Context<UpdateConfigAc>,
    new_authority: Option<Pubkey>,
    new_fee_recipient: Option<Pubkey>,
) -> Result<()> {
    crate::log_fn!("admin", "update_config_internal");
    let global_config = &mut ctx.accounts.global_config;

    // 2-step authority transfer: set pending_authority instead of immediate transfer
    // If new_authority is Some, set it as pending. If None, cancel any pending transfer.
    if let Some(authority) = new_authority {
        msg!(
            "🔐 Authority transfer proposed: {} → {}",
            global_config.ext_authority,
            authority
        );
        global_config.pending_authority = authority;
    }

    // Update creation fee recipient if provided (this takes effect immediately)
    if let Some(fee_recipient) = new_fee_recipient {
        global_config.fee_recipient = fee_recipient;
    }
    Ok(())
}

/// Cancel a pending authority transfer.
/// Only the current `ext_authority` can call this function.
pub fn cancel_authority_transfer_internal(ctx: Context<UpdateConfigAc>) -> Result<()> {
    crate::log_fn!("admin", "cancel_authority_transfer_internal");
    let global_config = &mut ctx.accounts.global_config;
    msg!(
        "🔐 Authority transfer cancelled (was pending: {})",
        global_config.pending_authority
    );
    global_config.pending_authority = Pubkey::default();
    Ok(())
}

/// Accept a proposed authority transfer (2-step transfer, step 2).
/// Only the `pending_authority` can call this function.
/// Completes the transfer: ext_authority = pending_authority, pending_authority = default.
pub fn accept_authority_internal(ctx: Context<AcceptAuthority>) -> Result<()> {
    crate::log_fn!("admin", "accept_authority_internal");
    let global_config = &mut ctx.accounts.global_config;

    // Verify there is a pending transfer
    require!(
        global_config.pending_authority != Pubkey::default(),
        ErrorCode::NoPendingAuthority
    );

    // Verify the caller is the pending authority
    require!(
        global_config.pending_authority == ctx.accounts.new_authority.key(),
        ErrorCode::Unauthorized
    );

    msg!(
        "🔐 Authority transfer accepted: {} → {}",
        global_config.ext_authority,
        global_config.pending_authority
    );

    global_config.ext_authority = global_config.pending_authority;
    global_config.pending_authority = Pubkey::default();
    Ok(())
}

/// Update fee configuration (admin only)
///
/// Updates SOL fee distribution percentages and/or MineBtc distribution percentages.
/// These config fields use whole-percent precision (`100` = 100%).
/// All percentage splits must sum to the percentage denominator for their category.
///
/// # Parameters
/// - `new_protocol_fee_pct`: Optional new protocol fee percentage (SOL fees)
/// - `new_buyback_pct`: Optional new buyback percentage (SOL fees)
/// - `new_stakers_pct`: Optional new stakers percentage (SOL fees)
/// - `new_minebtc_stakers_pct`: Optional new MineBtc stakers percentage
/// - `new_minebtc_winners_pct`: Optional new MineBtc winners percentage
/// - `new_minebtc_same_faction_pct`: Optional new MineBtc same-faction percentage
/// - `new_minebtc_jackpot_pct`: Optional new MineBtc jackpot percentage
/// - `new_hodl_tax_pct`: Optional new HODL tax percentage
/// - `snapshot_interval`: Optional new snapshot interval (in seconds, minimum time between price snapshots)
///
/// # Validation
/// - SOL fees: protocol_fee_pct + buyback_pct + stakers_pct == `PERCENTAGE_DENOMINATOR`
/// - MineBtc dist: minebtc_stakers_pct + minebtc_winners_pct +
///   (`PredictionDirection::COUNT - 1`) * minebtc_same_faction_pct +
///   minebtc_jackpot_pct == `PERCENTAGE_DENOMINATOR`
pub fn update_fees_internal(
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
    crate::log_fn!("admin", "update_fees_internal");
    let global_config = &mut ctx.accounts.global_config;

    // Update SOL fee config if any values provided
    if new_protocol_fee_pct.is_some()
        || new_buyback_pct.is_some()
        || new_stakers_pct.is_some()
        || new_referral_fee_pct.is_some()
        || new_same_faction_referral_fee_pct.is_some()
        || new_cycle_sol_split_pct.is_some()
    {
        let protocol_fee_pct =
            new_protocol_fee_pct.unwrap_or(global_config.sol_fee_config.protocol_fee_pct);
        let buyback_pct = new_buyback_pct.unwrap_or(global_config.sol_fee_config.buyback_pct);
        let stakers_pct = new_stakers_pct.unwrap_or(global_config.sol_fee_config.stakers_pct);
        let referral_fee_pct =
            new_referral_fee_pct.unwrap_or(global_config.sol_fee_config.referral_fee_pct);
        let same_faction_referral_fee_pct = new_same_faction_referral_fee_pct
            .unwrap_or(global_config.sol_fee_config.same_faction_referral_fee_pct);
        let cycle_sol_split_pct =
            new_cycle_sol_split_pct.unwrap_or(global_config.sol_fee_config.cycle_sol_split_pct);

        require!(
            protocol_fee_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            buyback_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            stakers_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            referral_fee_pct <= MAX_REFERRAL_FEE_PCT,
            ErrorCode::InvalidParameters
        );
        require!(
            same_faction_referral_fee_pct <= MAX_REFERRAL_FEE_PCT,
            ErrorCode::InvalidParameters
        );
        require!(
            cycle_sol_split_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );

        global_config.sol_fee_config = SolFeeConfig {
            protocol_fee_pct,
            buyback_pct,
            stakers_pct,
            referral_fee_pct,
            same_faction_referral_fee_pct,
            cycle_sol_split_pct,
        };
    }

    // Update MineBtc distribution config if any values provided
    if new_minebtc_stakers_pct.is_some()
        || new_minebtc_winners_pct.is_some()
        || new_minebtc_same_faction_pct.is_some()
        || new_minebtc_jackpot_pct.is_some()
    {
        let minebtc_stakers_pct = new_minebtc_stakers_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_stakers_pct);
        let minebtc_winners_pct = new_minebtc_winners_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_winners_pct);
        let minebtc_same_faction_pct = new_minebtc_same_faction_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_same_faction_pct);
        let minebtc_jackpot_pct = new_minebtc_jackpot_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_jackpot_pct);

        require!(
            minebtc_stakers_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            minebtc_winners_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            minebtc_same_faction_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        require!(
            minebtc_jackpot_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );

        // `minebtc_same_faction_pct` is applied once for each losing direction on the
        // winning faction. With Up / Down / Neutral that means two losing directions.
        let losing_direction_count = (PredictionDirection::COUNT - 1) as u16;
        let total = minebtc_stakers_pct as u16
            + minebtc_winners_pct as u16
            + (minebtc_same_faction_pct as u16 * losing_direction_count)
            + minebtc_jackpot_pct as u16;

        require!(
            total == PERCENTAGE_DENOMINATOR_U16,
            ErrorCode::InvalidParameters
        );

        // Get current hodl_tax_pct to preserve it
        let current_hodl_tax_pct = global_config.minebtc_dist_config.hodl_tax_pct;

        global_config.minebtc_dist_config = MineBtcDistConfig {
            minebtc_stakers_pct,
            minebtc_winners_pct,
            minebtc_same_faction_pct,
            minebtc_jackpot_pct,
            hodl_tax_pct: current_hodl_tax_pct,
        };
    }

    // Update HODL tax if provided
    if let Some(hodl_tax_pct) = new_hodl_tax_pct {
        require!(
            hodl_tax_pct <= PERCENTAGE_DENOMINATOR_U8,
            ErrorCode::InvalidParameters
        );
        global_config.minebtc_dist_config.hodl_tax_pct = hodl_tax_pct;
    }

    // Update snapshot interval if provided
    if let Some(snapshot_interval) = snapshot_interval {
        global_config.snapshot_interval = snapshot_interval;
    }

    Ok(())
}

/// Toggle RPG progression (mutations, XP) during gameplay
pub fn update_rpg_progression_internal(ctx: Context<UpdateConfigAc>, enabled: bool) -> Result<()> {
    crate::log_fn!("admin", "update_rpg_progression_internal");
    ctx.accounts.global_config.gameplay_tuning.rpg_progression = enabled;
    Ok(())
}

/// Toggle the global pause flag (authority-only kill switch).
///
/// When `paused = true`:
///   - Blocks: new bets (manual + autominer), new round starts,
///     genesis doge mints, doge breeding.
///   - Does NOT block: round settlement, all reward claims (game/staking/
///     referral/faction-war), staking + unstaking, economy crank functions
///     (snapshot_price / update_rate / add_lp_and_burn). Users can always
///     exit; pending rounds always finish.
///
/// Intended as a kill switch for live exploits, not a long-term tool.
pub fn set_pause_internal(ctx: Context<UpdateConfigAc>, paused: bool) -> Result<()> {
    crate::log_fn!("admin", "set_pause_internal");
    let global_config = &mut ctx.accounts.global_config;
    if global_config.is_paused == paused {
        msg!("[set_pause] no-op (already {})", paused);
        return Ok(());
    }
    global_config.is_paused = paused;
    msg!(
        "[set_pause] authority={} is_paused={}",
        ctx.accounts.authority.key(),
        paused
    );
    emit!(crate::events::GamePauseToggled {
        is_paused: paused,
        authority: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

/// Update the highest evolution stage unlocked for gameplay doges.
///
/// `0` disables evolution entirely. `1` allows stage 0 -> 1 evolutions, etc.
pub fn update_evolution_unlock_stage_internal(
    ctx: Context<UpdateConfigAc>,
    max_stage: u8,
) -> Result<()> {
    crate::log_fn!("admin", "update_evolution_unlock_stage_internal");
    require!(
        max_stage <= MAX_EVOLUTION_STAGE,
        ErrorCode::InvalidParameters
    );

    ctx.accounts
        .global_config
        .gameplay_tuning
        .max_evolution_stage_unlocked = max_stage;
    msg!(
        "[update_evolution_unlock_stage] authority={} max_stage={}",
        ctx.accounts.authority.key(),
        max_stage
    );

    emit!(EvolutionUnlockStageUpdated {
        authority: ctx.accounts.authority.key(),
        max_evolution_stage_unlocked: max_stage,
    });
    Ok(())
}

/// Update emission adjustment parameters (admin only)
/// Allows updating price change threshold and emission increase/decrease percentages
pub fn update_emission_params_internal(
    ctx: Context<UpdateEmissionParams>,
    price_change_threshold: Option<u64>,
    emission_increase_pct: Option<u64>,
    emission_decrease_pct: Option<u64>,
) -> Result<()> {
    crate::log_fn!("admin", "update_emission_params_internal");
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    // Update price change threshold if provided
    if let Some(threshold) = price_change_threshold {
        require!(
            threshold > 0 && threshold <= PERCENTAGE_DENOMINATOR,
            ErrorCode::InvalidParameters
        );
        mine_btc_mining.price_change_threshold = threshold;
    }

    // Update emission increase percentage if provided
    if let Some(increase_pct) = emission_increase_pct {
        require!(
            increase_pct > 0 && increase_pct <= PERCENTAGE_DENOMINATOR,
            ErrorCode::InvalidParameters
        );
        mine_btc_mining.emission_increase_pct = increase_pct;
    }

    // Update emission decrease percentage if provided
    if let Some(decrease_pct) = emission_decrease_pct {
        require!(
            decrease_pct > 0 && decrease_pct <= PERCENTAGE_DENOMINATOR,
            ErrorCode::InvalidParameters
        );
        mine_btc_mining.emission_decrease_pct = decrease_pct;
    }

    Ok(())
}

/// Unified gameplay-tuning update surface.
/// This lets admin tune the live mutation engine and cycle reward split from one payload.
pub fn update_gameplay_tuning_internal(
    ctx: Context<UpdateConfigAc>,
    args: GameplayTuningUpdateArgs,
) -> Result<()> {
    crate::log_fn!("admin", "update_gameplay_tuning_internal");
    let global_config = &mut ctx.accounts.global_config;

    if global_config.gameplay_tuning.is_uninitialized() {
        global_config.gameplay_tuning.apply_defaults();
    }

    if let Some(enabled) = args.enable_rpg_progression {
        global_config.gameplay_tuning.rpg_progression = enabled;
    }

    if let Some(max_stage) = args.max_evolution_stage_unlocked {
        require!(
            max_stage <= MAX_EVOLUTION_STAGE,
            ErrorCode::InvalidParameters
        );
        global_config.gameplay_tuning.max_evolution_stage_unlocked = max_stage;
    }

    let tuning = &mut global_config.gameplay_tuning;

    let next_base_reward_bps = args
        .faction_war_base_reward_bps
        .unwrap_or(tuning.faction_war_base_reward_bps);
    let next_loyalty_reward_bps = args
        .faction_war_loyalty_reward_bps
        .unwrap_or(tuning.faction_war_loyalty_reward_bps);
    let next_mvp_reward_bps = args
        .faction_war_mvp_reward_bps
        .unwrap_or(tuning.faction_war_mvp_reward_bps);
    let next_doge_reward_bps = args
        .faction_war_doge_reward_bps
        .unwrap_or(tuning.faction_war_doge_reward_bps);
    // base + loyalty + MVP + doge must close to 100% — these are the four
    // lanes that `compute_faction_reward_pools` splits the cycle pool into.
    let reward_total = next_base_reward_bps as u32
        + next_loyalty_reward_bps as u32
        + next_mvp_reward_bps as u32
        + next_doge_reward_bps as u32;
    require!(
        reward_total == BASIS_POINTS_DENOMINATOR as u32,
        ErrorCode::InvalidParameters
    );

    let next_base_mutation_chance_bps = args
        .base_mutation_chance_bps
        .unwrap_or(tuning.base_mutation_chance_bps);
    let next_chance_floor_bps = args
        .mutation_chance_floor_bps
        .unwrap_or(tuning.mutation_chance_floor_bps);
    let next_chance_cap_bps = args
        .mutation_chance_cap_bps
        .unwrap_or(tuning.mutation_chance_cap_bps);
    require!(
        next_base_mutation_chance_bps <= BASIS_POINTS_DENOMINATOR as u16
            && next_chance_floor_bps <= next_chance_cap_bps
            && next_chance_cap_bps <= BASIS_POINTS_DENOMINATOR as u16,
        ErrorCode::InvalidParameters
    );

    let next_decay_bps = args
        .global_mutation_pressure_decay_bps
        .unwrap_or(tuning.global_mutation_pressure_decay_bps);
    let next_pressure_step_bps = args
        .global_mutation_pressure_per_mutation_bps
        .unwrap_or(tuning.global_mutation_pressure_per_mutation_bps);
    let next_target_mutations = args
        .target_mutations_per_cycle
        .unwrap_or(tuning.target_mutations_per_cycle);
    let next_target_rounds = args
        .target_rounds_per_cycle
        .unwrap_or(tuning.target_rounds_per_cycle);
    let next_pacing_max_adjustment_bps = args
        .pacing_max_adjustment_bps
        .unwrap_or(tuning.pacing_max_adjustment_bps);
    require!(
        next_decay_bps <= BASIS_POINTS_DENOMINATOR as u16
            && next_pressure_step_bps <= BASIS_POINTS_DENOMINATOR as u16
            && next_target_mutations > 0
            && next_target_rounds > 0
            && next_pacing_max_adjustment_bps <= BASIS_POINTS_DENOMINATOR as u16
            && args
                .faction_volume_threshold_lamports
                .unwrap_or(tuning.faction_volume_threshold_lamports)
                > 0
            && args
                .extra_volume_threshold_per_mutation_lamports
                .unwrap_or(tuning.extra_volume_threshold_per_mutation_lamports)
                > 0,
        ErrorCode::InvalidParameters
    );

    tuning.faction_war_base_reward_bps = next_base_reward_bps;
    tuning.faction_war_loyalty_reward_bps = next_loyalty_reward_bps;
    tuning.faction_war_mvp_reward_bps = next_mvp_reward_bps;
    tuning.faction_war_doge_reward_bps = next_doge_reward_bps;
    tuning.base_mutation_chance_bps = next_base_mutation_chance_bps;
    tuning.mutation_chance_floor_bps = next_chance_floor_bps;
    tuning.mutation_chance_cap_bps = next_chance_cap_bps;
    tuning.faction_volume_threshold_lamports = args
        .faction_volume_threshold_lamports
        .unwrap_or(tuning.faction_volume_threshold_lamports);
    tuning.extra_volume_threshold_per_mutation_lamports = args
        .extra_volume_threshold_per_mutation_lamports
        .unwrap_or(tuning.extra_volume_threshold_per_mutation_lamports);
    tuning.global_mutation_pressure_decay_bps = next_decay_bps;
    tuning.global_mutation_pressure_per_mutation_bps = next_pressure_step_bps;
    tuning.target_mutations_per_cycle = next_target_mutations;
    tuning.target_rounds_per_cycle = next_target_rounds;
    tuning.pacing_max_adjustment_bps = next_pacing_max_adjustment_bps;

    let rpg_progression = tuning.rpg_progression;
    let max_evolution_stage_unlocked = tuning.max_evolution_stage_unlocked;
    let faction_war_base_reward_bps = tuning.faction_war_base_reward_bps;
    let faction_war_loyalty_reward_bps = tuning.faction_war_loyalty_reward_bps;
    let faction_war_mvp_reward_bps = tuning.faction_war_mvp_reward_bps;
    let faction_war_doge_reward_bps = tuning.faction_war_doge_reward_bps;
    let base_mutation_chance_bps = tuning.base_mutation_chance_bps;
    let mutation_chance_floor_bps = tuning.mutation_chance_floor_bps;
    let mutation_chance_cap_bps = tuning.mutation_chance_cap_bps;
    let faction_volume_threshold_lamports = tuning.faction_volume_threshold_lamports;
    let extra_volume_threshold_per_mutation_lamports =
        tuning.extra_volume_threshold_per_mutation_lamports;
    let global_mutation_pressure_decay_bps = tuning.global_mutation_pressure_decay_bps;
    let global_mutation_pressure_per_mutation_bps =
        tuning.global_mutation_pressure_per_mutation_bps;
    let target_mutations_per_cycle = tuning.target_mutations_per_cycle;
    let target_rounds_per_cycle = tuning.target_rounds_per_cycle;
    let pacing_max_adjustment_bps = tuning.pacing_max_adjustment_bps;

    emit!(GameplayTuningUpdated {
        authority: ctx.accounts.authority.key(),
        rpg_progression,
        max_evolution_stage_unlocked,
        faction_war_base_reward_bps,
        faction_war_loyalty_reward_bps,
        faction_war_mvp_reward_bps,
        faction_war_doge_reward_bps,
        base_mutation_chance_bps,
        mutation_chance_floor_bps,
        mutation_chance_cap_bps,
        faction_volume_threshold_lamports,
        extra_volume_threshold_per_mutation_lamports,
        global_mutation_pressure_decay_bps,
        global_mutation_pressure_per_mutation_bps,
        target_mutations_per_cycle,
        target_rounds_per_cycle,
        pacing_max_adjustment_bps,
    });

    Ok(())
}

// --------------------------------------------------------------------------------
// --------------------------------------------------------------------------------
// ------------ mine_btc_MINING :: INITIALIZATION & UPDATES ------------
// --------------------------------------------------------------------------------
// --------------------------------------------------------------------------------

/// Initialize mining by setting the token vault and starting timestamp (admin only)
///
/// Sets up the MineBtc mining system with the token vault and initial mining parameters.
/// Can only be called once when `mining_start_timestamp == 0`.
///
/// # Parameters
/// - `start_timestamp`: Unix timestamp when mining should start
/// - `mine_btc_per_round`: Base MineBtc emission rate per slot
/// - `pool_state`: Raydium pool state address for price discovery
pub fn initialize_mining_internal(
    ctx: Context<InitializeMining>,
    start_timestamp: u64,
    mine_btc_per_round: u64,
    pool_state: Pubkey,
) -> Result<()> {
    crate::log_fn!("admin", "initialize_mining_internal");
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    // Check mining hasn't been initialized yet
    require!(
        mine_btc_mining.mining_start_timestamp == 0,
        ErrorCode::MiningAlreadyInitialized
    );
    require!(mine_btc_per_round > 0, ErrorCode::InvalidParameters);

    // ───── persist vault + bump(s) ─────
    mine_btc_mining.minebtc_token_vault = ctx.accounts.token_vault.key();
    mine_btc_mining.vault_auth_bump = ctx.bumps.vault_authority;

    // Initialize mining parameters
    mine_btc_mining.mining_start_timestamp = start_timestamp;
    mine_btc_mining.mine_btc_per_round = mine_btc_per_round;

    // Initialize dynamic distribution fields
    mine_btc_mining.raydium_pool_state = pool_state;
    mine_btc_mining.last_rate_update = Clock::get()?.unix_timestamp;

    mine_btc_mining.price_history = Vec::with_capacity(8);
    mine_btc_mining.recent_price = 0; // Default: 0.001 SOL/MINEBTC
    mine_btc_mining.track_price = 0; // Initialize with same default

    mine_btc_mining.sol_for_pol = 0; // Initialize POL tracking
    mine_btc_mining.pol_stats = ProtocolOwnedLiquidity::default(); // Initialize POL stats tracking

    // Emit event
    emit!(MiningTokenVaultSet {
        authority: ctx.accounts.authority.key(),
        token_vault: ctx.accounts.token_vault.key(),
        token_vault_authority: ctx.accounts.vault_authority.key(),
        mining_start_timestamp: start_timestamp,
    });

    Ok(())
}

/// Deposit MineBtc tokens to the mining vault (anyone can call)
///
/// Allows anyone to deposit MineBtc tokens into the mining vault.
/// These tokens will be distributed as rewards to stakers over time.
///
/// # Parameters
/// - `amount`: Amount of MineBtc tokens to deposit (in token's native decimals)
pub fn deposit_mine_btc_tokens_internal(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
    crate::log_fn!("admin", "deposit_mine_btc_tokens_internal");
    token_if::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(), // TOKEN_2022_PROGRAM_ID
            token_if::TransferChecked {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.minebtc_token_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
        MINEBTC_DECIMALS,
    )?;

    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HASHPOWER CONFIG (ADMIN) -------------------------------------------------
// ----------------------------------------------------------------------------------------

fn validate_hashpower_config(
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    require!(
        min_lockup_days <= max_lockup_days,
        ErrorCode::InvalidParameters
    );
    // Lockup may add up to 3x. Passive staked Doges may add another 3x,
    // giving a hard 9x max staking hashpower setup.
    require!(
        base_multiplier >= M_HUNDRED as u16
            && max_multiplier >= base_multiplier
            && max_multiplier <= 300,
        ErrorCode::InvalidParameters
    );
    Ok(())
}

pub fn initialize_hashpower_config_internal(
    ctx: Context<InitializeHashpowerConfig>,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    crate::log_fn!("admin", "initialize_hashpower_config_internal");
    validate_hashpower_config(
        min_lockup_days,
        max_lockup_days,
        base_multiplier,
        max_multiplier,
    )?;
    let hashpower_config = &mut ctx.accounts.hashpower_config;

    hashpower_config.min_lockup_days = min_lockup_days;
    hashpower_config.max_lockup_days = max_lockup_days;
    hashpower_config.base_multiplier = base_multiplier;
    hashpower_config.max_multiplier = max_multiplier;
    hashpower_config.bump = ctx.bumps.hashpower_config;

    Ok(())
}

pub fn update_hashpower_config_internal(
    ctx: Context<UpdateHashpowerConfig>,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    crate::log_fn!("admin", "update_hashpower_config_internal");
    validate_hashpower_config(
        min_lockup_days,
        max_lockup_days,
        base_multiplier,
        max_multiplier,
    )?;
    let hashpower_config = &mut ctx.accounts.hashpower_config;

    hashpower_config.min_lockup_days = min_lockup_days;
    hashpower_config.max_lockup_days = max_lockup_days;
    hashpower_config.base_multiplier = base_multiplier;
    hashpower_config.max_multiplier = max_multiplier;
    hashpower_config.bump = ctx.bumps.hashpower_config;
    Ok(())
}

// ----------------------------------------------------------------------------------------
// --------------  DOGE URI MANAGEMENT (ADMIN) ---------------------------------------
// ----------------------------------------------------------------------------------------

/// Initialize DogeConfig account (admin only).
///
/// Stores non-sale Doge state: collection, lifetime supply cap, and breeding config.
pub fn initialize_doge_config_internal(
    ctx: Context<InitializeDogeConfig>,
    max_supply: u64,
) -> Result<()> {
    crate::log_fn!("admin", "initialize_doge_config_internal");
    let doges_config = &mut ctx.accounts.doges_config;

    require!(max_supply > 0, ErrorCode::InvalidParameters);

    doges_config.bump = ctx.bumps.doges_config;
    doges_config.doge_collection = Pubkey::default();
    doges_config.max_supply = max_supply;
    doges_config.total_doges_minted = 0;
    doges_config.breeding_allowed = false;
    doges_config.breed_base_price = 0;
    doges_config.breed_curve_a = 100;

    msg!(
        "✅ [initialize_doge_config] max_supply={} total_doges_minted={} breeding_allowed={}",
        doges_config.max_supply,
        doges_config.total_doges_minted,
        doges_config.breeding_allowed
    );

    Ok(())
}

/// Initialize DogeMintConfig account (admin only).
///
/// Stores genesis-sale-only state: bonding curve, sale switch, ticket tiers, and per-faction caps.
pub fn initialize_doge_mint_config_internal(
    ctx: Context<InitializeDogeMintConfig>,
    base_price: u64,
    curve_a: u64,
    genesis_mint_limit: u64,
    max_genesis_mints_per_faction: u16,
) -> Result<()> {
    crate::log_fn!("admin", "initialize_doge_mint_config_internal");
    require!(base_price > 0, ErrorCode::InvalidParameters);
    require!(curve_a > 0, ErrorCode::InvalidParameters);
    require!(genesis_mint_limit > 0, ErrorCode::InvalidParameters);
    require!(
        max_genesis_mints_per_faction > 0,
        ErrorCode::InvalidParameters
    );
    require!(
        genesis_mint_limit <= max_genesis_mints_per_faction as u64 * NUM_FACTIONS as u64,
        ErrorCode::InvalidParameters
    );

    let doge_mint_config = &mut ctx.accounts.doge_mint_config;
    doge_mint_config.bump = ctx.bumps.doge_mint_config;
    doge_mint_config.is_active = false;
    doge_mint_config.base_price = base_price;
    doge_mint_config.curve_a = curve_a;
    doge_mint_config.genesis_mint_limit = genesis_mint_limit;
    doge_mint_config.genesis_mints = 0;
    doge_mint_config.max_genesis_mints_per_faction = max_genesis_mints_per_faction;
    doge_mint_config.genesis_mints_by_faction = [0u16; NUM_FACTIONS];
    doge_mint_config.ticket_tiers = Vec::new();

    msg!(
        "✅ [initialize_doge_mint_config] base_price={} curve_a={} genesis_mint_limit={} per_faction_limit={} countries_supported={}",
        base_price,
        curve_a,
        genesis_mint_limit,
        max_genesis_mints_per_faction,
        NUM_FACTIONS
    );

    Ok(())
}

/// Switch doge mining state (toggle is_active) (admin only)
///
/// Toggles the `is_active` field in the doge config.
/// When `is_active` is false, doge mining is paused.
///
/// This allows admins to pause/resume the doge mining without losing state.
pub fn switch_doge_mining_internal(ctx: Context<SwitchDogeMiningState>) -> Result<()> {
    crate::log_fn!("admin", "switch_doge_mining_internal");
    let doge_mint_config = &mut ctx.accounts.doge_mint_config;
    doge_mint_config.is_active = !doge_mint_config.is_active;
    msg!(
        "🔁 [switch_doge_mining] is_active={}",
        doge_mint_config.is_active
    );
    Ok(())
}

/// Create Doge collection with program PDA as authority (admin only)
///
/// Creates a new Metaplex Core collection for Doge NFTs.
/// The collection's update authority is set to a program-controlled PDA.
/// Requires DogeConfig to be initialized first.
///
/// # Parameters
/// - `name`: Collection name
/// - `uri`: Collection metadata URI
pub fn create_doge_collection_internal(
    ctx: Context<CreateDogeCollection>,
    name: String,
    uri: String,
) -> Result<()> {
    crate::log_fn!("admin", "create_doge_collection_internal");
    let doges_config = &mut ctx.accounts.doges_config;

    // Get the collection authority bump for signing
    let collection_authority_bump = ctx.bumps.collection_authority;
    let _collection_authority_seeds = &[COLLECTION_AUTHORITY_SEED, &[collection_authority_bump]];

    // Create the collection using CPI
    let mpl_core_program = &ctx.accounts.mpl_core_program.to_account_info();
    let mut cpi_builder = CreateCollectionV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.authority.to_account_info())
        .update_authority(Some(&ctx.accounts.collection_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name.clone())
        .uri(uri.clone())
        .invoke()?;

    // Store the collection address in global config
    doges_config.doge_collection = ctx.accounts.collection.key();

    emit!(DogeCollectionCreated {
        collection: ctx.accounts.collection.key(),
        update_authority: ctx.accounts.collection_authority.key(),
        name,
        uri,
    });

    Ok(())
}

/// Initialize royalties on the Doge collection (admin only)
///
/// Sets up royalty configuration for the Doge NFT collection using Metaplex Core.
/// Initializes with an empty ProgramDenyList that can be updated later.
///
/// # Parameters
/// - `basis_points`: Royalty percentage in basis points (e.g., 500 = 5%)
/// - `creators`: List of creator addresses and their percentage shares
///
/// # Validation
/// - At least one creator must be provided
/// - Sum of creator percentages must equal 100
pub fn init_doge_royalties_internal(
    ctx: Context<InitDogeRoyalties>,
    basis_points: u16,
    creators: Vec<CreatorInput>,
) -> Result<()> {
    crate::log_fn!("admin", "init_doge_royalties_internal");
    let global_config = &ctx.accounts.global_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    // Basic creator validation
    require!(!creators.is_empty(), ErrorCode::NoCreators);
    let total_pct: u16 = creators.iter().map(|c| c.percentage as u16).sum();
    require!(
        total_pct == PERCENTAGE_DENOMINATOR_U16,
        ErrorCode::InvalidCreatorShare
    );

    // Convert to mpl-core creators
    let creators_mpl: Vec<Creator> = creators
        .into_iter()
        .map(|c| Creator {
            address: c.address,
            percentage: c.percentage,
        })
        .collect();

    // Royalties plugin data
    let royalties = Royalties {
        basis_points,
        creators: creators_mpl,
        // Start with an EMPTY ProgramDenyList so you can add later.
        rule_set: RuleSet::ProgramDenyList(vec![]),
    };

    // PDA signer for collection authority (same PDA you used as update_authority)
    let bump = ctx.bumps.collection_authority;
    let seeds: &[&[u8]] = &[COLLECTION_AUTHORITY_SEED, &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let mpl_core_program = &ctx.accounts.mpl_core_program.to_account_info();
    let mut cpi = AddCollectionPluginV1CpiBuilder::new(mpl_core_program);

    cpi.collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.authority.to_account_info())
        // The authority that initializes the plugin is the collection update authority PDA.
        .authority(Some(&ctx.accounts.collection_authority.to_account_info()))
        .plugin(Plugin::Royalties(royalties))
        // Plugin authority is "UpdateAuthority", i.e. the collection update authority PDA.
        .init_authority(PluginAuthority::UpdateAuthority)
        .system_program(&ctx.accounts.system_program.to_account_info())
        // No log_wrapper needed; pass no extra accounts.
        .invoke_signed(signer_seeds)?;

    Ok(())
}

/// Add an UpdateDelegate to the collection (admin only)
///
/// Adds an external wallet as an UpdateDelegate on the Metaplex Core collection.
/// This allows the delegate to sign verification messages for marketplace registration
/// (e.g. Magic Eden) WITHOUT transferring the update authority away from the program PDA.
/// Minting continues to work since the PDA remains the update authority.
///
/// # Parameters
/// - `delegate`: The wallet address to add as a delegate
pub fn add_collection_delegate_internal(
    ctx: Context<AddCollectionDelegate>,
    delegate: Pubkey,
) -> Result<()> {
    crate::log_fn!("admin", "add_collection_delegate_internal");
    let bump = ctx.bumps.collection_authority;
    let seeds: &[&[u8]] = &[COLLECTION_AUTHORITY_SEED, &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let mpl_core_info = ctx.accounts.mpl_core_program.to_account_info();
    let collection_info = ctx.accounts.collection.to_account_info();
    let authority_info = ctx.accounts.authority.to_account_info();
    let collection_auth_info = ctx.accounts.collection_authority.to_account_info();
    let system_info = ctx.accounts.system_program.to_account_info();

    let mut cpi = AddCollectionPluginV1CpiBuilder::new(&mpl_core_info);

    cpi.collection(&collection_info)
        .payer(&authority_info)
        .authority(Some(&collection_auth_info))
        .plugin(Plugin::UpdateDelegate(UpdateDelegate {
            additional_delegates: vec![delegate],
        }))
        .init_authority(PluginAuthority::UpdateAuthority)
        .system_program(&system_info)
        .invoke_signed(signer_seeds)?;

    emit!(CollectionDelegateAdded {
        collection: ctx.accounts.collection.key(),
        delegate,
    });

    Ok(())
}

/// Update collection metadata (name, URI) via the program PDA (admin only)
///
/// Updates the collection's on-chain name and/or URI. Useful for fixing
/// dead image URLs or updating collection metadata.
///
/// # Parameters
/// - `new_name`: Optional new collection name
/// - `new_uri`: Optional new collection URI
pub fn update_collection_info_internal(
    ctx: Context<AddCollectionDelegate>,
    new_name: Option<String>,
    new_uri: Option<String>,
) -> Result<()> {
    crate::log_fn!("admin", "update_collection_info_internal");
    let bump = ctx.bumps.collection_authority;
    let seeds: &[&[u8]] = &[COLLECTION_AUTHORITY_SEED, &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let mpl_core_info = ctx.accounts.mpl_core_program.to_account_info();
    let collection_info = ctx.accounts.collection.to_account_info();
    let authority_info = ctx.accounts.authority.to_account_info();
    let collection_auth_info = ctx.accounts.collection_authority.to_account_info();
    let system_info = ctx.accounts.system_program.to_account_info();

    let mut cpi = UpdateCollectionV1CpiBuilder::new(&mpl_core_info);

    cpi.collection(&collection_info)
        .payer(&authority_info)
        .authority(Some(&collection_auth_info))
        .system_program(&system_info);

    if let Some(name) = &new_name {
        cpi.new_name(name.clone());
    }
    if let Some(uri) = &new_uri {
        cpi.new_uri(uri.clone());
    }

    cpi.invoke_signed(signer_seeds)?;

    emit!(CollectionInfoUpdated {
        collection: ctx.accounts.collection.key(),
        new_name,
        new_uri,
    });

    Ok(())
}

/// Add or update ticket tier configs (admin only)
///
/// Configures ticket tier options that users can choose when minting Doge.
/// Users receive free tickets based on the selected tier when they mint.
///
/// # Parameters
/// - `ticket_tier_index`: Index of the ticket tier (0-2, max 3 tiers)
/// - `ticket_value`: Value of each ticket in lamports (e.g., 10_000_000 = 0.01 SOL)
///
/// # Example
/// - Tier 0: 0.01 SOL × 1000 tickets
/// - Tier 1: 0.1 SOL × 10 tickets
pub fn add_ticket_tier_config_int(
    ctx: Context<UpdateDogeMintConfig>,
    ticket_tier_index: u8,
    ticket_value: u64,
) -> Result<()> {
    crate::log_fn!("admin", "add_ticket_tier_config_int");
    let global_config = &ctx.accounts.global_config;
    let doge_mint_config = &mut ctx.accounts.doge_mint_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    require!(
        ticket_tier_index < DogeMintConfig::MAX_TICKET_TIERS as u8,
        ErrorCode::InvalidParameters
    );
    require!(ticket_value > 0, ErrorCode::InvalidParameters);

    let tier_index = ticket_tier_index as usize;

    // Ensure vector is large enough
    while doge_mint_config.ticket_tiers.len() <= tier_index {
        doge_mint_config
            .ticket_tiers
            .push(TicketTier { ticket_value: 0 });
    }

    // Update or add ticket tier
    doge_mint_config.ticket_tiers[tier_index] = TicketTier { ticket_value };
    msg!(
        "🎟️ [add_ticket_tier_config] tier_index={} ticket_value={} configured_tiers={}",
        ticket_tier_index,
        ticket_value,
        doge_mint_config.ticket_tiers.len()
    );

    Ok(())
}

/// Set or update a user's free Doge mint allowance (admin only).
/// The whitelisted user still pays transaction fees and rent, but not the Doge mint fee.
pub fn set_doge_free_mint_allowance_internal(
    ctx: Context<SetDogeFreeMintAllowance>,
    user: Pubkey,
    remaining_free_mints: u8,
) -> Result<()> {
    crate::log_fn!("admin", "set_doge_free_mint_allowance_internal");
    require!(
        remaining_free_mints <= MAX_FREE_DOGE_MINTS_PER_USER,
        ErrorCode::MaxFreeDogeMintsExceeded
    );

    let allowance = &mut ctx.accounts.doge_free_mint_allowance;
    allowance.user = user;
    allowance.remaining_free_mints = remaining_free_mints;
    allowance.bump = ctx.bumps.doge_free_mint_allowance;

    msg!(
        "🎟️ [set_doge_free_mint_allowance] authority={} user={} remaining_free_mints={}",
        ctx.accounts.authority.key(),
        user,
        remaining_free_mints
    );

    emit!(DogeFreeMintAllowanceUpdated {
        authority: ctx.accounts.authority.key(),
        user,
        remaining_free_mints,
    });

    Ok(())
}

/// Update DogeConfig account (admin only)
///
/// Updates the DogeConfig account that stores collection, supply, and breeding state.
/// All parameters are optional -- only provided values are changed.
pub fn update_doge_config_internal(
    ctx: Context<UpdateDogeConfig>,
    max_supply: Option<u64>,
) -> Result<()> {
    crate::log_fn!("admin", "update_doge_config_internal");
    let doges_config = &mut ctx.accounts.doges_config;
    if let Some(supply) = max_supply {
        require!(
            supply >= doges_config.total_doges_minted,
            ErrorCode::InvalidParameters
        );
        doges_config.max_supply = supply;
    }
    msg!(
        "✅ [update_doge_config] max_supply={} total_doges_minted={} breeding_allowed={} breed_base_price={} breed_curve_a={}",
        doges_config.max_supply,
        doges_config.total_doges_minted,
        doges_config.breeding_allowed,
        doges_config.breed_base_price,
        doges_config.breed_curve_a
    );
    Ok(())
}

/// Update DogeMintConfig account (admin only).
pub fn update_doge_mint_config_internal(
    ctx: Context<UpdateDogeMintConfig>,
    base_price: Option<u64>,
    curve_a: Option<u64>,
    genesis_mint_limit: Option<u64>,
    max_genesis_mints_per_faction: Option<u16>,
) -> Result<()> {
    crate::log_fn!("admin", "update_doge_mint_config_internal");
    let doge_mint_config = &mut ctx.accounts.doge_mint_config;

    if let Some(price) = base_price {
        require!(price > 0, ErrorCode::InvalidParameters);
        doge_mint_config.base_price = price;
    }
    if let Some(curve) = curve_a {
        require!(curve > 0, ErrorCode::InvalidParameters);
        doge_mint_config.curve_a = curve;
    }
    if let Some(per_faction) = max_genesis_mints_per_faction {
        let current_max = doge_mint_config
            .genesis_mints_by_faction
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        require!(per_faction >= current_max, ErrorCode::InvalidParameters);
        doge_mint_config.max_genesis_mints_per_faction = per_faction;
    }
    if let Some(limit) = genesis_mint_limit {
        require!(
            limit >= doge_mint_config.genesis_mints,
            ErrorCode::InvalidParameters
        );
        doge_mint_config.genesis_mint_limit = limit;
    }
    require!(
        doge_mint_config.genesis_mint_limit
            <= doge_mint_config.max_genesis_mints_per_faction as u64 * NUM_FACTIONS as u64,
        ErrorCode::InvalidParameters
    );

    msg!(
        "✅ [update_doge_mint_config] base_price={} curve_a={} genesis_mints={} / {} per_faction_limit={} ticket_tiers={}",
        doge_mint_config.base_price,
        doge_mint_config.curve_a,
        doge_mint_config.genesis_mints,
        doge_mint_config.genesis_mint_limit,
        doge_mint_config.max_genesis_mints_per_faction,
        doge_mint_config.ticket_tiers.len()
    );
    Ok(())
}

/// Update breeding config (admin only)
pub fn update_breeding_config_internal(
    ctx: Context<UpdateDogeConfig>,
    breeding_allowed: bool,
    breed_base_price: u64,
    breed_curve_a: u64,
) -> Result<()> {
    crate::log_fn!("admin", "update_breeding_config_internal");
    let doges_config = &mut ctx.accounts.doges_config;
    doges_config.breeding_allowed = breeding_allowed;
    doges_config.breed_base_price = breed_base_price;
    doges_config.breed_curve_a = breed_curve_a;
    msg!(
        "🧬 [update_breeding_config] breeding_allowed={} breed_base_price={} breed_curve_a={} total_doges_minted={} / {}",
        breeding_allowed,
        breed_base_price,
        breed_curve_a,
        doges_config.total_doges_minted,
        doges_config.max_supply
    );

    Ok(())
}

// --------------------------------------------------------------------------------
// ------------ GAME STATE INITIALIZATION -----------------------------------------
// --------------------------------------------------------------------------------

/// Initialize the global game state (admin only)
///
/// Sets up the GlobalGameState account that tracks game rounds, betting, and rewards.
/// This must be called before any rounds can be started.
///
/// # Parameters
/// - `round_duration_seconds`: Duration of each game round in seconds
pub fn initialize_game_state_internal(
    ctx: Context<InitializeGameState>,
    round_duration_seconds: i64,
) -> Result<()> {
    crate::log_fn!("admin", "initialize_game_state_internal");
    let global_game_state = &mut ctx.accounts.global_game_state;

    // Initialize game state
    global_game_state.bump = ctx.bumps.global_game_state;
    global_game_state.is_active = true;
    global_game_state.can_begin_round = true;

    global_game_state.current_round_id = 0; // Will be incremented to 1 in start_round
    global_game_state.round_duration_seconds = round_duration_seconds;

    // Initialize previous round data
    global_game_state.last_round_id = 0;
    global_game_state.winning_faction_id = 0;

    Ok(())
}

/// Update game state (admin only)
///
/// Optionally toggles is_active and/or updates round duration.
pub fn update_game_state_internal(
    ctx: Context<UpdateGameState>,
    is_active: Option<bool>,
    round_duration_seconds: Option<i64>,
) -> Result<()> {
    crate::log_fn!("admin", "update_game_state_internal");
    let global_game_state = &mut ctx.accounts.global_game_state;
    if let Some(active) = is_active {
        global_game_state.is_active = active;
    }
    if let Some(duration) = round_duration_seconds {
        require!(duration > 0, ErrorCode::InvalidParameters);
        global_game_state.round_duration_seconds = duration;
    }
    Ok(())
}

// ----------------------------------------------------------------------------------------
// ------------ SYSTEM ACCOUNTS INITIALIZATION ----------------------------------
// ----------------------------------------------------------------------------------------

/// Initialize system referral account and buybacks system (admin only)
///
/// Creates and initializes both the system referral rewards account and the buybacks tracking account.
/// The system referral account reserves the sentinel PDA used for players who register
/// without a referral code.
/// The buybacks account tracks SOL accumulated for token buybacks.
///
/// # Initializes
/// - System referral rewards PDA
/// - Buybacks account PDA
/// - Buybacks SOL vault PDA
pub fn initialize_system_accounts_internal(ctx: Context<InitializeSystemAccounts>) -> Result<()> {
    crate::log_fn!("admin", "initialize_system_accounts_internal");
    // Initialize system referral rewards account
    let system_referral = &mut ctx.accounts.system_referral_rewards;
    system_referral.owner = ctx.accounts.system_program.key();
    system_referral.bump = ctx.bumps.system_referral_rewards;
    system_referral.owner_faction_id = u8::MAX;
    system_referral.referrals_count = 0;
    system_referral.same_faction_referrals_count = 0;
    system_referral.referred_faction_counts = [0u16; NUM_FACTIONS];
    system_referral.pending_sol_rewards = 0;
    system_referral.total_sol_earned = 0;

    // Initialize buybacks account
    let buybacks_ac = &mut ctx.accounts.buybacks_account;
    buybacks_ac.total_sol_accumulated = 0;

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
        space = MineBtcMining::LEN,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(
        init,
        payer = authority,
        space = HodlPool::LEN,
        seeds = [HODL_POOL_SEED.as_ref()],
        bump
    )]
    pub hodl_pool: Account<'info, HodlPool>,

    /// CHECK: 0-byte PDA that only stores lamports (System Account)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: Global autominer custody PDA (System Account) holding user autominer SOL
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetRaydiumPoolState<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: SOL rewards vault for stakers (System Account, 0-byte PDA)
    #[account(
        init_if_needed,
        payer = authority,
        space = 0,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// CHECK: SOL prize pot vault (System Account, 0-byte PDA)
    #[account(
        init_if_needed,
        payer = authority,
        space = 0,
        seeds = [JACKPOT_POT_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// CHECK: Faction-war SOL vault (System Account, 0-byte PDA)
    #[account(
        init_if_needed,
        payer = authority,
        space = 0,
        seeds = [FACTION_WAR_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub faction_war_sol_vault: UncheckedAccount<'info>,

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

    #[account(mut)]
    pub authority: Signer<'info>,
}

/// Accept authority transfer — only the pending_authority can call this
#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.pending_authority == new_authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// The new authority (must match pending_authority)
    #[account(mut)]
    pub new_authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateEmissionParams<'info> {
    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(
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
#[instruction(faction_name: String, faction_id: u8)]
pub struct AddFaction<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Faction state PDA (validated in instruction)
    #[account(
        init_if_needed,
        payer = authority,
        space = FactionState::LEN,
        seeds = [FACTION_STATE_SEED.as_ref(), faction_name.as_ref()],
        bump,
    )]
    pub faction_state: Account<'info, FactionState>,

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
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump,
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    //  Vault authority PDA (0-byte, signer only)
    #[account(
        seeds = [MINE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required
    pub vault_authority: UncheckedAccount<'info>,

    // ─────────────────── token-2022 vault account ────────────────────
    #[account(
        init,
        payer  = authority,
        owner  = token_program.key(),
        seeds  = [MINE_BTC_VAULT_SEED, mine_btc_mining.key().as_ref()],
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
pub struct DepositTokens<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        owner       = token_program.key(),                     // interface account check
        constraint  = depositor_token_account.owner == depositor.key() @ ErrorCode::Unauthorized,
        constraint  = depositor_token_account.mint  == minebtc_token_vault.mint @ ErrorCode::InvalidMint
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount2022>,

    // ─── mining token vault ───
    #[account(
        mut,
        seeds  = [MINE_BTC_VAULT_SEED, mine_btc_mining.key().as_ref()],
        bump,
        owner  = token_program.key(),
    )]
    pub minebtc_token_vault: InterfaceAccount<'info, TokenAccount2022>,

    #[account(
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(owner = token_program.key())]
    pub token_mint: InterfaceAccount<'info, Mint2022>,

    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct InitializeHashpowerConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
        space = HashpowerConfig::LEN,
        seeds = [HASHPOWER_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateHashpowerConfig<'info> {
    #[account(
        mut,
        seeds = [HASHPOWER_CONFIG_SEED.as_ref()],
        bump
    )]
    pub hashpower_config: Account<'info, HashpowerConfig>,

    #[account(
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
pub struct InitializeDogeConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = DogeConfig::LEN,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump
    )]
    pub doges_config: Account<'info, DogeConfig>,

    #[account(
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
pub struct InitializeDogeMintConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = DogeMintConfig::LEN,
        seeds = [DOGE_MINT_CONFIG_SEED.as_ref()],
        bump
    )]
    pub doge_mint_config: Account<'info, DogeMintConfig>,

    #[account(
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
pub struct CreateDogeCollection<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_CONFIG_SEED],
        bump = doges_config.bump,
    )]
    pub doges_config: Account<'info, DogeConfig>,

    /// CHECK: Doge collection account (will be created by MPL Core)
    #[account(mut, signer)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: Collection authority PDA that will be the update authority
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDogeConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doges_config.bump,
    )]
    pub doges_config: Account<'info, DogeConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDogeMintConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_MINT_CONFIG_SEED.as_ref()],
        bump = doge_mint_config.bump,
    )]
    pub doge_mint_config: Account<'info, DogeMintConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct SetDogeFreeMintAllowance<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init_if_needed,
        payer = authority,
        space = DogeFreeMintAllowance::LEN,
        seeds = [DOGE_FREE_MINT_ALLOWANCE_SEED.as_ref(), user.as_ref()],
        bump
    )]
    pub doge_free_mint_allowance: Account<'info, DogeFreeMintAllowance>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddCollectionDelegate<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doges_config.bump,
    )]
    pub doges_config: Account<'info, DogeConfig>,

    /// CHECK: Doge collection (already created via MPL Core)
    #[account(
        mut,
        address = doges_config.doge_collection @ ErrorCode::InvalidAccount
    )]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that is update_authority for the collection
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitDogeRoyalties<'info> {
    #[account(mut)]
    pub authority: Signer<'info>, // ext authority EOA

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump = doges_config.bump,
    )]
    pub doges_config: Account<'info, DogeConfig>,

    /// CHECK: Doge collection (already created via MPL Core)
    #[account(
        mut,
        address = doges_config.doge_collection @ ErrorCode::InvalidAccount
    )]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: PDA that is update_authority for the collection
    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED.as_ref()],
        bump
    )]
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    #[account(address = mpl_core::ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeGameState<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalGameSate::LEN,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
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
pub struct UpdateGameState<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Merged account context for initializing system referral account and buybacks system
#[derive(Accounts)]
pub struct InitializeSystemAccounts<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// Reserved sentinel referral rewards PDA for users who register without a referrer
    #[account(
        init,
        payer = authority,
        space = ReferralRewards::LEN,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), system_program.key().as_ref()],
        bump,
    )]
    pub system_referral_rewards: Account<'info, ReferralRewards>,

    /// Buybacks tracking account (admin only)
    #[account(
        init,
        payer = authority,
        space = BuybacksAccount::LEN,
        seeds = [BUYBACKS_SEED.as_ref()],
        bump
    )]
    pub buybacks_account: Account<'info, BuybacksAccount>,

    /// CHECK: SOL vault for buybacks (0-byte PDA, System Account)
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// --------------------------------------------------------------------------------
// ------------ CUSTODIAN TOKEN ACCOUNTS INITIALIZATION ------------
// --------------------------------------------------------------------------------

/// Initialize both custodian token accounts (admin only)
/// Initializes:
/// - MINEBTC custodian: Token-2022 account that holds all staked MINE_BTC tokens (global for all factions)
/// - Liquidity custodian: Standard SPL Token account that holds all staked LP tokens (global for all factions)
pub fn int_initialize_custodian_accounts(ctx: Context<InitializeCustodianAccounts>) -> Result<()> {
    crate::log_fn!("admin", "int_initialize_custodian_accounts");
    // Verify MINEBTC custodian
    require!(
        ctx.accounts.minebtc_custodian.mint == ctx.accounts.minebtc_mint.key(),
        ErrorCode::InvalidMint
    );
    require!(
        ctx.accounts.minebtc_custodian.owner == ctx.accounts.minebtc_custodian_authority.key(),
        ErrorCode::InvalidOwner
    );

    // Verify liquidity custodian
    require!(
        ctx.accounts.liquidity_custodian.mint == ctx.accounts.lp_mint.key(),
        ErrorCode::InvalidMint
    );
    require!(
        ctx.accounts.liquidity_custodian.owner == ctx.accounts.liquidity_custodian_authority.key(),
        ErrorCode::InvalidOwner
    );

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeCustodianAccounts<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: MINEBTC Mint (Token-2022)
    pub minebtc_mint: InterfaceAccount<'info, Mint2022>,

    /// MINEBTC custodian token account (Token-2022) - PDA owned by minebtc_custodian_authority
    #[account(
        init,
        payer = authority,
        seeds = [MINEBTC_CUSTODIAN_SEED.as_ref()],
        bump,
        token::mint = minebtc_mint,
        token::authority = minebtc_custodian_authority,
        token::token_program = token_2022_program,
    )]
    pub minebtc_custodian: InterfaceAccount<'info, TokenAccount2022>,

    /// CHECK: Authority PDA for minebtc_custodian (signs for token transfers)
    #[account(
        seeds = [MINEBTC_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub minebtc_custodian_authority: UncheckedAccount<'info>,

    /// CHECK: LP Mint (standard SPL Token)
    pub lp_mint: Account<'info, token::Mint>,

    /// Liquidity custodian token account (standard SPL Token) - PDA owned by liquidity_custodian_authority
    #[account(
        init,
        payer = authority,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref()],
        bump,
        token::mint = lp_mint,
        token::authority = liquidity_custodian_authority,
    )]
    pub liquidity_custodian: Account<'info, token::TokenAccount>,

    /// CHECK: Authority PDA for liquidity_custodian (signs for token transfers)
    #[account(
        seeds = [LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    pub liquidity_custodian_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_2022_program: Program<'info, Token2022>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SwitchDogeMiningState<'info> {
    #[account(
        mut,
        seeds = [DOGE_MINT_CONFIG_SEED.as_ref()],
        bump = doge_mint_config.bump,
    )]
    pub doge_mint_config: Account<'info, DogeMintConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.ext_authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
