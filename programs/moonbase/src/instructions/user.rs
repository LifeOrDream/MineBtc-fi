use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::errors::ErrorCode;
use crate::state::*;
use crate::instructions::helper;




// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8, referral_code: Option<Pubkey>) -> Result<()> {
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &mut ctx.accounts.global_config;
    
    // Increment total players count
    global_config.total_players = global_config.total_players.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    // Initialize player data
    player_data.owner = ctx.accounts.authority.key();
    player_data.bump = ctx.bumps.player_data;
    player_data.faction_id = faction_id;
    player_data.personal_passive_hashpower = 0;
    
    // Handle referral code logic
    let referrer_pubkey = if let Some(ref_code) = referral_code {
        // Validate referral code is not the same as owner
        require!(
            ref_code != ctx.accounts.authority.key(),
            ErrorCode::ReferralCannotBeSameAsOwner
        );
        
        // Update referrer's referral count if referrer_rewards account is provided
        if let Some(ref mut referrer_rewards) = ctx.accounts.referrer_rewards {
            // Validate that the referrer_rewards account belongs to the referral_code
            require!(
                referrer_rewards.owner == ref_code,
                ErrorCode::InvalidReferralAccount
            );
            
            referrer_rewards.referrals_count = referrer_rewards
                .referrals_count
                .checked_add(1)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
            
            msg!("Referral recorded: {} referred by {}", ctx.accounts.authority.key(), ref_code);
        }
        
        // Set player's referral code
        player_data.referral_code = ref_code;
        ref_code
    } else {
        // No referral code provided, use system referral account
        // System referral account PDA: [REFERRAL_REWARDS_SEED, system_program.key()]
        let system_referral_pubkey = ctx.accounts.system_program.key();
        player_data.referral_code = system_referral_pubkey;
        system_referral_pubkey
    };
    
    // Initialize empty vectors for tracking rounds
    player_data.sol_bets_rounds = Vec::new();
    player_data.sol_bets_amounts = Vec::new();
    
    // Initialize statistics
    player_data.rounds_played = 0;
    player_data.rounds_won = 0;
    player_data.total_sol_bet = 0;
    player_data.total_points_bet = 0;
    player_data.total_sol_won = 0;
    player_data.total_dbtc_won = 0;
    
    // Initialize reward debt tracking
    player_data.last_claimed_passive_dbtc_index = 0;
    player_data.last_claimed_passive_sol_index = 0;
    
    // Initialize new player's referral rewards account
    let new_player_rewards = &mut ctx.accounts.new_player_rewards;
    new_player_rewards.owner = ctx.accounts.authority.key();
    new_player_rewards.total_sol_earned = 0;
    new_player_rewards.referrals_count = 0;
    new_player_rewards.bump = ctx.bumps.new_player_rewards;
    
    msg!("Player initialized: {} for faction {}", ctx.accounts.authority.key(), faction_id);
    if referral_code.is_some() {
        msg!("  Referral code: {}", referrer_pubkey);
    } else {
        msg!("  Using system referral account: {}", referrer_pubkey);
    }
    
    Ok(())
}



/// Join a round by betting SOL
/// Users can bet on either:
/// - A specific block (block_id: 1-24)
/// - A faction + highest/lowest option (faction_id + is_highest)
pub fn join_round(
    ctx: Context<JoinRound>, 
    amount: u64,
    bet_type: BetType,
) -> Result<()> {
    let global_state = &mut ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    
    // Check round is active
    require!(
        clock.unix_timestamp < global_state.round_end_timestamp,
        ErrorCode::RoundEnded
    );
    
    require!(amount > 0, ErrorCode::InvalidAmount);
    
    // Validate bet type
    match &bet_type {
        BetType::Block { block_id } => {
            require!(
                *block_id >= 1 && *block_id <= NUM_BLOCKS as u8,
                ErrorCode::InvalidParameters
            );
        }
        BetType::FactionHighestLowest { faction_id, .. } => {
            require!(
                (*faction_id as usize) < global_config.supported_factions.len(),
                ErrorCode::InvalidFactionId
            );
        }
    }
    
    let player_data = &mut ctx.accounts.player_data;
    let game_session = &mut ctx.accounts.game_session;
    
    // Calculate fees using protocol_fee_pct from GlobalConfig
    let fee = amount
        .checked_mul(global_config.sol_fee_config.protocol_fee_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let net_amount = amount
        .checked_sub(fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Transfer all fees to sol_treasury (will be distributed via withdraw_sol_fees_internal)
    helper::transfer_to_sol_treasury(
        &ctx.accounts.user_wallet.to_account_info(),
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        fee,
    )?;
    
    // Transfer net amount to prize pot
    **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? += net_amount;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= net_amount;
    
    // Initialize or update UserGameBet PDA
    let user_bet = &mut ctx.accounts.user_game_bet;
    let is_new_bet = user_bet.owner == Pubkey::default();
    
    if is_new_bet {
        user_bet.owner = ctx.accounts.authority.key();
        user_bet.round_id = global_state.current_round_id;
        user_bet.bet_type = bet_type.clone();
        user_bet.sol_bet_amount = 0;
        user_bet.bump = ctx.bumps.user_game_bet;
    } else {
        // Validate that bet type matches (users can't change bet type for existing bet)
        require!(
            user_bet.bet_type == bet_type,
            ErrorCode::InvalidParameters
        );
    }
    
    require!(
        user_bet.round_id == global_state.current_round_id,
        ErrorCode::InvalidRound
    );
    
    // Update bet amount
    user_bet.sol_bet_amount = user_bet
        .sol_bet_amount
        .checked_add(net_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Validate GameSession exists and matches current round
    require!(
        game_session.round_id == global_state.current_round_id,
        ErrorCode::InvalidRound
    );
    
    // Validate that block assignments are set (round has been initialized by crank)
    require!(
        game_session.block_assignments.iter().any(|&f| f != 0),
        ErrorCode::InvalidParameters
    );
    
    // Add this bet to the round's bet indexes if it's a new bet
    if is_new_bet {
        game_session.sol_bets_indexes.push(user_bet.round_id);
    }
    
    // Update GameSession total bets for this round
    game_session.total_sol_bets = game_session
        .total_sol_bets
        .checked_add(net_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Update PlayerData to track this round (if not already tracked)
    let round_id = global_state.current_round_id;
    if !player_data.sol_bets_rounds.contains(&round_id) {
        player_data.sol_bets_rounds.push(round_id);
        player_data.sol_bets_amounts.push(0);
    }
    
    // Update the bet amount for this round in PlayerData
    if let Some(index) = player_data.sol_bets_rounds.iter().position(|&r| r == round_id) {
        player_data.sol_bets_amounts[index] = player_data.sol_bets_amounts[index]
            .checked_add(net_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    // Update cumulative statistics
    player_data.total_sol_bet = player_data.total_sol_bet
        .checked_add(net_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Update faction state (cumulative) - only if betting on a faction
    // Note: Faction state updates should be handled separately based on the bet faction
    // The faction_state account passed is for the player's faction, not necessarily the bet faction
    // For now, we skip faction state updates here - they should be handled in a separate function
    // that processes all bets and updates faction states accordingly
    
    // Update global state (cumulative)
    global_state.total_sol_bets = global_state.total_sol_bets
        .checked_add(net_amount as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    msg!(
        "User {} bet {} SOL (net) in round {} with bet type: {:?}",
        ctx.accounts.authority.key(),
        net_amount,
        round_id,
        bet_type
    );
    
    Ok(())
}



 
/// Claim rewards for a user after round ends
/// Checks if user won based on their bet type and the winning block
pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let global_state = &ctx.accounts.global_game_state;
    let game_session = &ctx.accounts.game_session;
    let user_bet = &ctx.accounts.user_game_bet;
    
    // Check user bet is for the last completed round
    require!(
        user_bet.round_id == global_state.last_round_id,
        ErrorCode::InvalidRound
    );
    
    require!(
        game_session.round_id == global_state.last_round_id,
        ErrorCode::InvalidRound
    );
    
    // Determine if user won by checking if their target block matches winning block
    let target_block = user_bet
        .get_target_block(&game_session.block_assignments)
        .ok_or(ErrorCode::InvalidParameters)?;
    
    let is_winner = target_block == game_session.winning_block;
    
    // Find the other block with the same faction (for same-faction distribution)
    let winning_faction_id = game_session.winning_faction_id;
    let mut same_faction_other_block = 0u8;
    for (block_idx, &assigned_faction) in game_session.block_assignments.iter().enumerate() {
        if assigned_faction == winning_faction_id && (block_idx + 1) as u8 != game_session.winning_block {
            same_faction_other_block = (block_idx + 1) as u8;
            break;
        }
    }
    
    // Determine if user bet on the same-faction other block
    let is_same_faction_other_block = !is_winner && target_block == same_faction_other_block;
    
    // Calculate SOL payout (winners only)
    let mut sol_reward = 0u64;
    if is_winner && game_session.total_sol_bet_on_winner > 0 {
        sol_reward = (user_bet.sol_bet_amount as u128)
            .checked_mul(game_session.total_sol_pot_net as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(game_session.total_sol_bet_on_winner as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        
        // Transfer SOL reward
        **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? += sol_reward;
        **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? -= sol_reward;
    }
    
    // Calculate DogeBtc payout according to ORE tokenomics
    let mut dbtc_reward = 0u64;
    let global_config = &ctx.accounts.global_config;
    
    if is_winner {
        // Winners get dbtc_winners_pct (pro-rata to SOL bet amount)
        if game_session.total_sol_bet_on_winner > 0 {
            dbtc_reward = (user_bet.sol_bet_amount as u128)
                .checked_mul(game_session.dbtc_winner_pool as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(game_session.total_sol_bet_on_winner as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        }
        
        // Add motherlode payout if hit (only winners get motherlode)
        if game_session.motherlode_hit && game_session.total_sol_bet_on_winner > 0 {
            let motherlode_share = (user_bet.sol_bet_amount as u128)
                .checked_mul(game_session.motherlode_pot_size_on_hit as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(game_session.total_sol_bet_on_winner as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
            
            dbtc_reward = dbtc_reward
                .checked_add(motherlode_share)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    } else if is_same_faction_other_block {
        // Same-faction other block bettors get dbtc_same_faction_pct (pro-rata to SOL bet amount)
        if game_session.total_sol_bet_on_losers > 0 {
            dbtc_reward = (user_bet.sol_bet_amount as u128)
                .checked_mul(game_session.dbtc_loser_pool as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(game_session.total_sol_bet_on_losers as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        }
    }
    // Other bettors get nothing
    
    // Apply refining fee (charged on claim, distributed to other unclaimed users)
    let refining_fee_pct = global_config.dbtc_dist_config.refining_fee;
    let refining_fee = if dbtc_reward > 0 && refining_fee_pct > 0 {
        (dbtc_reward as u128)
            .checked_mul(refining_fee_pct as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(ErrorCode::ArithmeticOverflow)? as u64
    } else {
        0
    };
    
    let dbtc_reward_after_fee = dbtc_reward
        .checked_sub(refining_fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Transfer DogeBtc if reward > 0
    if dbtc_reward_after_fee > 0 {
        // TODO: Implement token transfer with proper PDA signing
        // Transfer from dbtc_emission_vault to user's token account
        msg!("DogeBtc reward: {} (after refining fee: {}), refining fee: {} (to be distributed)", 
            dbtc_reward_after_fee, refining_fee, refining_fee);
    }
    
    // TODO: Distribute refining_fee to other users with unclaimed rewards
    // This requires tracking all unclaimed rewards globally and distributing proportionally
    // For now, refining fee is calculated but distribution is deferred
    
    // Update player stats
    let player_data = &mut ctx.accounts.player_data;
    if is_winner {
        player_data.rounds_won = player_data.rounds_won
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        player_data.total_sol_won = player_data.total_sol_won
            .checked_add(sol_reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    player_data.total_dbtc_won = player_data.total_dbtc_won
        .checked_add(dbtc_reward_after_fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Remove round from player's active rounds list
    if let Some(index) = player_data.sol_bets_rounds.iter().position(|&r| r == user_bet.round_id) {
        player_data.sol_bets_rounds.remove(index);
        player_data.sol_bets_amounts.remove(index);
    }
    
    // Close bet account and return rent
    let signer_key = ctx.accounts.user_wallet.key();
    let rent = Rent::get()?.minimum_balance(UserGameBet::LEN);
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? += rent;
    
    msg!(
        "User {} claimed rewards: {} SOL, {} DogeBtc (after {} refining fee) (Round {}, Target Block: {}, Winning Block: {}, Winner: {}, Same-Faction Other Block: {})",
        signer_key,
        sol_reward,
        dbtc_reward_after_fee,
        refining_fee,
        user_bet.round_id,
        target_block,
        game_session.winning_block,
        is_winner,
        is_same_faction_other_block
    );
    
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
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let global_config = &ctx.accounts.global_config;
    
    require!(!bet_types.is_empty(), ErrorCode::InvalidParameters);
    require!(
        bet_types.len() <= AutominerVault::MAX_BET_TYPES,
        ErrorCode::InvalidParameters
    );
    require!(bet_amount_per_bet > 0, ErrorCode::InvalidAmount);
    require!(num_rounds > 0, ErrorCode::InvalidAmount);
    
    // Validate bet types
    for bet_type in &bet_types {
        match bet_type {
            BetType::Block { block_id } => {
                require!(
                    *block_id >= 1 && *block_id <= NUM_BLOCKS as u8,
                    ErrorCode::InvalidParameters
                );
            }
            BetType::FactionHighestLowest { faction_id, .. } => {
                require!(
                    (*faction_id as usize) < global_config.supported_factions.len(),
                    ErrorCode::InvalidFactionId
                );
            }
        }
    }
    
    autominer_vault.owner = ctx.accounts.authority.key();
    autominer_vault.bet_types = bet_types.clone();
    autominer_vault.bet_amount_per_bet = bet_amount_per_bet;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.last_bet_round_id = 0;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    
    // Calculate total SOL needed: (bet_amount_per_bet * num_bet_types) * num_rounds + rent
    let sol_per_round = bet_amount_per_bet
        .checked_mul(bet_types.len() as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let total_sol = sol_per_round
        .checked_mul(num_rounds as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let total_transfer = total_sol
        .checked_add(rent)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Transfer SOL to vault
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? += total_transfer;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= total_transfer;
    
    msg!(
        "Autominer initialized: {} bet types, {} SOL per bet, {} rounds ({} SOL total)",
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
    let global_state = &ctx.accounts.global_game_state;
    let clock = Clock::get()?;
    
    // Read values before mutable borrow
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    let last_bet_round_id = ctx.accounts.autominer_vault.last_bet_round_id;
    let num_bets = ctx.accounts.autominer_vault.bet_types.len();
    let bet_amount_per_bet = ctx.accounts.autominer_vault.bet_amount_per_bet;
    
    require!(
        rounds_remaining > 0,
        ErrorCode::NoRoundsRemaining
    );
    
    // Check round hasn't ended
    require!(
        clock.unix_timestamp < global_state.round_end_timestamp,
        ErrorCode::RoundEnded
    );
    
    // Check bets haven't been placed for this round already
    require!(
        last_bet_round_id != global_state.current_round_id,
        ErrorCode::InvalidRound
    );
    
    // Calculate total SOL needed for this round
    let sol_per_round = bet_amount_per_bet
        .checked_mul(num_bets as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Check vault has enough SOL
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let available_sol = vault_lamports
        .checked_sub(rent)
        .ok_or(ErrorCode::InsufficientFunds)?;
    
    require!(
        available_sol >= sol_per_round,
        ErrorCode::InsufficientFunds
    );
    
    // Deduct SOL for bets (protocol fee will be deducted in join_round)
    // For now, we'll deduct the full amount - actual implementation should call join_round
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= sol_per_round;
    
    // Now borrow mutably to update state
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    
    // Mark bets as placed for this round
    let current_round_id = global_state.current_round_id;
    autominer_vault.last_bet_round_id = current_round_id;
    
    // Decrement rounds remaining
    let new_rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    autominer_vault.rounds_remaining = new_rounds_remaining;
    
    // If no rounds remaining, close vault and return remaining SOL
    if new_rounds_remaining == 0 {
        let remaining_sol = ctx.accounts.autominer_vault.to_account_info().lamports()
            .checked_sub(rent)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
    }
    
    msg!(
        "Autominer bets executed: {} bets of {} SOL each for round {}. Rounds remaining: {}",
        num_bets,
        bet_amount_per_bet,
        current_round_id,
        new_rounds_remaining
    );
    
    // TODO: In production, implement CPI calls to join_round for each bet type:
    // for bet_type in &autominer_vault.bet_types {
    //     join_round_cpi(ctx, autominer_vault.bet_amount_per_bet, bet_type.clone())?;
    // }
    
    Ok(())
}

/// Cancel autominer vault
pub fn cancel_autominer(ctx: Context<CancelAutominer>) -> Result<()> {
    let autominer_vault = &ctx.accounts.autominer_vault;
    
    require!(
        autominer_vault.owner == ctx.accounts.owner.key(),
        ErrorCode::Unauthorized
    );
    
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    
    // Return all SOL to owner
    **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += vault_lamports;
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= vault_lamports;
    
    msg!("Autominer cancelled. {} SOL returned to owner", vault_lamports);
    
    Ok(())
} 















// /// Update personal hashpower (CPI-only, called by mooneconomy program)
// pub fn update_personal_hashpower(
//     ctx: Context<UpdatePersonalHashpower>,
//     amount: i128,
//     user_pubkey: Pubkey,
// ) -> Result<()> {
//     // Security: This must be CPI-only, callable only by mooneconomy program
//     // The caller is verified via the mooneconomy_program account
    
//     let player_data = &mut ctx.accounts.player_data;
//     let faction_state = &mut ctx.accounts.faction_state;
    
//     require!(
//         player_data.owner == user_pubkey,
//         ErrorCode::InvalidOwner
//     );

//     require!(
//         player_data.faction_id == faction_state.faction_id,
//         ErrorCode::InvalidFactionId
//     );
    
//     // Update player's personal hashpower (saturating arithmetic)
//     if amount > 0 {
//         player_data.personal_passive_hashpower = player_data
//             .personal_passive_hashpower
//             .saturating_add(amount as u128);
//         faction_state.total_passive_hashpower = faction_state
//             .total_passive_hashpower
//             .saturating_add(amount as u128);
//     } else {
//         let abs_amount = (-amount) as u128;
//         player_data.personal_passive_hashpower = player_data
//             .personal_passive_hashpower
//             .saturating_sub(abs_amount);
//         faction_state.total_passive_hashpower = faction_state
//             .total_passive_hashpower
//             .saturating_sub(abs_amount);
//     }

//     msg!(
//         "Updated hashpower for {}: {} (faction {} total: {})",
//         user_pubkey,
//         player_data.personal_passive_hashpower,
//         faction_state.faction_id,
//         faction_state.total_passive_hashpower
//     );

//     Ok(())
// }

 
 
// // ----------------------------------------------------------------------------------------
// // -------------- DRAGON EGG NFT MANAGEMENT -----------------------------------------------
// // ----------------------------------------------------------------------------------------

// /// Mint Dragon Egg NFT
// /// Allows users to mint an egg with specified faction and tier
// pub fn mint_dragon_egg(
//     ctx: Context<MintDragonEgg>,
//     faction_id: u8,
//     tier: u8, // 1, 2, 3, or 4
// ) -> Result<()> {
//     require!(tier >= 1 && tier <= 4, ErrorCode::InvalidParameters);
    
//     let global_config = &mut ctx.accounts.global_config;
    
//     // Validate faction_id
//     require!(
//         (faction_id as usize) < global_config.supported_factions.len(),
//         ErrorCode::InvalidFactionId
//     );
    
//     // Calculate cost per egg based on tier
//     let cost_per_egg = match tier {
//         1 => PRICE_TIER_1,
//         2 => PRICE_TIER_2,
//         3 => PRICE_TIER_3,
//         4 => PRICE_TIER_4,
//         _ => return Err(ErrorCode::InvalidParameters.into()),
//     };

//     // Transfer SOL from user to treasury
//     helper::transfer_to_sol_treasury(
//         &ctx.accounts.user.to_account_info(),
//         &ctx.accounts.sol_treasury.to_account_info(),
//         &ctx.accounts.system_program.to_account_info(),
//         cost_per_egg,
//     )?;
    
//     // Use slot and user key as seed for egg count (no global counter needed)
//     let clock = Clock::get()?;
//     let slot = clock.slot;
//     let user_key = ctx.accounts.user.key();
//     let egg_seed = slot.wrapping_add(u64::from_le_bytes([
//         user_key.as_ref()[0], user_key.as_ref()[1], user_key.as_ref()[2], user_key.as_ref()[3],
//         user_key.as_ref()[4], user_key.as_ref()[5], user_key.as_ref()[6], user_key.as_ref()[7],
//     ]));
    
//     let family_type = (tier - 1) as u8; // Tier 2->Family 1, Tier 3->Family 2, Tier 4->Family 3
    
//     // Generate DNA
//     let dna = crate::genescience::generate_genesis_dna_with_tier(
//         egg_seed,
//         &user_key,
//         slot,
//         family_type,
//     )?;

//     // Get URI for this tier and faction
//     let uri = global_config
//         .get_dragon_egg_uri(tier, faction_id)
//         .unwrap_or_else(|_| format!("https://arweave.net/dragonegg/{}/{}", tier, faction_id));

//     let name = format!("Dragon Egg #{}", egg_seed);
    
//     // Calculate multiplier based on tier
//     let multiplier = match tier {
//         2 => 150, // 1.5x
//         3 => 200, // 2.0x
//         4 => 300, // 3.0x
//         _ => 100,
//     };
    
//     // Get collection authority seeds
//     let collection_authority_bump = ctx.bumps.collection_authority;
//     let collection_authority_seeds = &[
//         crate::state::COLLECTION_AUTHORITY_SEED,
//         &[collection_authority_bump],
//     ];

//     // Create NFT via MPL Core CPI
//     msg!("🎨 Creating Dragon Egg NFT via Metaplex Core CPI");
//     msg!("   Name: {}", name);
//     msg!("   URI: {}", uri);

//         crate::mpl_core_helpers::create_mpl_core_asset(
//         &ctx.accounts.dragon_egg_asset.to_account_info(),
//         ctx.accounts
//             .dragon_egg_collection
//             .as_ref()
//             .map(|c| c.to_account_info())
//             .as_ref(),
//         &ctx.accounts.collection_authority.to_account_info(),
//         &ctx.accounts.user.to_account_info(),
//         &ctx.accounts.user.to_account_info(),
//             &ctx.accounts.system_program.to_account_info(),
//         &ctx.accounts.mpl_core_program.to_account_info(),
//             name.clone(),
//             uri.clone(),
//         Some(&[collection_authority_seeds]),
//     )?;

//     // Initialize Dragon Egg metadata
//     let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
//     egg_metadata.mint = ctx.accounts.dragon_egg_asset.key();
//         egg_metadata.power = BASE_EGG_POWER;
//         egg_metadata.dna = dna;
//         egg_metadata.incubated_player_data = None;
//     egg_metadata.multiplier = multiplier;
//     egg_metadata.faction_id = faction_id;
//         egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
//         egg_metadata.created_at = Clock::get()?.unix_timestamp;
//     egg_metadata.bump = ctx.bumps.dragon_egg_metadata;

//     // Update global dragon egg power
//     global_config.global_dragon_egg_power = global_config
//         .global_dragon_egg_power
//         .saturating_add(BASE_EGG_POWER as u64);

//         emit!(DragonEggMinted {
//         egg_metadata_account: egg_metadata.key(),
//         dragon_egg_asset_signer: ctx.accounts.dragon_egg_asset.key(),
//         owner: ctx.accounts.user.key(),
//             mint: egg_metadata.mint,
//             name,
//             uri,
//             dna,
//             initial_power: BASE_EGG_POWER,
//         multiplier,
//         faction_id,
//     });
    
//     msg!("✅ Minted Dragon Egg #{} for faction {} (Tier {})", egg_seed, faction_id, tier);
//     Ok(())
// }
 
// /// Stake a Dragon Egg to boost hashpower (if faction matches)
// /// Eggs belonging to the same faction as the player's passive staking can boost hashpower
// pub fn stake_dragon_egg(ctx: Context<StakeDragonEgg>) -> Result<()> {
//     let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
//     let player_data = &ctx.accounts.player_data;
//     let faction_state = &mut ctx.accounts.faction_state;
    
//     // Verify ownership
//     let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
//     require!(
//         nft_owner == ctx.accounts.user.key(),
//         ErrorCode::NftNotOwnedByUser
//     );

//     // Validation
//     require!(
//         egg_metadata.incubated_player_data.is_none(),
//         ErrorCode::EggAlreadyIncubated
//     );
    
//     // Check if egg faction matches player faction (required for boosting)
//         require!(
//         egg_metadata.faction_id == player_data.faction_id,
//         ErrorCode::InvalidFactionId
//     );
    
//     let current_time = Clock::get()?.unix_timestamp;
    
//     // Transfer NFT to custody PDA (lock it)
//     msg!("🔒 Transferring NFT to custody PDA (locking)");
//     crate::mpl_core_helpers::transfer_mpl_core_asset(
//         &ctx.accounts.dragon_egg_asset.to_account_info(),
//         ctx.accounts
//             .dragon_egg_collection
//             .as_ref()
//             .map(|c| c.to_account_info())
//             .as_ref(),
//         &ctx.accounts.user.to_account_info(),
//         &ctx.accounts.user.to_account_info(),
//         &ctx.accounts.egg_custody_pda.to_account_info(),
//         &ctx.accounts.mpl_core_program.to_account_info(),
//         None,
//     )?;
    
//     // Calculate hashpower boost
//     // Boost = personal_passive_hashpower * (multiplier - 100) / 100
//     // Example: 1000 hashpower with 1.5x multiplier = +500 boost
//     let base_hashpower = player_data.personal_passive_hashpower;
//     let player_owner = player_data.owner; // Store owner before mutable borrow
//     let boost_amount = if base_hashpower > 0 && egg_metadata.multiplier > 100 {
//         let multiplier_excess = (egg_metadata.multiplier as u128)
//             .saturating_sub(100);
//         base_hashpower
//             .checked_mul(multiplier_excess)
//             .ok_or(ErrorCode::ArithmeticOverflow)?
//             .checked_div(100)
//             .ok_or(ErrorCode::ArithmeticOverflow)?
//     } else {
//         0u128
//     };
    
//     if boost_amount > 0 {
//         msg!("⚡ Applying Dragon Egg multiplier boost");
//         msg!("   Base hashpower: {}", base_hashpower);
//         msg!("   Egg multiplier: {}x", egg_metadata.multiplier as f64 / 100.0);
//         msg!("   Boost amount: {}", boost_amount);
        
//         // Update player hashpower
//         let player_data_mut = &mut ctx.accounts.player_data;
//         player_data_mut.personal_passive_hashpower = player_data_mut
//             .personal_passive_hashpower
//             .saturating_add(boost_amount);
        
//         // Update faction hashpower
//         faction_state.total_passive_hashpower = faction_state
//             .total_passive_hashpower
//             .saturating_add(boost_amount);
        
//         msg!("   New hashpower: {}", player_data_mut.personal_passive_hashpower);
//         msg!("   Faction total: {}", faction_state.total_passive_hashpower);
//                     } else {
//         msg!("⚠️ No hashpower to boost (stake tokens first)");
//     }
    
//     // Update egg metadata
//     egg_metadata.incubated_player_data = Some(player_owner);
//     egg_metadata.last_update_ts = current_time;
    
//     msg!("✅ Dragon Egg staked for player {}", player_owner);
//     msg!("   Egg: {}", egg_metadata.mint);
//     msg!("   Faction: {}", egg_metadata.faction_id);
    
//     Ok(())
// }

// /// Unstake a Dragon Egg (remove hashpower boost)
// pub fn unstake_dragon_egg(ctx: Context<UnstakeDragonEgg>) -> Result<()> {
//     let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
//     let player_data = &mut ctx.accounts.player_data;
//     let faction_state = &mut ctx.accounts.faction_state;
    
//     // Verify NFT is in custody PDA
//     let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
//     require!(
//         nft_owner == ctx.accounts.egg_custody_pda.key(),
//         ErrorCode::EggNotIncubated
//     );
    
//     require!(
//         egg_metadata.incubated_player_data.is_some(),
//         ErrorCode::EggNotIncubated
//     );
    
//     require!(
//         egg_metadata.incubated_player_data.unwrap() == player_data.owner,
//         ErrorCode::Unauthorized
//         );
        
//     let current_time = Clock::get()?.unix_timestamp;
        
//     // Calculate hashpower boost to remove
//     // Current hashpower includes the boost, so we need to reverse it
//     // original = current / (multiplier / 100)
//     // boost = current - original
//     let current_hashpower = player_data.personal_passive_hashpower;
//     let boost_amount = if current_hashpower > 0 && egg_metadata.multiplier > 100 {
//         // Calculate original hashpower before boost
//         let original_hashpower = (current_hashpower as u128)
//             .checked_mul(100)
//             .ok_or(ErrorCode::ArithmeticOverflow)?
//             .checked_div(egg_metadata.multiplier as u128)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
        
//         current_hashpower.saturating_sub(original_hashpower)
//         } else {
//         0u128
//     };
    
//     if boost_amount > 0 {
//         msg!("⚡ Removing Dragon Egg multiplier boost");
//         msg!("   Current hashpower: {}", current_hashpower);
//         msg!("   Boost to remove: {}", boost_amount);
        
//         // Remove boost from player hashpower
//         player_data.personal_passive_hashpower = player_data
//             .personal_passive_hashpower
//             .saturating_sub(boost_amount);
        
//         // Remove boost from faction hashpower
//         faction_state.total_passive_hashpower = faction_state
//             .total_passive_hashpower
//             .saturating_sub(boost_amount);
        
//         msg!("   New hashpower: {}", player_data.personal_passive_hashpower);
//         msg!("   Faction total: {}", faction_state.total_passive_hashpower);
//     }
    
//     // Transfer NFT back to user (unlock it)
//     msg!("🔓 Transferring NFT back to user (unlocking)");
//     let custody_seeds = &[DRAGON_EGG_CUSTODY_SEED, &[ctx.bumps.egg_custody_pda]];
//     let signer_seeds = &[&custody_seeds[..]];
    
//     crate::mpl_core_helpers::transfer_mpl_core_asset(
//         &ctx.accounts.dragon_egg_asset.to_account_info(),
//         ctx.accounts
//             .dragon_egg_collection
//             .as_ref()
//             .map(|c| c.to_account_info())
//             .as_ref(),
//         &ctx.accounts.egg_custody_pda.to_account_info(),
//         &ctx.accounts.egg_custody_pda.to_account_info(),
//         &ctx.accounts.user.to_account_info(),
//         &ctx.accounts.mpl_core_program.to_account_info(),
//         Some(signer_seeds),
//     )?;
    
//     // Update egg metadata
//     egg_metadata.incubated_player_data = None;
//     egg_metadata.last_update_ts = current_time;
    
//     msg!("✅ Dragon Egg unstaked");
//     msg!("   Egg: {}", egg_metadata.mint);
    
//     Ok(())
// }

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(faction_id: u8, tier: u8)]
pub struct MintDragonEgg<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    /// CHECK: PDA that holds collected SOL fees
    pub sol_treasury: UncheckedAccount<'info>,

    /// Metaplex Core asset (will be created)
    #[account(mut)]
    /// CHECK: Will be created via MPL Core CPI
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        init,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_asset.key().as_ref()],
        bump
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    #[account(
        seeds = [COLLECTION_AUTHORITY_SEED],
        bump
    )]
    /// CHECK: PDA authority for the collection
    pub collection_authority: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakeDragonEgg<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (source of truth for ownership)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,
    
    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeDragonEgg<'info> {
    #[account(
        mut,
        seeds = [PLAYER_DATA_SEED.as_ref(), user.key().as_ref()],
        bump = player_data.bump,
        constraint = player_data.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub player_data: Account<'info, PlayerData>,

    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,

    /// Metaplex Core asset (currently locked in custody PDA)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,
    
    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,
    
    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
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
    
    /// CHECK: User token account for DogeBtc
    #[account(mut)]
    pub user_token_account: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub user_wallet: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
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
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    /// CHECK: Owner account (to receive remaining SOL when vault closes)
    #[account(
        mut,
        constraint = owner.key() == autominer_vault.owner @ ErrorCode::Unauthorized
    )]
    pub owner: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelAutominer<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [AUTOMINER_VAULT_SEED.as_ref(), owner.key().as_ref()],
        bump = autominer_vault.vault_bump
    )]
    pub autominer_vault: Account<'info, AutominerVault>,
    
    #[account(mut)]
    pub owner: Signer<'info>,

    pub system_program: Program<'info, System>,
}

