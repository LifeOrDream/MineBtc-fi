use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::state::*;
use crate::instructions::helper;




// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
    msg!("👤 [initialize_player] Initializing player account. Authority: {}. Faction ID: {}", ctx.accounts.authority.key(), faction_id);
    
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;    
    global_config.total_players = global_config.total_players + 1;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;
    
    // Handle referral code logic
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        msg!("     Referral code provided: {}", ref_code);
        require!( ref_code != ctx.accounts.authority.key(), ErrorCode::ReferralCannotBeSameAsOwner);
        
        // Update referrer's referral count if referrer_rewards account is provided
        if let Some(ref mut referrer_rewards) = ctx.accounts.referrer_rewards {
            require!( referrer_rewards.owner == ref_code, ErrorCode::InvalidReferralAccount);            
            referrer_rewards.referrals_count = referrer_rewards.referrals_count + 1;
        } 
        
        // Set player's referral code
        player_data.referral_code = ref_code;
        ref_code
    } else {
        msg!("     No referral code provided, using system referral account");
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };

    player_data.bets_rounds = Vec::new();
    player_data.bets_points = Vec::new();

    // Initialize statistics
    player_data.rounds_played = 0;

    player_data.total_sol_bet = 0;
    player_data.total_points_bet = 0;
    player_data.total_sol_won = 0;
    player_data.total_dbtc_won = 0;
    
    // Initialize DogeBtc staking fields
    player_data.dogebtc_hashpower = 0;
    player_data.dogebtc_staked = 0;
    player_data.dbtc_dbtc_reward_debt = 0;
    player_data.dbtc_sol_reward_debt = 0;
    msg!("     DogeBtc staking fields initialized");
    
    // Initialize LP staking fields
    player_data.lp_hashpower = 0;
    player_data.lp_staked = 0;
    player_data.lp_sol_reward_debt = 0;
    player_data.lp_dbtc_reward_debt = 0;
    msg!("     LP staking fields initialized");
    
    // Initialize pending rewards
    player_data.pending_sol_rewards = 0;
    player_data.pending_dbtc_rewards = 0;
    msg!("     Pending rewards initialized");
    
    // Initialize position tracking vectors
    player_data.moondoge_position_indices = Vec::new();
    player_data.lp_position_indices = Vec::new();
    msg!("     Position tracking initialized");
    
    // Initialize egg staking
    player_data.staked_eggs = Vec::new();
    player_data.egg_multiplier = 100; // Default 1.0x (no eggs staked)
    msg!("     Egg staking initialized (0 eggs, 1.0x multiplier)");
    
    // Initialize free tickets vectors
    player_data.free_tickets = Vec::new();
    player_data.free_tickets_remaining = Vec::new();
    msg!("     Free tickets vectors initialized (empty)");
    
    // Initialize new player's referral rewards account
    msg!("   Initializing new player's referral rewards account...");
    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.pending_sol_rewards = 0;
    new_player_rewards.pending_dbtc_rewards = 0;
    new_player_rewards.total_sol_earned = 0;
    new_player_rewards.total_dbtc_earned = 0;
    msg!("     Referral rewards account initialized");
    
    msg!("✅ [initialize_player] Player initialized successfully");
    msg!("   Player: {} for faction {}", ctx.accounts.authority.key(), faction_id);
    if referral_code.is_some() {
        msg!("   Referral code: {}", referrer_pubkey);
    } else {
        msg!("   Using system referral account: {}", referrer_pubkey);
    }
    
    Ok(())
}



/// Join a round by betting SOL or using free tickets (single bet)
/// Users can bet on either:
/// - A specific block (block_id: 0-23, 0-indexed)
/// - A faction + highest/lowest option (faction_id + is_highest)
/// 
/// Parameters:
/// - amount: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - bet_type: The bet type (Block, FactionHighestLowest, FactionBoth, or RandomBlock)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
/// 
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn join_round(
    ctx: Context<JoinRound>, 
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🎲 [join_round] User joining round (single bet). User: {}", ctx.accounts.authority.key());
    msg!("   Bet type: {:?}", bet_type);
    
    // Call internal join_round with user as payer
    internal_join_round(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.game_session,
        &mut ctx.accounts.user_game_bet,
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.sol_prize_pot_vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.user_game_bet,
        ctx.accounts.authority.key(),
        amount,
        bet_type,
        use_ticket,
    )?;
    
    msg!("✅ [join_round] Bet placed successfully");
    Ok(())
}

/// Join a round with multiple bets in a single transaction
/// Users can bet on:
/// - Multiple blocks (e.g., [0, 4, 9, 14] - 0-indexed: 0-23)
/// - Multiple factions with settings: "low", "high", "both", or "random"
/// 
/// Parameters:
/// - bet_types: Vector of bet types to place (all must be for the same faction)
/// - amount_per_bet: Bet amount per bet type in lamports (for SOL) or points (for tickets)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
/// 
/// Note: Faction accounts are not required for user betting functions. Faction-related calculations
/// are handled in end_round_faction_rewards by cranker bots.
pub fn join_round_batch(
    ctx: Context<JoinRoundBatch>,
    bet_types: Vec<BetType>,
    amount_per_bet: u64,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🎲 [join_round_batch] User joining round with {} bets", bet_types.len());
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!("   Amount per bet: {} lamports", amount_per_bet);
    
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    require!(bet_types.len() <= 24, ErrorCode::InvalidParameters); // Max 24 bets (one per block)
    
    // Expand bet types (handle FactionBoth and RandomBlock)
    let mut expanded_bet_types = Vec::new();
    for bet_type in bet_types.iter() {
        match bet_type {
            BetType::FactionBoth { faction_id } => {
                // Expand to both highest and lowest
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: true 
                });
                expanded_bet_types.push(BetType::FactionHighestLowest { 
                    faction_id: *faction_id, 
                    is_highest: false 
                });
            }
            BetType::RandomBlock => {
                // For random block, we need to select a random block at runtime
                // Use slot hash or similar for randomness
                let clock = Clock::get()?;
                let slot_bytes = clock.slot.to_le_bytes();
                let random_block = ((slot_bytes[0] % 24) + 1) as u8; // 1-24
                expanded_bet_types.push(BetType::Block { block_id: random_block });
                msg!("   Random block selected: {}", random_block);
            }
            _ => {
                expanded_bet_types.push(bet_type.clone());
            }
        }
    }
    
    msg!("   Expanded to {} bet types", expanded_bet_types.len());
    
    // Place each bet
    for (idx, bet_type) in expanded_bet_types.iter().enumerate() {
        msg!("   Placing bet {} of {}: {:?}", idx + 1, expanded_bet_types.len(), bet_type);
                
        // Get target block and faction for this bet (0-indexed: 0-23)
        let target_block = get_target_block_from_bet_type(bet_type, &ctx.accounts.game_session.block_assignments)?;
        let target_faction = ctx.accounts.game_session.block_assignments[target_block as usize];
        
        if idx > 0 {
            let prev_target_block = get_target_block_from_bet_type(&expanded_bet_types[idx - 1], &ctx.accounts.game_session.block_assignments)?;
            let prev_target_faction = ctx.accounts.game_session.block_assignments[prev_target_block as usize];
            require!(
                target_faction == prev_target_faction,
                ErrorCode::InvalidParameters // All bets must be for same faction in batch
            );
        }
        
        // Call internal join_round for each bet
        internal_join_round(
            &ctx.accounts.global_game_state,
            &ctx.accounts.global_config,
            &mut ctx.accounts.player_data,
            &mut ctx.accounts.game_session,
            &mut ctx.accounts.user_game_bet,
            &ctx.accounts.user_wallet.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.user_game_bet,
            ctx.accounts.authority.key(),
            amount_per_bet,
            bet_type.clone(),
            use_ticket,
        )?;
    }
    
    msg!("✅ [join_round_batch] All {} bets placed successfully", expanded_bet_types.len());
    Ok(())
}

/// Internal join_round logic that can be called by both user and autominer
/// Payer can be either user wallet or autominer vault PDA
#[allow(clippy::too_many_arguments)]
fn internal_join_round<'info>(
    global_state: &Account<'info, GlobalGameSate>,
    global_config: &Account<'info, GlobalConfig>,
    player_data: &mut Account<'info, PlayerData>,
    game_session: &mut Account<'info, GameSession>,
    user_game_bet: &mut Account<'info, UserGameBet>,
    payer: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    user_game_bet_bump: u8,
    owner_key: Pubkey,
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<()> {
    let clock = Clock::get()?;
    
    require!(game_session.round_id == global_state.current_round_id, ErrorCode::InvalidRound);
    require!(  game_session.block_assignments.iter().any(|&f| f != 0), ErrorCode::InvalidParameters);
    let round_id = global_state.current_round_id;
    msg!("   Current round ID: {}, Current timestamp: {}, Round end timestamp: {}", round_id, clock.unix_timestamp, global_state.round_end_timestamp);    
    require!(amount > 0 || use_ticket.is_some(), ErrorCode::InvalidAmount);
    
    // Validate bet type
    msg!("   Validating bet type...");
    let target_block = get_target_block_from_bet_type( &bet_type, &game_session.block_assignments)?;
    let target_faction = game_session.block_assignments[target_block as usize];
    msg!("     ✓ Target faction {}", target_faction);

    // Determine if using ticket or SOL
    let (fee_amount, net_amount, points_amount) = if let Some(ticket_type_index) = use_ticket {
        msg!("   Using ticket type index: {}", ticket_type_index);
        require!(  (ticket_type_index as usize) < player_data.free_tickets.len() && (ticket_type_index as usize) < player_data.free_tickets_remaining.len(), ErrorCode::InvalidParameters );
        
        let ticket_value = player_data.free_tickets[ticket_type_index as usize];
        require!(ticket_value > 0, ErrorCode::InvalidAmount);
        msg!("     Ticket value: {} points ({} SOL)", ticket_value, ticket_value as f64 / 1_000_000_000.0);
        
        require!( player_data.free_tickets_remaining[ticket_type_index as usize] > 0, ErrorCode::InsufficientFunds);
        msg!("     Tickets remaining: {}", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        require!(amount == ticket_value, ErrorCode::InvalidAmount);
        msg!("     ✓ Ticket amount matches ticket value");
        
        validate_points_percentage_limit(game_session.total_points_bets, game_session.total_sol_bets, amount)?;
        
        // Deduct ticket
        player_data.free_tickets_remaining[ticket_type_index as usize] -= 1;
        msg!("     ✓ Ticket deducted (remaining: {})", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        // Points bets don't have fees and don't go to prize pot
        (0, 0, amount)
    } else {
        require!(amount > 0, ErrorCode::InvalidAmount);
        msg!("   Using SOL bet. Bet amount: {} SOL", (amount as f64) / 1_000_000_000.0);
        
        // Calculate fees using protocol_fee_pct from GlobalConfig
        let (net, fee_amount) = handle_fee(amount, global_config.sol_fee_config.protocol_fee_pct as u64)?;

        // Calculate faction staker fees (split between dbtc and LP stakers)
        let stakers_fee = fee_amount * global_config.sol_fee_config.stakers_pct as u64 / M_HUNDRED;
        game_session.stakers_fee += stakers_fee;



        // Transfer remaining protocol fees to sol_treasury
        let protocol_fee = fee_amount - stakers_fee;
        if protocol_fee > 0 {
            msg!("   Transferring protocol fees ({} SOL) to sol_treasury", (protocol_fee as f64 / 1_000_000_000.0));
            helper::transfer_to_sol_treasury(payer, sol_treasury, system_program, protocol_fee)?;
            msg!("     ✓ Protocol fees transferred to sol_treasury");
        }    

        // Transfer net amount to prize pot
        msg!("   Transferring net amount ({} SOL) to sol_prize_pot_vault", (net as f64 / 1_000_000_000.0));
        helper::transfer_to_sol_prize_pot_vault(payer, sol_prize_pot_vault, system_program, net)?;
        msg!("     ✓ Net amount transferred to prize pot");
        
        (fee_amount, net, net)
    };

    // Initialize or update UserGameBet PDA
    msg!("   Processing user bet account...");
    let is_new_bet = user_game_bet.owner == Pubkey::default();
    if is_new_bet {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.block_ids = Vec::new();
        user_game_bet.sol_bets = Vec::new();
        user_game_bet.points_bets = Vec::new();
        user_game_bet.total_sol_bet = 0;
        user_game_bet.total_points_bet = 0;
        user_game_bet.total_fee = 0;
        user_game_bet.bump = user_game_bet_bump;
        msg!("     ✓ New bet account initialized");
    } else {
        require!(
            user_game_bet.round_id == round_id,
            ErrorCode::InvalidRound
        );
        msg!("     ✓ Existing bet account found for round {}", round_id);
    }
    
    // Update block_ids, sol_bets, and points_bets vectors
    // Check if target_block is already in block_ids
    let block_index_in_user_bet = user_game_bet.block_ids.iter().position(|&b| b == target_block);
    
    if let Some(index) = block_index_in_user_bet {
        // Block already exists - update existing values
        msg!("     Block {} already in user bet, updating at index {}", target_block, index);
        user_game_bet.sol_bets[index] += net_amount;
        user_game_bet.points_bets[index] += points_amount;
        msg!("       Updated SOL bet: {})",  user_game_bet.sol_bets[index] as f64 / 1_000_000_000.0);
        msg!("       Updated points bet: {})",  user_game_bet.points_bets[index] as f64 / 1_000_000_000.0);
    } else {
        // New block - add to vectors
        msg!("     Adding new block {} to user bet", target_block);
        user_game_bet.block_ids.push(target_block);
        user_game_bet.sol_bets.push(net_amount);
        user_game_bet.points_bets.push(points_amount);
        msg!("       Added SOL bet: {}, points bet: {}", net_amount, points_amount);
    }
    
    // Update totals
    user_game_bet.total_sol_bet += net_amount;
    user_game_bet.total_points_bet += points_amount;
    user_game_bet.total_fee += fee_amount;
    msg!("     Total SOL bet: {} SOL. Total points bet: {} SOL. Total fee: {} SOL", 
        (user_game_bet.total_sol_bet as f64) / 1_000_000_000.0, 
        (user_game_bet.total_points_bet as f64) / 1_000_000_000.0,
        (user_game_bet.total_fee as f64) / 1_000_000_000.0);
    
    // Update block tracking arrays in GameSession (0-indexed: blocks 0-23)
    let block_index = target_block as usize;
    require!(block_index < NUM_BLOCKS, ErrorCode::InvalidParameters);
    
    // Only increment user count if this is a new bet for this block
    if block_index_in_user_bet.is_none() {
        game_session.user_block_indexes[block_index] += 1;
        msg!("     User count for block {}: {}", target_block, game_session.user_block_indexes[block_index]);
    }
    
    // Update SOL bet tracking in GameSession
    game_session.sol_bets_indexes[block_index] += net_amount;
    game_session.points_bets_indexes[block_index] += points_amount;
    game_session.total_sol_bets += net_amount;
    game_session.total_points_bets += points_amount;
    msg!("     SOL bet for block {}: {} (total: {})", target_block, net_amount, game_session.sol_bets_indexes[block_index]);
    msg!("     Points bet for block {}: {} (total: {})", target_block, points_amount, game_session.points_bets_indexes[block_index]);

    // Update PlayerData to track this round
    msg!("   Updating PlayerData for round {}...", round_id);
    if !player_data.bets_rounds.contains(&round_id) {
        player_data.rounds_played += 1;
        player_data.bets_rounds.push(round_id);
        player_data.bets_points.push(0);
        msg!("     Added round {} to player's active rounds", round_id);
    }
    
    // Update the bet amount for this round in PlayerData
    if let Some(index) = player_data.bets_rounds.iter().position(|&r| r == round_id) {
        player_data.bets_points[index] += points_amount;
        msg!("     Player bet amount for round {}: {} SOL", round_id, (player_data.bets_points[index] as f64) / 1_000_000_000.0);
    }
    
    // Update cumulative statistics
    player_data.total_sol_bet += net_amount;
    player_data.total_points_bet += points_amount;
    msg!("     Player total points bet: {} SOL", (player_data.total_points_bet as f64) / 1_000_000_000.0);
    msg!("     Player total SOL bet: {} SOL", (player_data.total_sol_bet as f64) / 1_000_000_000.0);
    
    msg!("   ✓ Bet placed: {} SOL on block {} (bet_type: {:?})", (amount as f64) / 1_000_000_000.0, target_block, bet_type);
     
    Ok(())
}
 
  

/// Get the target block ID from bet_type (0-indexed: 0-23)
/// For Block bets, returns the block_id directly (0-indexed)
/// For FactionHighestLowest bets, finds the faction's blocks and returns highest/lowest (0-indexed)
fn get_target_block_from_bet_type(bet_type: &BetType, block_assignments: &[u8; NUM_BLOCKS]) -> Result<u8> {
    match bet_type {
        BetType::Block { block_id } => {
            require!(*block_id < NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
            Ok(*block_id)
        }
        BetType::FactionHighestLowest { faction_id, is_highest } => {
            require!((*faction_id as usize) < block_assignments.len(), ErrorCode::InvalidParameters);
            // Find the two blocks assigned to this faction (0-indexed)
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }
            
            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );
            
            if *is_highest {
                Ok(*faction_blocks.iter().max().unwrap())
            } else {
                Ok(*faction_blocks.iter().min().unwrap())
            }
        }
        BetType::FactionBoth { faction_id } => {
            // For "both", return the highest block (will be expanded in batch function)
            require!((*faction_id as usize) < block_assignments.len(), ErrorCode::InvalidParameters);
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push(block_idx as u8); // 0-indexed (0-23)
                }
            }
            require!(
                faction_blocks.len() == BLOCKS_PER_FACTION as usize,
                ErrorCode::InvalidParameters
            );
            Ok(*faction_blocks.iter().max().unwrap()) // Return highest, but will be expanded
        }
        BetType::RandomBlock => {
            // Random block - use clock slot for randomness (0-indexed: 0-23)
            let clock = Clock::get()?;
            let slot_bytes = clock.slot.to_le_bytes();
            let random_block = ((slot_bytes[0] % 24)) as u8; // 0-23
            Ok(random_block)
        }
    }
}

fn handle_fee(amount: u64, protocol_fee_pct: u64) -> Result<(u64, u64)> {
    let fee = amount * protocol_fee_pct / M_HUNDRED;
    let net_amount = amount - fee;
    msg!("     Net amount (after fee): {} SOL. Protocol fee ({}%): {} SOL", (net_amount as f64) / 1_000_000_000.0, protocol_fee_pct, (fee as f64) / 1_000_000_000.0);
    return Ok((net_amount, fee));
}

/// Initialize autominer vault with bet types and amounts
/// Users can specify multiple bet types (blocks or faction+highest/lowest) and bet amount per bet
/// Bot can then call execute_autominer_bet to automatically place bets
pub fn init_autominer(
    ctx: Context<InitAutominer>,
    bet_types: Vec<BetType>,
    bet_amount_per_bet: u64,
    num_rounds: u32,
) -> Result<()> {
    msg!("🤖 [init_autominer] Initializing autominer vault");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Bet types count: {}", bet_types.len());
    msg!("   Bet amount per bet: {} lamports", bet_amount_per_bet);
    msg!("   Number of rounds: {}", num_rounds);
    
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;
    
    msg!("   Validating parameters...");
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    msg!("     ✓ Bet types not empty");
    
    require!(
        bet_types.len() <= AutominerVault::MAX_BET_TYPES,
        ErrorCode::InvalidParameters
    );
    msg!("     ✓ Bet types count <= MAX_BET_TYPES ({})", AutominerVault::MAX_BET_TYPES);
    
    require!(bet_amount_per_bet > 0, ErrorCode::InvalidAmount);
    msg!("     ✓ Bet amount per bet > 0");
    
    require!(num_rounds > 0, ErrorCode::InvalidAmount);
    msg!("     ✓ Number of rounds > 0");
    
    // Validate bet types
    msg!("   Validating bet types...");
    for (idx, bet_type) in bet_types.iter().enumerate() {
        match bet_type {
            BetType::Block { block_id } => {
                require!(
                    *block_id >= 1 && *block_id <= NUM_BLOCKS as u8,
                    ErrorCode::InvalidParameters
                );
                msg!("     Bet type {}: Block {} ✓", idx, block_id);
            }
            BetType::FactionHighestLowest { faction_id, is_highest } => {
                require!(
                    (*faction_id as usize) < global_config.supported_factions.len(),
                    ErrorCode::InvalidFactionId
                );
                msg!("     Bet type {}: Faction {} ({}) ✓", idx, faction_id, if *is_highest { "highest" } else { "lowest" });
            }
            BetType::FactionBoth { faction_id } => {
                require!(
                    (*faction_id as usize) < global_config.supported_factions.len(),
                    ErrorCode::InvalidFactionId
                );
                msg!("     Bet type {}: Faction {} (both) ✓", idx, faction_id);
            }
            BetType::RandomBlock => {
                msg!("     Bet type {}: Random block ✓", idx);
            }
        }
    }
    
    msg!("   Initializing autominer vault...");
    autominer_vault.owner = ctx.accounts.authority.key();
    autominer_vault.bet_types = bet_types.clone();
    autominer_vault.bet_amount_per_bet = bet_amount_per_bet;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    msg!("     Vault initialized for owner: {}", autominer_vault.owner);
    
    // Calculate total SOL needed: (bet_amount_per_bet * num_bet_types) * num_rounds + rent
    msg!("   Calculating total SOL needed...");
    let sol_per_round = bet_amount_per_bet
        .checked_mul(bet_types.len() as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("     SOL per round: {} lamports ({} bets × {} lamports)", sol_per_round, bet_types.len(), bet_amount_per_bet);
    
    let total_sol = sol_per_round
        .checked_mul(num_rounds as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("     Total SOL for all rounds: {} lamports ({} rounds × {} lamports)", total_sol, num_rounds, sol_per_round);
    
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let total_transfer = total_sol
        .checked_add(rent)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("     Rent: {} lamports", rent);
    msg!("     Total transfer: {} lamports", total_transfer);
    
    // Transfer SOL to vault
    msg!("   Transferring SOL to vault...");
    let vault_before = ctx.accounts.autominer_vault.to_account_info().lamports();
    let wallet_before = ctx.accounts.user_wallet.lamports();
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? += total_transfer;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= total_transfer;
    let vault_after = ctx.accounts.autominer_vault.to_account_info().lamports();
    let wallet_after = ctx.accounts.user_wallet.lamports();
    msg!("     Vault: {} -> {} lamports (+{})", vault_before, vault_after, total_transfer);
    msg!("     Wallet: {} -> {} lamports (-{})", wallet_before, wallet_after, total_transfer);
    
    msg!("✅ [init_autominer] Autominer initialized successfully");
    msg!("   {} bet types, {} SOL per bet, {} rounds ({} SOL total)",
        bet_types.len(),
        bet_amount_per_bet,
        num_rounds,
        total_sol
    );
    
    Ok(())
}

/// Execute autominer bets (keeper instruction - bot can call this)
/// Places bets for all configured bet types in the current round
/// Can be called once per round to place all bets automatically
pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
    msg!("🤖 [execute_autominer_bet] Executing autominer bets");
    msg!("   Owner: {}", ctx.accounts.autominer_vault.owner);
    
    let global_state = &ctx.accounts.global_game_state;
    let clock = Clock::get()?;
    
    // Read values before mutable borrow
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let last_bet_round_id = ctx.accounts.autominer_vault.last_bet_round_id;
    let num_bets = ctx.accounts.autominer_vault.bet_types.len();
    let bet_amount_per_bet = ctx.accounts.autominer_vault.bet_amount_per_bet;
    
    msg!("   Vault state:");
    msg!("     Rounds remaining: {}", rounds_remaining);
    msg!("     Last bet round ID: {}", last_bet_round_id);
    msg!("     Number of bet types: {}", num_bets);
    msg!("     Bet amount per bet: {} lamports", bet_amount_per_bet);
    msg!("   Current round ID: {}", global_state.current_round_id);
    msg!("   Current timestamp: {}", clock.unix_timestamp);
    msg!("   Round end timestamp: {}", global_state.round_end_timestamp);
    
    msg!("   Validating execution conditions...");
    require!(
        rounds_remaining > 0,
        ErrorCode::NoRoundsRemaining
    );
    msg!("     ✓ Rounds remaining > 0");
    
    // Check round hasn't ended
    require!(
        clock.unix_timestamp < global_state.round_end_timestamp,
        ErrorCode::RoundEnded
    );
    msg!("     ✓ Round hasn't ended");
    
    // Check bets haven't been placed for this round already
    require!(
        last_bet_round_id != global_state.current_round_id,
        ErrorCode::InvalidRound
    );
    msg!("     ✓ Bets not yet placed for this round");
    
    // Calculate total SOL needed for this round
    msg!("   Calculating SOL needed for this round...");
    let sol_per_round = bet_amount_per_bet
        .checked_mul(num_bets as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("     SOL per round: {} lamports ({} bets × {} lamports)", sol_per_round, num_bets, bet_amount_per_bet);
    
    // Check vault has enough SOL
    msg!("   Checking vault balance...");
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let available_sol = vault_lamports
        .checked_sub(rent)
        .ok_or(ErrorCode::InsufficientFunds)?;
    msg!("     Vault lamports: {}", vault_lamports);
    msg!("     Rent: {}", rent);
    msg!("     Available SOL: {}", available_sol);
    
    require!(
        available_sol >= sol_per_round,
        ErrorCode::InsufficientFunds
    );
    msg!("     ✓ Vault has sufficient SOL");
    
    // Deduct SOL for bets (protocol fee will be deducted in join_round)
    // For now, we'll deduct the full amount - actual implementation should call join_round
    msg!("   Deducting SOL from vault for bets...");
    let vault_before = ctx.accounts.autominer_vault.to_account_info().lamports();
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= sol_per_round;
    let vault_after = ctx.accounts.autominer_vault.to_account_info().lamports();
    msg!("     Vault: {} -> {} lamports (-{})", vault_before, vault_after, sol_per_round);
    
    // Now borrow mutably to update state
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    
    // Mark bets as placed for this round
    let current_round_id = global_state.current_round_id;
    autominer_vault.last_bet_round_id = current_round_id;
    msg!("   Updated last_bet_round_id: {} -> {}", last_bet_round_id, current_round_id);
    
    // Decrement rounds remaining
    let new_rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    autominer_vault.rounds_remaining = new_rounds_remaining;
    msg!("   Updated rounds_remaining: {} -> {}", rounds_remaining, new_rounds_remaining);
    
    // If no rounds remaining, close vault and return remaining SOL
    if new_rounds_remaining == 0 {
        msg!("   No rounds remaining - closing vault and returning remaining SOL...");
        let remaining_sol = ctx.accounts.autominer_vault.to_account_info().lamports()
            .checked_sub(rent)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        let owner_before = ctx.accounts.owner.lamports();
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
        let owner_after = ctx.accounts.owner.lamports();
        msg!("     Owner: {} -> {} lamports (+{})", owner_before, owner_after, remaining_sol);
        msg!("     Vault closed");
    }
    
    // Place bets for each bet type using internal_join_round
    msg!("   Placing {} bets for round {}...", num_bets, current_round_id);
    let bet_types = ctx.accounts.autominer_vault.bet_types.clone();
    let owner_key = ctx.accounts.autominer_vault.owner;
    
    for (idx, bet_type) in bet_types.iter().enumerate() {
        msg!("     Placing bet #{}: {:?} for {} lamports", idx + 1, bet_type, bet_amount_per_bet);
        
        // Call internal_join_round with autominer vault as payer
        internal_join_round(
            &ctx.accounts.global_game_state,
            &ctx.accounts.global_config,
            &mut ctx.accounts.player_data,
            &mut ctx.accounts.game_session,
            &mut ctx.accounts.user_game_bet,
            &ctx.accounts.autominer_vault.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.sol_prize_pot_vault.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            ctx.bumps.user_game_bet,
            owner_key,
            bet_amount_per_bet,
            bet_type.clone(),
            None, // autominer always uses SOL, not tickets
        )?;
        
        msg!("       ✓ Bet #{} placed successfully", idx + 1);
    }
    
    msg!("✅ [execute_autominer_bet] Autominer bets executed successfully");
    msg!("   {} bets of {} SOL each for round {}", num_bets, bet_amount_per_bet, current_round_id);
    msg!("   Rounds remaining: {}", new_rounds_remaining);
    
    Ok(())
}
 
 

fn validate_points_percentage_limit(current_points_bets: u64, current_sol_bets: u64, amount: u64) -> Result<()> {
        // Validate points percentage limit: points bets must stay at or below 25% of SOL bets for this session
        // Tickets can only be used when: (total_points_bets + ticket_amount) <= (total_sol_bets * 25 / 100)
        let new_points_bets = current_points_bets + amount;        
        msg!("     Current session stats: SOL bets: {} lamports, Points bets: {} lamports, New points bets if allowed: {} lamports", current_sol_bets, current_points_bets, new_points_bets);
        
        // Require that SOL bets exist before allowing ticket bets -  This ensures points percentage can be calculated and stays within 25% limit
        require!(  current_sol_bets > 0,  ErrorCode::InvalidParameters);
        msg!("     ✓ SOL bets exist in session");
        
        // Calculate max allowed points bets (25% of SOL bets) -  This ensures points percentage can be calculated and stays within 25% limit
        let max_allowed_points = current_sol_bets * 25 / 100;        
        msg!("       Max allowed points (25% of SOL): {} lamports", max_allowed_points);
        require!( new_points_bets <= max_allowed_points, ErrorCode::InvalidParameters);
        msg!("     ✓ Points bets stay within 25% limit");
        Ok(())
}

 


 

 
/// Claim rewards for a user after round ends
/// Checks if user won based on their bet type and the winning block
pub fn claim_round_rewards(ctx: Context<ClaimRoundRewards>) -> Result<()> {
    msg!("💰 [claim_rewards] User claiming rewards. User: {}", ctx.accounts.user_wallet.key());
    
    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    let player_data = &mut ctx.accounts.player_data;

    // Round should be completely over before user can claim rewards
    require!( game_session.stage == 2, ErrorCode::InvalidStage );
    
    msg!("   User bet round ID: {}. GameSession round ID: {}", user_bet.round_id, game_session.round_id);    
    require!( user_bet.round_id == game_session.round_id, ErrorCode::InvalidRound);
        
    // Check which blocks user bet on and calculate rewards
    msg!("   User bet on {} blocks: {:?}", user_bet.block_ids.len(), user_bet.block_ids);
    msg!("   Winning block: {}. Follow-up block: {}", game_session.winning_block, game_session.same_faction_other_block);
    msg!("     Winning faction ID: {}", game_session.winning_faction_id);
        
    // Calculate rewards for each block user bet on
    let mut total_sol_reward = 0u64;
    let mut total_dbtc_reward = 0u64;
    
    for (idx, &block_id) in user_bet.block_ids.iter().enumerate() {
        let points_bet_on_block = user_bet.points_bets.get(idx).copied().unwrap_or(0);
        
        msg!("     Block {}: Points bet: {} SOL", block_id, points_bet_on_block as f64 / 1_000_000_000.0);
        
        let is_winning_block = block_id == game_session.winning_block;
        let is_same_faction_block = block_id == game_session.same_faction_other_block;
        
        if is_winning_block {
            msg!("       ✓ Winning block - calculating rewards...");
            
            // SOL rewards (only for winning block)
            if game_session.sol_rewards_index > 0 && points_bet_on_block > 0 {
                let sol_reward = helper::mul_div(points_bet_on_block, game_session.sol_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_sol_reward += sol_reward;
                msg!("         SOL reward: {} lamports", sol_reward);
            }
            
            // DogeBtc rewards (winning block)
            if game_session.dbtc_rewards_index > 0 && points_bet_on_block > 0 {
                let dbtc_reward = helper::mul_div(points_bet_on_block, game_session.dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_dbtc_reward += dbtc_reward;
                msg!("         DogeBtc reward: {} tokens", dbtc_reward);
            }
        } else if is_same_faction_block {
            msg!("       ✓ Same-faction other block - calculating DogeBtc rewards...");
            
            // DogeBtc rewards (same-faction other block)
            if game_session.same_faction_dbtc_rewards_index > 0 && points_bet_on_block > 0 {
                let dbtc_reward = helper::mul_div(points_bet_on_block, game_session.same_faction_dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
                total_dbtc_reward += dbtc_reward;
                msg!("         DogeBtc reward: {} tokens", dbtc_reward);
            }
        } else {
            msg!("       ✗ Not a winning or same-faction block - no rewards");
        }
    }
    
    msg!("   Total SOL reward: {} lamports", total_sol_reward);
    msg!("   Total DogeBtc reward: {} tokens", total_dbtc_reward);

    player_data.total_sol_won += total_sol_reward;
    msg!("     Total SOL won: {} (+{})", player_data.total_sol_won, total_sol_reward);
    msg!("     Total DogeBtc won: {} (+{})", player_data.total_dbtc_won, total_dbtc_reward);

    player_data.pending_sol_rewards += total_sol_reward;
    add_to_total_claimable(&mut ctx.accounts.global_game_state, player_data, total_dbtc_reward);
    msg!("     Pending SOL rewards: {} (+{})", player_data.pending_sol_rewards, total_sol_reward);
    msg!("     Pending DogeBtc rewards: {} (+{})", player_data.pending_dbtc_rewards, total_dbtc_reward);
        
    // Remove round from player's active rounds list
    msg!("   Removing round from player's active rounds list...");
    if let Some(index) = player_data.bets_rounds.iter().position(|&r| r == user_bet.round_id) {
        let old_count = player_data.bets_rounds.len();
        player_data.bets_rounds.remove(index);
        player_data.bets_points.remove(index);
        msg!("     Removed round {} from active rounds (count: {} -> {})", user_bet.round_id, old_count, player_data.bets_rounds.len());
    }
    
    // Close bet account and return rent
    msg!("   Closing bet account and returning rent...");
    let signer_key = ctx.accounts.user_wallet.key();
    let rent = Rent::get()?.minimum_balance(UserGameBet::LEN);
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? += rent;
    msg!("     Returned {} lamports rent to user", rent);
    
    msg!("✅ [claim_rewards] Rewards claimed successfully");
    msg!("   User: {}", signer_key);
    msg!("   Round: {}", user_bet.round_id);
    
    Ok(())
}



/// Add to total claimable and pending rewards
fn add_to_total_claimable(game_state: &mut GlobalGameSate, player_data: &mut PlayerData, dbtc_rewards: u64) {

    // Calculate extra dogeBtc rewards due to unrefining
    let index_dif = game_state.unrefining_index - player_data.unrefining_index;
    let accrued_rewards = helper::mul_div_u128( player_data.pending_dbtc_rewards as u128, index_dif, INDEX_PRECISION as u128).unwrap() as u64;
    msg!("     Accrued DogeBtc rewards: {}", accrued_rewards );

    game_state.total_dbtc_claimable += dbtc_rewards + accrued_rewards;
    player_data.unrefining_index = game_state.unrefining_index;
    player_data.pending_dbtc_rewards += dbtc_rewards + accrued_rewards;
    player_data.total_dbtc_won += dbtc_rewards + accrued_rewards;
    player_data.unrefined_dbtc_rewards += accrued_rewards;
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
    
    /// Optional: Referrer's referral rewards account (if referral code is provided)
    /// CHECK: Validated manually that owner matches referral_code pubkey
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
pub struct JoinRound<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    /// GameSession PDA for the current round (must be initialized by crank function)
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA for this user's bet in this round
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Account struct for batch betting
/// Note: All bets must be for the same faction (same faction_state account)
#[derive(Accounts)]
pub struct JoinRoundBatch<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    /// GameSession PDA for the current round
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump = game_session.bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA (shared across all bets in batch)
    #[account(
        init_if_needed,
        payer = authority,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct ClaimRoundRewards<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.last_round_id.to_le_bytes()],
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
        close = user_wallet,
        seeds = [USER_GAME_BET_SEED.as_ref(), user_wallet.key().as_ref(), &game_session.round_id.to_le_bytes()],
        bump = user_game_bet.bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// Optional referrer rewards account (if player has a referrer)
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), player_data.referral_code.as_ref()],
        bump
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,
    
    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc emission vault
    #[account(
        mut,
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_emission_vault: UncheckedAccount<'info>,
        
    /// User whose bet this is (doesn't need to be signer - anyone can claim for them)
    /// CHECK: Validated via player_data.owner matching user_wallet
    #[account(mut)]
    pub user_wallet: UncheckedAccount<'info>,
    
    /// Caller (bot or user themselves) - can be anyone
    pub caller: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_types: Vec<BetType>, bet_amount_per_bet: u64, num_rounds: u32)]
pub struct InitAutominer<'info> {
    #[account(
        init,
        payer = authority,
        space = AutominerVault::LEN,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), authority.key().as_ref()],
        bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteAutominerBet<'info> {
    #[account(
        mut,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,

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
    
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), autominer_vault.owner.as_ref()],
        bump
    )]
    pub player_data: Account<'info, PlayerData>,
        
    #[account(
        mut,
        seeds = [GAME_SESSION_SEED.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub game_session: Account<'info, GameSession>,
    
    /// UserGameBet PDA for autominer bets (aggregates all bets from this vault for this round)
    #[account(
        init_if_needed,
        payer = caller,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_game_bet: Account<'info, UserGameBet>,
    
    /// CHECK: SOL treasury PDA
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
        
    /// CHECK: Owner account (to receive remaining SOL when vault closes)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,
    
    /// Caller (bot or anyone) - doesn't need to be owner
    #[account(mut)]
    pub caller: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
 