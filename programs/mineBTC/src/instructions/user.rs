// # User Instructions
//
// User-facing interactions: betting, autominers, round claims, gameplay hashbeasts, and story events.
//
// ## Key Functions
//
// - `initialize_player`: Creates a new player account and assigns them to a faction.
// - `join_bets`: Places one or more faction-direction bets for the current round.
// - `claim_round_rewards`: Claims winnings from completed rounds.
// - `init_autominer`: Sets up an automated recurring faction-direction betting system.
// - `execute_autominer_bet`: Executes an autominer bet (keeper function).
//
// The same bet now powers both round rewards and directional faction_war prediction rewards.
//

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::system_program::{transfer, Transfer};

use crate::errors::ErrorCode;
use crate::events::*;
use crate::genescience::{calculate_mutation_result, MutationType};
use crate::instructions::helper;
use crate::state::*;

fn load_program_account<T: AccountDeserialize>(account: &AccountInfo<'_>) -> Result<T> {
    require!(account.owner == &crate::ID, ErrorCode::InvalidAccount);
    let data = account.try_borrow_data()?;
    let mut data_slice: &[u8] = &data;
    T::try_deserialize(&mut data_slice)
}

fn load_global_config(account: &AccountInfo<'_>) -> Result<GlobalConfig> {
    load_program_account(account)
}

fn player_has_pending_reward_claims(player_data: &PlayerData) -> bool {
    player_data.pending_round_claims > 0 || player_data.pending_faction_war_claims > 0
}

// ========================================================================================
// =============================== PLAYER INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the MineBTC country arena
pub fn internal_initialize_player(
    ctx: Context<InitializePlayer>,
    faction_id: u8,
    referral_code: Option<Pubkey>,
) -> Result<()> {
    crate::log_fn!("user", "internal_initialize_player");
    msg!(
        "👤 [initialize_player] Initializing player account. Authority: {}. Faction ID: {}",
        ctx.accounts.authority.key(),
        faction_id
    );

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
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        msg!("     Referral code provided: {}", ref_code);
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
        msg!(
            "     Referrer's referral count: {} referrer_faction={}",
            referrer_rewards.referrals_count,
            referrer_faction_id
        );

        // Set player's referral code
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
        msg!("     No referral code provided, using system referral account");
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };

    player_data.active_multiplier = BASE_MULTIPLIER;

    // Initialize MineBtc staking fields
    player_data.degenbtc_hashpower = 0;
    player_data.degenbtc_staked = 0;
    player_data.degenbtc_degenbtc_reward_debt = 0;
    player_data.degenbtc_sol_reward_debt = 0;
    msg!("     MineBtc staking fields initialized");

    // Initialize LP staking fields
    player_data.lp_hashpower = 0;
    player_data.lp_staked = 0;
    player_data.lp_sol_reward_debt = 0;
    player_data.lp_degenbtc_reward_debt = 0;
    msg!("     LP staking fields initialized");

    // Initialize pending rewards
    player_data.pending_sol_rewards = 0;
    player_data.pending_dbtc_rewards = 0;
    player_data.pending_staking_dbtc_rewards = 0;
    player_data.pending_round_claims = 0;
    player_data.pending_faction_war_claims = 0;
    msg!("     Pending rewards initialized");

    // Initialize position tracking vectors
    player_data.degenbtc_position_indices = Vec::new();
    player_data.lp_position_indices = Vec::new();
    msg!("     Position tracking initialized");

    // Initialize hashbeast staking
    player_data.staked_hashbeasts = Vec::new();
    player_data.hashbeast_multiplier = BASE_MULTIPLIER as u16; // Default 1.0x (no hashbeasts staked)
    msg!("     HashBeast staking initialized (0 hashbeasts, 1.0x multiplier)");

    // Initialize free tickets vectors
    player_data.free_tickets = Vec::new();
    player_data.free_tickets_remaining = Vec::new();
    msg!("     Free tickets vectors initialized (empty)");

    // Initialize gameplay hashbeast state
    player_data.gameplay_hashbeast = Pubkey::default();
    player_data.gameplay_hashbeast_dna = [0u8; 32];
    player_data.gameplay_hashbeast_xp = 0;
    player_data.gameplay_unlock_request_faction_war = 0;
    player_data.current_faction_war_score = 0;
    player_data.current_faction_war_score_cycle_id = 0;
    msg!("     Gameplay hashbeast state initialized");

    // Initialize new player's referral rewards account
    msg!("   Initializing new player's referral rewards account...");
    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    new_player_rewards.owner_faction_id = faction_id;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.pending_sol_rewards = 0;
    new_player_rewards.total_sol_earned = 0;
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

/// Join a round by betting SOL or using free tickets (single prediction).
/// Each bet selects a faction and a faction_war direction.
///
/// Parameters:
/// - bet_types: Vector of bet types (`FactionDirection { faction_id, direction }`)
/// - amount_per_bet: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
#[inline(never)]
fn init_or_load_faction_war_state_account<'info>(
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    faction_war_state_info: &AccountInfo<'info>,
    faction_war_id: u64,
    faction_war_state_bump: u8,
) -> Result<Box<FactionWarState>> {
    let faction_war_id_bytes = faction_war_id.to_le_bytes();
    let faction_war_state_bump_seed = [faction_war_state_bump];
    let faction_war_state_seeds: &[&[u8]] = &[
        FACTION_WAR_STATE_SEED,
        faction_war_id_bytes.as_ref(),
        faction_war_state_bump_seed.as_ref(),
    ];
    let created = helper::init_pda_account_zeroed_if_needed::<FactionWarState>(
        payer,
        faction_war_state_info,
        system_program,
        faction_war_state_seeds,
        FactionWarState::LEN,
    )?;
    msg!(
        "🪖 [init_or_load_faction_war_state_account] faction_war_id={} account={} created={}",
        faction_war_id,
        faction_war_state_info.key(),
        created
    );
    load_faction_war_state_boxed(faction_war_state_info)
}

/// Load a `FactionWarState` directly into a heap allocation, deserializing
/// field-by-field to avoid stack-materializing the ~2.6KB struct.
#[inline(never)]
fn load_faction_war_state_boxed<'info>(
    account: &AccountInfo<'info>,
) -> Result<Box<FactionWarState>> {
    require!(
        account.owner == &FactionWarState::owner(),
        ErrorCode::InvalidAccount
    );
    let data = account.try_borrow_data()?;
    require!(data.len() >= DISCRIMINATOR_SIZE, ErrorCode::InvalidAccount);
    require!(
        &data[..DISCRIMINATOR_SIZE] == FactionWarState::DISCRIMINATOR,
        ErrorCode::InvalidAccount
    );
    let mut boxed: Box<FactionWarState> =
        unsafe { helper::alloc_zeroed_boxed::<FactionWarState>() };
    let mut cursor: &[u8] = &data[DISCRIMINATOR_SIZE..];
    FactionWarState::deserialize_into(&mut boxed, &mut cursor)?;
    Ok(boxed)
}

#[inline(never)]
fn init_or_load_user_faction_war_bets_account<'info>(
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_faction_war_bets_info: &AccountInfo<'info>,
    faction_war_id: u64,
    owner: Pubkey,
    user_faction_war_bets_bump: u8,
) -> Result<Box<UserFactionWarBets>> {
    let faction_war_id_bytes = faction_war_id.to_le_bytes();
    let user_faction_war_bets_bump_seed = [user_faction_war_bets_bump];
    let user_faction_war_bets_seeds: &[&[u8]] = &[
        USER_FACTION_WAR_BETS_SEED,
        owner.as_ref(),
        faction_war_id_bytes.as_ref(),
        user_faction_war_bets_bump_seed.as_ref(),
    ];
    let created = helper::init_pda_account_if_needed(
        payer,
        user_faction_war_bets_info,
        system_program,
        user_faction_war_bets_seeds,
        UserFactionWarBets::LEN,
        &UserFactionWarBets::blank(),
    )?;
    msg!(
        "🧾 [init_or_load_user_faction_war_bets_account] faction_war_id={} owner={} account={} created={}",
        faction_war_id,
        owner,
        user_faction_war_bets_info.key(),
        created
    );
    Ok(Box::new(helper::load_account_data::<UserFactionWarBets>(
        user_faction_war_bets_info,
    )?))
}

#[inline(never)]
pub fn internal_join_bets<'info>(
    accounts: &mut JoinBets<'info>,
    round_id: u64,
    faction_war_id: u64,
    bet_types: Vec<BetType>,
    amount_per_bet: u64,
    use_ticket: Option<u8>,
    user_game_bet_bump: u8,
    faction_war_state_bump: u8,
    user_faction_war_bets_bump: u8,
) -> Result<()> {
    crate::log_fn!("user", "internal_join_bets");
    msg!(
        "🎲 [join_bets] User joining round with {} bet positions. User: {}",
        bet_types.len(),
        accounts.authority.key()
    );
    msg!("   Amount per bet: {} lamports", amount_per_bet);
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

    let lp_ops = accounts.dbtc_mining.pol_stats.lp_operations_count;
    require!(
        accounts.faction_war_config.current_faction_war_id == faction_war_id,
        ErrorCode::InvalidParameters
    );
    let authority_info = accounts.authority.as_ref();
    let system_program_info = accounts.system_program.as_ref();
    let faction_war_state_info = accounts.faction_war_state.as_ref();
    let user_faction_war_bets_info = accounts.user_faction_war_bets.as_ref();
    let mut faction_war_state = init_or_load_faction_war_state_account(
        authority_info,
        system_program_info,
        faction_war_state_info,
        faction_war_id,
        faction_war_state_bump,
    )?;
    let mut user_faction_war_bets = init_or_load_user_faction_war_bets_account(
        authority_info,
        system_program_info,
        user_faction_war_bets_info,
        faction_war_id,
        accounts.authority.key(),
        user_faction_war_bets_bump,
    )?;
    msg!(
        "🪖 [join_bets] loaded faction_war_state_id={} stage={} user_fw_owner={} pending_claims={}",
        faction_war_state.faction_war_id,
        faction_war_state.stage,
        user_faction_war_bets.owner,
        accounts.player_data.pending_faction_war_claims
    );

    // Call internal_process_bets for all bets at once
    internal_process_bets(
        round_id,
        &global_config,
        &mut accounts.tax_config,
        &mut accounts.player_data,
        &mut accounts.game_session,
        &mut accounts.user_game_bet,
        &accounts.authority.to_account_info(),
        &accounts.sol_treasury.to_account_info(),
        &accounts.sol_rewards_vault.to_account_info(),
        &accounts.sol_prize_pot_vault.to_account_info(),
        &accounts.faction_war_sol_vault.to_account_info(),
        &accounts.system_program.to_account_info(),
        user_game_bet_bump,
        accounts.authority.key(),
        amount_per_bet,
        bet_types.clone(),
        use_ticket,
        None, // User wallet signs the transaction
        None, // No autominer info
        &mut accounts.faction_war_config,
        faction_war_state.as_mut(),
        faction_war_state_bump,
        lp_ops,
        user_faction_war_bets.as_mut(),
        user_faction_war_bets_bump,
        accounts.referrer_rewards.as_mut(),
    )?;
    helper::store_account_data(faction_war_state_info, faction_war_state.as_ref())?;
    helper::store_account_data(user_faction_war_bets_info, user_faction_war_bets.as_ref())?;
    msg!(
        "🧾 [join_bets] persisted faction_war_state_id={} mining_pool={} player_pending_claims={}",
        faction_war_state.faction_war_id,
        faction_war_state.faction_war_mining_pool,
        accounts.player_data.pending_faction_war_claims
    );

    msg!(
        "✅ [join_bets] All {} bet positions placed successfully",
        bet_types.len()
    );
    Ok(())
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

/// Initialize autominer vault for recurring faction-direction bets.
/// Block-based autominers are no longer supported.
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
    msg!("🤖 [init_autominer] Initializing autominer vault");
    msg!("   Owner: {}", ctx.accounts.user_wallet.key());
    msg!("   SOL per round: {} lamports", sol_per_round);
    msg!("   Number of rounds: {}", num_rounds);

    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;

    msg!("   Validating parameters...");
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
                msg!("     ✓ Specific autominer picks: {}", picks.len());
            }
            FactionsConfig::Random { count, direction } => {
                require!(
                    *count > 0 && *count <= global_config.supported_factions.len() as u8,
                    ErrorCode::InvalidParameters
                );
                msg!(
                    "     ✓ Factions: {} random factions with direction {:?}",
                    count,
                    direction
                );
                bets_per_round = *count as u64;
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
    msg!(
        "     Total SOL for all rounds: {} SOL ({} rounds × {} SOL)",
        (total_sol as f64 / 1e9),
        num_rounds,
        (reserve_per_round as f64 / 1e9)
    );

    autominer_vault.owner = ctx.accounts.user_wallet.key();
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

    // Transfer SOL to global autominer custody.
    if total_sol > 0 {
        msg!("   Transferring SOL to autominer custody...");
        helper::transfer_to_autominer_custody(
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.autominer_custody.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            total_sol,
        )?;
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

/// Update autominer run controls (add rounds, can_reload)
/// Can only be called by vault owner
/// Handles the extra SOL reserve for newly added rounds.
pub fn internal_update_autominer(
    ctx: Context<UpdateAutominer>,
    rounds_added_: Option<u32>,
    can_reload: Option<bool>,
) -> Result<()> {
    crate::log_fn!("user", "internal_update_autominer");
    msg!("🔄 [update_autominer] Updating autominer run controls");
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

    msg!("   Current configuration:");
    msg!("     SOL per round: {} lamports", old_sol_per_round);
    msg!("     Rounds remaining: {}", rounds_remaining);
    msg!("     SOL balance (remaining): {} lamports", old_sol_balance);
    msg!("     Can reload: {}", old_can_reload);

    msg!("   New configuration:");
    msg!("     SOL per round: {} lamports", new_sol_per_round);
    msg!("     Rounds remaining: {}", new_rounds_remaining);
    msg!("     Can reload: {}", new_can_reload);

    // Validate the existing per-round reserve based on autominer mode. Updating
    // no longer allows stake-size changes; users can only add more funded
    // rounds or toggle reload behavior.
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

    if rounds_added > 0 {
        msg!(
            "     ✓ Adding {} more rounds (total: {} rounds)",
            rounds_added,
            new_rounds_remaining
        );
    } else {
        msg!("     ✓ Rounds unchanged ({} rounds)", rounds_remaining);
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
    } else {
        msg!("   No SOL transfer needed");
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

    msg!("✅ [update_autominer] Autominer updated successfully");

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
    faction_war_id: u64,
    user_game_bet_bump: u8,
    faction_war_state_bump: u8,
    user_faction_war_bets_bump: u8,
    custody_bump: u8,
) -> Result<()> {
    crate::log_fn!("user", "internal_execute_autominer_bet");
    msg!("🤖 [execute_autominer_bet] Executing autominer bets");
    msg!("   Owner: {}", accounts.autominer_vault.owner);
    msg!("   Caller: {}", accounts.caller.key());
    require!(
        accounts.system_program.key() == anchor_lang::system_program::ID,
        ErrorCode::InvalidProgramId
    );
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
    require!(
        accounts.faction_war_config.current_faction_war_id == faction_war_id,
        ErrorCode::InvalidParameters
    );
    let caller_info = accounts.caller.as_ref();
    let system_program_info = accounts.system_program.as_ref();
    let faction_war_state_info = accounts.faction_war_state.as_ref();
    let user_faction_war_bets_info = accounts.user_faction_war_bets.as_ref();
    let mut faction_war_state = init_or_load_faction_war_state_account(
        caller_info,
        system_program_info,
        faction_war_state_info,
        faction_war_id,
        faction_war_state_bump,
    )?;
    let mut user_faction_war_bets = init_or_load_user_faction_war_bets_account(
        caller_info,
        system_program_info,
        user_faction_war_bets_info,
        faction_war_id,
        accounts.autominer_vault.owner,
        user_faction_war_bets_bump,
    )?;
    msg!(
        "🤖 [execute_autominer_bet] loaded faction_war_state_id={} stage={} user_fw_owner={} rounds_remaining={}",
        faction_war_state.faction_war_id,
        faction_war_state.stage,
        user_faction_war_bets.owner,
        accounts.autominer_vault.rounds_remaining
    );

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

    msg!("   Vault state:");
    msg!(
        "     Rounds remaining: {}. Last bet round ID: {}. SOL per round: {} SOL",
        rounds_remaining,
        last_bet_round_id,
        (sol_per_round as f64 / 1e9)
    );

    require!(rounds_remaining > 0, ErrorCode::NoRoundsRemaining);

    // Generate bet types dynamically from configuration
    msg!("   Generating bet types from configuration...");

    // Generate bet types using helper function
    let bet_types = make_bets_vec(factions_config.clone(), &clock, &global_config)?;
    msg!("     Generated {} bet types", bet_types.len());

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
    msg!(
        "     SOL reserve consumed this execution: {} SOL",
        (reserve_per_round as f64 / 1e9)
    );
    if use_ticket.is_some() {
        msg!(
            "     Ticket mode caller compensation: {} SOL",
            (total_caller_compensation as f64 / 1e9)
        );
    } else {
        msg!(
            "     Caller compensation: {} SOL (0.1% of {} SOL, max 0.00005 SOL)",
            (total_caller_compensation as f64 / 1e9),
            (sol_per_round as f64 / 1e9)
        );
    }

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
        validate_min_sol_bet_per_position(bet_per)?;
        msg!(
            "     SOL mode: {} SOL per bet ({} SOL / {} bets)",
            (bet_per as f64 / 1e9),
            (sol_for_betting as f64 / 1e9),
            bet_types.len()
        );
        (bet_per, None)
    };

    // Pay caller compensation.
    if total_caller_compensation > 0 {
        msg!("   Paying caller compensation...");
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
        msg!(
            "     Caller compensation: {} SOL transferred",
            (total_caller_compensation as f64 / 1e9)
        );
    }

    // Now borrow mutably to update state
    let autominer_vault = &mut accounts.autominer_vault;
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

    // Update remaining SOL balance tracked for this autominer.
    autominer_vault.sol_balance = autominer_vault
        .sol_balance
        .checked_sub(reserve_per_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Place bets using the shared round/faction_war prediction path.
    msg!(
        "   Placing {} bets for round {}...",
        bet_types.len(),
        current_round_id
    );

    // Prepare PDA signer seeds for autominer custody
    let autominer_seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];

    // Prepare autominer info
    let autominer_info = AutominerBetInfo {
        vault: accounts.autominer_vault.key(),
        caller: accounts.caller.key(),
        compensation: total_caller_compensation,
        rounds_remaining: new_rounds_remaining,
    };

    let lp_ops = accounts.dbtc_mining.pol_stats.lp_operations_count;

    // Call internal_process_bets with autominer vault as payer (PDA signs via seeds)
    // Process all bets at once
    internal_process_bets(
        current_round_id,
        &global_config,
        &mut accounts.tax_config,
        &mut accounts.player_data,
        &mut accounts.game_session,
        &mut accounts.user_game_bet,
        &autominer_custody_info,
        &accounts.sol_treasury.to_account_info(),
        &accounts.sol_rewards_vault.to_account_info(),
        &accounts.sol_prize_pot_vault.to_account_info(),
        &accounts.faction_war_sol_vault.to_account_info(),
        &accounts.system_program.to_account_info(),
        user_game_bet_bump,
        owner_key,
        bet_size_per_bet,
        bet_types.clone(),
        effective_use_ticket,  // None for SOL, Some(tier) for tickets
        Some(autominer_seeds), // PDA signs via seeds
        Some(autominer_info),
        &mut accounts.faction_war_config,
        faction_war_state.as_mut(),
        faction_war_state_bump,
        lp_ops,
        user_faction_war_bets.as_mut(),
        user_faction_war_bets_bump,
        accounts.referrer_rewards.as_mut(),
    )?;
    helper::store_account_data(faction_war_state_info, faction_war_state.as_ref())?;
    helper::store_account_data(user_faction_war_bets_info, user_faction_war_bets.as_ref())?;
    msg!(
        "🤖 [execute_autominer_bet] persisted faction_war_state_id={} mining_pool={} vault_rounds_left={}",
        faction_war_state.faction_war_id,
        faction_war_state.faction_war_mining_pool,
        accounts.autominer_vault.rounds_remaining
    );

    msg!("✅ [execute_autominer_bet] Autominer bets executed successfully");
    msg!(
        "   {} bets of {} SOL each for round {}",
        bet_types.len(),
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
    crate::log_fn!("user", "internal_stop_autominer");
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

/// Claim rewards for a user after round ends.
/// Round payouts depend on the winning faction plus the randomly resolved round direction.
pub fn internal_claim_round_rewards(round_id: u64, ctx: Context<ClaimRoundRewards>) -> Result<()> {
    crate::log_fn!("user", "internal_claim_round_rewards");
    msg!(
        "💰 [claim_rewards] User claiming rewards. User: {}",
        ctx.accounts.user_wallet.key()
    );
    msg!("   Round ID: {}", round_id);

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

    // Check which factions user bet on and calculate rewards.
    msg!(
        "   User bet on {} factions: {:?}",
        user_bet.faction_ids.len(),
        user_bet.faction_ids
    );
    msg!(
        "     Winning faction ID: {}, Winning direction: {}",
        game_session.winning_faction_id,
        game_session.winning_direction
    );

    // Calculate rewards using helper function
    let (total_sol_reward, total_dbtc_reward) = calculate_round_rewards(user_bet, game_session)?;
    let claim_won = total_sol_reward > 0 || total_dbtc_reward > 0;

    let mutation_type = if claim_won && user_bet.mutation_type == 0 {
        if let Some(roll) =
            build_round_claim_mutation_roll(user_bet, game_session, player_data.faction_id)
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

    // Update player rewards using helper function
    update_player_rewards(
        owner_key,
        player_data_key,
        player_data,
        &mut ctx.accounts.hodl_pool,
        total_sol_reward,
        total_dbtc_reward,
        round_id,
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
        &ctx.accounts.faction_war_state.to_account_info(),
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
        &mut ctx.accounts.lootbox_claim,
        ctx.bumps.lootbox_claim,
        owner_key,
        round_id,
    )?;

    player_data.pending_round_claims = player_data
        .pending_round_claims
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!("✅ [claim_rewards] Rewards claimed successfully");
    msg!("   Round: {}", user_bet.round_id);

    emit!(RoundRewardsClaimed {
        user: ctx.accounts.player_data.owner,
        player_data: ctx.accounts.player_data.key(),
        round_id: user_bet.round_id,
        sol_reward: total_sol_reward,
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
    msg!("🤖 [claim_autominer_rewards] Claiming rewards with auto-reload");
    msg!("   Round ID: {}", round_id);
    msg!("   Autominer owner: {}", ctx.accounts.autominer_vault.owner);

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

    // Calculate rewards using helper function
    let (total_sol_reward, total_dbtc_reward) = calculate_round_rewards(user_bet, game_session)?;
    let claim_won = total_sol_reward > 0 || total_dbtc_reward > 0;

    let mutation_type = if claim_won && user_bet.mutation_type == 0 {
        if let Some(roll) =
            build_round_claim_mutation_roll(user_bet, game_session, player_data.faction_id)
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

    // Update player rewards using helper function
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

    // Mutation-bonus cycle score (autominer claim path).
    apply_mutation_bonus_score(
        mutation_type,
        user_bet,
        game_session,
        &ctx.accounts.faction_war_state.to_account_info(),
        player_data,
        owner_key,
    )?;

    // Loser lottery roll (autominer claim path).
    maybe_run_loser_lootbox_roll(
        claim_won,
        user_bet,
        game_session,
        player_data,
        &mut ctx.accounts.lootbox_queue,
        &mut ctx.accounts.lootbox_claim,
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
    if autominer_vault.can_reload && autominer_vault.use_ticket.is_some() {
        let reserve_per_round = get_ticket_caller_compensation();
        let rounds_to_add = total_sol_reward / reserve_per_round;
        let leftover_sol = total_sol_reward % reserve_per_round;

        msg!("🔄 Ticket mode reload, processing SOL rewards...");
        msg!(
            "   Keeper reserve per round: {} lamports",
            reserve_per_round
        );
        msg!("   Rounds to add: {}", rounds_to_add);
        msg!("   Leftover SOL: {} lamports", leftover_sol);

        if rounds_to_add > 0 {
            let sol_for_rounds = rounds_to_add * reserve_per_round;
            helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.autominer_custody.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                sol_for_rounds,
                ctx.bumps.sol_prize_pot_vault,
            )?;
            autominer_vault.sol_balance = autominer_vault
                .sol_balance
                .checked_add(sol_for_rounds)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            autominer_vault.rounds_remaining = autominer_vault
                .rounds_remaining
                .checked_add(rounds_to_add as u32)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        if leftover_sol > 0 {
            helper::transfer_from_sol_prize_pot_vault(
                &ctx.accounts.sol_prize_pot_vault.to_account_info(),
                &ctx.accounts.owner_wallet.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                leftover_sol,
                ctx.bumps.sol_prize_pot_vault,
            )?;
        }

        emit!(AutominerReloaded {
            autominer_vault: autominer_vault.key(),
            rounds_to_add: rounds_to_add as u32,
            sol_for_rounds: total_sol_reward
                .checked_sub(leftover_sol)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
            leftover_sol,
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
            autominer_vault.sol_balance = autominer_vault
                .sol_balance
                .checked_add(sol_for_rounds)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            autominer_vault.rounds_remaining = autominer_vault
                .rounds_remaining
                .checked_add(rounds_to_add as u32)
                .ok_or(ErrorCode::ArithmeticOverflow)?;

            emit!(AutominerReloaded {
                autominer_vault: autominer_vault.key(),
                rounds_to_add: rounds_to_add as u32,
                sol_for_rounds,
                leftover_sol,
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
        dbtc_reward: total_dbtc_reward,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Calculate SOL and MineBTC rewards for a user bet.
/// Returns (total_sol_reward, total_dbtc_reward)
fn calculate_round_rewards(
    user_bet: &UserGameBet,
    game_session: &GameSession,
) -> Result<(u64, u64)> {
    let mut total_sol_reward = 0u64;
    let mut total_dbtc_reward = 0u64;

    for (idx, &faction_id) in user_bet.faction_ids.iter().enumerate() {
        let direction = user_bet.directions.get(idx).copied().unwrap_or(0);
        let points_bet_on_faction = user_bet.points_bets.get(idx).copied().unwrap_or(0);
        let wgtd_points_bet_on_faction = user_bet.wgtd_points_bets.get(idx).copied().unwrap_or(0);

        msg!(
            "     Faction {} (direction {}): Points: {} SOL, Wgtd: {} DegenBTC",
            faction_id,
            direction,
            points_bet_on_faction as f64 / 1e9,
            wgtd_points_bet_on_faction as f64 / 1e6
        );

        let is_winning_faction = faction_id == game_session.winning_faction_id;
        let is_winning_direction = direction == game_session.winning_direction;

        if is_winning_faction && is_winning_direction {
            msg!("       ✓ Exact winning faction+direction - calculating rewards...");

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
                msg!("         SOL reward: {} SOL", sol_reward as f64 / 1e9);
            }

            // Exact-direction MineBTC rewards.
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
                msg!(
                    "         MineBtc reward: {} DegenBTC",
                    dbtc_reward as f64 / 1e6
                );
            }
        } else if is_winning_faction {
            msg!("       ✓ Same faction, different direction - consolation MineBTC rewards...");

            let same_faction_pool =
                game_session.dbtc_same_faction_direction_pools[direction as usize];
            let same_faction_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
                [faction_id as usize][direction as usize];

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
                msg!(
                    "         Same-faction MineBtc reward: {} DegenBTC",
                    dbtc_reward as f64 / 1e6
                );
            }
        }

        // Jackpot rewards — ANY bet on the jackpot faction gets a share,
        // regardless of direction (no direction check).
        if game_session.jackpot_hit
            && game_session.jackpot_rewards_index > 0
            && faction_id == game_session.jackpot_faction_id
            && wgtd_points_bet_on_faction > 0
        {
            msg!("       ✓ Jackpot faction — calculating jackpot reward...");
            let jackpot_reward = u64::try_from(helper::mul_div_u128(
                wgtd_points_bet_on_faction as u128,
                game_session.jackpot_rewards_index,
                INDEX_PRECISION as u128,
            )?)
            .map_err(|_| ErrorCode::ArithmeticOverflow)?;
            total_dbtc_reward = total_dbtc_reward
                .checked_add(jackpot_reward)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "         Jackpot MineBtc reward: {} DegenBTC",
                jackpot_reward as f64 / 1e6
            );
        }

        if !is_winning_faction && faction_id != game_session.jackpot_faction_id {
            msg!("       ✗ Not the winning faction - no round rewards");
        }
    }

    Ok((total_sol_reward, total_dbtc_reward))
}

fn build_round_claim_mutation_roll(
    user_bet: &UserGameBet,
    game_session: &GameSession,
    player_faction_id: u8,
) -> Option<ClaimMutationRoll> {
    let winning_faction = game_session.winning_faction_id;
    let winning_faction_index = winning_faction as usize;
    if winning_faction_index >= NUM_FACTIONS {
        return None;
    }

    let mut exact_sol = 0u64;
    let mut same_faction_sol = 0u64;
    for (idx, &faction_id) in user_bet.faction_ids.iter().enumerate() {
        if faction_id != winning_faction {
            continue;
        }
        let sol = user_bet.sol_bets.get(idx).copied().unwrap_or(0);
        let direction = user_bet.directions.get(idx).copied().unwrap_or(0);
        if direction == game_session.winning_direction {
            exact_sol = exact_sol.saturating_add(sol);
        } else {
            same_faction_sol = same_faction_sol.saturating_add(sol);
        }
    }

    let stake = exact_sol.saturating_add(same_faction_sol / 4);
    if stake == 0 {
        return None;
    }

    let mut chance_boost_bps = if exact_sol > 0 { 12_000u64 } else { 5_000u64 };
    if winning_faction == player_faction_id {
        let loyalty_boost = if exact_sol > 0 { 15_000u64 } else { 11_000u64 };
        chance_boost_bps = chance_boost_bps
            .saturating_mul(loyalty_boost)
            .saturating_div(BASIS_POINTS_DENOMINATOR);
    }

    Some(ClaimMutationRoll {
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
    })
}

fn record_round_claim_mutation(
    game_session: &mut GameSession,
    faction_id: usize,
    mutation_type: u8,
) -> Result<()> {
    if mutation_type == 0 {
        return Ok(());
    }
    game_session.mutations_per_faction[faction_id] = game_session.mutations_per_faction[faction_id]
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_mutations_this_round = game_session
        .total_mutations_this_round
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok(())
}

/// Update player rewards stats and add MineBTC to pending rewards
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
        "     Pending MineBtc rewards: {} (+{})",
        player_data.pending_dbtc_rewards as f64 / 1e6,
        total_dbtc_reward as f64 / 1e6
    );

    Ok(())
}

/// Helper struct for passing autominer info to internal_process_bets
pub struct AutominerBetInfo {
    pub vault: Pubkey,
    pub caller: Pubkey,
    pub compensation: u64,
    pub rounds_remaining: u32,
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
fn total_wgtd_points_for_faction(user_bet: &UserGameBet, faction_id: u8) -> u64 {
    user_bet
        .faction_ids
        .iter()
        .zip(user_bet.wgtd_points_bets.iter())
        .filter_map(|(&fid, &amount)| (fid == faction_id).then_some(amount))
        .fold(0u64, |acc, amount| acc.saturating_add(amount))
}

fn mutation_bonus_weight(mutation_type: u8) -> u64 {
    match mutation_type {
        1 => MUTATION_BONUS_WEIGHT_EVOLUTION,
        2 => MUTATION_BONUS_WEIGHT_POWER,
        3 => MUTATION_BONUS_WEIGHT_TRAIT,
        _ => 0,
    }
}

/// Apply the round-claim mutation bonus to the cycle leaderboard.
///
/// Bonus formula: `user_wgtd_points_on_winner × active_multiplier / BASE_MULTIPLIER × mutation_weight`.
/// Bonus accrues to:
/// - `faction_war_state.faction_gameplay_scores[winner]` — the winning country's cycle score
/// - `player_data.current_faction_war_score` — for MVP tracking (with cycle-rollover reset)
/// - `faction_war_state.faction_mvp_*[winner]` — if this player just took the lead
///
/// Gating:
/// - `mutation_type` must be 1/2/3 (Evolution/Power/Trait)
/// - The passed `faction_war_state_info` must match the round's cycle id (seeds check)
/// - `faction_war_state.stage == 0` (cycle still active — late claims after settlement skip silently)
/// - `winner < active_faction_count`
/// - User had non-zero weighted points bet on the winning country
///
/// Bonus is silently skipped (Ok(())) if any gate fails. Gates that indicate
/// a *wrong* account (seed mismatch) return an error instead.
#[inline(never)]
fn apply_mutation_bonus_score<'info>(
    mutation_type: u8,
    user_bet: &UserGameBet,
    game_session: &GameSession,
    faction_war_state_info: &AccountInfo<'info>,
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

    let cycle_id = game_session.faction_war_id_when_played;
    let cycle_id_bytes = cycle_id.to_le_bytes();
    let (expected_pda, _) = Pubkey::find_program_address(
        &[FACTION_WAR_STATE_SEED, cycle_id_bytes.as_ref()],
        &crate::ID,
    );
    require_keys_eq!(
        faction_war_state_info.key(),
        expected_pda,
        ErrorCode::InvalidAccount
    );

    let mut faction_war_state = load_faction_war_state_boxed(faction_war_state_info)?;

    // Late-claim gate: cycle has settled → drop bonus.
    if faction_war_state.stage != 0 {
        msg!(
            "🎁 [apply_mutation_bonus_score] cycle {} already settled (stage={}); dropping bonus",
            cycle_id,
            faction_war_state.stage
        );
        return Ok(());
    }
    require!(
        faction_war_state.faction_war_id == cycle_id,
        ErrorCode::InvalidAccount
    );

    let winner = game_session.winning_faction_id;
    let winner_idx = winner as usize;
    if winner_idx >= faction_war_state.active_faction_count as usize {
        return Ok(());
    }

    let user_wgtd = total_wgtd_points_for_faction(user_bet, winner);
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

    faction_war_state.faction_gameplay_scores[winner_idx] = faction_war_state
        .faction_gameplay_scores[winner_idx]
        .checked_add(bonus)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if player_data.current_faction_war_score_cycle_id != cycle_id {
        player_data.current_faction_war_score = 0;
        player_data.current_faction_war_score_cycle_id = cycle_id;
    }
    player_data.current_faction_war_score = player_data
        .current_faction_war_score
        .checked_add(bonus)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    if player_data.current_faction_war_score > faction_war_state.faction_mvp_score[winner_idx] {
        faction_war_state.faction_mvp_user[winner_idx] = owner_key;
        faction_war_state.faction_mvp_score[winner_idx] = player_data.current_faction_war_score;
    }

    let total_after = faction_war_state.faction_gameplay_scores[winner_idx];
    helper::store_account_data(faction_war_state_info, faction_war_state.as_ref())?;

    emit!(crate::events::GameplayScoreAccumulated {
        faction_war_id: cycle_id,
        faction_id: winner,
        score_source: GAMEPLAY_SCORE_SOURCE_MUTATION_BONUS,
        score_added: bonus,
        faction_total_score: total_after,
        user: owner_key,
    });

    msg!(
        "🎁 [apply_mutation_bonus_score] cycle={} winner={} bonus={} (mut_type={}, user_wgtd={}, mult={})",
        cycle_id,
        winner,
        bonus,
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
pub fn maybe_run_loser_lootbox_roll(
    claim_won: bool,
    user_bet: &UserGameBet,
    game_session: &GameSession,
    player_data: &PlayerData,
    lootbox_queue: &mut LootboxQueue,
    lootbox_claim: &mut LootboxClaim,
    lootbox_claim_bump: u8,
    user_key: Pubkey,
    round_id: u64,
) -> Result<()> {
    if claim_won {
        return Ok(());
    }
    // Already has an outstanding reservation → block re-rolling.
    if lootbox_claim.asset != Pubkey::default() {
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
    let user_wgtd_on_home = total_wgtd_points_for_faction(user_bet, player_data.faction_id);
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

    // Populate the reservation. (Anchor `init_if_needed` already created
    // the account at struct-load time if it didn't exist; we just write
    // the fields here.)
    let now = Clock::get()?.unix_timestamp;
    lootbox_claim.bump = lootbox_claim_bump;
    lootbox_claim.user = user_key;
    lootbox_claim.asset = won_asset;
    lootbox_claim.faction_id = player_data.faction_id;

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

struct ClaimMutationRoll {
    faction_id: usize,
    stake: u64,
    /// Country's accumulated SOL volume since its last win — fed straight
    /// into the volume_factor in `calculate_mutation_result`. Sourced from
    /// `GameSession.winning_faction_volume_at_round` for round-claim, and
    /// from `faction_war_state.faction_sol_totals[home]` for cycle-claim.
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

    require!(
        hashbeast_metadata.is_some()
            && hashbeast_metadata.as_ref().unwrap().mint == expected_hashbeast,
        ErrorCode::HashBeastMetadataNotFound
    );
    let hashbeast_metadata = hashbeast_metadata.unwrap();
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

/// Internal join_bets logic for batched processing
/// Calculates totals, performs single transfers, and updates state for all bets
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn internal_process_bets<'info>(
    round_id: u64,
    global_config: &GlobalConfig,
    tax_config: &mut Account<'info, TaxConfig>,
    player_data: &mut Account<'info, PlayerData>,
    game_session: &mut Account<'info, GameSession>,
    user_game_bet: &mut Account<'info, UserGameBet>,
    payer: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    sol_rewards_vault: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    faction_war_sol_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_game_bet_bump: u8,
    owner_key: Pubkey,
    amount_per_bet: u64,
    bet_types: Vec<BetType>,
    use_ticket: Option<u8>,
    signer_seeds: Option<&[&[u8]]>,
    autominer_info: Option<AutominerBetInfo>,
    faction_war_config: &mut Account<'info, FactionWarConfig>,
    faction_war_state: &mut FactionWarState,
    faction_war_state_bump: u8,
    lp_operations_count: u32,
    user_faction_war_bets: &mut UserFactionWarBets,
    user_faction_war_bets_bump: u8,
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
    require!(faction_war_config.is_active, ErrorCode::FactionWarNotActive);

    msg!(
        "   Processing batch of {} bets for round {}",
        bet_types.len(),
        round_id
    );

    if faction_war_state.faction_war_id == 0 || faction_war_state.active_faction_count == 0 {
        let active_faction_count = global_config.supported_factions.len();
        let start_ranks = faction_war_config.prev_faction_war_ranks;
        let seeded_treasury_base = faction_war_state
            .treasury_reward_base_amount
            .checked_add(tax_config.unassigned_faction_war_treasury_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        faction_war_state.bump = faction_war_state_bump;
        faction_war_state.faction_war_id = faction_war_config.current_faction_war_id;
        faction_war_state.start_timestamp = clock.unix_timestamp.max(0) as u64;
        faction_war_state.stage = 0;
        faction_war_state.active_faction_count = active_faction_count as u8;
        faction_war_state.total_degenbtc_mined_in_faction_war = 0;
        faction_war_state.faction_war_mining_pool = 0;
        faction_war_state.start_ranks = start_ranks;
        faction_war_state.final_ranks = start_ranks;
        faction_war_state.rank_deltas = [0i8; NUM_FACTIONS];
        faction_war_state.resolved_directions =
            [PredictionDirection::Neutral.as_index() as u8; NUM_FACTIONS];
        faction_war_state.faction_direction_totals =
            [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        faction_war_state.loyalty_direction_totals =
            [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        faction_war_state.faction_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.loyalty_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_hashbeast_reward_pools = [0u64; NUM_FACTIONS];
        faction_war_state.faction_round_wins = [0u16; NUM_FACTIONS];
        faction_war_state.faction_sol_totals = [0u64; NUM_FACTIONS];
        faction_war_state.faction_sol_direction_totals =
            [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        faction_war_state.faction_gameplay_scores = [0u64; NUM_FACTIONS];
        faction_war_state.faction_mvp_user = [Pubkey::default(); NUM_FACTIONS];
        faction_war_state.faction_mvp_score = [0u64; NUM_FACTIONS];
        faction_war_state.faction_mvp_bonus = [0u64; NUM_FACTIONS];
        faction_war_state.eligible_hashbeast_direction_totals =
            [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        faction_war_state.treasury_reward_base_amount = seeded_treasury_base;
        faction_war_state.treasury_claimed_bitmap = 0;
        faction_war_state.sol_reward_pool = 0;
        tax_config.unassigned_faction_war_treasury_amount = 0;

        // Faction war settles after the next LP burn completes.
        faction_war_config.faction_war_settle_cycle = lp_operations_count
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        faction_war_config.reset_cycle_round_tracking();

        emit!(crate::events::FactionWarAutoStarted {
            faction_war_id: faction_war_state.faction_war_id,
            start_timestamp: faction_war_state.start_timestamp,
            settle_cycle: faction_war_config.faction_war_settle_cycle,
        });
        msg!(
            "   🌍 Auto-initialized faction war {} (settle after LP cycle #{}, treasury_base={})",
            faction_war_state.faction_war_id,
            faction_war_config.faction_war_settle_cycle,
            faction_war_state.treasury_reward_base_amount,
        );
    } else {
        require!(
            faction_war_state.faction_war_id == faction_war_config.current_faction_war_id,
            ErrorCode::InvalidState
        );
        require!(faction_war_state.stage == 0, ErrorCode::FactionWarNotActive);
    }

    if user_faction_war_bets.owner == Pubkey::default() {
        user_faction_war_bets.bump = user_faction_war_bets_bump;
        user_faction_war_bets.owner = owner_key;
        user_faction_war_bets.faction_war_id = faction_war_state.faction_war_id;
        user_faction_war_bets.gameplay_hashbeast = Pubkey::default();
        user_faction_war_bets.hashbeast_bonus_eligible = false;
        user_faction_war_bets.direction_bets = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        user_faction_war_bets.sol_direction_bets =
            [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
        player_data.pending_faction_war_claims = player_data
            .pending_faction_war_claims
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    } else {
        require!(
            user_faction_war_bets.owner == owner_key,
            ErrorCode::Unauthorized
        );
        require!(
            user_faction_war_bets.faction_war_id == faction_war_state.faction_war_id,
            ErrorCode::InvalidState
        );
    }

    let active_faction_war_faction_count = faction_war_state.active_faction_count as usize;

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
    msg!(
        "🎮 [user.internal_process_bets] cycle_sol_split_pct={} faction_war_active={} faction_war_id={}",
        cycle_sol_split_pct,
        faction_war_config.is_active,
        faction_war_state.faction_war_id
    );

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

        cycle_sol_split_per_bet = if faction_war_config.is_active && cycle_sol_split_pct > 0 {
            let split = amount_per_bet
                .checked_mul(cycle_sol_split_pct)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                / M_HUNDRED;
            msg!(
                "🎮 [user.internal_process_bets] cycle_split_per_bet={} SOL ({} pct of {} SOL)",
                split as f64 / 1e9,
                cycle_sol_split_pct,
                amount_per_bet as f64 / 1e9
            );
            split
        } else {
            msg!("🎮 [user.internal_process_bets] cycle_split=0 (inactive or pct=0)");
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
        msg!(
                "🎮 [user.internal_process_bets] gross={} SOL fee={} SOL cycle_split={} SOL pot_net={} SOL",
                amount_per_bet as f64 / 1e9,
                fee as f64 / 1e9,
                cycle_sol_split_per_bet as f64 / 1e9,
                net as f64 / 1e9
            );

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
            msg!(
                "   Referral fee (+{} bps, same_faction={}): {} SOL",
                bps,
                same_faction,
                cut as f64 / 1e9
            );
            cut
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

    // Transfer cycle SOL split to faction-war vault (only when cycles are active)
    let total_cycle_sol_split = if faction_war_config.is_active && cycle_sol_split_per_bet > 0 {
        cycle_sol_split_per_bet
            .checked_mul(num_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        0
    };
    if total_cycle_sol_split > 0 {
        // SECURITY: the JoinBets / ExecuteAutominerBet Accounts structs intentionally
        // omit the seeds constraint on this account to keep the parser stack under
        // 4KB. We validate the address manually here so an attacker can't redirect
        // the cycle SOL split to a wallet they control.
        let (expected_vault, vault_bump) =
            Pubkey::find_program_address(&[FACTION_WAR_SOL_VAULT_SEED], &crate::id());
        msg!(
            "💰 [user.internal_process_bets] cycle_split_transfer: expected_vault={} actual_vault={} bump={} amount={} SOL",
            expected_vault,
            faction_war_sol_vault.key(),
            vault_bump,
            total_cycle_sol_split as f64 / 1e9
        );
        require_keys_eq!(
            faction_war_sol_vault.key(),
            expected_vault,
            ErrorCode::InvalidAccount
        );
        msg!(
            "   Transferring cycle SOL split ({} SOL) to faction-war vault",
            total_cycle_sol_split as f64 / 1e9
        );
        do_transfer(faction_war_sol_vault, total_cycle_sol_split)?;
        let old_pool = faction_war_state.sol_reward_pool;
        faction_war_state.sol_reward_pool = faction_war_state
            .sol_reward_pool
            .checked_add(total_cycle_sol_split)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "🎮 [user.internal_process_bets] sol_reward_pool updated: {} -> {} SOL",
            old_pool as f64 / 1e9,
            faction_war_state.sol_reward_pool as f64 / 1e9
        );
    } else {
        msg!("🎮 [user.internal_process_bets] no cycle SOL split to transfer");
    }

    if total_referral_cut > 0 && has_referrer {
        helper::validate_referrer_rewards_account(
            &player_data.referral_code,
            referrer_rewards.as_deref(),
        )?;
        let rr = referrer_rewards.expect("validated above");
        let remaining_cap = MAX_REFERRER_SOL_LIFETIME.saturating_sub(rr.total_sol_earned);
        let referrer_cut = total_referral_cut.min(remaining_cap);
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
            msg!(
                "   Referrer SOL cut: {} SOL (cap_remaining_after={})",
                referrer_cut as f64 / 1e9,
                MAX_REFERRER_SOL_LIFETIME.saturating_sub(rr.total_sol_earned) as f64 / 1e9,
            );
        }
    }
    if total_stakers_fee > 0 {
        msg!(
            "   Transferring total stakers fees ({} SOL)",
            total_stakers_fee as f64 / 1e9
        );
        do_transfer(sol_rewards_vault, total_stakers_fee)?;
    }
    if total_protocol_fee > 0 {
        msg!(
            "   Transferring total protocol fees ({} SOL) to sol_treasury",
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

    // Initialize UserGameBet if needed.
    if user_game_bet.owner == Pubkey::default() {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.faction_war_id = faction_war_state.faction_war_id;
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
        msg!("     New bet account initialized");
    } else {
        require!(user_game_bet.round_id == round_id, ErrorCode::InvalidRound);
        require!(
            user_game_bet.faction_war_id == faction_war_state.faction_war_id,
            ErrorCode::InvalidState
        );
    }

    // Process each faction-direction bet.
    for bet_type in bet_types {
        let (faction_id, direction) = prediction_bet_parts(&bet_type)?;
        require!(
            (faction_id as usize) < active_faction_war_faction_count,
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

        // Update GameSession stats
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

        // Base faction-war rewards track correct predictions across every country.
        user_faction_war_bets.direction_bets[faction_index][direction_index] =
            user_faction_war_bets.direction_bets[faction_index][direction_index]
                .checked_add(wgtd_points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        user_faction_war_bets.sol_direction_bets[faction_index][direction_index] =
            user_faction_war_bets.sol_direction_bets[faction_index][direction_index]
                .checked_add(net_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        faction_war_state.faction_direction_totals[faction_index][direction_index] =
            faction_war_state.faction_direction_totals[faction_index][direction_index]
                .checked_add(wgtd_points_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        faction_war_state.faction_sol_direction_totals[faction_index][direction_index] =
            faction_war_state.faction_sol_direction_totals[faction_index][direction_index]
                .checked_add(net_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Additive per-country volume tracker (drives the volume_factor in
        // the mutation chance formula). Counts ALL bets on a country regardless
        // of the bettor's home country and direction. Reset to 0 when the
        // country wins a round.
        faction_war_config.faction_volume_since_last_win[faction_index] = faction_war_config
            .faction_volume_since_last_win[faction_index]
            .checked_add(net_per_bet)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Loyalty and HashBeast bonus layers only care about backing your own country.
        if faction_id == player_data.faction_id {
            faction_war_state.loyalty_direction_totals[faction_index][direction_index] =
                faction_war_state.loyalty_direction_totals[faction_index][direction_index]
                    .checked_add(wgtd_points_per_bet)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            faction_war_state.faction_sol_totals[faction_index] = faction_war_state
                .faction_sol_totals[faction_index]
                .checked_add(net_per_bet)
                .ok_or(ErrorCode::ArithmeticOverflow)?;

            if player_data.gameplay_hashbeast != Pubkey::default() {
                if !user_faction_war_bets.hashbeast_bonus_eligible {
                    user_faction_war_bets.hashbeast_bonus_eligible = true;
                    user_faction_war_bets.gameplay_hashbeast = player_data.gameplay_hashbeast;
                } else {
                    require_keys_eq!(
                        user_faction_war_bets.gameplay_hashbeast,
                        player_data.gameplay_hashbeast,
                        ErrorCode::InvalidAccount
                    );
                }
                faction_war_state.eligible_hashbeast_direction_totals[faction_index]
                    [direction_index] = faction_war_state.eligible_hashbeast_direction_totals
                    [faction_index][direction_index]
                    .checked_add(wgtd_points_per_bet)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                // Note: cycle leaderboard score is no longer added at bet time.
                // Country score now accrues only when (a) the country wins a
                // round (via track_faction_war_round_completion), and (b) when
                // a player's round-claim mutation roll succeeds (via
                // apply_mutation_bonus_score). See `mineBTC/CLAUDE.md`.
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
        gameplay_hashbeast: player_data.gameplay_hashbeast,
        gameplay_hashbeast_dna: player_data.gameplay_hashbeast_dna,
        active_multiplier: player_data.active_multiplier,
        gameplay_hashbeast_xp: player_data.gameplay_hashbeast_xp,
        round_id,
        faction_war_id: faction_war_state.faction_war_id,
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
                while random_factions.len() < *count as usize && attempts < 100 {
                    let slot_bytes = clock.slot.to_le_bytes();
                    let hash =
                        keccak::hash(&[slot_bytes, (attempts + 100u64).to_le_bytes()].concat());
                    let faction_id = hash.0[0] % max_factions as u8;
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
#[instruction(round_id: u64, faction_war_id: u64)]
pub struct JoinBets<'info> {
    /// CHECK: Program-owned PDA deserialized and validated in handler to keep parser stack small.
    /// No seeds/bump in derive macro to keep `JoinBets` stack under 4KB.
    #[account(mut)]
    pub global_config: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
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

    /// CHECK: Faction-war SOL vault (cycle jackpot reserve). No seeds/bump here
    /// to keep `JoinBets` stack small; validated manually in handler.
    #[account(mut)]
    pub faction_war_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump = tax_config.bump
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    /// Faction-war config (mut: settle cycle written on auto-start)
    #[account(mut, seeds = [FACTION_WAR_CONFIG_SEED], bump)]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists raw PDA state after betting.
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub faction_war_state: UncheckedAccount<'info>,

    /// Economy state (read for lp_operations_count to tie faction_war cycle to economy cycle)
    #[account(seeds = [MINE_BTC_MINING_SEED], bump = dbtc_mining.bump)]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists raw PDA state after betting.
    #[account(
        mut,
        seeds = [USER_FACTION_WAR_BETS_SEED, authority.key().as_ref(), &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub user_faction_war_bets: UncheckedAccount<'info>,

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
#[instruction(round_id: u64)]
pub struct ClaimRoundRewards<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump
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

    /// CHECK: Cycle state for the round being claimed. Seeds are validated
    /// inside the handler against `game_session.faction_war_id_when_played`,
    /// not in the macro (the cycle id is a runtime field on game_session, not
    /// an instruction arg). Mutated only when the mutation-bonus block fires.
    #[account(mut)]
    pub faction_war_state: UncheckedAccount<'info>,

    /// Country lootbox queue for the player's home faction. Read on every
    /// claim; mutated when a losing player's roll wins a slot.
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[player_data.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// Per-user reservation. `init_if_needed` so it exists for the eligibility
    /// check; populated only when a winning roll lands. Closed by
    /// `claim_lootbox_nft`.
    #[account(
        init_if_needed,
        payer = caller,
        space = LootboxClaim::LEN,
        seeds = [LOOTBOX_CLAIM_SEED, user_wallet.key().as_ref()],
        bump,
    )]
    pub lootbox_claim: Box<Account<'info, LootboxClaim>>,

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
        bump = player_data.bump
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

    /// CHECK: Cycle state for the round being claimed. Seeds validated in
    /// handler against `game_session.faction_war_id_when_played`. Mutated
    /// only when mutation-bonus block fires.
    #[account(mut)]
    pub faction_war_state: UncheckedAccount<'info>,

    /// Country lootbox queue for the autominer owner's home faction.
    #[account(
        mut,
        seeds = [LOOTBOX_QUEUE_SEED, &[player_data.faction_id]],
        bump = lootbox_queue.bump,
    )]
    pub lootbox_queue: Box<Account<'info, LootboxQueue>>,

    /// Per-user reservation, init_if_needed.
    #[account(
        init_if_needed,
        payer = caller,
        space = LootboxClaim::LEN,
        seeds = [LOOTBOX_CLAIM_SEED, owner_wallet.key().as_ref()],
        bump,
    )]
    pub lootbox_claim: Box<Account<'info, LootboxClaim>>,

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
#[instruction(current_round_id: u64, faction_war_id: u64)]
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
        bump
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
    #[account(mut)]
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

    /// CHECK: Faction-war SOL vault (cycle jackpot reserve). No seeds/bump here
    /// to keep `ExecuteAutominerBet` stack small; validated manually in handler.
    #[account(mut)]
    pub faction_war_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump = tax_config.bump
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    /// Faction-war config (mut: settle cycle written on auto-start)
    #[account(mut, seeds = [FACTION_WAR_CONFIG_SEED], bump)]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists raw PDA state after autominer execution.
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub faction_war_state: UncheckedAccount<'info>,

    /// Economy state (read for lp_operations_count to tie faction_war cycle to economy cycle)
    #[account(seeds = [MINE_BTC_MINING_SEED], bump = dbtc_mining.bump)]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists raw PDA state after autominer execution.
    #[account(
        mut,
        seeds = [USER_FACTION_WAR_BETS_SEED, autominer_vault.owner.as_ref(), &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub user_faction_war_bets: UncheckedAccount<'info>,

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

    /// CHECK: Validated in handler
    pub system_program: UncheckedAccount<'info>,
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

    msg!("🎮 === USING HASHBEAST FOR GAMEPLAY ===");
    msg!("   HashBeast mint: {}", hashbeast_mint);

    require!(
        ctx.accounts.global_config.gameplay_tuning.rpg_progression,
        ErrorCode::GameplayNotEnabled
    );
    require!(
        ctx.accounts.faction_war_config.is_active,
        ErrorCode::FactionWarNotActive
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
    msg!("🔒 Transferring hashbeast to custody PDA for gameplay");
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

    let gen = crate::genescience::get_evolution_stage(&hashbeast_metadata.dna);
    msg!("✅ HashBeast {} now active for gameplay", hashbeast_mint);
    msg!(
        "   Multiplier: {}, Gen: {}, XP: {}",
        hashbeast_metadata.multiplier,
        gen,
        hashbeast_metadata.xp
    );
    msg!(
        "   Faction hashbeasts playing: {}",
        faction_state.hashbeasts_playing
    );

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
    let current_faction_war_id = ctx.accounts.faction_war_config.current_faction_war_id;
    let current_time = Clock::get()?.unix_timestamp;

    msg!(
        "🔓 [request_hashbeast_unlock] user={}, hashbeast={}, faction_war_id={}",
        ctx.accounts.user.key(),
        player_data.gameplay_hashbeast,
        current_faction_war_id
    );

    require!(
        player_data.gameplay_hashbeast != Pubkey::default(),
        ErrorCode::InvalidState
    );
    require!(
        player_data.gameplay_unlock_request_faction_war == 0,
        ErrorCode::GameplayUnlockAlreadyRequested
    );

    player_data.gameplay_unlock_request_faction_war = current_faction_war_id;

    emit!(HashBeastGameplayUnlockRequested {
        user: ctx.accounts.user.key(),
        hashbeast_mint: player_data.gameplay_hashbeast,
        requested_during_faction_war_id: current_faction_war_id,
        unlock_available_after_faction_war_id: current_faction_war_id
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

    msg!("🎮 === WITHDRAWING HASHBEAST FROM GAMEPLAY ===");
    msg!("   HashBeast mint: {}", hashbeast_mint);

    // Verify NFT is in custody PDA
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.hashbeast_asset)?;
    require!(
        nft_owner == ctx.accounts.hashbeast_custody_pda.key(),
        ErrorCode::HashBeastNotAtGuard
    );

    // Verify this is the player's gameplay hashbeast
    require!(
        player_data.gameplay_hashbeast == hashbeast_mint,
        ErrorCode::InvalidParameters
    );

    // Verify hashbeast metadata matches player
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
        !ctx.accounts.faction_war_config.is_active
            || ctx.accounts.faction_war_config.current_faction_war_id
                > player_data.gameplay_unlock_request_faction_war,
        ErrorCode::GameplayUnlockNotReady
    );
    require!(
        !player_has_pending_reward_claims(player_data),
        ErrorCode::GameplayRewardsPending
    );

    // Transfer NFT back to user
    msg!("🔓 Transferring hashbeast back to user");
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
    msg!("   Syncing gameplay progress to hashbeast...");
    hashbeast_metadata.dna = player_data.gameplay_hashbeast_dna;
    hashbeast_metadata.xp = player_data.gameplay_hashbeast_xp;
    hashbeast_metadata.multiplier = player_data.active_multiplier;

    let gen = crate::genescience::get_evolution_stage(&hashbeast_metadata.dna);
    msg!(
        "   Final stats - Mult: {}, Gen: {}, XP: {}",
        hashbeast_metadata.multiplier,
        gen,
        hashbeast_metadata.xp
    );

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

    msg!("✅ HashBeast {} withdrawn from gameplay", hashbeast_mint);
    msg!(
        "   Faction hashbeasts playing: {}",
        faction_state.hashbeasts_playing
    );

    emit!(HashBeastWithdrawnFromGameplay {
        user: ctx.accounts.user.key(),
        hashbeast_mint,
        timestamp: current_time,
    });

    Ok(())
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

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: Optional collection
    #[account(mut)]
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

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = faction_war_config.bump)]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Metaplex Core program
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

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = faction_war_config.bump)]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

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

    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (in custody)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub hashbeast_asset: UncheckedAccount<'info>,

    /// CHECK: Optional collection
    #[account(mut)]
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

    #[account(seeds = [FACTION_WAR_CONFIG_SEED], bump = faction_war_config.bump)]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}
