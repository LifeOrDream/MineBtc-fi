use crate::errors::ErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token::{self as token_standard, Burn as StandardBurn};
use anchor_spl::token_2022::{self, Burn};

// WSOL mint address (Wrapped SOL)
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

// -----------------------------------------------------
// ------------ REFERRAL SYSTEM HELPERS ----------------
// -----------------------------------------------------

// Helper function to transfer SOL to the program's sol_treasury PDA
pub fn transfer_to_sol_treasury<'info>(
    from: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_treasury.to_account_info(),
            },
        ),
        amount,
    )
}

// Helper function to transfer SOL to the program's doges_treasury PDA
pub fn transfer_to_doges_treasury<'info>(
    from: &AccountInfo<'info>,
    doges_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: doges_treasury.to_account_info(),
            },
        ),
        amount,
    )
}

pub fn transfer_to_autominer_custody<'info>(
    from: &AccountInfo<'info>,
    autominer_custody: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: autominer_custody.to_account_info(),
            },
        ),
        amount,
    )
}

pub fn transfer_from_autominer_custody<'info>(
    autominer_custody: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    custody_bump: u8,
) -> Result<()> {
    let seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];
    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: autominer_custody.to_account_info(),
                to: to.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )
}

// Helper function to transfer SOL to the sol_rewards_vault PDA
pub fn transfer_to_sol_rewards_vault<'info>(
    from: &AccountInfo<'info>,
    sol_rewards_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_rewards_vault.to_account_info(),
            },
        ),
        amount,
    )
}

// Helper function to transfer SOL to the sol_prize_pot_vault PDA
pub fn transfer_to_sol_prize_pot_vault<'info>(
    from: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_prize_pot_vault.to_account_info(),
            },
        ),
        amount,
    )
}

// Helper function to transfer SOL FROM sol_rewards_vault to a user
// Uses PDA signer to authorize the transfer from the System Program-owned account
pub fn transfer_from_sol_rewards_vault<'info>(
    sol_rewards_vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    vault_bump: u8,
) -> Result<()> {
    let seeds = &[STAKER_SOL_REWARD_VAULT_SEED.as_ref(), &[vault_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: sol_rewards_vault.to_account_info(),
                to: to.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )
}

// Helper function to transfer SOL FROM sol_prize_pot_vault to a user
// Uses PDA signer to authorize the transfer from the System Program-owned account
pub fn transfer_from_sol_prize_pot_vault<'info>(
    sol_prize_pot_vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    vault_bump: u8,
) -> Result<()> {
    let seeds = &[SOL_PRIZE_POT_VAULT_SEED.as_ref(), &[vault_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: sol_prize_pot_vault.to_account_info(),
                to: to.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )
}

// Helper function to transfer WSOL to multisig token account
// Wraps SOL to WSOL first, then transfers WSOL
pub fn transfer_wsol_to_multisig<'info>(
    from: &AccountInfo<'info>,
    multisig_wsol_account: &AccountInfo<'info>,
    from_wsol_account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    // Step 1: Transfer SOL to the from_wsol_account (this wraps it)
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: from_wsol_account.to_account_info(),
            },
        ),
        amount,
    )?;

    // Step 2: Sync native account to update WSOL balance
    anchor_spl::token::sync_native(CpiContext::new(
        token_program.to_account_info(),
        anchor_spl::token::SyncNative {
            account: from_wsol_account.to_account_info(),
        },
    ))?;

    // Step 3: Transfer WSOL from from_wsol_account to multisig_wsol_account
    anchor_spl::token::transfer(
        CpiContext::new(
            token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: from_wsol_account.to_account_info(),
                to: multisig_wsol_account.to_account_info(),
                authority: from.to_account_info(),
            },
        ),
        amount,
    )?;

    Ok(())
}

// Helper function to calculate multiplier
pub fn calculate_multiplier(
    lockup_duration: u64,
    min_lockup: u64,
    max_lockup: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<u16> {
    let duration_range = max_lockup.saturating_sub(min_lockup);
    let multiplier_range = max_multiplier.saturating_sub(base_multiplier);

    let duration_above_min = lockup_duration.saturating_sub(min_lockup);

    let multiplier_increase = (duration_above_min as u128)
        .checked_mul(multiplier_range as u128)
        .unwrap()
        .checked_div(duration_range as u128)
        .unwrap() as u16;

    Ok(base_multiplier.checked_add(multiplier_increase).unwrap())
}

/// Add position index to user's minebtc positions
pub fn add_dogebtc_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    msg!(
        "🔍 [add_dogebtc_position] Adding position index: {}",
        position_index
    );
    msg!(
        "🔍 [add_dogebtc_position] MAX_ALLOWED_POSITIONS: {}",
        MAX_ALLOWED_POSITIONS
    );

    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // If this position index is not already active
    if !player_ac.dogebtc_position_indices.contains(&position_index) {
        msg!("🔍 [add_dogebtc_position] Position index is not already active");
        // Ensure we're not exceeding the max allowed positions
        if player_ac.dogebtc_position_indices.len() >= MAX_ALLOWED_POSITIONS as usize {
            msg!("🔍 [add_dogebtc_position] Exceeding max allowed positions");
            return Err(ErrorCode::InvalidParameters.into());
        }
        player_ac.dogebtc_position_indices.push(position_index);
        msg!(
            "🔍 [add_dogebtc_position] Position index added: {}",
            position_index
        );
    }

    Ok(())
}

/// Remove position index from user's minebtc positions
pub fn remove_dogebtc_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = player_ac
        .dogebtc_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        player_ac.dogebtc_position_indices.remove(pos);
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(())
}

/// Add position index to user's LP positions
pub fn add_lp_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // If this position index is not already active
    if !player_ac.lp_position_indices.contains(&position_index) {
        if player_ac.lp_position_indices.len() >= MAX_ALLOWED_POSITIONS as usize {
            return Err(ErrorCode::InvalidParameters.into());
        }
        player_ac.lp_position_indices.push(position_index);
    }

    Ok(())
}

/// Remove position index from user's LP positions
pub fn remove_lp_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    if let Some(pos) = player_ac
        .lp_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        player_ac.lp_position_indices.remove(pos);
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(())
}

pub fn calculate_staking_rewards(
    user_weighted_amt: u64,
    accumulated_sol_per_point: u128,
    reward_debt: u128,
) -> Result<u64> {
    let reward_diff = accumulated_sol_per_point
        .checked_sub(reward_debt)
        .unwrap_or(0);
    let new_rewards = mul_div_u128(
        user_weighted_amt as u128,
        reward_diff,
        INDEX_PRECISION as u128,
    )?;
    Ok(new_rewards as u64)
}

pub fn mul_div(a: u64, b: u64, c: u64) -> Result<u128> {
    let result = (a as u128)
        .checked_mul(b as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(c as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok(result)
}

pub fn mul_div_u128(a: u128, b: u128, c: u128) -> Result<u128> {
    let result = a
        .checked_mul(b)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(c)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    Ok(result)
}

pub fn init_position(
    position: &mut StakedPosition,
    position_type: u8,
    faction_id: u8,
    position_index: u8,
    staked_amount: u64,
    weighted_amount: u64,
    lockup_duration: u64,
    current_ts: i64,
    multiplier: u16,
) -> Result<()> {
    position.position_index = position_index;
    position.position_type = position_type;

    position.faction_id = faction_id;
    position.staked_amount = staked_amount;
    position.weighted_amount = weighted_amount;

    position.lockup_duration = lockup_duration;
    position.start_timestamp = current_ts;
    position.multiplier = multiplier;

    let seconds_to_add = lockup_duration * DAY_IN_SECONDS;
    position.lockup_end_timestamp = current_ts + seconds_to_add as i64;

    Ok(())
}

/// Add to total claimable and pending rewards
pub fn add_to_total_claimable(
    unrefined_minebtc: &mut UnrefinedRewards,
    player_data: &mut PlayerData,
    minebtc_rewards: u64,
) -> u64 {
    // Calculate extra dogeBtc rewards due to unrefining
    let index_dif = unrefined_minebtc.unrefining_index - player_data.unrefining_index;
    let accrued_rewards = mul_div_u128(
        player_data.pending_minebtc_rewards as u128,
        index_dif,
        INDEX_PRECISION as u128,
    )
    .unwrap() as u64;
    msg!("     Accrued MineBtc rewards: {}", accrued_rewards);

    unrefined_minebtc.total_minebtc_claimable += minebtc_rewards + accrued_rewards;
    player_data.unrefining_index = unrefined_minebtc.unrefining_index;
    player_data.pending_minebtc_rewards += minebtc_rewards + accrued_rewards;
    player_data.total_dogebtc_won += minebtc_rewards + accrued_rewards;
    player_data.unrefined_minebtc_rewards += accrued_rewards;

    return accrued_rewards;
}

pub fn calculate_emergency_tax(
    user_position: &StakedPosition,
    current_ts: i64,
    emergency_tax: u64,
) -> u64 {
    let total_lockup_seconds = user_position.lockup_end_timestamp - user_position.start_timestamp;
    let remaining_seconds = user_position.lockup_end_timestamp - current_ts;

    let mut remaining_seconds_pct = 0;
    if total_lockup_seconds > 0 {
        remaining_seconds_pct = (M_HUNDRED as i64 * remaining_seconds) / total_lockup_seconds;
    }
    msg!("   Lockup remaining: {}%", remaining_seconds_pct);

    let calc_penalty_pct = (emergency_tax * (remaining_seconds_pct as u64)) / M_HUNDRED;
    let penalty_amount = (user_position.staked_amount * calc_penalty_pct) / M_HUNDRED;

    return penalty_amount;
}

/// Charge emergency tax for MINEBTC tokens: 50% burned, 50% sent to MINEBTC vault
/// This function handles the penalty for early withdrawal from MINEBTC staking positions
/// Note: The 50% sent to vault accounts for 1% burn tax (transfers 99% of that 50%)
pub fn charge_emergency_tax<'info>(
    minebtc_custodian: &AccountInfo<'info>,
    minebtc_custodian_authority: &AccountInfo<'info>,
    minebtc_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    custodian_authority_bump: u8,
    penalty_amount: u64,
) -> Result<()> {
    msg!("💰 [charge_emergency_tax] Processing MINEBTC emergency tax");
    msg!("   Penalty amount: {} tokens", penalty_amount as f64 / 1e6);

    // Burn 50% from custodian
    if penalty_amount > 0 {
        msg!(
            "   Burning {} tokens from custodian...",
            penalty_amount as f64 / 1e6
        );

        // Get PDA signer seeds for the minebtc_custodian authority (global, no faction_id)
        let custodian_authority_seeds = &[
            MINEBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[custodian_authority_bump],
        ];
        let custodian_signer = &[&custodian_authority_seeds[..]];

        // Use Token-2022 burn instruction
        token_2022::burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: minebtc_mint.to_account_info(),
                    from: minebtc_custodian.to_account_info(),
                    authority: minebtc_custodian_authority.to_account_info(),
                },
                custodian_signer,
            ),
            penalty_amount,
        )?;
        msg!("   ✅ Burned {} tokens", penalty_amount as f64 / 1e6);
    }

    msg!("   ✅ Emergency tax charged successfully");

    Ok(())
}

/// Charge emergency tax for LP tokens: 100% burned
/// This function handles the penalty for early withdrawal from LP staking positions
/// LP tokens are fully burned with no rewards to stakers
pub fn charge_lp_emergency_tax<'info>(
    liquidity_custodian: &AccountInfo<'info>,
    liquidity_custodian_authority: &AccountInfo<'info>,
    lp_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    custodian_authority_bump: u8,
    penalty_amount: u64,
) -> Result<()> {
    msg!("💰 [charge_lp_emergency_tax] Processing LP emergency tax");
    msg!("   Penalty amount: {} tokens", penalty_amount as f64 / 1e6);
    msg!("   Burning 100% of penalty (no rewards to stakers)");

    if penalty_amount > 0 {
        // Get PDA signer seeds for the liquidity_custodian authority (global)
        let custodian_authority_seeds = &[
            LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[custodian_authority_bump],
        ];
        let custodian_signer = &[&custodian_authority_seeds[..]];

        // Burn 100% of penalty from custodian (standard SPL Token burn)
        token_standard::burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                StandardBurn {
                    mint: lp_mint.to_account_info(),
                    from: liquidity_custodian.to_account_info(),
                    authority: liquidity_custodian_authority.to_account_info(),
                },
                custodian_signer,
            ),
            penalty_amount,
        )?;
        msg!("   ✅ Burned {} LP tokens", penalty_amount as f64 / 1e6);
    }

    msg!("   ✅ LP emergency tax charged successfully");

    Ok(())
}

/// Calculate number of tickets given minting price
pub fn calc_tickets_count(total_price: u64, ticket_value: u64) -> u64 {
    return ((total_price * 150) / ticket_value) / 100;
}
