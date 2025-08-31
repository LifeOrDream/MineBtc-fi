use anchor_lang::prelude::*;
use crate::{constants::*, errors::DragonHiveError};

// ========================================================================================
// =============================== VALIDATION UTILITIES ================================== 
// ========================================================================================

/// Validate DragonBee name
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > MAX_NAME_LENGTH {
        return Err(DragonHiveError::NameTooLong.into());
    }
    Ok(())
}

/// Validate metadata URI
pub fn validate_uri(uri: &str) -> Result<()> {
    if uri.len() > MAX_URI_LENGTH {
        return Err(DragonHiveError::UriTooLong.into());
    }
    Ok(())
}

/// Validate DragonBee type
pub fn validate_bee_type(bee_type: u8) -> Result<()> {
    if bee_type < BEE_TYPE_SOLAR || bee_type > BEE_TYPE_MYSTIC {
        return Err(DragonHiveError::InvalidDragonBeeType.into());
    }
    Ok(())
}

/// Validate genetic data integrity
pub fn validate_genetic_data(genes: &[u8; 32]) -> Result<()> {
    // Check if genetic data is not all zeros (corrupted)
    if genes.iter().all(|&x| x == 0) {
        return Err(DragonHiveError::GeneticDataCorrupted.into());
    }
    
    // Validate DragonBee type from genetic data
    let bee_type = genes[0] & 0x0F;
    validate_bee_type(bee_type)?;
    
    // Validate evolution stage
    let evolution_stage = (genes[0] >> 4) & 0x07;
    if evolution_stage > EVOLUTION_DRAGON {
        return Err(DragonHiveError::InvalidGeneticData.into());
    }
    
    Ok(())
}

/// Validate power increase limits
pub fn validate_power_increase(increase: u32) -> Result<()> {
    if increase > MAX_POWER_INCREASE_PER_UPDATE {
        return Err(DragonHiveError::PowerIncreaseExceedsLimit.into());
    }
    Ok(())
}

// ========================================================================================
// =============================== CALCULATION UTILITIES ================================= 
// ========================================================================================

/// Calculate breeding fee based on parent evolution stages and types
pub fn calculate_breeding_fee(
    parent1_evolution: u8,
    parent2_evolution: u8,
    parent1_type: u8,
    parent2_type: u8,
    base_fee: u64,
) -> u64 {
    let mut fee = base_fee;
    
    // Higher evolution stages cost more
    let evolution_multiplier = (parent1_evolution + parent2_evolution) as u64;
    fee = fee.saturating_add(fee * evolution_multiplier / 10);
    
    // Cross-type breeding costs more
    if parent1_type != parent2_type {
        fee = fee.saturating_mul(150) / 100; // +50% for cross-type
    }
    
    fee
}

/// Calculate kill reward distribution
pub fn calculate_kill_reward(
    dragonbee_power: u32,
    total_active_power: u64,
    total_pool: u64,
) -> Result<u64> {
    if total_active_power == 0 {
        return Ok(0);
    }
    
    if dragonbee_power < MIN_KILL_POWER_THRESHOLD {
        return Err(DragonHiveError::InsufficientPowerForKill.into());
    }
    
    // Reward = (individual_power / total_power) * total_pool
    let reward = (dragonbee_power as u64)
        .checked_mul(total_pool)
        .ok_or(DragonHiveError::ArithmeticOverflow)?
        .checked_div(total_active_power)
        .ok_or(DragonHiveError::DivisionByZero)?;
    
    // Cap reward at 10% of total pool
    let max_reward = total_pool / 10;
    Ok(reward.min(max_reward))
}

/// Calculate evolution power boost
pub fn calculate_evolution_power_boost(current_power: u32, evolution_stage: u8) -> u32 {
    // Each evolution stage provides 20% power boost
    let boost_percentage = (evolution_stage as u32 + 1) * 20;
    current_power.saturating_mul(boost_percentage) / 100
}

/// Calculate breeding success rate
pub fn calculate_breeding_success_rate(
    parent1_evolution: u8,
    parent2_evolution: u8,
    parent1_type: u8,
    parent2_type: u8,
    parent1_power: u32,
    parent2_power: u32,
) -> u16 {
    let mut success_rate = BREEDING_BASE_SUCCESS_RATE;
    
    // Evolution stage bonus
    let evolution_diff = if parent1_evolution > parent2_evolution {
        parent1_evolution - parent2_evolution
    } else {
        parent2_evolution - parent1_evolution
    };
    
    if evolution_diff <= 2 {
        success_rate = success_rate.saturating_add(EVOLUTION_BONUS_RATE * evolution_diff as u16);
    }
    
    // Same type bonus
    if parent1_type == parent2_type {
        success_rate = success_rate.saturating_add(TYPE_MATCH_BONUS);
    }
    
    // Power compatibility bonus
    let power_diff = if parent1_power > parent2_power {
        parent1_power - parent2_power
    } else {
        parent2_power - parent1_power
    };
    
    if power_diff < 1000 {
        success_rate = success_rate.saturating_add(200); // +2% for similar power
    }
    
    // Cap at 100%
    success_rate.min(10000)
}

// ========================================================================================
// =============================== GENETIC UTILITIES ===================================== 
// ========================================================================================

/// Generate random genetic data for genesis DragonBees
pub fn generate_genesis_genes(bee_type: u8, slot: u64, user_key: &Pubkey) -> [u8; 32] {
    let mut genes = [0u8; 32];
    
    // Set DragonBee type (first 4 bits)
    genes[0] = bee_type & 0x0F;
    
    // Set evolution stage to 0 (bits 4-6)
    genes[0] |= 0x00; // Already 0, but explicit
    
    // Generate pseudo-random traits using slot and user key as seed
    let seed = slot.wrapping_add(user_key.to_bytes()[0] as u64);
    
    for i in 1..32 {
        // Simple PRNG for genetic diversity
        let random_value = seed
            .wrapping_mul(1103515245)
            .wrapping_add(12345 + i as u64)
            .wrapping_mul(user_key.to_bytes()[i % 32] as u64);
        
        genes[i] = (random_value & 0xFF) as u8;
    }
    
    genes
}

/// Breed two genetic codes to create offspring
pub fn breed_genetic_codes(
    parent1_genes: &[u8; 32],
    parent2_genes: &[u8; 32],
    random_seed: u64,
) -> Result<[u8; 32]> {
    let mut offspring_genes = [0u8; 32];
    
    // Offspring gets same type as parents (must be same type to breed)
    let parent1_type = parent1_genes[0] & 0x0F;
    let parent2_type = parent2_genes[0] & 0x0F;
    
    if parent1_type != parent2_type {
        return Err(DragonHiveError::IncompatibleBreedingTypes.into());
    }
    
    offspring_genes[0] = parent1_type; // Evolution stage starts at 0
    
    // Mix genetic traits
    for i in 1..32 {
        let bit_selector = (random_seed >> (i % 64)) & 1;
        
        if bit_selector == 0 {
            offspring_genes[i] = parent1_genes[i];
        } else {
            offspring_genes[i] = parent2_genes[i];
        }
        
        // Chance for mutation (5%)
        let mutation_chance = (random_seed >> ((i * 2) % 64)) & 0x1F;
        if mutation_chance == 0 {
            offspring_genes[i] = offspring_genes[i].wrapping_add(1);
        }
        
        // Chance for enhancement (15%)
        let enhancement_chance = (random_seed >> ((i * 3) % 64)) & 0x0F;
        if enhancement_chance < 2 {
            let parent_max = parent1_genes[i].max(parent2_genes[i]);
            offspring_genes[i] = offspring_genes[i].max(parent_max);
        }
    }
    
    Ok(offspring_genes)
}

/// Evolve genetic code (enhance traits for evolution)
pub fn evolve_genetic_code(genes: &[u8; 32], current_evolution: u8) -> Result<[u8; 32]> {
    if current_evolution >= EVOLUTION_DRAGON {
        return Err(DragonHiveError::AlreadyMaxEvolution.into());
    }
    
    let mut evolved_genes = *genes;
    
    // Increase evolution stage
    let new_evolution = current_evolution + 1;
    evolved_genes[0] = (evolved_genes[0] & 0x8F) | ((new_evolution & 0x07) << 4);
    
    // Enhance traits based on evolution stage
    for i in 1..32 {
        // Higher evolution stages get bigger enhancements
        let enhancement_chance = match new_evolution {
            1..=2 => 10, // 10% chance
            3..=4 => 15, // 15% chance
            5..=6 => 20, // 20% chance
            7 => 25,     // 25% chance (dragon stage)
            _ => 5,      // Default
        };
        
        if (i as u8 * new_evolution) % 100 < enhancement_chance {
            evolved_genes[i] = evolved_genes[i].saturating_add(1 + new_evolution / 2);
        }
    }
    
    Ok(evolved_genes)
}

// ========================================================================================
// =============================== TIME UTILITIES ======================================== 
// ========================================================================================

/// Get current timestamp
pub fn get_current_timestamp() -> Result<i64> {
    Clock::get()?.unix_timestamp.try_into()
        .map_err(|_| DragonHiveError::InvalidTimestamp.into())
}

/// Check if enough time has passed since last operation
pub fn check_cooldown(last_operation: i64, cooldown_duration: i64) -> Result<bool> {
    let current_time = get_current_timestamp()?;
    Ok(current_time >= last_operation.saturating_add(cooldown_duration))
}

/// Calculate cooldown end time
pub fn calculate_cooldown_end(start_time: i64, cooldown_duration: i64) -> Result<i64> {
    start_time.checked_add(cooldown_duration)
        .ok_or(DragonHiveError::ArithmeticOverflow.into())
}

// ========================================================================================
// =============================== ECONOMIC UTILITIES ==================================== 
// ========================================================================================

/// Calculate fee distribution (team vs buyback)
pub fn calculate_fee_distribution(total_amount: u64) -> Result<(u64, u64, u64)> {
    let team_portion = total_amount
        .checked_mul(TEAM_FEE_PERCENTAGE as u64)
        .ok_or(DragonHiveError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(DragonHiveError::DivisionByZero)?;
    
    let buyback_portion = total_amount
        .checked_mul(BUYBACK_PERCENTAGE as u64)
        .ok_or(DragonHiveError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(DragonHiveError::DivisionByZero)?;
    
    let kill_pool_portion = buyback_portion
        .checked_mul(KILL_POOL_PERCENTAGE as u64)
        .ok_or(DragonHiveError::ArithmeticOverflow)?
        .checked_div(100)
        .ok_or(DragonHiveError::DivisionByZero)?;
    
    Ok((team_portion, buyback_portion - kill_pool_portion, kill_pool_portion))
}

/// Calculate auction bid increment
pub fn calculate_min_bid_increment(current_bid: u64) -> u64 {
    // Minimum 1% increment, but at least 0.01 SOL
    let one_percent = current_bid / 100;
    one_percent.max(MIN_BID_AMOUNT)
}

// ========================================================================================
// =============================== RANDOM UTILITIES ====================================== 
// ========================================================================================

/// Generate pseudo-random number from slot and seeds
pub fn generate_random(slot: u64, seed1: &Pubkey, seed2: Option<&Pubkey>) -> u64 {
    let mut hasher = slot;
    
    // Mix in first seed
    for byte in seed1.to_bytes().iter() {
        hasher = hasher.wrapping_mul(31).wrapping_add(*byte as u64);
    }
    
    // Mix in second seed if provided
    if let Some(seed2) = seed2 {
        for byte in seed2.to_bytes().iter() {
            hasher = hasher.wrapping_mul(37).wrapping_add(*byte as u64);
        }
    }
    
    // Final mixing
    hasher = hasher.wrapping_mul(1103515245).wrapping_add(12345);
    hasher
}

/// Check if random event should occur based on rate
pub fn should_random_event_occur(random_value: u64, rate_out_of_10000: u16) -> bool {
    (random_value % 10000) < rate_out_of_10000 as u64
}

// ========================================================================================
// =============================== RARITY UTILITIES ====================================== 
// ========================================================================================

/// Calculate DragonBee rarity score based on traits
pub fn calculate_rarity_score(genes: &[u8; 32]) -> u32 {
    let mut rarity_score = 0u32;
    
    // Higher evolution stages are rarer
    let evolution_stage = (genes[0] >> 4) & 0x07;
    rarity_score += (evolution_stage as u32) * 1000;
    
    // Count high-value traits
    for &gene_byte in genes.iter().skip(1) {
        // Traits with values > 200 are considered rare
        if gene_byte > 200 {
            rarity_score += 100;
        }
        // Traits with values > 240 are very rare
        if gene_byte > 240 {
            rarity_score += 200;
        }
        // Perfect traits (255) are legendary
        if gene_byte == 255 {
            rarity_score += 500;
        }
    }
    
    rarity_score
}

/// Determine if DragonBee has rare traits
pub fn has_rare_traits(genes: &[u8; 32]) -> Vec<String> {
    let mut rare_traits = Vec::new();
    
    let evolution_stage = (genes[0] >> 4) & 0x07;
    if evolution_stage >= EVOLUTION_ROYAL {
        rare_traits.push("High Evolution".to_string());
    }
    
    // Check for perfect traits
    let perfect_count = genes.iter().skip(1).filter(|&&x| x == 255).count();
    if perfect_count > 0 {
        rare_traits.push(format!("{} Perfect Traits", perfect_count));
    }
    
    // Check for very high traits
    let high_traits_count = genes.iter().skip(1).filter(|&&x| x > 240).count();
    if high_traits_count > 5 {
        rare_traits.push("Exceptional Genetics".to_string());
    }
    
    rare_traits
}
