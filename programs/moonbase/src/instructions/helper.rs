use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use crate::state::*;
use crate::errors::ErrorCode;

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
    anchor_spl::token::sync_native(
        CpiContext::new(
            token_program.to_account_info(),
            anchor_spl::token::SyncNative {
                account: from_wsol_account.to_account_info(),
            },
        ),
    )?;

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

/// Add position index to user's moondoge positions
pub fn add_dogebtc_position(
    player_ac: &mut PlayerData,
    position_index: u8,
) -> Result<()> {
    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // If this position index is not already active
    if !player_ac
        .moondoge_position_indices
        .contains(&position_index)
    {
        // Ensure we're not exceeding the max allowed positions
        if player_ac.active_moondoge_positions >= MAX_ALLOWED_POSITIONS {
            return Err(ErrorCode::InvalidParameters.into());
        }

        // Add position index to the vector
        player_ac
            .moondoge_position_indices
            .push(position_index);

        // Increment active positions counter
        player_ac.active_moondoge_positions += 1;
    }

    Ok(())
}

/// Remove position index from user's moondoge positions
pub fn remove_moondoge_position(
    player_ac: &mut PlayerData,
    position_index: u8,
) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = player_ac
        .moondoge_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        // Remove the position index
        player_ac.moondoge_position_indices.remove(pos);

        // Decrement active positions counter
        if player_ac.active_moondoge_positions > 0 {
            player_ac.active_moondoge_positions -= 1;
        }
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
        // Ensure we're not exceeding the max allowed positions
        if player_ac.active_lp_positions >= MAX_ALLOWED_POSITIONS {
            return Err(ErrorCode::InvalidParameters.into());
        }

        // Add position index to the vector
        player_ac.lp_position_indices.push(position_index);

        // Increment active positions counter
        player_ac.active_lp_positions += 1;
    }

    Ok(())
}

/// Remove position index from user's LP positions
pub fn remove_lp_position(
    player_ac: &mut PlayerData,
    position_index: u8,
) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = player_ac
        .lp_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        // Remove the position index
        player_ac.lp_position_indices.remove(pos);

        // Decrement active positions counter
        if player_ac.active_lp_positions > 0 {
            player_ac.active_lp_positions -= 1;
        }
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(())
}

 
pub fn calculate_staking_rewards(
    user_weighted_amt: u64,
    accumulated_sol_per_point: u64,
    reward_debt: u64,
) -> Result<u64> {
    let reward_diff = accumulated_sol_per_point.checked_sub(reward_debt).unwrap_or(0);
    let new_rewards = mul_div(user_weighted_amt, reward_diff, INDEX_PRECISION)?;
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

pub fn init_position(position: &mut StakedPosition, faction_id: u8, position_index: u8, staked_amount: u64, weighted_amount: u64, 
                    lockup_duration: u64, current_ts: i64, multiplier: u16) -> Result<()> {
    position.position_index = position_index;

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