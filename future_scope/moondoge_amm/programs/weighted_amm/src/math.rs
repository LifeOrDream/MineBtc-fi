use crate::constants::*;
use crate::errors::AmmError;
use anchor_lang::prelude::*;

/// Calculate the amount of tokens to receive for a given input using weighted math
/// Formula: amount_out = balance_out * (1 - (balance_in / (balance_in + amount_in))^(weight_in/weight_out))
pub fn calculate_out_given_in(
    token_balance_in: u128,
    token_weight_in: u128,
    token_balance_out: u128,
    token_weight_out: u128,
    token_amount_in: u128,
) -> Result<u128> {
    if token_amount_in == 0 {
        return Ok(0);
    }

    // Check swap ratio limits
    let swap_ratio = (token_amount_in * PRECISION) / token_balance_in;
    require!(swap_ratio <= MAX_SWAP_RATIO, AmmError::InvalidSwapRatio);

    // Calculate: balance_in + amount_in
    let new_balance_in = token_balance_in
        .checked_add(token_amount_in)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate: balance_in / (balance_in + amount_in)
    let base = (token_balance_in * PRECISION) / new_balance_in;

    // Calculate: (weight_in / weight_out)
    let exponent = (token_weight_in * PRECISION) / token_weight_out;

    // Calculate: base^exponent
    let power_result = pow_approx(base, exponent)?;

    // Calculate: 1 - power_result
    let factor = PRECISION
        .checked_sub(power_result)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate: balance_out * factor
    let amount_out = (token_balance_out * factor) / PRECISION;

    Ok(amount_out)
}

/// Calculate the amount of tokens needed as input for a given output using weighted math
/// Formula: amount_in = balance_in * ((balance_out / (balance_out - amount_out))^(weight_out/weight_in) - 1)
pub fn calculate_in_given_out(
    token_balance_in: u128,
    token_weight_in: u128,
    token_balance_out: u128,
    token_weight_out: u128,
    token_amount_out: u128,
) -> Result<u128> {
    if token_amount_out == 0 {
        return Ok(0);
    }

    require!(
        token_amount_out < token_balance_out,
        AmmError::InsufficientLiquidity
    );

    // Check swap ratio limits
    let swap_ratio = (token_amount_out * PRECISION) / token_balance_out;
    require!(swap_ratio <= MAX_SWAP_RATIO, AmmError::InvalidSwapRatio);

    // Calculate: balance_out - amount_out
    let new_balance_out = token_balance_out
        .checked_sub(token_amount_out)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate: balance_out / (balance_out - amount_out)
    let base = (token_balance_out * PRECISION) / new_balance_out;

    // Calculate: (weight_out / weight_in)
    let exponent = (token_weight_out * PRECISION) / token_weight_in;

    // Calculate: base^exponent
    let power_result = pow_approx(base, exponent)?;

    // Calculate: power_result - 1
    let factor = power_result
        .checked_sub(PRECISION)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate: balance_in * factor
    let amount_in = (token_balance_in * factor) / PRECISION;

    Ok(amount_in)
}

/// Calculate LP tokens to mint for given token deposits
/// Formula: lp_out = lp_supply * ((balance_0_new/balance_0)^weight_0 * (balance_1_new/balance_1)^weight_1 - 1)
pub fn calculate_lp_tokens_for_deposit(
    token_0_balance: u128,
    token_1_balance: u128,
    token_0_deposit: u128,
    token_1_deposit: u128,
    token_0_weight: u128,
    token_1_weight: u128,
    lp_supply: u128,
) -> Result<u128> {
    if lp_supply == 0 {
        // For first deposit, use geometric mean
        return Ok(isqrt(token_0_deposit * token_1_deposit));
    }

    if token_0_deposit == 0 && token_1_deposit == 0 {
        return Ok(0);
    }

    let new_balance_0 = token_0_balance
        .checked_add(token_0_deposit)
        .ok_or(AmmError::MathOverflow)?;
    let new_balance_1 = token_1_balance
        .checked_add(token_1_deposit)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate ratio for token 0
    let ratio_0 = if token_0_balance > 0 {
        (new_balance_0 * PRECISION) / token_0_balance
    } else {
        PRECISION
    };

    // Calculate ratio for token 1
    let ratio_1 = if token_1_balance > 0 {
        (new_balance_1 * PRECISION) / token_1_balance
    } else {
        PRECISION
    };

    // Calculate weighted ratios
    let weighted_ratio_0 = pow_approx(ratio_0, token_0_weight)?;
    let weighted_ratio_1 = pow_approx(ratio_1, token_1_weight)?;

    // Calculate combined ratio
    let combined_ratio = (weighted_ratio_0 * weighted_ratio_1) / PRECISION;

    // Calculate LP tokens to mint
    let factor = combined_ratio
        .checked_sub(PRECISION)
        .ok_or(AmmError::MathOverflow)?;

    let lp_tokens = (lp_supply * factor) / PRECISION;

    Ok(lp_tokens)
}

/// Calculate token amounts to withdraw for given LP tokens
/// Formula: token_out = balance * (1 - (1 - lp_ratio)^(1/weight))
pub fn calculate_tokens_for_withdrawal(
    token_0_balance: u128,
    token_1_balance: u128,
    token_0_weight: u128,
    token_1_weight: u128,
    lp_tokens: u128,
    lp_supply: u128,
) -> Result<(u128, u128)> {
    if lp_supply == 0 || lp_tokens == 0 {
        return Ok((0, 0));
    }

    // Calculate LP ratio
    let lp_ratio = (lp_tokens * PRECISION) / lp_supply;

    // Calculate (1 - lp_ratio)
    let complement = PRECISION
        .checked_sub(lp_ratio)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate exponents (1/weight)
    let exp_0 = PRECISION / token_0_weight;
    let exp_1 = PRECISION / token_1_weight;

    // Calculate power results
    let power_0 = pow_approx(complement, exp_0)?;
    let power_1 = pow_approx(complement, exp_1)?;

    // Calculate factors (1 - power_result)
    let factor_0 = PRECISION
        .checked_sub(power_0)
        .ok_or(AmmError::MathOverflow)?;
    let factor_1 = PRECISION
        .checked_sub(power_1)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate token amounts
    let token_0_out = (token_0_balance * factor_0) / PRECISION;
    let token_1_out = (token_1_balance * factor_1) / PRECISION;

    Ok((token_0_out, token_1_out))
}

/// Calculate trading fee for a given amount
pub fn calculate_fee(amount: u128, fee_rate: u64) -> Result<u128> {
    let fee = (amount * fee_rate as u128) / FEE_RATE_DENOMINATOR as u128;
    Ok(fee)
}

/// Approximate power function using Taylor series
/// Calculates base^exp where both are in PRECISION units
pub fn pow_approx(base: u128, exp: u128) -> Result<u128> {
    if exp == 0 {
        return Ok(PRECISION);
    }
    if exp == PRECISION {
        return Ok(base);
    }
    if base == 0 {
        return Ok(0);
    }
    if base == PRECISION {
        return Ok(PRECISION);
    }

    // Use natural logarithm approximation for x^y = e^(y * ln(x))
    let ln_base = ln_approx(base)?;
    let exp_ln = (exp * ln_base) / PRECISION;
    exp_approx(exp_ln)
}

/// Natural logarithm approximation using Taylor series
fn ln_approx(x: u128) -> Result<u128> {
    require!(x > 0, AmmError::DivisionByZero);
    
    if x == PRECISION {
        return Ok(0);
    }

    // Use the identity ln(x) = ln(x/e^k) + k where x/e^k is close to 1
    let mut result = 0i128;
    let mut value = x;

    // Normalize to range [0.5, 2) by factoring out powers of 2
    while value >= 2 * PRECISION {
        value /= 2;
        result += 693147180559945309; // ln(2) * PRECISION
    }
    while value < PRECISION / 2 {
        value *= 2;
        result -= 693147180559945309; // ln(2) * PRECISION
    }

    // Now use Taylor series for ln(1 + x) where x = value - 1
    let x_minus_1 = if value >= PRECISION {
        value - PRECISION
    } else {
        return Err(AmmError::MathOverflow.into());
    };

    // ln(1 + x) = x - x²/2 + x³/3 - x⁴/4 + ...
    let mut term = x_minus_1;
    let mut series_result = 0i128;
    
    for i in 1..=MAX_ITERATIONS {
        let term_contribution = term as i128 / i as i128;
        if i % 2 == 1 {
            series_result += term_contribution;
        } else {
            series_result -= term_contribution;
        }
        
        term = (term * x_minus_1) / PRECISION;
        if term < 1000 { // Convergence threshold
            break;
        }
    }

    result += series_result;
    
    if result < 0 {
        return Err(AmmError::MathOverflow.into());
    }
    
    Ok(result as u128)
}

/// Exponential function approximation using Taylor series
fn exp_approx(x: u128) -> Result<u128> {
    if x == 0 {
        return Ok(PRECISION);
    }

    // e^x = 1 + x + x²/2! + x³/3! + ...
    let mut result = PRECISION;
    let mut term = x;
    
    for i in 2..=MAX_ITERATIONS {
        result = result
            .checked_add(term)
            .ok_or(AmmError::MathOverflow)?;
        
        term = (term * x) / (PRECISION * i as u128);
        if term < 1000 { // Convergence threshold
            break;
        }
    }

    Ok(result)
}

/// Integer square root using Newton's method
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_out_given_in() {
        let balance_in = 1000 * PRECISION;
        let weight_in = 50 * PRECISION / 100; // 50%
        let balance_out = 2000 * PRECISION;
        let weight_out = 50 * PRECISION / 100; // 50%
        let amount_in = 100 * PRECISION;

        let result = calculate_out_given_in(
            balance_in,
            weight_in,
            balance_out,
            weight_out,
            amount_in,
        );
        
        assert!(result.is_ok());
        let amount_out = result.unwrap();
        
        // Should receive less than 200 tokens due to slippage
        assert!(amount_out > 180 * PRECISION);
        assert!(amount_out < 200 * PRECISION);
    }

    #[test]
    fn test_calculate_lp_tokens_for_deposit() {
        let balance_0 = 1000 * PRECISION;
        let balance_1 = 2000 * PRECISION;
        let deposit_0 = 100 * PRECISION;
        let deposit_1 = 200 * PRECISION;
        let weight_0 = 50 * PRECISION / 100;
        let weight_1 = 50 * PRECISION / 100;
        let lp_supply = 1000 * PRECISION;

        let result = calculate_lp_tokens_for_deposit(
            balance_0,
            balance_1,
            deposit_0,
            deposit_1,
            weight_0,
            weight_1,
            lp_supply,
        );
        
        assert!(result.is_ok());
        let lp_tokens = result.unwrap();
        
        // Should receive approximately 100 LP tokens for proportional deposit
        assert!(lp_tokens > 95 * PRECISION);
        assert!(lp_tokens < 105 * PRECISION);
    }

    #[test]
    fn test_pow_approx() {
        // Test 2^2 = 4
        let base = 2 * PRECISION;
        let exp = 2 * PRECISION;
        let result = pow_approx(base, exp).unwrap();
        let expected = 4 * PRECISION;
        
        // Allow for some approximation error
        let diff = if result > expected {
            result - expected
        } else {
            expected - result
        };
        assert!(diff < PRECISION / 100); // Less than 1% error
    }
}
