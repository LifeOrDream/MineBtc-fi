use anchor_lang::prelude::*;
use crate::{constants::*, errors::NftLaunchpadError};

// ========================================================================================
// =============================== NFT OWNERSHIP UTILITIES =============================== 
// ========================================================================================

/// Verify NFT ownership from Metaplex Core asset (source of truth)
/// Returns the actual owner's pubkey
pub fn verify_nft_ownership(
    asset_account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<()> {
    let actual_owner = get_nft_owner(asset_account)?;
    
    require!(
        actual_owner == *expected_owner,
        NftLaunchpadError::NftNotOwnedByUser
    );
    
    Ok(())
}

/// Get NFT owner from Metaplex Core asset
pub fn get_nft_owner(asset_account: &AccountInfo) -> Result<Pubkey> {
    crate::mpl_core_helpers::get_mpl_core_owner(asset_account)
}

// ========================================================================================
// =============================== VALIDATION UTILITIES ================================== 
// ========================================================================================

/// Validate NFT name
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > MAX_NAME_LENGTH {
        return Err(NftLaunchpadError::NameTooLong.into());
    }
    Ok(())
}

/// Validate metadata URI
pub fn validate_uri(uri: &str) -> Result<()> {
    if uri.len() > MAX_URI_LENGTH {
        return Err(NftLaunchpadError::UriTooLong.into());
    }
    Ok(())
}

/// Validate power value
pub fn validate_power(power: u32) -> Result<()> {
    if power > MAX_EGG_POWER {
        return Err(NftLaunchpadError::PowerExceedsLimit.into());
    }
    Ok(())
}

/// Validate money value
pub fn validate_money(money: u64) -> Result<()> {
    if money > MAX_DOGE_MONEY {
        return Err(NftLaunchpadError::MoneyExceedsLimit.into());
    }
    Ok(())
}

// ========================================================================================
// =============================== CALCULATION UTILITIES ================================= 
// ========================================================================================

/// Calculate power increase for a single egg
pub fn calculate_egg_power_increase(
    total_hashpower: u64,
    total_eggs: u8,
    time_elapsed: i64,
) -> Result<u32> {
    if total_eggs == 0 {
        return Ok(0);
    }
    
    if time_elapsed <= 0 {
        return Ok(0);
    }
    
    // Formula: power_increase = (total_hashpower / total_eggs) * time_elapsed / POWER_RATE_MULTIPLIER
    let hashpower_per_egg = total_hashpower
        .checked_div(total_eggs as u64)
        .ok_or(NftLaunchpadError::DivisionByZero)?;
    
    let power_increase = hashpower_per_egg
        .checked_mul(time_elapsed as u64)
        .ok_or(NftLaunchpadError::ArithmeticOverflow)?
        .checked_div(POWER_RATE_MULTIPLIER)
        .ok_or(NftLaunchpadError::DivisionByZero)? as u32;
    
    Ok(power_increase)
}

/// Calculate money increase for doge
pub fn calculate_doge_money_increase(
    dbtc_mined: u64,
) -> Result<u64> {
    // Formula: money_increase = (dbtc_mined * MONEY_RATE_MULTIPLIER) / 1_000_000
    let money_increase = dbtc_mined
        .checked_mul(MONEY_RATE_MULTIPLIER)
        .ok_or(NftLaunchpadError::ArithmeticOverflow)?
        .checked_div(1_000_000)
        .ok_or(NftLaunchpadError::DivisionByZero)?;
    
    Ok(money_increase)
}

// ========================================================================================
// =============================== TIME UTILITIES ======================================== 
// ========================================================================================

/// Get current timestamp
pub fn get_current_timestamp() -> Result<i64> {
    Clock::get()?
        .unix_timestamp
        .try_into()
        .map_err(|_| NftLaunchpadError::InvalidTimestamp.into())
}

/// Check if enough time has passed since last update
pub fn check_update_cooldown(last_update: i64, current_time: i64) -> Result<bool> {
    let time_elapsed = current_time
        .checked_sub(last_update)
        .ok_or(NftLaunchpadError::ArithmeticUnderflow)?;
    
    Ok(time_elapsed >= UPDATE_FREQUENCY_SECONDS)
}

// ========================================================================================
// =============================== PRICING UTILITIES ===================================== 
// ========================================================================================

/// Determine pricing tier from amount paid
pub fn determine_pricing_tier(amount: u64) -> Result<(bool, bool)> {
    // Returns (includes_doge, includes_egg)
    match amount {
        MOONBASE_BASIC_PRICE => Ok((false, false)),
        MOONBASE_DOGE_PRICE => Ok((true, false)),
        MOONBASE_FULL_PRICE => Ok((true, true)),
        _ => Err(NftLaunchpadError::InvalidPricingTier.into()),
    }
}

/// Get pricing tier name
pub fn get_pricing_tier_name(amount: u64) -> &'static str {
    match amount {
        MOONBASE_BASIC_PRICE => "basic",
        MOONBASE_DOGE_PRICE => "doge",
        MOONBASE_FULL_PRICE => "full",
        _ => "unknown",
    }
}

// ========================================================================================
// =============================== NFT UTILITIES =========================================
// ========================================================================================

/// Generate MoonDoge name
pub fn generate_moondoge_name(index: u64) -> String {
    format!("MoonDoge #{}", index)
}

/// Generate Dragon Egg name
pub fn generate_dragon_egg_name(index: u64) -> String {
    format!("Dragon Egg #{}", index)
}

/// Generate MoonDoge URI (placeholder - would be actual IPFS/Arweave URI)
pub fn generate_moondoge_uri(index: u64) -> String {
    format!("https://arweave.net/moondoge/{}", index)
}

/// Generate Dragon Egg URI (placeholder - would be actual IPFS/Arweave URI)
pub fn generate_dragon_egg_uri(index: u64, dna: &[u8; 32]) -> String {
    // In production, would generate URI based on DNA traits
    let dna_hash = u32::from_le_bytes([dna[0], dna[1], dna[2], dna[3]]);
    format!(
        "https://arweave.net/dragonegg/{}/{}",
        index,
        dna_hash
    )
}
