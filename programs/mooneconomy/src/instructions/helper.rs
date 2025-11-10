use crate::errors::ErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

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
pub fn add_moondoge_position(
    electricity_ac: &mut UserMoonElectricity,
    position_index: u8,
) -> Result<()> {
    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidPositionIndex.into());
    }

    // If this position index is not already active
    if !electricity_ac
        .moondoge_position_indices
        .contains(&position_index)
    {
        // Ensure we're not exceeding the max allowed positions
        if electricity_ac.active_moondoge_positions >= MAX_ALLOWED_POSITIONS {
            return Err(ErrorCode::MaxPositionsReached.into());
        }

        // Add position index to the vector
        electricity_ac
            .moondoge_position_indices
            .push(position_index);

        // Increment active positions counter
        electricity_ac.active_moondoge_positions += 1;
    }

    Ok(())
}

/// Remove position index from user's moondoge positions
pub fn remove_moondoge_position(
    electricity_ac: &mut UserMoonElectricity,
    position_index: u8,
) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = electricity_ac
        .moondoge_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        // Remove the position index
        electricity_ac.moondoge_position_indices.remove(pos);

        // Decrement active positions counter
        if electricity_ac.active_moondoge_positions > 0 {
            electricity_ac.active_moondoge_positions -= 1;
        }
    } else {
        return Err(ErrorCode::PositionNotFound.into());
    }

    Ok(())
}

/// Add position index to user's LP positions
pub fn add_lp_position(electricity_ac: &mut UserMoonElectricity, position_index: u8) -> Result<()> {
    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidPositionIndex.into());
    }

    // If this position index is not already active
    if !electricity_ac.lp_position_indices.contains(&position_index) {
        // Ensure we're not exceeding the max allowed positions
        if electricity_ac.active_lp_positions >= MAX_ALLOWED_POSITIONS {
            return Err(ErrorCode::MaxPositionsReached.into());
        }

        // Add position index to the vector
        electricity_ac.lp_position_indices.push(position_index);

        // Increment active positions counter
        electricity_ac.active_lp_positions += 1;
    }

    Ok(())
}

/// Remove position index from user's LP positions
pub fn remove_lp_position(
    electricity_ac: &mut UserMoonElectricity,
    position_index: u8,
) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = electricity_ac
        .lp_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        // Remove the position index
        electricity_ac.lp_position_indices.remove(pos);

        // Decrement active positions counter
        if electricity_ac.active_lp_positions > 0 {
            electricity_ac.active_lp_positions -= 1;
        }
    } else {
        return Err(ErrorCode::PositionNotFound.into());
    }

    Ok(())
}

/// Helper function to update personal hashpower via CPI to MoonBase program
pub fn update_personal_hashpower_cpi<'info>(
    moonbase_program: &AccountInfo<'info>,
    player_data: &AccountInfo<'info>,
    faction_state: &AccountInfo<'info>,
    mooneconomy_program: &AccountInfo<'info>,
    amount: i128,
    user_pubkey: Pubkey,
) -> Result<()> {
    let cpi_accounts = moonbase::cpi::accounts::UpdatePersonalHashpower {
        player_data: player_data.clone(),
        faction_state: faction_state.clone(),
        mooneconomy_program: mooneconomy_program.clone(),
    };

    let cpi_ctx = CpiContext::new(moonbase_program.clone(), cpi_accounts);
    moonbase::cpi::update_personal_hashpower(cpi_ctx, amount, user_pubkey)?;

    Ok(())
}

/// Calculate total electricity for a user (sum of all staking positions)
pub fn calculate_total_electricity(
    electricity_ac: &UserMoonElectricity,
    _global_config: &GlobalConfig,
    _dogebtc_vault: &DogeBtcVault,
    _liquidity_vault: &LiquidityVault,
) -> Result<u64> {
    // electricity_earned already includes electricity from all DOGE_BTC and LP positions
    // It's updated whenever user stakes/unstakes
    Ok(electricity_ac.electricity_earned)
}
