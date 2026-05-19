//! # User instructions
//!
//! User-facing game actions: player onboarding, manual bets, autominers, round
//! reward claims, gameplay HashBeast locking, and claim-time story events.
//!
//! A single bet is a country + direction prediction. The same bet powers:
//! - round rewards, where one country + direction is resolved randomly; and
//! - faction-war cycle accounting, where the oracle-set country movements
//!   settle epoch-style prediction rewards.
//!
//! Keep this file fail-closed: reward math must reject malformed bet vectors,
//! SOL transfers must preserve vault rent, so native-SOL payouts may be capped
//! to the withdrawable balance while optional HashBeast metadata must be proven
//! to be the canonical PDA before syncing NFT progression.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::system_program::{transfer, Transfer};
use mpl_core::ID as MPL_CORE_PROGRAM_ID;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::genescience::{calculate_mutation_result, MutationType};
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// =============================== USER TRANSACTION HANDLERS ==============================
// ========================================================================================

/// Initialize a player account for the DegenBTC country arena
pub fn internal_initialize_player(
    ctx: Context<InitializePlayer>,
    faction_id: u8,
    referral_code: Option<Pubkey>,
) -> Result<()> {
    crate::log_fn!("user", "internal_initialize_player");

    let player_data_key = ctx.accounts.player_data.key();
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;
    global_config.total_players = global_config
        .total_players
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );

    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;
    player_data.origin_faction_id = faction_id;
    player_data.referrer_faction_id = u8::MAX;

    // Treat the system-program sentinel as "no referral" so users can register without
    // providing a real referral code, even if the client forwards the sentinel explicitly.
    let referral_code = referral_code.filter(|code| *code != ctx.accounts.system_program.key());

    // Handle referral code logic
    let mut recruited_event: Option<PlayerRecruited> = None;
    let _referrer_pubkey = if let Some(ref_code) = referral_code {
        require!(
            ref_code != ctx.accounts.authority.key(),
            ErrorCode::ReferralCannotBeSameAsOwner
        );

        helper::validate_referrer_rewards_account(
            &ref_code,
            ctx.accounts.referrer_rewards.as_ref(),
        )?;

        let referrer_rewards = ctx
            .accounts
            .referrer_rewards
            .as_mut()
            .ok_or(ErrorCode::ReferralRewardsAccountRequired)?;
        referrer_rewards.referrals_count = referrer_rewards
            .referrals_count
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let referrer_faction_id = referrer_rewards.owner_faction_id;
        require!(
            (referrer_faction_id as usize) < global_config.supported_factions.len(),
            ErrorCode::InvalidFactionId
        );

        player_data.referral_code = ref_code;
        player_data.referrer_faction_id = referrer_faction_id;
        recruited_event = Some(PlayerRecruited {
            player: ctx.accounts.authority.key(),
            referrer: ref_code,
            player_origin_faction_id: faction_id,
            referrer_origin_faction_id: referrer_faction_id,
            referrer_total_recruits: referrer_rewards.referrals_count,
            timestamp: Clock::get()?.unix_timestamp,
        });
        ref_code
    } else {
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };

    player_data.active_multiplier = BASE_MULTIPLIER;

    player_data.degenbtc_hashpower = 0;
    player_data.degenbtc_staked = 0;
    player_data.degenbtc_degenbtc_reward_debt = 0;
    player_data.degenbtc_sol_reward_debt = 0;

    player_data.lp_hashpower = 0;
    player_data.lp_staked = 0;
    player_data.lp_sol_reward_debt = 0;
    player_data.lp_degenbtc_reward_debt = 0;

    player_data.pending_sol_rewards = 0;
    player_data.pending_dbtc_rewards = 0;
    player_data.pending_staking_dbtc_rewards = 0;
    player_data.pending_round_claims = 0;
    player_data.pending_war_claims = 0;

    player_data.degenbtc_position_indices = Vec::new();
    player_data.lp_position_indices = Vec::new();

    player_data.staked_hashbeasts = Vec::new();
    player_data.hashbeast_multiplier = BASE_MULTIPLIER as u16; // Default 1.0x (no hashbeasts staked)

    player_data.free_tickets = Vec::new();
    player_data.free_tickets_remaining = Vec::new();

    player_data.gameplay_hashbeast = Pubkey::default();
    player_data.gameplay_hashbeast_dna = [0u8; 32];
    player_data.gameplay_hashbeast_xp = 0;
    player_data.gameplay_unlock_request_faction_war = 0;
    player_data.current_war_score = 0;
    player_data.current_war_score_cycle_id = 0;

    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    new_player_rewards.owner_faction_id = faction_id;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.pending_sol_rewards = 0;
    new_player_rewards.total_sol_earned = 0;

    emit!(PlayerInitialized {
        user: ctx.accounts.authority.key(),
        player_data: player_data_key,
        faction_id,
        origin_faction_id: faction_id,
        referral_code,
        referrer_faction_id: if player_data.referrer_faction_id == u8::MAX {
            None
        } else {
            Some(player_data.referrer_faction_id)
        },
        timestamp: Clock::get()?.unix_timestamp,
    });
    if let Some(event) = recruited_event {
        emit!(event);
    }

    Ok(())
}

/// Initialize autominer vault for recurring faction-direction bets.
/// Round-based autominers only.
/// Can be called multiple times, but only when rounds_remaining == 0
/// SOL mode reserves `sol_per_round × num_rounds`; each round pays a small
/// keeper compensation and uses the remainder for generated bets.
/// Ticket mode accepts `sol_per_round == 0`; it internally reserves a fixed
/// keeper compensation per round while ticket value supplies generated bet points.
pub fn internal_init_autominer(
    ctx: Context<InitAutominer>,
    factions_config: Option<FactionsConfig>,
    sol_per_round: u64,
    num_rounds: u32,
    can_reload: bool,
    use_ticket: Option<u8>,
) -> Result<()> {
    crate::log_fn!("user", "internal_init_autominer");

    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;

    require!(num_rounds > 0, ErrorCode::InvalidAmount);
    if use_ticket.is_none() {
        require!(sol_per_round > 0, ErrorCode::InvalidAmount);
    } else {
        require!(sol_per_round == 0, ErrorCode::InvalidAmount);
    }
    require!(factions_config.is_some(), ErrorCode::InvalidParameters);

    // Check if vault already exists and has remaining rounds
    // Only allow initialization if rounds_remaining == 0 (must stop first if in progress)
    require!(
        autominer_vault.rounds_remaining == 0,
        ErrorCode::InvalidParameters
    );
    let mut bets_per_round = 0;

    // Validate factions_config if provided
    if let Some(ref factions_cfg) = factions_config {
        match factions_cfg {
            FactionsConfig::Specific { picks } => {
                require!(!picks.is_empty(), ErrorCode::InvalidParameters);
                require!(
                    picks.len() <= AutominerVault::MAX_PICKS,
                    ErrorCode::InvalidParameters
                );
                for pick in picks.iter() {
                    require!(
                        (pick.faction_id as usize) < global_config.supported_factions.len(),
                        ErrorCode::InvalidFactionId
                    );
                }
                bets_per_round = picks.len() as u64;
            }
            FactionsConfig::Random {
                count,
                direction: _,
            } => {
                require!(
                    *count > 0 && *count <= global_config.supported_factions.len() as u8,
                    ErrorCode::InvalidParameters
                );
                bets_per_round = *count as u64;
            }
        }
    }

    require!(bets_per_round > 0, ErrorCode::InvalidParameters);

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
        let _ = player_data.free_tickets[ticket_tier_index as usize];
    }

    let total_caller_compensation = if use_ticket.is_some() {
        get_ticket_caller_compensation()
    } else {
        get_caller_compensation(sol_per_round)?
    };

    // Calculate bet size per bet.
    // In SOL mode, reserve keeper compensation first and split the remaining round budget
    // evenly across generated country+direction picks.
    let bet_size_per_bet = if use_ticket.is_some() {
        0 // Resolved at execution time from player_data.free_tickets[tier]
    } else {
        let sol_for_betting = sol_per_round
            .checked_sub(total_caller_compensation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        sol_for_betting / bets_per_round
    };
    if use_ticket.is_none() {
        validate_min_sol_bet_per_position(bet_size_per_bet)?;
    }

    // Store config flags before moving values
    let has_factions_config = factions_config.is_some();

    // Calculate total SOL needed.
    // Both modes reserve SOL upfront. In ticket mode this is fixed keeper gas only.
    let reserve_per_round = if use_ticket.is_some() {
        total_caller_compensation
    } else {
        sol_per_round
    };
    let total_sol = reserve_per_round
        .checked_mul(num_rounds as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    autominer_vault.owner = ctx.accounts.user_wallet.key();
    autominer_vault.factions_config = factions_config;
    autominer_vault.sol_per_round = sol_per_round;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    autominer_vault.sol_balance = total_sol;
    autominer_vault.can_reload = can_reload;
    autominer_vault.use_ticket = use_ticket;
    autominer_vault.pending_autominer_claims = 0;
    autominer_vault.accrued_reload_sol = 0;

    // Transfer SOL to global autominer custody.
    if total_sol > 0 {
        helper::transfer_to_autominer_custody(
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol,
        )?;
    }

    emit!(AutominerInitialized {
        owner: ctx.accounts.user_wallet.key(),
        player_data: ctx.accounts.player_data.key(),
        gameplay_hashbeast: ctx.accounts.player_data.gameplay_hashbeast,
        autominer_vault: ctx.accounts.autominer_vault.key(),
        sol_per_round,
        num_rounds,
        bets_per_round,
        bet_size_per_bet,
        has_factions_config,
        can_reload,
        use_ticket,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[inline(never)]
pub fn internal_join_bets<'info>(
    accounts: &mut JoinBets<'info>,
    round_id: u64,
    war_id: u64,
    bet_types: Vec<BetType>,
    amount_per_bet: u64,
    use_ticket: Option<u8>,
    user_game_bet_bump: u8,
    user_war_bets_bump: u8,
) -> Result<()> {
    crate::log_fn!("user", "internal_join_bets");
    let global_config = load_global_config(&accounts.global_config.to_account_info())?;

    require!(
        accounts.game_session.round_id == round_id,
        ErrorCode::InvalidRound
    );

    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    require!(
        bet_types.len() <= UserGameBet::MAX_POSITIONS_PER_BET,
        ErrorCode::InvalidParameters
    );

    internal_process_bets(
        round_id,
        war_id,
        &global_config,
        &mut accounts.player_data,
        &mut accounts.game_session,
        &mut accounts.user_game_bet,
        &accounts.authority.to_account_info(),
        &accounts.sol_treasury.to_account_info(),
        &accounts.sol_rewards_vault.to_account_info(),
        &accounts.sol_prize_pot_vault.to_account_info(),
        &accounts.war_sol_vault.to_account_info(),
        &accounts.system_program.to_account_info(),
        user_game_bet_bump,
        accounts.authority.key(),
        amount_per_bet,
        bet_types.clone(),
        use_ticket,
        None, // User wallet signs the transaction
        None, // No autominer info
        &mut accounts.user_war_bets,
        user_war_bets_bump,
        accounts.referrer_rewards.as_mut(),
    )?;
    Ok(())
}

/// Execute autominer bets (keeper instruction - callable by anyone).
/// Generates faction-direction bets dynamically from the configured country set.
/// Pays the caller a small keeper compensation for tx costs.
/// SOL mode: 0.1% of `sol_per_round`, capped at 0.00005 SOL.
/// Ticket mode: pays a fixed 0.00005 SOL keeper reserve per round.
/// Uses the same round/faction_war betting path as manual users.
#[inline(never)]
pub fn internal_execute_autominer_bet<'info>(
    accounts: &mut ExecuteAutominerBet<'info>,
    current_round_id: u64,
    war_id: u64,
    user_game_bet_bump: u8,
    user_war_bets_bump: u8,
    custody_bump: u8,
) -> Result<()> {
    crate::log_fn!("user", "internal_execute_autominer_bet");
    let (expected_sol_treasury, _) =
        Pubkey::find_program_address(&[SOL_TREASURY_SEED.as_ref()], &crate::ID);
    require!(
        accounts.sol_treasury.key() == expected_sol_treasury,
        ErrorCode::InvalidAccount
    );

    let global_state: GlobalGameSate =
        load_program_account(&accounts.global_game_state.to_account_info())?;
    let global_config = load_global_config(&accounts.global_config.to_account_info())?;
    let clock = Clock::get()?;

    // Read values before mutable borrow
    let owner_key = accounts.autominer_vault.owner;
    let rounds_remaining = accounts.autominer_vault.rounds_remaining;
    let last_bet_round_id = accounts.autominer_vault.last_bet_round_id;
    let sol_per_round = accounts.autominer_vault.sol_per_round;
    let factions_config = accounts.autominer_vault.factions_config.clone();
    let sol_balance = accounts.autominer_vault.sol_balance;
    let use_ticket = accounts.autominer_vault.use_ticket;
    let autominer_custody_info = accounts.autominer_custody.to_account_info();

    require!(
        accounts.game_session.round_id == current_round_id,
        ErrorCode::InvalidRound
    );
    require!(
        global_state.current_round_id == current_round_id,
        ErrorCode::InvalidRound
    );

    if rounds_remaining == 0 || last_bet_round_id == current_round_id {
        return Ok(());
    }

    require!(rounds_remaining > 0, ErrorCode::NoRoundsRemaining);

    // Generate bet types using helper function
    let bet_types = make_bets_vec(factions_config.clone(), &clock, &global_config)?;

    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);

    let total_caller_compensation = if use_ticket.is_some() {
        get_ticket_caller_compensation()
    } else {
        get_caller_compensation(sol_per_round)?
    };
    let reserve_per_round = if use_ticket.is_some() {
        total_caller_compensation
    } else {
        sol_per_round
    };
    require!(
        sol_balance >= reserve_per_round,
        ErrorCode::InsufficientFunds
    );

    // Determine bet parameters based on mode (SOL vs tickets)
    let (bet_size_per_bet, effective_use_ticket) = if let Some(ticket_tier_index) = use_ticket {
        // Ticket mode: bet amount comes from player's ticket value.
        // SOL is reserved only for keeper compensation; tickets provide bet points.
        let player_data = &accounts.player_data;
        require!(
            (ticket_tier_index as usize) < player_data.free_tickets.len(),
            ErrorCode::InvalidParameters
        );
        let ticket_value = player_data.free_tickets[ticket_tier_index as usize];
        require!(ticket_value > 0, ErrorCode::InvalidAmount);
        (ticket_value, Some(ticket_tier_index))
    } else {
        // SOL mode: deduct caller compensation from sol_per_round to get betting amount
        let sol_for_betting = sol_per_round
            .checked_sub(total_caller_compensation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let bet_per = sol_for_betting / bet_types.len() as u64;
        validate_min_sol_bet_per_position(bet_per)?;
        (bet_per, None)
    };

    // Pay caller compensation.
    if total_caller_compensation > 0 {
        let autominer_seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];
        transfer(
            CpiContext::new_with_signer(
                accounts.system_program.to_account_info(),
                Transfer {
                    from: autominer_custody_info.clone(),
                    to: accounts.caller.to_account_info(),
                },
                &[autominer_seeds],
            ),
            total_caller_compensation,
        )?;
    }

    // Now borrow mutably to update state
    let autominer_vault = &mut accounts.autominer_vault;
    autominer_vault.last_bet_round_id = current_round_id;

    // Decrement rounds remaining
    let new_rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    autominer_vault.rounds_remaining = new_rounds_remaining;

    // Update remaining SOL balance tracked for this autominer.
    autominer_vault.sol_balance = autominer_vault
        .sol_balance
        .checked_sub(reserve_per_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Prepare PDA signer seeds for autominer custody
    let autominer_seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];

    // Prepare autominer info
    let autominer_info = AutominerBetInfo {
        vault: accounts.autominer_vault.key(),
        caller: accounts.caller.key(),
        compensation: total_caller_compensation,
        rounds_remaining: new_rounds_remaining,
    };

    // Call internal_process_bets with autominer vault as payer (PDA signs via seeds)
    // Process all bets at once
    internal_process_bets(
        current_round_id,
        war_id,
        &global_config,
        &mut accounts.player_data,
        &mut accounts.game_session,
        &mut accounts.user_game_bet,
        &autominer_custody_info,
        &accounts.sol_treasury.to_account_info(),
        &accounts.sol_rewards_vault.to_account_info(),
        &accounts.sol_prize_pot_vault.to_account_info(),
        &accounts.war_sol_vault.to_account_info(),
        &accounts.system_program.to_account_info(),
        user_game_bet_bump,
        owner_key,
        bet_size_per_bet,
        bet_types.clone(),
        effective_use_ticket,  // None for SOL, Some(tier) for tickets
        Some(autominer_seeds), // PDA signs via seeds
        Some(autominer_info),
        &mut accounts.user_war_bets,
        user_war_bets_bump,
        accounts.referrer_rewards.as_mut(),
    )?;

    // Bet placed successfully; this bet now expects a future claim. Drives the
    // bulk-reload trigger in `claim_autominer_rewards`.
    accounts.autominer_vault.pending_autominer_claims = accounts
        .autominer_vault
        .pending_autominer_claims
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    Ok(())
}

/// Update autominer run controls (add rounds, can_reload)
/// Can only be called by vault owner
/// Handles the extra SOL reserve for newly added rounds.
pub fn internal_update_autominer(
    ctx: Context<UpdateAutominer>,
    rounds_added_: Option<u32>,
    can_reload: Option<bool>,
) -> Result<()> {
    crate::log_fn!("user", "internal_update_autominer");

    let autominer_vault = &mut ctx.accounts.autominer_vault;

    require!(
        ctx.accounts.user_wallet.key() == autominer_vault.owner,
        ErrorCode::Unauthorized
    );

    // Read current values
    let old_sol_per_round = autominer_vault.sol_per_round;
    let rounds_remaining = autominer_vault.rounds_remaining;
    let old_can_reload = autominer_vault.can_reload;
    let old_sol_balance = autominer_vault.sol_balance;

    // Apply updates
    let new_sol_per_round = old_sol_per_round;
    let new_can_reload = can_reload.unwrap_or(old_can_reload);
    let rounds_added = rounds_added_.unwrap_or(0);
    require!(
        rounds_added > 0 || can_reload.is_some(),
        ErrorCode::InvalidParameters
    );
    let new_rounds_remaining = rounds_remaining
        .checked_add(rounds_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Validate the existing per-round reserve based on autominer mode. Updating
    // only supports adding more funded rounds or toggling reload; stake-size
    // changes require stopping the autominer and re-initializing.
    if autominer_vault.use_ticket.is_none() {
        require!(new_sol_per_round > 0, ErrorCode::InvalidAmount);
        let bets_per_round = count_autominer_bets_per_round(&autominer_vault.factions_config)?;
        let caller_compensation = get_caller_compensation(new_sol_per_round)?;
        let sol_for_betting = new_sol_per_round
            .checked_sub(caller_compensation)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let bet_size_per_bet = sol_for_betting / bets_per_round;
        validate_min_sol_bet_per_position(bet_size_per_bet)?;
    } else {
        require!(new_sol_per_round == 0, ErrorCode::InvalidAmount);
    }

    // SOL accounting covers both modes. In ticket mode, the per-round reserve is
    // only the keeper compensation. Updates are deposit-only, so a reload toggle
    // can never withdraw from custody.
    let reserve_per_round = if autominer_vault.use_ticket.is_some() {
        get_ticket_caller_compensation()
    } else {
        old_sol_per_round
    };
    let deposit_amount = (rounds_added as u64)
        .checked_mul(reserve_per_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if deposit_amount > 0 {
        helper::transfer_to_autominer_custody(
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            deposit_amount,
        )?;
    }

    // Update vault state
    autominer_vault.sol_per_round = new_sol_per_round;
    autominer_vault.rounds_remaining = new_rounds_remaining;
    autominer_vault.can_reload = new_can_reload;
    autominer_vault.sol_balance = old_sol_balance
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let sol_diff_for_event =
        i64::try_from(deposit_amount).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    emit!(AutominerUpdated {
        owner: autominer_vault.owner,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: autominer_vault.key(),
        sol_per_round: new_sol_per_round,
        rounds_remaining: new_rounds_remaining,
        can_reload: new_can_reload,
        sol_diff: sol_diff_for_event,
    });

    Ok(())
}

/// Stop autominer and refund remaining SOL
/// Can only be called by vault owner
/// Refunds all remaining SOL (after rent) and resets rounds_remaining to 0
pub fn internal_stop_autominer(ctx: Context<StopAutominer>) -> Result<()> {
    crate::log_fn!("user", "internal_stop_autominer");

    // Read values before mutable borrow
    let owner_key = ctx.accounts.autominer_vault.owner;
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let sol_balance = ctx.accounts.autominer_vault.sol_balance;
    let accrued_reload_sol = ctx.accounts.autominer_vault.accrued_reload_sol;

    require!(
        ctx.accounts.authority.key() == owner_key,
        ErrorCode::Unauthorized
    );

    // Refund both the funded-rounds reserve (`sol_balance`) and any earned-but-
    // not-yet-converted SOL parked in `accrued_reload_sol`. Both buckets sit in
    // the same custody PDA, so a single transfer of the sum returns it all.
    let refund_amount = sol_balance
        .checked_add(accrued_reload_sol)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    if refund_amount > 0 {
        helper::transfer_from_autominer_custody(
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.owner.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            refund_amount,
            ctx.bumps.autominer_custody,
        )?;
    }
    ctx.accounts.autominer_vault.accrued_reload_sol = 0;

    emit!(AutominerStopped {
        owner: owner_key,
        player_data: ctx.accounts.player_data.key(),
        autominer_vault: ctx.accounts.autominer_vault.key(),
        rounds_remaining,
        refund_amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Claim rewards for a user after round ends.
/// Round payouts depend on the winning faction plus the randomly resolved round direction.
pub fn internal_claim_round_rewards(round_id: u64, ctx: Context<ClaimRoundRewards>) -> Result<()> {
    crate::log_fn!("user", "internal_claim_round_rewards");

    let player_data_key = ctx.accounts.player_data.key();
    let game_session = &mut ctx.accounts.game_session;
    let user_bet = &mut ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;
    let owner_key = ctx.accounts.user_wallet.key();

    // Round should be completely over before user can claim rewards
    require!(game_session.stage == 2, ErrorCode::InvalidStage);
    require!(
        round_id == user_bet.round_id && round_id == game_session.round_id,
        ErrorCode::InvalidRound
    );
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);
    require_keys_eq!(user_bet.owner, owner_key, ErrorCode::InvalidOwner);

    let (total_sol_reward, total_dbtc_reward) = calculate_round_rewards(user_bet, game_session)?;
    let claim_won = total_sol_reward > 0 || total_dbtc_reward > 0;

    let mutation_type = if claim_won && user_bet.mutation_type == 0 {
        if let Some(roll) =
            build_round_claim_mutation_roll(user_bet, game_session, player_data.faction_id)?
        {
            let faction_id = roll.faction_id;
            let mutation_type = roll_claim_mutation(
                STORY_EVENT_ORIGIN_ROUND,
                round_id,
                user_bet.gameplay_hashbeast,
                owner_key,
                player_data,
                &ctx.accounts.global_config.gameplay_tuning,
                roll,
            )?;
            user_bet.mutation_type = mutation_type;
            record_round_claim_mutation(game_session, faction_id, mutation_type)?;
            mutation_type
        } else {
            0
        }
    } else {
        user_bet.mutation_type
    };

    update_player_rewards(
        owner_key,
        player_data_key,
        player_data,
        &mut ctx.accounts.hodl_pool,
        total_sol_reward,
        total_dbtc_reward,
        round_id,
    )?;

    // Transfer SOL winnings directly to user from prize pot vault. If the
    // vault only has `reward - rent_dust` available, pay the available amount
    // and keep the claim moving.
    let mut sol_reward_paid = 0u64;
    if total_sol_reward > 0 {
        sol_reward_paid = helper::transfer_from_sol_prize_pot_vault(
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol_reward,
            ctx.bumps.sol_prize_pot_vault,
        )?;
    }

    // === ACCUMULATED VALUE & CLAIM-TIME MUTATION SYNC ===
    let accum_pct = mutation_accum_pct(mutation_type);
    let accum_add = if total_dbtc_reward > 0 {
        u64::try_from(helper::mul_div(total_dbtc_reward, accum_pct, 1000)?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    sync_claim_hashbeast_state(
        user_bet.gameplay_hashbeast,
        player_data,
        ctx.accounts.hashbeast_metadata.as_mut(),
        accum_add,
        accum_pct as u32,
        claim_won,
    )?;

    // Mutation-bonus cycle score. Gated internally on mutation_type > 0 +
    // cycle still active. Skipped silently if either fails.
    apply_mutation_bonus_score(
        mutation_type,
        user_bet,
        game_session,
        &mut ctx.accounts.war_state,
        &mut ctx.accounts.user_war_bets,
        player_data,
        owner_key,
    )?;

    // Loser lottery roll (inline). Skipped silently for winners or any
    // ineligibility — see `maybe_run_loser_lootbox_roll`.
    maybe_run_loser_lootbox_roll(
        claim_won,
        user_bet,
        game_session,
        player_data,
        &mut ctx.accounts.lootbox_queue,
        &ctx.accounts.lootbox_claim.to_account_info(),
        &ctx.accounts.caller.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.lootbox_claim,
        owner_key,
        round_id,
    )?;

    player_data.pending_round_claims = player_data
        .pending_round_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    emit!(RoundRewardsClaimed {
        user: ctx.accounts.player_data.owner,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: sol_reward_paid,
        dbtc_reward: total_dbtc_reward,
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
    crate::log_fn!("user", "internal_claim_autominer_rewards");

    let player_data_key = ctx.accounts.player_data.key();
    let game_session = &mut ctx.accounts.game_session;
    let user_bet = &mut ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let owner_key = autominer_vault.owner;

    // Round should be completely over
    require!(game_session.stage == 2, ErrorCode::InvalidStage);
    require!(
        round_id == user_bet.round_id && round_id == game_session.round_id,
        ErrorCode::InvalidRound
    );
    require_keys_eq!(player_data.owner, owner_key, ErrorCode::InvalidOwner);
    require_keys_eq!(user_bet.owner, owner_key, ErrorCode::InvalidOwner);

    let (total_sol_reward, total_dbtc_reward) = calculate_round_rewards(user_bet, game_session)?;
    let claim_won = total_sol_reward > 0 || total_dbtc_reward > 0;

    let mutation_type = if claim_won && user_bet.mutation_type == 0 {
        if let Some(roll) =
            build_round_claim_mutation_roll(user_bet, game_session, player_data.faction_id)?
        {
            let faction_id = roll.faction_id;
            let mutation_type = roll_claim_mutation(
                STORY_EVENT_ORIGIN_ROUND,
                round_id,
                user_bet.gameplay_hashbeast,
                owner_key,
                player_data,
                &ctx.accounts.global_config.gameplay_tuning,
                roll,
            )?;
            user_bet.mutation_type = mutation_type;
            record_round_claim_mutation(game_session, faction_id, mutation_type)?;
            mutation_type
        } else {
            0
        }
    } else {
        user_bet.mutation_type
    };

    update_player_rewards(
        owner_key,
        player_data_key,
        player_data,
        &mut ctx.accounts.hodl_pool,
        total_sol_reward,
        total_dbtc_reward,
        round_id,
    )?;

    // === ACCUMULATED VALUE & CLAIM-TIME MUTATION SYNC ===
    let accum_pct = mutation_accum_pct(mutation_type);
    let accum_add = if total_dbtc_reward > 0 {
        u64::try_from(helper::mul_div(total_dbtc_reward, accum_pct, 1000)?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    sync_claim_hashbeast_state(
        user_bet.gameplay_hashbeast,
        player_data,
        ctx.accounts.hashbeast_metadata.as_mut(),
        accum_add,
        accum_pct as u32,
        claim_won,
    )?;

    apply_mutation_bonus_score(
        mutation_type,
        user_bet,
        game_session,
        &mut ctx.accounts.war_state,
        &mut ctx.accounts.user_war_bets,
        player_data,
        owner_key,
    )?;

    maybe_run_loser_lootbox_roll(
        claim_won,
        user_bet,
        game_session,
        player_data,
        &mut ctx.accounts.lootbox_queue,
        &ctx.accounts.lootbox_claim.to_account_info(),
        &ctx.accounts.caller.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.lootbox_claim,
        owner_key,
        round_id,
    )?;

    player_data.pending_round_claims = player_data
        .pending_round_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // === AUTO-RELOAD LOGIC ===
    // Auto-reload uses SOL winnings to fund more rounds in both modes.
    // In ticket mode, funded SOL is keeper compensation reserve; ticket balance still gates execution.
    let payable_sol_reward = if total_sol_reward > 0 {
        helper::native_lamports_available_after_rent(
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        )?
        .min(total_sol_reward)
    } else {
        0
    };
    if payable_sol_reward < total_sol_reward {
        msg!(
            "   ⚠️ Capping autominer SOL reward for rent reserve: requested={} actual={}",
            total_sol_reward,
            payable_sol_reward
        );
    }
    let mut sol_reward_paid = 0u64;

    // Each claim consumes one queued autominer bet. Drives the bulk-reload trigger below.
    autominer_vault.pending_autominer_claims = autominer_vault
        .pending_autominer_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // reserve_per_round = the unit cost the bulk reload will quote each "round" at.
    //   - Ticket mode:  fixed keeper-compensation reserve (ticket value drives bet points).
    //   - SOL mode:     autominer_vault.sol_per_round.
    //   - Either way:   zero means reload is not applicable for this vault config.
    let reserve_per_round: u64 = if autominer_vault.use_ticket.is_some() {
        get_ticket_caller_compensation()
    } else {
        autominer_vault.sol_per_round
    };
    let reload_eligible = autominer_vault.can_reload && reserve_per_round > 0;

    if payable_sol_reward > 0 {
        if reload_eligible {
            // Park this round's reward in autominer_custody; book it as accrued.
            // No event, no transfer to owner yet — wait for the last unclaimed
            // bet of the cycle to convert the accrued bucket in one shot.
            let parked = helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                payable_sol_reward,
                ctx.bumps.sol_prize_pot_vault,
            )?;
            autominer_vault.accrued_reload_sol = autominer_vault
                .accrued_reload_sol
                .checked_add(parked)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            sol_reward_paid = sol_reward_paid
                .checked_add(parked)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        } else {
            // No-reload path: pay payable_sol_reward straight to owner this claim.
            let paid = helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.owner_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                payable_sol_reward,
                ctx.bumps.sol_prize_pot_vault,
            )?;
            sol_reward_paid = sol_reward_paid
                .checked_add(paid)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    // FINAL-CLAIM BULK RELOAD: only fires once per funded cycle, when this
    // claim was the last unclaimed autominer bet AND we have accrued reward
    // SOL to convert. Translates the accrued bucket into additional rounds,
    // and sends the sub-round-size leftover back to the owner wallet.
    if reload_eligible
        && autominer_vault.pending_autominer_claims == 0
        && autominer_vault.accrued_reload_sol > 0
    {
        let accrued = autominer_vault.accrued_reload_sol;
        let rounds_to_add_u64 = accrued / reserve_per_round;
        let sol_for_rounds = rounds_to_add_u64
            .checked_mul(reserve_per_round)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        let leftover = accrued
            .checked_sub(sol_for_rounds)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        let rounds_to_add_u32 = u32::try_from(rounds_to_add_u64)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        if rounds_to_add_u32 > 0 {
            // SOL is already in autominer_custody from the accrual transfers.
            // Just bump the bookkeeping that locks it to rounds.
            autominer_vault.rounds_remaining = autominer_vault
                .rounds_remaining
                .checked_add(rounds_to_add_u32)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            autominer_vault.sol_balance = autominer_vault
                .sol_balance
                .checked_add(sol_for_rounds)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        let mut leftover_paid: u64 = 0;
        if leftover > 0 {
            leftover_paid = helper::transfer_from_autominer_custody(
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.owner_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                leftover,
                ctx.bumps.autominer_custody,
            )?;
        }

        autominer_vault.accrued_reload_sol = 0;

        emit!(AutominerReloaded {
            autominer_vault: autominer_vault.key(),
            rounds_to_add: rounds_to_add_u32,
            sol_for_rounds,
            leftover_sol: leftover_paid,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    emit!(RoundRewardsClaimed {
        user: ctx.accounts.autominer_vault.owner,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: sol_reward_paid,
        dbtc_reward: total_dbtc_reward,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ========================================================================================
// =============================== GAMEPLAY HASHBEAST FUNCTIONS =================================
// ========================================================================================

/// Use a HashBeast for gameplay - deposits the HashBeast to program custody and sets it as active gameplay HashBeast
pub fn internal_use_hashbeast_for_gameplay(ctx: Context<UseHashBeastForGameplay>) -> Result<()> {
    crate::log_fn!("user", "internal_use_hashbeast_for_gameplay");
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    let hashbeast_mint = hashbeast_metadata.mint;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        ctx.accounts.global_config.gameplay_tuning.rpg_progression,
        ErrorCode::GameplayNotEnabled
    );

    // Verify ownership
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.hashbeast_asset)?;
    require!(
        nft_owner == ctx.accounts.user.key(),
        ErrorCode::NftNotOwnedByUser
    );

    // Verify hashbeast is not already incubated (staked)
    require!(
        hashbeast_metadata.incubated_player_data == Pubkey::default(),
        ErrorCode::HashBeastAlreadyAtGuard
    );

    // Verify no hashbeast currently in gameplay
    require!(
        player_data.gameplay_hashbeast == Pubkey::default(),
        ErrorCode::InvalidParameters
    );
    require!(
        player_data.gameplay_unlock_request_faction_war == 0,
        ErrorCode::InvalidState
    );

    // Gameplay hashbeasts must match the player's current home faction.
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        hashbeast_metadata.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );

    // Transfer NFT to custody PDA
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.hashbeast_custody_pda.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        None,
    )?;

    // Update player data - cache hashbeast fields for mutation calculations
    // Note: generation is stored in DNA bits 4-6, not separately
    player_data.gameplay_hashbeast = hashbeast_mint;
    player_data.active_multiplier = hashbeast_metadata
        .multiplier
        .min(GAMEPLAY_MAX_MULTIPLIER as u32);
    player_data.gameplay_hashbeast_dna = hashbeast_metadata.dna;
    player_data.gameplay_hashbeast_xp = hashbeast_metadata.xp;
    player_data.gameplay_unlock_request_faction_war = 0;

    // Update faction state
    faction_state.hashbeasts_playing = faction_state
        .hashbeasts_playing
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Update hashbeast metadata
    hashbeast_metadata.incubated_player_data = player_data.owner;
    hashbeast_metadata.last_update_ts = current_time;

    emit!(HashBeastUsedForGameplay {
        user: ctx.accounts.user.key(),
        hashbeast_mint,
        timestamp: current_time,
    });

    Ok(())
}

/// Request gameplay hashbeast unlock. Actual withdrawal is only allowed after the next faction_war starts.
pub fn internal_request_hashbeast_gameplay_unlock(
    ctx: Context<RequestHashBeastGameplayUnlock>,
) -> Result<()> {
    crate::log_fn!("user", "internal_request_hashbeast_gameplay_unlock");
    let player_data = &mut ctx.accounts.player_data;
    let current_war_id = ctx.accounts.war_config.current_war_id;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        player_data.gameplay_hashbeast != Pubkey::default(),
        ErrorCode::InvalidState
    );
    require!(
        player_data.gameplay_unlock_request_faction_war == 0,
        ErrorCode::GameplayUnlockAlreadyRequested
    );

    player_data.gameplay_unlock_request_faction_war = current_war_id;

    emit!(HashBeastGameplayUnlockRequested {
        user: ctx.accounts.user.key(),
        hashbeast_mint: player_data.gameplay_hashbeast,
        requested_during_war_id: current_war_id,
        unlock_available_after_war_id: current_war_id
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?,
        timestamp: current_time,
    });

    Ok(())
}

/// Withdraw hashbeast from gameplay - returns hashbeast to user and clears gameplay hashbeast
pub fn internal_withdraw_hashbeast_from_gameplay(
    ctx: Context<WithdrawHashBeastFromGameplay>,
) -> Result<()> {
    crate::log_fn!("user", "internal_withdraw_hashbeast_from_gameplay");
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    let hashbeast_metadata = &mut ctx.accounts.hashbeast_metadata;
    let hashbeast_mint = hashbeast_metadata.mint;
    let current_time = Clock::get()?.unix_timestamp;

    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.hashbeast_asset)?;
    require!(
        nft_owner == ctx.accounts.hashbeast_custody_pda.key(),
        ErrorCode::HashBeastNotAtGuard
    );

    require!(
        player_data.gameplay_hashbeast == hashbeast_mint,
        ErrorCode::InvalidParameters
    );

    require!(
        hashbeast_metadata.incubated_player_data == player_data.owner,
        ErrorCode::Unauthorized
    );
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        hashbeast_metadata.faction_id == player_data.faction_id,
        ErrorCode::InvalidFactionId
    );
    require!(
        player_data.gameplay_unlock_request_faction_war != 0,
        ErrorCode::GameplayUnlockNotRequested
    );
    require!(
        ctx.accounts.war_config.current_war_id > player_data.gameplay_unlock_request_faction_war,
        ErrorCode::GameplayUnlockNotReady
    );
    require!(
        !player_has_pending_reward_claims(player_data),
        ErrorCode::GameplayRewardsPending
    );

    // Transfer NFT back to user
    let custody_seeds = &[HASHBEAST_CUSTODY_SEED, &[ctx.bumps.hashbeast_custody_pda]];
    let signer_seeds = &[&custody_seeds[..]];

    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.hashbeast_asset.to_account_info(),
        ctx.accounts
            .hashbeast_collection
            .as_ref()
            .map(|c| c.to_account_info())
            .as_ref(),
        &ctx.accounts.user.to_account_info(), // Payer (User pays)
        &ctx.accounts.hashbeast_custody_pda.to_account_info(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.mpl_core_program.to_account_info(),
        Some(signer_seeds),
    )?;

    // Sync cached data back to hashbeast metadata before withdrawal
    // Note: generation is stored in DNA bits 4-6
    hashbeast_metadata.dna = player_data.gameplay_hashbeast_dna;
    hashbeast_metadata.xp = player_data.gameplay_hashbeast_xp;
    hashbeast_metadata.multiplier = player_data.active_multiplier;

    // Clear player data gameplay fields.
    player_data.gameplay_hashbeast = Pubkey::default();
    player_data.active_multiplier = BASE_MULTIPLIER;
    player_data.gameplay_hashbeast_dna = [0u8; 32];
    player_data.gameplay_hashbeast_xp = 0;
    player_data.gameplay_unlock_request_faction_war = 0;

    // Update faction state
    faction_state.hashbeasts_playing = faction_state
        .hashbeasts_playing
        .checked_sub(1)
        .ok_or(ErrorCode::InvalidState)?;

    // Update hashbeast metadata
    hashbeast_metadata.incubated_player_data = Pubkey::default();
    hashbeast_metadata.last_update_ts = current_time;

    emit!(HashBeastWithdrawnFromGameplay {
        user: ctx.accounts.user.key(),
        hashbeast_mint,
        timestamp: current_time,
    });

    Ok(())
}

// ========================================================================================
// =============================== HELPER FUNCTIONS ======================================
// ========================================================================================

/// Internal join_bets logic for batched processing
/// Calculates totals, performs single transfers, and updates state for all bets
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn internal_process_bets<'info>(
    round_id: u64,
    war_id: u64,
    global_config: &GlobalConfig,
    player_data: &mut Account<'info, PlayerData>,
    game_session: &mut Account<'info, GameSession>,
    user_game_bet: &mut Account<'info, UserGameBet>,
    payer: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    sol_rewards_vault: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    war_sol_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_game_bet_bump: u8,
    owner_key: Pubkey,
    amount_per_bet: u64,
    bet_types: Vec<BetType>,
    use_ticket: Option<u8>,
    signer_seeds: Option<&[&[u8]]>,
    autominer_info: Option<AutominerBetInfo>,
    user_war_bets: &mut UserFactionWarBets,
    user_war_bets_bump: u8,
    referrer_rewards: Option<&mut Account<'info, ReferralRewards>>,
) -> Result<()> {
    let clock = Clock::get()?;

    require!(!global_config.is_paused, ErrorCode::GamePaused);
    require!(game_session.round_id == round_id, ErrorCode::InvalidRound);
    require!(game_session.stage == 0, ErrorCode::RoundEnded);
    require!(
        clock.unix_timestamp < game_session.round_end_timestamp,
        ErrorCode::RoundEnded
    );
    require!(
        amount_per_bet > 0 || use_ticket.is_some(),
        ErrorCode::InvalidAmount
    );
    if use_ticket.is_none() {
        validate_min_sol_bet_per_position(amount_per_bet)?;
    }
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);

    // Cross-check the war_id arg against the round's snapshot. The arg is
    // authenticated by the user_war_bets PDA seed (Anchor's seeds constraint
    // on the JoinBets / ExecuteAutominerBet account); `game_session.war_id_when_played`
    // was set at start_round under the cycle_end_round_id guard, so a valid
    // game_session here implies the war is current and active.
    require!(
        war_id == game_session.war_id_when_played,
        ErrorCode::InvalidState
    );

    if user_war_bets.owner == Pubkey::default() {
        user_war_bets.bump = user_war_bets_bump;
        user_war_bets.owner = owner_key;
        user_war_bets.war_id = war_id;
        user_war_bets.gameplay_hashbeast = Pubkey::default();
        user_war_bets.mutation_score = 0;
        user_war_bets.direction_bets = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        user_war_bets.sol_direction_bets = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        player_data.pending_war_claims = player_data
            .pending_war_claims
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    } else {
        require!(user_war_bets.owner == owner_key, ErrorCode::Unauthorized);
        require!(user_war_bets.war_id == war_id, ErrorCode::InvalidState);
    }

    // faction_count snapshot lives on war_state which we no longer load here.
    // Read from global_config — mid-war faction additions are not expected
    // (the active-cycle guard blocks new rounds during settle), so the
    // snapshot vs. live read is effectively the same for in-flight bets.
    let supported_faction_count = global_config.supported_factions.len();
    require!(
        supported_faction_count == NUM_FACTIONS,
        ErrorCode::InvalidFactionId
    );

    // Arrays to return for events
    let mut evt_faction_ids = Vec::new();
    let mut evt_directions = Vec::new();
    let mut evt_net_amounts = Vec::new();
    let mut evt_fee_amounts = Vec::new();
    let mut evt_points_amounts = Vec::new();
    let mut evt_wgtd_points_amounts = Vec::new();

    // Initialize totals
    let num_bets = bet_types.len() as u64;
    let mut total_stakers_fee = 0u64;
    let mut total_protocol_fee = 0u64;
    let mut total_net_to_pot = 0u64;
    let mut total_referral_cut = 0u64;

    let has_referrer = player_data.referral_code != system_program.key();

    // Get multiplier (default 1000 = 1x if not set)
    let active_mult = if player_data.active_multiplier == 0 {
        BASE_MULTIPLIER as u64
    } else {
        player_data.active_multiplier as u64
    };

    // --- CYCLE SOL SPLIT: computed upfront so it is available in outer scope ---
    let cycle_sol_split_pct = global_config.sol_fee_config.cycle_sol_split_pct as u64;
    let mut cycle_sol_split_per_bet: u64 = 0;

    // Calculate amounts per bet (uniform across batch)
    // wgtd_points: points * multiplier / BASE_MULTIPLIER for SOL bets, else points (tickets)
    let (net_per_bet, fee_per_bet, points_per_bet, wgtd_points_per_bet) = if let Some(
        ticket_type_index,
    ) = use_ticket
    {
        // Ticket Logic - no multiplier applied
        require!(
            (ticket_type_index as usize) < player_data.free_tickets.len(),
            ErrorCode::InvalidParameters
        );
        require!(
            (ticket_type_index as usize) < player_data.free_tickets_remaining.len(),
            ErrorCode::InvalidParameters
        );
        let ticket_value = player_data.free_tickets[ticket_type_index as usize];
        require!(amount_per_bet == ticket_value, ErrorCode::InvalidAmount);

        require!(
            player_data.free_tickets_remaining[ticket_type_index as usize] >= num_bets,
            ErrorCode::InsufficientFunds
        );

        // Validate total points limit
        let total_points = amount_per_bet
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        validate_points_percentage_limit(
            game_session.total_points_bets,
            game_session.total_sol_bets,
            total_points,
        )?;

        // Deduct tickets
        player_data.free_tickets_remaining[ticket_type_index as usize] = player_data
            .free_tickets_remaining[ticket_type_index as usize]
            .checked_sub(num_bets)
            .ok_or(ErrorCode::InsufficientFunds)?;

        (0, 0, amount_per_bet, amount_per_bet) // wgtd_points = points for tickets
    } else {
        // SOL Logic - apply multiplier for wgtd_points
        require!(amount_per_bet > 0, ErrorCode::InvalidAmount);

        cycle_sol_split_per_bet = if cycle_sol_split_pct > 0 {
            amount_per_bet
                .checked_mul(cycle_sol_split_pct)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / M_HUNDRED
        } else {
            0
        };
        // Protocol fee and referral are computed from the GROSS bet (principal),
        // not from the amount remaining after the cycle SOL split.
        let (net_after_fee, fee) = handle_fee(
            amount_per_bet,
            global_config.sol_fee_config.protocol_fee_pct as u64,
        )?;

        // Net to pot = gross - protocol fee - cycle split
        let net = net_after_fee
            .checked_sub(cycle_sol_split_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Referral fee: tiered based on faction alignment.
        // Same-country recruits: 1.0% of gross bet. Cross-country: 0.5%.
        // Deducted from protocol fee before staker/treasury split.
        let referral_cut_per_bet = if has_referrer {
            let same_faction = player_data.referrer_faction_id != u8::MAX
                && player_data.faction_id == player_data.referrer_faction_id;
            let bps = if same_faction {
                crate::state::REFERRAL_FEE_BPS_SAME_FACTION
            } else {
                crate::state::REFERRAL_FEE_BPS_CROSS_FACTION
            };
            let cut = u64::try_from(helper::mul_div(amount_per_bet, bps as u64, 10_000)?)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            cut.min(fee)
        } else {
            0
        };
        let effective_fee = fee
            .checked_sub(referral_cut_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Split remaining fee between stakers and treasury
        let stakers_fee = u64::try_from(helper::mul_div(
            effective_fee,
            global_config.sol_fee_config.stakers_pct as u64,
            M_HUNDRED,
        )?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?;
        let protocol_fee = effective_fee
            .checked_sub(stakers_fee)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Accumulate totals for transfer
        total_stakers_fee = stakers_fee
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        total_protocol_fee = protocol_fee
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        total_net_to_pot = net
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        total_referral_cut = referral_cut_per_bet
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // wgtd_points = points * multiplier / BASE_MULTIPLIER for SOL bets
        let wgtd = u64::try_from(helper::mul_div(net, active_mult, BASE_MULTIPLIER as u64)?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
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

    // Transfer cycle SOL split to faction-war vault
    let total_cycle_sol_split = if cycle_sol_split_per_bet > 0 {
        cycle_sol_split_per_bet
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    if total_cycle_sol_split > 0 {
        do_transfer(war_sol_vault, total_cycle_sol_split)?;
        // Track this round's SOL contribution on game_session. Folded into
        // war_state.sol_reward_pool at settle_round, same fold pattern as the
        // other GameSession → FactionWarState aggregates.
        game_session.cycle_sol_pool = game_session
            .cycle_sol_pool
            .checked_add(total_cycle_sol_split)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    let mut unpaid_referral_cut = 0u64;
    if total_referral_cut > 0 && has_referrer {
        helper::validate_referrer_rewards_account(
            &player_data.referral_code,
            referrer_rewards.as_deref(),
        )?;
        let rr = referrer_rewards.ok_or(ErrorCode::ReferralRewardsAccountRequired)?;
        let remaining_cap = MAX_REFERRER_SOL_LIFETIME.saturating_sub(rr.total_sol_earned);
        let referrer_cut = total_referral_cut.min(remaining_cap);
        unpaid_referral_cut = total_referral_cut
            .checked_sub(referrer_cut)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        if referrer_cut > 0 {
            do_transfer(&rr.to_account_info(), referrer_cut)?;
            rr.pending_sol_rewards = rr
                .pending_sol_rewards
                .checked_add(referrer_cut)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            rr.total_sol_earned = rr
                .total_sol_earned
                .checked_add(referrer_cut)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }
    if unpaid_referral_cut > 0 {
        total_protocol_fee = total_protocol_fee
            .checked_add(unpaid_referral_cut)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if total_stakers_fee > 0 {
        do_transfer(sol_rewards_vault, total_stakers_fee)?;
    }
    if total_protocol_fee > 0 {
        do_transfer(sol_treasury, total_protocol_fee)?;
    }
    if total_net_to_pot > 0 {
        do_transfer(sol_prize_pot_vault, total_net_to_pot)?;
    }

    // Initialize UserGameBet if needed.
    if user_game_bet.owner == Pubkey::default() {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.war_id = war_id;
        user_game_bet.faction_ids = Vec::new();
        user_game_bet.directions = Vec::new();
        user_game_bet.sol_bets = Vec::new();
        user_game_bet.points_bets = Vec::new();
        user_game_bet.wgtd_points_bets = Vec::new();
        user_game_bet.gameplay_hashbeast = player_data.gameplay_hashbeast;
        user_game_bet.total_sol_bet = 0;
        user_game_bet.total_points_bet = 0;
        user_game_bet.total_wgtd_points_bet = 0;
        user_game_bet.total_fee = 0;
        user_game_bet.bump = user_game_bet_bump;
        user_game_bet.mutation_type = 0;

        player_data.pending_round_claims = player_data
            .pending_round_claims
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    } else {
        require!(user_game_bet.round_id == round_id, ErrorCode::InvalidRound);
        require!(user_game_bet.war_id == war_id, ErrorCode::InvalidState);
    }

    // Process each faction-direction bet.
    for bet_type in bet_types {
        let (faction_id, direction) = prediction_bet_parts(&bet_type)?;
        require!(
            (faction_id as usize) < supported_faction_count,
            ErrorCode::InvalidFactionId
        );
        let faction_index = faction_id as usize;
        let direction_index = direction.as_index();
        let direction_u8 = direction_index as u8;

        let existing_position_index = user_game_bet
            .faction_ids
            .iter()
            .zip(user_game_bet.directions.iter())
            .position(|(&existing_faction, &existing_direction)| {
                existing_faction == faction_id && existing_direction == direction_u8
            });
        let faction_already_present = user_game_bet.faction_ids.contains(&faction_id);

        if let Some(index) = existing_position_index {
            user_game_bet.sol_bets[index] = user_game_bet.sol_bets[index]
                .checked_add(net_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            user_game_bet.points_bets[index] = user_game_bet.points_bets[index]
                .checked_add(points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            user_game_bet.wgtd_points_bets[index] = user_game_bet.wgtd_points_bets[index]
                .checked_add(wgtd_points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        } else {
            require!(
                user_game_bet.faction_ids.len() < UserGameBet::MAX_POSITIONS_PER_BET,
                ErrorCode::InvalidParameters
            );
            user_game_bet.faction_ids.push(faction_id);
            user_game_bet.directions.push(direction_u8);
            user_game_bet.sol_bets.push(net_per_bet);
            user_game_bet.points_bets.push(points_per_bet);
            user_game_bet.wgtd_points_bets.push(wgtd_points_per_bet);

            if !faction_already_present {
                game_session.user_faction_indexes[faction_index] = game_session
                    .user_faction_indexes[faction_index]
                    .checked_add(1)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            }
        }

        // Update GameSession stats. These per-faction-direction aggregates are
        // folded into FactionWarState + FactionWarConfig once per round at
        // settle_round, rather than touched per-bet — keeps JoinBets fast under
        // high throughput.
        game_session.sol_bets_by_faction[faction_index] = game_session.sol_bets_by_faction
            [faction_index]
            .checked_add(net_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        game_session.points_bets_by_faction_direction[faction_index][direction_index] =
            game_session.points_bets_by_faction_direction[faction_index][direction_index]
                .checked_add(points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        game_session.wgtd_points_bets_by_faction_direction[faction_index][direction_index] =
            game_session.wgtd_points_bets_by_faction_direction[faction_index][direction_index]
                .checked_add(wgtd_points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Per-user, per-war prediction record — drives base + hashbeast claim
        // pools at war end. Still updated per-bet (per-user state, no cheap
        // fold-in target).
        user_war_bets.direction_bets[faction_index][direction_index] = user_war_bets.direction_bets
            [faction_index][direction_index]
            .checked_add(wgtd_points_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user_war_bets.sol_direction_bets[faction_index][direction_index] = user_war_bets
            .sol_direction_bets[faction_index][direction_index]
            .checked_add(net_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Lock the cycle's gameplay hashbeast on the user's first home-faction
        // bet (while they have an HB deployed). HB-bonus payout itself is no
        // longer driven by bet stake — it's driven by mutation_score
        // accumulated during round-claim mutation rolls (see
        // apply_mutation_bonus_score). Tracking the HB pubkey here lets the
        // claim path validate and attribute mutations to the right beast.
        if faction_id == player_data.faction_id
            && player_data.gameplay_hashbeast != Pubkey::default()
        {
            if user_war_bets.gameplay_hashbeast == Pubkey::default() {
                user_war_bets.gameplay_hashbeast = player_data.gameplay_hashbeast;
            } else {
                require_keys_eq!(
                    user_war_bets.gameplay_hashbeast,
                    player_data.gameplay_hashbeast,
                    ErrorCode::InvalidAccount
                );
            }
        }

        // Record for events
        evt_faction_ids.push(faction_id);
        evt_directions.push(direction_u8);
        evt_net_amounts.push(net_per_bet);
        evt_fee_amounts.push(fee_per_bet);
        evt_points_amounts.push(points_per_bet);
        evt_wgtd_points_amounts.push(wgtd_points_per_bet);
    }

    // Update Totals
    let total_net_added = net_per_bet
        .checked_mul(num_bets)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_points_added = points_per_bet
        .checked_mul(num_bets)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_wgtd_points_added = wgtd_points_per_bet
        .checked_mul(num_bets)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_fee_added = fee_per_bet
        .checked_mul(num_bets)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    user_game_bet.total_sol_bet = user_game_bet
        .total_sol_bet
        .checked_add(total_net_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    user_game_bet.total_points_bet = user_game_bet
        .total_points_bet
        .checked_add(total_points_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    user_game_bet.total_wgtd_points_bet = user_game_bet
        .total_wgtd_points_bet
        .checked_add(total_wgtd_points_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    user_game_bet.total_fee = user_game_bet
        .total_fee
        .checked_add(total_fee_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_sol_bets = game_session
        .total_sol_bets
        .checked_add(total_net_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_points_bets = game_session
        .total_points_bets
        .checked_add(total_points_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_wgtd_points_bets = game_session
        .total_wgtd_points_bets
        .checked_add(total_wgtd_points_added)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.stakers_fee = game_session
        .stakers_fee
        .checked_add(total_stakers_fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

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
        gameplay_hashbeast: player_data.gameplay_hashbeast,
        gameplay_hashbeast_dna: player_data.gameplay_hashbeast_dna,
        active_multiplier: player_data.active_multiplier,
        gameplay_hashbeast_xp: player_data.gameplay_hashbeast_xp,
        round_id,
        war_id,
        num_bets: num_bets as u8,
        faction_ids: evt_faction_ids,
        directions: evt_directions,
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
        total_cycle_sol_split,
        timestamp: clock.unix_timestamp,
    });

    // DNA mutation rolls intentionally happen when winning rewards are claimed,
    // not when bets are placed. Bet placement only records prediction weight and
    // deterministic gameplay score for country movement.

    Ok(())
}

/// Calculate SOL and degenBTC rewards for a user bet.
/// Returns (total_sol_reward, total_dbtc_reward)
fn calculate_round_rewards(
    user_bet: &UserGameBet,
    game_session: &GameSession,
) -> Result<(u64, u64)> {
    require_user_game_bet_vectors_aligned(user_bet)?;
    let mut total_sol_reward = 0u64;
    let mut total_dbtc_reward = 0u64;

    for (idx, &faction_id) in user_bet.faction_ids.iter().enumerate() {
        let direction = user_bet
            .directions
            .get(idx)
            .copied()
            .ok_or(ErrorCode::InvalidState)?;
        let faction_index = faction_id as usize;
        let direction_index = direction as usize;
        require!(faction_index < NUM_FACTIONS, ErrorCode::InvalidFactionId);
        require!(
            direction_index < PredictionDirection::COUNT,
            ErrorCode::InvalidState
        );
        let points_bet_on_faction = user_bet
            .points_bets
            .get(idx)
            .copied()
            .ok_or(ErrorCode::InvalidState)?;
        let wgtd_points_bet_on_faction = user_bet
            .wgtd_points_bets
            .get(idx)
            .copied()
            .ok_or(ErrorCode::InvalidState)?;

        let is_winning_faction = faction_id == game_session.winning_faction_id;
        let is_winning_direction = direction == game_session.winning_direction;

        if is_winning_faction && is_winning_direction {
            // SOL rewards only go to the exact winning direction.
            if game_session.sol_rewards_index > 0 && points_bet_on_faction > 0 {
                let sol_reward = u64::try_from(helper::mul_div_u128(
                    points_bet_on_faction as u128,
                    game_session.sol_rewards_index,
                    INDEX_PRECISION as u128,
                )?)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?;
                total_sol_reward = total_sol_reward
                    .checked_add(sol_reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            }

            // Exact-direction degenBTC rewards.
            if game_session.dbtc_rewards_index > 0 && wgtd_points_bet_on_faction > 0 {
                let dbtc_reward = u64::try_from(helper::mul_div_u128(
                    wgtd_points_bet_on_faction as u128,
                    game_session.dbtc_rewards_index,
                    INDEX_PRECISION as u128,
                )?)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?;
                total_dbtc_reward = total_dbtc_reward
                    .checked_add(dbtc_reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            }
        } else if is_winning_faction {

            let same_faction_pool = game_session.dbtc_same_faction_direction_pools[direction_index];
            let same_faction_wgtd_points =
                game_session.wgtd_points_bets_by_faction_direction[faction_index][direction_index];

            if same_faction_pool > 0
                && same_faction_wgtd_points > 0
                && wgtd_points_bet_on_faction > 0
            {
                let dbtc_reward = u64::try_from(helper::mul_div(
                    wgtd_points_bet_on_faction,
                    same_faction_pool,
                    same_faction_wgtd_points,
                )?)
                .map_err(|_| ErrorCode::ArithmeticOverflow)?;
                total_dbtc_reward = total_dbtc_reward
                    .checked_add(dbtc_reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            }
        }

        // Jackpot rewards — ANY bet on the jackpot faction gets a share,
        // regardless of direction (no direction check).
        if game_session.jackpot_hit
            && game_session.jackpot_rewards_index > 0
            && faction_id == game_session.jackpot_faction_id
            && wgtd_points_bet_on_faction > 0
        {
            let jackpot_reward = u64::try_from(helper::mul_div_u128(
                wgtd_points_bet_on_faction as u128,
                game_session.jackpot_rewards_index,
                INDEX_PRECISION as u128,
            )?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            total_dbtc_reward = total_dbtc_reward
                .checked_add(jackpot_reward)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    Ok((total_sol_reward, total_dbtc_reward))
}

fn build_round_claim_mutation_roll(
    user_bet: &UserGameBet,
    game_session: &GameSession,
    _player_faction_id: u8,
) -> Result<Option<ClaimMutationRoll>> {
    require_user_game_bet_vectors_aligned(user_bet)?;
    let winning_faction = game_session.winning_faction_id;
    let winning_faction_index = winning_faction as usize;
    if winning_faction_index >= NUM_FACTIONS {
        return Ok(None);
    }

    let mut exact_sol = 0u64;
    let mut same_faction_sol = 0u64;
    for (idx, &faction_id) in user_bet.faction_ids.iter().enumerate() {
        if faction_id != winning_faction {
            continue;
        }
        let sol = user_bet
            .sol_bets
            .get(idx)
            .copied()
            .ok_or(ErrorCode::InvalidState)?;
        let direction = user_bet
            .directions
            .get(idx)
            .copied()
            .ok_or(ErrorCode::InvalidState)?;
        if direction == game_session.winning_direction {
            exact_sol = exact_sol
                .checked_add(sol)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        } else {
            same_faction_sol = same_faction_sol
                .checked_add(sol)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    let stake = exact_sol
        .checked_add(same_faction_sol / 4)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    if stake == 0 {
        return Ok(None);
    }

    let chance_boost_bps = if exact_sol > 0 { 12_000u64 } else { 5_000u64 };

    Ok(Some(ClaimMutationRoll {
        faction_id: winning_faction_index,
        stake,
        // Additive volume — the country's accumulated SOL bets since its
        // last win, snapshotted onto GameSession at round-end.
        faction_volume: game_session.winning_faction_volume_at_round,
        chance_boost_bps,
        entropy_slot: game_session
            .entropy_slot_used
            .max(game_session.round_start_slot),
        total_sol_bets: game_session.total_sol_bets,
        total_points_bets: game_session.total_points_bets,
        total_wgtd_points_bets: game_session.total_wgtd_points_bets,
        faction_mutation_count: game_session.mutations_per_faction[winning_faction_index],
        cycle_rounds_elapsed: 1,
        cycle_mutations_triggered: game_session.total_mutations_this_round as u16,
    }))
}

fn record_round_claim_mutation(
    game_session: &mut GameSession,
    faction_id: usize,
    mutation_type: u8,
) -> Result<()> {
    if mutation_type == 0 {
        return Ok(());
    }
    // Saturating add: these counters live on a per-round `GameSession` and feed
    // back into the mutation-roll probability formula. They are NOT funds and
    // NOT consensus state across users. With a `checked_add` here, the 256th
    // mutating winner in a single round would have their entire reward claim
    // revert — turning a soft accounting cap into a hard DOS on valid winners.
    // Saturating at 255 stabilizes the late-round mutation chance instead.
    // (Same reason `total_mutations_this_round` is saturated below.)
    game_session.mutations_per_faction[faction_id] =
        game_session.mutations_per_faction[faction_id].saturating_add(1);
    game_session.total_mutations_this_round =
        game_session.total_mutations_this_round.saturating_add(1);
    Ok(())
}

/// Update player rewards stats and add degenBTC to pending rewards
fn update_player_rewards(
    owner: Pubkey,
    player_data_key: Pubkey,
    player_data: &mut PlayerData,
    hodl_pool: &mut HodlPool,
    _total_sol_reward: u64,
    total_dbtc_reward: u64,
    round_id: u64,
) -> Result<()> {
    helper::add_to_total_claimable(
        hodl_pool,
        player_data,
        total_dbtc_reward,
        owner,
        player_data_key,
        CLAIMABLE_DBTC_SOURCE_ROUND,
        round_id,
    )?;
    msg!(
        "     Pending degenBTC rewards: {} (+{})",
        player_data.pending_dbtc_rewards as f64 / 1e6,
        total_dbtc_reward as f64 / 1e6
    );

    Ok(())
}

/// Apply the round-claim mutation bonus to the cycle's score accounting.
///
/// Bonus formula: `user_wgtd_points_on_winner × active_multiplier / BASE_MULTIPLIER × mutation_weight`.
///
/// # Home vs foreign win
///
/// - **Home win** (winner == player's home faction): full bonus credits
///     - `war_state.gameplay_scores[winner]` += bonus (country leaderboard)
///     - `war_state.faction_mutation_score[winner]` += bonus (HB-bonus pool denominator)
///     - `player_data.current_war_score` += bonus (MVP candidacy tracker)
///     - `war_state.mvp_user[winner]` updates if this user surpasses current MVP
///     - `user_war_bets.mutation_score` += bonus (HB-bonus pool numerator)
///
/// - **Foreign win** (player backed a country other than their own — "mercenary"):
///     - `war_state.gameplay_scores[winner]` += bonus / 2 (50% penalty)
///     - All other counters skipped. No HB-bonus credit, no MVP candidacy,
///       no `user.mutation_score` growth. The mercenary's contribution
///       moves the foreign country up the leaderboard but does not earn
///       that country's HashBeast-bonus or MVP pools.
///
/// The hashbeast itself (DNA / XP / multiplier) is updated upstream in
/// `sync_claim_hashbeast_state` regardless of home / foreign — that's the
/// player's personal NFT progression and runs independently of this function.
///
/// # Gating
///
/// - `mutation_type` must be 1/2/3 (Evolution / Power / Trait)
/// - `war_state_info` PDA matches the round's `war_id_when_played` (seed check)
/// - `user_war_bets_info` PDA matches owner + war_id (seed check)
/// - `war_state.stage == 0` (cycle still active — late claims after settle drop silently)
/// - `winner_idx < faction_count`
/// - User had non-zero weighted points on the winning faction
///
/// Silent skip (Ok) on most gate failures (cycle settled, no winning bet,
/// zero bonus). Returns Err only on seed mismatches.
#[inline(never)]
fn apply_mutation_bonus_score(
    mutation_type: u8,
    user_bet: &UserGameBet,
    game_session: &GameSession,
    war_state: &mut FactionWarState,
    user_war_bets: &mut UserFactionWarBets,
    player_data: &mut PlayerData,
    owner_key: Pubkey,
) -> Result<()> {
    if mutation_type == 0 {
        return Ok(());
    }
    let weight = mutation_bonus_weight(mutation_type);
    if weight == 0 {
        return Ok(());
    }

    let cycle_id = game_session.war_id_when_played;

    // Late-claim gate: cycle has settled → drop bonus.
    if war_state.stage != 0 {
        msg!(
            "🎁 [apply_mutation_bonus_score] cycle {} already settled (stage={}); dropping bonus",
            cycle_id,
            war_state.stage
        );
        return Ok(());
    }

    let winner = game_session.winning_faction_id;
    let winner_idx = winner as usize;
    if winner_idx >= war_state.faction_count as usize {
        return Ok(());
    }

    let user_wgtd = total_wgtd_points_for_faction(user_bet, winner)?;
    if user_wgtd == 0 {
        return Ok(());
    }

    let bonus_u128 = (user_wgtd as u128)
        .checked_mul(player_data.active_multiplier as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASE_MULTIPLIER as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_mul(weight as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let bonus = u64::try_from(bonus_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    if bonus == 0 {
        return Ok(());
    }

    // Home vs foreign split:
    // - Home win (winner == player's home faction): full credit on every
    //   counter — country leaderboard, MVP candidacy, HB-bonus pool, and the
    //   user's HB-bonus numerator.
    // - Foreign win ("mercenary" mutation by a player backing a country other
    //   than their own): the country leaderboard still moves but with a 50%
    //   penalty; the user gets NO HB-bonus / MVP / mutation_score credit. The
    //   NFT-level mutation (DNA / XP / multiplier) already fired upstream in
    //   `sync_claim_hashbeast_state` regardless — this function only governs
    //   the cycle score accounting.
    let is_home_win = winner_idx == player_data.faction_id as usize;
    let leaderboard_score_add = leaderboard_score_for_mutation(bonus, is_home_win);
    if leaderboard_score_add == 0 {
        // Foreign + bonus=1 → halved to 0; nothing observable changes here.
        return Ok(());
    }

    // Country leaderboard always moves (full for home, halved for foreign).
    war_state.gameplay_scores[winner_idx] = war_state.gameplay_scores[winner_idx]
        .checked_add(leaderboard_score_add)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if is_home_win {
        // Country-level mutation pot — denominator for HB-bonus claim share.
        // Only counts home contributions so the ratio
        // `user.mutation_score / faction_mutation_score[home]` stays ≤ 1.
        war_state.faction_mutation_score[winner_idx] = war_state.faction_mutation_score[winner_idx]
            .checked_add(bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        if player_data.current_war_score_cycle_id != cycle_id {
            player_data.current_war_score = 0;
            player_data.current_war_score_cycle_id = cycle_id;
        }
        player_data.current_war_score = player_data
            .current_war_score
            .checked_add(bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        if player_data.current_war_score > war_state.mvp_score[winner_idx] {
            war_state.mvp_user[winner_idx] = owner_key;
            war_state.mvp_score[winner_idx] = player_data.current_war_score;
        }
    }

    let total_after = war_state.gameplay_scores[winner_idx];

    if is_home_win {
        // Per-user mutation pot — numerator for HB-bonus claim share.
        // Lives on the per-cycle UserFactionWarBets PDA (closed at war claim)
        // so it survives the claim-vs-next-cycle race that `player_data` can't.
        require!(user_war_bets.owner == owner_key, ErrorCode::InvalidAccount);
        require!(user_war_bets.war_id == cycle_id, ErrorCode::InvalidAccount);
        user_war_bets.mutation_score = user_war_bets
            .mutation_score
            .checked_add(bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    } else {
        msg!(
            "🎁 [apply_mutation_bonus_score] foreign win (winner={} home={}): +{} to leaderboard only, no HB/MVP credit",
            winner_idx,
            player_data.faction_id,
            leaderboard_score_add
        );
    }

    emit!(crate::events::GameplayScoreAccumulated {
        war_id: cycle_id,
        faction_id: winner,
        score_source: GAMEPLAY_SCORE_SOURCE_MUTATION_BONUS,
        // Reflect what actually moved on the leaderboard (halved for foreign
        // mercenary mutations). Indexers attribute country leaderboard moves
        // to this value.
        score_added: leaderboard_score_add,
        faction_total_score: total_after,
    });

    msg!(
        "🎁 [apply_mutation_bonus_score] cycle={} winner={} home_win={} bonus={} leaderboard_added={} (mut_type={}, user_wgtd={}, mult={})",
        cycle_id,
        winner,
        is_home_win,
        bonus,
        leaderboard_score_add,
        mutation_type,
        user_wgtd,
        player_data.active_multiplier
    );

    Ok(())
}

/// Run the loser-roll lottery inline during a round-claim. Called from
/// `claim_round_rewards` and `claim_autominer_rewards` AFTER `claim_won` is
/// computed.
///
/// Eligibility (any failure → silently skip, no error):
/// - `claim_won == false` (loser-only)
/// - `lootbox_claim` PDA is empty (no prior pending claim)
/// - `lootbox_queue.filled_count > 0` (something to win)
/// - user had `wgtd_points` bet on their home faction this round (skin-in-the-game,
///   blocks pure foreign-country bettors from rolling)
///
/// On a winning roll: pop a random slot from the queue (random pick + shift-left
/// to keep slots packed), populate the `LootboxClaim` PDA fields with the
/// reserved asset. Asset stays on `inventory_pda`; user or cranker calls
/// `claim_lootbox_nft` separately to actually deliver it to the recorded user.
#[inline(never)]
pub fn maybe_run_loser_lootbox_roll<'info>(
    claim_won: bool,
    user_bet: &UserGameBet,
    game_session: &GameSession,
    player_data: &PlayerData,
    lootbox_queue: &mut LootboxQueue,
    lootbox_claim_info: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    lootbox_claim_bump: u8,
    user_key: Pubkey,
    round_id: u64,
) -> Result<()> {
    if claim_won {
        return Ok(());
    }
    let existing_claim = if lootbox_claim_info.lamports() > 0 && !lootbox_claim_info.data_is_empty()
    {
        require_keys_eq!(
            *lootbox_claim_info.owner,
            crate::ID,
            ErrorCode::InvalidAccount
        );
        let data = lootbox_claim_info.try_borrow_data()?;
        let claim = LootboxClaim::try_deserialize(&mut &data[..])?;
        require!(
            claim.user == Pubkey::default() || claim.user == user_key,
            ErrorCode::InvalidOwner
        );
        Some(claim)
    } else {
        require!(
            *lootbox_claim_info.owner == anchor_lang::system_program::ID
                && lootbox_claim_info.data_is_empty(),
            ErrorCode::InvalidAccount
        );
        None
    };

    if existing_claim
        .as_ref()
        .is_some_and(|claim| claim.asset != Pubkey::default())
    {
        msg!("🎟  [loser_roll] skipped: user already has a pending claim");
        return Ok(());
    }
    if lootbox_queue.filled_count == 0 {
        msg!(
            "🎟  [loser_roll] skipped: faction {} queue empty",
            player_data.faction_id
        );
        return Ok(());
    }
    let user_wgtd_on_home = total_wgtd_points_for_faction(user_bet, player_data.faction_id)?;
    if user_wgtd_on_home == 0 {
        msg!("🎟  [loser_roll] skipped: no wgtd_points bet on home faction");
        return Ok(());
    }

    // Compute chance from queue depth.
    let threshold_bps = compute_loser_drop_chance_bps(lootbox_queue.filled_count);
    if threshold_bps == 0 {
        return Ok(());
    }

    // Entropy: slot-hash-derived game_session entropy + round id + user
    // pubkey + queue state. We intentionally don't pull the SlotHashes
    // sysvar here (not yet in the claim Accounts struct); game_session
    // already contains the entropy_hash sampled at round end.
    let queue_state_seed = lootbox_queue.slots[0].to_bytes();
    let entropy = anchor_lang::solana_program::keccak::hashv(&[
        b"minebtc-loser-roll",
        &game_session.entropy_hash,
        &round_id.to_le_bytes(),
        &user_key.to_bytes(),
        &queue_state_seed,
        &[lootbox_queue.filled_count],
    ])
    .to_bytes();

    let roll_value = u16::from_le_bytes([entropy[0], entropy[1]]) % 10_000;
    let queue_depth_before = lootbox_queue.filled_count;

    if roll_value >= threshold_bps {
        emit!(LootboxRollMissed {
            user: user_key,
            faction_id: player_data.faction_id,
            queue_depth: queue_depth_before,
            roll_value,
            threshold_bps,
            timestamp: Clock::get()?.unix_timestamp,
        });
        msg!(
            "🎟  [loser_roll] miss: user={} roll={} threshold={} depth={}",
            user_key,
            roll_value,
            threshold_bps,
            queue_depth_before
        );
        return Ok(());
    }

    // WIN — pick a random slot index, pop it, shift-left to repack.
    let pick_idx_u32 = u32::from_le_bytes([entropy[2], entropy[3], entropy[4], entropy[5]]);
    let pick_idx = (pick_idx_u32 as usize) % (queue_depth_before as usize);
    let won_asset = lootbox_queue.slots[pick_idx];

    let last_idx = (queue_depth_before as usize) - 1;
    if pick_idx < last_idx {
        for i in pick_idx..last_idx {
            lootbox_queue.slots[i] = lootbox_queue.slots[i + 1];
        }
    }
    lootbox_queue.slots[last_idx] = Pubkey::default();
    lootbox_queue.filled_count = lootbox_queue
        .filled_count
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Populate the reservation. The PDA is intentionally created only after a
    // winning roll so ordinary reward claims do not pay rent for empty
    // lootbox reservations.
    let now = Clock::get()?.unix_timestamp;
    let claim = LootboxClaim {
        bump: lootbox_claim_bump,
        user: user_key,
        asset: won_asset,
        faction_id: player_data.faction_id,
    };
    if existing_claim.is_some() {
        helper::store_account_data(lootbox_claim_info, &claim)?;
    } else {
        let user_bytes = user_key.to_bytes();
        let seeds: &[&[u8]] = &[
            LOOTBOX_CLAIM_SEED,
            user_bytes.as_ref(),
            core::slice::from_ref(&lootbox_claim_bump),
        ];
        let created = helper::init_pda_account_if_needed::<LootboxClaim>(
            payer,
            lootbox_claim_info,
            system_program,
            seeds,
            LootboxClaim::LEN,
            &claim,
        )?;
        require!(created, ErrorCode::InvalidState);
    }

    emit!(LootboxRollWon {
        user: user_key,
        faction_id: player_data.faction_id,
        asset: won_asset,
        queue_depth_before,
        roll_value,
        threshold_bps,
        timestamp: now,
    });

    msg!(
        "🎉 [loser_roll] WIN: user={} asset={} threshold={}",
        user_key,
        won_asset,
        threshold_bps
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn roll_claim_mutation(
    origin: u8,
    origin_id: u64,
    expected_hashbeast: Pubkey,
    owner_key: Pubkey,
    player_data: &mut PlayerData,
    tuning: &GameplayTuningConfig,
    roll: ClaimMutationRoll,
) -> Result<u8> {
    if !tuning.rpg_progression
        || roll.stake == 0
        || expected_hashbeast == Pubkey::default()
        || expected_hashbeast != player_data.gameplay_hashbeast
        || player_data.gameplay_hashbeast == Pubkey::default()
    {
        return Ok(0);
    }

    let hashbeast_mint = player_data.gameplay_hashbeast;
    let mutation_result = calculate_mutation_result(
        origin,
        origin_id,
        roll.stake,
        player_data.active_multiplier,
        player_data.gameplay_hashbeast_dna,
        player_data.gameplay_hashbeast_xp,
        tuning.max_evolution_stage_unlocked,
        roll.faction_mutation_count,
        roll.faction_volume.max(roll.stake),
        tuning,
        roll.chance_boost_bps,
        roll.cycle_rounds_elapsed,
        roll.cycle_mutations_triggered,
        roll.total_sol_bets,
        roll.total_points_bets,
        roll.total_wgtd_points_bets,
        roll.entropy_slot,
        &owner_key,
        &hashbeast_mint,
    );

    player_data.gameplay_hashbeast_xp = player_data
        .gameplay_hashbeast_xp
        .checked_add(mutation_result.xp_gained)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if let Some(mutation_type) = mutation_result.mutation_type {
        let new_mult = player_data
            .active_multiplier
            .checked_add(mutation_result.multiplier_increase)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        player_data.active_multiplier = new_mult.min(GAMEPLAY_MAX_MULTIPLIER as u32);
        player_data.gameplay_hashbeast_xp = player_data
            .gameplay_hashbeast_xp
            .saturating_sub(mutation_result.xp_consumed);
        player_data.gameplay_hashbeast_dna = mutation_result.new_dna;

        let mutation_type_u8 = mutation_type_to_u8(mutation_type);
        emit!(StoryEventTriggered {
            origin,
            origin_id,
            user: owner_key,
            hashbeast_mint,
            story_event_type: mutation_type_u8,
            xp_gained: mutation_result.xp_gained,
            multiplier_after: player_data.active_multiplier,
        });
        msg!(
            "🧬 Claim mutation fired: origin={} id={} faction={} type={} mult={} xp={}",
            origin,
            origin_id,
            roll.faction_id,
            mutation_type_u8,
            player_data.active_multiplier,
            player_data.gameplay_hashbeast_xp
        );
        Ok(mutation_type_u8)
    } else {
        msg!(
            "🧬 Claim mutation roll missed: origin={} id={} faction={} xp={}",
            origin,
            origin_id,
            roll.faction_id,
            player_data.gameplay_hashbeast_xp
        );
        Ok(0)
    }
}

fn sync_claim_hashbeast_state<'info>(
    expected_hashbeast: Pubkey,
    player_data: &PlayerData,
    hashbeast_metadata: Option<&mut Box<Account<'info, HashBeastMetadata>>>,
    accumulated_add: u64,
    accum_pct: u32,
    claim_won: bool,
) -> Result<()> {
    if !claim_won
        || expected_hashbeast == Pubkey::default()
        || expected_hashbeast != player_data.gameplay_hashbeast
        || player_data.gameplay_hashbeast == Pubkey::default()
    {
        return Ok(());
    }

    let hashbeast_metadata = hashbeast_metadata.ok_or(ErrorCode::HashBeastMetadataNotFound)?;
    require_keys_eq!(
        hashbeast_metadata.mint,
        expected_hashbeast,
        ErrorCode::HashBeastMetadataNotFound
    );
    let (expected_metadata_pda, _) = Pubkey::find_program_address(
        &[
            HASHBEAST_METADATA_SEED.as_ref(),
            expected_hashbeast.as_ref(),
        ],
        &crate::ID,
    );
    require_keys_eq!(
        hashbeast_metadata.key(),
        expected_metadata_pda,
        ErrorCode::InvalidAccount
    );
    if accumulated_add > 0 {
        hashbeast_metadata.accumulated_val = hashbeast_metadata
            .accumulated_val
            .checked_add(accumulated_add)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    hashbeast_metadata.dna = player_data.gameplay_hashbeast_dna;
    hashbeast_metadata.xp = player_data.gameplay_hashbeast_xp;
    hashbeast_metadata.multiplier = player_data.active_multiplier;

    emit!(HashBeastSynced {
        hashbeast_mint: hashbeast_metadata.mint,
        hashbeast_metadata_account: hashbeast_metadata.key(),
        dna: hashbeast_metadata.dna.to_vec(),
        xp: hashbeast_metadata.xp,
        multiplier: hashbeast_metadata.multiplier,
        accumulated_val: hashbeast_metadata.accumulated_val,
        accum_pct,
    });

    msg!(
        "🧬 Synced claim hashbeast: {} accumulated_add={} xp={} mult={}",
        hashbeast_metadata.mint,
        accumulated_add,
        hashbeast_metadata.xp,
        hashbeast_metadata.multiplier
    );
    Ok(())
}

/// Join a round by betting SOL or using free tickets (single prediction).
/// Each bet selects a faction and a faction_war direction.
///
/// Parameters:
/// - bet_types: Vector of bet types (`FactionDirection { faction_id, direction }`)
/// - amount_per_bet: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
// The cycle's FactionWarState PDA is created exclusively by
// `initialize_war_internal` (cranker). The cranker must run that ix before
// any rounds (and therefore any bets) can land in the cycle — start_round
// stays blocked until init_war clears `war_config.cycle_end_round_id`.

fn handle_fee(amount: u64, protocol_fee_pct: u64) -> Result<(u64, u64)> {
    let fee = amount
        .checked_mul(protocol_fee_pct)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        / M_HUNDRED;
    let net_amount = amount
        .checked_sub(fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "     Net amount (after fee): {} SOL. Protocol fee ({}%): {} SOL",
        (net_amount as f64) / 1_000_000_000.0,
        protocol_fee_pct,
        (fee as f64) / 1_000_000_000.0
    );
    Ok((net_amount, fee))
}

fn validate_points_percentage_limit(
    current_points_bets: u64,
    current_sol_bets: u64,
    amount: u64,
) -> Result<()> {
    // total_points_bets includes SOL-backed points plus ticket-backed points.
    // Enforce the 25% cap only on the ticket-backed portion.
    let current_ticket_points = current_points_bets
        .checked_sub(current_sol_bets)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let new_ticket_points = current_ticket_points
        .checked_add(amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "     Current session stats: SOL bets: {} lamports, Total points: {} lamports, Ticket-backed points: {} lamports, New ticket-backed points if allowed: {} lamports",
        current_sol_bets,
        current_points_bets,
        current_ticket_points,
        new_ticket_points
    );

    require!(current_sol_bets > 0, ErrorCode::InvalidParameters);
    msg!("     ✓ SOL bets exist in session");

    let max_allowed_points = u64::try_from(helper::mul_div(current_sol_bets, 25, M_HUNDRED)?)
        .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!(
        "       Max allowed ticket-backed points (25% of SOL): {} lamports",
        max_allowed_points
    );
    require!(
        new_ticket_points <= max_allowed_points,
        ErrorCode::TicketBetCapExceeded
    );
    msg!("     ✓ Points bets stay within 25% limit");
    Ok(())
}

/// Generate faction-direction bet types from the autominer configuration.
/// Returns vector of bet types to place
fn make_bets_vec(
    factions_config: Option<FactionsConfig>,
    clock: &Clock,
    global_config: &GlobalConfig,
) -> Result<Vec<BetType>> {
    let mut bet_types = Vec::new();

    if let Some(ref factions_cfg) = factions_config {
        match factions_cfg {
            FactionsConfig::Specific { picks } => {
                msg!("     Specific autominer picks: {:?}", picks);
                for pick in picks.iter() {
                    require!(
                        (pick.faction_id as usize) < global_config.supported_factions.len(),
                        ErrorCode::InvalidFactionId
                    );
                    bet_types.push(BetType::FactionDirection {
                        faction_id: pick.faction_id,
                        direction: pick.direction,
                    });
                }
            }
            FactionsConfig::Random { count, direction } => {
                let mut random_factions = Vec::new();
                let mut used_factions = [false; NUM_FACTIONS];
                let mut attempts: u64 = 0;
                let max_factions = global_config.supported_factions.len();
                require!(
                    max_factions > 0 && max_factions <= NUM_FACTIONS,
                    ErrorCode::InvalidFactionId
                );
                require!(
                    (*count as usize) <= max_factions,
                    ErrorCode::InvalidParameters
                );
                while random_factions.len() < *count as usize && attempts < 100 {
                    let slot_bytes = clock.slot.to_le_bytes();
                    let hash =
                        keccak::hash(&[slot_bytes, (attempts + 100u64).to_le_bytes()].concat());
                    let faction_id = hash.0[0] % max_factions as u8;
                    let faction_index = faction_id as usize;
                    if faction_index < max_factions && !used_factions[faction_index] {
                        random_factions.push(faction_id);
                        used_factions[faction_index] = true;
                    }
                    attempts += 1;
                }
                require!(
                    random_factions.len() == *count as usize,
                    ErrorCode::InvalidParameters
                );
                msg!(
                    "     Random autominer factions: {:?} (direction: {:?})",
                    random_factions,
                    direction
                );
                for faction_id in random_factions {
                    bet_types.push(BetType::FactionDirection {
                        faction_id,
                        direction: *direction,
                    });
                }
            }
        }
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(bet_types)
}

/// Calculate caller compensation: 0.1% of sol_per_round, max 0.00005 SOL.
fn get_caller_compensation(sol_per_round: u64) -> Result<u64> {
    let caller_compensation = (sol_per_round / 1_000).min(crate::state::MAX_CALLER_COMPENSATION);
    Ok(caller_compensation)
}

/// Ticket autominers use a fixed keeper gas reserve per round.
fn get_ticket_caller_compensation() -> u64 {
    crate::state::TICKET_AUTOMINER_CALLER_COMPENSATION
}

fn validate_min_sol_bet_per_position(amount: u64) -> Result<()> {
    require!(
        amount >= MIN_SOL_BET_PER_POSITION,
        ErrorCode::BetBelowMinimum
    );
    Ok(())
}

fn count_autominer_bets_per_round(factions_config: &Option<FactionsConfig>) -> Result<u64> {
    match factions_config {
        Some(FactionsConfig::Specific { picks }) => {
            require!(!picks.is_empty(), ErrorCode::InvalidParameters);
            Ok(picks.len() as u64)
        }
        Some(FactionsConfig::Random { count, .. }) => {
            require!(*count > 0, ErrorCode::InvalidParameters);
            Ok(*count as u64)
        }
        None => err!(ErrorCode::InvalidParameters),
    }
}

fn prediction_bet_parts(bet_type: &BetType) -> Result<(u8, PredictionDirection)> {
    match bet_type {
        BetType::FactionDirection {
            faction_id,
            direction,
        } => Ok((*faction_id, *direction)),
    }
}

fn mutation_type_to_u8(mutation_type: MutationType) -> u8 {
    match mutation_type {
        MutationType::Evolution => 1,
        MutationType::Power => 2,
        MutationType::Trait => 3,
    }
}

fn mutation_accum_pct(mutation_type: u8) -> u64 {
    match mutation_type {
        1 => 69, // Evolution: 6.9%
        2 => 42, // Power: 4.2%
        3 => 30, // Trait: 3%
        _ => 10, // No mutation: 1%
    }
}

/// Returns the user's total weighted points bet on a single faction across
/// all directions. This is the metric used for both round-end score
/// accumulation (round-wide) and mutation-bonus base (per-user).
fn total_wgtd_points_for_faction(user_bet: &UserGameBet, faction_id: u8) -> Result<u64> {
    require_user_game_bet_vectors_aligned(user_bet)?;
    let mut total = 0u64;
    for (idx, &fid) in user_bet.faction_ids.iter().enumerate() {
        if fid == faction_id {
            let amount = user_bet
                .wgtd_points_bets
                .get(idx)
                .copied()
                .ok_or(ErrorCode::InvalidState)?;
            total = total
                .checked_add(amount)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }
    Ok(total)
}

fn require_user_game_bet_vectors_aligned(user_bet: &UserGameBet) -> Result<()> {
    let len = user_bet.faction_ids.len();
    require!(
        user_bet.directions.len() == len
            && user_bet.sol_bets.len() == len
            && user_bet.points_bets.len() == len
            && user_bet.wgtd_points_bets.len() == len,
        ErrorCode::InvalidState
    );
    require!(
        len <= UserGameBet::MAX_POSITIONS_PER_BET,
        ErrorCode::InvalidParameters
    );
    Ok(())
}

fn mutation_bonus_weight(mutation_type: u8) -> u64 {
    match mutation_type {
        1 => MUTATION_BONUS_WEIGHT_EVOLUTION,
        2 => MUTATION_BONUS_WEIGHT_POWER,
        3 => MUTATION_BONUS_WEIGHT_TRAIT,
        _ => 0,
    }
}

/// Leaderboard score-add for a successful mutation roll.
///
/// - **Home win** (winner == player's home faction): full `bonus`.
/// - **Foreign win** (player backed a country other than their own): **50%**
///   of `bonus` — the "mercenary penalty". Foreign mutations still move the
///   country leaderboard, but only at half the rate of a citizen's contribution.
///
/// The NFT-level mutation effects (DNA / XP / multiplier) are applied upstream
/// regardless of home / foreign. This function only scales the cycle's
/// `gameplay_scores` impact. HB-bonus / MVP / `user.mutation_score` accounting
/// is gated separately on `is_home_win` inside `apply_mutation_bonus_score`.
#[inline]
fn leaderboard_score_for_mutation(bonus: u64, is_home_win: bool) -> u64 {
    if is_home_win {
        bonus
    } else {
        bonus / 2
    }
}

fn player_has_pending_reward_claims(player_data: &PlayerData) -> bool {
    player_data.pending_round_claims > 0 || player_data.pending_war_claims > 0
}

fn load_global_config(account: &AccountInfo<'_>) -> Result<GlobalConfig> {
    load_program_account(account)
}

fn load_program_account<T: AccountDeserialize>(account: &AccountInfo<'_>) -> Result<T> {
    require!(account.owner == &crate::ID, ErrorCode::InvalidAccount);
    let data = account.try_borrow_data()?;
    let mut data_slice: &[u8] = &data;
    T::try_deserialize(&mut data_slice)
}

/// Helper struct for passing autominer info to internal_process_bets
pub struct AutominerBetInfo {
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub compensation: u64,
    pub rounds_remaining: u32,
}

struct ClaimMutationRoll {
    faction_id: usize,
    stake: u64,
    /// Country's accumulated SOL volume since its last win — fed straight
    /// into the volume_factor in `calculate_mutation_result`. Sourced from
    /// `GameSession.winning_faction_volume_at_round` for round-claim, and
    /// from `war_state.total_cycle_sol` for cycle-claim.
    faction_volume: u64,
    chance_boost_bps: u64,
    entropy_slot: u64,
    total_sol_bets: u64,
    total_points_bets: u64,
    total_wgtd_points_bets: u64,
    faction_mutation_count: u8,
    cycle_rounds_elapsed: u16,
    cycle_mutations_triggered: u16,
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

    /// Optional only when no referral code is supplied.
    /// If a referral code is provided, this account must be the canonical referrer's
    /// ReferralRewards PDA and is validated in the instruction handler.
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
#[instruction(factions_config: Option<FactionsConfig>, sol_per_round: u64, num_rounds: u32)]
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
        bump,
        constraint = player_data.owner == user_wallet.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(mut)]
    pub user_wallet: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64, war_id: u64)]
pub struct JoinBets<'info> {
    /// CHECK: Program-owned PDA deserialized and validated in handler to keep parser stack small.
    /// No seeds/bump in derive macro to keep `JoinBets` stack under 4KB.
    #[account(mut)]
    pub global_config: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == authority.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    /// GameSession PDA for the current round (must be initialized by crank function)
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// UserGameBet PDA for this user's bet in this round
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &round_id.to_le_bytes()],
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
        seeds = [JACKPOT_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// CHECK: Faction-war SOL vault (cycle jackpot reserve).
    #[account(
        mut,
        seeds = [FACTION_WAR_SOL_VAULT_SEED.as_ref()],
        bump,
    )]
    pub war_sol_vault: UncheckedAccount<'info>,

    /// Per-user, per-cycle bets PDA. Created lazily on the user's first bet
    /// of the cycle (init_if_needed), then read+mutated on subsequent bets and
    /// at round claim time.
    #[account(
        init_if_needed,
        payer = authority,
        space = UserFactionWarBets::LEN,
        seeds = [USER_FACTION_WAR_BETS_SEED, authority.key().as_ref(), &war_id.to_le_bytes()],
        bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    /// Referrer's commission account. Required when the betting player has a referrer
    /// (`player_data.referral_code != system_program`); the SDK derives the PDA via
    /// `[REFERRAL_REWARDS_SEED, player_data.referral_code]` and passes it here.
    /// Optional only for unreferred players.
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), player_data.referral_code.as_ref()],
        bump,
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(current_round_id: u64, war_id: u64)]
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

    /// CHECK: Program-owned PDA deserialized and validated in handler to keep parser stack small
    #[account(seeds = [GLOBAL_GAME_STATE_SEED.as_ref()], bump)]
    pub global_game_state: UncheckedAccount<'info>,

    /// CHECK: PDA + owner/type validated in handler to keep account parser stack small
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump,
        constraint = player_data.owner == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &current_round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    /// UserGameBet PDA for autominer bets (aggregates all bets from this vault for this round)
    #[account(
        init_if_needed,
        payer = caller,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &current_round_id.to_le_bytes()],
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
        seeds = [JACKPOT_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// CHECK: Faction-war SOL vault (cycle jackpot reserve).
    #[account(
        mut,
        seeds = [FACTION_WAR_SOL_VAULT_SEED.as_ref()],
        bump,
    )]
    pub war_sol_vault: UncheckedAccount<'info>,

    /// Per-user, per-cycle bets PDA for the autominer's owner. Created lazily
    /// on the owner's first autominer-driven bet of the cycle (init_if_needed).
    /// Cranker (`caller`) pays rent.
    #[account(
        init_if_needed,
        payer = caller,
        space = UserFactionWarBets::LEN,
        seeds = [USER_FACTION_WAR_BETS_SEED, autominer_vault.owner.as_ref(), &war_id.to_le_bytes()],
        bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    /// Referrer's commission account. Required when the autominer's owner has a referrer.
    /// SDK derives `[REFERRAL_REWARDS_SEED, player_data.referral_code]`.
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), player_data.referral_code.as_ref()],
        bump,
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    /// Caller (bot or anyone) - doesn't need to be owner
    #[account(mut)]
    pub caller: Signer<'info>,

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
        bump,
        constraint = player_data.owner == autominer_vault.owner @ ErrorCode::Unauthorized
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
        bump,
        constraint = player_data.owner == autominer_vault.owner @ ErrorCode::Unauthorized
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
#[instruction(round_id: u64)]
pub struct ClaimRoundRewards<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user_wallet.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(mut, seeds = [HODL_POOL_SEED.as_ref()], bump)]
    pub hodl_pool: Box<Account<'info, HodlPool>>,

    #[account(mut, seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()], bump = game_session.bump)]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// Global game state (for current round entropy)
    #[account(seeds = [GLOBAL_GAME_STATE_SEED.as_ref()], bump = global_game_state.bump)]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [JACKPOT_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        close = caller,
        seeds = [USER_GAME_BET_SEED.as_ref(), user_wallet.key().as_ref(), &round_id.to_le_bytes()],
        bump = user_game_bet.bump,
        constraint = user_game_bet.owner == user_wallet.key() @ ErrorCode::InvalidOwner
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: User whose bet this is
    #[account(mut)]
    pub user_wallet: UncheckedAccount<'info>,

    /// Caller (bot or user themselves)
    #[account(mut)]
    pub caller: Signer<'info>,

    /// Optional HashBeastMetadata account for syncing mutation
    #[account(mut)]
    pub hashbeast_metadata: Option<Box<Account<'info, HashBeastMetadata>>>,

    /// Cycle state for the round being claimed. Address is pinned via seeds
    /// keyed by `game_session.war_id_when_played`.
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &game_session.war_id_when_played.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    /// Per-user, per-cycle bets PDA. Address pinned via seeds keyed by the
    /// claiming user + the cycle id stored on `game_session`. Lazily created
    /// by `join_bets` (the first bet of the cycle); claim ixs read+mutate the
    /// already-existing account.
    #[account(
        mut,
        seeds = [
            USER_FACTION_WAR_BETS_SEED,
            user_wallet.key().as_ref(),
            &game_session.war_id_when_played.to_le_bytes(),
        ],
        bump = user_war_bets.bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    /// Country lootbox queue for the player's home faction. Read on every
    /// claim; mutated when a losing player's roll wins a slot.
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[player_data.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// CHECK: Per-user reservation PDA. Address is pinned by seeds, and the
    /// handler either verifies the existing program-owned `LootboxClaim` data or
    /// lazily creates it only after a winning loser-roll lands.
    #[account(
        mut,
        seeds = [LOOTBOX_CLAIM_SEED, user_wallet.key().as_ref()],
        bump,
    )]
    pub lootbox_claim: UncheckedAccount<'info>,

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
    pub autominer_vault: Box<Account<'info, AutominerVault>>,

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
        bump = player_data.bump,
        constraint = player_data.owner == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub player_data: Box<Account<'info, PlayerData>>,

    #[account(mut, seeds = [HODL_POOL_SEED.as_ref()], bump)]
    pub hodl_pool: Box<Account<'info, HodlPool>>,

    #[account(mut, seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()], bump = game_session.bump)]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(seeds = [GLOBAL_CONFIG_SEED.as_ref()], bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [JACKPOT_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,

    /// User game bet account - will be closed
    #[account(
        mut,
        close = caller,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &round_id.to_le_bytes()],
        bump = user_game_bet.bump,
        constraint = user_game_bet.owner == autominer_vault.owner @ ErrorCode::InvalidOwner
    )]
    pub user_game_bet: Box<Account<'info, UserGameBet>>,

    /// CHECK: Owner wallet to receive leftover SOL / rent
    #[account(mut, constraint = owner_wallet.key() == autominer_vault.owner @ ErrorCode::Unauthorized)]
    pub owner_wallet: UncheckedAccount<'info>,

    /// Caller (backend script)
    #[account(mut)]
    pub caller: Signer<'info>,

    /// Optional HashBeastMetadata account for syncing mutation
    #[account(mut)]
    pub hashbeast_metadata: Option<Box<Account<'info, HashBeastMetadata>>>,

    /// Cycle state for the round being claimed. Address pinned via seeds keyed
    /// by `game_session.war_id_when_played`.
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &game_session.war_id_when_played.to_le_bytes()],
        bump = war_state.bump,
    )]
    pub war_state: Box<Account<'info, FactionWarState>>,

    /// Per-user, per-cycle bets PDA for the autominer's owner. Seeds pinned
    /// to `autominer_vault.owner` + the cycle id on `game_session`.
    #[account(
        mut,
        seeds = [
            USER_FACTION_WAR_BETS_SEED,
            autominer_vault.owner.as_ref(),
            &game_session.war_id_when_played.to_le_bytes(),
        ],
        bump = user_war_bets.bump,
    )]
    pub user_war_bets: Box<Account<'info, UserFactionWarBets>>,

    /// Country lootbox queue for the autominer owner's home faction.
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[player_data.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// CHECK: Per-user reservation PDA. Address is pinned by seeds, and the
    /// handler either verifies the existing program-owned `LootboxClaim` data or
    /// lazily creates it only after a winning loser-roll lands.
    #[account(
        mut,
        seeds = [LOOTBOX_CLAIM_SEED, owner_wallet.key().as_ref()],
        bump,
    )]
    pub lootbox_claim: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UseHashBeastForGameplay<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        constraint = faction_state.faction_id == player_data.faction_id @ ErrorCode::InvalidFactionId
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Read-only — anchors the `hashbeast_collection` address constraint to
    /// the canonical collection set by admin.
    #[account(seeds = [HASHBEAST_CONFIG_SEED.as_ref()], bump = hashbeast_config.bump)]
    pub hashbeast_config: Box<Account<'info, HashBeastConfig>>,

    /// Metaplex Core asset
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Collection account for the HashBeast — address-pinned to the official
    /// Core collection. Even with the metadata-PDA invariant guarding all
    /// HashBeasts in circulation, binding here closes the foot-gun of MPL Core
    /// transfer CPIs accepting a wrong-collection account.
    /// CHECK: Address-constrained.
    #[account(
        mut,
        address = hashbeast_config.hashbeast_collection @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_metadata.mint.as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    /// CHECK: PDA for NFT custody
    #[account(seeds = [HASHBEAST_CUSTODY_SEED], bump)]
    pub hashbeast_custody_pda: UncheckedAccount<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = war_config.bump)]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RequestHashBeastGameplayUnlock<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = war_config.bump)]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawHashBeastFromGameplay<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        constraint = faction_state.faction_id == player_data.faction_id @ ErrorCode::InvalidFactionId
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Read-only — anchors the `hashbeast_collection` address constraint to
    /// the canonical collection set by admin.
    #[account(seeds = [HASHBEAST_CONFIG_SEED.as_ref()], bump = hashbeast_config.bump)]
    pub hashbeast_config: Box<Account<'info, HashBeastConfig>>,

    /// Metaplex Core asset (in custody)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// Collection account for the HashBeast — address-pinned to the official
    /// Core collection.
    /// CHECK: Address-constrained.
    #[account(
        mut,
        address = hashbeast_config.hashbeast_collection @ ErrorCode::InvalidAccount,
    )]
    pub hashbeast_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [HASHBEAST_METADATA_SEED.as_ref(), hashbeast_metadata.mint.as_ref()],
        bump = hashbeast_metadata.bump,
        constraint = hashbeast_metadata.mint == hashbeast_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub hashbeast_metadata: Account<'info, HashBeastMetadata>,

    /// CHECK: PDA for NFT custody
    #[account(seeds = [HASHBEAST_CUSTODY_SEED], bump)]
    pub hashbeast_custody_pda: UncheckedAccount<'info>,

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = war_config.bump)]
    pub war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Metaplex Core program
    #[account(address = MPL_CORE_PROGRAM_ID @ ErrorCode::InvalidMplCoreProgram)]
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // leaderboard_score_for_mutation — home vs foreign-mercenary scoring
    // ------------------------------------------------------------------------

    #[test]
    fn leaderboard_score_home_win_full_credit() {
        assert_eq!(leaderboard_score_for_mutation(100, true), 100);
        assert_eq!(leaderboard_score_for_mutation(1_000_000, true), 1_000_000);
        assert_eq!(leaderboard_score_for_mutation(0, true), 0);
    }

    #[test]
    fn leaderboard_score_foreign_win_half_credit() {
        assert_eq!(leaderboard_score_for_mutation(100, false), 50);
        assert_eq!(leaderboard_score_for_mutation(1_000_000, false), 500_000);
        assert_eq!(leaderboard_score_for_mutation(0, false), 0);
    }

    #[test]
    fn leaderboard_score_foreign_truncates_down() {
        // Integer division rounds toward zero, so odd values lose 1 lamport.
        assert_eq!(leaderboard_score_for_mutation(1, false), 0);
        assert_eq!(leaderboard_score_for_mutation(3, false), 1);
        assert_eq!(leaderboard_score_for_mutation(99, false), 49);
    }

    #[test]
    fn leaderboard_score_home_never_loses_to_foreign_at_same_bonus() {
        // Property: home credit ≥ foreign credit at every bonus value.
        // Reinforces the design intent (home loyalty rewarded more).
        for bonus in [0u64, 1, 2, 10, 100, 1_000_000, u64::MAX / 2] {
            let home = leaderboard_score_for_mutation(bonus, true);
            let foreign = leaderboard_score_for_mutation(bonus, false);
            assert!(
                home >= foreign,
                "home {} < foreign {} at bonus {}",
                home,
                foreign,
                bonus
            );
        }
    }
}
