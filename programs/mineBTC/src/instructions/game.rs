// # Game Instructions
//
// Core game loop: 60-second rounds with slot-hash randomness, winner selection, and reward distribution.
//
// ## Game Mechanics
//
// The game operates in rounds where:
// 1. A caller starts a new round.
// 2. Players place faction-direction bets that also count toward the active rebase.
// 3. Once the round duration passes, anyone can finalize the round from its pre-scheduled
//    slot-hash entropy source.
// 4. If the scheduled slot-hash aged out before anyone finalized the round, settlement falls back
//    to the latest available slot-hash.
// 5. Exact faction+direction bettors receive the main round rewards, while other directions on the
//    winning faction can still share a consolation MineBTC pool.
//
// ## Key Functions
//
// - `start_round`: Initializes a new round.
// - `end_round`: Finalizes the round using slot-hash entropy and calculates initial rewards.
// - `end_round_faction_rewards`: Distributes MineBTC rewards to stakers and faction pools.
//
// The slot-hash system avoids reveal-timing manipulation while keeping finalization permissionless.
//

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::solana_program::sysvar::slot_hashes;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// =============================== GAME ROUND MANAGEMENT ============================
// ========================================================================================

/// Start a new round and initialize its GameSession.
pub fn int_start_round(ctx: Context<StartRound>, round_id: u64) -> Result<()> {
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    let round_duration_seconds = u64::try_from(global_state.round_duration_seconds)
        .map_err(|_| ErrorCode::InvalidParameters)?;
    require!(round_duration_seconds > 0, ErrorCode::InvalidParameters);

    require!(global_state.is_active, ErrorCode::InvalidParameters);
    require!(global_state.can_begin_round, ErrorCode::CannotBeginRound);

    let expected_round_id = global_state.current_round_id + 1;
    require!(round_id == expected_round_id, ErrorCode::InvalidRound);

    global_state.current_round_id = round_id;

    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = round_id;
    game_session.round_start_slot = clock.slot;
    game_session.round_start_timestamp = clock.unix_timestamp;
    game_session.round_end_timestamp = clock
        .unix_timestamp
        .checked_add(global_state.round_duration_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.scheduled_entropy_slot = clock
        .slot
        .checked_add(
            round_duration_seconds
                .checked_mul(ROUND_ENTROPY_SLOTS_PER_SECOND_ESTIMATE)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
        .and_then(|slot| slot.checked_add(ROUND_PRIMARY_ENTROPY_DELAY_SLOTS))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.entropy_slot_used = 0;
    game_session.entropy_hash = [0u8; 32];
    game_session.used_entropy_fallback = false;
    game_session.stage = 0;
    game_session.total_sol_bets = 0;
    game_session.total_points_bets = 0;
    game_session.total_wgtd_points_bets = 0;
    game_session.stakers_fee = 0;
    game_session.user_faction_indexes = [0u64; NUM_FACTIONS];
    game_session.sol_bets_by_faction = [0u64; NUM_FACTIONS];
    game_session.points_bets_by_faction_direction =
        [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    game_session.wgtd_points_bets_by_faction_direction =
        [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    game_session.winning_faction_id = 0;
    game_session.winning_direction = PredictionDirection::Neutral.as_index() as u8;
    game_session.minebtc_winner_pool = 0;
    game_session.minebtc_same_faction_direction_pools = [0u64; PredictionDirection::COUNT];
    game_session.faction_stakers = 0;
    game_session.motherlode_rewards = 0;
    game_session.sol_rewards_index = 0;
    game_session.minebtc_rewards_index = 0;
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;
    game_session.highest_sol_bet_per_faction = [0u64; NUM_FACTIONS];
    game_session.mutations_per_faction = [0u8; NUM_FACTIONS];
    game_session.total_mutations_this_round = 0;
    global_state.can_begin_round = false;

    emit!(RoundStarted {
        round_id,
        game_session: game_session.key(),
        rebase_id: ctx.accounts.rebase_config.current_rebase_id,
        round_start_slot: game_session.round_start_slot,
        round_start_timestamp: game_session.round_start_timestamp,
        round_end_timestamp: game_session.round_end_timestamp,
        scheduled_entropy_slot: game_session.scheduled_entropy_slot,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

fn slot_hash_entry_count(data: &[u8]) -> Result<usize> {
    let length_bytes: [u8; 8] = data
        .get(..8)
        .ok_or(ErrorCode::InvalidAccount)?
        .try_into()
        .map_err(|_| ErrorCode::InvalidAccount)?;
    Ok(u64::from_le_bytes(length_bytes) as usize)
}

fn read_slot_hash_entry(data: &[u8], index: usize) -> Result<(u64, [u8; 32])> {
    let offset = 8 + (index * 40);
    let slot_bytes: [u8; 8] = data
        .get(offset..offset + 8)
        .ok_or(ErrorCode::InvalidAccount)?
        .try_into()
        .map_err(|_| ErrorCode::InvalidAccount)?;
    let hash_bytes: [u8; 32] = data
        .get(offset + 8..offset + 40)
        .ok_or(ErrorCode::InvalidAccount)?
        .try_into()
        .map_err(|_| ErrorCode::InvalidAccount)?;
    Ok((u64::from_le_bytes(slot_bytes), hash_bytes))
}

fn resolve_round_entropy(
    slot_hashes_account: &AccountInfo<'_>,
    scheduled_entropy_slot: u64,
) -> Result<(u64, [u8; 32], bool)> {
    require!(
        slot_hashes_account.key() == slot_hashes::id(),
        ErrorCode::InvalidAccount
    );

    let data = slot_hashes_account.try_borrow_data()?;
    let entry_count = slot_hash_entry_count(&data)?;
    require!(entry_count > 0, ErrorCode::InvalidAccount);

    let mut latest: Option<(u64, [u8; 32])> = None;
    for index in 0..entry_count {
        let (slot, hash) = read_slot_hash_entry(&data, index)?;
        if index == 0 {
            latest = Some((slot, hash));
        }
        if slot == scheduled_entropy_slot {
            return Ok((slot, hash, false));
        }
    }

    latest
        .map(|(slot, hash)| (slot, hash, true))
        .ok_or(ErrorCode::InvalidAccount.into())
}

/// Finalize the current round using its pre-scheduled slot-hash entropy.
/// If the scheduled slot hash aged out of the sysvar, fall back to the latest available slot hash.
pub fn int_end_round(ctx: Context<EndRound>) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let global_state = &mut ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    let active_faction_count = global_config.supported_factions.len();

    require!(
        active_faction_count > 0 && active_faction_count <= NUM_FACTIONS,
        ErrorCode::InvalidFactionId
    );

    if game_session.stage == 1 || game_session.stage == 2 {
        return Ok(());
    }

    require!(
        clock.unix_timestamp >= game_session.round_end_timestamp,
        ErrorCode::RoundNotEnded
    );
    require!(game_session.stage == 0, ErrorCode::InvalidStage);

    require!(
        clock.slot > game_session.scheduled_entropy_slot,
        ErrorCode::RoundEntropyNotReady
    );

    let (entropy_slot_used, entropy_hash, used_entropy_fallback) = resolve_round_entropy(
        &ctx.accounts.slot_hashes.to_account_info(),
        game_session.scheduled_entropy_slot,
    )?;
    game_session.entropy_slot_used = entropy_slot_used;
    game_session.entropy_hash = entropy_hash;
    game_session.used_entropy_fallback = used_entropy_fallback;
    msg!(
        "🎲 [end_round] round_id={} scheduled_entropy_slot={} entropy_slot_used={} fallback={}",
        game_session.round_id,
        game_session.scheduled_entropy_slot,
        game_session.entropy_slot_used,
        game_session.used_entropy_fallback
    );

    let final_hash_bytes = keccak::hashv(&[
        &game_session.entropy_hash,
        &game_session.round_id.to_le_bytes(),
        &game_session.total_sol_bets.to_le_bytes(),
        &game_session.total_wgtd_points_bets.to_le_bytes(),
    ])
    .to_bytes();

    let total_users: u64 = game_session.user_faction_indexes[..active_faction_count]
        .iter()
        .sum();
    let winning_faction_id = if total_users == 0 {
        (u64::from_le_bytes([
            final_hash_bytes[0],
            final_hash_bytes[1],
            final_hash_bytes[2],
            final_hash_bytes[3],
            0,
            0,
            0,
            0,
        ]) % active_faction_count as u64) as u8
    } else {
        let faction_random_seed = u64::from_le_bytes([
            final_hash_bytes[0],
            final_hash_bytes[1],
            final_hash_bytes[2],
            final_hash_bytes[3],
            0,
            0,
            0,
            0,
        ]);
        find_valid_winning_faction(
            faction_random_seed,
            &game_session.user_faction_indexes,
            active_faction_count,
        )?
    };

    game_session.winning_faction_id = winning_faction_id;
    let initial_direction = (u64::from_le_bytes([
        final_hash_bytes[4],
        final_hash_bytes[5],
        final_hash_bytes[6],
        final_hash_bytes[7],
        0,
        0,
        0,
        0,
    ]) % PredictionDirection::COUNT as u64) as u8;
    let winning_direction = if total_users == 0 {
        initial_direction
    } else {
        find_valid_winning_direction(
            u64::from_le_bytes([
                final_hash_bytes[4],
                final_hash_bytes[5],
                final_hash_bytes[6],
                final_hash_bytes[7],
                0,
                0,
                0,
                0,
            ]),
            &game_session.points_bets_by_faction_direction[winning_faction_id as usize],
        )?
    };
    game_session.winning_direction = winning_direction;

    if total_users == 0 {
        global_state.last_round_id = game_session.round_id;
        global_state.winning_faction_id = winning_faction_id;
        global_state.can_begin_round = true;
        game_session.stage = 2;

        emit_round_ended(
            game_session,
            game_session.key(),
            winning_faction_id,
            winning_direction,
            game_session.entropy_slot_used,
            game_session.used_entropy_fallback,
            0,
            0,
            false,
            clock.unix_timestamp,
        );

        return Ok(());
    }

    let minebtc_rewards = ctx.accounts.mine_btc_mining.mine_btc_per_round;
    let (
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        motherlode_rewards,
    ) = calculate_minebtc_split(
        minebtc_rewards,
        global_config.minebtc_dist_config.minebtc_stakers_pct,
        global_config.minebtc_dist_config.minebtc_winners_pct,
        global_config.minebtc_dist_config.minebtc_same_faction_pct,
        global_config.minebtc_dist_config.minebtc_motherlode_pct,
    );

    let winning_points = game_session.points_bets_by_faction_direction[winning_faction_id as usize]
        [winning_direction as usize];
    let winning_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];
    let mut same_faction_direction_pools = [0u64; PredictionDirection::COUNT];
    let mut same_faction_total = 0u64;

    for direction_idx in 0..PredictionDirection::COUNT {
        if direction_idx == winning_direction as usize {
            continue;
        }

        let losing_direction_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
            [winning_faction_id as usize][direction_idx];
        if losing_direction_wgtd_points > 0 && same_faction_direction_rewards_each > 0 {
            same_faction_direction_pools[direction_idx] = same_faction_direction_rewards_each;
            same_faction_total = same_faction_total
                .checked_add(same_faction_direction_rewards_each)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    game_session.minebtc_winner_pool = winning_direction_rewards;
    game_session.minebtc_same_faction_direction_pools = same_faction_direction_pools;
    game_session.faction_stakers = faction_stakers;
    game_session.motherlode_rewards = motherlode_rewards;

    let total_distributed_this_round = game_session
        .minebtc_winner_pool
        .checked_add(same_faction_total)
        .and_then(|total| total.checked_add(faction_stakers))
        .and_then(|total| total.checked_add(motherlode_rewards))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    ctx.accounts.mine_btc_mining.total_tokens_mined = ctx
        .accounts
        .mine_btc_mining
        .total_tokens_mined
        .checked_add(total_distributed_this_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if winning_points > 0 {
        game_session.sol_rewards_index = game_session
            .sol_rewards_index
            .checked_add(helper::mul_div(
                game_session.total_sol_bets,
                INDEX_PRECISION,
                winning_points,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if winning_wgtd_points > 0 {
        game_session.minebtc_rewards_index = game_session
            .minebtc_rewards_index
            .checked_add(helper::mul_div(
                game_session.minebtc_winner_pool,
                INDEX_PRECISION,
                winning_wgtd_points,
            )?)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    let motherlode_random = u64::from_le_bytes([
        final_hash_bytes[8],
        final_hash_bytes[9],
        final_hash_bytes[10],
        final_hash_bytes[11],
        0,
        0,
        0,
        0,
    ]) % MOTHERLODE_CHANCE;
    game_session.motherlode_hit = motherlode_random == 0;
    game_session.stage = 1;

    emit_round_ended(
        game_session,
        game_session.key(),
        winning_faction_id,
        winning_direction,
        game_session.entropy_slot_used,
        game_session.used_entropy_fallback,
        faction_stakers,
        motherlode_rewards,
        game_session.motherlode_hit,
        clock.unix_timestamp,
    );

    Ok(())
}

#[inline(never)]
fn emit_round_ended(
    game_session: &GameSession,
    game_session_key: Pubkey,
    winning_faction_id: u8,
    winning_direction: u8,
    entropy_slot_used: u64,
    used_entropy_fallback: bool,
    minebtc_faction_stakers: u64,
    minebtc_motherlode: u64,
    motherlode_hit: bool,
    timestamp: i64,
) {
    emit!(RoundEnded {
        round_id: game_session.round_id,
        game_session: game_session_key,
        winning_faction_id,
        winning_direction,
        entropy_slot_used,
        used_entropy_fallback,
        total_sol_bets: game_session.total_sol_bets,
        total_points_bets: game_session.total_points_bets,
        user_bets_count: game_session.user_faction_indexes,
        faction_sol_bets: game_session.sol_bets_by_faction,
        minebtc_winner_pool: game_session.minebtc_winner_pool,
        minebtc_same_faction_direction_pools: game_session.minebtc_same_faction_direction_pools,
        minebtc_faction_stakers,
        minebtc_motherlode,
        motherlode_hit,
        timestamp,
    });
}

/// Find a valid winning faction that has at least one bettor.
/// Starts from the initial faction and decrements until finding a faction with active users.
fn find_valid_winning_faction(
    random_seed: u64,
    user_faction_indexes: &[u64; NUM_FACTIONS],
    active_faction_count: usize,
) -> Result<u8> {
    let mut active_factions = [0u8; NUM_FACTIONS];
    let mut active_count = 0usize;

    for faction_id in 0..active_faction_count {
        if user_faction_indexes[faction_id] > 0 {
            active_factions[active_count] = faction_id as u8;
            active_count += 1;
        }
    }

    require!(active_count > 0, ErrorCode::InvalidParameters);

    Ok(active_factions[(random_seed % active_count as u64) as usize])
}

fn find_valid_winning_direction(
    random_seed: u64,
    direction_points: &[u64; PredictionDirection::COUNT],
) -> Result<u8> {
    let mut active_directions = [0u8; PredictionDirection::COUNT];
    let mut active_count = 0usize;

    for direction in 0..PredictionDirection::COUNT {
        if direction_points[direction] > 0 {
            active_directions[active_count] = direction as u8;
            active_count += 1;
        }
    }

    require!(active_count > 0, ErrorCode::InvalidParameters);

    Ok(active_directions[(random_seed % active_count as u64) as usize])
}

fn calculate_minebtc_split(
    minebtc_rewards: u64,
    minebtc_stakers_pct: u8,
    minebtc_winners_pct: u8,
    minebtc_same_faction_pct: u8,
    minebtc_motherlode_pct: u8,
) -> (u64, u64, u64, u64) {
    let winning_direction_rewards =
        (minebtc_rewards as u128 * minebtc_winners_pct as u128 / 100) as u64;
    let same_faction_direction_rewards_each =
        (minebtc_rewards as u128 * minebtc_same_faction_pct as u128 / 100) as u64;
    let faction_stakers = (minebtc_rewards as u128 * minebtc_stakers_pct as u128 / 100) as u64;
    let motherlode_rewards =
        (minebtc_rewards as u128 * minebtc_motherlode_pct as u128 / 100) as u64;
    (
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        motherlode_rewards,
    )
}

fn split_staker_lane_rewards(
    total_rewards: u64,
    dogebtc_active: bool,
    lp_active: bool,
) -> (u64, u64) {
    match (dogebtc_active, lp_active) {
        (true, true) => {
            let dogebtc_share = total_rewards / 2;
            (dogebtc_share, total_rewards - dogebtc_share)
        }
        (true, false) => (total_rewards, 0),
        (false, true) => (0, total_rewards),
        (false, false) => (0, 0),
    }
}

/// Finalize the faction-level reward distribution for the round.
/// This function:
/// 1. Uses the already-finalized winning faction/direction from `end_round`
/// 2. Distributes the winning faction's staker and motherlode rewards
/// 3. Advances rebase accounting when the current rebase window has ended
pub fn int_end_round_faction_rewards(ctx: Context<EndRoundFactionRewards>) -> Result<()> {
    msg!("🏁 [end_round_faction_rewards] Ending current round");

    let game_session = &mut ctx.accounts.game_session;
    let faction_state = &mut ctx.accounts.faction_state;
    let global_state = &mut ctx.accounts.global_game_state;

    if game_session.stage == 0 || game_session.stage == 2 {
        msg!("   Round has not ended yet or already distributed faction rewards, skipping");
        return Ok(());
    }
    // Validate round has ended
    require!(game_session.stage == 1, ErrorCode::InvalidStage);

    // Get winning faction from the round result
    let winning_faction_id = game_session.winning_faction_id;
    require!(
        faction_state.faction_id == winning_faction_id,
        ErrorCode::InvalidFactionId
    );

    // dogeBTC rewards to be distributed among stakers (50% to dogeBTC stakers, 50% to LP stakers)
    let minebtc_staker_rewards = game_session.faction_stakers;
    // SOL rewards to be distributed among stakers (50% to dogeBTC stakers, 50% to LP stakers)
    let sol_staker_fees = game_session.stakers_fee;
    msg!(
        "   Dbtc staker rewards: {} SOL. Sol staker fees: {} SOL",
        (minebtc_staker_rewards as f64 / 1_000_000.0),
        (sol_staker_fees as f64 / 1_000_000_000.0)
    );

    let winning_direction = game_session.winning_direction;
    let exact_winning_wgtd_pts = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];
    let dogebtc_active = faction_state.total_dogebtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;

    if dogebtc_active || lp_active {
        // dBTC + SOL distribution to staking lanes of the winning faction.
        distribute_rewards_amg_stakers(
            minebtc_staker_rewards,
            sol_staker_fees,
            faction_state,
            game_session.round_id,
        )?;
    } else {
        msg!(
            "   Winning faction {} has no active stakers. Redirecting {} MINEBTC and {} SOL staker rewards to exact winners",
            faction_state.faction_id,
            minebtc_staker_rewards as f64 / 1_000_000.0,
            sol_staker_fees as f64 / 1_000_000_000.0
        );

        let winning_points = game_session.points_bets_by_faction_direction
            [winning_faction_id as usize][winning_direction as usize];
        if sol_staker_fees > 0 && winning_points > 0 {
            let sol_reward_delta =
                helper::mul_div(sol_staker_fees, INDEX_PRECISION, winning_points)?;
            game_session.sol_rewards_index = game_session
                .sol_rewards_index
                .checked_add(sol_reward_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        if minebtc_staker_rewards > 0 && exact_winning_wgtd_pts > 0 {
            let minebtc_reward_delta = helper::mul_div(
                minebtc_staker_rewards,
                INDEX_PRECISION,
                exact_winning_wgtd_pts,
            )?;
            game_session.minebtc_rewards_index = game_session
                .minebtc_rewards_index
                .checked_add(minebtc_reward_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    // Increment motherlode pot size (always, regardless of hit)
    faction_state.motherlode_pot_size = faction_state
        .motherlode_pot_size
        .checked_add(game_session.motherlode_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   Motherlode pot: {} -> {} (+{})",
        (faction_state.motherlode_pot_size - game_session.motherlode_rewards) as f64 / 1_000_000.0,
        faction_state.motherlode_pot_size as f64 / 1_000_000.0,
        game_session.motherlode_rewards as f64 / 1_000_000.0
    );
    let motherlode_hit = game_session.motherlode_hit;

    if motherlode_hit && faction_state.motherlode_pot_size > 0 {
        let motherlode_bonus = faction_state.motherlode_pot_size;
        faction_state.motherlode_pot_size = 0;
        game_session.minebtc_winner_pool = game_session
            .minebtc_winner_pool
            .checked_add(motherlode_bonus)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        game_session.motherlode_pot_size_on_hit = motherlode_bonus;

        if motherlode_bonus > 0 && exact_winning_wgtd_pts > 0 {
            let minebtc_rewards_delta =
                helper::mul_div(motherlode_bonus, INDEX_PRECISION, exact_winning_wgtd_pts)?;
            game_session.minebtc_rewards_index = game_session
                .minebtc_rewards_index
                .checked_add(minebtc_rewards_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }

        emit!(MotherlodeHit {
            round_id: game_session.round_id,
            faction_id: faction_state.faction_id,
            winning_direction,
            winning_faction_rewards: motherlode_bonus,
            minebtc_rewards_index: game_session.minebtc_rewards_index,
        });
    }

    // Update global state with previous round results
    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;

    game_session.stage = 2;

    // Can start new round now
    global_state.can_begin_round = true;
    msg!("   Can begin new round: {}", global_state.can_begin_round);

    // --- REBASE MINING TRACKING (inline) ---
    // Only count dogeBTC that was actually distributed this round (not the full emission).
    // Empty rounds or rounds with no bets on certain directions may distribute less.
    let rebase_config = &mut ctx.accounts.rebase_config;
    let rebase_state = &mut ctx.accounts.rebase_state;
    let actually_distributed = game_session
        .minebtc_winner_pool
        .saturating_add(
            game_session
                .minebtc_same_faction_direction_pools
                .iter()
                .sum::<u64>(),
        )
        .saturating_add(game_session.faction_stakers)
        .saturating_add(game_session.motherlode_rewards);

    // If settle_rebase fired mid-round (LP burn landed during this round's
    // play window), rebase_config.current_rebase_id advanced but the new
    // rebase_state was never initialized by a bet. The Accounts struct uses
    // init_if_needed so we got an empty account — stamp its bump + rebase_id
    // and align rebase_settle_cycle the same way the first bet would (see
    // internal_process_bets init branch). After this, the next join_bets on
    // the new rebase enters the existing populate branch (rebase_id != 0).
    if rebase_state.rebase_id == 0 {
        // Mirror the init branch of internal_process_bets so a subsequent
        // join_bets enters the populate/validate branch (rebase_id != 0).
        let global_config = &ctx.accounts.global_config;
        let active_faction_count = global_config.supported_factions.len() as u8;
        let start_ranks = rebase_config.prev_rebase_mutation_ranks;

        rebase_state.bump = ctx.bumps.rebase_state;
        rebase_state.rebase_id = rebase_config.current_rebase_id;
        rebase_state.start_timestamp =
            Clock::get()?.unix_timestamp.max(0) as u64;
        rebase_state.stage = 0;
        rebase_state.active_faction_count = active_faction_count;
        rebase_state.start_ranks = start_ranks;
        rebase_state.final_ranks = start_ranks;
        rebase_state.resolved_directions =
            [PredictionDirection::Neutral.as_index() as u8; NUM_FACTIONS];

        let lp_ops = ctx.accounts.mine_btc_mining.pol_stats.lp_operations_count;
        rebase_config.rebase_settle_cycle = lp_ops
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   🧱 Initialized empty rebase_state for rebase {} (active_factions={}, settle after LP cycle #{})",
            rebase_state.rebase_id,
            active_faction_count,
            rebase_config.rebase_settle_cycle
        );
        // Nothing further to track — this is an empty/seed state. The next
        // bet will fill mutation scores, direction totals, etc.
    } else if rebase_config.is_active && rebase_state.stage == 0 {
        rebase_state.total_dogebtc_mined_in_rebase = rebase_state
            .total_dogebtc_mined_in_rebase
            .checked_add(actually_distributed)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Auto-settle rebase when the economy cycle's LP burn has completed.
        let lp_ops = ctx.accounts.mine_btc_mining.pol_stats.lp_operations_count;
        if lp_ops >= rebase_config.rebase_settle_cycle && rebase_config.rebase_settle_cycle > 0 {
            crate::instructions::rebase::finalize_rebase_settlement(rebase_config, rebase_state)?;

            emit!(RebaseAutoSettled {
                rebase_id: rebase_state.rebase_id,
                mining_pool: rebase_state.rebase_mining_pool,
            });
        }
    }

    emit!(RewardsDistributedForRound {
        round_id: game_session.round_id,
    });

    Ok(())
}

/// Internal function, called by int_end_round_faction_rewards to distribute rewards among AMG stakers (50% to dogeBTC stakers, 50% to LP stakers)
fn distribute_rewards_amg_stakers(
    minebtc_staker_rewards: u64,
    sol_staker_fees: u64,
    faction_state: &mut FactionState,
    round_id: u64,
) -> Result<()> {
    let dogebtc_active = faction_state.total_dogebtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;

    let (dogebtc_minebtc_share, lp_minebtc_share) =
        split_staker_lane_rewards(minebtc_staker_rewards, dogebtc_active, lp_active);
    let (dogebtc_sol_share, lp_sol_share) =
        split_staker_lane_rewards(sol_staker_fees, dogebtc_active, lp_active);

    if dogebtc_active && (dogebtc_minebtc_share > 0 || dogebtc_sol_share > 0) {
        let minebtc_per_share = helper::mul_div(
            dogebtc_minebtc_share,
            INDEX_PRECISION,
            faction_state.total_dogebtc_hashpower,
        )?;
        faction_state.dogebtc_dogebtc_reward_index = faction_state
            .dogebtc_dogebtc_reward_index
            .checked_add(minebtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Faction stakers MINEBTC reward index: {} -> {} (+{})",
            faction_state.dogebtc_dogebtc_reward_index - minebtc_per_share,
            faction_state.dogebtc_dogebtc_reward_index,
            minebtc_per_share
        );

        let sol_reward_inc = helper::mul_div(
            dogebtc_sol_share,
            INDEX_PRECISION,
            faction_state.total_dogebtc_hashpower,
        )?;
        faction_state.dogebtc_sol_reward_index = faction_state
            .dogebtc_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Faction stakers SOL reward index: {} -> {} (+{})",
            faction_state.dogebtc_sol_reward_index - sol_reward_inc,
            faction_state.dogebtc_sol_reward_index,
            sol_reward_inc
        );

        emit!(DogeBtcStakingRewardsDistributed {
            round_id: round_id,
            faction_id: faction_state.faction_id,
            minebtc_staker_rewards: dogebtc_minebtc_share,
            sol_staker_rewards: dogebtc_sol_share,
            dogebtc_dogebtc_reward_index: faction_state.dogebtc_dogebtc_reward_index,
            dogebtc_sol_reward_index: faction_state.dogebtc_sol_reward_index
        });
    }

    if lp_active && (lp_minebtc_share > 0 || lp_sol_share > 0) {
        let minebtc_per_share = helper::mul_div(
            lp_minebtc_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_dogebtc_reward_index = faction_state
            .lp_dogebtc_reward_index
            .checked_add(minebtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Faction stakers MINEBTC reward index: {} -> {} (+{})",
            faction_state.lp_dogebtc_reward_index - minebtc_per_share,
            faction_state.lp_dogebtc_reward_index,
            minebtc_per_share
        );

        let sol_reward_inc = helper::mul_div(
            lp_sol_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_sol_reward_index = faction_state
            .lp_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   Faction stakers SOL reward index: {} -> {} (+{})",
            faction_state.lp_sol_reward_index - sol_reward_inc,
            faction_state.lp_sol_reward_index,
            sol_reward_inc
        );

        emit!(LpStakingRewardsDistributed {
            round_id: round_id,
            faction_id: faction_state.faction_id,
            minebtc_staker_rewards: lp_minebtc_share,
            sol_staker_rewards: lp_sol_share,
            lp_dogebtc_reward_index: faction_state.lp_dogebtc_reward_index,
            lp_sol_reward_index: faction_state.lp_sol_reward_index
        });
    }

    Ok(())
}

// ========================================================================================
// =============================== ACCOUNT CONTEXTS ======================================
// ========================================================================================

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct StartRound<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    #[account(
        init,
        payer = authority,
        space = GameSession::LEN,
        seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(
        seeds = [REBASE_CONFIG_SEED],
        bump = rebase_config.bump,
    )]
    pub rebase_config: Box<Account<'info, RebaseConfig>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EndRound<'info> {
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump
    )]
    pub mine_btc_mining: Box<Account<'info, MineBtcMining>>,

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

    /// CHECK: Recent slot hashes sysvar used for round entropy
    #[account(address = slot_hashes::id() @ ErrorCode::InvalidAccount)]
    pub slot_hashes: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EndRoundFactionRewards<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    /// Read-only: used to seed `active_faction_count` on a freshly-
    /// init'd rebase_state so the next join_bets' faction_id bounds check
    /// passes (otherwise active_faction_count defaults to 0 and every
    /// faction_id reverts with InvalidFactionId).
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump
    )]
    pub mine_btc_mining: Box<Account<'info, MineBtcMining>>,

    /// Winning faction state (for updating staker rewards and motherlode)
    /// CHECK: Validated manually that faction_id matches winning_faction_id
    #[account(mut)]
    pub faction_state: Box<Account<'info, FactionState>>,

    /// CHECK: SOL rewards vault for stakers (PDA)
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_rewards_vault: UncheckedAccount<'info>,

    /// Epoch config (mut for auto-settle + auto-start)
    #[account(
        mut,
        seeds = [REBASE_CONFIG_SEED],
        bump = rebase_config.bump,
    )]
    pub rebase_config: Box<Account<'info, RebaseConfig>>,

    /// Rebase state for current rebase (mut for mining tracking + settlement).
    /// `init_if_needed` so a settle_rebase that fires between end_round and
    /// end_round_faction_rewards (e.g. economy-cycle-loop LP burn landing mid-round)
    /// doesn't permanently brick can_begin_round: without this, current_rebase_id
    /// advances but no bet has yet initialized the new rebase_state, and every
    /// subsequent end_round_faction_rewards reverts with AccountNotInitialized,
    /// leaving the game stuck at stage=1 forever. init_if_needed lets this
    /// instruction create an empty rebase_state if needed; the first bet of the
    /// new cycle will go through the same PDA and fill in real state.
    #[account(
        init_if_needed,
        payer = authority,
        space = RebaseState::LEN,
        seeds = [REBASE_STATE_SEED, &rebase_config.current_rebase_id.to_le_bytes()],
        bump,
    )]
    pub rebase_state: Box<Account<'info, RebaseState>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
