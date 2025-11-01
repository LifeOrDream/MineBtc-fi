use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ErrorCode;

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
pub fn add_moondoge_position(electricity_ac: &mut UserMoonElectricity, position_index: u8) -> Result<()> {
    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidPositionIndex.into());
    }
    
    // If this position index is not already active
    if !electricity_ac.moondoge_position_indices.contains(&position_index) {
        // Ensure we're not exceeding the max allowed positions
        if electricity_ac.active_moondoge_positions >= MAX_ALLOWED_POSITIONS {
            return Err(ErrorCode::MaxPositionsReached.into());
        }
        
        // Add position index to the vector
        electricity_ac.moondoge_position_indices.push(position_index);
        
        // Increment active positions counter
        electricity_ac.active_moondoge_positions += 1;
    }
    
    Ok(())
}

/// Remove position index from user's moondoge positions
pub fn remove_moondoge_position(electricity_ac: &mut UserMoonElectricity, position_index: u8) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = electricity_ac.moondoge_position_indices.iter().position(|&x| x == position_index) {
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
pub fn remove_lp_position(electricity_ac: &mut UserMoonElectricity, position_index: u8) -> Result<()> {
    // Find the position index in the vector
    if let Some(pos) = electricity_ac.lp_position_indices.iter().position(|&x| x == position_index) {
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

/// Helper function to update user electricity via CPI to MoonBase program
pub fn update_user_electricity_cpi<'info>(
    moonbase_program: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    user_moonbase: &AccountInfo<'info>,
    mining_state: &AccountInfo<'info>,
    global_config: &AccountInfo<'info>,
    fee_collector: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    fee_collector_bump: u8,
    to_increase: bool,
    amount: u64,
) -> Result<()> {
    // Fee collector seeds for CPI signer
    let fee_collector_seeds = &[
        crate::state::FEE_COLLECTOR_SEED.as_ref(),
        &[fee_collector_bump],
    ];
    let signer_seeds = &[&fee_collector_seeds[..]];
    
    let cpi_accounts = moonbase::cpi::accounts::UpdateUserElectricity {
        user: authority.clone(),
        user_moonbase: user_moonbase.clone(),
        mining_state: mining_state.clone(),
        global_config: global_config.clone(),
        authority: fee_collector.clone(),
        system_program: system_program.clone(),
    };
    
    let cpi_ctx = CpiContext::new_with_signer(moonbase_program.clone(), cpi_accounts, signer_seeds);
    moonbase::cpi::update_user_electricity(cpi_ctx, to_increase, amount)?;
    
    Ok(())
}