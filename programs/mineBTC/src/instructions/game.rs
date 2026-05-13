// # Arena Cycle Instructions
//
// Core arena loop: 60-second cycles with slot-hash randomness, winner selection, and reward distribution.
// SOL contributions from faction-direction predictions fuel the compute budget for on-chain content
// generation and the self-improving game economy.
//
// ## Arena Mechanics
//
// The arena operates in cycles where:
// 1. A cranker starts a new arena cycle.
// 2. Players submit faction-direction predictions that also count toward the active faction_war.
// 3. Once the cycle duration passes, anyone can finalize the cycle from its pre-scheduled
//    slot-hash entropy source.
// 4. If the scheduled slot-hash aged out before anyone finalized the cycle, settlement falls back
//    to the latest available slot-hash.
// 5. Exact faction+direction predictors receive the main cycle rewards, while other directions on the
//    winning faction can still share a consolation degenBTC pool.
//
// ## Key Functions
//
// - `start_round`: Initializes a new arena cycle.
// - `end_round`: Finalizes the cycle using slot-hash entropy and calculates initial rewards.
// - `settle_round`: Distributes degenBTC rewards to stakers and faction pools.
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
    crate::log_fn!("game", "int_start_round");
    msg!("🎲 [game.int_start_round] round_id={}", round_id);
    require!(!ctx.accounts.global_config.is_paused, ErrorCode::GamePaused);

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

    let session_bump = ctx.bumps.game_session;
    game_session.bump = session_bump;
    game_session.round_id = round_id;
    game_session.round_start_slot = clock.slot;
    game_session.round_start_timestamp = clock.unix_timestamp;

    let round_end_timestamp = clock
        .unix_timestamp
        .checked_add(global_state.round_duration_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.round_end_timestamp = round_end_timestamp;

    let entropy_delay_slots = round_duration_seconds
        .checked_mul(ROUND_ENTROPY_SLOTS_PER_SECOND_ESTIMATE)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let scheduled_entropy_slot = clock
        .slot
        .checked_add(entropy_delay_slots)
        .and_then(|slot| slot.checked_add(ROUND_PRIMARY_ENTROPY_DELAY_SLOTS))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    game_session.scheduled_entropy_slot = scheduled_entropy_slot;
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
    game_session.dbtc_winner_pool = 0;
    game_session.dbtc_same_faction_direction_pools = [0u64; PredictionDirection::COUNT];
    game_session.faction_stakers = 0;
    game_session.jackpot_rewards = 0;
    game_session.sol_rewards_index = 0;
    game_session.dbtc_rewards_index = 0;
    game_session.jackpot_hit = false;
    game_session.jackpot_faction_id = 0;
    game_session.jackpot_pot_size_on_hit = 0;
    game_session.jackpot_rewards_index = 0;
    game_session.jackpot_distributed = false;
    game_session.mutations_per_faction = [0u8; NUM_FACTIONS];
    game_session.total_mutations_this_round = 0;
    game_session.winning_faction_volume_at_round = 0;
    // Snapshot the active cycle ID at round start. Round-claim handlers use
    // this to detect a late claim (cycle has already settled) and skip the
    // mutation-bonus score-add for that case.
    game_session.faction_war_id_when_played =
        ctx.accounts.faction_war_config.current_faction_war_id;

    global_state.can_begin_round = false;

    emit!(RoundStarted {
        round_id,
        game_session: game_session.key(),
        faction_war_id: ctx.accounts.faction_war_config.current_faction_war_id,
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
    let entry_count = u64::from_le_bytes(length_bytes) as usize;
    Ok(entry_count)
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
    let slot = u64::from_le_bytes(slot_bytes);
    Ok((slot, hash_bytes))
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

    // SlotHashes is sorted newest -> oldest in 40-byte rows.
    let (latest_slot, latest_hash) = read_slot_hash_entry(&data, 0)?;
    if scheduled_entropy_slot > latest_slot {
        return err!(ErrorCode::RoundEntropyNotReady);
    }

    // Age-out short-circuit: if the scheduled slot is older than the OLDEST
    // entry in the ring buffer (~512 slots = ~3.4 min), it's gone forever.
    // Detect this in O(1) via the last entry instead of walking the whole
    // buffer to fall through to the post-loop fallback (which would also
    // burn CU on every msg!() inside the loop).
    let (oldest_slot, _) = read_slot_hash_entry(&data, entry_count - 1)?;
    if scheduled_entropy_slot < oldest_slot {
        msg!(
            "⚠️ [game.resolve_round_entropy] scheduled slot {} aged out (oldest={}, latest={}); fallback=true",
            scheduled_entropy_slot,
            oldest_slot,
            latest_slot
        );
        return Ok((latest_slot, latest_hash, true));
    }

    // Scheduled slot is in range. Walk newest -> oldest until we hit it.
    // No per-iteration msg!() — those have been killing CU on late settles.
    for index in 0..entry_count {
        let (slot, hash) = if index == 0 {
            (latest_slot, latest_hash)
        } else {
            read_slot_hash_entry(&data, index)?
        };

        if slot == scheduled_entropy_slot {
            msg!(
                "✅ [game.resolve_round_entropy] exact match at index={} slot={}",
                index,
                slot
            );
            return Ok((slot, hash, false));
        }

        if slot < scheduled_entropy_slot {
            // Skipped slot (fork) — scheduled was in range but never landed.
            msg!(
                "⚠️ [game.resolve_round_entropy] scheduled slot {} skipped (fork); fallback=true",
                scheduled_entropy_slot
            );
            return Ok((latest_slot, latest_hash, true));
        }
    }

    // Defensively unreachable given the age-out check above; treat as fallback.
    msg!(
        "⚠️ [game.resolve_round_entropy] reached end of SlotHashes without match; fallback latest={}",
        latest_slot
    );
    Ok((latest_slot, latest_hash, true))
}

/// Finalize the current round using its pre-scheduled slot-hash entropy.
/// If the scheduled slot hash aged out of the sysvar, fall back to the latest available slot hash.
pub fn int_end_round(ctx: Context<EndRound>) -> Result<()> {
    crate::log_fn!("game", "int_end_round");
    let game_session = &mut ctx.accounts.game_session;
    let global_state = &mut ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    let active_faction_count = global_config.supported_factions.len();

    if game_session.stage == 1 || game_session.stage == 2 {
        msg!("⚠️ [end_round] early return stage={}", game_session.stage);
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
        "🔍 [end_round] entropy: round={} slot={} fallback={}",
        game_session.round_id,
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
        let direction_seed = u64::from_le_bytes([
            final_hash_bytes[4],
            final_hash_bytes[5],
            final_hash_bytes[6],
            final_hash_bytes[7],
            0,
            0,
            0,
            0,
        ]);
        find_valid_winning_direction(
            direction_seed,
            &game_session.points_bets_by_faction_direction[winning_faction_id as usize],
        )?
    };
    game_session.winning_direction = winning_direction;
    msg!(
        "🏆 [end_round] winner: faction={} direction={} users={}",
        winning_faction_id,
        winning_direction,
        total_users
    );

    if total_users == 0 {
        msg!("⚠️ [end_round] empty round — short-circuit");
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
            0,
            clock.unix_timestamp,
        );

        return Ok(());
    }

    let dbtc_rewards = ctx.accounts.dbtc_mining.dbtc_per_round;
    let (
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        jackpot_rewards,
    ) = calculate_dbtc_split(
        dbtc_rewards,
        global_config.dbtc_dist_config.dbtc_stakers_pct,
        global_config.dbtc_dist_config.dbtc_winners_pct,
        global_config.dbtc_dist_config.dbtc_same_faction_pct,
        global_config.dbtc_dist_config.dbtc_jackpot_pct,
    )?;

    let winning_points = game_session.points_bets_by_faction_direction[winning_faction_id as usize]
        [winning_direction as usize];
    let winning_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];

    let mut same_faction_direction_pools = [0u64; PredictionDirection::COUNT];
    let mut same_faction_total = 0u64;

    for (direction_idx, same_faction_pool) in same_faction_direction_pools
        .iter_mut()
        .enumerate()
        .take(PredictionDirection::COUNT)
    {
        if direction_idx == winning_direction as usize {
            continue;
        }

        let losing_direction_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
            [winning_faction_id as usize][direction_idx];
        if losing_direction_wgtd_points > 0 && same_faction_direction_rewards_each > 0 {
            *same_faction_pool = same_faction_direction_rewards_each;
            same_faction_total = same_faction_total
                .checked_add(same_faction_direction_rewards_each)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    // Redirect orphaned same-faction allocations to exact-winners pool.
    let max_same_faction_capacity = same_faction_direction_rewards_each
        .checked_mul((PredictionDirection::COUNT - 1) as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let unallocated_same_faction = max_same_faction_capacity
        .checked_sub(same_faction_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let winning_direction_rewards = if unallocated_same_faction > 0 {
        msg!(
            "⚠️ [end_round] redirecting {} unallocated same-faction to winners",
            unallocated_same_faction
        );
        winning_direction_rewards
            .checked_add(unallocated_same_faction)
            .ok_or(ErrorCode::ArithmeticOverflow)?
    } else {
        winning_direction_rewards
    };

    game_session.dbtc_winner_pool = winning_direction_rewards;
    game_session.dbtc_same_faction_direction_pools = same_faction_direction_pools;
    game_session.faction_stakers = faction_stakers;
    game_session.jackpot_rewards = jackpot_rewards;

    // Accumulate this round's jackpot allocation into the global jackpot pot.
    global_state.jackpot_pot = global_state
        .jackpot_pot
        .checked_add(jackpot_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let total_distributed_this_round = game_session
        .dbtc_winner_pool
        .checked_add(same_faction_total)
        .and_then(|total| total.checked_add(faction_stakers))
        .and_then(|total| total.checked_add(jackpot_rewards))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "📊 [end_round] total_distributed={} (winner={} same_faction={} stakers={} jackpot={})",
        total_distributed_this_round,
        game_session.dbtc_winner_pool,
        same_faction_total,
        faction_stakers,
        jackpot_rewards
    );
    ctx.accounts.dbtc_mining.total_tokens_mined = ctx
        .accounts
        .dbtc_mining
        .total_tokens_mined
        .checked_add(total_distributed_this_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if winning_points > 0 {
        let sol_reward_delta =
            helper::mul_div(game_session.total_sol_bets, INDEX_PRECISION, winning_points)?;
        game_session.sol_rewards_index = game_session
            .sol_rewards_index
            .checked_add(sol_reward_delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    if winning_wgtd_points > 0 {
        let dbtc_reward_delta = helper::mul_div(
            game_session.dbtc_winner_pool,
            INDEX_PRECISION,
            winning_wgtd_points,
        )?;
        game_session.dbtc_rewards_index = game_session
            .dbtc_rewards_index
            .checked_add(dbtc_reward_delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    let jackpot_random = u64::from_le_bytes([
        final_hash_bytes[8],
        final_hash_bytes[9],
        final_hash_bytes[10],
        final_hash_bytes[11],
        0,
        0,
        0,
        0,
    ]) % JACKPOT_CHANCE;
    game_session.jackpot_hit = jackpot_random == 0;

    if game_session.jackpot_hit {
        let total_sol = game_session.total_sol_bets.max(1);
        let mut weights = [0u64; NUM_FACTIONS];
        let mut total_weight: u128 = 0;

        #[allow(clippy::needless_range_loop)]
        for i in 0..active_faction_count {
            let faction_bets = game_session.sol_bets_by_faction[i];
            let bet_share_bps = (faction_bets as u128 * BASIS_POINTS_DENOMINATOR as u128
                / total_sol as u128) as u64;
            let inverse_share_bps = BASIS_POINTS_DENOMINATOR.saturating_sub(bet_share_bps);
            let weight_bps = 5000u64 + inverse_share_bps;
            weights[i] = weight_bps.max(100);
            total_weight += weights[i] as u128;
        }

        let jackpot_faction_roll = u64::from_le_bytes([
            final_hash_bytes[12],
            final_hash_bytes[13],
            final_hash_bytes[14],
            final_hash_bytes[15],
            0,
            0,
            0,
            0,
        ]) % total_weight as u64;

        let mut cumulative: u128 = 0;
        #[allow(clippy::needless_range_loop)]
        for i in 0..active_faction_count {
            cumulative += weights[i] as u128;
            if jackpot_faction_roll < cumulative as u64 {
                game_session.jackpot_faction_id = i as u8;
                break;
            }
        }
    } else {
        game_session.jackpot_faction_id = u8::MAX; // sentinel: no jackpot this round
    }

    if !game_session.jackpot_hit && jackpot_random <= 10 {
        emit!(crate::events::JackpotNearMiss {
            round_id: game_session.round_id,
            roll: jackpot_random,
            threshold: 0,
            pot_size: global_state.jackpot_pot,
            timestamp: clock.unix_timestamp,
        });
    }

    msg!(
        "🎰 [end_round] jackpot_hit={} faction={} pot={}",
        game_session.jackpot_hit,
        game_session.jackpot_faction_id,
        global_state.jackpot_pot
    );
    game_session.stage = 1;

    emit_round_ended(
        game_session,
        game_session.key(),
        winning_faction_id,
        winning_direction,
        game_session.entropy_slot_used,
        game_session.used_entropy_fallback,
        faction_stakers,
        jackpot_rewards,
        game_session.jackpot_hit,
        game_session.jackpot_faction_id,
        clock.unix_timestamp,
    );

    msg!("✅ [end_round] round {} finalized", game_session.round_id);
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
    dbtc_stakers: u64,
    dbtc_jackpot: u64,
    jackpot_hit: bool,
    jackpot_faction_id: u8,
    timestamp: i64,
) {
    msg!(
        "📣 RoundEnded: round={} winner={}/{} jackpot_hit={} pot={}",
        game_session.round_id,
        winning_faction_id,
        winning_direction,
        jackpot_hit,
        game_session.dbtc_winner_pool
    );
    emit!(RoundEnded {
        round_id: game_session.round_id,
        game_session: game_session_key,
        winning_faction_id,
        winning_direction,
        entropy_slot_used,
        used_entropy_fallback,
        total_sol_bets: game_session.total_sol_bets,
        total_points_bets: game_session.total_points_bets,
        total_wgtd_points_bets: game_session.total_wgtd_points_bets,
        user_bets_count: game_session.user_faction_indexes,
        faction_sol_bets: game_session.sol_bets_by_faction,
        dbtc_winner_pool: game_session.dbtc_winner_pool,
        dbtc_same_faction_direction_pools: game_session.dbtc_same_faction_direction_pools,
        dbtc_stakers,
        dbtc_jackpot,
        jackpot_hit,
        jackpot_faction_id,
        // PDA-only fields surfaced via the event so off-chain indexers don't
        // need a separate getAccountInfo call. All populated by end_round.
        stakers_fee: game_session.stakers_fee,
        sol_rewards_index: game_session.sol_rewards_index,
        dbtc_rewards_index: game_session.dbtc_rewards_index,
        mutations_per_faction: game_session.mutations_per_faction,
        total_mutations_this_round: game_session.total_mutations_this_round,
        faction_war_id_when_played: game_session.faction_war_id_when_played,
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

    for (faction_id, faction_points) in user_faction_indexes
        .iter()
        .enumerate()
        .take(active_faction_count)
    {
        if *faction_points > 0 {
            active_factions[active_count] = faction_id as u8;
            active_count += 1;
        }
    }

    require!(active_count > 0, ErrorCode::InvalidParameters);

    let winner_index = (random_seed % active_count as u64) as usize;
    let winning_faction = active_factions[winner_index];
    msg!("🏆 winning_faction={} ({} active)", winning_faction, active_count);
    Ok(winning_faction)
}

fn find_valid_winning_direction(
    random_seed: u64,
    direction_points: &[u64; PredictionDirection::COUNT],
) -> Result<u8> {
    let mut active_directions = [0u8; PredictionDirection::COUNT];
    let mut active_count = 0usize;

    for (direction, points) in direction_points
        .iter()
        .enumerate()
        .take(PredictionDirection::COUNT)
    {
        if *points > 0 {
            active_directions[active_count] = direction as u8;
            active_count += 1;
        }
    }

    require!(active_count > 0, ErrorCode::InvalidParameters);

    let winner_index = (random_seed % active_count as u64) as usize;
    let winning_direction = active_directions[winner_index];
    msg!("🏆 winning_direction={} ({} active)", winning_direction, active_count);
    Ok(winning_direction)
}

fn calculate_dbtc_split(
    dbtc_rewards: u64,
    dbtc_stakers_pct: u8,
    dbtc_winners_pct: u8,
    dbtc_same_faction_pct: u8,
    dbtc_jackpot_pct: u8,
) -> Result<(u64, u64, u64, u64)> {
    let winning_direction_rewards = u64::try_from(helper::mul_div(
        dbtc_rewards,
        dbtc_winners_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let same_faction_direction_rewards_each = u64::try_from(helper::mul_div(
        dbtc_rewards,
        dbtc_same_faction_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let faction_stakers = u64::try_from(helper::mul_div(
        dbtc_rewards,
        dbtc_stakers_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let jackpot_rewards = u64::try_from(helper::mul_div(
        dbtc_rewards,
        dbtc_jackpot_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!(
        "📊 split: winners={} same_faction={} stakers={} jackpot={}",
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        jackpot_rewards
    );
    Ok((
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        jackpot_rewards,
    ))
}

fn split_staker_lane_rewards(
    total_rewards: u64,
    degenbtc_active: bool,
    lp_active: bool,
) -> (u64, u64) {
    let result = match (degenbtc_active, lp_active) {
        (true, true) => {
            let degenbtc_share = total_rewards / 2;
            let lp_share = total_rewards - degenbtc_share;
            (degenbtc_share, lp_share)
        }
        (true, false) => (total_rewards, 0),
        (false, true) => (0, total_rewards),
        (false, false) => (0, 0),
    };
    msg!("📊 staker_split: degenbtc={} lp={}", result.0, result.1);
    result
}

#[inline(never)]
fn track_faction_war_round_completion(
    faction_war_config: &mut FactionWarConfig,
    faction_war_state: &mut FactionWarState,
    game_session: &mut GameSession,
    tax_config: &mut TaxConfig,
    dbtc_mining: &DegenBtcMining,
    gameplay_tuning: &GameplayTuningConfig,
    winning_faction_id: u8,
    actually_distributed: u64,
    round_score: u64,
) -> Result<()> {
    msg!(
        "🪖 track_faction_war_round_completion: winner={} distributed={} score={} war={}",
        winning_faction_id,
        actually_distributed,
        round_score,
        faction_war_state.faction_war_id
    );

    let winning_faction_index = winning_faction_id as usize;
    if winning_faction_index < faction_war_state.active_faction_count as usize {
        faction_war_state.faction_round_wins[winning_faction_index] = faction_war_state
            .faction_round_wins[winning_faction_index]
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        if round_score > 0 {
            faction_war_state.faction_gameplay_scores[winning_faction_index] = faction_war_state
                .faction_gameplay_scores[winning_faction_index]
                .checked_add(round_score)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            emit!(crate::events::GameplayScoreAccumulated {
                faction_war_id: faction_war_state.faction_war_id,
                faction_id: winning_faction_id,
                score_source: GAMEPLAY_SCORE_SOURCE_ROUND_WIN,
                score_added: round_score,
                faction_total_score: faction_war_state.faction_gameplay_scores
                    [winning_faction_index],
                user: Pubkey::default(),
            });
        }
    }

    faction_war_state.total_degenbtc_mined_in_faction_war = faction_war_state
        .total_degenbtc_mined_in_faction_war
        .checked_add(actually_distributed)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if faction_war_config.last_processed_round_id != game_session.round_id {
        faction_war_config.last_processed_round_id = game_session.round_id;

        if winning_faction_index < faction_war_config.faction_volume_since_last_win.len() {
            let snap =
                faction_war_config.faction_volume_since_last_win[winning_faction_index];
            game_session.winning_faction_volume_at_round = snap;
            faction_war_config.faction_volume_since_last_win[winning_faction_index] = 0;
        }
    }

    let lp_ops = dbtc_mining.pol_stats.lp_operations_count;
    let should_settle = lp_ops >= faction_war_config.faction_war_settle_cycle
        && faction_war_config.faction_war_settle_cycle > 0;
    if should_settle {
        crate::instructions::faction_war::finalize_faction_war_settlement(
            faction_war_config,
            faction_war_state,
            tax_config,
            gameplay_tuning,
        )?;

        msg!(
            "🪖 FactionWarAutoSettled: war={} mining_pool={}",
            faction_war_state.faction_war_id,
            faction_war_state.faction_war_mining_pool
        );
        emit!(FactionWarAutoSettled {
            faction_war_id: faction_war_state.faction_war_id,
            mining_pool: faction_war_state.faction_war_mining_pool,
        });
    }

    Ok(())
}

#[inline(never)]
pub fn int_settle_round<'info>(
    accounts: &mut SettleRound<'info>,
    faction_war_id: u64,
) -> Result<()> {
    crate::log_fn!("game", "int_settle_round");
    msg!("🏁 [settle_round] Ending current round");

    let game_session = &mut accounts.game_session;
    let faction_state = &mut accounts.faction_state;
    let global_state = &mut accounts.global_game_state;

    if game_session.stage == 0 || game_session.stage == 2 {
        msg!("⚠️ [settle_round] early return stage={}", game_session.stage);
        return Ok(());
    }
    require!(game_session.stage == 1, ErrorCode::InvalidStage);

    // Get winning faction from the round result
    let winning_faction_id = game_session.winning_faction_id;
    require!(
        faction_state.faction_id == winning_faction_id,
        ErrorCode::InvalidFactionId
    );

    // degenBTC rewards to be distributed among stakers (50% to degenBTC stakers, 50% to LP stakers)
    let dbtc_staker_rewards = game_session.faction_stakers;
    // SOL rewards to be distributed among stakers (50% to degenBTC stakers, 50% to LP stakers)
    let sol_staker_fees = game_session.stakers_fee;

    let winning_direction = game_session.winning_direction;
    let exact_winning_wgtd_pts = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];
    let degenbtc_active = faction_state.total_degenbtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;
    if degenbtc_active || lp_active {
        distribute_rewards_amg_stakers(
            dbtc_staker_rewards,
            sol_staker_fees,
            faction_state,
            game_session.round_id,
        )?;
    } else {
        msg!(
            "⚠️ [settle_round] no stakers on faction {} — redirecting {} degenBTC + {} sol to winners",
            faction_state.faction_id,
            dbtc_staker_rewards,
            sol_staker_fees
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

        if dbtc_staker_rewards > 0 && exact_winning_wgtd_pts > 0 {
            let dbtc_reward_delta = helper::mul_div(
                dbtc_staker_rewards,
                INDEX_PRECISION,
                exact_winning_wgtd_pts,
            )?;
            game_session.dbtc_rewards_index = game_session
                .dbtc_rewards_index
                .checked_add(dbtc_reward_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }

    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;
    game_session.stage = 2;
    global_state.can_begin_round = true;
    msg!("✅ [settle_round] round={} stage=2 can_begin=true", game_session.round_id);

    // --- FACTION_WAR MINING TRACKING (inline) ---
    // Only count degenBTC that was actually distributed this round (not the full emission).
    // Empty rounds or rounds with no bets on certain directions may distribute less.
    require!(
        accounts.faction_war_config.current_faction_war_id == faction_war_id,
        ErrorCode::InvalidParameters
    );
    let same_faction_sum: u64 = game_session
        .dbtc_same_faction_direction_pools
        .iter()
        .sum::<u64>();
    let actually_distributed = game_session.dbtc_winner_pool + same_faction_sum + game_session.faction_stakers + game_session.jackpot_rewards;
    let round_id_for_event = game_session.round_id;
    let winning_faction_for_event = winning_faction_id;
    let winning_direction_for_event = game_session.winning_direction;

    if accounts.faction_war_config.is_active && accounts.faction_war_state.stage == 0 {
        let winner_idx = winning_faction_id as usize;
        let round_score: u64 = game_session.wgtd_points_bets_by_faction_direction[winner_idx]
            .iter()
            .copied()
            .try_fold(0u64, |acc, v| acc.checked_add(v))
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        track_faction_war_round_completion(
            &mut accounts.faction_war_config,
            accounts.faction_war_state.as_mut(),
            &mut accounts.game_session,
            &mut accounts.tax_config,
            &accounts.dbtc_mining,
            &accounts.global_config.gameplay_tuning,
            winning_faction_id,
            actually_distributed,
            round_score,
        )?;
    } else {
        msg!(
            "⚠️ [settle_round] faction_war tracking skipped: active={} stage={}",
            accounts.faction_war_config.is_active,
            accounts.faction_war_state.stage
        );
    }

    // By this point track_faction_war_round_completion has run (when applicable)
    // and snapshotted the winning faction's drought volume onto the GameSession.
    // Surface it on the event so the indexer can populate `latest_result`
    // without a separate PDA read.
    //
    // Read via `accounts.game_session` rather than the local `game_session`
    // binding: the latter's mutable borrow is still alive in scope, and the
    // borrow checker would otherwise reject the `track_*` reborrow above.
    // Reading through `accounts.*` here lets NLL drop the earlier borrow.
    let volume_snapshot = accounts.game_session.winning_faction_volume_at_round;
    emit!(RewardsDistributedForRound {
        round_id: round_id_for_event,
        winning_faction_id: winning_faction_for_event,
        winning_direction: winning_direction_for_event,
        winning_faction_volume_at_round: volume_snapshot,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Internal function, called by int_settle_round to distribute rewards among AMG stakers (50% to degenBTC stakers, 50% to LP stakers)
#[inline(never)]
fn distribute_rewards_amg_stakers(
    dbtc_staker_rewards: u64,
    sol_staker_fees: u64,
    faction_state: &mut FactionState,
    round_id: u64,
) -> Result<()> {
    msg!(
        "💰 distribute_rewards_amg_stakers: degenBTC={} sol={} faction={} round={}",
        dbtc_staker_rewards,
        sol_staker_fees,
        faction_state.faction_id,
        round_id
    );
    let degenbtc_active = faction_state.total_degenbtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;

    let (dbtc_dbtc_share, lp_dbtc_share) =
        split_staker_lane_rewards(dbtc_staker_rewards, degenbtc_active, lp_active);
    let (degenbtc_sol_share, lp_sol_share) =
        split_staker_lane_rewards(sol_staker_fees, degenbtc_active, lp_active);

    if degenbtc_active && (dbtc_dbtc_share > 0 || degenbtc_sol_share > 0) {
        let dbtc_per_share = helper::mul_div(
            dbtc_dbtc_share,
            INDEX_PRECISION,
            faction_state.total_degenbtc_hashpower,
        )?;
        faction_state.degenbtc_degenbtc_reward_index = faction_state
            .degenbtc_degenbtc_reward_index
            .checked_add(dbtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        let sol_reward_inc = helper::mul_div(
            degenbtc_sol_share,
            INDEX_PRECISION,
            faction_state.total_degenbtc_hashpower,
        )?;
        faction_state.degenbtc_sol_reward_index = faction_state
            .degenbtc_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        emit!(DegenBtcStakingRewardsDistributed {
            round_id,
            faction_id: faction_state.faction_id,
            dbtc_staker_rewards: dbtc_dbtc_share,
            sol_staker_rewards: degenbtc_sol_share,
            degenbtc_degenbtc_reward_index: faction_state.degenbtc_degenbtc_reward_index,
            degenbtc_sol_reward_index: faction_state.degenbtc_sol_reward_index
        });
    }

    if lp_active && (lp_dbtc_share > 0 || lp_sol_share > 0) {
        let dbtc_per_share = helper::mul_div(
            lp_dbtc_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_degenbtc_reward_index = faction_state
            .lp_degenbtc_reward_index
            .checked_add(dbtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        let sol_reward_inc = helper::mul_div(
            lp_sol_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_sol_reward_index = faction_state
            .lp_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        emit!(LpStakingRewardsDistributed {
            round_id,
            faction_id: faction_state.faction_id,
            dbtc_staker_rewards: lp_dbtc_share,
            sol_staker_rewards: lp_sol_share,
            lp_degenbtc_reward_index: faction_state.lp_degenbtc_reward_index,
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
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

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
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = faction_war_config.bump,
    )]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ════════════════════════════════════════════════════════════════════════════════════════
//  JACKPOT DISTRIBUTION
// ════════════════════════════════════════════════════════════════════════════════════════

#[inline(never)]
pub fn int_distribute_jackpot_rewards<'info>(
    accounts: &mut DistributeJackpotRewards<'info>,
    _round_id: u64,
) -> Result<()> {
    crate::log_fn!("game", "int_distribute_jackpot_rewards");
    let game_session = &mut accounts.game_session;
    let global_state = &mut accounts.global_game_state;
    let global_config = &accounts.global_config;

    if game_session.jackpot_distributed {
        return Ok(());
    }
    if !game_session.jackpot_hit {
        return Ok(());
    }
    if global_state.jackpot_pot == 0 {
        game_session.jackpot_distributed = true;
        return Ok(());
    }

    let jackpot_faction_id = game_session.jackpot_faction_id as usize;
    require!(
        jackpot_faction_id < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );

    let winning_direction = game_session.winning_direction;

    let total_jackpot_wgtd_pts: u64 = game_session
        .wgtd_points_bets_by_faction_direction[jackpot_faction_id]
        .iter()
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let jackpot_bonus = global_state.jackpot_pot;

    if total_jackpot_wgtd_pts > 0 {
        global_state.jackpot_pot = 0;
        game_session.jackpot_pot_size_on_hit = jackpot_bonus;

        let jackpot_index = helper::mul_div(jackpot_bonus, INDEX_PRECISION, total_jackpot_wgtd_pts)?;
        game_session.jackpot_rewards_index = jackpot_index as u128;
        game_session.jackpot_distributed = true;

        msg!(
            "🎰 jackpot_paid={} index={} faction={}",
            jackpot_bonus,
            jackpot_index,
            jackpot_faction_id
        );

        emit!(crate::events::JackpotHit {
            round_id: game_session.round_id,
            faction_id: jackpot_faction_id as u8,
            winning_direction,
            jackpot_pot_size_on_hit: jackpot_bonus,
            jackpot_rewards_index: game_session.jackpot_rewards_index,
        });
    } else {
        game_session.jackpot_pot_size_on_hit = 0;
        game_session.jackpot_rewards_index = 0;
        game_session.jackpot_distributed = true;

        msg!(
            "🎰 jackpot rolled over: faction={} pot={}",
            jackpot_faction_id,
            jackpot_bonus
        );

        emit!(crate::events::JackpotRolledOver {
            round_id: game_session.round_id,
            faction_id: jackpot_faction_id as u8,
            pot_size: jackpot_bonus,
            reason: 0,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct DistributeJackpotRewards<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Box<Account<'info, GameSession>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut)]
    pub authority: Signer<'info>,
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
        bump = dbtc_mining.bump
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

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
#[instruction(faction_war_id: u64)]
pub struct SettleRound<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Box<Account<'info, GlobalGameSate>>,

    /// Read-only: used to seed `active_faction_count` on a freshly-
    /// init'd faction_war_state so the next join_bets' faction_id bounds check
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
        bump = dbtc_mining.bump
    )]
    pub dbtc_mining: Box<Account<'info, DegenBtcMining>>,

    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED],
        bump = tax_config.bump
    )]
    pub tax_config: Box<Account<'info, TaxConfig>>,

    /// Winning faction state (for updating staker rewards and jackpot payout)
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

    /// Faction-war config (mut for auto-settle + auto-start)
    #[account(
        mut,
        seeds = [FACTION_WAR_CONFIG_SEED],
        bump = faction_war_config.bump,
    )]
    pub faction_war_config: Box<Account<'info, FactionWarConfig>>,

    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump = faction_war_state.bump,
    )]
    pub faction_war_state: Box<Account<'info, FactionWarState>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
