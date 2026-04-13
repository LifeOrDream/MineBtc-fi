// # Game Instructions
//
// This module implements the core game loop for the MineBTC Faction Surge betting game.
//
// ## Game Mechanics
//
// The game operates in rounds where:
// 1. A cranker bot starts a new round by committing a randomness hash (commit-reveal scheme).
// 2. Players place faction-direction bets that also count toward the active epoch market.
// 3. After the round ends, the cranker reveals the seed to select a winning faction and direction.
// 4. Exact faction+direction bettors receive the main round rewards, while other directions on the
//    winning faction can still share a consolation MineBTC pool.
//
// ## Key Functions
//
// - `start_round`: Initializes a new round with committed randomness.
// - `end_round`: Reveals the seed, selects the winning faction and direction, and calculates initial rewards.
// - `end_round_faction_rewards`: Distributes MineBTC rewards to stakers and faction pools.
//
// The commit-reveal randomness system ensures fairness and prevents manipulation.
//

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::solana_program::sysvar::Sysvar;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::instructions::helper;
use crate::state::*;

// ========================================================================================
// =============================== GAME ROUND MANAGEMENT ============================
// ========================================================================================

/// Start a new round by committing a hash and initializing GameSession.
pub fn int_start_round(ctx: Context<StartRound>, round_id: u64, commit: [u8; 32]) -> Result<()> {
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;

    require!(global_state.is_active, ErrorCode::InvalidParameters);
    require!(global_state.can_begin_round, ErrorCode::CannotBeginRound);

    let expected_round_id = global_state.current_round_id + 1;
    require!(round_id == expected_round_id, ErrorCode::InvalidRound);

    require!(
        global_state
            .cranker_bots
            .contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );

    global_state.current_round_commit = commit;
    global_state.current_round_seed = None;

    global_state.current_round_id = round_id;

    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = round_id;
    game_session.round_start_timestamp = clock.unix_timestamp;
    game_session.stage = 0;
    game_session.total_sol_bets = 0;
    game_session.total_points_bets = 0;
    game_session.total_wgtd_points_bets = 0;
    game_session.stakers_fee = 0;
    game_session.user_faction_indexes = [0u64; NUM_FACTIONS];
    game_session.sol_bets_by_faction = [0u64; NUM_FACTIONS];
    game_session.points_bets_by_faction = [0u64; NUM_FACTIONS];
    game_session.wgtd_points_bets_by_faction = [0u64; NUM_FACTIONS];
    game_session.points_bets_by_faction_direction =
        [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    game_session.wgtd_points_bets_by_faction_direction =
        [[0u64; PredictionDirection::COUNT]; NUM_FACTIONS];
    game_session.winning_faction_id = 0;
    game_session.winning_direction = PredictionDirection::Neutral.as_index() as u8;
    game_session.minebtc_winner_pool = 0;
    game_session.minebtc_same_faction_pool = 0;
    game_session.faction_stakers = 0;
    game_session.motherlode_rewards = 0;
    game_session.sol_rewards_index = 0;
    game_session.minebtc_rewards_index = 0;
    game_session.same_faction_minebtc_rewards_index = 0;
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;
    game_session.highest_sol_bet_per_faction = [0u64; NUM_FACTIONS];
    game_session.mutation_occurred_per_faction = [false; NUM_FACTIONS];
    global_state.can_begin_round = false;

    emit!(RoundStarted {
        round_id,
        game_session: game_session.key(),
        commit_hash: commit,
        epoch_id: ctx.accounts.epoch_config.current_epoch_id,
        active_index_id: ctx.accounts.epoch_config.active_index_id,
        active_question_hash: ctx.accounts.epoch_config.active_question_hash,
        round_start_timestamp: game_session.round_start_timestamp,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// End the current round by revealing seed, selecting winner, and starting next round.
/// This function:
/// 1. Verifies revealed seed matches commit hash
/// 2. Generates final randomness using seed + current slot + current timestamp
/// 3. Selects a winning faction from factions that actually received bets
/// 4. Selects a winning direction inside that faction from active directional bets
/// 5. Calculates winners and updates payout data
pub fn int_end_round(ctx: Context<EndRound>, revealed_seed: [u8; 32]) -> Result<()> {
    let game_session = &mut ctx.accounts.game_session;
    let global_state = &mut ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;

    if game_session.stage == 1 || game_session.stage == 2 {
        return Ok(());
    }

    require!(
        clock.unix_timestamp
            >= game_session.round_start_timestamp + global_state.round_duration_seconds,
        ErrorCode::RoundNotEnded
    );
    require!(game_session.stage == 0, ErrorCode::InvalidStage);
    require!(
        global_state
            .cranker_bots
            .contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );

    let seed_hash = keccak::hash(&revealed_seed);
    require!(
        seed_hash.to_bytes() == global_state.current_round_commit,
        ErrorCode::InvalidParameters
    );
    global_state.current_round_seed = Some(revealed_seed);

    let mut final_randomness_seed = Vec::new();
    final_randomness_seed.extend_from_slice(&revealed_seed);
    final_randomness_seed.extend_from_slice(&clock.slot.to_le_bytes());
    final_randomness_seed.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
    let final_hash_bytes = keccak::hash(&final_randomness_seed).to_bytes();

    let total_users: u64 = game_session.user_faction_indexes.iter().sum();
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
        ]) % NUM_FACTIONS as u64) as u8
    } else {
        let initial_faction = (u64::from_le_bytes([
            final_hash_bytes[0],
            final_hash_bytes[1],
            final_hash_bytes[2],
            final_hash_bytes[3],
            0,
            0,
            0,
            0,
        ]) % NUM_FACTIONS as u64) as u8;
        find_valid_winning_faction(initial_faction, &game_session.user_faction_indexes)?
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
            initial_direction,
            &game_session.points_bets_by_faction_direction[winning_faction_id as usize],
        )?
    };
    game_session.winning_direction = winning_direction;

    if total_users == 0 {
        global_state.last_round_id = game_session.round_id;
        global_state.winning_faction_id = winning_faction_id;
        global_state.total_sol_bets += game_session.total_sol_bets as u128;
        global_state.can_begin_round = true;
        game_session.stage = 2;

        emit!(RoundEnded {
            round_id: game_session.round_id,
            game_session: game_session.key(),
            winning_faction_id,
            winning_direction,
            total_sol_bets: game_session.total_sol_bets,
            total_points_bets: game_session.total_points_bets,
            user_bets_count: game_session.user_faction_indexes,
            faction_sol_bets: game_session.sol_bets_by_faction,
            faction_points: game_session.points_bets_by_faction,
            faction_wgtd_points: game_session.wgtd_points_bets_by_faction,
            minebtc_winner_pool: 0,
            minebtc_same_faction_pool: 0,
            minebtc_faction_stakers: 0,
            minebtc_motherlode: 0,
            motherlode_hit: false,
            timestamp: clock.unix_timestamp,
        });

        return Ok(());
    }

    let minebtc_rewards = ctx.accounts.mine_btc_mining.mine_btc_per_round;
    let (
        mut winning_direction_rewards,
        mut same_faction_direction_rewards,
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
    let same_faction_wgtd_points = game_session.wgtd_points_bets_by_faction
        [winning_faction_id as usize]
        .saturating_sub(winning_wgtd_points);

    // If the winning faction only had one active direction, roll the consolation pool into
    // the exact winner pool so emission is still fully claimable.
    if same_faction_wgtd_points == 0 && same_faction_direction_rewards > 0 {
        winning_direction_rewards = winning_direction_rewards
            .checked_add(same_faction_direction_rewards)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        same_faction_direction_rewards = 0;
    }

    game_session.minebtc_winner_pool = winning_direction_rewards;
    game_session.minebtc_same_faction_pool = same_faction_direction_rewards;
    game_session.faction_stakers = faction_stakers;
    game_session.motherlode_rewards = motherlode_rewards;

    let total_distributed_this_round = game_session.minebtc_winner_pool
        + game_session.minebtc_same_faction_pool
        + faction_stakers
        + motherlode_rewards;
    ctx.accounts.mine_btc_mining.total_tokens_mined += total_distributed_this_round;

    if winning_points > 0 {
        game_session.sol_rewards_index +=
            helper::mul_div(game_session.total_sol_bets, INDEX_PRECISION, winning_points)?;
    }
    if winning_wgtd_points > 0 {
        game_session.minebtc_rewards_index += helper::mul_div(
            game_session.minebtc_winner_pool,
            INDEX_PRECISION,
            winning_wgtd_points,
        )?;
    }
    if same_faction_wgtd_points > 0 && game_session.minebtc_same_faction_pool > 0 {
        game_session.same_faction_minebtc_rewards_index += helper::mul_div(
            game_session.minebtc_same_faction_pool,
            INDEX_PRECISION,
            same_faction_wgtd_points,
        )?;
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

    emit!(RoundEnded {
        round_id: game_session.round_id,
        game_session: game_session.key(),
        winning_faction_id,
        winning_direction,
        total_sol_bets: game_session.total_sol_bets,
        total_points_bets: game_session.total_points_bets,
        user_bets_count: game_session.user_faction_indexes,
        faction_sol_bets: game_session.sol_bets_by_faction,
        faction_points: game_session.points_bets_by_faction,
        faction_wgtd_points: game_session.wgtd_points_bets_by_faction,
        minebtc_winner_pool: game_session.minebtc_winner_pool,
        minebtc_same_faction_pool: game_session.minebtc_same_faction_pool,
        minebtc_faction_stakers: faction_stakers,
        minebtc_motherlode: motherlode_rewards,
        motherlode_hit: game_session.motherlode_hit,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Find a valid winning faction that has at least one bettor.
/// Starts from the initial faction and decrements until finding a faction with active users.
fn find_valid_winning_faction(
    initial_faction: u8,
    user_faction_indexes: &[u64; NUM_FACTIONS],
) -> Result<u8> {
    let mut current_faction = initial_faction;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u8 = NUM_FACTIONS as u8;

    loop {
        if attempts >= MAX_ATTEMPTS {
            return Err(ErrorCode::InvalidParameters.into());
        }

        if user_faction_indexes[current_faction as usize] > 0 {
            return Ok(current_faction);
        }

        if current_faction == 0 {
            current_faction = (NUM_FACTIONS - 1) as u8;
        } else {
            current_faction -= 1;
        }

        attempts += 1;
    }
}

fn find_valid_winning_direction(
    initial_direction: u8,
    direction_points: &[u64; PredictionDirection::COUNT],
) -> Result<u8> {
    let mut current_direction = initial_direction;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u8 = PredictionDirection::COUNT as u8;

    loop {
        if attempts >= MAX_ATTEMPTS {
            return Err(ErrorCode::InvalidParameters.into());
        }

        if direction_points[current_direction as usize] > 0 {
            return Ok(current_direction);
        }

        if current_direction == 0 {
            current_direction = (PredictionDirection::COUNT - 1) as u8;
        } else {
            current_direction -= 1;
        }

        attempts += 1;
    }
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
    let same_faction_direction_rewards =
        (minebtc_rewards as u128 * minebtc_same_faction_pct as u128 / 100) as u64;
    let faction_stakers = (minebtc_rewards as u128 * minebtc_stakers_pct as u128 / 100) as u64;
    let motherlode_rewards =
        (minebtc_rewards as u128 * minebtc_motherlode_pct as u128 / 100) as u64;
    (
        winning_direction_rewards,
        same_faction_direction_rewards,
        faction_stakers,
        motherlode_rewards,
    )
}

/// Finalize the faction-level reward distribution for the round.
/// This function:
/// 1. Verifies revealed seed matches commit hash
/// 2. Distributes the winning faction's staker and motherlode rewards
/// 3. Advances epoch accounting when the current epoch window has ended
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

    // Validate caller is a whitelisted cranker bot
    require!(
        global_state
            .cranker_bots
            .contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );

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

    // dBTC + SOL distribution to dBTC stakers of the winning faction
    distribute_rewards_amg_stakers(
        minebtc_staker_rewards,
        sol_staker_fees,
        faction_state,
        game_session.round_id,
    )?;

    // Increment motherlode pot size (always, regardless of hit)
    faction_state.motherlode_pot_size =
        faction_state.motherlode_pot_size + game_session.motherlode_rewards;
    msg!(
        "   Motherlode pot: {} -> {} (+{})",
        (faction_state.motherlode_pot_size - game_session.motherlode_rewards) as f64 / 1_000_000.0,
        faction_state.motherlode_pot_size as f64 / 1_000_000.0,
        game_session.motherlode_rewards as f64 / 1_000_000.0
    );

    let motherlode_hit = game_session.motherlode_hit;
    let winning_direction = game_session.winning_direction;
    let exact_winning_wgtd_pts = game_session.wgtd_points_bets_by_faction_direction
        [winning_faction_id as usize][winning_direction as usize];

    if motherlode_hit && faction_state.motherlode_pot_size > 0 {
        let motherlode_bonus = faction_state.motherlode_pot_size;
        faction_state.motherlode_pot_size = 0;
        game_session.minebtc_winner_pool += motherlode_bonus;
        game_session.motherlode_pot_size_on_hit = motherlode_bonus;

        if motherlode_bonus > 0 && exact_winning_wgtd_pts > 0 {
            let minebtc_rewards_delta =
                helper::mul_div(motherlode_bonus, INDEX_PRECISION, exact_winning_wgtd_pts)?;
            game_session.minebtc_rewards_index += minebtc_rewards_delta;
        }

        emit!(MotherlodeHit {
            round_id: game_session.round_id,
            faction_id: faction_state.faction_id,
            winning_direction,
            winning_faction_rewards: motherlode_bonus,
            minebtc_rewards_index: game_session.minebtc_rewards_index,
        });
    }

    // Update faction wins
    faction_state.total_wins = faction_state.total_wins + 1;

    // Update global state with previous round results
    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;
    msg!(
        "   Global state updated: last_round_id: {}, winning_faction_id: {}",
        global_state.last_round_id,
        global_state.winning_faction_id
    );

    // Update total SOL bets in global state (cumulative)
    global_state.total_sol_bets =
        global_state.total_sol_bets + (game_session.total_sol_bets as u128);
    msg!(
        "   Updated global state. Total SOL bets: {}",
        global_state.total_sol_bets as f64 / 1_000_000_000.0
    );

    game_session.stage = 2;

    // Can start new round now
    global_state.can_begin_round = true;
    msg!("   Can begin new round: {}", global_state.can_begin_round);

    // --- EPOCH MINING TRACKING (inline) ---
    let epoch_config = &mut ctx.accounts.epoch_config;
    let epoch_state = &mut ctx.accounts.epoch_state;
    let mine_btc_per_round = ctx.accounts.mine_btc_mining.mine_btc_per_round;

    if epoch_config.is_active && epoch_state.stage == 0 {
        epoch_state.total_dogebtc_mined_in_epoch = epoch_state
            .total_dogebtc_mined_in_epoch
            .checked_add(mine_btc_per_round)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        let clock = Clock::get()?;
        if clock.unix_timestamp >= epoch_state.end_timestamp as i64 {
            require!(
                ctx.accounts.index_state.index_id == epoch_state.index_id,
                ErrorCode::InvalidIndexState
            );
            crate::instructions::epoch::finalize_epoch_settlement(
                epoch_config,
                epoch_state,
                &clock,
            )?;

            emit!(EpochAutoSettled {
                epoch_id: epoch_state.epoch_id,
                index_id: epoch_state.index_id,
                mining_pool: epoch_state.epoch_mining_pool,
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
    mut minebtc_staker_rewards: u64,
    mut sol_staker_fees: u64,
    faction_state: &mut FactionState,
    round_id: u64,
) -> Result<()> {
    if faction_state.total_dogebtc_hashpower > 0 {
        // Calculate shares BEFORE modifying the totals
        let dogebtc_minebtc_share = minebtc_staker_rewards / 2;
        let dogebtc_sol_share = sol_staker_fees / 2;

        let minebtc_per_share = helper::mul_div(
            dogebtc_minebtc_share,
            INDEX_PRECISION,
            faction_state.total_dogebtc_hashpower,
        )?;
        faction_state.dogebtc_dogebtc_reward_index =
            faction_state.dogebtc_dogebtc_reward_index + minebtc_per_share;
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
        faction_state.dogebtc_sol_reward_index += sol_reward_inc;
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

        // Deduct shares AFTER event emission
        minebtc_staker_rewards = minebtc_staker_rewards - dogebtc_minebtc_share;
        sol_staker_fees = sol_staker_fees - dogebtc_sol_share;
    }

    if faction_state.total_lp_hashpower > 0 {
        let minebtc_per_share = helper::mul_div(
            minebtc_staker_rewards,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_dogebtc_reward_index =
            faction_state.lp_dogebtc_reward_index + minebtc_per_share;
        msg!(
            "   Faction stakers MINEBTC reward index: {} -> {} (+{})",
            faction_state.lp_dogebtc_reward_index - minebtc_per_share,
            faction_state.lp_dogebtc_reward_index,
            minebtc_per_share
        );

        let sol_reward_inc = helper::mul_div(
            sol_staker_fees,
            INDEX_PRECISION,
            faction_state.total_lp_hashpower,
        )?;
        faction_state.lp_sol_reward_index += sol_reward_inc;
        msg!(
            "   Faction stakers SOL reward index: {} -> {} (+{})",
            faction_state.lp_sol_reward_index - sol_reward_inc,
            faction_state.lp_sol_reward_index,
            sol_reward_inc
        );

        emit!(LpStakingRewardsDistributed {
            round_id: round_id,
            faction_id: faction_state.faction_id,
            minebtc_staker_rewards: minebtc_staker_rewards,
            sol_staker_rewards: sol_staker_fees,
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
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        init,
        payer = authority,
        space = GameSession::LEN,
        seeds = [GAME_SESSION_SEED.as_ref(), &round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Account<'info, GameSession>,

    #[account(
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

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
    pub game_session: Account<'info, GameSession>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

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
    pub global_game_state: Account<'info, GlobalGameSate>,

    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,

    #[account(
        mut,
        seeds = [MINE_BTC_MINING_SEED.as_ref()],
        bump = mine_btc_mining.bump
    )]
    pub mine_btc_mining: Account<'info, MineBtcMining>,

    /// Winning faction state (for updating staker rewards and motherlode)
    /// CHECK: Validated manually that faction_id matches winning_faction_id
    #[account(mut)]
    pub faction_state: Account<'info, FactionState>,

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
        seeds = [EPOCH_CONFIG_SEED],
        bump = epoch_config.bump,
    )]
    pub epoch_config: Account<'info, EpochConfig>,

    /// Epoch state for current epoch (mut for mining tracking + settlement)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch_config.current_epoch_id.to_le_bytes()],
        bump = epoch_state.bump,
    )]
    pub epoch_state: Account<'info, EpochState>,

    #[account(
        seeds = [INDEX_STATE_SEED, &[index_state.index_id]],
        bump = index_state.bump,
        constraint = index_state.index_id == epoch_state.index_id @ ErrorCode::InvalidIndexState,
    )]
    pub index_state: Account<'info, IndexState>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
