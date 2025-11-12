use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_lang::solana_program::sysvar::Sysvar;
use anchor_lang::AccountDeserialize;
use anchor_lang::AccountSerialize;

use crate::errors::ErrorCode;
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
/// For the first round, use next_round_commit from global_state (set during initialization).
pub fn start_round(
    ctx: Context<StartRound>,
    commit_hash: Option<[u8; 32]>, // Optional: if None, uses next_round_commit from global_state
) -> Result<()> {
    msg!("🎮 [start_round] Starting new round");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    
    msg!("   Current round ID: {}", global_state.current_round_id);
    msg!("   Current timestamp: {}", clock.unix_timestamp);
    msg!("   Round end timestamp: {}", global_state.round_end_timestamp);
    
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
    
    // Increment round ID
    let new_round_id = global_state.current_round_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   New round ID: {}", new_round_id);
    
    // Set commit hash for this round
    global_state.current_round_commit = commit;
    global_state.current_round_seed = None; // Will be set in end_round
    msg!("   Commit hash set for round {}", new_round_id);

    // Update global state
    global_state.current_round_id = new_round_id;
    global_state.round_end_timestamp = game_session.round_end_timestamp;
    msg!("   Global state updated: current_round_id={}, round_end_timestamp={}", new_round_id, game_session.round_end_timestamp);

    // Initialize GameSession for the new round
    msg!("   Initializing GameSession for round {}", new_round_id);
    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = new_round_id;
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

    game_session.total_sol_pot_net = 0;
    game_session.total_sol_bet_on_winner = 0;
    game_session.total_sol_bet_on_losers = 0;

    game_session.dbtc_winner_pool = 0;

    game_session.motherlode_hit_faction_id = 0;
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;
    
    
    msg!("✅ [start_round] Round {} started successfully", new_round_id);
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
    
    // Select winning block (1-24) using final hash
    let winning_block = ((u64::from_le_bytes([
        final_hash_bytes[0],
        final_hash_bytes[1],
        final_hash_bytes[2],
        final_hash_bytes[3],
        0, 0, 0, 0,
    ]) % NUM_BLOCKS as u64) + 1) as u8; // 1-indexed blocks
    
    // Get winning faction from block assignments
    let winning_faction_id = game_session.block_assignments[(winning_block - 1) as usize];
    let same_faction_other_block = game_session.block_assignments.iter().position(|&x| x == winning_faction_id && x != winning_block).unwrap() as u8;

    game_session.winning_block = winning_block;
    game_session.winning_faction_id = winning_faction_id;
    game_session.same_faction_other_block = same_faction_other_block;
    msg!("   🎯 Winning block selected: {} (Faction: {}) Same-faction other block: {}", winning_block, winning_faction_id, same_faction_other_block);
    
    // Calculate DogeBtc emission for this round
    let dbtc_rewards = doge_btc_mining.current_dist_rate;
    msg!("   Calculating DogeBtc emission for round...");
    msg!("   Current dist rate: {}", dbtc_rewards);
    
    // Calculate DogeBtc distribution pools according to ORE tokenomics
    let (winning_block_rewards, same_faction_rewards, faction_stakers, motherlode_rewards) = calculate_dbtc_split(dbtc_rewards, global_config.dbtc_dist_config.dbtc_stakers_pct, global_config.dbtc_dist_config.dbtc_winners_pct, global_config.dbtc_dist_config.dbtc_same_faction_pct, global_config.dbtc_dist_config.dbtc_motherlode_pct);
    game_session.dbtc_winner_pool = winning_block_rewards;
    game_session.dbtc_loser_pool = same_faction_rewards;

    // Calculate SOL rewards index --> basically rewards claimable by a user = his points * sol_rewards_index / PRECISION;
    let winning_block_pts = game_session.points_bets_indexes[(winning_block - 1) as usize];

    game_session.sol_rewards_index = game_session.sol_rewards_index + (game_session.total_sol_bets * PRECISION / winning_block_pts);

    // Calculate DogeBtc rewards index for winning block and same-faction other block
    game_session.dbtc_rewards_index = game_session.dbtc_rewards_index + (winning_block_rewards * PRECISION / winning_block_pts);
    game_session.same_faction_dbtc_rewards_index = game_session.same_faction_dbtc_rewards_index + (same_faction_rewards * PRECISION / winning_block_pts);

    // dBTC distribution to stakers of the winning faction
    faction_state.dbtc_reward_index = faction_state.dbtc_reward_index +  (faction_stakers * PRECISION / faction_state.total_passive_hashpower) ;
    
    // Increment motherlode pot size
    faction_state.motherlode_pot_size = faction_state.motherlode_pot_size + motherlode_rewards;


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
    if motherlode_hit {
        msg!("   🎰 MOTHERLODE HIT! Random value: {} (mod {})", motherlode_random, MOTHERLODE_CHANCE);

        game_session.motherlode_hit_faction_id = winning_faction_id;

        game_session.dbtc_winner_pool = game_session.dbtc_winner_pool + motherlode_rewards / 2;
        game_session.dbtc_loser_pool = game_session.dbtc_loser_pool + motherlode_rewards / 2;
        game_session.motherlode_pot_size_on_hit = motherlode_rewards;

        game_session.dbtc_rewards_index = game_session.dbtc_rewards_index + (motherlode_rewards/2 * PRECISION / winning_block_pts);
        game_session.same_faction_dbtc_rewards_index = game_session.same_faction_dbtc_rewards_index + (motherlode_rewards/2 * PRECISION / winning_block_pts);
    } else {
        msg!("   Motherlode miss. Random value: {} (mod {})", motherlode_random, MOTHERLODE_CHANCE);
    }
    
    // If motherlode was hit, record the pot size
    if motherlode_hit {
        game_session.motherlode_pot_size_on_hit = faction_state.motherlode_pot_size;
        msg!("   🎰 Motherlode pot size on hit: {}", game_session.motherlode_pot_size_on_hit);
    }  
    
    // // Update faction wins
    faction_state.total_wins = faction_state.total_wins.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Faction wins: {} -> {}", old_wins, faction_state.total_wins);
        
    // Update global state with previous round results
    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;
    msg!("   Updated global state: last_round_id={}, winning_faction_id={}", global_state.last_round_id, winning_faction_id);
    
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
        seeds = [GAME_SESSION_SEED.as_ref(), &(global_game_state.current_round_id.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?).to_le_bytes()],
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

