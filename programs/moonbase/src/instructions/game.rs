use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::solana_program::sysvar::Sysvar;
use anchor_lang::AccountDeserialize;
use anchor_lang::AccountSerialize;

use crate::errors::ErrorCode;
use crate::state::*;
use crate::instructions::helper;

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
/// For the first round, use next_round_commit from global_state (set during initialization).
pub fn start_round(
    ctx: Context<StartRound>,
    round_id: u64,
    commit_hash: Option<[u8; 32]>, // Optional: if None, uses next_round_commit from global_state
) -> Result<()> {
    msg!("🎮 [start_round] Starting new round");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    
    msg!("   Current round ID: {}", global_state.current_round_id);
    msg!("   Requested round ID: {}", round_id);
    msg!("   Current timestamp: {}", clock.unix_timestamp);
    msg!("   Round end timestamp: {}", global_state.round_end_timestamp);
    
    // Validate round_id matches expected value (current_round_id + 1)
    let expected_round_id = global_state.current_round_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    require!(
        round_id == expected_round_id,
        ErrorCode::InvalidRound
    );
    msg!("   ✓ Round ID validated: {} (expected: {})", round_id, expected_round_id);
    
    // Validate caller is a whitelisted cranker bot
    msg!("   Validating cranker bot authorization...");
    require!(
        global_state.cranker_bots.contains(&ctx.accounts.authority.key()),
        ErrorCode::Unauthorized
    );
    msg!("     ✓ Caller is whitelisted cranker bot");
    
    // Validate game is active
    require!(global_state.is_active, ErrorCode::InvalidParameters);
    msg!("   ✓ Game is active");
    
    // Validate that previous round has ended (if not first round)
    if global_state.current_round_id > 0 {
        require!(
            clock.unix_timestamp >= global_state.round_end_timestamp,
            ErrorCode::RoundNotEnded
        );
        msg!("   ✓ Previous round has ended");
    } else {
        msg!("   ✓ First round - no previous round to check");
    }
    
    // Use provided commit_hash or next_round_commit from global_state
    let commit = commit_hash.unwrap_or(global_state.next_round_commit);
    if commit_hash.is_some() {
        msg!("   Using provided commit hash");
    } else {
        msg!("   Using next_round_commit from global_state");
    }
    
    // Set commit hash for this round
    global_state.current_round_commit = commit;
    global_state.current_round_seed = None; // Will be set in end_round
    msg!("   Commit hash set for round {}", round_id);

    // Update global state
    global_state.current_round_id = round_id;
    global_state.round_end_timestamp = game_session.round_end_timestamp;
    msg!("   Global state updated: current_round_id={}, round_end_timestamp={}", round_id, game_session.round_end_timestamp);

    // Initialize GameSession for the new round
    msg!("   Initializing GameSession for round {}", round_id);
    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = round_id;
    game_session.round_start_timestamp = clock.unix_timestamp;
    game_session.round_end_timestamp = clock.unix_timestamp
        .checked_add(global_state.round_duration_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_sol_bets = 0;
    game_session.total_points_bets = 0;
    
    // Initialize 24-sized arrays for block tracking
    game_session.user_block_indexes = vec![0u64; NUM_BLOCKS];
    game_session.sol_bets_indexes = vec![0u64; NUM_BLOCKS];
    game_session.points_bets_indexes = vec![0u64; NUM_BLOCKS];
    msg!("   Initialized block tracking arrays (24 blocks)");
    msg!("   Round duration: {} seconds", global_state.round_duration_seconds);
    msg!("   Round starts at: {}", game_session.round_start_timestamp);
    msg!("   Round ends at: {}", game_session.round_end_timestamp);
    
    // Randomly assign 12 factions to 24 blocks (2 blocks per faction)
    // Use commit hash + slot for deterministic but unpredictable randomness
    msg!("   Assigning {} factions to {} blocks ({} blocks per faction)", NUM_FACTIONS, NUM_BLOCKS, BLOCKS_PER_FACTION);
    let mut block_assignments = [0u8; NUM_BLOCKS];
    let mut faction_blocks_assigned = [0u8; NUM_FACTIONS]; // Track blocks assigned per faction
    
    // Create seed for randomness: commit_hash + current_slot
    let mut random_seed = Vec::new();
    random_seed.extend_from_slice(&commit);
    random_seed.extend_from_slice(&clock.slot.to_le_bytes());
    let hash = keccak::hash(&random_seed);
    let hash_bytes = hash.to_bytes();
    msg!("   Generated randomness seed from commit hash + slot {}", clock.slot);
    
    // Assign factions to blocks
    let mut hash_offset = 0;
    for block_idx in 0..NUM_BLOCKS {
        // Find a faction that hasn't been assigned 2 blocks yet
        let mut attempts = 0;
        loop {
            if attempts >= 100 {
                msg!("   ✗ Failed to assign faction to block {} after 100 attempts", block_idx + 1);
                return Err(ErrorCode::InvalidParameters.into()); // Safety check
            }
            
            // Use hash bytes for randomness
            let random_byte = hash_bytes[hash_offset % 32];
            let faction_id = (random_byte % NUM_FACTIONS as u8) as usize;
            
            if faction_blocks_assigned[faction_id] < BLOCKS_PER_FACTION as u8 {
                block_assignments[block_idx] = faction_id as u8;
                faction_blocks_assigned[faction_id] += 1;
                hash_offset += 1;
                break;
            }
            
            hash_offset += 1;
            attempts += 1;
        }
    }
    
    // Verify all factions got exactly 2 blocks
    msg!("   Verifying block assignments...");
    for (faction_idx, &count) in faction_blocks_assigned.iter().enumerate() {
        require!(count == BLOCKS_PER_FACTION as u8, ErrorCode::InvalidParameters);
        msg!("     Faction {}: {} blocks assigned", faction_idx, count);
    }
    msg!("   ✓ All factions assigned exactly {} blocks", BLOCKS_PER_FACTION);
    
    game_session.block_assignments = block_assignments;
    game_session.winning_block = 0; // Will be set in end_round
    game_session.winning_faction_id = 0; // Will be set in end_round
    game_session.same_faction_other_block = 0; // Will be set in end_round
    
    // Initialize reward indexes
    game_session.sol_rewards_index = 0;
    game_session.dbtc_rewards_index = 0;
    game_session.same_faction_dbtc_rewards_index = 0;
    
    // Initialize DogeBtc pools
    game_session.dbtc_winner_pool = 0;
    game_session.dbtc_loser_pool = 0;
    
    // Initialize motherlode fields
    game_session.motherlode_hit_faction_id = 0;
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;
    
    
    msg!("✅ [start_round] Round {} started successfully", round_id);
    msg!("   Commit hash: {:?}", commit);
    msg!("   Round ends at timestamp: {}", game_session.round_end_timestamp);
    
    Ok(())
}


/// End the current round by revealing seed, selecting winner, and starting next round
/// This function:
/// 1. Verifies revealed seed matches commit hash
/// 2. Generates final randomness using seed + blockhash
/// 3. Selects winning block
/// 4. Calculates winners and updates payout data
/// 5. Commits hash for next round
pub fn end_round(
    ctx: Context<EndRound>,
    revealed_seed: [u8; 32], // The secret seed that was hashed to create commit_hash
    next_round_commit: [u8; 32], // hash(secret_seed_for_next_round)
) -> Result<()> {
    msg!("🏁 [end_round] Ending current round");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let global_config = &ctx.accounts.global_config;
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;
    let clock = Clock::get()?;
    
    // Validate round has ended
    require!( clock.unix_timestamp >= game_session.round_end_timestamp,ErrorCode::RoundNotEnded);
    msg!("   ✓ Round has ended");

    msg!("   Current round ID: {}", game_session.round_id);
    msg!("   Current timestamp: {}", clock.unix_timestamp);
    msg!("   Round end timestamp: {}", game_session.round_end_timestamp);
    
    // Validate caller is a whitelisted cranker bot
    msg!("   Validating cranker bot authorization...");
    require!( global_state.cranker_bots.contains(&ctx.accounts.authority.key()), ErrorCode::Unauthorized);
    msg!("     ✓ Caller is whitelisted cranker bot");
        
    // Verify commit-reveal: hash(revealed_seed) must equal current_round_commit
    msg!("   Verifying commit-reveal scheme...");
    let seed_hash = keccak::hash(&revealed_seed);
    let seed_hash_bytes = seed_hash.to_bytes();
    msg!("   Revealed seed hash: {:?}", seed_hash_bytes);
    msg!("   Expected commit hash: {:?}", global_state.current_round_commit);
    require!(
        seed_hash_bytes == global_state.current_round_commit,
        ErrorCode::InvalidParameters
    );
    msg!("   ✓ Commit-reveal verification passed");
    
    // Store revealed seed
    global_state.current_round_seed = Some(revealed_seed);
    msg!("   Stored revealed seed for round {}", game_session.round_id);
    
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
    msg!("   Final hash generated from seed + slot {} + timestamp {}", clock.slot, clock.unix_timestamp);
    
    // Select initial winning block (1-24) using final hash
    let initial_winning_block = ((u64::from_le_bytes([
        final_hash_bytes[0],
        final_hash_bytes[1],
        final_hash_bytes[2],
        final_hash_bytes[3],
        0, 0, 0, 0,
    ]) % NUM_BLOCKS as u64) + 1) as u8; // 1-indexed blocks
    msg!("   Initial winning block from hash: {}", initial_winning_block);
    
    // Find a valid winning block (must have at least 1 user who bet on it)
    let winning_block = find_valid_winning_block(initial_winning_block, &game_session.user_block_indexes)?;
    msg!("   Valid winning block selected: {} (has {} users)", winning_block, game_session.user_block_indexes[(winning_block - 1) as usize]);
    
    // Get winning faction from block assignments
    let winning_faction_id = game_session.block_assignments[(winning_block - 1) as usize];
    
    // Find the other block with the same faction (0-indexed, convert to 1-indexed)
    let same_faction_other_block = game_session.block_assignments
        .iter()
        .enumerate()
        .find(|(idx, &faction)| faction == winning_faction_id && (*idx + 1) != winning_block as usize)
        .map(|(idx, _)| (idx + 1) as u8)
        .ok_or(ErrorCode::InvalidParameters)?; // Should always find the other block
    
    game_session.winning_block = winning_block;
    game_session.winning_faction_id = winning_faction_id;
    game_session.same_faction_other_block = same_faction_other_block;
    msg!("   🎯 Winning block selected: {} (Faction: {})", winning_block, winning_faction_id);
    msg!("   Same-faction other block: {}", same_faction_other_block);
    
    // Calculate DogeBtc emission for this round
    let dbtc_rewards = doge_btc_mining.current_dist_rate;
    msg!("   Calculating DogeBtc emission for round...");
    msg!("   Current dist rate: {}", dbtc_rewards);
    
    // Calculate DogeBtc distribution pools according to ORE tokenomics
    msg!("   Calculating DogeBtc distribution pools...");
    let (winning_block_rewards, same_faction_rewards, faction_stakers, motherlode_rewards) = calculate_dbtc_split(
        dbtc_rewards,
        global_config.dbtc_dist_config.dbtc_stakers_pct,
        global_config.dbtc_dist_config.dbtc_winners_pct,
        global_config.dbtc_dist_config.dbtc_same_faction_pct,
        global_config.dbtc_dist_config.dbtc_motherlode_pct
    );
    game_session.dbtc_winner_pool = winning_block_rewards;
    game_session.dbtc_loser_pool = same_faction_rewards;
    msg!("   Set DogeBtc pools: winner_pool={}, same_faction_pool={}", winning_block_rewards, same_faction_rewards);
    
    // Update total_tokens_mined in DogeBtcMining account
    // This tracks cumulative tokens distributed across all rounds
    let doge_btc_mining_mut = &mut ctx.accounts.doge_btc_mining;
    let old_total_mined = doge_btc_mining_mut.total_tokens_mined;
    let total_distributed_this_round = winning_block_rewards
        .checked_add(same_faction_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(faction_stakers)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_add(motherlode_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    doge_btc_mining_mut.total_tokens_mined = doge_btc_mining_mut.total_tokens_mined
        .checked_add(total_distributed_this_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Updated DogeBtcMining.total_tokens_mined: {} -> {} (+{} this round)", 
        old_total_mined, 
        doge_btc_mining_mut.total_tokens_mined,
        total_distributed_this_round
    );
    
    // Deserialize winning faction state
    msg!("   Loading winning faction state (faction {})...", winning_faction_id);
    let mut faction_state_data = ctx.accounts.winning_faction_state.try_borrow_mut_data()?;
    let mut faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_state_data[..])?;
    
    // Validate faction_id matches winning faction
    require!(
        faction_state.faction_id == winning_faction_id,
        ErrorCode::InvalidFactionId
    );
    msg!("   ✓ Faction state matches winning faction");
    
    let old_wins = faction_state.total_wins;
    let old_motherlode = faction_state.motherlode_pot_size;
    
    // Calculate SOL rewards index --> basically rewards claimable by a user = his points * sol_rewards_index / INDEX_PRECISION;
    let winning_block_pts = game_session.points_bets_indexes[(winning_block - 1) as usize];
    msg!("   Winning block points: {}", winning_block_pts);
    
    if winning_block_pts > 0 {
        // Calculate reward indexes (only if there are points bet on winning block)
        let sol_rewards_delta = helper::mul_div(game_session.total_sol_bets, INDEX_PRECISION, winning_block_pts)?;        
        game_session.sol_rewards_index = game_session.sol_rewards_index+ sol_rewards_delta;
        msg!("   SOL rewards index: {} -> {} (+{})", game_session.sol_rewards_index - sol_rewards_delta, game_session.sol_rewards_index, sol_rewards_delta);
        
        // Calculate DogeBtc rewards index for winning block
        let dbtc_rewards_delta = helper::mul_div(winning_block_rewards, INDEX_PRECISION, winning_block_pts)?;
        game_session.dbtc_rewards_index = game_session.dbtc_rewards_index + dbtc_rewards_delta;
        msg!("   DogeBtc rewards index (winning block): {} -> {} (+{})", game_session.dbtc_rewards_index - dbtc_rewards_delta, game_session.dbtc_rewards_index, dbtc_rewards_delta);
        
        // Calculate DogeBtc rewards index for same-faction other block
        let same_faction_pts = game_session.points_bets_indexes[(same_faction_other_block - 1) as usize];
        if same_faction_pts > 0 {
            let same_faction_dbtc_delta = helper::mul_div(same_faction_rewards, INDEX_PRECISION, same_faction_pts)?;
            game_session.same_faction_dbtc_rewards_index = game_session.same_faction_dbtc_rewards_index + same_faction_dbtc_delta;
            msg!("   DogeBtc rewards index (same-faction): {} -> {} (+{})", game_session.same_faction_dbtc_rewards_index - same_faction_dbtc_delta, game_session.same_faction_dbtc_rewards_index, same_faction_dbtc_delta);
        } else {
            msg!("   ⚠️ No points bet on same-faction block {}, skipping same-faction rewards index", same_faction_other_block);
        }
    } else {
        msg!("   ⚠️ No points bet on winning block {}, skipping reward index calculations", winning_block);
    }
    
    // dBTC distribution to stakers of the winning faction
    if faction_state.total_dbtc_hashpower > 0 {
        let dbtc_per_share = helper::mul_div(faction_stakers / 2, INDEX_PRECISION, faction_state.total_dbtc_hashpower)?;
        faction_state.dbtc_dbtc_reward_index = faction_state.dbtc_dbtc_reward_index + dbtc_per_share;
        msg!("   Faction stakers reward index: {} -> {} (+{})", faction_state.dbtc_dbtc_reward_index - dbtc_per_share, faction_state.dbtc_dbtc_reward_index, dbtc_per_share);
    }

    // LP distribution to stakers of the winning faction
    if faction_state.total_lp_hashpower > 0 {
        let lp_per_share = helper::mul_div(faction_stakers / 2, INDEX_PRECISION, faction_state.total_lp_hashpower)?;
        faction_state.lp_dbtc_reward_index = faction_state.lp_dbtc_reward_index + lp_per_share;
        msg!("   Faction stakers reward index: {} -> {} (+{})", faction_state.lp_dbtc_reward_index - lp_per_share, faction_state.lp_dbtc_reward_index, lp_per_share);
    }
    
    // Increment motherlode pot size (always, regardless of hit)    
    faction_state.motherlode_pot_size = faction_state.motherlode_pot_size + motherlode_rewards;
    msg!("   Motherlode pot: {} -> {} (+{})", old_motherlode, faction_state.motherlode_pot_size, motherlode_rewards);


    // Check for motherlode (random chance)
    msg!("   Checking for motherlode hit (1 in {} chance)...", MOTHERLODE_CHANCE);
    let motherlode_random = u64::from_le_bytes([
        final_hash_bytes[4],
        final_hash_bytes[5],
        final_hash_bytes[6],
        final_hash_bytes[7],
        0, 0, 0, 0,
    ]) % MOTHERLODE_CHANCE;
    let motherlode_hit = motherlode_random == 0;
    
    game_session.motherlode_hit = motherlode_hit;
    game_session.motherlode_hit_faction_id = winning_faction_id;
    
    if motherlode_hit {
        msg!("   🎰 MOTHERLODE HIT! Random value: {} (mod {})", motherlode_random, MOTHERLODE_CHANCE);
        
        // Split motherlode pot between winners and same-faction bettors
        let motherlode_split = motherlode_rewards / 2;
        let old_winner_pool = game_session.dbtc_winner_pool;
        let old_loser_pool = game_session.dbtc_loser_pool;
        
        game_session.dbtc_winner_pool = game_session.dbtc_winner_pool
            .checked_add(motherlode_split)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        game_session.dbtc_loser_pool = game_session.dbtc_loser_pool
            .checked_add(motherlode_split)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("   DogeBtc pools updated: winner {} -> {}, same_faction {} -> {}", 
            old_winner_pool, game_session.dbtc_winner_pool,
            old_loser_pool, game_session.dbtc_loser_pool);
        
        // Record the full motherlode pot size that was hit
        game_session.motherlode_pot_size_on_hit = faction_state.motherlode_pot_size;
        msg!("   🎰 Motherlode pot size on hit: {}", game_session.motherlode_pot_size_on_hit);
        
        // Update reward indexes with motherlode split (only if there are points bet)
        if winning_block_pts > 0 {
            let motherlode_winner_delta = (motherlode_split as u128)
                .checked_mul(INDEX_PRECISION as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(winning_block_pts as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            game_session.dbtc_rewards_index = game_session.dbtc_rewards_index
                .checked_add(motherlode_winner_delta)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!("   DogeBtc rewards index (winning block) updated with motherlode: +{}", motherlode_winner_delta);
            
            let same_faction_pts = game_session.points_bets_indexes[(same_faction_other_block - 1) as usize];
            if same_faction_pts > 0 {
                let motherlode_same_faction_delta = (motherlode_split as u128)
                    .checked_mul(INDEX_PRECISION as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?
                    .checked_div(same_faction_pts as u128)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                game_session.same_faction_dbtc_rewards_index = game_session.same_faction_dbtc_rewards_index
                    .checked_add(motherlode_same_faction_delta)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                msg!("   DogeBtc rewards index (same-faction) updated with motherlode: +{}", motherlode_same_faction_delta);
            }
        }
    } else {
        msg!("   Motherlode miss. Random value: {} (mod {})", motherlode_random, MOTHERLODE_CHANCE);
        game_session.motherlode_pot_size_on_hit = 0;
    }
    
    // Update faction wins
    faction_state.total_wins = faction_state.total_wins
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Faction wins: {} -> {}", old_wins, faction_state.total_wins);
    
    // Serialize updated faction state back
    faction_state.try_serialize(&mut &mut faction_state_data[..])?;
    msg!("   ✓ Faction state updated and serialized");
        
    // Update global state with previous round results
    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;
    
    // Update total SOL bets in global state (cumulative)
    let old_global_total = global_state.total_sol_bets;
    global_state.total_sol_bets = global_state.total_sol_bets
        .checked_add(game_session.total_sol_bets as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Updated global state:");
    msg!("     last_round_id: {}", global_state.last_round_id);
    msg!("     winning_faction_id: {}", global_state.winning_faction_id);
    msg!("     total_sol_bets: {} -> {} (+{})", old_global_total, global_state.total_sol_bets, game_session.total_sol_bets);
    
    // Set commit hash for next round
    global_state.next_round_commit = next_round_commit;
    msg!("   Set next round commit hash");
        
    msg!("✅ [end_round] Round {} ended successfully", game_session.round_id);
    msg!("   Winning block: {}, Winning faction: {}, Motherlode: {}",
        winning_block,
        winning_faction_id,
        if motherlode_hit { "HIT!" } else { "miss" }
    );
    
    Ok(())
}




/// Find a valid winning block that has at least 1 user who bet on it
/// Starts from the initial_block and decrements until finding a block with users
/// Wraps around if needed (24 -> 23 -> ... -> 1 -> 24 -> ...)
fn find_valid_winning_block(initial_block: u8, user_block_indexes: &[u64]) -> Result<u8> {
    msg!("   Finding valid winning block starting from block {}...", initial_block);
    
    // Start from initial block and check if it has users
    let mut current_block = initial_block;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u8 = NUM_BLOCKS as u8; // Maximum attempts = number of blocks
    
    loop {
        if attempts >= MAX_ATTEMPTS {
            msg!("   ✗ No block found with users after checking all {} blocks", MAX_ATTEMPTS);
            return Err(ErrorCode::InvalidParameters.into());
        }
        
        let block_index = (current_block - 1) as usize;
        let user_count = user_block_indexes[block_index];
        
        if user_count > 0 {
            msg!("   ✓ Found valid block {} with {} users", current_block, user_count);
            return Ok(current_block);
        }
        
        msg!("   Block {} has no users, trying next block...", current_block);
        
        // Decrement block (wrap around: 1 -> 24)
        if current_block == 1 {
            current_block = NUM_BLOCKS as u8;
        } else {
            current_block -= 1;
        }
        
        attempts += 1;
    }
}

fn calculate_dbtc_split( dbtc_rewards: u64, dbtc_stakers_pct: u8, dbtc_winners_pct: u8, dbtc_same_faction_pct: u8, dbtc_motherlode_pct: u8) -> (u64, u64, u64, u64) {

    let winning_block_rewards = (dbtc_rewards as u128 * dbtc_winners_pct as u128 / 100) as u64;
    msg!("     Winners ({}%): {}", dbtc_winners_pct, winning_block_rewards);

    let same_faction_rewards = (dbtc_rewards as u128 * dbtc_same_faction_pct as u128 / 100) as u64;
    msg!("     Same-faction ({}%): {}", dbtc_same_faction_pct, same_faction_rewards);

    let faction_stakers = (dbtc_rewards as u128 * dbtc_stakers_pct as u128 / 100) as u64;
    msg!("     Stakers ({}%): {}", dbtc_stakers_pct, faction_stakers);
            
    let motherlode_rewards = (dbtc_rewards as u128 * dbtc_motherlode_pct as u128 / 100) as u64;
    msg!("     Motherlode ({}%): {}", dbtc_motherlode_pct, motherlode_rewards);
    
    (winning_block_rewards, same_faction_rewards, faction_stakers, motherlode_rewards)
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
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// Winning faction state (for updating staker rewards and motherlode)
    /// CHECK: Validated manually that faction_id matches winning_faction_id
    #[account(mut)]
    pub winning_faction_state: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc emission vault (holds tokens for distribution)
    #[account(
        mut,
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_emission_vault: UncheckedAccount<'info>,
    
    /// CHECK: SlotHashes sysvar for recent blockhash (optional, using clock as fallback)
    /// CHECK: Not strictly required - using clock slot + timestamp for randomness
    pub slot_hashes: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

