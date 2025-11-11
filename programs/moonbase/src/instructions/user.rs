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




// /// Crank end surge - determines winner and distributes rewards
// pub fn crank_end_surge(ctx: Context<CrankEndSurge>) -> Result<()> {
//     let global_state = &mut ctx.accounts.global_game_state;
//     let global_config = &ctx.accounts.global_config;
//     let doge_btc_mining = &ctx.accounts.doge_btc_mining;
//     let clock = Clock::get()?;
    
//     // Check round has ended
//     require!(
//         clock.unix_timestamp > global_state.round_end_timestamp,
//         ErrorCode::RoundNotEnded
//     );
    
//     // Collect all faction states and calculate scores
//     let mut faction_scores: Vec<(u8, u128)> = Vec::new();
//     let mut total_score: u128 = 0;
//     let mut winning_faction_total_bets = 0u64;
    
//     for faction_account in ctx.remaining_accounts.iter() {
//         let faction_data = faction_account.try_borrow_data()?;
//         let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
        
//         // Calculate score: passive hashpower + (active SOL bets * HASHPOWER_PER_SOL)
//         let sol_hashpower = (faction_state.total_active_sol_bets as u128)
//             .checked_mul(HASHPOWER_PER_SOL_CONSTANT)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
        
//         let score = faction_state
//             .total_passive_hashpower
//             .checked_add(sol_hashpower)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
        
//         faction_scores.push((faction_state.faction_id, score));
//         total_score = total_score
//             .checked_add(score)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
//     }
    
//     require!(!faction_scores.is_empty(), ErrorCode::NoFactions);
//     require!(total_score > 0, ErrorCode::NoBets);
    
//     // Select winner using slot as RNG
//     let slot = clock.slot;
//     let mut random_value = (slot % (total_score.min(u64::MAX as u128) as u64)) as u128;
//     let mut winning_faction_id = faction_scores[0].0;
    
//     for (faction_id, score) in faction_scores.iter() {
//         if random_value < *score {
//             winning_faction_id = *faction_id;
//             break;
//         }
//         random_value = random_value.saturating_sub(*score);
//     }
    
//     // Get winning faction total bets
//     for faction_account in ctx.remaining_accounts.iter() {
//         let faction_data = faction_account.try_borrow_data()?;
//         let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
//         if faction_state.faction_id == winning_faction_id {
//             winning_faction_total_bets = faction_state.total_active_sol_bets;
//             break;
//         }
//     }
    
//     // Split DogeBtc emission using configurable percentages from GlobalConfig
//     // Calculate emission_per_round from mining state: current_dist_rate * slots_per_round
//     // Solana has ~2 slots per second, so slots_per_round = round_duration_seconds * 2
//     let slots_per_round = (global_state.round_duration_seconds as u64)
//         .checked_mul(2)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     let emission_per_round = doge_btc_mining.current_dist_rate
//         .checked_mul(slots_per_round)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     let dbtc_stakers = emission_per_round
//         .checked_mul(global_config.dbtc_dist_config.dbtc_stakers_pct as u64)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     let dbtc_winners = emission_per_round
//         .checked_mul(global_config.dbtc_dist_config.dbtc_winners_pct as u64)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     let dbtc_same_faction = emission_per_round
//         .checked_mul(global_config.dbtc_dist_config.dbtc_same_faction_pct as u64)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     let dbtc_motherlode = emission_per_round
//         .checked_mul(global_config.dbtc_dist_config.dbtc_motherlode_pct as u64)
//         .ok_or(ErrorCode::ArithmeticOverflow)?
//         .checked_div(100)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     // Transfer to staker reward vault via CPI (will be implemented in mooneconomy)
//     // For now, we'll store the amounts in global state
    
//     // Transfer motherlode portion
//     let _motherlode_pot_vault = &ctx.accounts.motherlode_pot_vault;
//     let _dbtc_emission_vault = &ctx.accounts.dbtc_emission_vault;
    
//     // Note: Token transfers will need to be implemented with proper PDA signing
    
//     // Check motherlode hit
//     let motherlode_roll = clock.slot % MOTHERLODE_CHANCE;
//     let motherlode_hit = motherlode_roll == 0;
    
//     // Save results to global state
//     global_state.last_round_id = global_state.current_round_id;
//     global_state.winning_faction_id = winning_faction_id;
//     global_state.total_sol_pot_net = ctx.accounts.sol_prize_pot_vault.lamports();
//     global_state.total_sol_bet_on_winner = winning_faction_total_bets;
    
//     // Calculate total bets on losers
//     let mut total_loser_bets = 0u64;
//     for faction_account in ctx.remaining_accounts.iter() {
//         let faction_data = faction_account.try_borrow_data()?;
//         let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
//         if faction_state.faction_id != winning_faction_id {
//             total_loser_bets = total_loser_bets
//                 .checked_add(faction_state.total_active_sol_bets)
//                 .ok_or(ErrorCode::ArithmeticOverflow)?;
//         }
//     }
//     global_state.total_sol_bet_on_losers = total_loser_bets;
    
//     // Calculate total bets across all factions
//     let mut total_all_bets = 0u64;
//     for faction_account in ctx.remaining_accounts.iter() {
//         let faction_data = faction_account.try_borrow_data()?;
//         let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
//         total_all_bets = total_all_bets
//             .checked_add(faction_state.total_active_sol_bets)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
//     }
//     global_state.total_sol_bet_all_factions = total_all_bets;
    
//     global_state.dbtc_winner_pool = dbtc_winners;
//     global_state.dbtc_loser_pool = dbtc_same_faction;
//     global_state.motherlode_hit = motherlode_hit;
    
//     if motherlode_hit {
//         global_state.motherlode_pot_size_on_hit = ctx.accounts.motherlode_pot_vault.lamports();
//     }
    
//     // Reset for next round using round_duration_seconds from GlobalGameSate
//     global_state.current_round_id = global_state.current_round_id.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
//     global_state.round_end_timestamp = clock.unix_timestamp.checked_add(global_state.round_duration_seconds).ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     // Reset all faction active bets (faction states passed as remaining_accounts should be mutable)
//     // Note: In production, faction states should be explicitly passed as mutable accounts
//     // For now, we'll skip the reset here and handle it in the account context

//     msg!(
//         "Round {} ended. Winner: Faction {}, Motherlode: {}",
//         global_state.last_round_id,
//         winning_faction_id,
//         if motherlode_hit { "HIT!" } else { "miss" }
//     );

//     Ok(())
// }
 
// /// Claim surge rewards for a user
// pub fn claim_surge_rewards(ctx: Context<ClaimSurgeRewards>) -> Result<()> {
//     let global_state = &ctx.accounts.global_game_state;
//     let user_bet = &mut ctx.accounts.user_surge_bet;
    
//     // Check user bet is for the last completed round
//     require!(
//         user_bet.round_id == global_state.last_round_id,
//         ErrorCode::InvalidRound
//     );
    
//     let is_winner = user_bet.faction_id == global_state.winning_faction_id;
    
//     // Calculate SOL payout (winners only)
//     let mut sol_reward = 0u64;
//     if is_winner && global_state.total_sol_bet_on_winner > 0 {
//         sol_reward = (user_bet.sol_bet_amount as u128)
//             .checked_mul(global_state.total_sol_pot_net as u128)
//             .ok_or(ErrorCode::ArithmeticOverflow)?
//             .checked_div(global_state.total_sol_bet_on_winner as u128)
//             .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        
//         // Transfer SOL reward
//         **ctx.accounts.signer.to_account_info().try_borrow_mut_lamports()? += sol_reward;
//         **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? -= sol_reward;
//     }
    
//     // Calculate DogeBtc payout
//     let mut dbtc_reward = 0u64;
//     if is_winner {
//         if global_state.total_sol_bet_on_winner > 0 {
//             dbtc_reward = (user_bet.sol_bet_amount as u128)
//                 .checked_mul(global_state.dbtc_winner_pool as u128)
//                 .ok_or(ErrorCode::ArithmeticOverflow)?
//                 .checked_div(global_state.total_sol_bet_on_winner as u128)
//                 .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
//         }
//     } else {
//         if global_state.total_sol_bet_on_losers > 0 {
//             dbtc_reward = (user_bet.sol_bet_amount as u128)
//                 .checked_mul(global_state.dbtc_loser_pool as u128)
//                 .ok_or(ErrorCode::ArithmeticOverflow)?
//                 .checked_div(global_state.total_sol_bet_on_losers as u128)
//                 .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
//         }
//     }
    
//     // Add motherlode payout if hit
//     if global_state.motherlode_hit && global_state.total_sol_bet_all_factions > 0 {
//         let motherlode_share = (user_bet.sol_bet_amount as u128)
//             .checked_mul(global_state.motherlode_pot_size_on_hit as u128)
//             .ok_or(ErrorCode::ArithmeticOverflow)?
//             .checked_div(global_state.total_sol_bet_all_factions as u128)
//             .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        
//         dbtc_reward = dbtc_reward
//             .checked_add(motherlode_share)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
//     }
    
//     // Transfer DogeBtc (token transfer will be implemented with proper PDA signing)
//     // For now, we'll need to implement token transfers
    
//     // Close bet account
//     let signer_key = ctx.accounts.signer.key();
//     let rent = Rent::get()?.minimum_balance(UserGameBet::LEN);
//     **ctx.accounts.signer.to_account_info().try_borrow_mut_lamports()? += rent;
    
//     msg!(
//         "User {} claimed rewards: {} SOL, {} DogeBtc (Round {}, Winner: {})",
//         signer_key,
//         sol_reward,
//         dbtc_reward,
//         user_bet.round_id,
//         is_winner
//     );
    
//     Ok(())
// }

// /// Initialize autominer vault
// pub fn init_autominer(
//     ctx: Context<InitAutominer>,
//     sol_per_round: u64,
//     num_rounds: u32,
// ) -> Result<()> {
//     let autominer_vault = &mut ctx.accounts.autominer_vault;
//     let player_data = &ctx.accounts.player_data;
    
//     require!(sol_per_round > 0, ErrorCode::InvalidAmount);
//     require!(num_rounds > 0, ErrorCode::InvalidAmount);
    
//     autominer_vault.owner = ctx.accounts.authority.key();
//     autominer_vault.faction_id = player_data.faction_id;
//     autominer_vault.sol_per_round = sol_per_round;
//     autominer_vault.rounds_remaining = num_rounds;
//     autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    
//     // Transfer SOL to vault
//     let total_sol = sol_per_round
//         .checked_mul(num_rounds as u64)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
//     let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
//     let total_transfer = total_sol
//         .checked_add(rent)
//         .ok_or(ErrorCode::ArithmeticOverflow)?;
    
//     **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? += total_transfer;
//     **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= total_transfer;
    
//     msg!(
//         "Autominer initialized: {} SOL per round for {} rounds",
//         sol_per_round,
//         num_rounds
//     );
    
//     Ok(())
// }

// /// Execute autominer bet (keeper instruction)
// pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
//         require!(
//         ctx.accounts.autominer_vault.rounds_remaining > 0,
//         ErrorCode::NoRoundsRemaining
//     );
    
//     // Check vault has enough SOL
//     let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
//     let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
//     let available_sol = vault_lamports
//         .checked_sub(rent)
//         .ok_or(ErrorCode::InsufficientFunds)?;
    
//     let sol_per_round = ctx.accounts.autominer_vault.sol_per_round;
//     require!(
//         available_sol >= sol_per_round,
//         ErrorCode::InsufficientFunds
//     );
    
//     // Execute bet logic (similar to join_surge but from vault)
//     // This is simplified - full implementation would call join_surge logic
    
//     let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
//     ctx.accounts.autominer_vault.rounds_remaining = rounds_remaining
//         .checked_sub(1)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
        
//     // If no rounds remaining, close vault and return remaining SOL
//     if ctx.accounts.autominer_vault.rounds_remaining == 0 {
//         let remaining_sol = vault_lamports
//             .checked_sub(rent)
//             .ok_or(ErrorCode::ArithmeticOverflow)?;
        
//         **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
//         **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
//     }
    
//     msg!("Autominer bet executed. Rounds remaining: {}", ctx.accounts.autominer_vault.rounds_remaining);
    
//     Ok(())
// }

// /// Cancel autominer vault
// pub fn cancel_autominer(ctx: Context<CancelAutominer>) -> Result<()> {
//     let autominer_vault = &ctx.accounts.autominer_vault;
    
//     require!(
//         autominer_vault.owner == ctx.accounts.owner.key(),
//         ErrorCode::Unauthorized
//     );
    
//     let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    
//     // Return all SOL to owner
//     **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += vault_lamports;
//     **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= vault_lamports;
    
//     msg!("Autominer cancelled. {} SOL returned to owner", vault_lamports);
    
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
pub struct ClaimSurgeRewards<'info> {
    #[account(
        seeds = [GLOBAL_GAME_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        mut,
        close = signer,
        seeds = [USER_GAME_BET_SEED.as_ref(), signer.key().as_ref(), &global_game_state.last_round_id.to_le_bytes()],
        bump = user_surge_bet.bump
    )]
    pub user_surge_bet: Account<'info, UserGameBet>,
    
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
    pub signer: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(sol_per_round: u64, num_rounds: u32)]
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
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
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
    
    /// CHECK: Faction state for the vault's faction
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[autominer_vault.faction_id]],
        bump
    )]
    pub faction_state: UncheckedAccount<'info>,

    /// CHECK: User surge bet account
    #[account(
        init_if_needed,
        payer = autominer_vault,
        space = UserGameBet::LEN,
        seeds = [USER_GAME_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_surge_bet: Account<'info, UserGameBet>,
    
    /// CHECK: All other accounts needed for join_surge
    #[account(mut)]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: SOL treasury PDA (fees go here)
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump
    )]
    pub sol_treasury: UncheckedAccount<'info>,
    
    /// CHECK: Owner account for returning SOL when vault closes
    #[account(mut)]
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

