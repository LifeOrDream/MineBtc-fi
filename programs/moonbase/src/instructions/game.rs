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
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let clock = Clock::get()?;
    
    // Validate game is active
    require!(global_state.is_active, ErrorCode::InvalidParameters);
    
    // Validate that previous round has ended (if not first round)
    if global_state.current_round_id > 0 {
        require!(
            clock.unix_timestamp >= global_state.round_end_timestamp,
            ErrorCode::RoundNotEnded
        );
    }
    
    // Use provided commit_hash or next_round_commit from global_state
    let commit = commit_hash.unwrap_or(global_state.next_round_commit);
    
    // Increment round ID
    let new_round_id = global_state.current_round_id
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Set commit hash for this round
    global_state.current_round_commit = commit;
    global_state.current_round_seed = None; // Will be set in end_round
    
    // Initialize GameSession for the new round
    game_session.bump = ctx.bumps.game_session;
    game_session.round_id = new_round_id;
    game_session.round_start_timestamp = clock.unix_timestamp;
    game_session.round_end_timestamp = clock.unix_timestamp
        .checked_add(global_state.round_duration_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    game_session.total_sol_bets = 0;
    game_session.total_points_bets = 0;
    game_session.sol_bets_indexes = Vec::new();
    game_session.points_bets_indexes = Vec::new();
    
    // Randomly assign 12 factions to 24 blocks (2 blocks per faction)
    // Use commit hash + slot for deterministic but unpredictable randomness
    let mut block_assignments = [0u8; NUM_BLOCKS];
    let mut faction_blocks_assigned = [0u8; NUM_FACTIONS]; // Track blocks assigned per faction
    
    // Create seed for randomness: commit_hash + current_slot
    let mut random_seed = Vec::new();
    random_seed.extend_from_slice(&commit);
    random_seed.extend_from_slice(&clock.slot.to_le_bytes());
    let hash = keccak::hash(&random_seed);
    let hash_bytes = hash.to_bytes();
    
    // Assign factions to blocks
    let mut hash_offset = 0;
    for block_idx in 0..NUM_BLOCKS {
        // Find a faction that hasn't been assigned 2 blocks yet
        let mut attempts = 0;
        loop {
            if attempts >= 100 {
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
    for &count in faction_blocks_assigned.iter() {
        require!(count == BLOCKS_PER_FACTION as u8, ErrorCode::InvalidParameters);
    }
    
    game_session.block_assignments = block_assignments;
    game_session.winning_block = 0; // Will be set in end_round
    game_session.winning_faction_id = 0; // Will be set in end_round
    game_session.total_sol_pot_net = 0;
    game_session.total_sol_bet_on_winner = 0;
    game_session.total_sol_bet_on_losers = 0;
    game_session.total_sol_bet_all_factions = 0;
    game_session.dbtc_winner_pool = 0;
    game_session.dbtc_loser_pool = 0;
    game_session.motherlode_hit = false;
    game_session.motherlode_pot_size_on_hit = 0;
    
    // Update global state
    global_state.current_round_id = new_round_id;
    global_state.round_end_timestamp = game_session.round_end_timestamp;
    
    msg!(
        "Round {} started. Commit hash: {:?}, Round ends at timestamp: {}",
        new_round_id,
        commit,
        game_session.round_end_timestamp
    );
    
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
    let global_state = &mut ctx.accounts.global_game_state;
    let game_session = &mut ctx.accounts.game_session;
    let global_config = &ctx.accounts.global_config;
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;
    let clock = Clock::get()?;
    
    // Validate round has ended
    require!(
        clock.unix_timestamp >= game_session.round_end_timestamp,
        ErrorCode::RoundNotEnded
    );
    
    // Verify commit-reveal: hash(revealed_seed) must equal current_round_commit
    let seed_hash = keccak::hash(&revealed_seed);
    require!(
        seed_hash.to_bytes() == global_state.current_round_commit,
        ErrorCode::InvalidParameters
    );
    
    // Store revealed seed
    global_state.current_round_seed = Some(revealed_seed);
    
    // Generate final randomness: hash(revealed_seed + clock slot + timestamp)
    // This combines the revealed seed with on-chain values that were unpredictable during betting
    let clock = Clock::get()?;
    let mut final_randomness_seed = Vec::new();
    final_randomness_seed.extend_from_slice(&revealed_seed);
    final_randomness_seed.extend_from_slice(&clock.slot.to_le_bytes());
    final_randomness_seed.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
    let final_hash = keccak::hash(&final_randomness_seed);
    let final_hash_bytes = final_hash.to_bytes();
    
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
    
    game_session.winning_block = winning_block;
    game_session.winning_faction_id = winning_faction_id;
    
    // Calculate DogeBtc emission for this round
    let slots_per_round = (global_state.round_duration_seconds as u64)
        .checked_mul(2)
        .ok_or(ErrorCode::ArithmeticOverflow)?; // ~2 slots per second
    let emission_per_round = doge_btc_mining.current_dist_rate
        .checked_mul(slots_per_round)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Calculate DogeBtc distribution pools according to ORE tokenomics
    let dbtc_stakers = emission_per_round
        .checked_mul(global_config.dbtc_dist_config.dbtc_stakers_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let dbtc_winners = emission_per_round
        .checked_mul(global_config.dbtc_dist_config.dbtc_winners_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let dbtc_same_faction = emission_per_round
        .checked_mul(global_config.dbtc_dist_config.dbtc_same_faction_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let dbtc_motherlode = emission_per_round
        .checked_mul(global_config.dbtc_dist_config.dbtc_motherlode_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Check for motherlode (random chance)
    let motherlode_random = u64::from_le_bytes([
        final_hash_bytes[4],
        final_hash_bytes[5],
        final_hash_bytes[6],
        final_hash_bytes[7],
        0, 0, 0, 0,
    ]) % MOTHERLODE_CHANCE;
    let motherlode_hit = motherlode_random == 0;
    
    game_session.motherlode_hit = motherlode_hit;
    
    // Calculate SOL pot (from prize pot vault)
    let total_sol_pot_net = ctx.accounts.sol_prize_pot_vault.lamports();
    game_session.total_sol_pot_net = total_sol_pot_net;
    
    // Calculate total bets on winner and same-faction losers
    // Note: This will need to be calculated by iterating through all UserGameBet accounts
    // For now, we'll set placeholder values - actual calculation should be done separately
    // or by passing remaining_accounts with all UserGameBet PDAs
    // TODO: Calculate from UserGameBet accounts:
    // - total_sol_bet_on_winner: sum of sol_bet_amount for bets where target_block == winning_block
    // - total_sol_bet_on_same_faction: sum of sol_bet_amount for bets on the other block with same faction
    game_session.total_sol_bet_on_winner = 0; // TODO: Calculate from UserGameBet accounts
    game_session.total_sol_bet_on_losers = 0; // TODO: Calculate from UserGameBet accounts (same-faction other block)
    game_session.total_sol_bet_all_factions = game_session.total_sol_bets;
    
    // Set DogeBtc pools for winners and same-faction bettors
    game_session.dbtc_winner_pool = dbtc_winners;
    game_session.dbtc_loser_pool = dbtc_same_faction;
    
    // Update winning faction's staker reward index (dbtc_stakers_pct)
    // This accumulates DogeBtc rewards for stakers of the winning faction
    let mut faction_state_data = ctx.accounts.winning_faction_state.try_borrow_mut_data()?;
    let mut faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_state_data[..])?;
    
    // Validate faction_id matches winning faction
    require!(
        faction_state.faction_id == winning_faction_id,
        ErrorCode::InvalidFactionId
    );
    
    if faction_state.total_passive_hashpower > 0 {
        // Calculate dbtc_per_share and add to reward index
        let dbtc_per_share = (dbtc_stakers as u128)
            .checked_mul(INDEX_PRECISION as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(faction_state.total_passive_hashpower)
            .unwrap_or(0);
        
        faction_state.dbtc_reward_index = faction_state.dbtc_reward_index
            .checked_add(dbtc_per_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    // Update winning faction's motherlode pot (dbtc_motherlode_pct)
    // Always add to motherlode pot, regardless of whether it was hit this round
    faction_state.motherlode_pot_size = faction_state.motherlode_pot_size
        .checked_add(dbtc_motherlode)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // If motherlode was hit, record the pot size
    if motherlode_hit {
        game_session.motherlode_pot_size_on_hit = faction_state.motherlode_pot_size;
    } else {
        game_session.motherlode_pot_size_on_hit = 0;
    }
    
    // Update faction wins
    faction_state.total_wins = faction_state.total_wins
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Serialize updated faction state back
    faction_state.try_serialize(&mut &mut faction_state_data[..])?;
    
    // Update global state with previous round results
    global_state.last_round_id = game_session.round_id;
    global_state.winning_faction_id = winning_faction_id;
    
    // Set commit hash for next round
    global_state.next_round_commit = next_round_commit;
    
    // Update round end timestamp for next round (will be set in start_round)
    // For now, we keep the current timestamp - start_round will update it
    
    msg!(
        "Round {} ended. Winning block: {}, Winning faction: {}, Motherlode: {}",
        game_session.round_id,
        winning_block,
        winning_faction_id,
        if motherlode_hit { "HIT!" } else { "miss" }
    );
    msg!(
        "DogeBtc distribution: stakers={}, winners={}, same_faction={}, motherlode={}",
        dbtc_stakers,
        dbtc_winners,
        dbtc_same_faction,
        dbtc_motherlode
    );
    
    Ok(())
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

