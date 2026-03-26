// # User Instructions
//
// This module handles all user-facing interactions in the MineBTC Faction Surge game.
//
// ## Key Functions
//
// - `initialize_player`: Creates a new player account and assigns them to a faction.
// - `change_faction`: Allows players to switch factions (requires no active stakes).
// - `join_round`: Places a bet on a block or faction for the current round.
// - `join_round_batch`: Places multiple bets in a single transaction.
// - `claim_round_rewards`: Claims winnings from completed rounds.
// - `init_autominer`: Sets up an automated betting system for recurring bets.
// - `execute_autominer_bet`: Executes an autominer bet (keeper function).
//
// Players can earn rewards through winning bets, same-faction bonuses, motherlode jackpots, and referrals.
//

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::Token;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::genescience::{calculate_mutation_result, MutationType};
use crate::instructions::helper;
use crate::instructions::stake;
use crate::state::*;

// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn internal_initialize_player(
    ctx: Context<InitializePlayer>,
    faction_id: u8,
    referral_code: Option<Pubkey>,
) -> Result<()> {
    msg!(
        "👤 [initialize_player] Initializing player account. Authority: {}. Faction ID: {}",
        ctx.accounts.authority.key(),
        faction_id
    );

    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;
    global_config.total_players = global_config.total_players + 1;

    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );

    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;

    // Handle referral code logic
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        msg!("     Referral code provided: {}", ref_code);
        require!(
            ref_code != ctx.accounts.authority.key(),
            ErrorCode::ReferralCannotBeSameAsOwner
        );

        // Update referrer's referral count if referrer_rewards account is provided
        if let Some(ref mut referrer_rewards) = ctx.accounts.referrer_rewards {
            require!(
                referrer_rewards.owner == ref_code,
                ErrorCode::InvalidReferralAccount
            );
            require!(
                referrer_rewards.referrals_count < stake::MAX_REFERRALS_PER_CODE,
                ErrorCode::MaxReferralsReached
            );
            referrer_rewards.referrals_count = referrer_rewards.referrals_count + 1;
            msg!(
                "     Referrer's referral count: {}/{}",
                referrer_rewards.referrals_count,
                stake::MAX_REFERRALS_PER_CODE
            );
        }

        // Set player's referral code
        player_data.referral_code = ref_code;
        ref_code
    } else {
        msg!("     No referral code provided, using system referral account");
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };

    // Initialize statistics
    player_data.rounds_played = 0;
    player_data.active_multiplier = BASE_MULTIPLIER as u32;

    player_data.total_sol_bet = 0;
    player_data.total_points_bet = 0;
    player_data.total_sol_won = 0;
    player_data.total_dogebtc_won = 0;

    // Initialize MineBtc staking fields
    player_data.dogebtc_hashpower = 0;
    player_data.dogebtc_staked = 0;
    player_data.dogebtc_dogebtc_reward_debt = 0;
    player_data.dogebtc_sol_reward_debt = 0;
    msg!("     MineBtc staking fields initialized");

    // Initialize LP staking fields
    player_data.lp_hashpower = 0;
    player_data.lp_staked = 0;
    player_data.lp_sol_reward_debt = 0;
    player_data.lp_dogebtc_reward_debt = 0;
    msg!("     LP staking fields initialized");

    // Initialize pending rewards
    player_data.pending_sol_rewards = 0;
    player_data.pending_minebtc_rewards = 0;
    msg!("     Pending rewards initialized");

    // Initialize position tracking vectors
    player_data.dogebtc_position_indices = Vec::new();
    player_data.lp_position_indices = Vec::new();
    msg!("     Position tracking initialized");

    // Initialize doge staking
    player_data.staked_doges = Vec::new();
    player_data.doge_multiplier = BASE_MULTIPLIER as u16; // Default 1.0x (no doges staked)
    msg!("     Doge staking initialized (0 doges, 1.0x multiplier)");

    // Initialize free tickets vectors
    player_data.free_tickets = Vec::new();
    player_data.free_tickets_remaining = Vec::new();
    msg!("     Free tickets vectors initialized (empty)");

    // Initialize new player's referral rewards account
    msg!("   Initializing new player's referral rewards account...");
    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.pending_minebtc_rewards = 0;
    new_player_rewards.total_minebtc_earned = 0;
    msg!("     Referral rewards account initialized");

    msg!("✅ [initialize_player] Player initialized successfully");
    msg!(
        "   Player: {} for faction {}",
        ctx.accounts.authority.key(),
        faction_id
    );
    if referral_code.is_some() {
        msg!("   Referral code: {}", referrer_pubkey);
    } else {
        msg!("   Using system referral account: {}", referrer_pubkey);
    }

    emit!(PlayerInitialized {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        faction_id,
        referral_code,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Change user's faction
/// Requires:
/// - No minebtc hashpower (dogebtc_hashpower == 0)
/// - No lp hashpower (lp_hashpower == 0)
/// - No doges staked (staked_doges.is_empty())
/// Charges change_faction_fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
pub fn internal_change_faction(ctx: Context<ChangeFaction>, new_faction_id: u8) -> Result<()> {
    msg!(
        "🔄 [change_faction] User changing faction. User: {}",
        ctx.accounts.authority.key()
    );
    msg!(
        "   Current faction ID: {}. New faction ID: {}",
        ctx.accounts.player_data.faction_id,
        new_faction_id
    );

    let player_data = &mut ctx.accounts.player_data;
    let global_config = &ctx.accounts.global_config;

    // Validate new faction_id
    require!(
        (new_faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.faction_id != new_faction_id,
        ErrorCode::InvalidParameters
    );

    // Validate user has no staked positions
    msg!("   Validating user has no staked positions...");
    require!(
        player_data.dogebtc_hashpower == 0
            && player_data.lp_hashpower == 0
            && player_data.staked_doges.is_empty(),
        ErrorCode::InvalidParameters
    );

    // Charge change_faction_fee
    let change_fee = global_config.change_faction_fee;
    require!(change_fee > 0, ErrorCode::InvalidAmount);
    msg!("   Change faction fee: {} SOL", (change_fee as f64 / 1e9));

    // Split fee: 50% to sol_treasury, 50% to fee_recipient (as WSOL)
    let treasury_amt = change_fee / 2;
    let dev_amt = change_fee - treasury_amt;

    msg!(
        "   Transferring {} SOL to sol_treasury",
        (treasury_amt as f64 / 1e9)
    );
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        treasury_amt,
    )?;

    msg!(
        "   Transferring {} SOL to fee_recipient (as WSOL)",
        (dev_amt as f64 / 1e9)
    );
    helper::transfer_wsol_to_multisig(
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.multisig_wsol_account.to_account_info(),
        &ctx.accounts.user_wsol_account.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        dev_amt,
    )?;

    // Update faction_id
    let old_faction_id = player_data.faction_id;
    player_data.faction_id = new_faction_id;

    msg!("✅ [change_faction] Faction changed successfully");
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!(
        "   Old faction ID: {} -> New faction ID: {}",
        old_faction_id,
        new_faction_id
    );

    emit!(FactionChanged {
        user: ctx.accounts.authority.key(),
        player_data: ctx.accounts.player_data.key(),
        new_faction_id,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Join a round by betting SOL or using free tickets (single bet)
/// Users can bet on either:
/// - A specific block (block_id: 0-23, 0-indexed)
/// - A faction + highest/lowest option (faction_id + is_highest)
///
/// Parameters:
/// - amount: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - bet_type: The bet type (Block, FactionHighestLowest, FactionBoth, or RandomBlock)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
///
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn internal_join_round(
    ctx: Context<JoinRound>,
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!(
        "🎲 [join_round] User joining round (single bet). User: {}",
        ctx.accounts.authority.key()
    );
    msg!("   Bet type: {:?}", bet_type);

    // Call internal_process_bets with user as payer (None for signer_seeds - user signs the tx)
    // Wrap single bet in vector
    internal_process_bets(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.game_session,
        &mut ctx.accounts.user_game_bet,
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.sol_rewards_vault.to_account_info(),
        &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.user_game_bet,
        ctx.accounts.authority.key(),
        amount,
        vec![bet_type.clone()],
        use_ticket,
        None, // User wallet signs the transaction
        None, // No autominer info
        &mut ctx.accounts.epoch_config,
        &mut ctx.accounts.epoch_state,
        &mut ctx.accounts.user_epoch_bets,
        ctx.bumps.user_epoch_bets,
    )?;

    msg!("✅ [join_round] Bet placed successfully");
    Ok(())
}

/// Join a round with multiple bets in a single transaction
/// Users can bet on:
/// - Multiple blocks (e.g., [0, 4, 9, 14] - 0-indexed: 0-23)
/// - Multiple factions with settings: "low", "high", "both", or "random"
///
/// Parameters:
/// - bet_types: Vector of bet types to place (all must be for the same faction)
/// - amount_per_bet: Bet amount per bet type in lamports (for SOL) or points (for tickets)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
///
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn internal_join_round_batch(
    ctx: Context<JoinRoundBatch>,
    bet_types: Vec<BetType>,
    amount_per_bet: u64,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!(
        "🎲 [join_round_batch] User joining round with {} bets",
        bet_types.len()
    );
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!("   Amount per bet: {} lamports", amount_per_bet);

    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    require!(bet_types.len() <= 24, ErrorCode::InvalidParameters); // Max 24 bets (one per block)

    // Expand bet types (handle FactionBoth and RandomBlock)
    let mut expanded_bet_types = Vec::new();
    for bet_type in bet_types.iter() {
        match bet_type {
            BetType::FactionBoth { faction_id } => {
                // Expand to both highest and lowest
                expanded_bet_types.push(BetType::FactionHighestLowest {
                    faction_id: *faction_id,
                    is_highest: true,
                });
                expanded_bet_types.push(BetType::FactionHighestLowest {
                    faction_id: *faction_id,
                    is_highest: false,
                });
            }
            BetType::RandomBlock => {
                // For random block, we need to select a random block at runtime
                // Use slot hash or similar for randomness
                let clock = Clock::get()?;
                let slot_bytes = clock.slot.to_le_bytes();
                let random_block = (slot_bytes[0] % 24) as u8; // 0-23 (0-indexed)
                expanded_bet_types.push(BetType::Block {
                    block_id: random_block,
                });
                msg!("   Random block selected: {}", random_block);
            }
            _ => {
                expanded_bet_types.push(bet_type.clone());
            }
        }
    }

    msg!("   Expanded to {} bet types", expanded_bet_types.len());

    // Call internal_process_bets for all bets at once
    internal_process_bets(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.game_session,
        &mut ctx.accounts.user_game_bet,
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.sol_rewards_vault.to_account_info(),
        &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.user_game_bet,
        ctx.accounts.authority.key(),
        amount_per_bet,
        expanded_bet_types.clone(),
        use_ticket,
        None, // User wallet signs the transaction
        None, // No autominer info
        &mut ctx.accounts.epoch_config,
        &mut ctx.accounts.epoch_state,
        &mut ctx.accounts.user_epoch_bets,
        ctx.bumps.user_epoch_bets,
    )?;

    msg!(
        "✅ [join_round_batch] All {} bets placed successfully",
        expanded_bet_types.len()
    );
    Ok(())
}

/// Initialize autominer vault with flexible block/faction configuration
/// Users configure either blocks OR factions (at least one required)
/// Can be called multiple times, but only when rounds_remaining == 0
/// Total SOL = sol_per_round × num_rounds
/// Bet size per bet = sol_per_round / total_bets_per_round
pub fn internal_init_autominer(
    ctx: Context<InitAutominer>,
    blocks_config: Option<BlocksConfig>,
    factions_config: Option<FactionsConfig>,
    sol_per_round: u64,
    num_rounds: u32,
    can_reload: bool,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🤖 [init_autominer] Initializing autominer vault");
    msg!("   Owner: {}", ctx.accounts.user_wallet.key());
    msg!("   SOL per round: {} lamports", sol_per_round);
    msg!("   Number of rounds: {}", num_rounds);

    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;

    msg!("   Validating parameters...");
    require!(num_rounds > 0, ErrorCode::InvalidAmount);
    // SOL mode requires sol_per_round > 0; ticket mode allows 0
    if use_ticket.is_none() {
        require!(sol_per_round > 0, ErrorCode::InvalidAmount);
    }
    require!(
        blocks_config.is_some() || factions_config.is_some(),
        ErrorCode::InvalidParameters
    );
    require!(
        !(blocks_config.is_some() && factions_config.is_some()),
        ErrorCode::InvalidParameters
    ); // Only one config allowed

    // Check if vault already exists and has remaining rounds
    // Only allow initialization if rounds_remaining == 0 (must stop first if in progress)
    require!(
        autominer_vault.rounds_remaining == 0,
        ErrorCode::InvalidParameters
    );
    let mut bets_per_round = 0;

    // Validate blocks_config if provided
    if let Some(ref blocks_cfg) = blocks_config {
        match blocks_cfg {
            BlocksConfig::Specific { blocks } => {
                require!(!blocks.is_empty(), ErrorCode::InvalidParameters);
                require!(
                    blocks.len() <= AutominerVault::MAX_BLOCKS,
                    ErrorCode::InvalidParameters
                );
                for &block_id in blocks.iter() {
                    require!(block_id < NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
                }
                bets_per_round = blocks.len() as u64;
                msg!("     ✓ Blocks: {} specific blocks", blocks.len());
            }
            BlocksConfig::Random { count } => {
                require!(
                    *count > 0 && *count <= NUM_BLOCKS as u8,
                    ErrorCode::InvalidParameters
                );
                msg!("     ✓ Blocks: {} random blocks", count);
                bets_per_round = *count as u64;
            }
        }
    }

    // Validate factions_config if provided
    if let Some(ref factions_cfg) = factions_config {
        match factions_cfg {
            FactionsConfig::Specific { factions, strategy } => {
                require!(!factions.is_empty(), ErrorCode::InvalidParameters);
                require!(
                    factions.len() <= AutominerVault::MAX_FACTIONS,
                    ErrorCode::InvalidParameters
                );
                for &faction_id in factions.iter() {
                    require!(
                        (faction_id as usize) < global_config.supported_factions.len(),
                        ErrorCode::InvalidFactionId
                    );
                }
                let strategy_multiplier = get_strategy_multiplier(strategy.clone());
                bets_per_round = factions.len() as u64 * strategy_multiplier;
                msg!("     ✓ Factions: {} specific factions", factions.len());
            }
            FactionsConfig::Random { count, strategy } => {
                require!(
                    *count > 0 && *count <= global_config.supported_factions.len() as u8,
                    ErrorCode::InvalidParameters
                );
                msg!("     ✓ Factions: {} random factions", count);
                let strategy_multiplier = get_strategy_multiplier(strategy.clone());
                bets_per_round = *count as u64 * strategy_multiplier;
            }
        }
    }

    require!(bets_per_round > 0, ErrorCode::InvalidParameters);
    msg!("     ✓ Bets per round: {}", bets_per_round);

    // Validate ticket configuration if provided
    if let Some(ticket_tier_index) = use_ticket {
        let player_data = &ctx.accounts.player_data;
        require!(
            (ticket_tier_index as usize) < player_data.free_tickets.len(),
            ErrorCode::InvalidParameters
        );
        require!(
            player_data.free_tickets_remaining[ticket_tier_index as usize] > 0,
            ErrorCode::InvalidParameters
        );
        let ticket_value = player_data.free_tickets[ticket_tier_index as usize];
        msg!(
            "     ✓ Ticket tier {} selected: {} SOL value, {} remaining",
            ticket_tier_index,
            (ticket_value as f64 / 1e9),
            player_data.free_tickets_remaining[ticket_tier_index as usize]
        );
    }

    // Calculate bet size per bet
    // In ticket mode, sol_per_round is only for caller compensation (gas fees)
    // Actual bet amount is determined at execution time from ticket value
    let bet_size_per_bet = if use_ticket.is_some() {
        0 // Resolved at execution time from player_data.free_tickets[tier]
    } else {
        sol_per_round / bets_per_round
    };
    if use_ticket.is_none() {
        require!(bet_size_per_bet > 0, ErrorCode::InvalidAmount);
        msg!(
            "     Bet size per bet: {} SOL per bet ({} SOL / {} bets)",
            (bet_size_per_bet as f64 / 1e9),
            (sol_per_round as f64 / 1e9),
            bets_per_round
        );
    } else {
        msg!("     Ticket mode: bet size determined at execution time");
    }

    msg!("   Initializing autominer vault...");

    // Store config flags before moving values
    let has_blocks_config = blocks_config.is_some();
    let has_factions_config = factions_config.is_some();

    // Calculate total SOL needed
    // Ticket mode: no SOL needed (bets use tickets, no caller compensation)
    // SOL mode: sol_per_round × num_rounds
    let total_sol = if use_ticket.is_some() {
        0 // Ticket mode: no SOL deposit needed
    } else {
        sol_per_round * num_rounds as u64
    };
    msg!(
        "     Total SOL for all rounds: {} SOL ({} rounds × {} SOL)",
        (total_sol as f64 / 1e9),
        num_rounds,
        (sol_per_round as f64 / 1e9)
    );

    autominer_vault.owner = ctx.accounts.user_wallet.key();
    autominer_vault.blocks_config = blocks_config;
    autominer_vault.factions_config = factions_config;
    autominer_vault.sol_per_round = sol_per_round;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    autominer_vault.sol_balance = total_sol;
    autominer_vault.can_reload = can_reload;
    autominer_vault.use_ticket = use_ticket;
    msg!(
        "     Vault initialized for owner: {}",
        autominer_vault.owner
    );

    // Transfer SOL to global autominer custody (SOL mode only)
    if total_sol > 0 {
        msg!("   Transferring SOL to autominer custody...");
        helper::transfer_to_autominer_custody(
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol,
        )?;
    } else {
        msg!("   Ticket mode: no SOL deposit needed");
    }

    msg!("✅ [init_autominer] Autominer initialized successfully");
    msg!(
        "   {} SOL per round, {} rounds ({} SOL total)",
        (sol_per_round as f64 / 1e9),
        num_rounds,
        (total_sol as f64 / 1e9)
    );

    emit!(AutominerInitialized {
        owner: ctx.accounts.user_wallet.key(),
        player_data: ctx.accounts.player_data.key(),
        gameplay_doge: ctx.accounts.player_data.gameplay_doge.clone(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        sol_per_round,
        num_rounds,
        bets_per_round,
        bet_size_per_bet,
        has_blocks_config,
        has_factions_config,
        can_reload,
        use_ticket,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Update autominer configuration (sol_per_round, num_rounds, can_reload)
/// Can only be called by vault owner
/// Handles SOL transfers automatically (deposit more if needed, refund excess)
pub fn internal_update_autominer(
    ctx: Context<UpdateAutominer>,
    sol_per_round: Option<u64>,
    rounds_added_: Option<u32>,
    can_reload: Option<bool>,
) -> Result<()> {
    msg!("🔄 [update_autominer] Updating autominer configuration");
    msg!("   Owner: {}", ctx.accounts.autominer_vault.owner);

    let autominer_vault = &mut ctx.accounts.autominer_vault;

    // Verify caller is owner
    require!(
        ctx.accounts.user_wallet.key() == autominer_vault.owner,
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is owner");

    // Read current values
    let old_sol_per_round = autominer_vault.sol_per_round;
    let rounds_remaining = autominer_vault.rounds_remaining;
    let old_can_reload = autominer_vault.can_reload;
    let old_sol_balance = autominer_vault.sol_balance;

    // Apply updates
    let new_sol_per_round = sol_per_round.unwrap_or(old_sol_per_round);
    let new_can_reload = can_reload.unwrap_or(old_can_reload);
    let rounds_added = rounds_added_.unwrap_or(0);

    msg!("   Current configuration:");
    msg!("     SOL per round: {} lamports", old_sol_per_round);
    msg!("     Rounds remaining: {}", rounds_remaining);
    msg!("     SOL balance (remaining): {} lamports", old_sol_balance);
    msg!("     Can reload: {}", old_can_reload);

    msg!("   New configuration:");
    msg!("     SOL per round: {} lamports", new_sol_per_round);
    msg!("     Rounds remaining: {}", rounds_remaining + rounds_added);
    msg!("     Can reload: {}", new_can_reload);

    // Validate new values (SOL mode requires sol_per_round > 0)
    if autominer_vault.use_ticket.is_none() {
        require!(new_sol_per_round > 0, ErrorCode::InvalidAmount);
    }

    if rounds_added > 0 {
        msg!(
            "     ✓ Adding {} more rounds (total: {} rounds)",
            rounds_added,
            rounds_remaining + rounds_added
        );
    } else {
        msg!("     ✓ Rounds unchanged ({} rounds)", rounds_remaining);
    }

    // SOL accounting: only for SOL mode. Ticket mode has no SOL balance.
    let sol_diff = if autominer_vault.use_ticket.is_some() {
        // Ticket mode: no SOL transfers, just update rounds and can_reload
        msg!("   Ticket mode: no SOL accounting needed");
        0i64
    } else {
        // Calculate SOL needed for remaining rounds with old config
        let current_sol_needed = rounds_remaining as u64 * old_sol_per_round;
        msg!(
            "     Current SOL needed for {} remaining rounds: {} lamports",
            rounds_remaining,
            current_sol_needed
        );

        // Calculate new SOL needed for new rounds with new config
        let new_sol_needed = (rounds_remaining + rounds_added) as u64 * new_sol_per_round;
        msg!(
            "     New SOL needed for {} rounds: {} lamports",
            rounds_remaining + rounds_added,
            new_sol_needed
        );

        // Calculate SOL difference: new_sol_needed - current_sol_balance
        let diff = new_sol_needed as i64 - old_sol_balance as i64;
        msg!("     SOL difference: {} lamports", diff);

        // Handle SOL transfers
        if diff > 0 {
            let deposit_amount = diff as u64;
            msg!(
                "   Depositing {} SOL to autominer custody...",
                deposit_amount as f64 / 1e9
            );
            helper::transfer_to_autominer_custody(
                &ctx.accounts.user_wallet.to_account_info(),
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                deposit_amount,
            )?;
            msg!("     ✓ Deposited {} SOL", deposit_amount as f64 / 1e9);
        } else if diff < 0 {
            let refund_amount = (-diff) as u64;
            msg!(
                "   Refunding {} SOL from autominer custody...",
                refund_amount as f64 / 1e9
            );
            helper::transfer_from_autominer_custody(
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.user_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                refund_amount,
                ctx.bumps.autominer_custody,
            )?;
            msg!("     ✓ Refunded {} SOL", refund_amount as f64 / 1e9);
        } else {
            msg!("   No SOL transfer needed");
        }

        diff
    };

    // Update vault state
    autominer_vault.sol_per_round = new_sol_per_round;
    autominer_vault.rounds_remaining += rounds_added;
    autominer_vault.can_reload = new_can_reload;
    if autominer_vault.use_ticket.is_none() {
        autominer_vault.sol_balance = (old_sol_balance as i64 + sol_diff) as u64;
    }

    msg!("✅ [update_autominer] Autominer updated successfully");

    emit!(AutominerUpdated {
        owner: autominer_vault.owner,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: autominer_vault.key(),
        sol_per_round: new_sol_per_round,
        rounds_remaining: rounds_remaining + rounds_added,
        can_reload: new_can_reload,
        sol_diff,
    });

    Ok(())
}

fn get_strategy_multiplier(strategy: FactionStrategy) -> u64 {
    match strategy {
        FactionStrategy::Both => 2,
        _ => 1,
    }
}

/// Execute autominer bets (keeper instruction - callable by anyone)
/// Generates bet types dynamically from blocks_config and factions_config
/// Pays caller 1% of bet size (max 0.005 SOL) per bet for tx costs
/// Uses join_round_batch to place all bets efficiently
pub fn internal_execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
    msg!("🤖 [execute_autominer_bet] Executing autominer bets");
    msg!("   Owner: {}", ctx.accounts.autominer_vault.owner);
    msg!("   Caller: {}", ctx.accounts.caller.key());

    let global_state = &ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    // Read values before mutable borrow
    let owner_key = ctx.accounts.autominer_vault.owner;
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let last_bet_round_id = ctx.accounts.autominer_vault.last_bet_round_id;
    let sol_per_round = ctx.accounts.autominer_vault.sol_per_round;
    let blocks_config = ctx.accounts.autominer_vault.blocks_config.clone(); // Already Option<BlocksConfig>
    let factions_config = ctx.accounts.autominer_vault.factions_config.clone();
    let sol_balance = ctx.accounts.autominer_vault.sol_balance;
    let use_ticket = ctx.accounts.autominer_vault.use_ticket;
    let custody_bump = ctx.bumps.autominer_custody;
    let autominer_custody_info = ctx.accounts.autominer_custody.to_account_info();

    if rounds_remaining == 0 || last_bet_round_id == global_state.current_round_id {
        return Ok(());
    }

    msg!("   Vault state:");
    msg!(
        "     Rounds remaining: {}. Last bet round ID: {}. SOL per round: {} SOL",
        rounds_remaining,
        last_bet_round_id,
        (sol_per_round as f64 / 1e9)
    );

    require!(rounds_remaining > 0, ErrorCode::NoRoundsRemaining);
    // SOL balance check only for SOL mode (ticket mode doesn't need SOL for bets)
    if use_ticket.is_none() {
        require!(sol_balance >= sol_per_round, ErrorCode::InsufficientFunds);
    }

    // Generate bet types dynamically from configuration
    msg!("   Generating bet types from configuration...");

    // Determine blocks to bet on (if blocks_config provided)
    let blocks_to_bet = compute_blocks_to_bet(blocks_config, &clock)?;

    // Generate bet types using helper function
    let bet_types = make_bets_vec(
        factions_config.clone(),
        blocks_to_bet.clone(),
        &ctx.accounts.game_session,
        &clock,
        &global_config,
    )?;
    msg!("     Generated {} bet types", bet_types.len());

    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);

    // Calculate caller compensation FIRST: 1% of sol_per_round, max 0.005 SOL
    let total_caller_compensation = get_caller_compensation(sol_per_round)?;
    msg!(
        "     Caller compensation: {} SOL (1% of {} SOL, max 0.005 SOL)",
        (total_caller_compensation as f64 / 1e9),
        (sol_per_round as f64 / 1e9)
    );

    // Determine bet parameters based on mode (SOL vs tickets)
    let (bet_size_per_bet, effective_use_ticket) = if let Some(ticket_tier_index) = use_ticket {
        // Ticket mode: bet amount comes from player's ticket value
        // No SOL needed for bets — tickets are free. No caller compensation either.
        let player_data = &ctx.accounts.player_data;
        require!(
            (ticket_tier_index as usize) < player_data.free_tickets.len(),
            ErrorCode::InvalidParameters
        );
        let ticket_value = player_data.free_tickets[ticket_tier_index as usize];
        require!(ticket_value > 0, ErrorCode::InvalidAmount);
        msg!(
            "     Ticket mode: tier {}, value {} SOL, bets: {}",
            ticket_tier_index,
            (ticket_value as f64 / 1e9),
            bet_types.len()
        );
        (ticket_value, Some(ticket_tier_index))
    } else {
        // SOL mode: deduct caller compensation from sol_per_round to get betting amount
        let sol_for_betting = sol_per_round
            .checked_sub(total_caller_compensation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let bet_per = sol_for_betting / bet_types.len() as u64;
        require!(bet_per > 0, ErrorCode::InvalidAmount);
        msg!(
            "     SOL mode: {} SOL per bet ({} SOL / {} bets)",
            (bet_per as f64 / 1e9),
            (sol_for_betting as f64 / 1e9),
            bet_types.len()
        );
        (bet_per, None)
    };

    // Pay caller compensation (SOL mode only — ticket mode skips compensation)
    if total_caller_compensation > 0 && use_ticket.is_none() {
        msg!("   Paying caller compensation...");
        let autominer_seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: autominer_custody_info.clone(),
                    to: ctx.accounts.caller.to_account_info(),
                },
                &[autominer_seeds],
            ),
            total_caller_compensation,
        )?;
        msg!(
            "     Caller compensation: {} SOL transferred",
            (total_caller_compensation as f64 / 1e9)
        );
    }

    // Now borrow mutably to update state
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let current_round_id = global_state.current_round_id;

    // Mark bets as placed for this round
    autominer_vault.last_bet_round_id = current_round_id;
    msg!(
        "   Updated last_bet_round_id: {} -> {}",
        last_bet_round_id,
        current_round_id
    );

    // Decrement rounds remaining
    let new_rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    autominer_vault.rounds_remaining = new_rounds_remaining;
    msg!(
        "   Updated rounds_remaining: {} -> {}",
        rounds_remaining,
        new_rounds_remaining
    );

    // Update remaining SOL balance tracked for this autominer (SOL mode only)
    if use_ticket.is_none() {
        autominer_vault.sol_balance = autominer_vault.sol_balance.saturating_sub(sol_per_round);
    }

    // Place bets using join_round_batch
    msg!(
        "   Placing {} bets for round {} using join_round_batch...",
        bet_types.len(),
        current_round_id
    );

    // Expand bet types (handle FactionBoth and RandomBlock)
    let mut expanded_bet_types = Vec::new();
    for bet_type in bet_types.iter() {
        match bet_type {
            BetType::FactionBoth { faction_id } => {
                expanded_bet_types.push(BetType::FactionHighestLowest {
                    faction_id: *faction_id,
                    is_highest: true,
                });
                expanded_bet_types.push(BetType::FactionHighestLowest {
                    faction_id: *faction_id,
                    is_highest: false,
                });
            }
            BetType::RandomBlock => {
                let slot_bytes = clock.slot.to_le_bytes();
                let random_block = (slot_bytes[0] % 24) as u8;
                expanded_bet_types.push(BetType::Block {
                    block_id: random_block,
                });
            }
            _ => {
                expanded_bet_types.push(bet_type.clone());
            }
        }
    }

    msg!("     Expanded to {} bet types", expanded_bet_types.len());

    // Prepare PDA signer seeds for autominer custody
    let autominer_seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];

    // Prepare autominer info
    let autominer_info = AutominerBetInfo {
        vault: ctx.accounts.autominer_vault.key(),
        caller: ctx.accounts.caller.key(),
        compensation: total_caller_compensation,
        rounds_remaining: new_rounds_remaining,
    };

    // Call internal_process_bets with autominer vault as payer (PDA signs via seeds)
    // Process all bets at once
    internal_process_bets(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.game_session,
        &mut ctx.accounts.user_game_bet,
        &autominer_custody_info,
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.sol_rewards_vault.to_account_info(),
        &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.user_game_bet,
        owner_key,
        bet_size_per_bet,
        expanded_bet_types.clone(),
        effective_use_ticket,  // None for SOL, Some(tier) for tickets
        Some(autominer_seeds), // PDA signs via seeds
        Some(autominer_info),
        &mut ctx.accounts.epoch_config,
        &mut ctx.accounts.epoch_state,
        &mut ctx.accounts.user_epoch_bets,
        ctx.bumps.user_epoch_bets,
    )?;

    msg!("✅ [execute_autominer_bet] Autominer bets executed successfully");
    msg!(
        "   {} bets of {} SOL each for round {}",
        expanded_bet_types.len(),
        (bet_size_per_bet as f64 / 1e9),
        current_round_id
    );
    msg!("   Rounds remaining: {}", new_rounds_remaining);
    msg!(
        "   Caller compensation: {} SOL",
        (total_caller_compensation as f64 / 1e9)
    );

    Ok(())
}

/// Stop autominer and refund remaining SOL
/// Can only be called by vault owner
/// Refunds all remaining SOL (after rent) and resets rounds_remaining to 0
pub fn internal_stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
    msg!("🛑 [stop_autominer] Stopping autominer");

    // Read values before mutable borrow
    let owner_key = ctx.accounts.autominer_vault.owner;
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let sol_balance = ctx.accounts.autominer_vault.sol_balance;

    msg!("   Owner: {}", owner_key);

    // Verify caller is owner
    require!(
        ctx.accounts.authority.key() == owner_key,
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is owner");

    msg!("   Vault state:");
    msg!("     Rounds remaining: {}", rounds_remaining);
    msg!("     Remaining SOL to refund: {}", sol_balance);

    // Calculate rent that will be returned
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    msg!("     Rent to be returned: {} lamports", rent);

    // Refund remaining SOL to owner (transfer from custody PDA to owner)
    if sol_balance > 0 {
        msg!(
            "   Refunding {} SOL to owner...",
            (sol_balance as f64 / 1e9)
        );
        helper::transfer_from_autominer_custody(
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.owner.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            sol_balance,
            ctx.bumps.autominer_custody,
        )?;
        msg!(
            "     ✓ Refunded {} SOL to owner",
            (sol_balance as f64 / 1e9)
        );
    }

    msg!("   Closing autominer vault account and returning rent...");
    msg!("     Rent returned: {} lamports", rent);

    msg!("✅ [stop_autominer] Autominer stopped successfully");
    msg!("   Refunded {} SOL to owner", (sol_balance as f64 / 1e9));
    msg!("   Returned {} lamports rent to authority", rent);

    emit!(AutominerStopped {
        owner: owner_key,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        rounds_remaining,
        refund_amount: sol_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Claim rewards for a user after round ends
/// Checks if user won based on their bet type and the winning block
pub fn internal_claim_round_rewards(round_id: u64, ctx: Context<ClaimRoundRewards>) -> Result<()> {
    msg!(
        "💰 [claim_rewards] User claiming rewards. User: {}",
        ctx.accounts.user_wallet.key()
    );
    msg!("   Round ID: {}", round_id);

    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;

    // Round should be completely over before user can claim rewards
    require!(game_session.stage == 2, ErrorCode::InvalidStage);
    require!(
        round_id == user_bet.round_id && round_id == game_session.round_id,
        ErrorCode::InvalidRound
    );

    // Check which blocks user bet on and calculate rewards
    msg!(
        "   User bet on {} blocks: {:?}",
        user_bet.block_ids.len(),
        user_bet.block_ids
    );
    msg!(
        "   Winning block: {}. Follow-up block: {}",
        game_session.winning_block,
        game_session.same_faction_other_block
    );
    msg!(
        "     Winning faction ID: {}",
        game_session.winning_faction_id
    );

    // Calculate rewards using helper function
    let (total_sol_reward, total_minebtc_reward) = calculate_round_rewards(user_bet, game_session)?;

    // Update player rewards using helper function
    update_player_rewards(
        player_data,
        &mut ctx.accounts.unrefined_rewards,
        total_sol_reward,
        total_minebtc_reward,
    )?;

    // Transfer SOL winnings directly to user from prize pot vault
    if total_sol_reward > 0 {
        msg!(
            "   Transferring {} SOL winnings from prize pot to user",
            total_sol_reward as f64 / 1e9
        );
        helper::transfer_from_sol_prize_pot_vault(
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol_reward,
            ctx.bumps.sol_prize_pot_vault,
        )?;
    }

    // === ACCUMULATED VALUE & MUTATION SYNC ===
    process_mutation_sync(
        user_bet,
        player_data,
        ctx.accounts.doge_metadata.as_mut(),
        total_minebtc_reward,
    )?;

    msg!("✅ [claim_rewards] Rewards claimed successfully");
    msg!("   Round: {}", user_bet.round_id);

    emit!(RoundRewardsClaimed {
        user: ctx.accounts.player_data.owner,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: total_sol_reward,
        minebtc_reward: total_minebtc_reward,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Claim autominer rewards with auto-reload feature
/// Called by backend script - no owner check, uses SOL rewards to reload autominer
pub fn internal_claim_autominer_rewards(
    round_id: u64,
    ctx: Context<ClaimAutominerRewards>,
) -> Result<()> {
    msg!("🤖 [claim_autominer_rewards] Claiming rewards with auto-reload");
    msg!("   Round ID: {}", round_id);
    msg!("   Autominer owner: {}", ctx.accounts.autominer_vault.owner);

    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;
    let autominer_vault = &mut ctx.accounts.autominer_vault;

    // Round should be completely over
    require!(game_session.stage == 2, ErrorCode::InvalidStage);
    require!(
        round_id == user_bet.round_id && round_id == game_session.round_id,
        ErrorCode::InvalidRound
    );

    // Calculate rewards using helper function
    let (total_sol_reward, total_minebtc_reward) = calculate_round_rewards(user_bet, game_session)?;

    // Update player rewards using helper function
    update_player_rewards(
        player_data,
        &mut ctx.accounts.unrefined_rewards,
        total_sol_reward,
        total_minebtc_reward,
    )?;

    // === ACCUMULATED VALUE & MUTATION SYNC ===
    process_mutation_sync(
        user_bet,
        player_data,
        ctx.accounts.doge_metadata.as_mut(),
        total_minebtc_reward,
    )?;

    // === AUTO-RELOAD LOGIC ===
    // Ticket mode + reload: just keep running (add rounds back), send SOL winnings to owner.
    // SOL mode + reload: use SOL winnings to fund more rounds.
    if autominer_vault.can_reload && autominer_vault.use_ticket.is_some() {
        // TICKET MODE RELOAD: Add rounds back, send all SOL rewards to owner
        // Tickets are consumed from player's balance — no SOL needed to reload.
        // The autominer just keeps going until all tickets are exhausted.
        if total_sol_reward > 0 {
            msg!("🔄 Ticket mode reload: sending SOL winnings to owner...");
            helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.owner_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                total_sol_reward,
                ctx.bumps.sol_prize_pot_vault,
            )?;
            msg!(
                "   ✓ Transferred {} SOL to owner",
                total_sol_reward as f64 / 1e9
            );
        }

        // Add 1 round back (each execution consumed 1 round, reload gives it back)
        autominer_vault.rounds_remaining += 1;
        msg!(
            "   ✓ Ticket reload: rounds_remaining restored to {}",
            autominer_vault.rounds_remaining
        );

        emit!(AutominerReloaded {
            autominer_vault: autominer_vault.key(),
            rounds_to_add: 1,
            sol_for_rounds: 0,
            leftover_sol: total_sol_reward,
            timestamp: Clock::get()?.unix_timestamp,
        });
    } else if total_sol_reward > 0
        && autominer_vault.can_reload
        && autominer_vault.sol_per_round > 0
    {
        // SOL MODE RELOAD: Use SOL winnings to fund more rounds
        msg!("🔄 SOL mode reload, processing SOL rewards...");

        let sol_per_round = autominer_vault.sol_per_round;
        let rounds_to_add = total_sol_reward / sol_per_round;
        let leftover_sol = total_sol_reward % sol_per_round;

        msg!("   SOL per round: {} lamports", sol_per_round);
        msg!("   Rounds to add: {}", rounds_to_add);
        msg!("   Leftover SOL: {} lamports", leftover_sol);

        if rounds_to_add > 0 {
            let sol_for_rounds = rounds_to_add * sol_per_round;

            // Transfer SOL from prize pot to autominer custody
            helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                sol_for_rounds,
                ctx.bumps.sol_prize_pot_vault,
            )?;

            // Update autominer state
            autominer_vault.sol_balance += sol_for_rounds;
            autominer_vault.rounds_remaining += rounds_to_add as u32;

            emit!(AutominerReloaded {
                autominer_vault: autominer_vault.key(),
                rounds_to_add: rounds_to_add as u32,
                sol_for_rounds: sol_for_rounds,
                leftover_sol: leftover_sol,
                timestamp: Clock::get()?.unix_timestamp,
            });

            msg!(
                "   ✓ Added {} rounds, {} SOL to autominer",
                rounds_to_add,
                sol_for_rounds
            );
        }

        // Transfer leftover SOL to owner
        if leftover_sol > 0 {
            helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.owner_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                leftover_sol,
                ctx.bumps.sol_prize_pot_vault,
            )?;
            msg!("   ✓ Transferred {} leftover SOL to owner", leftover_sol);
        }
    } else if total_sol_reward > 0 {
        // No reload (or no sol_per_round) - transfer all SOL to owner
        msg!("   Reload disabled, transferring all SOL to owner...");
        helper::transfer_from_sol_prize_pot_vault(
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.owner_wallet.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol_reward,
            ctx.bumps.sol_prize_pot_vault,
        )?;
    }

    msg!("✅ [claim_autominer_rewards] Completed");
    emit!(RoundRewardsClaimed {
        user: ctx.accounts.autominer_vault.owner,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: total_sol_reward,
        minebtc_reward: total_minebtc_reward,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Calculate SOL and MineBTC rewards for a user bet
/// Returns (total_sol_reward, total_minebtc_reward)
fn calculate_round_rewards(
    user_bet: &UserGameBet,
    game_session: &GameSession,
) -> Result<(u64, u64)> {
    let mut total_sol_reward = 0u64;
    let mut total_minebtc_reward = 0u64;

    for (idx, &block_id) in user_bet.block_ids.iter().enumerate() {
        let points_bet_on_block = user_bet.points_bets.get(idx).copied().unwrap_or(0);
        let wgtd_points_bet_on_block = user_bet.wgtd_points_bets.get(idx).copied().unwrap_or(0);

        msg!(
            "     Block {}: Points: {} SOL, Wgtd: {} DogeBTC",
            block_id,
            points_bet_on_block as f64 / 1e9,
            wgtd_points_bet_on_block as f64 / 1e6
        );

        let is_winning_block = block_id == game_session.winning_block;
        let is_same_faction_block = block_id == game_session.same_faction_other_block;

        if is_winning_block {
            msg!("       ✓ Winning block - calculating rewards...");

            // SOL rewards: use regular points
            if game_session.sol_rewards_index > 0 && points_bet_on_block > 0 {
                let sol_reward = helper::mul_div(
                    points_bet_on_block,
                    game_session.sol_rewards_index as u64,
                    INDEX_PRECISION,
                )? as u64;
                total_sol_reward += sol_reward;
                msg!("         SOL reward: {} SOL", sol_reward as f64 / 1e9);
            }

            // MineBtc rewards: use wgtd_points
            if game_session.minebtc_rewards_index > 0 && wgtd_points_bet_on_block > 0 {
                let minebtc_reward = helper::mul_div(
                    wgtd_points_bet_on_block,
                    game_session.minebtc_rewards_index as u64,
                    INDEX_PRECISION,
                )? as u64;
                total_minebtc_reward += minebtc_reward;
                msg!(
                    "         MineBtc reward: {} DogeBTC",
                    minebtc_reward as f64 / 1e6
                );
            }
        } else if is_same_faction_block {
            msg!("       ✓ Same-faction other block - calculating MineBtc rewards...");

            // MineBtc rewards: use wgtd_points
            if game_session.same_faction_minebtc_rewards_index > 0 && wgtd_points_bet_on_block > 0 {
                let minebtc_reward = helper::mul_div(
                    wgtd_points_bet_on_block,
                    game_session.same_faction_minebtc_rewards_index as u64,
                    INDEX_PRECISION,
                )? as u64;
                total_minebtc_reward += minebtc_reward;
                msg!(
                    "         MineBtc reward: {} DogeBTC",
                    minebtc_reward as f64 / 1e6
                );
            }
        } else {
            msg!("       ✗ Not a winning or same-faction block - no rewards");
        }
    }

    Ok((total_sol_reward, total_minebtc_reward))
}

/// Update player rewards stats and add MineBTC to pending rewards
fn update_player_rewards(
    player_data: &mut PlayerData,
    unrefined_rewards: &mut UnrefinedRewards,
    total_sol_reward: u64,
    total_minebtc_reward: u64,
) -> Result<()> {
    player_data.total_sol_won += total_sol_reward;
    msg!(
        "     Total SOL won: {} (+{})",
        player_data.total_sol_won as f64 / 1e9,
        total_sol_reward as f64 / 1e9
    );

    helper::add_to_total_claimable(unrefined_rewards, player_data, total_minebtc_reward);
    player_data.total_dogebtc_won += total_minebtc_reward;
    msg!(
        "     Total MineBtc won: {} (+{})",
        player_data.total_dogebtc_won as f64 / 1e6,
        total_minebtc_reward as f64 / 1e6
    );
    msg!(
        "     Pending MineBtc rewards: {} (+{})",
        player_data.pending_minebtc_rewards as f64 / 1e6,
        total_minebtc_reward as f64 / 1e6
    );

    Ok(())
}

/// Process mutation sync: update accumulated_val and sync DNA/XP/multiplier
fn process_mutation_sync<'info>(
    user_bet: &UserGameBet,
    player_data: &mut PlayerData,
    doge_metadata: Option<&mut Box<Account<'info, DogeMetadata>>>,
    total_minebtc_reward: u64,
) -> Result<()> {
    // If no doge is being used for gameplay, return
    if player_data.gameplay_doge_xp == 0 && player_data.gameplay_doge == Pubkey::default() {
        return Ok(());
    }

    if user_bet.gameplay_doge == player_data.gameplay_doge && total_minebtc_reward > 0 {
        require!(
            doge_metadata.is_some()
                && doge_metadata.as_ref().unwrap().mint == user_bet.gameplay_doge,
            ErrorCode::DogeMetadataNotFound
        );
        if let Some(doge_metadata) = doge_metadata {
            // Calculate accumulated_val % based on mutation type
            // 0 = no mutation (1%), 1 = Evolution (6.9%), 2 = Power (4.2%), 3 = Trait (3%)
            let accum_pct = match user_bet.mutation_type {
                1 => 69u64, // Evolution: 6.9%
                2 => 42u64, // Power: 4.2%
                3 => 30u64, // Trait: 3%
                _ => 10u64, // No mutation: 1%
            };
            let accum_add = (total_minebtc_reward * accum_pct) / 1000;
            doge_metadata.accumulated_val = doge_metadata.accumulated_val.saturating_add(accum_add);
            msg!(
                "💎 Doge accumulated_val +{} ({}%)",
                accum_add,
                accum_pct as f64 / 10.0
            );

            // Sync DNA/XP/multiplier from PlayerData cache
            // Note: generation is stored in DNA bits 4-6, not as separate field
            doge_metadata.dna = player_data.gameplay_doge_dna;
            doge_metadata.xp = player_data.gameplay_doge_xp;
            doge_metadata.multiplier = player_data.active_multiplier;

            // For Evolution, reset XP (DNA already updated by evolve_stage)
            if user_bet.mutation_type == 1 {
                doge_metadata.xp = 0;
                player_data.gameplay_doge_xp = 0;
            }

            emit!(DogeSynced {
                doge_mint: doge_metadata.mint,
                doge_metadata_account: doge_metadata.key(),
                dna: doge_metadata.dna.to_vec(),
                xp: doge_metadata.xp,
                multiplier: doge_metadata.multiplier as u32,
                accumulated_val: doge_metadata.accumulated_val,
                accum_pct: accum_pct as u32,
            });

            msg!("🧬 Synced to doge: {}", doge_metadata.mint);
        }
    }
    Ok(())
}

/// Helper struct for passing autominer info to internal_process_bets
pub struct AutominerBetInfo {
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub compensation: u64,
    pub rounds_remaining: u32,
}

/// Internal join_round logic for batched processing
/// Calculates totals, performs single transfers, and updates state for all bets
#[allow(clippy::too_many_arguments)]
fn internal_process_bets<'info>(
    global_state: &Account<'info, GlobalGameSate>,
    global_config: &Account<'info, GlobalConfig>,
    player_data: &mut Account<'info, PlayerData>,
    game_session: &mut Account<'info, GameSession>,
    user_game_bet: &mut Account<'info, UserGameBet>,
    payer: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    sol_rewards_vault: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_game_bet_bump: u8,
    owner_key: Pubkey,
    amount_per_bet: u64,
    bet_types: Vec<BetType>,
    use_ticket: Option<u8>,
    signer_seeds: Option<&[&[u8]]>,
    autominer_info: Option<AutominerBetInfo>,
    epoch_config: &mut Account<'info, EpochConfig>,
    epoch_state: &mut Account<'info, EpochState>,
    user_epoch_bets: &mut Account<'info, UserEpochBets>,
    user_epoch_bets_bump: u8,
) -> Result<()> {
    let round_id = global_state.current_round_id;

    require!(game_session.round_id == round_id, ErrorCode::InvalidRound);
    require!(
        game_session.block_assignments.iter().any(|&f| f != 0),
        ErrorCode::InvalidParameters
    );
    require!(
        amount_per_bet > 0 || use_ticket.is_some(),
        ErrorCode::InvalidAmount
    );
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);

    msg!(
        "   Processing batch of {} bets for round {}",
        bet_types.len(),
        round_id
    );

    // Arrays to return for events
    let mut evt_target_blocks = Vec::new();
    let mut evt_net_amounts = Vec::new();
    let mut evt_fee_amounts = Vec::new();
    let mut evt_points_amounts = Vec::new();
    let mut evt_wgtd_points_amounts = Vec::new();

    // Initialize totals
    let num_bets = bet_types.len() as u64;
    let mut total_stakers_fee = 0u64;
    let mut total_protocol_fee = 0u64;
    let mut total_net_to_pot = 0u64;

    // Get multiplier (default 1000 = 1x if not set)
    let active_mult = if player_data.active_multiplier == 0 {
        BASE_MULTIPLIER as u64
    } else {
        player_data.active_multiplier as u64
    };

    // Calculate amounts per bet (uniform across batch)
    // wgtd_points: points * multiplier / BASE_MULTIPLIER for SOL bets, else points (tickets)
    let (net_per_bet, fee_per_bet, points_per_bet, wgtd_points_per_bet) =
        if let Some(ticket_type_index) = use_ticket {
            // Ticket Logic - no multiplier applied
            require!(
                (ticket_type_index as usize) < player_data.free_tickets.len(),
                ErrorCode::InvalidParameters
            );
            let ticket_value = player_data.free_tickets[ticket_type_index as usize];
            require!(amount_per_bet == ticket_value, ErrorCode::InvalidAmount);

            require!(
                player_data.free_tickets_remaining[ticket_type_index as usize] >= num_bets,
                ErrorCode::InsufficientFunds
            );

            // Validate total points limit
            let total_points = amount_per_bet * num_bets;
            validate_points_percentage_limit(
                game_session.total_points_bets,
                game_session.total_sol_bets,
                total_points,
            )?;

            // Deduct tickets
            player_data.free_tickets_remaining[ticket_type_index as usize] -= num_bets;
            msg!(
                "     Deducted {} tickets of tier {}",
                num_bets,
                ticket_type_index
            );

            (0, 0, amount_per_bet, amount_per_bet) // wgtd_points = points for tickets
        } else {
            // SOL Logic - apply multiplier for wgtd_points
            require!(amount_per_bet > 0, ErrorCode::InvalidAmount);
            let (net, fee) = handle_fee(
                amount_per_bet,
                global_config.sol_fee_config.protocol_fee_pct as u64,
            )?;

            // Split fee
            let stakers_fee = fee * global_config.sol_fee_config.stakers_pct as u64 / M_HUNDRED;
            let protocol_fee = fee - stakers_fee;

            // Accumulate totals for transfer
        total_stakers_fee = stakers_fee.saturating_mul(num_bets);
        total_protocol_fee = protocol_fee.saturating_mul(num_bets);
        total_net_to_pot = net.saturating_mul(num_bets);

            // wgtd_points = points * multiplier / BASE_MULTIPLIER for SOL bets
            let wgtd = net * active_mult / BASE_MULTIPLIER as u64;
            (net, fee, net, wgtd)
        };

    // Perform Bulk Transfers
    let do_transfer = |to: &AccountInfo<'info>, amount: u64| -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        if let Some(seeds) = signer_seeds {
            transfer(
                CpiContext::new_with_signer(
                    system_program.to_account_info(),
                    Transfer {
                        from: payer.to_account_info(),
                        to: to.to_account_info(),
                    },
                    &[seeds],
                ),
                amount,
            )
        } else {
            transfer(
                CpiContext::new(
                    system_program.to_account_info(),
                    Transfer {
                        from: payer.to_account_info(),
                        to: to.to_account_info(),
                    },
                ),
                amount,
            )
        }
    };

    if total_stakers_fee > 0 {
        msg!(
            "   Transferring total stakers fees ({} SOL)",
            total_stakers_fee as f64 / 1e9
        );
        do_transfer(sol_rewards_vault, total_stakers_fee)?;
    }
    if total_protocol_fee > 0 {
        msg!(
            "   Transferring total protocol fees ({} SOL)",
            total_protocol_fee as f64 / 1e9
        );
        do_transfer(sol_treasury, total_protocol_fee)?;
    }
    if total_net_to_pot > 0 {
        msg!(
            "   Transferring total net amount to pot ({} SOL)",
            total_net_to_pot as f64 / 1e9
        );
        do_transfer(sol_prize_pot_vault, total_net_to_pot)?;
    }

    // Initialize UserGameBet if needed
    if user_game_bet.owner == Pubkey::default() {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.block_ids = Vec::new();
        user_game_bet.sol_bets = Vec::new();
        user_game_bet.points_bets = Vec::new();
        user_game_bet.wgtd_points_bets = Vec::new();
        user_game_bet.gameplay_doge = player_data.gameplay_doge;
        user_game_bet.bump = user_game_bet_bump;

        player_data.rounds_played += 1;
        msg!("     New bet account initialized");
    } else {
        require!(user_game_bet.round_id == round_id, ErrorCode::InvalidRound);
    }

    // Process each bet state
    for bet_type in bet_types {
        let target_block =
            get_target_block_from_bet_type(&bet_type, &game_session.block_assignments)?;
        let block_index = target_block as usize;
        require!(block_index < NUM_BLOCKS, ErrorCode::InvalidParameters);

        // Update UserGameBet vectors
        if let Some(index) = user_game_bet
            .block_ids
            .iter()
            .position(|&b| b == target_block)
        {
            user_game_bet.sol_bets[index] += net_per_bet;
            user_game_bet.points_bets[index] += points_per_bet;
            user_game_bet.wgtd_points_bets[index] += wgtd_points_per_bet;
        } else {
            user_game_bet.block_ids.push(target_block);
            user_game_bet.sol_bets.push(net_per_bet);
            user_game_bet.points_bets.push(points_per_bet);
            user_game_bet.wgtd_points_bets.push(wgtd_points_per_bet);

            // Increment user count for this block only if new
            game_session.user_block_indexes[block_index] += 1;
        }

        // Update GameSession stats
        game_session.sol_bets_indexes[block_index] += net_per_bet;
        game_session.points_bets_indexes[block_index] += points_per_bet;
        game_session.wgtd_points_bets_indexes[block_index] += wgtd_points_per_bet;

        // Record for events
        evt_target_blocks.push(target_block);
        evt_net_amounts.push(net_per_bet);
        evt_fee_amounts.push(fee_per_bet);
        evt_points_amounts.push(points_per_bet);
        evt_wgtd_points_amounts.push(wgtd_points_per_bet);
    }

    // Update Totals
    let total_net_added = net_per_bet * num_bets;
    let total_points_added = points_per_bet * num_bets;
    let total_wgtd_points_added = wgtd_points_per_bet * num_bets;
    let total_fee_added = fee_per_bet * num_bets;

    user_game_bet.total_sol_bet = user_game_bet.total_sol_bet.saturating_add(total_net_added);
    user_game_bet.total_points_bet += total_points_added;
    user_game_bet.total_wgtd_points_bet += total_wgtd_points_added;
    user_game_bet.total_fee += total_fee_added;

    game_session.total_sol_bets = game_session.total_sol_bets.saturating_add(total_net_added);
    game_session.total_points_bets += total_points_added;
    game_session.total_wgtd_points_bets += total_wgtd_points_added;
    game_session.stakers_fee += total_stakers_fee;

    player_data.total_sol_bet = player_data.total_sol_bet.saturating_add(total_net_added);
    player_data.total_points_bet += total_points_added;

    msg!(
        "   Batch processed: {} bets. Total Net: {} SOL",
        num_bets,
        total_net_added as f64 / 1e9
    );

    // Emit consolidated event
    let (
        is_autominer,
        autominer_vault,
        caller,
        caller_compensation,
        rounds_remaining,
        vault_closed,
    ) = if let Some(info) = autominer_info {
        (
            true,
            Some(info.vault),
            Some(info.caller),
            info.compensation,
            Some(info.rounds_remaining),
            Some(info.rounds_remaining == 0),
        )
    } else {
        (false, None, None, 0, None, None)
    };

    let clock = Clock::get()?;
    emit!(BetsPlaced {
        user: owner_key,
        player_data: player_data.key(),
        gameplay_doge: player_data.gameplay_doge,
        gameplay_doge_dna: player_data.gameplay_doge_dna,
        active_multiplier: player_data.active_multiplier,
        gameplay_doge_xp: player_data.gameplay_doge_xp,
        round_id,
        num_bets: num_bets as u8,
        target_blocks: evt_target_blocks.clone(),
        net_amounts: evt_net_amounts,
        fee_amounts: evt_fee_amounts,
        points_amounts: evt_points_amounts,
        wgtd_points_amounts: evt_wgtd_points_amounts,
        used_ticket: use_ticket.is_some(),
        ticket_type_index: use_ticket,
        is_autominer,
        autominer_vault,
        caller,
        caller_compensation,
        rounds_remaining,
        vault_closed,
        timestamp: clock.unix_timestamp,
    });

    // === EPOCH BET ACCUMULATION (inline) ===
    if epoch_config.is_active && epoch_state.stage == 0 {
        // Initialize epoch_state if freshly created (init_if_needed)
        if epoch_state.epoch_id == 0 && epoch_state.start_timestamp == 0 && epoch_config.current_epoch_id > 0 {
            let clock_now = Clock::get()?;
            epoch_state.epoch_id = epoch_config.current_epoch_id;
            epoch_state.start_timestamp = clock_now.unix_timestamp as u64;
            epoch_state.end_timestamp = epoch_state.start_timestamp + epoch_config.epoch_duration;
            epoch_state.stage = 0;
            epoch_state.faction_scores = [0u16; NUM_FACTIONS];
            epoch_state.faction_total_sol_bets = [0u64; NUM_FACTIONS];
            epoch_state.total_dogebtc_mined_in_epoch = 0;
            epoch_state.risk_factor_snapshot = 0;
            epoch_state.epoch_mining_pool = 0;
            epoch_state.faction_reward_pools = [0u64; NUM_FACTIONS];
            epoch_state.score_updates_count = 0;

            emit!(crate::events::EpochAutoStarted {
                epoch_id: epoch_state.epoch_id,
                start_timestamp: epoch_state.start_timestamp,
                end_timestamp: epoch_state.end_timestamp,
            });
            msg!("   🌍 Auto-initialized epoch {} ({} -> {})",
                epoch_state.epoch_id, epoch_state.start_timestamp, epoch_state.end_timestamp);
        }

        // Initialize user_epoch_bets if first bet this epoch
        if user_epoch_bets.owner == Pubkey::default() {
            user_epoch_bets.owner = owner_key;
            user_epoch_bets.epoch_id = epoch_state.epoch_id;
            user_epoch_bets.faction_bets = [0u64; NUM_FACTIONS];
            user_epoch_bets.bump = user_epoch_bets_bump;
        }

        // Map each block bet to faction and accumulate
        for block_id in evt_target_blocks.iter() {
            let faction = game_session.block_assignments[*block_id as usize] as usize;
            if faction < NUM_FACTIONS {
                user_epoch_bets.faction_bets[faction] += wgtd_points_per_bet;
                epoch_state.faction_total_sol_bets[faction] += wgtd_points_per_bet;
            }
        }
    }

    // === INSTANT MUTATION & XP LOGIC ===
    // Only if RPG progression is enabled, SOL bet > 0, and player has gameplay_doge
    let faction_id = player_data.faction_id as usize;
    if global_config.rpg_progression
        && amount_per_bet > 0
        && use_ticket.is_none()
        && user_game_bet.mutation_type == 0
        && player_data.gameplay_doge != Pubkey::default()
    {
        // Update highest bet for faction
        if user_game_bet.total_sol_bet > game_session.highest_sol_bet_per_faction[faction_id] {
            game_session.highest_sol_bet_per_faction[faction_id] = user_game_bet.total_sol_bet;
        }

        // Calculate mutation result (generation derived from DNA)
        let doge_mint = player_data.gameplay_doge;
        let mutation_result = calculate_mutation_result(
            round_id,
            user_game_bet.total_sol_bet,
            game_session.highest_sol_bet_per_faction[faction_id],
            player_data.active_multiplier,
            player_data.gameplay_doge_dna,
            player_data.gameplay_doge_xp,
            game_session.total_sol_bets,
            game_session.total_points_bets,
            game_session.total_wgtd_points_bets,
            clock.slot,
            &owner_key,
            &doge_mint,
        );

        // Always add XP to PlayerData (even without mutation)
        player_data.gameplay_doge_xp = player_data.gameplay_doge_xp + mutation_result.xp_gained;

        // Process mutation if triggered
        if let Some(mutation_type) = mutation_result.mutation_type {
            // Evolution: only 1 per faction per round. Power/Trait: unlimited
            let is_evolution = matches!(mutation_type, MutationType::Evolution);
            let can_apply =
                !is_evolution || !game_session.mutation_occurred_per_faction[faction_id];

            player_data.active_multiplier =
                player_data.active_multiplier + mutation_result.multiplier_increase;

            if can_apply {
                user_game_bet.mutation_type = match mutation_type {
                    MutationType::Evolution => 1,
                    MutationType::Power => 2,
                    MutationType::Trait => 3,
                };
                player_data.gameplay_doge_dna = mutation_result.new_dna;

                if is_evolution {
                    game_session.mutation_occurred_per_faction[faction_id] = true;
                }

                emit!(MutationTriggered {
                    round_id,
                    user: owner_key,
                    doge_mint: player_data.gameplay_doge,
                    xp_gained: mutation_result.xp_gained,
                });

                msg!(
                    "🧬 Mutation! Type: {}, Mult: {}",
                    user_game_bet.mutation_type,
                    player_data.active_multiplier
                );
            }
        }

        msg!("   XP: {}", player_data.gameplay_doge_xp);
    }

    Ok(())
}

/// Get the target block ID from bet_type (0-indexed: 0-23)
/// For Block bets, returns the block_id directly (0-indexed)
/// For FactionHighestLowest bets, finds the faction's blocks and returns highest/lowest (0-indexed)
fn get_target_block_from_bet_type(
    bet_type: &BetType,
    block_assignments: &[u8; NUM_BLOCKS],
) -> Result<u8> {
    match bet_type {
        BetType::Block { block_id } => {
            require!(*block_id < NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
            Ok(*block_id)
        }
        BetType::FactionHighestLowest {
            faction_id,
            is_highest,
        } => {
            require!(
                (*faction_id as usize) < block_assignments.len(),
                ErrorCode::InvalidParameters
            );
            // Find the two blocks assigned to this faction (0-indexed)
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }

            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );

            if *is_highest {
                Ok(*faction_blocks.iter().max().unwrap())
            } else {
                Ok(*faction_blocks.iter().min().unwrap())
            }
        }
        BetType::FactionBoth { faction_id } => {
            // For "both", return the highest block (will be expanded in batch function)
            require!(
                (*faction_id as usize) < block_assignments.len(),
                ErrorCode::InvalidParameters
            );
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }
            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );
            Ok(*faction_blocks.iter().max().unwrap()) // Return highest, but will be expanded
        }
        BetType::RandomBlock => {
            // Random block - use clock slot for randomness (0-indexed: 0-23)
            let clock = Clock::get()?;
            let slot_bytes = clock.slot.to_le_bytes();
            let random_block = (slot_bytes[0] % 24) as u8; // 0-23
            Ok(random_block)
        }
    }
}

fn handle_fee(amount: u64, protocol_fee_pct: u64) -> Result<(u64, u64)> {
    let fee = amount.saturating_mul(protocol_fee_pct) / M_HUNDRED;
    let net_amount = amount.saturating_sub(fee);
    msg!(
        "     Net amount (after fee): {} SOL. Protocol fee ({}%): {} SOL",
        (net_amount as f64) / 1_000_000_000.0,
        protocol_fee_pct,
        (fee as f64) / 1_000_000_000.0
    );
    return Ok((net_amount, fee));
}

fn validate_points_percentage_limit(
    current_points_bets: u64,
    current_sol_bets: u64,
    amount: u64,
) -> Result<()> {
    // Validate points percentage limit: points bets must stay at or below 25% of SOL bets for this session
    // Tickets can only be used when: (total_points_bets + ticket_amount) <= (total_sol_bets * 25 / 100)
    let new_points_bets = current_points_bets + amount;
    msg!("     Current session stats: SOL bets: {} lamports, Points bets: {} lamports, New points bets if allowed: {} lamports", current_sol_bets, current_points_bets, new_points_bets);

    // Require that SOL bets exist before allowing ticket bets -  This ensures points percentage can be calculated and stays within 25% limit
    require!(current_sol_bets > 0, ErrorCode::InvalidParameters);
    msg!("     ✓ SOL bets exist in session");

    // Calculate max allowed points bets (25% of SOL bets) -  This ensures points percentage can be calculated and stays within 25% limit
    let max_allowed_points = current_sol_bets * 25 / 100;
    msg!(
        "       Max allowed points (25% of SOL): {} lamports",
        max_allowed_points
    );
    require!(
        new_points_bets <= max_allowed_points,
        ErrorCode::InvalidParameters
    );
    msg!("     ✓ Points bets stay within 25% limit");
    Ok(())
}

// Compute blocks to bet on based on blocks_config and clock
fn compute_blocks_to_bet(
    blocks_config: Option<BlocksConfig>,
    clock: &Clock,
) -> Result<Option<Vec<u8>>> {
    let blocks_to_bet = if let Some(ref blocks_cfg) = blocks_config {
        match blocks_cfg {
            BlocksConfig::Specific { blocks } => Some(blocks.clone()),
            BlocksConfig::Random { count } => {
                // Generate random blocks using slot hash
                let mut random_blocks = Vec::new();
                let mut used_blocks = [false; 24];
                let mut attempts: u64 = 0;
                while random_blocks.len() < *count as usize && attempts < 100 {
                    let slot_bytes = clock.slot.to_le_bytes();
                    let hash = keccak::hash(&[slot_bytes, attempts.to_le_bytes()].concat());
                    let block_id = (hash.0[0] % 24) as u8;
                    if !used_blocks[block_id as usize] {
                        random_blocks.push(block_id);
                        used_blocks[block_id as usize] = true;
                    }
                    attempts += 1;
                }
                require!(
                    random_blocks.len() == *count as usize,
                    ErrorCode::InvalidParameters
                );
                Some(random_blocks)
            }
        }
    } else {
        None
    };

    if let Some(ref blocks) = blocks_to_bet {
        msg!("     Blocks to bet on: {:?}", blocks);
    }

    Ok(blocks_to_bet)
}

/// Generate bet types from blocks_config and factions_config
/// Returns vector of bet types to place
fn make_bets_vec<'info>(
    factions_config: Option<FactionsConfig>,
    blocks_to_bet: Option<Vec<u8>>,
    game_session: &Account<'info, GameSession>,
    clock: &Clock,
    global_config: &Account<'info, GlobalConfig>,
) -> Result<Vec<BetType>> {
    let mut bet_types = Vec::new();

    // Generate faction bets if factions_config is provided
    if let Some(ref factions_cfg) = factions_config {
        let factions_to_bet = match factions_cfg {
            FactionsConfig::Specific { factions, .. } => factions.clone(),
            FactionsConfig::Random { count, .. } => {
                // Generate random factions
                let mut random_factions = Vec::new();
                let mut used_factions = [false; 12];
                let mut attempts: u64 = 0;
                let max_factions = global_config.supported_factions.len();
                while random_factions.len() < *count as usize && attempts < 100 {
                    let slot_bytes = clock.slot.to_le_bytes();
                    let hash =
                        keccak::hash(&[slot_bytes, (attempts + 100u64).to_le_bytes()].concat());
                    let faction_id = (hash.0[0] % max_factions as u8) as u8;
                    if !used_factions[faction_id as usize] && (faction_id as usize) < max_factions {
                        random_factions.push(faction_id);
                        used_factions[faction_id as usize] = true;
                    }
                    attempts += 1;
                }
                require!(
                    random_factions.len() == *count as usize,
                    ErrorCode::InvalidParameters
                );
                random_factions
            }
        };

        let strategy = match factions_cfg {
            FactionsConfig::Specific { strategy, .. } => strategy,
            FactionsConfig::Random { strategy, .. } => strategy,
        };

        msg!(
            "     Factions to bet on: {:?} (strategy: {:?})",
            factions_to_bet,
            strategy
        );

        // Generate bet types for each faction
        // If blocks_to_bet provided, bet on those blocks for each faction
        // If no blocks_to_bet, bet on all blocks assigned to those factions in current round
        if let Some(ref blocks) = blocks_to_bet {
            // Blocks config provided - bet on specified blocks for each faction
            for &faction_id in factions_to_bet.iter() {
                match strategy {
                    FactionStrategy::Highest => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest {
                                faction_id,
                                is_highest: true,
                            });
                        }
                    }
                    FactionStrategy::Lowest => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest {
                                faction_id,
                                is_highest: false,
                            });
                        }
                    }
                    FactionStrategy::Both => {
                        for _block_id in blocks.iter() {
                            bet_types.push(BetType::FactionHighestLowest {
                                faction_id,
                                is_highest: true,
                            });
                            bet_types.push(BetType::FactionHighestLowest {
                                faction_id,
                                is_highest: false,
                            });
                        }
                    }
                }
            }
        } else {
            // No blocks config - bet on all blocks assigned to selected factions in current round
            for &faction_id in factions_to_bet.iter() {
                for block_id in 0..NUM_BLOCKS as u8 {
                    if game_session.block_assignments[block_id as usize] == faction_id {
                        match strategy {
                            FactionStrategy::Highest => {
                                bet_types.push(BetType::FactionHighestLowest {
                                    faction_id,
                                    is_highest: true,
                                });
                            }
                            FactionStrategy::Lowest => {
                                bet_types.push(BetType::FactionHighestLowest {
                                    faction_id,
                                    is_highest: false,
                                });
                            }
                            FactionStrategy::Both => {
                                bet_types.push(BetType::FactionHighestLowest {
                                    faction_id,
                                    is_highest: true,
                                });
                                bet_types.push(BetType::FactionHighestLowest {
                                    faction_id,
                                    is_highest: false,
                                });
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(ref blocks) = blocks_to_bet {
        // Only blocks config - bet on blocks directly
        for &block_id in blocks.iter() {
            bet_types.push(BetType::Block { block_id });
        }
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(bet_types)
}

/// Calculate caller compensation: 1% of sol_per_round, max 0.005 SOL
fn get_caller_compensation(sol_per_round: u64) -> Result<u64> {
    let caller_compensation = (sol_per_round / 100).min(crate::state::MAX_CALLER_COMPENSATION);
    Ok(caller_compensation)
}

// ========================================================================================
// =============================== ACCOUNT CONTEXTS ======================================
// ========================================================================================

#[derive(Accounts)]
#[instruction(faction_id: u8, referral_code: Option<Pubkey>)]
pub struct InitializePlayer<'info> {
    #[account(
        init,
        payer = authority,
        space = PlayerData::LEN,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// Optional: Referrer's referral rewards account (if referral code is provided)
    /// CHECK: Validated manually that owner matches referral_code pubkey
    #[account(mut)]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    #[account(
        init,
        payer = authority,
        space = ReferralRewards::LEN,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), authority.key().as_ref()],
        bump
    )]
    pub new_player_rewards: Account<'info, ReferralRewards>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ChangeFaction<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: SOL treasury PDA (50% of fee goes here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// Multisig WSOL token account (destination for WSOL transfers)
    /// MUST be owned by global_config.fee_recipient (the multisig address)
    #[account(
        mut,
        constraint = multisig_wsol_account.mint == wsol_mint.key() @ ErrorCode::InvalidMint,
        constraint = multisig_wsol_account.owner == global_config.fee_recipient @ ErrorCode::Unauthorized
    )]
    pub multisig_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// User's WSOL token account (for wrapping SOL to WSOL)
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = wsol_mint,
        associated_token::authority = authority,
    )]
    pub user_wsol_account: Account<'info, anchor_spl::token::TokenAccount>,

    /// CHECK: WSOL mint
    #[account(
        constraint = wsol_mint.key() == anchor_spl::token::spl_token::native_mint::id() @ ErrorCode::InvalidMint
    )]
    pub wsol_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct JoinRound<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    /// GameSession PDA for the current round (must be initialized by crank function)
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// UserGameBet PDA for this user's bet in this round
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: SOL rewards vault (staker fees go here)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// Epoch config (read for current_epoch_id)
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Box<Account<'info, EpochConfig>>,

    /// Epoch state for current epoch (init_if_needed for new epochs)
    #[account(
        init_if_needed,
        payer = authority,
        space = EpochState::LEN,
        seeds = [EPOCH_STATE_SEED, &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub epoch_state: Box<Account<'info, EpochState>>,

    /// User epoch bets for current epoch (init_if_needed for first bet)
    #[account(
        init_if_needed,
        payer = authority,
        space = UserEpochBets::LEN,
        seeds = [USER_EPOCH_BETS_SEED, authority.key().as_ref(), &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub user_epoch_bets: Box<Account<'info, UserEpochBets>>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Account struct for batch betting
/// Note: All bets must be for the same faction (same faction_state account)
#[derive(Accounts)]
pub struct JoinRoundBatch<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    /// GameSession PDA for the current round
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// UserGameBet PDA (shared across all bets in batch)
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: SOL rewards vault (staker fees go here)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// Epoch config (read for current_epoch_id)
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Box<Account<'info, EpochConfig>>,

    /// Epoch state for current epoch (init_if_needed for new epochs)
    #[account(
        init_if_needed,
        payer = authority,
        space = EpochState::LEN,
        seeds = [EPOCH_STATE_SEED, &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub epoch_state: Box<Account<'info, EpochState>>,

    /// User epoch bets for current epoch (init_if_needed for first bet)
    #[account(
        init_if_needed,
        payer = authority,
        space = UserEpochBets::LEN,
        seeds = [USER_EPOCH_BETS_SEED, authority.key().as_ref(), &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub user_epoch_bets: Box<Account<'info, UserEpochBets>>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct ClaimRoundRewards<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut, seeds = [UNREFINED_REWARDS_SEED.as_ref()], bump)]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump)]
    pub global_config: Account<'info, GlobalConfig>,

    /// Global game state (for current round entropy)
    #[account(seeds = [GLOBAL_GAME_STATE_SEED.as_ref()], bump = global_game_state.bump)]
    pub global_game_state: Account<'info, GlobalGameSate>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    #[account(mut, close = user_wallet)]
    pub user_game_bet: Account<'info, UserGameBet>,

    /// CHECK: User whose bet this is
    #[account(mut)]
    pub user_wallet: UncheckedAccount<'info>,

    /// Caller (bot or user themselves)
    pub caller: Signer<'info>,

    /// Optional DogeMetadata account for syncing mutation
    #[account(mut)]
    pub doge_metadata: Option<Box<Account<'info, DogeMetadata>>>,

    pub system_program: Program<'info, System>,
}

/// Context for claiming autominer rewards with auto-reload
#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct ClaimAutominerRewards<'info> {
    /// Autominer vault to reload
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    /// CHECK: Global autominer custody PDA holding SOL deposits
    #[account(
        mut,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut, seeds = [UNREFINED_REWARDS_SEED.as_ref()], bump)]
    pub unrefined_rewards: Account<'info, UnrefinedRewards>,

    #[account(seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()], bump = game_session.bump)]
    pub game_session: Account<'info, GameSession>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// User game bet account - will be closed
    #[account(mut, close = owner_wallet)]
    pub user_game_bet: Account<'info, UserGameBet>,

    /// CHECK: Owner wallet to receive leftover SOL / rent
    #[account(mut, constraint = owner_wallet.key() == autominer_vault.owner @ ErrorCode::Unauthorized)]
    pub owner_wallet: UncheckedAccount<'info>,

    /// Caller (backend script)
    pub caller: Signer<'info>,

    /// Optional DogeMetadata account for syncing mutation
    #[account(mut)]
    pub doge_metadata: Option<Box<Account<'info, DogeMetadata>>>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(blocks_config: Option<BlocksConfig>, factions_config: Option<FactionsConfig>, sol_per_round: u64, num_rounds: u32)]
pub struct InitAutominer<'info> {
    #[account(
        init_if_needed,
        payer = user_wallet,
        space = AutominerVault::LEN,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), user_wallet.key().as_ref()],
        bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    /// CHECK: Global autominer custody PDA holding SOL deposits
    #[account(
        mut,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAutominer<'info> {
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    /// CHECK: Global autominer custody PDA holding SOL deposits
    #[account(
        mut,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StopAutominer<'info> {
    #[account(
        mut,
        close = authority,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    /// CHECK: Autominer custody PDA holding SOL
    #[account(
        mut,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,

    /// CHECK: Owner account (to receive refunded SOL)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,

    /// Authority (must be owner)
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteAutominerBet<'info> {
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Box<Account<'info, AutominerVault>>,

    /// CHECK: Autominer custody PDA holding SOL deposits
    #[account(
        mut,
        seeds = [AUTOMINER_CUSTODY_SEED.as_ref()],
        bump
    )]
    pub autominer_custody: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// UserGameBet PDA for autominer bets (aggregates all bets from this vault for this round)
    #[account(
        init_if_needed,
        payer = caller,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: SOL treasury PDA
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: SOL rewards vault (staker fees go here)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// CHECK: Owner account (to receive remaining SOL when vault closes)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,

    /// Epoch config (read for current_epoch_id)
    #[account(
        mut,
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Box<Account<'info, EpochConfig>>,

    /// Epoch state for current epoch (init_if_needed for new epochs)
    #[account(
        init_if_needed,
        payer = caller,
        space = EpochState::LEN,
        seeds = [EPOCH_STATE_SEED, &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub epoch_state: Box<Account<'info, EpochState>>,

    /// User epoch bets for vault OWNER (init_if_needed, payer=caller)
    #[account(
        init_if_needed,
        payer = caller,
        space = UserEpochBets::LEN,
        seeds = [USER_EPOCH_BETS_SEED, autominer_vault.owner.as_ref(), &epoch_config.current_epoch_id.to_le_bytes()],
        bump,
    )]
    pub user_epoch_bets: Box<Account<'info, UserEpochBets>>,

    /// Caller (bot or anyone) - doesn't need to be owner
    #[account(mut)]
    pub caller: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// =============================== GAMEPLAY DOGE FUNCTIONS =================================
// ========================================================================================

/// Use an doge for gameplay - deposits doge to program custody and sets it as active gameplay doge
pub fn internal_use_doge_for_gameplay(ctx: Context<UseDogeForGameplay>) -> Result<()> {
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let doge_metadata = &mut ctx.accounts.doge_metadata;
    let doge_mint = doge_metadata.mint;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("🎮 === USING DOGE FOR GAMEPLAY ===");
    msg!("   Doge mint: {}", doge_mint);

    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.doge_asset)?;
    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );

    // Verify doge is not already incubated (staked)
    require!(
        doge_metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::DogeAlreadyAtGuard
    );

    // Verify no doge currently in gameplay
    require!(
        player_data.gameplay_doge == Pubkey::default(),
        ErrorCode::InvalidParameters
    );

    // Verify faction matches
    require!(
        doge_metadata.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Transfer NFT to custody PDA
    msg!("🔒 Transferring doge to custody PDA for gameplay");
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.doge_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;

    // Update player data - cache doge fields for mutation calculations
    // Note: generation is stored in DNA bits 4-6, not separately
    player_data.gameplay_doge = doge_mint;
    player_data.active_multiplier = doge_metadata.multiplier;
    player_data.gameplay_doge_dna = doge_metadata.dna;
    player_data.gameplay_doge_xp = doge_metadata.xp;

    // Update faction state
    faction_state.doges_playing += 1;

    // Update doge metadata
    doge_metadata.incubated_player_data = player_data.owner;
    doge_metadata.last_update_ts = current_time;

    let gen = crate::genescience::get_evolution_stage(&doge_metadata.dna);
    msg!("✅ Doge {} now active for gameplay", doge_mint);
    msg!(
        "   Multiplier: {}, Gen: {}, XP: {}",
        doge_metadata.multiplier,
        gen,
        doge_metadata.xp
    );
    msg!("   Faction doges playing: {}", faction_state.doges_playing);

    emit!(DogeUsedForGameplay {
        user: ctx.accounts.user.key(),
        doge_mint,
        faction_id: player_data.faction_id,
        timestamp: current_time,
    });

    Ok(())
}

/// Withdraw doge from gameplay - returns doge to user and clears gameplay doge
pub fn internal_withdraw_doge_from_gameplay(ctx: Context<WithdrawDogeFromGameplay>) -> Result<()> {
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let doge_metadata = &mut ctx.accounts.doge_metadata;
    let doge_mint = doge_metadata.mint;
    let current_time = Clock::get()?.unix_timestamp;

    msg!("🎮 === WITHDRAWING DOGE FROM GAMEPLAY ===");
    msg!("   Doge mint: {}", doge_mint);

    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.doge_asset)?;
    require!(
        nft_owner == ctx.accounts.doge_custody_pda.key(),
        ErrorCode::DogeNotAtGuard
    );

    // Verify this is the player's gameplay doge
    require!(
        player_data.gameplay_doge == doge_mint,
        ErrorCode::InvalidParameters
    );

    // Verify doge metadata matches player
    require!(
        doge_metadata.incubated_player_data == player_data.owner,
        ErrorCode::Unauthorized
    );

    // Transfer NFT back to user
    msg!("🔓 Transferring doge back to user");
    let custody_seeds = &[DOGE_CUSTODY_SEED, &[ctx.bumps.doge_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];

    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.doge_asset.to_account_info(),
        ctx.accounts
            .doge_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(), // Payer (User pays)
        &ctx.accounts.doge_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;

    // Sync cached data back to doge metadata before withdrawal
    // Note: generation is stored in DNA bits 4-6
    msg!("   Syncing gameplay progress to doge...");
    require!(
        doge_metadata.dna == player_data.gameplay_doge_dna || player_data.gameplay_doge_xp > 0,
        ErrorCode::ClaimPendingRoundRewards
    );
    doge_metadata.dna = player_data.gameplay_doge_dna;
    doge_metadata.xp = player_data.gameplay_doge_xp;
    doge_metadata.multiplier = player_data.active_multiplier;

    let gen = crate::genescience::get_evolution_stage(&doge_metadata.dna);
    msg!(
        "   Final stats - Mult: {}, Gen: {}, XP: {}",
        doge_metadata.multiplier,
        gen,
        doge_metadata.xp
    );

    // Clear player data gameplay fields
    player_data.gameplay_doge = Pubkey::default();
    player_data.active_multiplier = BASE_MULTIPLIER; // Reset to BASE_MULTIPLIER
    player_data.gameplay_doge_dna = [0u8; 32];
    player_data.gameplay_doge_xp = 0;

    // Update faction state
    faction_state.doges_playing = faction_state.doges_playing.saturating_sub(1);

    // Update doge metadata
    doge_metadata.incubated_player_data = Pubkey::default();
    doge_metadata.last_update_ts = current_time;

    msg!("✅ Doge {} withdrawn from gameplay", doge_mint);
    msg!("   Faction doges playing: {}", faction_state.doges_playing);

    emit!(DogeWithdrawnFromGameplay {
        user: ctx.accounts.user.key(),
        doge_mint,
        faction_id: player_data.faction_id,
        timestamp: current_time,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UseDogeForGameplay<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(), doge_metadata.mint.as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// CHECK: PDA for NFT custody
    #[account(seeds = [DOGE_CUSTODY_SEED], bump)]
    pub doge_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawDogeFromGameplay<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (in custody)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub doge_asset: UncheckedAccount<'info>,

    /// CHECK: Optional collection
    pub doge_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DOGE_METADATA_SEED.as_ref(), doge_metadata.mint.as_ref()],
        bump = doge_metadata.bump,
        constraint = doge_metadata.mint == doge_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub doge_metadata: Account<'info, DogeMetadata>,

    /// CHECK: PDA for NFT custody
    #[account(seeds = [DOGE_CUSTODY_SEED], bump)]
    pub doge_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}
