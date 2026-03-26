// # Game Instructions
//
// This module implements the core game loop for the MineBTC Faction Surge betting game.
//
// ## Game Mechanics
//
// The game operates in rounds where:
// 1. A cranker bot starts a new round by committing a randomness hash (commit-reveal scheme).
// 2. Players bet SOL on specific blocks (1-24) or factions (highest/lowest).
// 3. After the round ends, the cranker reveals the seed to select a winning block.
// 4. Winners receive SOL and MineBTC rewards; stakers and same-faction bettors also earn rewards.
//
// ## Key Functions
//
// - `start_round`: Initializes a new round with committed randomness.
// - `end_round`: Reveals the seed, selects the winning block, and calculates initial rewards.
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

/// Start a new round by committing a hash and initializing GameSession
/// This function:
/// 1. Commits a hash for randomness (commit-reveal scheme)
/// 2. Randomly assigns 12 factions to 24 blocks (2 blocks per faction)
/// 3. Initializes GameSession for the new round
///
/// The commit hash should be hash(secret_seed) where secret_seed will be revealed in end_round.
pub fn int_start_round(ctx: Context<StartRound>, round_id: u64, commit: [u8; 32]) -> Result<()> {
    msg!("🎮 [start_round] Starting new round");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    // Validate game is active
    require!(global_state.is_active, ErrorCode::InvalidParameters);
    require!(global_state.can_begin_round, ErrorCode::CannotBeginRound);
    msg!("   ✓ Game is active and can begin round");

    msg!(
        "   Current round ID: {}, Requested round ID: {}",
        global_state.current_round_id,
        round_id
    );

    // Validate round_id matches expected value (current_round_id + 1)
    let expected_round_id = global_state.current_round_id + 1;
    require!(round_id == expected_round_id, ErrorCode::InvalidRound);

    // Validate caller is a whitelisted cranker bot
    require!(
        global_state
            .cranker_bots
            .contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is whitelisted cranker bot");

    msg!("   Commit hash: {:?}", commit);

    // Set commit hash for this round
    global_state.current_round_commit = commit;
    global_state.current_round_seed = None; // Will be set in end_round
    msg!("   Commit hash set for round {}", round_id);

    // Update global state
    global_state.current_round_id = round_id;
    msg!("   Global state updated: current_round_id={}", round_id,);

    // Initialize GameSession for the new round
    msg!("   Initializing GameSession for round {}", round_id);
    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = round_id;
    game_session.round_start_timestamp = clock.unix_timestamp;
    game_session.total_sol_bets = 0;
    game_session.total_points_bets = 0;

    // Initialize 24-sized arrays for block tracking
    game_session.user_block_indexes = vec![0u64; NUM_BLOCKS];
    game_session.sol_bets_indexes = vec![0u64; NUM_BLOCKS];
    game_session.points_bets_indexes = vec![0u64; NUM_BLOCKS];
    game_session.wgtd_points_bets_indexes = vec![0u64; NUM_BLOCKS];

    // Initialize instant mutation tracking per faction
    game_session.highest_sol_bet_per_faction = [0u64; NUM_FACTIONS];
    game_session.mutation_occurred_per_faction = [false; NUM_FACTIONS];

    // Randomly assign 12 factions to 24 blocks (2 blocks per faction)
    // Use commit hash + slot for deterministic but unpredictable randomness
    msg!(
        "   Assigning {} factions to {} blocks ({} blocks per faction)",
        NUM_FACTIONS,
        NUM_BLOCKS,
        BLOCKS_PER_FACTION
    );

    // Create seed for randomness: commit_hash + current_slot
    let mut random_seed = Vec::new();
    random_seed.extend_from_slice(&commit);
    random_seed.extend_from_slice(&clock.slot.to_le_bytes());
    let hash = keccak::hash(&random_seed);
    let hash_bytes = hash.to_bytes();
    msg!(
        "   Generated randomness seed from commit hash + slot {}",
        clock.slot
    );

    // Create a list with each faction appearing BLOCKS_PER_FACTION times
    // This guarantees each faction gets exactly BLOCKS_PER_FACTION blocks
    let mut faction_list = Vec::new();
    for faction_id in 0..NUM_FACTIONS {
        for _ in 0..BLOCKS_PER_FACTION {
            faction_list.push(faction_id as u8);
        }
    }
    msg!(
        "   Created faction list with {} entries ({} factions × {} blocks each)",
        faction_list.len(),
        NUM_FACTIONS,
        BLOCKS_PER_FACTION
    );

    // Shuffle the faction list using Fisher-Yates algorithm with hash bytes as randomness
    let mut hash_offset = 0;
    for i in (1..faction_list.len()).rev() {
        // Use hash bytes for randomness (cycling through hash bytes)
        let random_byte = hash_bytes[hash_offset % 32];
        let j = (random_byte as usize) % (i + 1);
        faction_list.swap(i, j);
        hash_offset += 1;
    }
    msg!("   Shuffled faction list using hash-based randomness");

    // Assign shuffled factions to blocks
    let mut block_assignments = [0u8; NUM_BLOCKS];
    for (block_idx, &faction_id) in faction_list.iter().enumerate() {
        block_assignments[block_idx] = faction_id;
    }

    // Verify all factions got exactly BLOCKS_PER_FACTION blocks
    msg!("   Verifying block assignments...");
    let mut faction_blocks_assigned = [0u8; NUM_FACTIONS];
    for &faction_id in block_assignments.iter() {
        faction_blocks_assigned[faction_id as usize] += 1;
    }
    for (faction_idx, &count) in faction_blocks_assigned.iter().enumerate() {
        require!(
            count == BLOCKS_PER_FACTION as u8,
            ErrorCode::InvalidParameters
        );
        msg!("     Faction {}: {} blocks assigned", faction_idx, count);
    }
    msg!(
        "   ✓ All factions assigned exactly {} blocks",
        BLOCKS_PER_FACTION
    );
    msg!("   Block assignments: {:?}", block_assignments);

    game_session.stage = 0;
    game_session.block_assignments = block_assignments;
    game_session.winning_block = 0; // Will be set in end_round
    game_session.winning_faction_id = 0; // Will be set in end_round
    game_session.same_faction_other_block = 0; // Will be set in end_round

    // Initialize reward indexes
    game_session.sol_rewards_index = 0;
    game_session.minebtc_rewards_index = 0;
    game_session.same_faction_minebtc_rewards_index = 0;

    // Initialize MineBtc pools
    game_session.minebtc_winner_pool = 0;
    game_session.minebtc_loser_pool = 0;

    // Initialize motherlode fields
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;

    // Cannot start new round till this on-going round is ended
    global_state.can_begin_round = false;

    msg!("✅ [start_round] Round {} started successfully", round_id);
    msg!("   Commit hash: {:?}", commit);

    emit!(RoundStarted {
        round_id,
        game_session: game_session.key(),
        commit_hash: commit,
        block_assignments: block_assignments,
        round_start_timestamp: game_session.round_start_timestamp,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// End the current round by revealing seed, selecting winner, and starting next round
/// This function:
/// 1. Verifies revealed seed matches commit hash
/// 2. Generates final randomness using seed + blockhash
/// 3. Selects winning block
/// 4. Calculates winners and updates payout data
/// 5. Commits hash for next round
pub fn int_end_round(
    ctx: Context<EndRound>,
    revealed_seed: [u8; 32], // The secret seed that was hashed to create commit_hash
) -> Result<()> {
    msg!("🏁 [end_round] Ending current round");
    msg!("   Authority: {}", ctx.accounts.authority.key());

    let game_session = &mut ctx.accounts.game_session;
    let global_state = &mut ctx.accounts.global_game_state;

    let global_config = &ctx.accounts.global_config;
    let mine_btc_mining = &ctx.accounts.mine_btc_mining;
    let clock = Clock::get()?;

    if game_session.stage == 1 || game_session.stage == 2 {
        msg!("   Round has already ended or already distributed faction rewards, skipping");
        return Ok(());
    }

    // check how many blocks bets have been placed on
    let used_blocks = game_session
        .user_block_indexes
        .iter()
        .filter(|&x| *x > 0)
        .count();
    msg!("   Used blocks: {}", used_blocks);

    if used_blocks < 2 {
        msg!("   ⚠️ Less than 2 blocks have been bet on - round in extended mode");
        return Ok(());
    }

    // Validate round has ended
    require!(
        clock.unix_timestamp
            >= game_session.round_start_timestamp + global_state.round_duration_seconds,
        ErrorCode::RoundNotEnded
    );
    require!(game_session.stage == 0, ErrorCode::InvalidStage);
    msg!(
        "   ✓ Round has ended. Current round ID: {}, Current timestamp: {}, ",
        game_session.round_id,
        clock.unix_timestamp
    );

    // Validate caller is a whitelisted cranker bot
    require!(
        global_state
            .cranker_bots
            .contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is whitelisted cranker bot");

    // Verify commit-reveal: hash(revealed_seed) must equal current_round_commit
    msg!("   Verifying commit-reveal scheme...");
    let seed_hash = keccak::hash(&revealed_seed);
    let seed_hash_bytes = seed_hash.to_bytes();
    msg!("   Revealed seed hash: {:?}", seed_hash_bytes);
    msg!(
        "   Expected commit hash: {:?}",
        global_state.current_round_commit
    );
    require!(
        seed_hash_bytes == global_state.current_round_commit,
        ErrorCode::InvalidParameters
    );
    msg!("   ✓ Commit-reveal verification passed");

    // Store revealed seed
    global_state.current_round_seed = Some(revealed_seed);
    msg!(
        "   Stored revealed seed for round {}",
        game_session.round_id
    );

    // Generate final randomness: hash(revealed_seed + clock slot + timestamp)
    // This combines the revealed seed with on-chain values that were unpredictable during betting
    msg!("   Generating final randomness from revealed seed + on-chain values...");
    let clock = Clock::get()?;
    let mut final_randomness_seed = Vec::new();
    final_randomness_seed.extend_from_slice(&revealed_seed);
    final_randomness_seed.extend_from_slice(&clock.slot.to_le_bytes());
    final_randomness_seed.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
    let final_hash = keccak::hash(&final_randomness_seed);
    let final_hash_bytes = final_hash.to_bytes();
    msg!(
        "   Final hash generated from seed + slot {} + timestamp {}",
        clock.slot,
        clock.unix_timestamp
    );

    // Check if there are any users/bets in this round
    let total_users: u64 = game_session.user_block_indexes.iter().sum();

    if total_users == 0 {
        msg!("   ⚠️ No users or bets in this round - finishing round without rewards");

        // Set default values for winning block (use initial block from hash, but no rewards)
        let initial_winning_block = (u64::from_le_bytes([
            final_hash_bytes[0],
            final_hash_bytes[1],
            final_hash_bytes[2],
            final_hash_bytes[3],
            0,
            0,
            0,
            0,
        ]) % NUM_BLOCKS as u64) as u8;

        let winning_faction_id = game_session.block_assignments[initial_winning_block as usize];
        let same_faction_other_block = game_session
            .block_assignments
            .iter()
            .enumerate()
            .find(|(idx, &faction)| {
                faction == winning_faction_id && *idx != initial_winning_block as usize
            })
            .map(|(idx, _)| idx as u8)
            .unwrap_or(initial_winning_block); // Fallback to same block if not found

        game_session.winning_block = initial_winning_block;
        game_session.winning_faction_id = winning_faction_id;
        game_session.same_faction_other_block = same_faction_other_block;
        msg!(
            "   🎯 Winning block selected: {} (Faction: {}). Same-faction other block: {}",
            initial_winning_block,
            winning_faction_id,
            same_faction_other_block
        );

        // Update global state
        global_state.last_round_id = game_session.round_id;
        global_state.winning_faction_id = winning_faction_id;
        global_state.total_sol_bets =
            global_state.total_sol_bets + (game_session.total_sol_bets as u128);
        global_state.can_begin_round = true;
        msg!("   Global state updated: last_round_id: {}, winning_faction_id: {}, total_sol_bets: {}, can_begin_round: {}", global_state.last_round_id, global_state.winning_faction_id, global_state.total_sol_bets, global_state.can_begin_round);

        // Skip to stage 2 (no rewards to claim, round is complete)
        game_session.stage = 2;

        msg!(
            "   ✅ Round {} finished with no users/bets - skipping to stage 2",
            game_session.round_id
        );

        emit!(RoundEnded {
            round_id: game_session.round_id,
            game_session: game_session.key(),
            winning_block: initial_winning_block,
            winning_faction_id,
            same_faction_other_block,
            total_sol_bets: game_session.total_sol_bets,
            total_points_bets: game_session.total_points_bets,
            user_bets_count: game_session.user_block_indexes.clone(),
            block_sol_bets: game_session.sol_bets_indexes.clone(),
            block_points: game_session.points_bets_indexes.clone(),
            block_wgtd_points: game_session.wgtd_points_bets_indexes.clone(),
            minebtc_winner_pool: 0,
            minebtc_same_faction_pool: 0,
            minebtc_faction_stakers: 0,
            minebtc_motherlode: 0,
            motherlode_hit: false,
            timestamp: clock.unix_timestamp,
        });

        return Ok(());
    }

    // Select initial winning block (0-23) using final hash
    let initial_winning_block = (u64::from_le_bytes([
        final_hash_bytes[0],
        final_hash_bytes[1],
        final_hash_bytes[2],
        final_hash_bytes[3],
        0,
        0,
        0,
        0,
    ]) % NUM_BLOCKS as u64) as u8; // 0-indexed blocks
    msg!(
        "   Initial winning block from hash: {}",
        initial_winning_block
    );

    // Find a valid winning block (must have at least 1 user who bet on it, 0-indexed: 0-23)
    let winning_block =
        find_valid_winning_block(initial_winning_block, &game_session.user_block_indexes)?;
    msg!(
        "   Valid winning block selected: {} (has {} users)",
        winning_block,
        game_session.user_block_indexes[winning_block as usize]
    );

    msg!(
        "   Finalized winning block: {} (has {} users, {} points)",
        winning_block,
        game_session.user_block_indexes[winning_block as usize],
        game_session.points_bets_indexes[winning_block as usize]
    );

    // Get winning faction from block assignments (0-indexed: 0-23)
    let winning_faction_id = game_session.block_assignments[winning_block as usize];

    // Find the other block with the same faction (0-indexed: 0-23)
    let same_faction_other_block = game_session
        .block_assignments
        .iter()
        .enumerate()
        .find(|(idx, &faction)| faction == winning_faction_id && *idx != winning_block as usize)
        .map(|(idx, _)| idx as u8)
        .ok_or(ErrorCode::InvalidParameters)?; // Should always find the other block

    game_session.winning_block = winning_block;
    game_session.winning_faction_id = winning_faction_id;
    game_session.same_faction_other_block = same_faction_other_block;
    msg!(
        "   🎯 Winning block selected: {} (Faction: {}). Same-faction other block: {}",
        winning_block,
        winning_faction_id,
        same_faction_other_block
    );

    // ================= REWARDS CALCULATION ======================

    // Calculate MineBtc emission for this round
    let minebtc_rewards = mine_btc_mining.mine_btc_per_round;
    msg!("   Current dist rate: {}", minebtc_rewards);

    // Calculate MineBtc distribution pools according to ORE tokenomics
    msg!("   Calculating MineBtc distribution pools...");
    let (winning_block_rewards, same_faction_rewards, faction_stakers, motherlode_rewards) =
        calculate_minebtc_split(
            minebtc_rewards,
            global_config.minebtc_dist_config.minebtc_stakers_pct,
            global_config.minebtc_dist_config.minebtc_winners_pct,
            global_config.minebtc_dist_config.minebtc_same_faction_pct,
            global_config.minebtc_dist_config.minebtc_motherlode_pct,
        );
    game_session.minebtc_winner_pool = winning_block_rewards;
    game_session.minebtc_loser_pool = same_faction_rewards;
    game_session.faction_stakers = faction_stakers;
    game_session.motherlode_rewards = motherlode_rewards;
    msg!("   Set MineBtc pools: winner_pool={}, same_faction_pool={}, faction_stakers={}, motherlode_pool={}", winning_block_rewards, same_faction_rewards, faction_stakers, motherlode_rewards);

    let mine_btc_mining_mut = &mut ctx.accounts.mine_btc_mining;
    let total_distributed_this_round =
        winning_block_rewards + same_faction_rewards + faction_stakers + motherlode_rewards;
    mine_btc_mining_mut.total_tokens_mined += total_distributed_this_round;
    msg!(
        "   Updated MineBtcMining.total_tokens_mined: {} (+{} this round)",
        mine_btc_mining_mut.total_tokens_mined,
        total_distributed_this_round
    );

    // Calculate SOL rewards index --> rewards = user's points * sol_rewards_index / INDEX_PRECISION
    let winning_block_pts = game_session.points_bets_indexes[winning_block as usize];
    let winning_block_wgtd_pts = game_session.wgtd_points_bets_indexes[winning_block as usize];
    msg!(
        "   Winning block points: {}, wgtd_points: {}",
        winning_block_pts,
        winning_block_wgtd_pts
    );

    if winning_block_pts > 0 {
        // SOL rewards: use regular points
        let sol_rewards_delta = helper::mul_div(
            game_session.total_sol_bets,
            INDEX_PRECISION,
            winning_block_pts,
        )?;
        game_session.sol_rewards_index = game_session.sol_rewards_index + sol_rewards_delta;
        msg!("   SOL rewards index: {}", game_session.sol_rewards_index);

        // MineBtc rewards: use wgtd_points (multiplier-weighted)
        if winning_block_wgtd_pts > 0 {
            let minebtc_rewards_delta = helper::mul_div(
                winning_block_rewards,
                INDEX_PRECISION,
                winning_block_wgtd_pts,
            )?;
            game_session.minebtc_rewards_index =
                game_session.minebtc_rewards_index + minebtc_rewards_delta;
            msg!(
                "   MineBtc rewards index (winning block): {}",
                game_session.minebtc_rewards_index
            );
        }

        // Same-faction block: use wgtd_points for MineBtc
        let same_faction_wgtd_pts =
            game_session.wgtd_points_bets_indexes[same_faction_other_block as usize];
        if same_faction_wgtd_pts > 0 {
            let same_faction_minebtc_delta =
                helper::mul_div(same_faction_rewards, INDEX_PRECISION, same_faction_wgtd_pts)?;
            game_session.same_faction_minebtc_rewards_index =
                game_session.same_faction_minebtc_rewards_index + same_faction_minebtc_delta;
            msg!(
                "   MineBtc rewards index (same-faction): {}",
                game_session.same_faction_minebtc_rewards_index
            );
        } else {
            msg!(
                "   ⚠️ No wgtd_points on same-faction block {}, distributing to winning block",
                same_faction_other_block
            );
            if winning_block_wgtd_pts > 0 {
                let minebtc_rewards_delta = helper::mul_div(
                    same_faction_rewards,
                    INDEX_PRECISION,
                    winning_block_wgtd_pts,
                )?;
                game_session.minebtc_rewards_index =
                    game_session.minebtc_rewards_index + minebtc_rewards_delta;
                msg!(
                    "   MineBtc rewards index (winning block, +same-faction): {}",
                    game_session.minebtc_rewards_index
                );
            }
        }
    } else {
        msg!(
            "   ⚠️ No points bet on winning block {}, skipping reward index calculations",
            winning_block
        );
    }

    // Check for motherlode (random chance)
    msg!(
        "   Checking for motherlode hit (1 in {} chance)...",
        MOTHERLODE_CHANCE
    );
    let motherlode_random = u64::from_le_bytes([
        final_hash_bytes[4],
        final_hash_bytes[5],
        final_hash_bytes[6],
        final_hash_bytes[7],
        0,
        0,
        0,
        0,
    ]) % MOTHERLODE_CHANCE;
    let motherlode_hit = motherlode_random == 0;

    game_session.motherlode_hit = motherlode_hit;

    game_session.stage = 1;
    msg!(
        "✅ [end_round] Round {} ended successfully",
        game_session.round_id
    );

    emit!(RoundEnded {
        round_id: game_session.round_id,
        game_session: game_session.key(),
        winning_block,
        winning_faction_id,
        same_faction_other_block,
        total_sol_bets: game_session.total_sol_bets,
        total_points_bets: game_session.total_points_bets,

        user_bets_count: game_session.user_block_indexes.clone(),
        block_sol_bets: game_session.sol_bets_indexes.clone(),
        block_points: game_session.points_bets_indexes.clone(),
        block_wgtd_points: game_session.wgtd_points_bets_indexes.clone(),

        minebtc_winner_pool: winning_block_rewards,
        minebtc_same_faction_pool: same_faction_rewards,
        minebtc_faction_stakers: faction_stakers,
        minebtc_motherlode: motherlode_rewards,
        motherlode_hit,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Find a valid winning block that has at least 1 user who bet on it
/// Starts from the initial_block (0-indexed: 0-23) and decrements until finding a block with users
/// Wraps around if needed (23 -> 22 -> ... -> 0 -> 23 -> ...)
/// Returns 0-indexed block number (0-23)
fn find_valid_winning_block(initial_block: u8, user_block_indexes: &[u64]) -> Result<u8> {
    msg!(
        "   Finding valid winning block starting from block {}...",
        initial_block
    );

    // Start from initial block and check if it has users (0-indexed: 0-23)
    let mut current_block = initial_block;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u8 = NUM_BLOCKS as u8; // Maximum attempts = number of blocks

    loop {
        if attempts >= MAX_ATTEMPTS {
            msg!(
                "   ✗ No block found with users after checking all {} blocks",
                MAX_ATTEMPTS
            );
            return Err(ErrorCode::InvalidParameters.into());
        }

        let block_index = current_block as usize;
        let user_count = user_block_indexes[block_index];

        if user_count > 0 {
            msg!(
                "   ✓ Found valid block {} with {} users",
                current_block,
                user_count
            );
            return Ok(current_block);
        }

        msg!(
            "   Block {} has no users, trying next block...",
            current_block
        );

        // Decrement block (wrap around: 0 -> 23)
        if current_block == 0 {
            current_block = (NUM_BLOCKS - 1) as u8;
        } else {
            current_block -= 1;
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
    let winning_block_rewards =
        (minebtc_rewards as u128 * minebtc_winners_pct as u128 / 100) as u64;
    msg!(
        "     Winners ({}%): {}",
        minebtc_winners_pct,
        winning_block_rewards
    );

    let same_faction_rewards =
        (minebtc_rewards as u128 * minebtc_same_faction_pct as u128 / 100) as u64;
    msg!(
        "     Same-faction ({}%): {}",
        minebtc_same_faction_pct,
        same_faction_rewards
    );

    let faction_stakers = (minebtc_rewards as u128 * minebtc_stakers_pct as u128 / 100) as u64;
    msg!(
        "     Stakers ({}%): {}",
        minebtc_stakers_pct,
        faction_stakers
    );

    let motherlode_rewards =
        (minebtc_rewards as u128 * minebtc_motherlode_pct as u128 / 100) as u64;
    msg!(
        "     Motherlode ({}%): {}",
        minebtc_motherlode_pct,
        motherlode_rewards
    );

    (
        winning_block_rewards,
        same_faction_rewards,
        faction_stakers,
        motherlode_rewards,
    )
}

/// End the current round by revealing seed, selecting winner, and starting next round
/// This function:
/// 1. Verifies revealed seed matches commit hash
/// 2. Generates final randomness using seed + blockhash
/// 3. Selects winning block
/// 4. Calculates winners and updates payout data
/// 5. Commits hash for next round
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

    // Get winning faction from block assignments
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

    // boolean to check if motherlode was hit
    let motherlode_hit = game_session.motherlode_hit;
    let winning_block_wgtd_pts =
        game_session.wgtd_points_bets_indexes[game_session.winning_block as usize];
    let same_faction_wgtd_pts =
        game_session.wgtd_points_bets_indexes[game_session.same_faction_other_block as usize];
    msg!(
        "   Winning block weighted points: {}, Same-faction weighted points: {}",
        winning_block_wgtd_pts,
        same_faction_wgtd_pts
    );

    // -----------------------------------
    // MOTHERLODE HIT LOGIC
    // -----------------------------------
    if motherlode_hit && faction_state.motherlode_pot_size > 0 {
        msg!("   🎰 MOTHERLODE HIT!");

        let mut wining_block_rewards = 0;
        let mut same_faction_rewards = 0;

        if game_session.minebtc_winner_pool > 0 && game_session.minebtc_loser_pool > 0 {
            wining_block_rewards = faction_state.motherlode_pot_size / 2;
            same_faction_rewards = faction_state.motherlode_pot_size - wining_block_rewards;
            faction_state.motherlode_pot_size = 0;
        } else if game_session.minebtc_winner_pool > 0 {
            wining_block_rewards = faction_state.motherlode_pot_size;
            faction_state.motherlode_pot_size = 0;
        } else if game_session.minebtc_loser_pool > 0 {
            same_faction_rewards = faction_state.motherlode_pot_size;
            faction_state.motherlode_pot_size = 0;
        }
        msg!(
            "   Motherlode split: winner block: {} MINEBTC, same_faction block: {} MINEBTC",
            (wining_block_rewards as f64 / 1_000_000.0),
            (same_faction_rewards as f64 / 1_000_000.0)
        );

        game_session.minebtc_winner_pool = game_session.minebtc_winner_pool + wining_block_rewards;
        game_session.minebtc_loser_pool = game_session.minebtc_loser_pool + same_faction_rewards;

        // Record the full motherlode pot size that was hit
        game_session.motherlode_pot_size_on_hit = wining_block_rewards + same_faction_rewards;
        msg!(
            "   🎰 Motherlode pot size on hit: {}",
            game_session.motherlode_pot_size_on_hit
        );

        // Update reward indexes with motherlode split (only if there are points bet)
        if wining_block_rewards > 0 {
            let minebtc_rewards_delta = helper::mul_div(
                wining_block_rewards,
                INDEX_PRECISION,
                winning_block_wgtd_pts,
            )?;
            game_session.minebtc_rewards_index += minebtc_rewards_delta;
            msg!("   MineBtc rewards index (winning block) updated with motherlode: +{}. (Total: {})", minebtc_rewards_delta, game_session.minebtc_rewards_index);
        }

        if same_faction_rewards > 0 {
            let minebtc_rewards_delta =
                helper::mul_div(same_faction_rewards, INDEX_PRECISION, same_faction_wgtd_pts)?;
            game_session.same_faction_minebtc_rewards_index += minebtc_rewards_delta;
            msg!(
                "   MineBtc rewards index (same-faction) updated with motherlode: +{}. (Total: {})",
                minebtc_rewards_delta,
                game_session.same_faction_minebtc_rewards_index
            );
        }

        emit!(MotherlodeHit {
            round_id: game_session.round_id,
            faction_id: faction_state.faction_id,
            wining_block_rewards: wining_block_rewards,
            same_faction_rewards: same_faction_rewards,
            minebtc_rewards_index: game_session.minebtc_rewards_index,
            same_faction_minebtc_rewards_index: game_session.same_faction_minebtc_rewards_index
        });
    } else {
        msg!("   Motherlode miss.");
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

    if epoch_config.is_active && epoch_state.stage < 2 {
        epoch_state.total_dogebtc_mined_in_epoch += mine_btc_per_round;
        msg!(
            "   🌍 Epoch {}: +{} dogeBTC mined (total: {})",
            epoch_state.epoch_id,
            mine_btc_per_round,
            epoch_state.total_dogebtc_mined_in_epoch
        );

        // --- AUTO-SETTLE if epoch expired and scores are posted (stage == 1) ---
        let clock = Clock::get()?;
        if clock.unix_timestamp >= epoch_state.end_timestamp as i64 && epoch_state.stage == 1 {
            // Settle: compute pool
            epoch_state.risk_factor_snapshot = epoch_config.risk_factor;
            epoch_state.epoch_mining_pool = (epoch_state.total_dogebtc_mined_in_epoch as u128)
                .checked_mul(epoch_state.risk_factor_snapshot as u128)
                .unwrap_or(0)
                .checked_div(100)
                .unwrap_or(0) as u64;

            // Compute faction reward pools (Model 5 + Top 3)
            crate::instructions::epoch::compute_faction_reward_pools(epoch_state, epoch_config);

            epoch_state.stage = 2; // settled

            msg!(
                "   🌍 Auto-settled epoch {}: pool={}",
                epoch_state.epoch_id,
                epoch_state.epoch_mining_pool
            );

            emit!(EpochAutoSettled {
                epoch_id: epoch_state.epoch_id,
                mining_pool: epoch_state.epoch_mining_pool,
                total_weighted_bets: 0, // deprecated, kept for event compat
            });

            // --- AUTO-START next epoch ---
            epoch_config.current_epoch_id += 1;
            epoch_config.last_epoch_start = clock.unix_timestamp.max(0) as u64;

            msg!(
                "   🌍 Next epoch_id set to {}",
                epoch_config.current_epoch_id
            );
        }
    }

    emit!(RewardsDistributedForRound {
        round_id: game_session.round_id,
    });

    msg!(
        "✅ [end_round] Round {} ended successfully",
        game_session.round_id
    );
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

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}
