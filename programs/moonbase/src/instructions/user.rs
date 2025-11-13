use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::state::*;
use crate::instructions::helper;




// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
    msg!("👤 [initialize_player] Initializing player account");
    msg!("   Authority: {}", ctx.accounts.authority.key());
    msg!("   Faction ID: {}", faction_id);
    
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;
    
    msg!("   Total players before: {}", global_config.total_players);
    // Increment total players count
    global_config.total_players = global_config.total_players.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Total players after: {}", global_config.total_players);
    
    // Validate faction_id
    msg!("   Validating faction_id...");
    msg!("   Supported factions count: {}", global_config.supported_factions.len());
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    msg!("   ✓ Faction ID {} is valid", faction_id);
    
    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;
    
    // Handle referral code logic
    msg!("   Processing referral code...");
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        msg!("     Referral code provided: {}", ref_code);
        // Validate referral code is not the same as owner
        require!(
            ref_code != ctx.accounts.authority.key(),
            ErrorCode::ReferralCannotBeSameAsOwner
        );
        msg!("     ✓ Referral code is different from owner");
        
        // Update referrer's referral count if referrer_rewards account is provided
        if let Some(ref mut referrer_rewards) = ctx.accounts.referrer_rewards {
            msg!("     Referrer rewards account provided");
            // Validate that the referrer_rewards account belongs to the referral_code
            require!(
                referrer_rewards.owner == ref_code,
                ErrorCode::InvalidReferralAccount
            );
            msg!("     ✓ Referrer rewards account validated");
            
            let old_count = referrer_rewards.referrals_count;
            referrer_rewards.referrals_count = referrer_rewards
                .referrals_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            msg!("     Referrals count: {} -> {}", old_count, referrer_rewards.referrals_count);
        } else {
            msg!("     ⚠️ Referrer rewards account not provided (optional)");
        }
        
        // Set player's referral code
        player_data.referral_code = ref_code;
        ref_code
    } else {
        msg!("     No referral code provided, using system referral account");
        // No referral code provided, use system referral account
        // System referral account PDA: [REFERRAL_REWARDS_SEED, system_program.key()]
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        msg!("     System referral: {}", system_referral_pubkey);
        system_referral_pubkey
    };
    
    // Initialize empty vectors for tracking rounds
    msg!("   Initializing player data fields...");
    player_data.sol_bets_rounds = Vec::new();
    player_data.sol_bets_amounts = Vec::new();
    msg!("     Initialized empty vectors for round tracking");
    
    // Initialize statistics
    player_data.rounds_played = 0;
    player_data.rounds_won = 0;
    player_data.total_sol_bet = 0;
    player_data.total_points_bet = 0;
    player_data.total_sol_won = 0;
    player_data.total_dbtc_won = 0;
    msg!("     Statistics initialized to 0");
    
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
    player_data.active_moondoge_positions = 0;
    player_data.active_lp_positions = 0;
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



/// Join a round by betting SOL or using free tickets
/// Users can bet on either:
/// - A specific block (block_id: 1-24)
/// - A faction + highest/lowest option (faction_id + is_highest)
/// 
/// Parameters:
/// - amount: Bet amount in lamports (for SOL) or points (for tickets). 1 point = 1 SOL lamport
/// - bet_type: The bet type (Block or FactionHighestLowest)
/// - use_ticket: Optional ticket type index (0-4). If None, uses SOL. If Some(index), uses ticket from free_tickets[index]
pub fn join_round(
    ctx: Context<JoinRound>, 
    amount: u64,
    bet_type: BetType,
    use_ticket: Option<u8>,
) -> Result<()> {
    msg!("🎲 [join_round] User joining round");
    msg!("   User: {}", ctx.accounts.authority.key());
    msg!("   Bet type: {:?}", bet_type);
    
    // Call internal join_round with user as payer
    internal_join_round(
        &ctx.accounts.global_game_state,
        &ctx.accounts.global_config,
        &mut ctx.accounts.player_data,
        &mut ctx.accounts.faction_state,
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

/// Internal join_round logic that can be called by both user and autominer
/// Payer can be either user wallet or autominer vault PDA
#[allow(clippy::too_many_arguments)]
fn internal_join_round<'info>(
    global_state: &Account<'info, GlobalGameSate>,
    global_config: &Account<'info, GlobalConfig>,
    player_data: &mut Account<'info, PlayerData>,
    faction_state: &mut Account<'info, FactionState>,
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
    msg!("   Validated GameSession...");
    require!(  game_session.block_assignments.iter().any(|&f| f != 0), ErrorCode::InvalidParameters);
    msg!("     ✓ Block assignments are set");
    let round_id = global_state.current_round_id;

    msg!("   Current round ID: {}, Current timestamp: {}, Round end timestamp: {}", round_id, clock.unix_timestamp, global_state.round_end_timestamp);
    
    require!(amount > 0, ErrorCode::InvalidAmount);
    msg!("   ✓ Bet amount is valid");
    
    // Validate bet type
    msg!("   Validating bet type...");
    let target_block = get_target_block_from_bet_type( &bet_type, &game_session.block_assignments)?;
    msg!("     ✓ Bet type is valid");
    let target_faction = game_session.block_assignments[target_block as usize];
    msg!("     ✓ Target faction {}", target_faction);

    assert!(target_faction == faction_state.faction_id);

    // Determine if using ticket or SOL
    let is_ticket_bet = use_ticket.is_some();
    let (net_amount, points_amount) = if let Some(ticket_type_index) = use_ticket {
        msg!("   Using ticket type index: {}", ticket_type_index);
        require!(  (ticket_type_index as usize) < player_data.free_tickets.len() && (ticket_type_index as usize) < player_data.free_tickets_remaining.len(), ErrorCode::InvalidParameters );
        
        let ticket_value = player_data.free_tickets[ticket_type_index as usize];
        require!(ticket_value > 0, ErrorCode::InvalidAmount);
        msg!("     Ticket value: {} points ({} SOL)", ticket_value, ticket_value as f64 / 1_000_000_000.0);
        
        require!( player_data.free_tickets_remaining[ticket_type_index as usize] > 0, ErrorCode::InsufficientFunds);
        msg!("     Tickets remaining: {}", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        require!(amount == ticket_value, ErrorCode::InvalidAmount);
        msg!("     ✓ Ticket amount matches ticket value");
        
        // Deduct ticket
        player_data.free_tickets_remaining[ticket_type_index as usize] -= 1;
        msg!("     ✓ Ticket deducted (remaining: {})", player_data.free_tickets_remaining[ticket_type_index as usize]);
        
        // Points bets don't have fees and don't go to prize pot
        (0, amount)
    } else {
        msg!("   Using SOL bet. Bet amount: {} lamports", amount);
        
        // Calculate fees using protocol_fee_pct from GlobalConfig
        let (net, fee_amount) = handle_fee(amount, global_config.sol_fee_config.protocol_fee_pct as u64)?;
        msg!("     ✓ Fees calculated (net: {} lamports, fee: {} lamports)", net, fee_amount);

        // Calculate faction staker fees (split between dbtc and LP stakers)
        let stakers_fee = fee_amount * global_config.sol_fee_config.stakers_pct as u64 / M_HUNDRED;
        let dbtc_reward_inc = helper::mul_div(stakers_fee / 2, INDEX_PRECISION, faction_state.total_dbtc_hashpower)?;
        faction_state.dbtc_sol_reward_index += dbtc_reward_inc;

        let lp_reward_inc = helper::mul_div(stakers_fee / 2, INDEX_PRECISION, faction_state.total_lp_hashpower)?;
        faction_state.lp_sol_reward_index += lp_reward_inc;

        // Transfer all fees to sol_treasury
        helper::transfer_to_sol_treasury(payer, sol_treasury, system_program, fee_amount)?;
        msg!("     ✓ Fee transferred");    

        // Transfer net amount to prize pot
        **sol_prize_pot_vault.try_borrow_mut_lamports()? += net;
        **payer.try_borrow_mut_lamports()? -= net;
        msg!("     ✓ Net amount transferred to prize pot");
        
        (net, net)
    };

    // Initialize or update UserGameBet PDA
    msg!("   Processing user bet account...");
    let is_new_bet = user_game_bet.owner == Pubkey::default();
    if is_new_bet {
        user_game_bet.owner = owner_key;
        user_game_bet.round_id = round_id;
        user_game_bet.bet_type = bet_type.clone();
        user_game_bet.sol_bet_amount = 0;
        user_game_bet.bump = user_game_bet_bump;
    }
    msg!("     ✓ User bet account initialized");
    
    // Update bet amount (SOL bet amount only, points tracked separately)
    if !is_ticket_bet {
        user_game_bet.sol_bet_amount += net_amount;
        msg!("     User SOL bet amount: {}", user_game_bet.sol_bet_amount);
    }
    
    // Update block tracking arrays (0-indexed: block 1-24 -> index 0-23)
    let block_index = (target_block - 1) as usize;
    require!(block_index < NUM_BLOCKS, ErrorCode::InvalidParameters);
    
    if is_new_bet {
        game_session.user_block_indexes[block_index] += 1;
        msg!("     User count for block {}: {}", target_block, game_session.user_block_indexes[block_index]);
    }
    
    // Update SOL bet tracking
    if !is_ticket_bet {
        game_session.sol_bets_indexes[block_index] += net_amount;
        game_session.total_sol_bets += net_amount;
        msg!("     SOL bet for block {}: {} (total: {})", target_block, net_amount, game_session.sol_bets_indexes[block_index]);
    }
    
    // Update Points bet tracking
    game_session.points_bets_indexes[block_index] += points_amount;
    game_session.total_points_bets += points_amount;
    msg!("     Points bet for block {}: {} (total: {})", target_block, points_amount, game_session.points_bets_indexes[block_index]);

    // Update PlayerData to track this round
    msg!("   Updating PlayerData for round {}...", round_id);
    if !player_data.sol_bets_rounds.contains(&round_id) {
        player_data.rounds_played += 1;
        player_data.sol_bets_rounds.push(round_id);
        player_data.sol_bets_amounts.push(0);
        msg!("     Added round {} to player's active rounds", round_id);
    }
    
    // Update the bet amount for this round in PlayerData
    if let Some(index) = player_data.sol_bets_rounds.iter().position(|&r| r == round_id) {
        player_data.sol_bets_amounts[index] += points_amount;
        msg!("     Player bet amount for round {}: {} lamports", round_id, player_data.sol_bets_amounts[index]);
    }
    
    // Update cumulative statistics
    if is_ticket_bet {
        player_data.total_points_bet += points_amount;
        msg!("     Player total points bet: {}", player_data.total_points_bet);
    } else {
        player_data.total_sol_bet += net_amount;
        msg!("     Player total SOL bet: {}", player_data.total_sol_bet);
    }
    
    msg!("   ✓ Bet placed: {} lamports on block {} (bet_type: {:?})", amount, target_block, bet_type);
     
    
    Ok(())
}
 
  

/// Get the target block ID from bet_type
/// For Block bets, returns the block_id directly
/// For FactionHighestLowest bets, finds the faction's blocks and returns highest/lowest
fn get_target_block_from_bet_type(bet_type: &BetType, block_assignments: &[u8; NUM_BLOCKS],) -> Result<u8> {
    match bet_type {
        BetType::Block { block_id } => {
            require!(*block_id >= 1 && *block_id <= NUM_BLOCKS as u8, ErrorCode::InvalidParameters);
            Ok(*block_id)
        }
        BetType::FactionHighestLowest { faction_id, is_highest } => {
            require!((*faction_id as usize) < block_assignments.len(), ErrorCode::InvalidParameters);
            // Find the two blocks assigned to this faction
            let mut faction_blocks: Vec<u8> = Vec::new();
            for (block_idx, assigned_faction) in block_assignments.iter().enumerate() {
                if *assigned_faction == *faction_id {
                    faction_blocks.push((block_idx + 1) as u8); // block_id is 1-indexed
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
    }
}

fn handle_fee(amount: u64, protocol_fee_pct: u64) -> Result<(u64, u64)> {
    let fee = amount * protocol_fee_pct / M_HUNDRED;
    let net_amount = amount - fee;

    msg!("     Protocol fee ({}%): {} lamports", protocol_fee_pct, fee);
    msg!("     Net amount (after fee): {} lamports", net_amount);

    return Ok((net_amount, fee));
}
 

fn init_user_bet(current_round_id: u64, is_new_bet: bool, user_bet: &mut UserGameBet, bet_type: BetType, authority: &Signer,  bumps: u8) -> Result<()> {
    if is_new_bet {
        msg!("     Creating new bet account for round {}", current_round_id);
        user_bet.owner = authority.key();
        user_bet.round_id = current_round_id;
        user_bet.bet_type = bet_type.clone();
        user_bet.sol_bet_amount = 0;
        user_bet.bump = bumps;
    } else {
        msg!("     Updating existing bet account for round {}", current_round_id);
        require!(
            user_bet.bet_type == bet_type,
            ErrorCode::InvalidParameters
        );
    }
    return Ok(());
}

















 
/// Claim rewards for a user after round ends
/// Checks if user won based on their bet type and the winning block
pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    msg!("💰 [claim_rewards] User claiming rewards");
    msg!("   User: {}", ctx.accounts.user_wallet.key());
    
    let global_state = &ctx.accounts.global_game_state;
    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    
    msg!("   User bet round ID: {}", user_bet.round_id);
    msg!("   Last completed round ID: {}", global_state.last_round_id);
    msg!("   GameSession round ID: {}", game_session.round_id);
    
    // Check user bet is for the last completed round
    require!(
        user_bet.round_id == global_state.last_round_id,
        ErrorCode::InvalidRound
    );
    msg!("   ✓ User bet is for last completed round");
    
    require!(
        game_session.round_id == global_state.last_round_id,
        ErrorCode::InvalidRound
    );
    msg!("   ✓ GameSession matches last completed round");
    
    // Determine if user won by checking if their target block matches winning block
    msg!("   Determining user's target block from bet type...");
    let target_block = user_bet
        .get_target_block(&game_session.block_assignments)
        .ok_or(ErrorCode::InvalidParameters)?;
    msg!("     Target block: {}", target_block);
    msg!("     Winning block: {}", game_session.winning_block);
    
    let is_winner = target_block == game_session.winning_block;
    msg!("     Is winner: {}", is_winner);
    
    // Find the other block with the same faction (for same-faction distribution)
    msg!("   Finding same-faction other block...");
    let winning_faction_id = game_session.winning_faction_id;
    msg!("     Winning faction ID: {}", winning_faction_id);
    let mut same_faction_other_block = 0u8;
    for (block_idx, &assigned_faction) in game_session.block_assignments.iter().enumerate() {
        if assigned_faction == winning_faction_id && (block_idx + 1) as u8 != game_session.winning_block {
            same_faction_other_block = (block_idx + 1) as u8;
            break;
        }
    }
    msg!("     Same-faction other block: {}", same_faction_other_block);
    
    // Determine if user bet on the same-faction other block
    let is_same_faction_other_block = !is_winner && target_block == same_faction_other_block;
    msg!("     Is same-faction other block: {}", is_same_faction_other_block);
    
    // Calculate SOL payout (winners only, using reward index)
    msg!("   Calculating SOL rewards...");
    let mut sol_reward = 0u64;
    let user_points = user_bet.sol_bet_amount; // points_amount tracked in sol_bet_amount for historical rounds
    
    if is_winner {
        msg!("     User is winner - calculating SOL reward using reward index...");
        // Winners get SOL based on reward index
        if game_session.sol_rewards_index > 0 && user_points > 0 {
            msg!("       SOL reward index: {}", game_session.sol_rewards_index);
            msg!("       User points: {}", user_points);
            sol_reward = helper::mul_div(user_points, game_session.sol_rewards_index as u64, INDEX_PRECISION)? as u64;
            msg!("       Calculated SOL reward: {} lamports", sol_reward);
            
            // Transfer SOL reward
            let prize_pot_before = ctx.accounts.sol_prize_pot_vault.lamports();
            **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? += sol_reward;
            **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? -= sol_reward;
            let prize_pot_after = ctx.accounts.sol_prize_pot_vault.lamports();
            msg!("       ✓ SOL transferred: prize pot {} -> {} lamports (-{})", prize_pot_before, prize_pot_after, sol_reward);
        } else {
            msg!("       No SOL reward (index = 0 or user_points = 0)");
        }
    } else {
        msg!("     User is not winner - no SOL reward");
    }
    
    // Calculate DogeBtc payout using reward indexes from GameSession
    msg!("   Calculating DogeBtc rewards...");
    let mut dbtc_reward = 0u64;
    let user_points = user_bet.sol_bet_amount; // points_amount tracked in sol_bet_amount for historical rounds
    
    if is_winner {
        msg!("     User is winner - calculating winner pool reward using reward index...");
        // Winners get dbtc based on reward index
        if game_session.dbtc_rewards_index > 0 && user_points > 0 {
            msg!("       Winner reward index: {}", game_session.dbtc_rewards_index);
            msg!("       User points: {}", user_points);
            dbtc_reward = helper::mul_div(user_points, game_session.dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
            msg!("       Winner pool reward: {} tokens", dbtc_reward);
        } else {
            msg!("       No winner pool reward (index = 0 or user_points = 0)");
        }
    } else if is_same_faction_other_block {
        msg!("     User bet on same-faction other block - calculating same-faction pool reward...");
        // Same-faction other block bettors get dbtc based on same-faction reward index
        if game_session.same_faction_dbtc_rewards_index > 0 && user_points > 0 {
            msg!("       Same-faction reward index: {}", game_session.same_faction_dbtc_rewards_index);
            msg!("       User points: {}", user_points);
            dbtc_reward = helper::mul_div(user_points, game_session.same_faction_dbtc_rewards_index as u64, INDEX_PRECISION)? as u64;
            msg!("       Same-faction reward: {} tokens", dbtc_reward);
        } else {
            msg!("       No same-faction reward (index = 0 or user_points = 0)");
        }
    } else {
        msg!("     User is not winner or same-faction other block - no DogeBtc reward");
    }
    
    msg!("   Total DogeBtc reward: {} tokens", dbtc_reward);
    
    // Add DogeBtc reward to pending_dbtc_rewards (user will claim later via claim_dbtc_rewards with refining fee)
    let player_data = &mut ctx.accounts.player_data;
    if dbtc_reward > 0 {
        msg!("   Processing DogeBtc rewards...");
        
        // Check if user has a referrer (not system referral account)
        let has_referrer = player_data.referral_code != ctx.accounts.system_program.key();
        let referral_fee = if has_referrer {
            let fee = dbtc_reward * 5 / 100; // 5% referral fee
            msg!("     Referral fee (5%): {} dbtc", fee);
            
            // Add fee to referrer's pending rewards
            if let Some(referrer_rewards) = &mut ctx.accounts.referrer_rewards {
                referrer_rewards.pending_dbtc_rewards += fee;
                referrer_rewards.total_dbtc_earned += fee;
                msg!("     Added {} dbtc to referrer's rewards", fee);
            }
            fee
        } else {
            0
        };
        
        let player_dbtc = dbtc_reward - referral_fee;
        player_data.pending_dbtc_rewards += player_dbtc;
        msg!("   Added {} DogeBtc to player's pending_dbtc_rewards (after {} referral fee)", player_dbtc, referral_fee);
        msg!("     Pending DogeBtc rewards: {} tokens", player_data.pending_dbtc_rewards);
    } else {
        msg!("   No DogeBtc reward to add to pending rewards");
    }
    
    // Update player stats
    msg!("   Updating player statistics...");
    let player_data = &mut ctx.accounts.player_data;
    if is_winner {
        player_data.rounds_won += 1;
        msg!("     Rounds won: {}", player_data.rounds_won);
        
        player_data.total_sol_won += sol_reward;
        msg!("     Total SOL won: {} lamports (+{})", player_data.total_sol_won, sol_reward);
    }
    
    // Remove round from player's active rounds list
    msg!("   Removing round from player's active rounds list...");
    if let Some(index) = player_data.sol_bets_rounds.iter().position(|&r| r == user_bet.round_id) {
        let old_count = player_data.sol_bets_rounds.len();
        player_data.sol_bets_rounds.remove(index);
        player_data.sol_bets_amounts.remove(index);
        msg!("     Removed round {} from active rounds (count: {} -> {})", user_bet.round_id, old_count, player_data.sol_bets_rounds.len());
    } else {
        msg!("     ⚠️ Round {} not found in active rounds list", user_bet.round_id);
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
    msg!("   Target Block: {}, Winning Block: {}", target_block, game_session.winning_block);
    msg!("   Winner: {}, Same-Faction Other Block: {}", is_winner, is_same_faction_other_block);
    msg!("   SOL reward: {} lamports (transferred)", sol_reward);
    msg!("   DogeBtc reward: {} tokens (added to pending_dbtc_rewards, claim via claim_dbtc_rewards)", dbtc_reward);
    
    Ok(())
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
            &mut ctx.accounts.faction_state,
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
#[instruction(amount: i128, user_pubkey: Pubkey)]
pub struct UpdatePersonalHashpower<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user_pubkey.as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,
    
    /// CHECK: Verified via constraint that this is the mooneconomy program
    pub mooneconomy_program: AccountInfo<'info>,
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
    
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump
    )]
    pub faction_state: Account<'info, FactionState>,
    
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

#[derive(Accounts)]
pub struct CrankEndSurge<'info> {
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
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    /// CHECK: All 11 faction states passed as remaining_accounts
    /// CHECK: SOL prize pot vault
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: Motherlode pot vault
    #[account(
        mut,
        seeds = [MOTHERLODE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub motherlode_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: DogeBtc emission vault
    #[account(
        mut,
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_emission_vault: UncheckedAccount<'info>,
    
    /// CHECK: MoonEconomy program for CPI
    pub mooneconomy_program: AccountInfo<'info>,
    
    /// CHECK: MoonEconomy staker reward vault
    #[account(mut)]
    pub mooneconomy_staker_reward_vault: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
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
        seeds = [PLAYER_DATA_SEED.as_ref(), user_wallet.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        mut,
        close = user_wallet,
        seeds = [USER_GAME_BET_SEED.as_ref(), user_wallet.key().as_ref(), &global_game_state.last_round_id.to_le_bytes()],
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
    
    /// CHECK: Motherlode pot vault
    #[account(
        mut,
        seeds = [MOTHERLODE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub motherlode_pot_vault: UncheckedAccount<'info>,
    
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
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump
    )]
    pub faction_state: Account<'info, FactionState>,
    
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
 