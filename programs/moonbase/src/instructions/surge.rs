use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::errors::ErrorCode;
use crate::state::*;

// ========================================================================================
// =============================== FACTION SURGE INSTRUCTIONS ============================
// ========================================================================================

/// Initialize a player account for the Faction Surge game
pub fn initialize_player(ctx: Context<InitializePlayer>, faction_id: u8) -> Result<()> {
    let player_data = &mut ctx.accounts.player_data;
    let global_config = &ctx.accounts.global_config;
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    player_data.owner = ctx.accounts.authority.key();
    player_data.faction_id = faction_id;
    player_data.personal_passive_hashpower = 0;
    player_data.bump = ctx.bumps.player_data;
    
    msg!("Player initialized: {} for faction {}", ctx.accounts.authority.key(), faction_id);
    Ok(())
}

/// Update personal hashpower (CPI-only, called by mooneconomy program)
pub fn update_personal_hashpower(
    ctx: Context<UpdatePersonalHashpower>,
    amount: i128,
    user_pubkey: Pubkey,
) -> Result<()> {
    // Security: This must be CPI-only, callable only by mooneconomy program
    // The caller is verified via the mooneconomy_program account
    
    let player_data = &mut ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    
    require!(
        player_data.owner == user_pubkey,
        ErrorCode::InvalidOwner
    );
    
    require!(
        player_data.faction_id == faction_state.faction_id,
        ErrorCode::InvalidFactionId
    );
    
    // Update player's personal hashpower (saturating arithmetic)
    if amount > 0 {
        player_data.personal_passive_hashpower = player_data
            .personal_passive_hashpower
            .saturating_add(amount as u128);
        faction_state.total_passive_hashpower = faction_state
            .total_passive_hashpower
            .saturating_add(amount as u128);
    } else {
        let abs_amount = (-amount) as u128;
        player_data.personal_passive_hashpower = player_data
            .personal_passive_hashpower
            .saturating_sub(abs_amount);
        faction_state.total_passive_hashpower = faction_state
            .total_passive_hashpower
            .saturating_sub(abs_amount);
    }
    
    msg!(
        "Updated hashpower for {}: {} (faction {} total: {})",
        user_pubkey,
        player_data.personal_passive_hashpower,
        faction_state.faction_id,
        faction_state.total_passive_hashpower
    );
    
    Ok(())
}

/// Join a surge round by betting SOL
pub fn join_surge(ctx: Context<JoinSurge>, amount: u64) -> Result<()> {
    let global_state = &ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    
    // Check round is active
    require!(
        clock.unix_timestamp < global_state.round_end_timestamp,
        ErrorCode::RoundEnded
    );
    
    require!(amount > 0, ErrorCode::InvalidAmount);
    
    let player_data = &ctx.accounts.player_data;
    let faction_state = &mut ctx.accounts.faction_state;
    
    // Calculate fees using protocol_fee_pct from GlobalConfig
    let fee = amount
        .checked_mul(global_config.protocol_fee_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let net_amount = amount
        .checked_sub(fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Split fee: buyback_pct%, stakers_pct%, remainder to admin
    let buyback_fee = fee
        .checked_mul(global_config.buyback_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let staker_fee = fee
        .checked_mul(global_config.stakers_pct as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let admin_fee = fee
        .checked_sub(buyback_fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(staker_fee)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Transfer fees
    **ctx.accounts.buybacks_sol_vault.to_account_info().try_borrow_mut_lamports()? += buyback_fee;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= buyback_fee;
    
    **ctx.accounts.staker_sol_reward_vault.to_account_info().try_borrow_mut_lamports()? += staker_fee;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= staker_fee;
    
    **ctx.accounts.admin_treasury.to_account_info().try_borrow_mut_lamports()? += admin_fee;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= admin_fee;
    
    // Transfer net amount to prize pot
    **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? += net_amount;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= net_amount;
    
    // Initialize or update bet slip
    let user_bet = &mut ctx.accounts.user_surge_bet;
    if user_bet.owner == Pubkey::default() {
        user_bet.owner = ctx.accounts.authority.key();
        user_bet.round_id = global_state.current_round_id;
        user_bet.faction_id = player_data.faction_id;
        user_bet.sol_bet_amount = 0;
        user_bet.bump = ctx.bumps.user_surge_bet;
    }
    
    require!(
        user_bet.round_id == global_state.current_round_id,
        ErrorCode::InvalidRound
    );
    
    user_bet.sol_bet_amount = user_bet
        .sol_bet_amount
        .checked_add(net_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Update both total_sol_bets and total_active_sol_bets (keep them in sync)
    faction_state.total_sol_bets = faction_state
        .total_sol_bets
        .checked_add(net_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    faction_state.total_active_sol_bets = faction_state.total_sol_bets;
    
    msg!(
        "User {} bet {} SOL (net) on faction {} in round {}",
        ctx.accounts.authority.key(),
        net_amount,
        player_data.faction_id,
        global_state.current_round_id
    );
    
    Ok(())
}

/// Crank end surge - determines winner and distributes rewards
pub fn crank_end_surge(ctx: Context<CrankEndSurge>) -> Result<()> {
    let global_state = &mut ctx.accounts.global_game_state;
    let global_config = &ctx.accounts.global_config;
    let clock = Clock::get()?;
    
    // Check round has ended
    require!(
        clock.unix_timestamp > global_state.round_end_timestamp,
        ErrorCode::RoundNotEnded
    );
    
    // Collect all faction states and calculate scores
    let mut faction_scores: Vec<(u8, u128)> = Vec::new();
    let mut total_score: u128 = 0;
    let mut winning_faction_total_bets = 0u64;
    
    for faction_account in ctx.remaining_accounts.iter() {
        let faction_data = faction_account.try_borrow_data()?;
        let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
        
        // Calculate score: passive hashpower + (active SOL bets * HASHPOWER_PER_SOL)
        let sol_hashpower = (faction_state.total_active_sol_bets as u128)
            .checked_mul(HASHPOWER_PER_SOL_CONSTANT)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        let score = faction_state
            .total_passive_hashpower
            .checked_add(sol_hashpower)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        faction_scores.push((faction_state.faction_id, score));
        total_score = total_score
            .checked_add(score)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    require!(!faction_scores.is_empty(), ErrorCode::NoFactions);
    require!(total_score > 0, ErrorCode::NoBets);
    
    // Select winner using slot as RNG
    let slot = clock.slot;
    let mut random_value = (slot % (total_score.min(u64::MAX as u128) as u64)) as u128;
    let mut winning_faction_id = faction_scores[0].0;
    
    for (faction_id, score) in faction_scores.iter() {
        if random_value < *score {
            winning_faction_id = *faction_id;
            break;
        }
        random_value = random_value.saturating_sub(*score);
    }
    
    // Get winning faction total bets
    for faction_account in ctx.remaining_accounts.iter() {
        let faction_data = faction_account.try_borrow_data()?;
        let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
        if faction_state.faction_id == winning_faction_id {
            winning_faction_total_bets = faction_state.total_active_sol_bets;
            break;
        }
    }
    
    // Split DogeBtc emission using emission_per_round from GlobalConfig (50/30/10/10)
    let emission_per_round = global_config.emission_per_round;
    let _dbtc_50_stakers = emission_per_round
        .checked_mul(50)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let dbtc_30_winners = emission_per_round
        .checked_mul(30)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    let dbtc_10_losers = emission_per_round
        .checked_mul(10)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Note: dbtc_10_motherlode will be transferred to motherlode pot
    let _dbtc_10_motherlode = emission_per_round
        .checked_mul(10)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Transfer to staker reward vault via CPI (will be implemented in mooneconomy)
    // For now, we'll store the amounts in global state
    
    // Transfer motherlode portion
    let _motherlode_pot_vault = &ctx.accounts.motherlode_pot_vault;
    let _dbtc_emission_vault = &ctx.accounts.dbtc_emission_vault;
    
    // Note: Token transfers will need to be implemented with proper PDA signing
    
    // Check motherlode hit
    let motherlode_roll = clock.slot % MOTHERLODE_CHANCE;
    let motherlode_hit = motherlode_roll == 0;
    
    // Save results to global state
    global_state.last_round_id = global_state.current_round_id;
    global_state.winning_faction_id = winning_faction_id;
    global_state.total_sol_pot_net = ctx.accounts.sol_prize_pot_vault.lamports();
    global_state.total_sol_bet_on_winner = winning_faction_total_bets;
    
    // Calculate total bets on losers
    let mut total_loser_bets = 0u64;
    for faction_account in ctx.remaining_accounts.iter() {
        let faction_data = faction_account.try_borrow_data()?;
        let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
        if faction_state.faction_id != winning_faction_id {
            total_loser_bets = total_loser_bets
                .checked_add(faction_state.total_active_sol_bets)
                .ok_or(ErrorCode::ArithmeticOverflow)?;
        }
    }
    global_state.total_sol_bet_on_losers = total_loser_bets;
    
    // Calculate total bets across all factions
    let mut total_all_bets = 0u64;
    for faction_account in ctx.remaining_accounts.iter() {
        let faction_data = faction_account.try_borrow_data()?;
        let faction_state: FactionState = AccountDeserialize::try_deserialize(&mut &faction_data[..])?;
        total_all_bets = total_all_bets
            .checked_add(faction_state.total_active_sol_bets)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    global_state.total_sol_bet_all_factions = total_all_bets;
    
    global_state.dbtc_winner_pool = dbtc_30_winners;
    global_state.dbtc_loser_pool = dbtc_10_losers;
    global_state.motherlode_hit = motherlode_hit;
    
    if motherlode_hit {
        global_state.motherlode_pot_size_on_hit = ctx.accounts.motherlode_pot_vault.lamports();
    }
    
    // Reset for next round using round_duration_seconds from GlobalGameSate
    global_state.current_round_id = global_state.current_round_id.checked_add(1).ok_or(ErrorCode::ArithmeticOverflow)?;
    global_state.round_end_timestamp = clock.unix_timestamp.checked_add(global_state.round_duration_seconds).ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Reset all faction active bets (faction states passed as remaining_accounts should be mutable)
    // Note: In production, faction states should be explicitly passed as mutable accounts
    // For now, we'll skip the reset here and handle it in the account context
    
    msg!(
        "Round {} ended. Winner: Faction {}, Motherlode: {}",
        global_state.last_round_id,
        winning_faction_id,
        if motherlode_hit { "HIT!" } else { "miss" }
    );
    
    Ok(())
}

/// Claim surge rewards for a user
pub fn claim_surge_rewards(ctx: Context<ClaimSurgeRewards>) -> Result<()> {
    let global_state = &ctx.accounts.global_game_state;
    let user_bet = &mut ctx.accounts.user_surge_bet;
    
    // Check user bet is for the last completed round
    require!(
        user_bet.round_id == global_state.last_round_id,
        ErrorCode::InvalidRound
    );
    
    let is_winner = user_bet.faction_id == global_state.winning_faction_id;
    
    // Calculate SOL payout (winners only)
    let mut sol_reward = 0u64;
    if is_winner && global_state.total_sol_bet_on_winner > 0 {
        sol_reward = (user_bet.sol_bet_amount as u128)
            .checked_mul(global_state.total_sol_pot_net as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(global_state.total_sol_bet_on_winner as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        
        // Transfer SOL reward
        **ctx.accounts.signer.to_account_info().try_borrow_mut_lamports()? += sol_reward;
        **ctx.accounts.sol_prize_pot_vault.to_account_info().try_borrow_mut_lamports()? -= sol_reward;
    }
    
    // Calculate DogeBtc payout
    let mut dbtc_reward = 0u64;
    if is_winner {
        if global_state.total_sol_bet_on_winner > 0 {
            dbtc_reward = (user_bet.sol_bet_amount as u128)
                .checked_mul(global_state.dbtc_winner_pool as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(global_state.total_sol_bet_on_winner as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        }
    } else {
        if global_state.total_sol_bet_on_losers > 0 {
            dbtc_reward = (user_bet.sol_bet_amount as u128)
                .checked_mul(global_state.dbtc_loser_pool as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(global_state.total_sol_bet_on_losers as u128)
                .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        }
    }
    
    // Add motherlode payout if hit
    if global_state.motherlode_hit && global_state.total_sol_bet_all_factions > 0 {
        let motherlode_share = (user_bet.sol_bet_amount as u128)
            .checked_mul(global_state.motherlode_pot_size_on_hit as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(global_state.total_sol_bet_all_factions as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
        
        dbtc_reward = dbtc_reward
            .checked_add(motherlode_share)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    // Transfer DogeBtc (token transfer will be implemented with proper PDA signing)
    // For now, we'll need to implement token transfers
    
    // Close bet account
    let signer_key = ctx.accounts.signer.key();
    let rent = Rent::get()?.minimum_balance(UserSurgeBet::LEN);
    **ctx.accounts.signer.to_account_info().try_borrow_mut_lamports()? += rent;
    
    msg!(
        "User {} claimed rewards: {} SOL, {} DogeBtc (Round {}, Winner: {})",
        signer_key,
        sol_reward,
        dbtc_reward,
        user_bet.round_id,
        is_winner
    );
    
    Ok(())
}

/// Initialize autominer vault
pub fn init_autominer(
    ctx: Context<InitAutominer>,
    sol_per_round: u64,
    num_rounds: u32,
) -> Result<()> {
    let autominer_vault = &mut ctx.accounts.autominer_vault;
    let player_data = &ctx.accounts.player_data;
    
    require!(sol_per_round > 0, ErrorCode::InvalidAmount);
    require!(num_rounds > 0, ErrorCode::InvalidAmount);
    
    autominer_vault.owner = ctx.accounts.authority.key();
    autominer_vault.faction_id = player_data.faction_id;
    autominer_vault.sol_per_round = sol_per_round;
    autominer_vault.rounds_remaining = num_rounds;
    autominer_vault.vault_bump = ctx.bumps.autominer_vault;
    
    // Transfer SOL to vault
    let total_sol = sol_per_round
        .checked_mul(num_rounds as u64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let total_transfer = total_sol
        .checked_add(rent)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? += total_transfer;
    **ctx.accounts.user_wallet.to_account_info().try_borrow_mut_lamports()? -= total_transfer;
    
    msg!(
        "Autominer initialized: {} SOL per round for {} rounds",
        sol_per_round,
        num_rounds
    );
    
    Ok(())
}

/// Execute autominer bet (keeper instruction)
pub fn execute_autominer_bet(ctx: Context<ExecuteAutominerBet>) -> Result<()> {
    require!(
        ctx.accounts.autominer_vault.rounds_remaining > 0,
        ErrorCode::NoRoundsRemaining
    );
    
    // Check vault has enough SOL
    let vault_lamports = ctx.accounts.autominer_vault.to_account_info().lamports();
    let rent = Rent::get()?.minimum_balance(AutominerVault::LEN);
    let available_sol = vault_lamports
        .checked_sub(rent)
        .ok_or(ErrorCode::InsufficientFunds)?;
    
    let sol_per_round = ctx.accounts.autominer_vault.sol_per_round;
    require!(
        available_sol >= sol_per_round,
        ErrorCode::InsufficientFunds
    );
    
    // Execute bet logic (similar to join_surge but from vault)
    // This is simplified - full implementation would call join_surge logic
    
    let rounds_remaining = ctx.accounts.autominer_vault.rounds_remaining;
    ctx.accounts.autominer_vault.rounds_remaining = rounds_remaining
        .checked_sub(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // If no rounds remaining, close vault and return remaining SOL
    if ctx.accounts.autominer_vault.rounds_remaining == 0 {
        let remaining_sol = vault_lamports
            .checked_sub(rent)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        **ctx.accounts.owner.to_account_info().try_borrow_mut_lamports()? += remaining_sol;
        **ctx.accounts.autominer_vault.to_account_info().try_borrow_mut_lamports()? -= remaining_sol;
    }
    
    msg!("Autominer bet executed. Rounds remaining: {}", ctx.accounts.autominer_vault.rounds_remaining);
    
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

// ========================================================================================
// =============================== ACCOUNT CONTEXTS ======================================
// ========================================================================================

#[derive(Accounts)]
#[instruction(faction_id: u8)]
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
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
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
pub struct JoinSurge<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_SURGE_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        seeds = [PLAYER_DATA_SEED.as_ref(), authority.key().as_ref()],
        bump = player_data.bump
    )]
    pub player_data: Account<'info, PlayerData>,
    
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[player_data.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,
    
    #[account(
        init_if_needed,
        payer = authority,
        space = UserSurgeBet::LEN,
        seeds = [USER_SURGE_BET_SEED.as_ref(), authority.key().as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_surge_bet: Account<'info, UserSurgeBet>,
    
    /// CHECK: SOL prize pot vault (PDA)
    #[account(
        mut,
        seeds = [SOL_PRIZE_POT_VAULT_SEED.as_ref()],
        bump
    )]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: Buybacks SOL vault
    #[account(
        mut,
        seeds = [BUYBACKS_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub buybacks_sol_vault: UncheckedAccount<'info>,
    
    /// CHECK: Staker SOL reward vault
    #[account(
        mut,
        seeds = [STAKER_SOL_REWARD_VAULT_SEED.as_ref()],
        bump
    )]
    pub staker_sol_reward_vault: UncheckedAccount<'info>,
    
    /// CHECK: Admin treasury
    #[account(mut)]
    pub admin_treasury: UncheckedAccount<'info>,
    
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
        seeds = [GLOBAL_SURGE_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
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
        seeds = [GLOBAL_SURGE_STATE_SEED.as_ref()],
        bump = global_game_state.bump
    )]
    pub global_game_state: Account<'info, GlobalGameSate>,
    
    #[account(
        mut,
        close = signer,
        seeds = [USER_SURGE_BET_SEED.as_ref(), signer.key().as_ref(), &global_game_state.last_round_id.to_le_bytes()],
        bump = user_surge_bet.bump
    )]
    pub user_surge_bet: Account<'info, UserSurgeBet>,
    
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
        seeds = [GLOBAL_SURGE_STATE_SEED.as_ref()],
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
        space = UserSurgeBet::LEN,
        seeds = [USER_SURGE_BET_SEED.as_ref(), autominer_vault.owner.as_ref(), &global_game_state.current_round_id.to_le_bytes()],
        bump
    )]
    pub user_surge_bet: Account<'info, UserSurgeBet>,
    
    /// CHECK: All other accounts needed for join_surge
    #[account(mut)]
    pub sol_prize_pot_vault: UncheckedAccount<'info>,
    
    /// CHECK: Buybacks SOL vault PDA
    #[account(mut)]
    pub buybacks_sol_vault: UncheckedAccount<'info>,
    
    /// CHECK: Staker SOL reward vault PDA
    #[account(mut)]
    pub staker_sol_reward_vault: UncheckedAccount<'info>,
    
    /// CHECK: Admin treasury account
    #[account(mut)]
    pub admin_treasury: UncheckedAccount<'info>,
    
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

