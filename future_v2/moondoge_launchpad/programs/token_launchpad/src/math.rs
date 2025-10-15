use crate::constants::*;
use crate::errors::LaunchpadError;
use anchor_lang::prelude::*;

/// Calculate tokens to receive for a given SOL amount using constant product formula
/// Formula: tokens_out = (token_reserves * sol_in) / (sol_reserves + sol_in)
pub fn calculate_tokens_out(
    sol_amount: u64,
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
) -> Result<u64> {
    if sol_amount == 0 {
        return Ok(0);
    }

    let sol_amount_u128 = sol_amount as u128;
    let virtual_sol_reserves_u128 = virtual_sol_reserves as u128;
    let virtual_token_reserves_u128 = virtual_token_reserves as u128;

    // Calculate: (token_reserves * sol_in) / (sol_reserves + sol_in)
    let numerator = virtual_token_reserves_u128
        .checked_mul(sol_amount_u128)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    let denominator = virtual_sol_reserves_u128
        .checked_add(sol_amount_u128)
        .ok_or(LaunchpadError::MathOverflow)?;

    if denominator == 0 {
        return Err(LaunchpadError::DivisionByZero.into());
    }

    let tokens_out = numerator
        .checked_div(denominator)
        .ok_or(LaunchpadError::DivisionByZero)?;

    // Ensure result fits in u64
    if tokens_out > u64::MAX as u128 {
        return Err(LaunchpadError::MathOverflow.into());
    }

    Ok(tokens_out as u64)
}

/// Calculate SOL to receive for a given token amount using constant product formula
/// Formula: sol_out = (sol_reserves * tokens_in) / (token_reserves + tokens_in)
pub fn calculate_sol_out(
    token_amount: u64,
    virtual_sol_reserves: u64,
    virtual_token_reserves: u64,
) -> Result<u64> {
    if token_amount == 0 {
        return Ok(0);
    }

    let token_amount_u128 = token_amount as u128;
    let virtual_sol_reserves_u128 = virtual_sol_reserves as u128;
    let virtual_token_reserves_u128 = virtual_token_reserves as u128;

    // Calculate: (sol_reserves * tokens_in) / (token_reserves + tokens_in)
    let numerator = virtual_sol_reserves_u128
        .checked_mul(token_amount_u128)
        .ok_or(LaunchpadError::MathOverflow)?;
    
    let denominator = virtual_token_reserves_u128
        .checked_add(token_amount_u128)
        .ok_or(LaunchpadError::MathOverflow)?;

    if denominator == 0 {
        return Err(LaunchpadError::DivisionByZero.into());
    }

    let sol_out = numerator
        .checked_div(denominator)
        .ok_or(LaunchpadError::DivisionByZero)?;

    // Ensure result fits in u64
    if sol_out > u64::MAX as u128 {
        return Err(LaunchpadError::MathOverflow.into());
    }

    Ok(sol_out as u64)
}

/// Calculate platform fee for a given amount
pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<u64> {
    let amount_u128 = amount as u128;
    let fee_bps_u128 = fee_bps as u128;
    let bps_denominator_u128 = BPS_DENOMINATOR as u128;

    let fee = amount_u128
        .checked_mul(fee_bps_u128)
        .ok_or(LaunchpadError::MathOverflow)?
        .checked_div(bps_denominator_u128)
        .ok_or(LaunchpadError::DivisionByZero)?;

    if fee > u64::MAX as u128 {
        return Err(LaunchpadError::MathOverflow.into());
    }

    Ok(fee as u64)
}

/// Check if bonding curve is complete based on SOL reserves
pub fn is_curve_complete(real_sol_reserves: u64) -> bool {
    real_sol_reserves >= BONDING_CURVE_COMPLETION_THRESHOLD
}

/// Calculate price impact for a trade
pub fn calculate_price_impact(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
) -> Result<u64> {
    if reserve_in == 0 || reserve_out == 0 {
        return Ok(0);
    }

    let amount_in_u128 = amount_in as u128;
    let reserve_in_u128 = reserve_in as u128;
    let reserve_out_u128 = reserve_out as u128;

    // Calculate spot price before trade
    let spot_price_before = reserve_out_u128
        .checked_mul(PRECISION)
        .ok_or(LaunchpadError::MathOverflow)?
        .checked_div(reserve_in_u128)
        .ok_or(LaunchpadError::DivisionByZero)?;

    // Calculate effective price for this trade
    let amount_out = calculate_tokens_out(amount_in, reserve_in, reserve_out)?;
    let effective_price = if amount_out > 0 {
        (amount_out as u128)
            .checked_mul(PRECISION)
            .ok_or(LaunchpadError::MathOverflow)?
            .checked_div(amount_in_u128)
            .ok_or(LaunchpadError::DivisionByZero)?
    } else {
        0
    };

    // Calculate price impact as percentage
    let price_impact = if spot_price_before > effective_price {
        (spot_price_before - effective_price)
            .checked_mul(10000) // Convert to basis points
            .ok_or(LaunchpadError::MathOverflow)?
            .checked_div(spot_price_before)
            .ok_or(LaunchpadError::DivisionByZero)?
    } else {
        0
    };

    if price_impact > u64::MAX as u128 {
        return Err(LaunchpadError::MathOverflow.into());
    }

    Ok(price_impact as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tokens_out() {
        let sol_amount = 1_000_000_000; // 1 SOL
        let virtual_sol_reserves = 30_000_000_000; // 30 SOL
        let virtual_token_reserves = 1_073_000_000_000_000; // 1.073B tokens

        let result = calculate_tokens_out(sol_amount, virtual_sol_reserves, virtual_token_reserves);
        assert!(result.is_ok());
        
        let tokens_out = result.unwrap();
        assert!(tokens_out > 0);
        
        // Should receive approximately 34.6M tokens for 1 SOL
        assert!(tokens_out > 30_000_000_000_000); // > 30M tokens
        assert!(tokens_out < 40_000_000_000_000); // < 40M tokens
    }

    #[test]
    fn test_calculate_sol_out() {
        let token_amount = 34_000_000_000_000; // 34M tokens
        let virtual_sol_reserves = 31_000_000_000; // 31 SOL (after previous buy)
        let virtual_token_reserves = 1_039_000_000_000_000; // Remaining tokens

        let result = calculate_sol_out(token_amount, virtual_sol_reserves, virtual_token_reserves);
        assert!(result.is_ok());
        
        let sol_out = result.unwrap();
        assert!(sol_out > 0);
        
        // Should receive close to 1 SOL back (minus slippage)
        assert!(sol_out > 900_000_000); // > 0.9 SOL
        assert!(sol_out < 1_000_000_000); // < 1 SOL
    }

    #[test]
    fn test_calculate_fee() {
        let amount = 1_000_000_000; // 1 SOL
        let fee_bps = 100; // 1%

        let result = calculate_fee(amount, fee_bps);
        assert!(result.is_ok());
        
        let fee = result.unwrap();
        assert_eq!(fee, 10_000_000); // 0.01 SOL
    }

    #[test]
    fn test_is_curve_complete() {
        assert!(!is_curve_complete(84_000_000_000)); // 84 SOL - not complete
        assert!(is_curve_complete(85_000_000_000));  // 85 SOL - complete
        assert!(is_curve_complete(100_000_000_000)); // 100 SOL - complete
    }
}
