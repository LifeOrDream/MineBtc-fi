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
use mpl_core::{instructions::CreateCollectionV1CpiBuilder, ID as MPL_CORE_PROGRAM_ID};

use crate::errors::ErrorCode;

use anchor_spl::token::{self, Token};
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{
    self as token_if, Mint as Mint2022, TokenAccount as TokenAccount2022,
};

use mpl_core::{
    instructions::AddCollectionPluginV1CpiBuilder,
    types::{Creator, Plugin, PluginAuthority, Royalties, RuleSet},
};

/// Helper type for passing creators from client
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreatorInput {
    pub address: Pubkey,
    /// Percentage share (0–100). Sum must be exactly 100.
    pub percentage: u8,
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
    msg!("🔧 [internal_initialize] Initializing global config");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Creation fee recipient: {}", fee_recipient);

    let global_config = &mut ctx.accounts.global_config;
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    // Initialize GlobalConfig
    msg!("   Initializing GlobalConfig...");
    global_config.ext_authority = ctx.accounts.authority.key();
    global_config.fee_recipient = fee_recipient;
    msg!("     Authority: {}", global_config.ext_authority);
    msg!(
        "     Creation fee recipient: {}",
        global_config.fee_recipient
    );

    // Store both PDA bumps for future derivation
    global_config.pda_sol_treasury = ctx.accounts.sol_treasury.key();
    global_config.treasury_bump = ctx.bumps.sol_treasury;
    msg!(
        "     SOL treasury PDA: {} (bump: {})",
        global_config.pda_sol_treasury,
        global_config.treasury_bump
    );

    // Initialize SOL fee config with defaults
    msg!("   Initializing SOL fee config...");
    global_config.sol_fee_config = SolFeeConfig {
        protocol_fee_pct: 10,
        buyback_pct: 80,
        stakers_pct: 40,
    };
    msg!(
        "     Protocol fee: {}%",
        global_config.sol_fee_config.protocol_fee_pct
    );
    msg!(
        "     Buyback: {}%",
        global_config.sol_fee_config.buyback_pct
    );
    msg!(
        "     Stakers: {}%",
        global_config.sol_fee_config.stakers_pct
    );

    // Initialize MineBtc distribution config with defaults
    msg!("   Initializing MineBtc distribution config...");
    global_config.minebtc_dist_config = MineBtcDistConfig {
        minebtc_stakers_pct: 50,
        minebtc_winners_pct: 30,
        minebtc_same_faction_pct: 10,
        minebtc_motherlode_pct: 10,
        refining_fee: 5,
    };
    msg!(
        "     Stakers: {}%",
        global_config.minebtc_dist_config.minebtc_stakers_pct
    );
    msg!(
        "     Winners: {}%",
        global_config.minebtc_dist_config.minebtc_winners_pct
    );
    msg!(
        "     Same-faction: {}%",
        global_config.minebtc_dist_config.minebtc_same_faction_pct
    );
    msg!(
        "     Motherlode: {}%",
        global_config.minebtc_dist_config.minebtc_motherlode_pct
    );
    msg!(
        "     Refining fee: {}%",
        global_config.minebtc_dist_config.refining_fee
    );

    global_config.change_faction_fee = 4_200_000_000; // 4.2 SOL

    // Initialize snapshot interval (default: 1800 seconds = 30 minutes)
    global_config.snapshot_interval = 1800;
    msg!(
        "     Snapshot interval: {} seconds (30 minutes)",
        global_config.snapshot_interval
    );

    // Initialize Raydium pool state to default (must be set via admin function)
    global_config.raydium_pool_state = Pubkey::default();
    msg!(
        "   Raydium pool state: {} (default, must be set via admin)",
        global_config.raydium_pool_state
    );

    global_config.bump = ctx.bumps.global_config;
    msg!("   Global config bump: {}", global_config.bump);

    // Initialize empty factions list
    global_config.supported_factions = Vec::new();
    msg!(
        "   Supported factions: {} (empty, must be added via admin)",
        global_config.supported_factions.len()
    );

    // Optionally drop 1 lamport into the vaults for future-proof rent-exempt status
    msg!("   Transferring 1 lamport to SOL treasury for rent-exempt status...");
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
    msg!("     ✓ 1 lamport transferred to SOL treasury");

    msg!("   Transferring 1 lamport to Doge treasury for rent-exempt status...");
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.eggs_treasury.to_account_info(),
            },
        ),
        1,
    )?;
    msg!("     ✓ 1 lamport transferred to Doge treasury");

    msg!("   Transferring 1 lamport to Autominer custody for rent-exempt status...");
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
    msg!("     ✓ 1 lamport transferred to Autominer custody");

    // Initialize MineBtcMining
    msg!("   Initializing MineBtcMining...");
    mine_btc_mining.minebtc_token_vault = Pubkey::default(); // Will be set during initialize_mining
    mine_btc_mining.mining_start_timestamp = 0; // Set to 0 to indicate mining not started
    mine_btc_mining.mine_btc_per_round = 0;

    mine_btc_mining.total_tokens_mined = 0;
    mine_btc_mining.bump = ctx.bumps.mine_btc_mining;
    mine_btc_mining.vault_auth_bump = 0; // Will be set during initialize_mining
    msg!(
        "     MineBtc token vault: {} (default, will be set during initialize_mining)",
        mine_btc_mining.minebtc_token_vault
    );
    msg!(
        "     Mining start timestamp: {} (0 = not started)",
        mine_btc_mining.mining_start_timestamp
    );
    msg!(
        "     MineBtc per slot: {}",
        mine_btc_mining.mine_btc_per_round
    );
    msg!("     Bump: {}", mine_btc_mining.bump);

    // Initialize dynamic distribution fields with defaults
    msg!("   Initializing dynamic distribution fields...");
    mine_btc_mining.raydium_pool_state = Pubkey::default();
    mine_btc_mining.last_rate_update = 0;
    mine_btc_mining.price_history = Vec::new();
    mine_btc_mining.recent_price = 0; // Default: 0.001 SOL/MINEBTC
    mine_btc_mining.track_price = 0;
    mine_btc_mining.sol_for_pol = 0;
    msg!(
        "     Raydium pool state: {} (default)",
        mine_btc_mining.raydium_pool_state
    );
    msg!("     Recent price: {}", mine_btc_mining.recent_price);

    // Initialize emission adjustment parameters with defaults
    msg!("   Initializing emission adjustment parameters...");
    mine_btc_mining.price_change_threshold = 3; // 3% threshold
    mine_btc_mining.emission_increase_pct = 1; // 1% increase when price goes up
    mine_btc_mining.emission_decrease_pct = 3; // 3% decrease when price goes down
    msg!(
        "     Price change threshold: {}%",
        mine_btc_mining.price_change_threshold
    );
    msg!(
        "     Emission increase: {}%",
        mine_btc_mining.emission_increase_pct
    );
    msg!(
        "     Emission decrease: {}%",
        mine_btc_mining.emission_decrease_pct
    );

    // ---------------------------- Unrefined Rewards ---------------------------------
    let unrefined_rewards = &mut ctx.accounts.unrefined_rewards;
    unrefined_rewards.unrefining_index = INDEX_PRECISION as u128;
    unrefined_rewards.total_minebtc_claimable = 0;

    //--------------------------------- Global Game State ---------------------------------
    // Note: GlobalGameSate should be initialized separately via initialize_game_state function
    // We don't initialize it here to keep Initialize simple
    msg!("   ⚠️ Note: GlobalGameSate must be initialized separately via initialize_game_state");

    msg!("✅ [internal_initialize] Global config initialized successfully");
    msg!(
        "   SOL Treasury PDA: {} (bump: {})",
        ctx.accounts.sol_treasury.key(),
        ctx.bumps.sol_treasury
    );

    Ok(())
}

/// Set the Raydium pool state address (admin only)
/// Also initializes sol_rewards_vault and sol_prize_pot_vault if not already initialized
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
    let global_config = &mut ctx.accounts.global_config;

    require!(
        raydium_pool_state != Pubkey::default(),
        ErrorCode::InvalidAccount
    );

    global_config.raydium_pool_state = raydium_pool_state;

    msg!("✅ Set Raydium pool state: {}", raydium_pool_state);

    // Initialize sol_rewards_vault if not already initialized
    msg!("   Initializing sol_rewards_vault...");
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
        msg!(
            "     ✓ sol_rewards_vault initialized (bump: {})",
            ctx.bumps.sol_rewards_vault
        );
    } else {
        msg!("     ℹ️ sol_rewards_vault already initialized");
    }

    // Initialize sol_prize_pot_vault if not already initialized
    msg!("   Initializing sol_prize_pot_vault...");
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
        msg!(
            "     ✓ sol_prize_pot_vault initialized (bump: {})",
            ctx.bumps.sol_prize_pot_vault
        );
    } else {
        msg!("     ℹ️ sol_prize_pot_vault already initialized");
    }

    Ok(())
}

/// Add a single faction to the global config (admin only)
///
/// Adds a new faction to the supported factions list and initializes its FactionState account.
/// Maximum of MAX_FACTIONS (12) factions can be added.
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
    msg!("🏛️ [add_faction_internal] Adding faction");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Faction name: {}", faction_name);

    let global_config = &mut ctx.accounts.global_config;
    let faction_state = &mut ctx.accounts.faction_state;

    msg!(
        "   Current factions count: {}",
        global_config.supported_factions.len()
    );
    msg!("   Validating faction name...");
    // Validate faction name
    require!(
        faction_name.len() > 0 && faction_name.len() <= MAX_FACTION_NAME_LENGTH,
        ErrorCode::InvalidFactionName
    );
    msg!(
        "     ✓ Faction name length valid ({} chars, max: {})",
        faction_name.len(),
        MAX_FACTION_NAME_LENGTH
    );

    // Check we don't exceed max factions
    let current_faction_count = global_config.supported_factions.len();
    require!(
        current_faction_count < MAX_FACTIONS,
        ErrorCode::MaxFactionsReached
    );
    msg!("     ✓ Factions count < MAX_FACTIONS ({})", MAX_FACTIONS);

    require!(
        faction_id == current_faction_count as u8,
        ErrorCode::InvalidFactionId
    );
    msg!(
        "     ✓ Faction ID matches current factions count ({})",
        current_faction_count
    );

    // Initialize faction state data
    msg!("   Initializing faction state data...");
    faction_state.bump = ctx.bumps.faction_state;
    faction_state.faction_id = faction_id;
    faction_state.total_minebtc_hashpower = 0;
    faction_state.minebtc_staked = 0;
    faction_state.minebtc_minebtc_reward_index = 0;
    faction_state.minebtc_sol_reward_index = 0;
    faction_state.total_lp_hashpower = 0;
    faction_state.lp_sol_reward_index = 0;
    faction_state.lp_minebtc_reward_index = 0;
    faction_state.total_sol_bets = 0;
    faction_state.total_wins = 0;
    faction_state.sol_reward_index = 0;
    faction_state.motherlode_pot_size = 0;
    msg!("     Faction state initialized:");

    // Add faction to config
    msg!("   Adding faction to supported_factions list...");
    global_config.supported_factions.push(faction_name.clone());
    msg!(
        "     New factions count: {}",
        global_config.supported_factions.len()
    );

    msg!("✅ [add_faction_internal] Faction added successfully");
    msg!("   Faction: {} (ID: {})", faction_name, faction_id);
    msg!(
        "   Total factions: {}",
        global_config.supported_factions.len()
    );

    // Emit event for off-chain indexing
    emit!(FactionAdded {
        authority: ctx.accounts.authority.key(),
        faction_name: faction_name.clone(),
        faction_id: faction_id,
        faction_key: faction_state.key(),
    });

    Ok(())
}

/// Update the global configuration parameters (admin only)
///
/// Updates the program authority and/or fee recipient address.
/// Only the current `ext_authority` can call this function.
///
/// # Parameters
/// - `new_authority`: Optional new program authority (if None, authority unchanged)
/// - `new_fee_recipient`: Optional new fee recipient (if None, fee recipient unchanged)
pub fn update_config_internal(
    ctx: Context<UpdateConfigAc>,
    new_authority: Option<Pubkey>,
    new_fee_recipient: Option<Pubkey>,
) -> Result<()> {
    msg!("🔧 [update_config_internal] Updating global config");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let global_config = &mut ctx.accounts.global_config;

    msg!("   Current config:");
    msg!("     Authority: {}", global_config.ext_authority);
    msg!(
        "     Creation fee recipient: {}",
        global_config.fee_recipient
    );

    // Update fields if provided
    if let Some(authority) = new_authority {
        let old_authority = global_config.ext_authority;
        global_config.ext_authority = authority;
        msg!("   Updated authority: {} -> {}", old_authority, authority);
    } else {
        msg!("   Authority: not updated");
    }

    // Update creation fee recipient if provided
    if let Some(fee_recipient) = new_fee_recipient {
        let old_recipient = global_config.fee_recipient;
        global_config.fee_recipient = fee_recipient;
        msg!(
            "   Updated creation fee recipient: {} -> {}",
            old_recipient,
            fee_recipient
        );
    } else {
        msg!("   Creation fee recipient: not updated");
    }

    msg!("✅ [update_config_internal] Config updated successfully");
    Ok(())
}

/// Update fee configuration (admin only)
///
/// Updates SOL fee distribution percentages and/or MineBtc distribution percentages.
/// All percentages must sum to 100% for their respective categories.
///
/// # Parameters
/// - `new_protocol_fee_pct`: Optional new protocol fee percentage (SOL fees)
/// - `new_buyback_pct`: Optional new buyback percentage (SOL fees)
/// - `new_stakers_pct`: Optional new stakers percentage (SOL fees)
/// - `new_minebtc_stakers_pct`: Optional new MineBtc stakers percentage
/// - `new_minebtc_winners_pct`: Optional new MineBtc winners percentage
/// - `new_minebtc_same_faction_pct`: Optional new MineBtc same-faction percentage
/// - `new_minebtc_motherlode_pct`: Optional new MineBtc motherlode percentage
/// - `new_refining_fee`: Optional new refining fee percentage
/// - `change_faction_fee`: Optional new change faction fee (in lamports)
/// - `snapshot_interval`: Optional new snapshot interval (in seconds, minimum time between price snapshots)
///
/// # Validation
/// - SOL fees: protocol_fee_pct + buyback_pct + stakers_pct == 100
/// - MineBtc dist: minebtc_stakers_pct + minebtc_winners_pct + minebtc_same_faction_pct + minebtc_motherlode_pct == 100
pub fn update_fees_internal(
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
    msg!("💰 [update_fees_internal] Updating fee configuration");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let global_config = &mut ctx.accounts.global_config;

    msg!("   Current SOL fee config:");
    msg!(
        "     Protocol fee: {}%",
        global_config.sol_fee_config.protocol_fee_pct
    );
    msg!(
        "     Buyback: {}%",
        global_config.sol_fee_config.buyback_pct
    );
    msg!(
        "     Stakers: {}%",
        global_config.sol_fee_config.stakers_pct
    );
    msg!("   Current MineBtc dist config:");
    msg!(
        "     Stakers: {}%",
        global_config.minebtc_dist_config.minebtc_stakers_pct
    );
    msg!(
        "     Winners: {}%",
        global_config.minebtc_dist_config.minebtc_winners_pct
    );
    msg!(
        "     Same-faction: {}%",
        global_config.minebtc_dist_config.minebtc_same_faction_pct
    );
    msg!(
        "     Motherlode: {}%",
        global_config.minebtc_dist_config.minebtc_motherlode_pct
    );
    msg!(
        "     Refining fee: {}%",
        global_config.minebtc_dist_config.refining_fee
    );

    // Update SOL fee config if any values provided
    if new_protocol_fee_pct.is_some() || new_buyback_pct.is_some() || new_stakers_pct.is_some() {
        msg!("   Updating SOL fee config...");
        let protocol_fee_pct =
            new_protocol_fee_pct.unwrap_or(global_config.sol_fee_config.protocol_fee_pct);
        let buyback_pct = new_buyback_pct.unwrap_or(global_config.sol_fee_config.buyback_pct);
        let stakers_pct = new_stakers_pct.unwrap_or(global_config.sol_fee_config.stakers_pct);

        global_config.sol_fee_config = SolFeeConfig {
            protocol_fee_pct,
            buyback_pct,
            stakers_pct,
        };
        msg!(
            "     Updated: protocol fee -> {}%, buyback -> {}%, stakers -> {}%",
            protocol_fee_pct,
            buyback_pct,
            stakers_pct
        );
    } else {
        msg!("   SOL fee config: not updated");
    }

    // Update MineBtc distribution config if any values provided
    if new_minebtc_stakers_pct.is_some()
        || new_minebtc_winners_pct.is_some()
        || new_minebtc_same_faction_pct.is_some()
        || new_minebtc_motherlode_pct.is_some()
    {
        msg!("   Updating MineBtc distribution config...");
        let minebtc_stakers_pct = new_minebtc_stakers_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_stakers_pct);
        let minebtc_winners_pct = new_minebtc_winners_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_winners_pct);
        let minebtc_same_faction_pct = new_minebtc_same_faction_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_same_faction_pct);
        let minebtc_motherlode_pct = new_minebtc_motherlode_pct
            .unwrap_or(global_config.minebtc_dist_config.minebtc_motherlode_pct);

        msg!(
            "     New values: stakers={}%, winners={}%, same_faction={}%, motherlode={}%",
            minebtc_stakers_pct,
            minebtc_winners_pct,
            minebtc_same_faction_pct,
            minebtc_motherlode_pct
        );
        let total = minebtc_stakers_pct as u16
            + minebtc_winners_pct as u16
            + minebtc_same_faction_pct as u16
            + minebtc_motherlode_pct as u16;
        msg!("     Total: {}%", total);

        require!(total == 100, ErrorCode::InvalidParameters);
        msg!("     ✓ Percentages sum to 100%");

        // Get current refining_fee to preserve it
        let current_refining_fee = global_config.minebtc_dist_config.refining_fee;
        msg!("     Preserving refining_fee: {}%", current_refining_fee);

        let old_config = global_config.minebtc_dist_config.clone();
        global_config.minebtc_dist_config = MineBtcDistConfig {
            minebtc_stakers_pct,
            minebtc_winners_pct,
            minebtc_same_faction_pct,
            minebtc_motherlode_pct,
            refining_fee: current_refining_fee,
        };
        msg!("     Updated: stakers {}% -> {}%, winners {}% -> {}%, same_faction {}% -> {}%, motherlode {}% -> {}%",
            old_config.minebtc_stakers_pct, minebtc_stakers_pct,
            old_config.minebtc_winners_pct, minebtc_winners_pct,
            old_config.minebtc_same_faction_pct, minebtc_same_faction_pct,
            old_config.minebtc_motherlode_pct, minebtc_motherlode_pct
        );
    } else {
        msg!("   MineBtc dist config: not updated");
    }

    // Update refining fee if provided
    if let Some(refining_fee) = new_refining_fee {
        let old_refining_fee = global_config.minebtc_dist_config.refining_fee;
        global_config.minebtc_dist_config.refining_fee = refining_fee;
        msg!(
            "   Updated refining fee: {}% -> {}%",
            old_refining_fee,
            refining_fee
        );
    } else {
        msg!(
            "   Refining fee: {}% (not updated)",
            global_config.minebtc_dist_config.refining_fee
        );
    }

    // Update change faction fee if provided
    if let Some(change_faction_fee) = change_faction_fee {
        global_config.change_faction_fee = change_faction_fee;
        msg!(
            "   Updated change faction fee: {} SOL -> {} SOL",
            global_config.change_faction_fee,
            change_faction_fee
        );
    } else {
        msg!(
            "   Change faction fee: {} SOL (not updated)",
            global_config.change_faction_fee
        );
    }

    // Update snapshot interval if provided
    if let Some(snapshot_interval) = snapshot_interval {
        let old_interval = global_config.snapshot_interval;
        global_config.snapshot_interval = snapshot_interval;
        msg!(
            "   Updated snapshot interval: {} seconds -> {} seconds",
            old_interval,
            snapshot_interval
        );
    } else {
        msg!(
            "   Snapshot interval: {} seconds (not updated)",
            global_config.snapshot_interval
        );
    }

    msg!("✅ [update_fees_internal] Fee configuration updated successfully");
    Ok(())
}

/// Toggle RPG progression (mutations, XP) during gameplay
pub fn update_rpg_progression_internal(ctx: Context<UpdateConfigAc>, enabled: bool) -> Result<()> {
    msg!("🎮 [update_rpg_progression] Setting rpg_progression to {}", enabled);
    ctx.accounts.global_config.rpg_progression = enabled;
    msg!("✅ RPG progression updated");
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
    msg!("📊 [update_emission_params_internal] Updating emission adjustment parameters");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    msg!("   Current emission adjustment parameters:");
    msg!(
        "     Price change threshold: {}%",
        mine_btc_mining.price_change_threshold
    );
    msg!(
        "     Emission increase: {}%",
        mine_btc_mining.emission_increase_pct
    );
    msg!(
        "     Emission decrease: {}%",
        mine_btc_mining.emission_decrease_pct
    );

    // Update price change threshold if provided
    if let Some(threshold) = price_change_threshold {
        require!(
            threshold > 0 && threshold <= 100,
            ErrorCode::InvalidParameters
        );
        let old_threshold = mine_btc_mining.price_change_threshold;
        mine_btc_mining.price_change_threshold = threshold;
        msg!(
            "   Updated price change threshold: {}% -> {}%",
            old_threshold,
            threshold
        );
    } else {
        msg!(
            "   Price change threshold: {}% (not updated)",
            mine_btc_mining.price_change_threshold
        );
    }

    // Update emission increase percentage if provided
    if let Some(increase_pct) = emission_increase_pct {
        require!(
            increase_pct > 0 && increase_pct <= 100,
            ErrorCode::InvalidParameters
        );
        let old_increase = mine_btc_mining.emission_increase_pct;
        mine_btc_mining.emission_increase_pct = increase_pct;
        msg!(
            "   Updated emission increase: {}% -> {}%",
            old_increase,
            increase_pct
        );
    } else {
        msg!(
            "   Emission increase: {}% (not updated)",
            mine_btc_mining.emission_increase_pct
        );
    }

    // Update emission decrease percentage if provided
    if let Some(decrease_pct) = emission_decrease_pct {
        require!(
            decrease_pct > 0 && decrease_pct <= 100,
            ErrorCode::InvalidParameters
        );
        let old_decrease = mine_btc_mining.emission_decrease_pct;
        mine_btc_mining.emission_decrease_pct = decrease_pct;
        msg!(
            "   Updated emission decrease: {}% -> {}%",
            old_decrease,
            decrease_pct
        );
    } else {
        msg!(
            "   Emission decrease: {}% (not updated)",
            mine_btc_mining.emission_decrease_pct
        );
    }

    msg!(
        "✅ [update_emission_params_internal] Emission adjustment parameters updated successfully"
    );
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
    let mine_btc_mining = &mut ctx.accounts.mine_btc_mining;

    // Check mining hasn't been initialized yet
    require!(
        mine_btc_mining.mining_start_timestamp == 0,
        ErrorCode::MiningAlreadyInitialized
    );

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

    msg!("Initialized dynamic distribution system (30min snapshots, 4hr cycles) with Raydium pool: {}", pool_state);

    // Emit event
    emit!(MiningTokenVaultSet {
        authority: ctx.accounts.authority.key(),
        token_vault: ctx.accounts.token_vault.key(),
        token_vault_authority: ctx.accounts.vault_authority.key(),
        mining_start_timestamp: start_timestamp,
    });

    msg!(
        "Mining initialized with token vault: {}",
        ctx.accounts.token_vault.key()
    );

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

    msg!("Deposited {} MDOGE into mining vault", amount);
    Ok(())
}

// ----------------------------------------------------------------------------------------
// -------------- HASHPOWER CONFIG (ADMIN) -------------------------------------------------
// ----------------------------------------------------------------------------------------

pub fn initialize_hashpower_config_internal(
    ctx: Context<InitializeHashpowerConfig>,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    let hashpower_config = &mut ctx.accounts.hashpower_config;

    hashpower_config.min_lockup_days = min_lockup_days;
    hashpower_config.max_lockup_days = max_lockup_days;
    hashpower_config.base_multiplier = base_multiplier;
    hashpower_config.max_multiplier = max_multiplier;
    hashpower_config.bump = ctx.bumps.hashpower_config;

    msg!("✅ [initialize_hashpower_config_internal] Hashpower config initialized successfully");
    Ok(())
}

pub fn update_hashpower_config_internal(
    ctx: Context<UpdateHashpowerConfig>,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    let hashpower_config = &mut ctx.accounts.hashpower_config;

    hashpower_config.min_lockup_days = min_lockup_days;
    hashpower_config.max_lockup_days = max_lockup_days;
    hashpower_config.base_multiplier = base_multiplier;
    hashpower_config.max_multiplier = max_multiplier;
    hashpower_config.bump = ctx.bumps.hashpower_config;
    msg!("✅ [update_hashpower_config_internal] Hashpower config updated successfully");
    Ok(())
}

// ----------------------------------------------------------------------------------------
// --------------  DOGE URI MANAGEMENT (ADMIN) ---------------------------------------
// ----------------------------------------------------------------------------------------

/// Initialize EggConfig account (admin only)
///
/// Creates the EggConfig account that stores Doge collection configuration.
/// This must be called before creating the Doge collection.
///
/// # Parameters
/// - `base_price`: Base price for Doge in SOL (lamports)
/// - `curve_a`: Bonding curve parameter (controls price growth rate)
/// - `max_supply`: Maximum number of Doge that can be minted
pub fn initialize_doge_config_internal(
    ctx: Context<InitializeEggConfig>,
    base_price: u64,
    curve_a: u64,
    max_supply: u64,
) -> Result<()> {
    msg!("🥚 [initialize_doge_config_internal] Initializing EggConfig");

    let eggs_config = &mut ctx.accounts.eggs_config;

    eggs_config.bump = ctx.bumps.eggs_config;
    eggs_config.egg_collection = Pubkey::default();
    eggs_config.eggs_minted = 0;
    eggs_config.base_price = base_price;
    eggs_config.curve_a = curve_a;
    eggs_config.max_supply = max_supply;
    eggs_config.egg_uris = Vec::new();
    eggs_config.ticket_tiers = Vec::new();
    eggs_config.breeding_allowed = false;
    eggs_config.breed_base_price = 0;
    eggs_config.breed_curve_a = 100;

    msg!("   ✅ EggConfig initialized");
    msg!("   Base Price: {} lamports, Max Supply: {}", base_price, max_supply);

    Ok(())
}

/// Create Doge collection with program PDA as authority (admin only)
///
/// Creates a new Metaplex Core collection for Doge NFTs.
/// The collection's update authority is set to a program-controlled PDA.
/// Requires EggConfig to be initialized first.
///
/// # Parameters
/// - `name`: Collection name
/// - `uri`: Collection metadata URI
pub fn create_doge_collection_internal(
    ctx: Context<CreateEggCollection>,
    name: String,
    uri: String,
) -> Result<()> {
    let eggs_config = &mut ctx.accounts.eggs_config;

    msg!("Creating Doge collection with program PDA as update authority");
    msg!("Collection: {}", ctx.accounts.collection.key());
    msg!(
        "Collection Authority PDA: {}",
        ctx.accounts.collection_authority.key()
    );

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
    eggs_config.egg_collection = ctx.accounts.collection.key();

    emit!(EggCollectionCreated {
        collection: ctx.accounts.collection.key(),
        update_authority: ctx.accounts.collection_authority.key(),
        name,
        uri,
    });

    Ok(())
}

/// Set Doge URIs for all factions (admin only)
///
/// Sets the metadata URIs for Doge, one URI per faction.
/// The number of URIs must match the number of supported factions.
///
/// # Parameters
/// - `uris`: Vector of URIs, one per faction (must match `supported_factions.len()`)
pub fn set_doge_uris_internal(ctx: Context<UpdateDogeConfig>, uris: Vec<String>) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let eggs_config = &mut ctx.accounts.eggs_config;

    require!(
        uris.len() == global_config.supported_factions.len(),
        ErrorCode::InvalidParameters
    );

    // Validate URIs
    for uri in &uris {
        require!(uri.len() <= MAX_URI_LENGTH, ErrorCode::UriTooLong);
    }

    // Set URIs for all factions
    eggs_config.egg_uris = uris.clone();

    msg!("✅ Set {} Doge URIs (one per faction)", uris.len());
    msg!("   Factions: {}", global_config.supported_factions.len());

    Ok(())
}

/// Clear all Doge URIs (admin only)
///
/// Removes all Doge metadata URIs from the configuration.
/// This can be used to reset URIs before setting new ones.
pub fn clear_doge_uris_internal(ctx: Context<UpdateDogeConfig>) -> Result<()> {
    let eggs_config = &mut ctx.accounts.eggs_config;
    eggs_config.egg_uris.clear();

    msg!("✅ Cleared all Doge URIs");

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
    ctx: Context<InitEggRoyalties>,
    basis_points: u16,
    creators: Vec<CreatorInput>,
) -> Result<()> {
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
    require!(total_pct == 100, ErrorCode::InvalidCreatorShare);

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
    let signer_seeds: &[&[&[u8]]] = &[&seeds];

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

    msg!(
        "✅ Initialized Doge royalties: {} basis points",
        basis_points
    );
    Ok(())
}

/// Add or update ticket tier configs (admin only)
///
/// Configures ticket tier options that users can choose when minting Doge.
/// Users receive free tickets based on the selected tier when they mint.
///
/// # Parameters
/// - `ticket_tier_index`: Index of the ticket tier (0-3, max 4 tiers)
/// - `ticket_value`: Value of each ticket in lamports (e.g., 10_000_000 = 0.01 SOL)
/// - `ticket_count`: Number of tickets given with this tier
///
/// # Example
/// - Tier 0: 0.01 SOL × 1000 tickets
/// - Tier 1: 0.1 SOL × 10 tickets
pub fn add_ticket_tier_config_int(
    ctx: Context<UpdateDogeConfig>,
    ticket_tier_index: u8,
    ticket_value: u64,
) -> Result<()> {
    let global_config = &ctx.accounts.global_config;
    let eggs_config = &mut ctx.accounts.eggs_config;
    let authority = &ctx.accounts.authority;

    // Authority check
    require!(
        global_config.ext_authority == authority.key(),
        ErrorCode::Unauthorized
    );

    require!(
        ticket_tier_index < EggConfig::MAX_TICKET_TIERS as u8,
        ErrorCode::InvalidParameters
    );

    let tier_index = ticket_tier_index as usize;

    // Ensure vector is large enough
    while eggs_config.ticket_tiers.len() <= tier_index {
        eggs_config.ticket_tiers.push(TicketTier {
            ticket_value: 0
        });
    }

    // Update or add ticket tier
    eggs_config.ticket_tiers[tier_index] = TicketTier {
        ticket_value,
    };

    msg!(
        "✅ Updated ticket tier config #{}: {} SOL",
        ticket_tier_index,
        ticket_value as f64 / 1e9
    );
    Ok(())
}

/// Update EggConfig account (admin only)
///
/// Updates the EggConfig account that stores Doge collection configuration.
///
/// # Parameters
/// - `base_price`: Base price for Doge in SOL (lamports)
/// - `curve_a`: Bonding curve parameter (controls price growth rate)
pub fn update_doge_config_internal(
    ctx: Context<UpdateDogeConfig>,
    base_price: u64,
    curve_a: u64,
) -> Result<()> {
    msg!("🥚 [update_doge_config_internal] Updating EggConfig");

    let eggs_config = &mut ctx.accounts.eggs_config;
    eggs_config.base_price = base_price;
    eggs_config.curve_a = curve_a;

    msg!("   ✅ EggConfig updated");
    msg!("   Base Price: {} lamports", base_price);
    msg!("   Curve A: {}", curve_a);

    Ok(())
}

/// Update breeding config (admin only)
pub fn update_breeding_config_internal(
    ctx: Context<UpdateDogeConfig>,
    breeding_allowed: bool,
    breed_base_price: u64,
    breed_curve_a: u64,
) -> Result<()> {
    msg!("🧬 [update_breeding_config] Updating breeding config");
    let eggs_config = &mut ctx.accounts.eggs_config;
    eggs_config.breeding_allowed = breeding_allowed;
    eggs_config.breed_base_price = breed_base_price;
    eggs_config.breed_curve_a = breed_curve_a;
    msg!("   ✅ Breeding: {}, Base: {} SOL, CurveA: {}", breeding_allowed, breed_base_price as f64 / 1e9, breed_curve_a);
    Ok(())
}

// --------------------------------------------------------------------------------
// ------------ FACTION SURGE GAME STATE INITIALIZATION ---------------------------
// --------------------------------------------------------------------------------

/// Initialize the global game state for Faction Surge (admin only)
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
    msg!("🎮 [initialize_game_state_internal] Initializing global game state");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Round duration: {} seconds", round_duration_seconds);

    let global_game_state = &mut ctx.accounts.global_game_state;
    let clock = Clock::get()?;

    msg!("   Current timestamp: {}", clock.unix_timestamp);

    // Initialize game state
    msg!("   Initializing game state fields...");
    global_game_state.bump = ctx.bumps.global_game_state;
    global_game_state.is_active = true;
    global_game_state.can_begin_round = true;

    global_game_state.current_round_id = 0; // Will be incremented to 1 in start_round
    global_game_state.round_end_timestamp = 0;
    global_game_state.round_duration_seconds = round_duration_seconds;
    msg!("     Bump: {}", global_game_state.bump);
    msg!("     Is active: {}", global_game_state.is_active);
    msg!(
        "     Current round ID: {} (will be incremented to 1 in start_round)",
        global_game_state.current_round_id
    );
    msg!(
        "     Round duration: {} seconds",
        global_game_state.round_duration_seconds
    );
    msg!(
        "     Round end timestamp: {}",
        global_game_state.round_end_timestamp
    );

    // Initialize previous round data
    global_game_state.last_round_id = 0;
    global_game_state.winning_faction_id = 0;
    msg!("     Last round ID: {}", global_game_state.last_round_id);
    msg!(
        "     Winning faction ID: {} (no rounds completed yet)",
        global_game_state.winning_faction_id
    );

    // Initialize commit-reveal randomness fields
    global_game_state.current_round_commit = [0u8; 32]; // Will be set in start_round
    global_game_state.current_round_seed = None;
    msg!(
        "     Current round commit: {:?} (will be set in start_round)",
        global_game_state.current_round_commit
    );
    msg!(
        "     Current round seed: {:?} (will be set in end_round)",
        global_game_state.current_round_seed
    );

    // Initialize cumulative stats
    global_game_state.total_sol_bets = 0;
    msg!("     Total SOL bets: {}", global_game_state.total_sol_bets);

    // Initialize empty cranker bots whitelist
    global_game_state.cranker_bots = Vec::new();
    msg!(
        "     Cranker bots whitelist: {} (empty, must be added via admin)",
        global_game_state.cranker_bots.len()
    );

    msg!("✅ [initialize_game_state_internal] Global game state initialized successfully");
    msg!("   Round duration: {} seconds", round_duration_seconds);
    msg!(
        "   First round ends at: {}",
        global_game_state.round_end_timestamp
    );

    Ok(())
}

/// Add a cranker bot to the whitelist (admin only)
///
/// Adds a bot address to the whitelist of authorized cranker bots.
/// Only whitelisted bots can call `start_round` and `end_round` functions.
/// Maximum of MAX_CRANKER_BOTS (3) bots can be whitelisted.
///
/// # Parameters
/// - `bot_pubkey`: Public key of the bot to whitelist
pub fn add_cranker_bot_internal(ctx: Context<UpdateGameState>, bot_pubkey: Pubkey) -> Result<()> {
    msg!("🤖 [add_cranker_bot_internal] Adding cranker bot to whitelist");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Bot pubkey: {}", bot_pubkey);

    let global_game_state = &mut ctx.accounts.global_game_state;

    msg!(
        "   Current cranker bots count: {}",
        global_game_state.cranker_bots.len()
    );
    msg!("   Maximum allowed: {}", MAX_CRANKER_BOTS);

    // Check if bot is already whitelisted
    require!(
        !global_game_state.cranker_bots.contains(&bot_pubkey),
        ErrorCode::InvalidParameters // Bot already whitelisted
    );
    msg!("     ✓ Bot not already whitelisted");

    // Check if we've reached the maximum
    require!(
        global_game_state.cranker_bots.len() < MAX_CRANKER_BOTS,
        ErrorCode::InvalidParameters // Max cranker bots reached
    );
    msg!("     ✓ Under maximum limit");

    // Add bot to whitelist
    global_game_state.cranker_bots.push(bot_pubkey);
    msg!("   Added bot to whitelist");
    msg!(
        "     New cranker bots count: {}",
        global_game_state.cranker_bots.len()
    );

    msg!("✅ [add_cranker_bot_internal] Cranker bot added successfully");
    msg!("   Bot: {}", bot_pubkey);
    msg!(
        "   Total whitelisted bots: {}",
        global_game_state.cranker_bots.len()
    );

    Ok(())
}

/// Remove a cranker bot from the whitelist (admin only)
///
/// Removes a bot address from the whitelist of authorized cranker bots.
///
/// # Parameters
/// - `bot_pubkey`: Public key of the bot to remove from whitelist
pub fn remove_cranker_bot_internal(
    ctx: Context<UpdateGameState>,
    bot_pubkey: Pubkey,
) -> Result<()> {
    msg!("🤖 [remove_cranker_bot_internal] Removing cranker bot from whitelist");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Bot pubkey: {}", bot_pubkey);

    let global_game_state = &mut ctx.accounts.global_game_state;

    msg!(
        "   Current cranker bots count: {}",
        global_game_state.cranker_bots.len()
    );

    // Find and remove bot
    let initial_count = global_game_state.cranker_bots.len();
    global_game_state
        .cranker_bots
        .retain(|&bot| bot != bot_pubkey);

    require!(
        global_game_state.cranker_bots.len() < initial_count,
        ErrorCode::InvalidParameters // Bot not found in whitelist
    );
    msg!("     ✓ Bot removed from whitelist");
    msg!(
        "     New cranker bots count: {}",
        global_game_state.cranker_bots.len()
    );

    msg!("✅ [remove_cranker_bot_internal] Cranker bot removed successfully");
    msg!("   Bot: {}", bot_pubkey);
    msg!(
        "   Remaining whitelisted bots: {}",
        global_game_state.cranker_bots.len()
    );

    Ok(())
}

/// Switch game state (toggle is_active) (admin only)
///
/// Toggles the `is_active` field in the global game state.
/// When `is_active` is false, rounds cannot be started or ended.
///
/// This allows admins to pause/resume the game without losing state.
pub fn switch_game_state_internal(ctx: Context<UpdateGameState>) -> Result<()> {
    msg!("🔄 [switch_game_state_internal] Switching game state");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let global_game_state = &mut ctx.accounts.global_game_state;

    let old_state = global_game_state.is_active;
    global_game_state.is_active = !global_game_state.is_active;
    let new_state = global_game_state.is_active;

    msg!("   Game state: {} -> {}", old_state, new_state);

    if new_state {
        msg!("   ✅ Game is now ACTIVE - rounds can be started/ended");
    } else {
        msg!("   ⏸️  Game is now PAUSED - rounds cannot be started/ended");
    }

    msg!("✅ [switch_game_state_internal] Game state switched successfully");
    msg!(
        "   New state: {}",
        if new_state { "ACTIVE" } else { "PAUSED" }
    );

    Ok(())
}

/// Update round duration (admin only)
///
/// Updates the `round_duration_seconds` field in the global game state.
/// This controls how long each game round lasts.
///
/// # Parameters
/// - `new_round_duration_seconds`: New round duration in seconds (must be > 0)
pub fn update_round_duration_internal(
    ctx: Context<UpdateGameState>,
    new_round_duration_seconds: i64,
) -> Result<()> {
    msg!("⏱️ [update_round_duration_internal] Updating round duration");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let global_game_state = &mut ctx.accounts.global_game_state;
    require!(new_round_duration_seconds > 0, ErrorCode::InvalidParameters);
    global_game_state.round_duration_seconds = new_round_duration_seconds;
    msg!("✅ [update_round_duration_internal] Round duration updated successfully");
    msg!(
        "   New duration: {} seconds ({} minutes)",
        new_round_duration_seconds,
        new_round_duration_seconds / 60
    );

    Ok(())
}

// ----------------------------------------------------------------------------------------
// ------------ SYSTEM ACCOUNTS INITIALIZATION ----------------------------------
// ----------------------------------------------------------------------------------------

/// Initialize system referral account and buybacks system (admin only)
///
/// Creates and initializes both the system referral rewards account and the buybacks tracking account.
/// The system referral account tracks rewards for the system referral code.
/// The buybacks account tracks SOL accumulated for token buybacks.
///
/// # Initializes
/// - System referral rewards PDA
/// - Buybacks account PDA
/// - Buybacks SOL vault PDA
pub fn initialize_system_accounts_internal(ctx: Context<InitializeSystemAccounts>) -> Result<()> {
    msg!("🔧 [initialize_system_accounts_internal] Initializing system accounts");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    // Initialize system referral rewards account
    msg!("   Initializing system referral rewards account...");
    let system_referral = &mut ctx.accounts.system_referral_rewards;
    system_referral.owner = ctx.accounts.system_program.key();
    system_referral.bump = ctx.bumps.system_referral_rewards;
    system_referral.referrals_count = 0;
    system_referral.pending_sol_rewards = 0;
    system_referral.pending_minebtc_rewards = 0;
    system_referral.total_sol_earned = 0;
    system_referral.total_minebtc_earned = 0;
    msg!("     System referral account initialized");
    msg!("     Owner: {}", system_referral.owner);
    msg!("     Bump: {}", system_referral.bump);

    // Initialize buybacks account
    msg!("   Initializing buybacks account...");
    let buybacks_ac = &mut ctx.accounts.buybacks_account;
    buybacks_ac.total_sol_accumulated = 0;
    msg!("     Buybacks account initialized");
    msg!(
        "     Total SOL accumulated: {}",
        buybacks_ac.total_sol_accumulated
    );

    msg!("✅ [initialize_system_accounts_internal] System accounts initialized successfully");
    msg!(
        "   System referral rewards PDA: {}",
        ctx.accounts.system_referral_rewards.key()
    );
    msg!(
        "   Buybacks account PDA: {}",
        ctx.accounts.buybacks_account.key()
    );
    msg!(
        "   Buybacks SOL vault PDA: {}",
        ctx.accounts.buybacks_sol_vault.key()
    );

    Ok(())
}

// ----------------------------------------------------------------------------------------
// ------------ WITHDRAW SOL FEES ----------------------------------
// ----------------------------------------------------------------------------------------

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
        space = UnrefinedRewards::LEN,
        seeds = [UNREFINED_REWARDS_SEED.as_ref()],
        bump
    )]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

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

    /// CHECK: 0-byte PDA that only stores lamports (System Account) for doge minting fees
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [DOGES_TREASURY_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub eggs_treasury: UncheckedAccount<'info>,

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
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

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

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump,
    )]
    pub mine_btc_mining: Option<Account<'info, MineBtcMining>>,

    #[account(mut)]
    pub authority: Signer<'info>,

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
pub struct InitializeEggConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = EggConfig::LEN,
        seeds = [DOGE_CONFIG_SEED.as_ref()],
        bump
    )]
    pub eggs_config: Account<'info, EggConfig>,

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
pub struct CreateEggCollection<'info> {
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
        bump = eggs_config.bump,
    )]
    pub eggs_config: Account<'info, EggConfig>,

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
        bump = eggs_config.bump,
    )]
    pub eggs_config: Account<'info, EggConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitEggRoyalties<'info> {
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
        bump = eggs_config.bump,
    )]
    pub eggs_config: Account<'info, EggConfig>,

    /// CHECK: Doge collection (already created via MPL Core)
    #[account(
        mut,
        address = eggs_config.egg_collection @ ErrorCode::InvalidAccount
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

    /// System referral rewards account (can be initialized by anyone)
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
    msg!("🔧 [initialize_custodian_accounts] Initializing custodian token accounts");

    // Verify MINEBTC custodian
    msg!("   MINEBTC Custodian:");
    msg!("     Mint: {}", ctx.accounts.minebtc_mint.key());
    msg!(
        "     Custodian PDA: {}",
        ctx.accounts.minebtc_custodian.key()
    );
    msg!(
        "     Authority PDA: {}",
        ctx.accounts.minebtc_custodian_authority.key()
    );
    require!(
        ctx.accounts.minebtc_custodian.mint == ctx.accounts.minebtc_mint.key(),
        ErrorCode::InvalidMint
    );
    require!(
        ctx.accounts.minebtc_custodian.owner == ctx.accounts.minebtc_custodian_authority.key(),
        ErrorCode::InvalidOwner
    );

    // Verify liquidity custodian
    msg!("   Liquidity Custodian:");
    msg!("     Mint: {}", ctx.accounts.lp_mint.key());
    msg!(
        "     Custodian PDA: {}",
        ctx.accounts.liquidity_custodian.key()
    );
    msg!(
        "     Authority PDA: {}",
        ctx.accounts.liquidity_custodian_authority.key()
    );
    require!(
        ctx.accounts.liquidity_custodian.mint == ctx.accounts.lp_mint.key(),
        ErrorCode::InvalidMint
    );
    require!(
        ctx.accounts.liquidity_custodian.owner == ctx.accounts.liquidity_custodian_authority.key(),
        ErrorCode::InvalidOwner
    );

    msg!(
        "✅ [initialize_custodian_accounts] Both custodian token accounts initialized successfully"
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
