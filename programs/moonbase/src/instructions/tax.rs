use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount as TokenAccount2022};
use anchor_spl::token::{self, burn, Burn};
use anchor_lang::solana_program::{
    instruction::AccountMeta,
    program::invoke_signed,
};

use crate::errors::ErrorCode;
use crate::state::*;
use crate::instructions::helper;

// ========================================================================================
// ============================= TAX WITHDRAWAL =========================================
// ========================================================================================

/// Withdraw withheld tax from a token account and distribute it according to TaxConfig
/// Callable by anyone - program-controlled withdraw authority
pub fn withdraw_withheld_tax(ctx: Context<WithdrawWithheldTax>) -> Result<()> {
    msg!("💰 [withdraw_withheld_tax] Withdrawing withheld tax from token account");
    
    let tax_config = &ctx.accounts.tax_config;
    
    msg!("   Withdrawing all withheld tokens from account");
    
    // Note: We'll calculate the withheld amount after withdrawal by comparing balances
    // Distribution amounts will be calculated after we know the actual withdrawn amount
    
    // Withdraw all withheld tokens to withdraw authority's token account
    let withdraw_authority_bump = ctx.bumps.withdraw_withheld_authority;
    let withdraw_authority_seeds = &[
        WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
        &[withdraw_authority_bump],
    ];
    let withdraw_authority_signer = &[&withdraw_authority_seeds[..]];
    
    // Get balance before withdrawal to calculate withheld amount
    let balance_before = ctx.accounts.withdraw_authority_token_account.amount;
    
    // Build instruction manually for Token-2022 withdraw_withheld_tokens_from_accounts
    // Instruction discriminator: 20 (WithdrawWithheldTokensFromAccounts)
    let instruction_data = vec![20u8]; // Instruction discriminator
    
    // Build accounts vector
    let accounts = vec![
        AccountMeta::new(ctx.accounts.withdraw_authority_token_account.key(), false),
        AccountMeta::new_readonly(ctx.accounts.withdraw_withheld_authority.key(), true),
        AccountMeta::new_readonly(ctx.accounts.token_program_2022.key(), false),
        AccountMeta::new(ctx.accounts.token_account.key(), false),
    ];
    
    let instruction = anchor_lang::solana_program::instruction::Instruction {
        program_id: ctx.accounts.token_program_2022.key(),
        accounts,
        data: instruction_data,
    };
    
    invoke_signed(
        &instruction,
        &[
            ctx.accounts.withdraw_authority_token_account.to_account_info(),
            ctx.accounts.withdraw_withheld_authority.to_account_info(),
            ctx.accounts.token_program_2022.to_account_info(),
            ctx.accounts.token_account.to_account_info(),
        ],
        withdraw_authority_signer,
    )?;
    
    // Get balance after withdrawal to calculate actual withdrawn amount
    ctx.accounts.withdraw_authority_token_account.reload()?;
    let balance_after = ctx.accounts.withdraw_authority_token_account.amount;
    let withheld_amount = balance_after
        .checked_sub(balance_before)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    if (withheld_amount == 0) {
        msg!("   ❌ No withheld tokens to withdraw");
        return Ok(());
    }
    
    msg!("   ✅ Withdrawn {} tokens to authority account", (withheld_amount as f64) / 1e6);
    
    // Now calculate distribution amounts based on actual withdrawn amount
    let nft_floor_sweep_amount = helper::mul_div(withheld_amount, tax_config.nft_floor_sweep_pct as u64, 100)? as u64;    
    let faction_treasury_amount = helper::mul_div(withheld_amount, tax_config.faction_treasury_pct as u64, 100)? as u64;    
    let burn_amount = withheld_amount - nft_floor_sweep_amount - faction_treasury_amount;
    
    msg!("   Distribution: NFT Floor Sweep: {} tokens ({}%), Burn: {} tokens ({}%), Faction Treasury: {} tokens ({}%)", (nft_floor_sweep_amount as f64) / 1e6, tax_config.nft_floor_sweep_pct, (burn_amount as f64) / 1e6, (M_HUNDRED - tax_config.nft_floor_sweep_pct - tax_config.faction_treasury_pct), (faction_treasury_amount as f64) / 1e6, tax_config.faction_treasury_pct);
    // Transfer NFT floor sweep portion
    if nft_floor_sweep_amount > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program_2022.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.withdraw_authority_token_account.to_account_info(),
                    to: ctx.accounts.nft_floor_sweep_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
            ),
            nft_floor_sweep_amount,
        )?;
        msg!("   ✅ Transferred {} tokens to NFT floor sweep vault", nft_floor_sweep_amount);
    }
    
    // Transfer faction treasury portion
    if faction_treasury_amount > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program_2022.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.withdraw_authority_token_account.to_account_info(),
                    to: ctx.accounts.faction_treasury_vault.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
            ),
            faction_treasury_amount,
        )?;
        msg!("   ✅ Transferred {} tokens to faction treasury vault", faction_treasury_amount);
    }
    
    // Burn the burn portion
    if burn_amount > 0 {
        let burn_seeds = &[
            WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref(),
            &[withdraw_authority_bump],
        ];
        let burn_signer = &[&burn_seeds[..]];
        
        burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                Burn {
                    mint: ctx.accounts.dbtc_mint.to_account_info(),
                    from: ctx.accounts.withdraw_authority_token_account.to_account_info(),
                    authority: ctx.accounts.withdraw_withheld_authority.to_account_info(),
                },
                burn_signer,
            ),
            burn_amount,
        )?;
        
        // Update total burnt counter
        let tax_config = &mut ctx.accounts.tax_config;
        tax_config.total_burnt = tax_config.total_burnt
            .checked_add(burn_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("   ✅ Burnt {} tokens (Total burnt: {})", burn_amount, tax_config.total_burnt);
    }
    
    msg!("✅ [withdraw_withheld_tax] Tax withdrawal and distribution complete");
    Ok(())
}

// ========================================================================================
// ============================= DISTRIBUTION ROUND MANAGEMENT ===========================
// ========================================================================================

/// Start a new distribution round (callable by anyone after 7-day cooldown)
pub fn start_distribution_round(ctx: Context<StartDistributionRound>) -> Result<()> {
    msg!("🎯 [start_distribution_round] Starting new distribution round");
    
    let tax_config = &mut ctx.accounts.tax_config;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;
    
    // Check if 7 days have passed since last distribution round ended
    require!(
        !tax_config.round_active,
        ErrorCode::InvalidState
    );
    
    require!(
        current_time >= tax_config.end_timestamp + TaxConfig::DISTRIBUTION_COOLDOWN_SECONDS,
        ErrorCode::InvalidState
    );
    
    // Check faction treasury has funds
    let faction_treasury_balance = ctx.accounts.faction_treasury_vault.amount;
    require!(faction_treasury_balance > 0, ErrorCode::InvalidAmount);
    
    msg!("   Faction treasury balance: {} tokens", faction_treasury_balance);
    
    // Reset distribution round state
    tax_config.round_active = true;
    tax_config.start_timestamp = current_time;
    tax_config.leaderboard_factions_count = 0;
    tax_config.rewards_calculated = false;
    tax_config.factions_claimed_count = 0;
    
    // Clear leaderboard and rewards
    tax_config.leaderboard_faction_ids.clear();
    tax_config.leaderboard_hashpower.clear();
    tax_config.faction_rewards.clear();
    tax_config.faction_claimed.clear();
    
    // Initialize vectors with capacity
    tax_config.leaderboard_faction_ids.resize(MAX_FACTIONS, 0);
    tax_config.leaderboard_hashpower.resize(MAX_FACTIONS, 0);
    tax_config.faction_rewards.resize(MAX_FACTIONS, 0);
    tax_config.faction_claimed.resize(MAX_FACTIONS, false);
    
    msg!("✅ [start_distribution_round] Distribution round started");
    msg!("   Round start timestamp: {}", current_time);
    
    Ok(())
}

/// Calculate and store leaderboard position for one faction
/// Must be called 12 times (once per faction) to build complete leaderboard
pub fn calculate_faction_leaderboard_position(ctx: Context<CalculateFactionLeaderboard>) -> Result<()> {
    msg!("📊 [calculate_faction_leaderboard_position] Calculating leaderboard position");
    
    let tax_config = &mut ctx.accounts.tax_config;
    let faction_state = &ctx.accounts.faction_state;
    
    require!(
        tax_config.round_active,
        ErrorCode::InvalidState
    );
    
    require!(
        tax_config.leaderboard_factions_count < MAX_FACTIONS as u8,
        ErrorCode::InvalidState
    );
    
    // Calculate total hashpower for this faction (dbtc + lp)
    let total_hashpower = faction_state.total_dbtc_hashpower
        .checked_add(faction_state.total_lp_hashpower)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    msg!("   Faction ID: {}", faction_state.faction_id);
    msg!("   Total hashpower: {} (dbtc: {}, lp: {})", 
        total_hashpower, 
        faction_state.total_dbtc_hashpower,
        faction_state.total_lp_hashpower);
    
    // Find insertion position (maintain descending order by hashpower)
    let mut insert_index = tax_config.leaderboard_factions_count as usize;
    for i in 0..tax_config.leaderboard_factions_count as usize {
        if total_hashpower > tax_config.leaderboard_hashpower[i] {
            insert_index = i;
            break;
        }
    }
    
    // Shift existing entries down
    for i in (insert_index..tax_config.leaderboard_factions_count as usize).rev() {
        if i + 1 < MAX_FACTIONS {
            tax_config.leaderboard_faction_ids[i + 1] = tax_config.leaderboard_faction_ids[i];
            tax_config.leaderboard_hashpower[i + 1] = tax_config.leaderboard_hashpower[i];
        }
    }
    
    // Insert new entry
    tax_config.leaderboard_faction_ids[insert_index] = faction_state.faction_id;
    tax_config.leaderboard_hashpower[insert_index] = total_hashpower;
    tax_config.leaderboard_factions_count += 1;
    
    msg!("   Rank: {} (0 = highest)", insert_index);
    msg!("   Leaderboard count: {}/12", tax_config.leaderboard_factions_count);
    
    Ok(())
}

/// Calculate rewards for all factions based on leaderboard
/// Can only be called after all 12 factions are on leaderboard
pub fn calculate_faction_rewards(ctx: Context<CalculateFactionRewards>) -> Result<()> {
    msg!("💰 [calculate_faction_rewards] Calculating faction rewards");
    
    let tax_config = &mut ctx.accounts.tax_config;
    
    require!(
        tax_config.round_active,
        ErrorCode::InvalidState
    );
    
    require!(
        tax_config.leaderboard_factions_count == MAX_FACTIONS as u8,
        ErrorCode::InvalidState
    );
    
    require!(
        !tax_config.rewards_calculated,
        ErrorCode::InvalidState
    );
    
    // Get total faction treasury balance
    let total_treasury = ctx.accounts.faction_treasury_vault.amount;
    require!(total_treasury > 0, ErrorCode::InvalidAmount);
    
    msg!("   Total treasury: {} tokens", total_treasury);
    
    // Calculate rewards based on rank:
    // Rank 0 (1st): 25%
    // Rank 1 (2nd): 15%
    // Rank 2 (3rd): 10%
    // Ranks 3-11: Randomly select one to get remaining 50%
    
    let first_place_reward = (total_treasury as u128)
        .checked_mul(25)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
    
    let second_place_reward = (total_treasury as u128)
        .checked_mul(15)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
    
    let third_place_reward = (total_treasury as u128)
        .checked_mul(10)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(ErrorCode::ArithmeticOverflow)? as u64;
    
    let remaining_amount = total_treasury
        .checked_sub(first_place_reward)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(second_place_reward)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_sub(third_place_reward)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Randomly select one faction from ranks 3-11 to get remaining 50%
    // Use current timestamp as seed for pseudo-random selection
    let clock = Clock::get()?;
    let random_seed = clock.unix_timestamp as u64;
    let random_index = 3 + (random_seed % 9) as usize; // Random index between 3-11
    
    msg!("   Reward distribution:");
    msg!("     Rank 0 ({}): {} tokens (25%)", 
        tax_config.leaderboard_faction_ids[0], first_place_reward);
    msg!("     Rank 1 ({}): {} tokens (15%)", 
        tax_config.leaderboard_faction_ids[1], second_place_reward);
    msg!("     Rank 2 ({}): {} tokens (10%)", 
        tax_config.leaderboard_faction_ids[2], third_place_reward);
    msg!("     Rank {} ({}): {} tokens (50% - randomly selected)", 
        random_index, 
        tax_config.leaderboard_faction_ids[random_index], 
        remaining_amount);
    
    // Set rewards
    tax_config.faction_rewards[0] = first_place_reward;
    tax_config.faction_rewards[1] = second_place_reward;
    tax_config.faction_rewards[2] = third_place_reward;
    tax_config.faction_rewards[random_index] = remaining_amount;
    
    // All other ranks get 0
    for i in 0..MAX_FACTIONS {
        if i != 0 && i != 1 && i != 2 && i != random_index {
            tax_config.faction_rewards[i] = 0;
        }
    }
    
    tax_config.rewards_calculated = true;
    
    msg!("✅ [calculate_faction_rewards] Rewards calculated");
    
    Ok(())
}

/// Claim treasury rewards for one faction
/// Adds rewards to staking reward indexes (50% each to dbtc and lp stakers)
pub fn claim_faction_treasury_rewards(ctx: Context<ClaimFactionTreasuryRewards>) -> Result<()> {
    msg!("🎁 [claim_faction_treasury_rewards] Claiming treasury rewards");
    
    let tax_config = &mut ctx.accounts.tax_config;
    let faction_state = &mut ctx.accounts.faction_state;
    
    require!(
        tax_config.round_active,
        ErrorCode::InvalidState
    );
    
    require!(
        tax_config.rewards_calculated,
        ErrorCode::InvalidState
    );
    
    // Find faction's rank in leaderboard
    let mut faction_rank: Option<usize> = None;
    for i in 0..tax_config.leaderboard_factions_count as usize {
        if tax_config.leaderboard_faction_ids[i] == faction_state.faction_id {
            faction_rank = Some(i);
            break;
        }
    }
    
    let rank = faction_rank.ok_or(ErrorCode::InvalidFactionId)?;
    let reward_amount = tax_config.faction_rewards[rank];
    
    require!(reward_amount > 0, ErrorCode::InvalidAmount);
    require!(
        !tax_config.faction_claimed[faction_state.faction_id as usize],
        ErrorCode::InvalidState
    );
    
    msg!("   Faction ID: {}", faction_state.faction_id);
    msg!("   Rank: {}", rank);
    msg!("   Reward amount: {} tokens", reward_amount);
    
    // Split reward 50/50 between dbtc and lp stakers
    let dbtc_reward = reward_amount / 2;
    let lp_reward = reward_amount - dbtc_reward; // Handle odd amounts
    
    msg!("   Split: {} to dbtc stakers, {} to lp stakers", dbtc_reward, lp_reward);
    
    // Transfer tokens from treasury vault to emission vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            token::Transfer {
                from: ctx.accounts.faction_treasury_vault.to_account_info(),
                to: ctx.accounts.dbtc_emission_vault.to_account_info(),
                authority: ctx.accounts.dbtc_emission_vault_authority.to_account_info(),
            },
        ),
        reward_amount,
    )?;
    
    // Update reward indexes for dbtc stakers
    if dbtc_reward > 0 && faction_state.total_dbtc_hashpower > 0 {
        let index_increase = (dbtc_reward as u128)
            .checked_mul(INDEX_PRECISION as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(faction_state.total_dbtc_hashpower as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        faction_state.dbtc_dbtc_reward_index = faction_state.dbtc_dbtc_reward_index
            .checked_add(index_increase)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    // Update reward indexes for lp stakers
    if lp_reward > 0 && faction_state.total_lp_hashpower > 0 {
        let index_increase = (lp_reward as u128)
            .checked_mul(INDEX_PRECISION as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(faction_state.total_lp_hashpower as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        faction_state.lp_dbtc_reward_index = faction_state.lp_dbtc_reward_index
            .checked_add(index_increase)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    
    // Mark faction as claimed
    tax_config.faction_claimed[faction_state.faction_id as usize] = true;
    tax_config.factions_claimed_count += 1;
    
    // Clear reward for this faction
    tax_config.faction_rewards[rank] = 0;
    
    msg!("✅ [claim_faction_treasury_rewards] Rewards claimed and distributed");
    msg!("   Updated dbtc_dbtc_reward_index: {}", faction_state.dbtc_dbtc_reward_index);
    msg!("   Updated lp_dbtc_reward_index: {}", faction_state.lp_dbtc_reward_index);
    
    Ok(())
}

/// Finish distribution round (check all factions claimed and reset state)
pub fn finish_distribution_round(ctx: Context<FinishDistributionRound>) -> Result<()> {
    msg!("🏁 [finish_distribution_round] Finishing distribution round");
    
    let tax_config = &mut ctx.accounts.tax_config;
    let clock = Clock::get()?;
    
    require!(
        tax_config.round_active,
        ErrorCode::InvalidState
    );
    
    require!(
        tax_config.rewards_calculated,
        ErrorCode::InvalidState
    );
    
    // Check all factions with rewards have claimed
    let mut all_claimed = true;
    for i in 0..MAX_FACTIONS {
        if tax_config.faction_rewards[i] > 0 {
            let faction_id = tax_config.leaderboard_faction_ids[i];
            if !tax_config.faction_claimed[faction_id as usize] {
                all_claimed = false;
                msg!("   ⚠️ Faction {} (rank {}) has not claimed rewards", faction_id, i);
            }
        }
    }
    
    require!(all_claimed, ErrorCode::InvalidState);
    
    // Reset distribution round state
    tax_config.round_active = false;
    tax_config.end_timestamp = clock.unix_timestamp;
    tax_config.rewards_calculated = false;
    tax_config.leaderboard_factions_count = 0;
    tax_config.factions_claimed_count = 0;
    
    // Clear leaderboard and rewards
    tax_config.leaderboard_faction_ids.clear();
    tax_config.leaderboard_hashpower.clear();
    tax_config.faction_rewards.clear();
    tax_config.faction_claimed.clear();
    
    msg!("✅ [finish_distribution_round] Distribution round finished");
    msg!("   Next round can start after: {} seconds", TaxConfig::DISTRIBUTION_COOLDOWN_SECONDS);
    
    Ok(())
}

// ========================================================================================
// ============================= ACCOUNT CONTEXTS =========================================
// ========================================================================================

#[derive(Accounts)]
pub struct WithdrawWithheldTax<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
    
    /// CHECK: Token account with withheld tax
    #[account(mut)]
    pub token_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [WITHDRAW_WITHHELD_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: Program-controlled withdraw withheld authority PDA
    pub withdraw_withheld_authority: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Token account owned by withdraw authority (receives withdrawn tokens)
    pub withdraw_authority_token_account: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(mut)]
    /// CHECK: NFT floor sweep vault token account
    pub nft_floor_sweep_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(mut)]
    /// CHECK: Faction treasury vault token account
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(mut)]
    /// CHECK: DogeBtc mint (for burning)
    pub dbtc_mint: InterfaceAccount<'info, Mint>,
    
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct StartDistributionRound<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
    
    #[account(mut)]
    /// CHECK: Faction treasury vault (checked for balance)
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
}

#[derive(Accounts)]
pub struct CalculateFactionLeaderboard<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
    
    #[account(
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_state.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,
}

#[derive(Accounts)]
pub struct CalculateFactionRewards<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
    
    #[account(mut)]
    /// CHECK: Faction treasury vault (checked for balance)
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
}

#[derive(Accounts)]
pub struct ClaimFactionTreasuryRewards<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
    
    #[account(
        mut,
        seeds = [FACTION_STATE_SEED.as_ref(), &[faction_state.faction_id]],
        bump = faction_state.bump
    )]
    pub faction_state: Account<'info, FactionState>,
    
    #[account(mut)]
    /// CHECK: Faction treasury vault
    pub faction_treasury_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(mut)]
    /// CHECK: DogeBtc emission vault (receives transferred tokens)
    pub dbtc_emission_vault: InterfaceAccount<'info, TokenAccount2022>,
    
    #[account(
        seeds = [DBTC_EMISSION_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: Emission vault authority PDA
    pub dbtc_emission_vault_authority: UncheckedAccount<'info>,
    
    pub token_program_2022: Program<'info, anchor_spl::token_2022::Token2022>,
}

#[derive(Accounts)]
pub struct FinishDistributionRound<'info> {
    #[account(
        mut,
        seeds = [TAX_CONFIG_SEED.as_ref()],
        bump = tax_config.bump
    )]
    pub tax_config: Account<'info, TaxConfig>,
}

