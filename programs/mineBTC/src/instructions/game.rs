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
//    winning faction can still share a consolation MineBTC pool.
//
// ## Key Functions
//
// - `start_round`: Initializes a new arena cycle.
// - `end_round`: Finalizes the cycle using slot-hash entropy and calculates initial rewards.
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
    game_session.minebtc_winner_pool = 0;
    game_session.minebtc_same_faction_direction_pools = [0u64; PredictionDirection::COUNT];
    game_session.faction_stakers = 0;
    game_session.jackpot_rewards = 0;
    game_session.sol_rewards_index = 0;
    game_session.minebtc_rewards_index = 0;
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

    msg!("🎲 [game.int_start_round] state mutation: global_state.can_begin_round false -> false");
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

    require!(
        active_faction_count > 0 && active_faction_count <= NUM_FACTIONS,
        ErrorCode::InvalidFactionId
    );

    if game_session.stage == 1 || game_session.stage == 2 {
        msg!("⚠️ [game.int_end_round] early return: stage already {} (round already ended or ending)", game_session.stage);
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
        "🔍 [game.int_end_round] entropy resolved: round_id={} scheduled_entropy_slot={} entropy_slot_used={} fallback={}",
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
    msg!(
        "📊 [game.int_end_round] total_users={} total_sol_bets={} total_points_bets={} total_weighted_points={} active_factions={}",
        total_users,
        game_session.total_sol_bets,
        game_session.total_points_bets,
        game_session.total_wgtd_points_bets,
        active_faction_count
    );

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

    msg!(
        "🏆 [game.int_end_round] state mutation: winning_faction_id = {}",
        winning_faction_id
    );
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
    msg!(
        "🏆 [game.int_end_round] state mutation: winning_direction = {}",
        winning_direction
    );
    game_session.winning_direction = winning_direction;
    msg!(
        "🏆 [game.int_end_round] winner determined: winning_faction_id={} winning_direction={} total_users={}",
        winning_faction_id,
        winning_direction,
        total_users
    );

    if total_users == 0 {
        msg!("⚠️ [game.int_end_round] branch: total_users == 0 -> short-circuit round end");
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

    let minebtc_rewards = ctx.accounts.mine_btc_mining.mine_btc_per_round;
    let (
        winning_direction_rewards,
        same_faction_direction_rewards_each,
        faction_stakers,
        jackpot_rewards,
    ) = calculate_minebtc_split(
        minebtc_rewards,
        global_config.minebtc_dist_config.minebtc_stakers_pct,
        global_config.minebtc_dist_config.minebtc_winners_pct,
        global_config.minebtc_dist_config.minebtc_same_faction_pct,
        global_config.minebtc_dist_config.minebtc_jackpot_pct,
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
            msg!("🎲 [game.int_end_round] loop progress: direction_idx={} is winning direction, skip", direction_idx);
            continue;
        }

        let losing_direction_wgtd_points = game_session.wgtd_points_bets_by_faction_direction
            [winning_faction_id as usize][direction_idx];
        msg!("🎲 [game.int_end_round] loop progress: direction_idx={} losing_direction_wgtd_points={} same_faction_direction_rewards_each={}",
            direction_idx, losing_direction_wgtd_points, same_faction_direction_rewards_each);
        if losing_direction_wgtd_points > 0 && same_faction_direction_rewards_each > 0 {
            *same_faction_pool = same_faction_direction_rewards_each;
            same_faction_total = same_faction_total
                .checked_add(same_faction_direction_rewards_each)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!("📊 [game.int_end_round] loop progress: allocated same_faction_pool={} same_faction_total now={}", same_faction_direction_rewards_each, same_faction_total);
        }
    }

    // Redirect orphaned same-faction allocations (losing directions with no
    // bettors) to the exact-winners pool. Otherwise that share of the round
    // emission would be permanently stranded in the mining vault. Winners
    // are guaranteed to exist here because `find_valid_winning_direction`
    // selects a direction with bets.
    let max_same_faction_capacity = same_faction_direction_rewards_each
        .checked_mul((PredictionDirection::COUNT - 1) as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "📊 [game.int_end_round] computation: max_same_faction_capacity = {} * {} = {}",
        same_faction_direction_rewards_each,
        PredictionDirection::COUNT - 1,
        max_same_faction_capacity
    );
    let unallocated_same_faction = max_same_faction_capacity
        .checked_sub(same_faction_total)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "📊 [game.int_end_round] computation: unallocated_same_faction = {} - {} = {}",
        max_same_faction_capacity,
        same_faction_total,
        unallocated_same_faction
    );
    let winning_direction_rewards = if unallocated_same_faction > 0 {
        msg!(
            "⚠️ [game.int_end_round] redirecting {} unallocated same-faction tokens to exact-winners pool",
            unallocated_same_faction
        );
        let redirected = winning_direction_rewards
            .checked_add(unallocated_same_faction)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "📊 [game.int_end_round] computation: winning_direction_rewards {} -> {}",
            winning_direction_rewards,
            redirected
        );
        redirected
    } else {
        msg!("🎲 [game.int_end_round] no unallocated same-faction tokens");
        winning_direction_rewards
    };

    game_session.minebtc_winner_pool = winning_direction_rewards;
    game_session.minebtc_same_faction_direction_pools = same_faction_direction_pools;
    game_session.faction_stakers = faction_stakers;
    game_session.jackpot_rewards = jackpot_rewards;

    // Accumulate this round's jackpot allocation into the global jackpot pot.
    global_state.jackpot_pot = global_state
        .jackpot_pot
        .checked_add(jackpot_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let total_distributed_this_round = game_session
        .minebtc_winner_pool
        .checked_add(same_faction_total)
        .and_then(|total| total.checked_add(faction_stakers))
        .and_then(|total| total.checked_add(jackpot_rewards))
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("📊 [game.int_end_round] computation: total_distributed_this_round = winner_pool({}) + same_faction_total({}) + faction_stakers({}) + jackpot_rewards({}) = {}",
        game_session.minebtc_winner_pool, same_faction_total, faction_stakers, jackpot_rewards, total_distributed_this_round);
    ctx.accounts.mine_btc_mining.total_tokens_mined = ctx
        .accounts
        .mine_btc_mining
        .total_tokens_mined
        .checked_add(total_distributed_this_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if winning_points > 0 {
        let sol_reward_delta =
            helper::mul_div(game_session.total_sol_bets, INDEX_PRECISION, winning_points)?;
        let old_sol_rewards_index = game_session.sol_rewards_index;
        game_session.sol_rewards_index = game_session
            .sol_rewards_index
            .checked_add(sol_reward_delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("📊 [game.int_end_round] state mutation: sol_rewards_index {} -> {} (+{}) (total_sol_bets={} / winning_points={})",
            old_sol_rewards_index, game_session.sol_rewards_index, sol_reward_delta, game_session.total_sol_bets, winning_points);
    } else {
        msg!("⚠️ [game.int_end_round] winning_points == 0, skipping sol_rewards_index update");
    }
    if winning_wgtd_points > 0 {
        let minebtc_reward_delta = helper::mul_div(
            game_session.minebtc_winner_pool,
            INDEX_PRECISION,
            winning_wgtd_points,
        )?;
        let old_minebtc_rewards_index = game_session.minebtc_rewards_index;
        game_session.minebtc_rewards_index = game_session
            .minebtc_rewards_index
            .checked_add(minebtc_reward_delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("📊 [game.int_end_round] state mutation: minebtc_rewards_index {} -> {} (+{}) (winner_pool={} / winning_wgtd_points={})",
            old_minebtc_rewards_index, game_session.minebtc_rewards_index, minebtc_reward_delta, game_session.minebtc_winner_pool, winning_wgtd_points);
    } else {
        msg!("⚠️ [game.int_end_round] winning_wgtd_points == 0, skipping minebtc_rewards_index update");
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
    msg!(
        "🎰 [game.int_end_round] computation: jackpot_random = {} (hit={})",
        jackpot_random,
        jackpot_random == 0
    );
    game_session.jackpot_hit = jackpot_random == 0;

    // If jackpot hits, select the winning faction using inverse bet-volume weighting.
    // Factions with lower SOL prediction volume receive higher weight,
    // creating underdog moments and encouraging diversification.
    if game_session.jackpot_hit {
        msg!("🎰 [game.int_end_round] branch: jackpot_hit=true -> selecting jackpot faction");
        let total_sol = game_session.total_sol_bets.max(1);
        msg!(
            "📊 [game.int_end_round] computation: total_sol = max({}, 1) = {}",
            game_session.total_sol_bets,
            total_sol
        );
        let mut weights = [0u64; NUM_FACTIONS];
        let mut total_weight: u128 = 0;

        msg!(
            "🎰 [game.int_end_round] loop: computing jackpot weights for {} factions",
            active_faction_count
        );
        #[allow(clippy::needless_range_loop)]
        for i in 0..active_faction_count {
            let faction_bets = game_session.sol_bets_by_faction[i];
            let bet_share_bps = (faction_bets as u128 * BASIS_POINTS_DENOMINATOR as u128
                / total_sol as u128) as u64;
            let inverse_share_bps = BASIS_POINTS_DENOMINATOR.saturating_sub(bet_share_bps);
            let weight_bps = 5000u64 + inverse_share_bps;
            weights[i] = weight_bps.max(100);
            total_weight += weights[i] as u128;
            msg!("🎰 [game.int_end_round] loop progress: i={} faction_bets={} bet_share_bps={} inverse_share_bps={} weight_bps={} weight={}",
                i, faction_bets, bet_share_bps, inverse_share_bps, weight_bps, weights[i]);
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
        msg!(
            "🎰 [game.int_end_round] computation: jackpot_faction_roll = {} (total_weight={})",
            jackpot_faction_roll,
            total_weight
        );

        let mut cumulative: u128 = 0;
        msg!("🎰 [game.int_end_round] loop: selecting jackpot faction by cumulative weight");
        #[allow(clippy::needless_range_loop)]
        for i in 0..active_faction_count {
            cumulative += weights[i] as u128;
            msg!(
                "🎰 [game.int_end_round] loop progress: i={} cumulative={} roll={} cmp={}",
                i,
                cumulative,
                jackpot_faction_roll,
                jackpot_faction_roll < cumulative as u64
            );
            if jackpot_faction_roll < cumulative as u64 {
                msg!(
                    "🏆 [game.int_end_round] state mutation: jackpot_faction_id = {}",
                    i as u8
                );
                game_session.jackpot_faction_id = i as u8;
                break;
            }
        }
    } else {
        msg!("🎰 [game.int_end_round] branch: jackpot_hit=false -> no jackpot faction selection");
    }

    // Near miss: within 10 closest rolls of the jackpot threshold.
    // Frontend uses this to hook users with "so close!" notifications.
    if !game_session.jackpot_hit && jackpot_random <= 10 {
        msg!(
            "🎰 [game.int_end_round] near miss: jackpot_random={} <= 10, emitting JackpotNearMiss",
            jackpot_random
        );
        emit!(crate::events::JackpotNearMiss {
            round_id: game_session.round_id,
            roll: jackpot_random,
            threshold: 0,
            pot_size: global_state.jackpot_pot,
            timestamp: clock.unix_timestamp,
        });
    }

    msg!("🎲 [game.int_end_round] state mutation: game_session.stage = 1");
    game_session.stage = 1;

    msg!("🎲 [game.int_end_round] helper call: emit_round_ended(round_id={}, winning_faction_id={}, winning_direction={}, jackpot_hit={}, jackpot_faction_id={})",
        game_session.round_id, winning_faction_id, winning_direction, game_session.jackpot_hit, game_session.jackpot_faction_id);
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

    msg!("✅ [game.int_end_round] round finalized successfully");
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
    minebtc_jackpot: u64,
    jackpot_hit: bool,
    jackpot_faction_id: u8,
    timestamp: i64,
) {
    msg!("🎲 [game.emit_round_ended] emitting RoundEnded: round_id={} game_session={} winning_faction_id={} winning_direction={} entropy_slot_used={} fallback={} minebtc_winner_pool={} minebtc_stakers={} minebtc_jackpot={} jackpot_hit={} jackpot_faction_id={} timestamp={}",
        game_session.round_id, game_session_key, winning_faction_id, winning_direction, entropy_slot_used, used_entropy_fallback,
        game_session.minebtc_winner_pool, minebtc_faction_stakers, minebtc_jackpot, jackpot_hit, jackpot_faction_id, timestamp);
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
        minebtc_jackpot,
        jackpot_hit,
        jackpot_faction_id,
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
    msg!(
        "🏆 [game.find_valid_winning_faction] random_seed={} active_faction_count={}",
        random_seed,
        active_faction_count
    );
    let mut active_factions = [0u8; NUM_FACTIONS];
    let mut active_count = 0usize;

    msg!(
        "🏆 [game.find_valid_winning_faction] loop: scanning {} factions for active bettors",
        active_faction_count
    );
    for (faction_id, faction_points) in user_faction_indexes
        .iter()
        .enumerate()
        .take(active_faction_count)
    {
        if *faction_points > 0 {
            active_factions[active_count] = faction_id as u8;
            active_count += 1;
            msg!("🏆 [game.find_valid_winning_faction] loop progress: faction_id={} has {} points -> active_factions[{}] = {}", faction_id, faction_points, active_count - 1, faction_id);
        } else {
            msg!("🏆 [game.find_valid_winning_faction] loop progress: faction_id={} has 0 points, skipping", faction_id);
        }
    }

    msg!(
        "🔍 [game.find_valid_winning_faction] require: active_count > 0: active_count={}",
        active_count
    );
    require!(active_count > 0, ErrorCode::InvalidParameters);

    let winner_index = (random_seed % active_count as u64) as usize;
    let winning_faction = active_factions[winner_index];
    msg!("🏆 [game.find_valid_winning_faction] result: active_count={} winner_index={} winning_faction={}", active_count, winner_index, winning_faction);
    Ok(winning_faction)
}

fn find_valid_winning_direction(
    random_seed: u64,
    direction_points: &[u64; PredictionDirection::COUNT],
) -> Result<u8> {
    msg!(
        "🏆 [game.find_valid_winning_direction] random_seed={} direction_points={:?}",
        random_seed,
        direction_points
    );
    let mut active_directions = [0u8; PredictionDirection::COUNT];
    let mut active_count = 0usize;

    msg!(
        "🏆 [game.find_valid_winning_direction] loop: scanning {} directions for active bets",
        PredictionDirection::COUNT
    );
    for (direction, points) in direction_points
        .iter()
        .enumerate()
        .take(PredictionDirection::COUNT)
    {
        if *points > 0 {
            active_directions[active_count] = direction as u8;
            active_count += 1;
            msg!("🏆 [game.find_valid_winning_direction] loop progress: direction={} has {} points -> active_directions[{}] = {}", direction, points, active_count - 1, direction);
        } else {
            msg!("🏆 [game.find_valid_winning_direction] loop progress: direction={} has 0 points, skipping", direction);
        }
    }

    msg!(
        "🔍 [game.find_valid_winning_direction] require: active_count > 0: active_count={}",
        active_count
    );
    require!(active_count > 0, ErrorCode::InvalidParameters);

    let winner_index = (random_seed % active_count as u64) as usize;
    let winning_direction = active_directions[winner_index];
    msg!("🏆 [game.find_valid_winning_direction] result: active_count={} winner_index={} winning_direction={}", active_count, winner_index, winning_direction);
    Ok(winning_direction)
}

fn calculate_minebtc_split(
    minebtc_rewards: u64,
    minebtc_stakers_pct: u8,
    minebtc_winners_pct: u8,
    minebtc_same_faction_pct: u8,
    minebtc_jackpot_pct: u8,
) -> Result<(u64, u64, u64, u64)> {
    msg!("📊 [game.calculate_minebtc_split] minebtc_rewards={} stakers_pct={} winners_pct={} same_faction_pct={} jackpot_pct={}",
        minebtc_rewards, minebtc_stakers_pct, minebtc_winners_pct, minebtc_same_faction_pct, minebtc_jackpot_pct);
    let winning_direction_rewards = u64::try_from(helper::mul_div(
        minebtc_rewards,
        minebtc_winners_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let same_faction_direction_rewards_each = u64::try_from(helper::mul_div(
        minebtc_rewards,
        minebtc_same_faction_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let faction_stakers = u64::try_from(helper::mul_div(
        minebtc_rewards,
        minebtc_stakers_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    let jackpot_rewards = u64::try_from(helper::mul_div(
        minebtc_rewards,
        minebtc_jackpot_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!("📊 [game.calculate_minebtc_split] result: winners={} same_faction_each={} stakers={} jackpot={}",
        winning_direction_rewards, same_faction_direction_rewards_each, faction_stakers, jackpot_rewards);
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
    msg!(
        "📊 [game.split_staker_lane_rewards] total_rewards={} degenbtc_active={} lp_active={}",
        total_rewards,
        degenbtc_active,
        lp_active
    );
    let result = match (degenbtc_active, lp_active) {
        (true, true) => {
            let degenbtc_share = total_rewards / 2;
            let lp_share = total_rewards - degenbtc_share;
            msg!("📊 [game.split_staker_lane_rewards] branch (true,true): degenbtc_share={} lp_share={}", degenbtc_share, lp_share);
            (degenbtc_share, lp_share)
        }
        (true, false) => {
            msg!("📊 [game.split_staker_lane_rewards] branch (true,false): degenbtc_share={} lp_share=0", total_rewards);
            (total_rewards, 0)
        }
        (false, true) => {
            msg!("📊 [game.split_staker_lane_rewards] branch (false,true): degenbtc_share=0 lp_share={}", total_rewards);
            (0, total_rewards)
        }
        (false, false) => {
            msg!("📊 [game.split_staker_lane_rewards] branch (false,false): degenbtc_share=0 lp_share=0");
            (0, 0)
        }
    };
    result
}

/// Finalize the faction-level reward distribution for the round.
/// This function:
/// 1. Uses the already-finalized winning faction/direction from `end_round`
/// 2. Distributes the winning faction's staker and global jackpot rewards
/// 3. Advances faction_war accounting when the current faction_war window has ended
#[inline(never)]
fn init_or_load_round_faction_war_state<'info>(
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
        "🪖 [init_or_load_round_faction_war_state] faction_war_id={} account={} created={}",
        faction_war_id,
        faction_war_state_info.key(),
        created
    );
    load_faction_war_state_boxed(faction_war_state_info)
}

#[inline(never)]
fn load_faction_war_state_boxed<'info>(
    account: &AccountInfo<'info>,
) -> Result<Box<FactionWarState>> {
    msg!(
        "🔍 [game.load_faction_war_state_boxed] account={} owner={} expected_owner={}",
        account.key(),
        account.owner,
        FactionWarState::owner()
    );
    msg!(
        "🔍 [game.load_faction_war_state_boxed] require: account.owner == FactionWarState::owner()"
    );
    require!(
        account.owner == &FactionWarState::owner(),
        ErrorCode::InvalidAccount
    );
    let data = account.try_borrow_data()?;
    msg!(
        "🔍 [game.load_faction_war_state_boxed] data.len={} DISCRIMINATOR_SIZE={}",
        data.len(),
        DISCRIMINATOR_SIZE
    );
    msg!("🔍 [game.load_faction_war_state_boxed] require: data.len >= DISCRIMINATOR_SIZE");
    require!(data.len() >= DISCRIMINATOR_SIZE, ErrorCode::InvalidAccount);
    msg!("🔍 [game.load_faction_war_state_boxed] require: discriminator match");
    require!(
        &data[..DISCRIMINATOR_SIZE] == FactionWarState::DISCRIMINATOR,
        ErrorCode::InvalidAccount
    );
    let mut boxed: Box<FactionWarState> =
        unsafe { helper::alloc_zeroed_boxed::<FactionWarState>() };
    let mut cursor: &[u8] = &data[DISCRIMINATOR_SIZE..];
    msg!(
        "🔍 [game.load_faction_war_state_boxed] deserialize_into cursor.len={}",
        cursor.len()
    );
    FactionWarState::deserialize_into(&mut boxed, &mut cursor)?;
    msg!(
        "✅ [game.load_faction_war_state_boxed] loaded faction_war_id={} stage={}",
        boxed.faction_war_id,
        boxed.stage
    );
    Ok(boxed)
}

#[inline(never)]
fn seed_empty_faction_war_from_round<'info>(
    accounts: &mut EndRoundFactionRewards<'info>,
    faction_war_state: &mut FactionWarState,
    faction_war_state_bump: u8,
) -> Result<()> {
    msg!(
        "🎲 [game.seed_empty_faction_war_from_round] bump={}",
        faction_war_state_bump
    );
    let global_config = &accounts.global_config;
    let active_faction_count = global_config.supported_factions.len() as u8;
    let start_ranks = accounts.faction_war_config.prev_faction_war_ranks;
    msg!(
        "📊 [game.seed_empty_faction_war_from_round] active_faction_count={} start_ranks={:?}",
        active_faction_count,
        start_ranks
    );

    let old_treasury_base = faction_war_state.treasury_reward_base_amount;
    let unassigned = accounts.tax_config.unassigned_faction_war_treasury_amount;
    let seeded_treasury_base = faction_war_state
        .treasury_reward_base_amount
        .checked_add(unassigned)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("📊 [game.seed_empty_faction_war_from_round] computation: seeded_treasury_base = {} + {} = {}", old_treasury_base, unassigned, seeded_treasury_base);

    msg!(
        "🎲 [game.seed_empty_faction_war_from_round] state mutation: bump={} faction_war_id={}",
        faction_war_state_bump,
        accounts.faction_war_config.current_faction_war_id
    );
    faction_war_state.bump = faction_war_state_bump;
    faction_war_state.faction_war_id = accounts.faction_war_config.current_faction_war_id;
    let now_ts = Clock::get()?.unix_timestamp.max(0) as u64;
    msg!(
        "🎲 [game.seed_empty_faction_war_from_round] state mutation: start_timestamp = {}",
        now_ts
    );
    faction_war_state.start_timestamp = now_ts;
    faction_war_state.stage = 0;
    faction_war_state.active_faction_count = active_faction_count;
    faction_war_state.total_degenbtc_mined_in_faction_war = 0;
    faction_war_state.faction_war_mining_pool = 0;
    faction_war_state.rank_deltas = [0i8; NUM_FACTIONS];
    faction_war_state.faction_direction_totals = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    faction_war_state.loyalty_direction_totals = [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
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
    faction_war_state.start_ranks = start_ranks;
    faction_war_state.final_ranks = start_ranks;
    faction_war_state.resolved_directions =
        [PredictionDirection::Neutral.as_index() as u8; NUM_FACTIONS];
    faction_war_state.treasury_reward_base_amount = seeded_treasury_base;
    faction_war_state.treasury_claimed_bitmap = 0;
    faction_war_state.sol_reward_pool = 0;
    msg!("🎲 [game.seed_empty_faction_war_from_round] state mutation: tax_config.unassigned_faction_war_treasury_amount {} -> 0", accounts.tax_config.unassigned_faction_war_treasury_amount);
    accounts.tax_config.unassigned_faction_war_treasury_amount = 0;

    let lp_ops = accounts.mine_btc_mining.pol_stats.lp_operations_count;
    let settle_cycle = lp_ops.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("📊 [game.seed_empty_faction_war_from_round] computation: faction_war_settle_cycle = {} + 1 = {}", lp_ops, settle_cycle);
    accounts.faction_war_config.faction_war_settle_cycle = settle_cycle;
    msg!("🎲 [game.seed_empty_faction_war_from_round] helper call: reset_cycle_round_tracking()");
    accounts.faction_war_config.reset_cycle_round_tracking();
    msg!(
        "✅ [game.seed_empty_faction_war_from_round] initialized empty faction-war seed state for war {} (active_factions={}, settle after LP cycle #{}, treasury_base={})",
        faction_war_state.faction_war_id,
        active_faction_count,
        accounts.faction_war_config.faction_war_settle_cycle,
        faction_war_state.treasury_reward_base_amount,
    );
    Ok(())
}

#[inline(never)]
fn track_faction_war_round_completion<'info>(
    accounts: &mut EndRoundFactionRewards<'info>,
    faction_war_state: &mut FactionWarState,
    winning_faction_id: u8,
    actually_distributed: u64,
    round_score: u64,
) -> Result<()> {
    msg!("🎲 [game.track_faction_war_round_completion] winning_faction_id={} actually_distributed={} round_score={} faction_war_id={}",
        winning_faction_id, actually_distributed, round_score, faction_war_state.faction_war_id);

    let winning_faction_index = winning_faction_id as usize;
    if winning_faction_index < faction_war_state.active_faction_count as usize {
        let old_wins = faction_war_state.faction_round_wins[winning_faction_index];
        faction_war_state.faction_round_wins[winning_faction_index] = faction_war_state
            .faction_round_wins[winning_faction_index]
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("🎲 [game.track_faction_war_round_completion] state mutation: faction_round_wins[{}] {} -> {}",
            winning_faction_index, old_wins, faction_war_state.faction_round_wins[winning_faction_index]);

        // --- Cycle leaderboard score: ROUND_WIN ---
        // Driven by the country actually winning the round. Score added equals
        // total weighted points bet on the winning country (any direction).
        // Mutation bonuses (round-claim time) accrue to the same field.
        if round_score > 0 {
            let old_score = faction_war_state.faction_gameplay_scores[winning_faction_index];
            faction_war_state.faction_gameplay_scores[winning_faction_index] = old_score
                .checked_add(round_score)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!(
                "🎲 [game.track_faction_war_round_completion] state mutation: faction_gameplay_scores[{}] {} -> {} (+{}, source=ROUND_WIN)",
                winning_faction_index,
                old_score,
                faction_war_state.faction_gameplay_scores[winning_faction_index],
                round_score
            );
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
    } else {
        msg!("⚠️ [game.track_faction_war_round_completion] winning_faction_index {} >= active_faction_count {}, skipping win increment", winning_faction_index, faction_war_state.active_faction_count);
    }

    let old_mined = faction_war_state.total_degenbtc_mined_in_faction_war;
    faction_war_state.total_degenbtc_mined_in_faction_war = faction_war_state
        .total_degenbtc_mined_in_faction_war
        .checked_add(actually_distributed)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("🎲 [game.track_faction_war_round_completion] state mutation: total_degenbtc_mined_in_faction_war {} -> {} (+{})",
        old_mined, faction_war_state.total_degenbtc_mined_in_faction_war, actually_distributed);

    msg!("🎲 [game.track_faction_war_round_completion] branch: last_processed_round_id({}) != round_id({}) ? {}",
        accounts.faction_war_config.last_processed_round_id, accounts.game_session.round_id,
        accounts.faction_war_config.last_processed_round_id != accounts.game_session.round_id);
    if accounts.faction_war_config.last_processed_round_id != accounts.game_session.round_id {
        msg!("🎲 [game.track_faction_war_round_completion] state mutation: last_processed_round_id = {}", accounts.game_session.round_id);
        accounts.faction_war_config.last_processed_round_id = accounts.game_session.round_id;

        // Snapshot the winning country's accumulated volume onto GameSession,
        // then reset the per-country counter so the next streak starts fresh.
        // Late round-claims read the snapshot, not the live counter, so the
        // mutation-roll volume is stable regardless of when claims land.
        if winning_faction_index
            < accounts
                .faction_war_config
                .faction_volume_since_last_win
                .len()
        {
            let snap =
                accounts.faction_war_config.faction_volume_since_last_win[winning_faction_index];
            accounts.game_session.winning_faction_volume_at_round = snap;
            accounts.faction_war_config.faction_volume_since_last_win[winning_faction_index] = 0;
            msg!(
                "🎲 [game.track_faction_war_round_completion] state mutation: faction_volume_since_last_win[{}] {} -> 0 (snapshotted to GameSession.winning_faction_volume_at_round)",
                winning_faction_index, snap
            );
        }
    }

    let lp_ops = accounts.mine_btc_mining.pol_stats.lp_operations_count;
    let should_settle = lp_ops >= accounts.faction_war_config.faction_war_settle_cycle
        && accounts.faction_war_config.faction_war_settle_cycle > 0;
    msg!("🎲 [game.track_faction_war_round_completion] branch: settle check lp_ops={} settle_cycle={} should_settle={}",
        lp_ops, accounts.faction_war_config.faction_war_settle_cycle, should_settle);
    if should_settle {
        msg!("🎲 [game.track_faction_war_round_completion] helper call: finalize_faction_war_settlement(faction_war_id={})", faction_war_state.faction_war_id);
        crate::instructions::faction_war::finalize_faction_war_settlement(
            &mut accounts.faction_war_config,
            faction_war_state,
            &mut accounts.tax_config,
            &accounts.global_config.gameplay_tuning,
        )?;

        msg!("🎲 [game.track_faction_war_round_completion] emit FactionWarAutoSettled: faction_war_id={} mining_pool={}",
            faction_war_state.faction_war_id, faction_war_state.faction_war_mining_pool);
        emit!(FactionWarAutoSettled {
            faction_war_id: faction_war_state.faction_war_id,
            mining_pool: faction_war_state.faction_war_mining_pool,
        });
    }

    msg!("✅ [game.track_faction_war_round_completion] done");
    Ok(())
}

#[inline(never)]
pub fn int_end_round_faction_rewards<'info>(
    accounts: &mut EndRoundFactionRewards<'info>,
    faction_war_id: u64,
    faction_war_state_bump: u8,
) -> Result<()> {
    crate::log_fn!("game", "int_end_round_faction_rewards");
    msg!("🏁 [end_round_faction_rewards] Ending current round");

    let game_session = &mut accounts.game_session;
    let faction_state = &mut accounts.faction_state;
    let global_state = &mut accounts.global_game_state;
    let faction_war_state_info = accounts.faction_war_state.to_account_info();
    let mut faction_war_state = init_or_load_round_faction_war_state(
        accounts.authority.as_ref(),
        accounts.system_program.as_ref(),
        &faction_war_state_info,
        faction_war_id,
        faction_war_state_bump,
    )?;

    if game_session.stage == 0 || game_session.stage == 2 {
        msg!("⚠️ [game.int_end_round_faction_rewards] early return: round not ended yet or already distributed faction rewards (stage={})", game_session.stage);
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
    let minebtc_staker_rewards = game_session.faction_stakers;
    // SOL rewards to be distributed among stakers (50% to degenBTC stakers, 50% to LP stakers)
    let sol_staker_fees = game_session.stakers_fee;

    let winning_direction = game_session.winning_direction;
    let exact_winning_wgtd_pts = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];
    let degenbtc_active = faction_state.total_degenbtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;
    msg!("🎲 [game.int_end_round_faction_rewards] winning_direction={} exact_winning_wgtd_pts={} degenbtc_active={} lp_active={}",
        winning_direction, exact_winning_wgtd_pts, degenbtc_active, lp_active);

    if degenbtc_active || lp_active {
        msg!("🎲 [game.int_end_round_faction_rewards] branch: stakers active -> helper call: distribute_rewards_amg_stakers(minebtc={}, sol={}, round_id={})",
            minebtc_staker_rewards, sol_staker_fees, game_session.round_id);
        distribute_rewards_amg_stakers(
            minebtc_staker_rewards,
            sol_staker_fees,
            faction_state,
            game_session.round_id,
        )?;
    } else {
        msg!(
            "⚠️ [game.int_end_round_faction_rewards] winning faction {} has no active stakers. Redirecting {} minebtc and {} sol staker rewards to exact winners",
            faction_state.faction_id,
            minebtc_staker_rewards,
            sol_staker_fees
        );

        let winning_points = game_session.points_bets_by_faction_direction
            [winning_faction_id as usize][winning_direction as usize];
        msg!(
            "📊 [game.int_end_round_faction_rewards] winning_points={}",
            winning_points
        );
        if sol_staker_fees > 0 && winning_points > 0 {
            let sol_reward_delta =
                helper::mul_div(sol_staker_fees, INDEX_PRECISION, winning_points)?;
            let old_sol_rewards_index = game_session.sol_rewards_index;
            game_session.sol_rewards_index = game_session
                .sol_rewards_index
                .checked_add(sol_reward_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!("📊 [game.int_end_round_faction_rewards] state mutation: sol_rewards_index {} -> {} (+{}) (redirected staker SOL)",
                old_sol_rewards_index, game_session.sol_rewards_index, sol_reward_delta);
        } else {
            msg!("⚠️ [game.int_end_round_faction_rewards] skip sol redirect: sol_staker_fees={} winning_points={}", sol_staker_fees, winning_points);
        }

        if minebtc_staker_rewards > 0 && exact_winning_wgtd_pts > 0 {
            let minebtc_reward_delta = helper::mul_div(
                minebtc_staker_rewards,
                INDEX_PRECISION,
                exact_winning_wgtd_pts,
            )?;
            let old_minebtc_rewards_index = game_session.minebtc_rewards_index;
            game_session.minebtc_rewards_index = game_session
                .minebtc_rewards_index
                .checked_add(minebtc_reward_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!("📊 [game.int_end_round_faction_rewards] state mutation: minebtc_rewards_index {} -> {} (+{}) (redirected staker MINEBTC)",
                old_minebtc_rewards_index, game_session.minebtc_rewards_index, minebtc_reward_delta);
        } else {
            msg!("⚠️ [game.int_end_round_faction_rewards] skip minebtc redirect: minebtc_staker_rewards={} exact_winning_wgtd_pts={}", minebtc_staker_rewards, exact_winning_wgtd_pts);
        }
    }

    // Update global state with previous round results
    msg!(
        "🎲 [game.int_end_round_faction_rewards] state mutation: global_state.last_round_id = {}",
        game_session.round_id
    );
    global_state.last_round_id = game_session.round_id;
    msg!("🎲 [game.int_end_round_faction_rewards] state mutation: global_state.winning_faction_id = {}", winning_faction_id);
    global_state.winning_faction_id = winning_faction_id;

    msg!("🎲 [game.int_end_round_faction_rewards] state mutation: game_session.stage = 2");
    game_session.stage = 2;

    // Can start new round now
    global_state.can_begin_round = true;
    msg!("✅ [game.int_end_round_faction_rewards] global_state.can_begin_round = true");

    // --- FACTION_WAR MINING TRACKING (inline) ---
    // Only count degenBTC that was actually distributed this round (not the full emission).
    // Empty rounds or rounds with no bets on certain directions may distribute less.
    msg!("🔍 [game.int_end_round_faction_rewards] require: faction_war_config.current_faction_war_id == faction_war_id: {} == {}",
        accounts.faction_war_config.current_faction_war_id, faction_war_id);
    require!(
        accounts.faction_war_config.current_faction_war_id == faction_war_id,
        ErrorCode::InvalidParameters
    );
    let same_faction_sum: u64 = game_session
        .minebtc_same_faction_direction_pools
        .iter()
        .sum::<u64>();
    let actually_distributed = game_session
        .minebtc_winner_pool
        .checked_add(same_faction_sum)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(game_session.faction_stakers)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(game_session.jackpot_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let round_id_for_event = game_session.round_id;
    msg!("📊 [game.int_end_round_faction_rewards] computation: actually_distributed = winner_pool({}) + same_faction_sum({}) + faction_stakers({}) + jackpot_rewards({}) = {}",
        game_session.minebtc_winner_pool, same_faction_sum, game_session.faction_stakers, game_session.jackpot_rewards, actually_distributed);

    // If settle_faction_war fired mid-round (LP burn landed during this round's
    // play window), faction_war_config.current_faction_war_id advanced but the new
    // faction_war_state was never initialized by a bet. The Accounts struct uses
    // init_if_needed so we got an empty account — stamp its bump + faction_war_id
    // and align faction_war_settle_cycle the same way the first bet would (see
    // internal_process_bets init branch). After this, the next join_bets on
    // the new faction_war enters the existing populate branch (faction_war_id != 0).
    msg!("🎲 [game.int_end_round_faction_rewards] branch: faction_war_state.faction_war_id={} active_faction_count={}",
        faction_war_state.faction_war_id, faction_war_state.active_faction_count);
    if faction_war_state.faction_war_id == 0 || faction_war_state.active_faction_count == 0 {
        msg!("⚠️ [game.int_end_round_faction_rewards] empty/seed faction_war_state -> helper call: seed_empty_faction_war_from_round");
        seed_empty_faction_war_from_round(
            accounts,
            faction_war_state.as_mut(),
            faction_war_state_bump,
        )?;
        // Nothing further to track — this is an empty/seed state. The next
        // bet will fill gameplay scores, direction totals, etc.
    } else if accounts.faction_war_config.is_active && faction_war_state.stage == 0 {
        // Round-win cycle score: sum of weighted points bet on the winning
        // country across all directions. Same metric we use for hashbeastBTC reward
        // distribution — keeps leaderboard math aligned with rewards math.
        let winner_idx = winning_faction_id as usize;
        let round_score: u64 = game_session.wgtd_points_bets_by_faction_direction[winner_idx]
            .iter()
            .copied()
            .try_fold(0u64, |acc, v| acc.checked_add(v))
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("🎲 [game.int_end_round_faction_rewards] faction_war active && stage==0 -> helper call: track_faction_war_round_completion(winning_faction_id={}, actually_distributed={}, round_score={})",
            winning_faction_id, actually_distributed, round_score);
        track_faction_war_round_completion(
            accounts,
            faction_war_state.as_mut(),
            winning_faction_id,
            actually_distributed,
            round_score,
        )?;
    } else {
        msg!("⚠️ [game.int_end_round_faction_rewards] faction_war tracking skipped: is_active={} stage={}",
            accounts.faction_war_config.is_active, faction_war_state.stage);
    }

    msg!(
        "🎲 [game.int_end_round_faction_rewards] emit RewardsDistributedForRound: round_id={}",
        round_id_for_event
    );
    emit!(RewardsDistributedForRound {
        round_id: round_id_for_event,
    });

    msg!("🔍 [game.int_end_round_faction_rewards] helper call: store_account_data(faction_war_state_info={})", faction_war_state_info.key());
    helper::store_account_data(&faction_war_state_info, faction_war_state.as_ref())?;

    msg!("✅ [game.int_end_round_faction_rewards] faction rewards finalized successfully");
    Ok(())
}

/// Internal function, called by int_end_round_faction_rewards to distribute rewards among AMG stakers (50% to degenBTC stakers, 50% to LP stakers)
#[inline(never)]
fn distribute_rewards_amg_stakers(
    minebtc_staker_rewards: u64,
    sol_staker_fees: u64,
    faction_state: &mut FactionState,
    round_id: u64,
) -> Result<()> {
    msg!("💰 [game.distribute_rewards_amg_stakers] minebtc_staker_rewards={} sol_staker_fees={} faction_id={} round_id={}",
        minebtc_staker_rewards, sol_staker_fees, faction_state.faction_id, round_id);
    let degenbtc_active = faction_state.total_degenbtc_hashpower > 0;
    let lp_active = faction_state.total_lp_hashpower > 0;
    msg!("💰 [game.distribute_rewards_amg_stakers] degenbtc_active={} total_degenbtc_hashpower={} lp_active={} total_lp_hashpower={}",
        degenbtc_active, faction_state.total_degenbtc_hashpower, lp_active, faction_state.total_lp_hashpower);

    msg!("🔍 [game.distribute_rewards_amg_stakers] helper call: split_staker_lane_rewards(minebtc={}, degenbtc_active={}, lp_active={})", minebtc_staker_rewards, degenbtc_active, lp_active);
    let (degenbtc_minebtc_share, lp_minebtc_share) =
        split_staker_lane_rewards(minebtc_staker_rewards, degenbtc_active, lp_active);
    msg!("🔍 [game.distribute_rewards_amg_stakers] helper call: split_staker_lane_rewards(sol={}, degenbtc_active={}, lp_active={})", sol_staker_fees, degenbtc_active, lp_active);
    let (degenbtc_sol_share, lp_sol_share) =
        split_staker_lane_rewards(sol_staker_fees, degenbtc_active, lp_active);
    msg!("📊 [game.distribute_rewards_amg_stakers] splits: degenbtc_minebtc={} lp_minebtc={} degenbtc_sol={} lp_sol={}",
        degenbtc_minebtc_share, lp_minebtc_share, degenbtc_sol_share, lp_sol_share);

    msg!("💰 [game.distribute_rewards_amg_stakers] branch: degenbtc_active={} && (minebtc>0 || sol>0) ? {} && ({} || {})",
        degenbtc_active, degenbtc_minebtc_share > 0, degenbtc_sol_share > 0, degenbtc_active && (degenbtc_minebtc_share > 0 || degenbtc_sol_share > 0));
    if degenbtc_active && (degenbtc_minebtc_share > 0 || degenbtc_sol_share > 0) {
        msg!("💰 [game.distribute_rewards_amg_stakers] helper call: mul_div(degenbtc_minebtc_share={}, INDEX_PRECISION, total_degenbtc_hashpower={})",
            degenbtc_minebtc_share, faction_state.total_degenbtc_hashpower);
        let minebtc_per_share = helper::mul_div(
            degenbtc_minebtc_share,
            INDEX_PRECISION,
            faction_state.total_degenbtc_hashpower,
        )?;
        let old_degenbtc_degenbtc_reward_index = faction_state.degenbtc_degenbtc_reward_index;
        faction_state.degenbtc_degenbtc_reward_index = faction_state
            .degenbtc_degenbtc_reward_index
            .checked_add(minebtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "💰 [game.distribute_rewards_amg_stakers] state mutation: degenbtc_degenbtc_reward_index {} -> {} (+minebtc_per_share={})",
            old_degenbtc_degenbtc_reward_index,
            faction_state.degenbtc_degenbtc_reward_index,
            minebtc_per_share
        );

        msg!("💰 [game.distribute_rewards_amg_stakers] helper call: mul_div(degenbtc_sol_share={}, INDEX_PRECISION, total_degenbtc_hashpower={})",
            degenbtc_sol_share, faction_state.total_degenbtc_hashpower);
        let sol_reward_inc = helper::mul_div(
            degenbtc_sol_share,
            INDEX_PRECISION,
            faction_state.total_degenbtc_hashpower,
        )?;
        let old_degenbtc_sol_reward_index = faction_state.degenbtc_sol_reward_index;
        faction_state.degenbtc_sol_reward_index = faction_state
            .degenbtc_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "💰 [game.distribute_rewards_amg_stakers] state mutation: degenbtc_sol_reward_index {} -> {} (+sol_reward_inc={})",
            old_degenbtc_sol_reward_index,
            faction_state.degenbtc_sol_reward_index,
            sol_reward_inc
        );

        msg!("💰 [game.distribute_rewards_amg_stakers] emit DegenBtcStakingRewardsDistributed: round_id={} faction_id={} minebtc={} sol={} degenbtc_degenbtc_reward_index={} degenbtc_sol_reward_index={}",
            round_id, faction_state.faction_id, degenbtc_minebtc_share, degenbtc_sol_share,
            faction_state.degenbtc_degenbtc_reward_index, faction_state.degenbtc_sol_reward_index);
        emit!(DegenBtcStakingRewardsDistributed {
            round_id,
            faction_id: faction_state.faction_id,
            minebtc_staker_rewards: degenbtc_minebtc_share,
            sol_staker_rewards: degenbtc_sol_share,
            degenbtc_degenbtc_reward_index: faction_state.degenbtc_degenbtc_reward_index,
            degenbtc_sol_reward_index: faction_state.degenbtc_sol_reward_index
        });
    }

    msg!("💰 [game.distribute_rewards_amg_stakers] branch: lp_active={} && (minebtc>0 || sol>0) ? {} && ({} || {})",
        lp_active, lp_minebtc_share > 0, lp_sol_share > 0, lp_active && (lp_minebtc_share > 0 || lp_sol_share > 0));
    if lp_active && (lp_minebtc_share > 0 || lp_sol_share > 0) {
        msg!("💰 [game.distribute_rewards_amg_stakers] helper call: mul_div(lp_minebtc_share={}, INDEX_PRECISION, total_lp_hashpower={})",
            lp_minebtc_share, faction_state.total_lp_hashpower);
        let minebtc_per_share = helper::mul_div(
            lp_minebtc_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        let old_lp_degenbtc_reward_index = faction_state.lp_degenbtc_reward_index;
        faction_state.lp_degenbtc_reward_index = faction_state
            .lp_degenbtc_reward_index
            .checked_add(minebtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "💰 [game.distribute_rewards_amg_stakers] state mutation: lp_degenbtc_reward_index {} -> {} (+minebtc_per_share={})",
            old_lp_degenbtc_reward_index,
            faction_state.lp_degenbtc_reward_index,
            minebtc_per_share
        );

        msg!("💰 [game.distribute_rewards_amg_stakers] helper call: mul_div(lp_sol_share={}, INDEX_PRECISION, total_lp_hashpower={})",
            lp_sol_share, faction_state.total_lp_hashpower);
        let sol_reward_inc = helper::mul_div(
            lp_sol_share,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        let old_lp_sol_reward_index = faction_state.lp_sol_reward_index;
        faction_state.lp_sol_reward_index = faction_state
            .lp_sol_reward_index
            .checked_add(sol_reward_inc)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "💰 [game.distribute_rewards_amg_stakers] state mutation: lp_sol_reward_index {} -> {} (+sol_reward_inc={})",
            old_lp_sol_reward_index,
            faction_state.lp_sol_reward_index,
            sol_reward_inc
        );

        msg!("💰 [game.distribute_rewards_amg_stakers] emit LpStakingRewardsDistributed: round_id={} faction_id={} minebtc={} sol={} lp_degenbtc_reward_index={} lp_sol_reward_index={}",
            round_id, faction_state.faction_id, lp_minebtc_share, lp_sol_share,
            faction_state.lp_degenbtc_reward_index, faction_state.lp_sol_reward_index);
        emit!(LpStakingRewardsDistributed {
            round_id,
            faction_id: faction_state.faction_id,
            minebtc_staker_rewards: lp_minebtc_share,
            sol_staker_rewards: lp_sol_share,
            lp_degenbtc_reward_index: faction_state.lp_degenbtc_reward_index,
            lp_sol_reward_index: faction_state.lp_sol_reward_index
        });
    }

    msg!("✅ [game.distribute_rewards_amg_stakers] done");
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

    // Idempotency: already processed this round
    if game_session.jackpot_distributed {
        msg!("🎰 [game.int_distribute_jackpot_rewards] early return: jackpot already distributed for round {}", game_session.round_id);
        return Ok(());
    }

    // No jackpot hit this round — nothing to do
    if !game_session.jackpot_hit {
        msg!("🎰 [game.int_distribute_jackpot_rewards] early return: no jackpot hit this round");
        return Ok(());
    }

    // Already paid out (pot was drained by a previous call)
    if global_state.jackpot_pot == 0 {
        msg!("🎰 [game.int_distribute_jackpot_rewards] early return: jackpot pot already empty");
        game_session.jackpot_distributed = true;
        return Ok(());
    }

    let jackpot_faction_id = game_session.jackpot_faction_id as usize;
    require!(
        jackpot_faction_id < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );

    let winning_direction = game_session.winning_direction;

    // Sum ALL weighted points on the jackpot faction across ALL directions.
    // Jackpot rewards go to anyone who bet on the jackpot faction, regardless of direction.
    let total_jackpot_wgtd_pts: u64 = game_session
        .wgtd_points_bets_by_faction_direction[jackpot_faction_id]
        .iter()
        .copied()
        .try_fold(0u64, |acc, v| acc.checked_add(v))
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    msg!(
        "🎰 [game.int_distribute_jackpot_rewards] jackpot_faction_id={} total_jackpot_wgtd_pts={} pot={}",
        jackpot_faction_id, total_jackpot_wgtd_pts, global_state.jackpot_pot
    );

    let jackpot_bonus = global_state.jackpot_pot;

    if total_jackpot_wgtd_pts > 0 {
        // Eligible bettors exist — pay out the entire accumulated pot
        global_state.jackpot_pot = 0;
        game_session.jackpot_pot_size_on_hit = jackpot_bonus;

        let jackpot_index = helper::mul_div(jackpot_bonus, INDEX_PRECISION, total_jackpot_wgtd_pts)?;
        game_session.jackpot_rewards_index = jackpot_index as u128;
        game_session.jackpot_distributed = true;

        msg!(
            "🎰 [game.int_distribute_jackpot_rewards] state mutation: jackpot_paid={} index={} (all directions on faction {})",
            jackpot_bonus, jackpot_index, jackpot_faction_id
        );

        emit!(crate::events::JackpotHit {
            round_id: game_session.round_id,
            faction_id: jackpot_faction_id as u8,
            winning_direction,
            jackpot_amount: jackpot_bonus,
            minebtc_rewards_index: game_session.jackpot_rewards_index,
        });
    } else {
        // No bettors on the jackpot faction — pot rolls over to next hit
        game_session.jackpot_pot_size_on_hit = 0;
        game_session.jackpot_rewards_index = 0;
        game_session.jackpot_distributed = true;

        msg!(
            "🎰 [game.int_distribute_jackpot_rewards] Jackpot hit for faction {} but no bettors — pot rolls over: {}",
            jackpot_faction_id, jackpot_bonus
        );

        emit!(crate::events::JackpotRolledOver {
            round_id: game_session.round_id,
            faction_id: jackpot_faction_id as u8,
            pot_size: jackpot_bonus,
            reason: 0, // 0 = no bettors on jackpot faction
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    msg!("✅ [game.int_distribute_jackpot_rewards] done");
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
#[instruction(faction_war_id: u64)]
pub struct EndRoundFactionRewards<'info> {
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
        bump = mine_btc_mining.bump
    )]
    pub mine_btc_mining: Box<Account<'info, MineBtcMining>>,

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

    /// CHECK: Program PDA; initialized manually in handler to keep parser stack small.
    /// Must remain `mut` because helper::store_account_data persists the seeded/updated faction-war state.
    #[account(
        mut,
        seeds = [FACTION_WAR_STATE_SEED, &faction_war_id.to_le_bytes()],
        bump,
    )]
    pub faction_war_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
