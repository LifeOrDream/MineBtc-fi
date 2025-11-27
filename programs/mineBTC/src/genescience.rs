use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

use crate::errors::ErrorCode;
use crate::state::{BASE_MULTIPLIER, MAX_BASE_CHANCE};

// ========================================================================================
// ============================= DNA STRUCTURE & CONSTANTS ================================
// ========================================================================================
//
/// DNA Generation and Manipulation for Cyber-Doge Assets
///
/// DNA STRUCTURE (256 bits / 32 bytes):
/// 1. Faction/Family (4 bits): Maps to Faction ID (0-11)
/// 2. Evolution Stage (3 bits): Current level (0-7)
/// 3. Appearance Genes (105 bits): 7 groups × 3 traits × 5 bits (0-31)
///    - Per group: [Dominant][Recessive][Minor Recessive]
/// 4. Combat Genes (60 bits): 5 groups × 3 traits × 4 bits (0-15)
///    - Per group: [Dominant][Recessive][Minor Recessive]
/// 5. Reserved (84 bits): Future use

const FACTION_TYPE_BITS: u8 = 4;
const EVOLUTION_STAGE_BITS: u8 = 3;
const APPEARANCE_TRAIT_BITS: u8 = 5;  // 0-31 values
const POWER_TRAIT_BITS: u8 = 4;       // 0-15 values

const APPEARANCE_OFFSET: u8 = FACTION_TYPE_BITS + EVOLUTION_STAGE_BITS; // 7
const COMBAT_OFFSET: u8 = APPEARANCE_OFFSET + (21 * APPEARANCE_TRAIT_BITS); // 7 + 105 = 112

const APPEARANCE_GROUPS: usize = 7;
const APPEARANCE_TRAITS_PER_GROUP: usize = 3;  // Dominant, Recessive, Minor Recessive
const APPEARANCE_TOTAL_TRAITS: usize = 21;     // 7 × 3

const COMBAT_GROUPS: usize = 5;
const COMBAT_TRAITS_PER_GROUP: usize = 3;
const COMBAT_TOTAL_TRAITS: usize = 15;         // 5 × 3

const EVOLUTION_STAGES: u8 = 8;
const APPEARANCE_MAX: u8 = 31;
const COMBAT_MAX: u8 = 15;

// ========================================================================================
// ============================= PRICE CALCULATION ========================================
// ========================================================================================

/// Calculate dynamic pricing for Genetic Assets (Bonding Curve)
pub fn compute_gene_price(base_price: u64, curve_a: u64, items_minted: u64) -> Result<u64> {
    if items_minted == 0 {
        return Ok(base_price);
    }

    let items_u128 = items_minted as u128;
    let squared = items_u128.checked_mul(items_u128).ok_or(ErrorCode::ArithmeticOverflow)?;

    let mut low: u128 = 1;
    let mut high = squared.min(1_000_000_000_000_000_000);
    let mut result: u128 = 0;

    while low <= high {
        let mid = (low + high) / 2;
        let cube = mid.checked_mul(mid).and_then(|x| x.checked_mul(mid)).ok_or(ErrorCode::ArithmeticOverflow)?;
        if cube <= squared {
            result = mid;
            low = mid + 1;
        } else {
            if mid == 0 { break; }
            high = mid - 1;
        }
    }

    let exponent_component = result.min(u64::MAX as u128) as u64;
    let price_increase = curve_a.checked_mul(exponent_component).ok_or(ErrorCode::ArithmeticOverflow)?;
    base_price.checked_add(price_increase).ok_or(ErrorCode::ArithmeticOverflow.into())
}

// ========================================================================================
// ============================= DNA GENERATION ===========================================
// ========================================================================================

/// Generate unique DNA for a Genesis Cyber-Doge
pub fn generate_genesis_dna(mint_number: u64, minter: &Pubkey, slot: u64, faction_id: u8) -> Result<[u8; 32]> {
    require!(faction_id < 16, ErrorCode::InvalidFactionId);

    let mut seed_data = Vec::new();
    seed_data.extend_from_slice(&mint_number.to_le_bytes());
    seed_data.extend_from_slice(&minter.to_bytes());
    seed_data.extend_from_slice(&slot.to_le_bytes());

    let hash = keccak::hash(&seed_data);
    let mut dna = hash.to_bytes();

    // Encode Faction (first 4 bits)
    dna[0] = (dna[0] & 0xF0) | (faction_id & 0x0F);
    // Evolution Stage 0 (bits 4-6)
    dna[0] = dna[0] & 0x8F;

    limit_trait_ranges(&mut dna);
    Ok(dna)
}

fn limit_trait_ranges(dna: &mut [u8; 32]) {
    for i in 0..APPEARANCE_TOTAL_TRAITS {
        let val = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        set_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8, val % 32);
    }
    for i in 0..COMBAT_TOTAL_TRAITS {
        let val = get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
        set_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, val % 16);
    }
}

// ========================================================================================
// ============================= MUTATION SYSTEM ==========================================
// ========================================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MutationType {
    /// Evolution: generation += 1, xp = 0 (~10%)
    Evolution,
    /// Power: multiplier += 25 (0.25x) (~30%)
    Power,
    /// Trait: reroll a visual gene in dna (~60%)
    Trait,
}

#[derive(Clone, Debug)]
pub struct MutationResult {
    /// Mutation type (None if no mutation triggered)
    pub mutation_type: Option<MutationType>,
    /// XP gained from betting (always > 0 for SOL bets)
    pub xp_gained: u32,
    /// Multiplier increase (0 if no Power mutation or no mutation)
    pub multiplier_increase: u32,
    /// New DNA after mutation  
    pub new_dna: [u8; 32],
}

/// **Chance Formula**: `base_chance * bet_strength * multiplier_penalty`
/// - `base_chance` = 30%
/// - `bet_strength` = min(user_bet / highest_bet, 1.0)
/// - `multiplier_penalty` = 100 / (100 + current_multiplier - 100) = 100 / current_multiplier
///   (Higher multiplier = lower chance, slowing progression for advanced eggs)
///
/// **XP Gain**: Always gained on SOL bets, scaled by bet size relative to 0.1 SOL
/// - Base XP per 0.1 SOL = 10 XP
/// 
/// **Multiplier Increase**: Only on Power mutation, +25 (0.25x)
pub fn calculate_mutation_result(
    user_total_bet: u64,
    highest_round_bet_for_faction: u64,
    current_multiplier: u32,
    mut gameplay_egg_dna: [u8; 32],
    gameplay_egg_generation: u8,
    gameplay_egg_xp: u32,
    total_sol_bets: u64,
    total_points_bets: u64,
    total_wgtd_points_bets: u64,
    slot: u64,
    user_key: &Pubkey,
) -> MutationResult {
    msg!("🧬 Calculating mutation result...");
    msg!("   User total bet: {}, Highest round bet for faction: {}", user_total_bet as f64 / 1e9, highest_round_bet_for_faction as f64 / 1e9);
    msg!("   Current multiplier: {}. Gameplay egg generation: {}. Gameplay egg XP: {}", current_multiplier, gameplay_egg_generation, gameplay_egg_xp);
    msg!("   Total sol bets: {}, Total points bets: {}, Total wgtd points bets: {}", total_sol_bets as f64 / 1e9, total_points_bets as f64 / 1e9, total_wgtd_points_bets as f64 / 1e9);
    msg!("   Slot: {}. User key: {}", slot, user_key);

    // --- STEP 1: CALCULATE TRIGGER CHANCE ---

    // XP always gained: 1 XP per 0.001 SOL, capped at 1000
    let xp_gained = ((user_total_bet as u128 * 1) / 1_000_000).min(1000) as u32;
    
    if user_total_bet == 0 {
        return MutationResult { mutation_type: None, xp_gained, multiplier_increase: 0, new_dna: gameplay_egg_dna };
    }

    // A. Bet Strength (0 - 10,000 bps) -  If highest is 0 (first bet), strength is 100%. Otherwise ratio.
    let effective_highest = if highest_round_bet_for_faction == 0 { user_total_bet } else { highest_round_bet_for_faction };
    let bet_strength = ((user_total_bet * 10000) / effective_highest).min(10000);

    msg!("   Bet strength: {}", bet_strength as f64 / 10000.0);

    // B. Multiplier Penalty (0 - 10,000 bps)
    // Formula: (BASE_MULTIPLIER / Current) * 10,000
    // 1.0x -> 1k/1k * 1k = 1000 (100% factor)
    // 6.9x -> 1k/6900 * 1k = 144.9 (14.5% factor)
    let mult_factor = (BASE_MULTIPLIER * 10000) / current_multiplier;
    msg!("   Multiplier factor: {}", mult_factor);

    // C. Final Chance Calculation
    // Chance = 30% * BetFactor * MultFactor
    // scale: 3000 * (0.0-1.0) * (0.14-1.0)
    // Final Chance = 30% * bet_strength * mult_factor / 100M
    let final_chance_bps = (MAX_BASE_CHANCE * bet_strength * (mult_factor as u64)) / 100_000_000;
    msg!("   Final chance: {}", final_chance_bps as f64 / 100.0);

    // --- STEP 2: ROLL THE DICE ---

    // Roll the dice
    let total_combined = total_sol_bets + total_points_bets + total_wgtd_points_bets;
    let seed = keccak::hashv(&[&slot.to_le_bytes(), &total_sol_bets.to_le_bytes(), &total_combined.to_le_bytes(), user_key.as_ref(), &gameplay_egg_dna]).to_bytes();

    // Roll 1: Did we hit the mutation? (0-10000)
    // Use first 2 bytes for higher precision (u16)    
    let roll_val = u16::from_le_bytes([seed[0], seed[1]]) as u64;
    let roll_normalized = (roll_val * 10_000) / 65535;

    if roll_normalized >= final_chance_bps {
        return MutationResult { mutation_type: None, xp_gained, multiplier_increase: 0, new_dna: gameplay_egg_dna };
    }

    // Mutation triggered - determine type
    let type_roll = seed[2];
    let evo_chance = 10 / (gameplay_egg_generation as u64 + 1);
    let evo_threshold = (255 * evo_chance) / 100;
    let power_threshold = evo_threshold + ((255 * 30) / 100);

    let (m_type, base_boost) = if (type_roll as u64) < evo_threshold {
        let _ = evolve_stage(&mut gameplay_egg_dna, &seed);
        (MutationType::Evolution, 50u32)
    } else if (type_roll as u64) < power_threshold {
        mutate_power_trait(&mut gameplay_egg_dna, &seed);
        (MutationType::Power, 25u32)
    } else {
        mutate_visual_trait(&mut gameplay_egg_dna, &seed);
        (MutationType::Trait, 5u32)
    };

    // XP Bonus: use % of current XP to boost multiplier
    let xp_roll = seed[3] as u64;
    let (min_pct, max_pct) = if m_type == MutationType::Evolution { (50, 100) } else { (10, 50) };
    let efficiency_pct = min_pct + ((xp_roll * (max_pct - min_pct)) / 255);
    let xp_mult_boost = ((gameplay_egg_xp as u64 * efficiency_pct) / 100) / 10;

    MutationResult {
        mutation_type: Some(m_type),
        xp_gained,
        multiplier_increase: base_boost + xp_mult_boost as u32,
        new_dna: gameplay_egg_dna,
    }
}


 
// ========================================================================================
// ============================= TRAIT MUTATION FUNCTIONS =================================
// ========================================================================================

/// Evolve to next Generation with guaranteed mutations
pub fn evolve_stage(dna: &mut [u8; 32], seed: &[u8]) -> Result<u8> {
    let current_stage = (dna[0] >> 4) & 0x07;
    require!(current_stage < 7, ErrorCode::InvalidParameters);

    let new_stage = current_stage + 1;
    dna[0] = (dna[0] & 0x8F) | ((new_stage & 0x07) << 4);
    msg!("🧬 EVOLUTION: Stage {} -> {}", current_stage, new_stage);

    let _ = mutate_visual_trait(dna, seed);
    let power_seed = keccak::hashv(&[seed, b"power"]).to_bytes();
    let _ = mutate_power_trait(dna, &power_seed);

    Ok(new_stage)
}

/// Mutate a random visual trait (+1 to +3, cap at 31)
pub fn mutate_visual_trait(dna: &mut [u8; 32], seed: &[u8]) -> u8 {
    let hash = keccak::hash(seed).to_bytes();
    let trait_index = hash[0] as usize % APPEARANCE_TOTAL_TRAITS;
    let current_val = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, trait_index as u8);
    let increase = (hash[1] % 3) + 1;
    let new_val = (current_val + increase).min(APPEARANCE_MAX);

    if new_val > current_val {
        set_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, trait_index as u8, new_val);
        msg!("🎨 Visual: Trait #{} {} -> {}", trait_index, current_val, new_val);
    }
    trait_index as u8
}

/// Mutate a random power trait (+1 to +3, cap at 15)
pub fn mutate_power_trait(dna: &mut [u8; 32], seed: &[u8]) -> u8 {
    let hash = keccak::hash(seed).to_bytes();
    let trait_index = hash[2] as usize % COMBAT_TOTAL_TRAITS;
    let current_val = get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, trait_index as u8);
    let increase = (hash[3] % 3) + 1;
    let new_val = (current_val + increase).min(COMBAT_MAX);

    if new_val > current_val {
        set_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, trait_index as u8, new_val);
        msg!("⚡ Power: Trait #{} {} -> {}", trait_index, current_val, new_val);
    }
    trait_index as u8
}

// ========================================================================================
// ============================= BREEDING SYSTEM ==========================================
// ========================================================================================

/// Breed two characters to create offspring
pub fn breed_genes(parent1_dna: &[u8; 32], parent2_dna: &[u8; 32], seed: &[u8]) -> Result<[u8; 32]> {
    let hash = keccak::hash(seed).to_bytes();
    let mut offspring_dna = [0u8; 32];

    offspring_dna[0] = parent1_dna[0] & 0x0F; // Same faction, stage 0

    mix_appearance_traits(&mut offspring_dna, parent1_dna, parent2_dna, &hash);
    mix_power_traits(&mut offspring_dna, parent1_dna, parent2_dna, &hash);

    Ok(offspring_dna)
}

fn mix_appearance_traits(offspring: &mut [u8; 32], p1: &[u8; 32], p2: &[u8; 32], random: &[u8; 32]) {
    for i in 0..APPEARANCE_TOTAL_TRAITS {
        let t1 = get_trait_value(p1, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        let t2 = get_trait_value(p2, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        let method = random[i % 32] % 4;
        let rand = random[(i + 16) % 32];

        let selected = match method {
            0 => if rand % 2 == 0 { t1 } else { t2 },
            1 => ((t1 as u16 + t2 as u16) / 2) as u8,
            2 => enhance_trait(t1, t2, rand, APPEARANCE_MAX),
            _ => mutate_trait_value(t1.max(t2), rand, APPEARANCE_MAX),
        };
        set_trait_value(offspring, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8, selected);
    }
}

fn mix_power_traits(offspring: &mut [u8; 32], p1: &[u8; 32], p2: &[u8; 32], random: &[u8; 32]) {
    for i in 0..COMBAT_TOTAL_TRAITS {
        let t1 = get_trait_value(p1, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
        let t2 = get_trait_value(p2, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
        let method = random[(i + 8) % 32] % 3;
        let rand = random[(i + 24) % 32];

        let selected = match method {
            0 => t1.max(t2),
            1 => enhance_trait(t1, t2, rand, COMBAT_MAX),
            _ => synergy_boost(t1, t2, rand, COMBAT_MAX),
        };
        set_trait_value(offspring, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, selected);
    }
}

fn enhance_trait(t1: u8, t2: u8, rand: u8, max: u8) -> u8 {
    let max_t = t1.max(t2);
    let min_t = t1.min(t2);
    if rand < 48 && max_t < max { max_t + 1 }
    else if rand < 96 && min_t < max_t { min_t + 1 }
    else { max_t }
}

fn mutate_trait_value(base: u8, rand: u8, max: u8) -> u8 {
    if rand == 0 { (rand << 1).min(max - 1) }
    else if rand < 48 && base > 0 { base - 1 }
    else if rand < 80 && base < max { base + 1 }
    else { base }
}

fn synergy_boost(t1: u8, t2: u8, rand: u8, max: u8) -> u8 {
    let max_t = t1.max(t2);
    let diff = if t1 > t2 { t1 - t2 } else { t2 - t1 };
    if diff <= 2 && rand < 64 && max_t < max { max_t + 1 } else { max_t }
}

// ========================================================================================
// ============================= EVOLUTION SYSTEM =========================================
// ========================================================================================

/// Evolve genes to next stage with probabilistic trait improvements
pub fn evolve_genes(dna: &mut [u8; 32], seed: &[u8]) -> Result<u8> {
    let current_stage = (dna[0] >> 4) & 0x07;
    require!(current_stage < 7, ErrorCode::InvalidParameters);

    let new_stage = current_stage + 1;
    dna[0] = (dna[0] & 0x8F) | ((new_stage & 0x07) << 4);

    let hash = keccak::hash(seed).to_bytes();
    evolve_appearance_traits(dna, new_stage, &hash);
    evolve_power_traits(dna, new_stage, &hash);

    Ok(new_stage)
}

fn evolve_appearance_traits(dna: &mut [u8; 32], stage: u8, random: &[u8; 32]) {
    let chance = evolution_chance(stage);
    for i in 0..APPEARANCE_TOTAL_TRAITS {
        if random[i % 32] < chance {
            let current = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
            let evolved = evolve_single_trait(current, stage, random[(i + 16) % 32], APPEARANCE_MAX);
            set_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8, evolved);
        }
    }
}

fn evolve_power_traits(dna: &mut [u8; 32], stage: u8, random: &[u8; 32]) {
    let chance = evolution_chance(stage);
    for i in 0..COMBAT_TOTAL_TRAITS {
        if random[(i + 8) % 32] < chance {
            let current = get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
            let evolved = evolve_single_trait(current, stage, random[(i + 24) % 32], COMBAT_MAX);
            set_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, evolved);
        }
    }
}

fn evolution_chance(stage: u8) -> u8 {
    match stage { 0 => 50, 1 => 70, 2 => 90, 3 => 110, 4 => 90, 5 => 70, 6 => 50, _ => 30 }
}

fn evolve_single_trait(current: u8, stage: u8, rand: u8, max_cap: u8) -> u8 {
    let max_for_stage = if stage == EVOLUTION_STAGES - 1 { max_cap } else { (max_cap * 3 / 4) + stage };
    if current >= max_for_stage { return current; }

    if rand < 192 { (current + 1).min(max_for_stage) }
    else if rand < 224 && current + 2 <= max_for_stage { current + 2 }
    else if rand < 244 && current > 0 { current - 1 }
    else if rand < 248 && current + 3 <= max_for_stage { current + 3 }
    else { current }
}

// ========================================================================================
// ============================= DNA DECODER FUNCTIONS ====================================
// ========================================================================================

/// Get faction/family type from DNA (first 4 bits)
pub fn get_family_type(dna: &[u8; 32]) -> u8 { dna[0] & 0x0F }

/// Get evolution stage from DNA (bits 4-6)
pub fn get_evolution_stage(dna: &[u8; 32]) -> u8 { (dna[0] >> 4) & 0x07 }

/// Decode all 21 appearance traits (7 groups × 3)
pub fn decode_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
    (0..APPEARANCE_TOTAL_TRAITS).map(|i| get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8)).collect()
}

/// Decode dominant appearance traits (7 traits - first from each group)
pub fn decode_dominant_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
    (0..APPEARANCE_GROUPS).map(|i| get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, (i * APPEARANCE_TRAITS_PER_GROUP) as u8)).collect()
}

/// Decode all 15 power traits (5 groups × 3)
pub fn decode_power_traits(dna: &[u8; 32]) -> Vec<u8> {
    (0..COMBAT_TOTAL_TRAITS).map(|i| get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8)).collect()
}

/// Decode dominant power traits (5 traits - first from each group)
pub fn decode_dominant_power_traits(dna: &[u8; 32]) -> Vec<u8> {
    (0..COMBAT_GROUPS).map(|i| get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, (i * COMBAT_TRAITS_PER_GROUP) as u8)).collect()
}

// ========================================================================================
// ============================= BIT MANIPULATION HELPERS =================================
// ========================================================================================

fn get_trait_value(dna: &[u8; 32], base_offset: u8, trait_bits: u8, index: u8) -> u8 {
    let start_bit = base_offset as usize + (index as usize * trait_bits as usize);
    let byte_idx = start_bit / 8;
    let bit_idx = start_bit % 8;
    if byte_idx >= 32 { return 0; }

    let mut val = 0u8;
    let mut remaining = trait_bits as usize;
    let mut curr_byte = byte_idx;
    let mut curr_bit = bit_idx;

    while remaining > 0 && curr_byte < 32 {
        let bits_in_byte = 8 - curr_bit;
        let take = remaining.min(bits_in_byte);
        let mask = ((1u8 << take) - 1) << curr_bit;
        let bits = (dna[curr_byte] & mask) >> curr_bit;
        val |= bits << (trait_bits as usize - remaining);
        remaining -= take;
        curr_byte += 1;
        curr_bit = 0;
    }
    val
}

fn set_trait_value(dna: &mut [u8; 32], base_offset: u8, trait_bits: u8, index: u8, value: u8) {
    let start_bit = base_offset as usize + (index as usize * trait_bits as usize);
    let byte_idx = start_bit / 8;
    let bit_idx = start_bit % 8;
    if byte_idx >= 32 { return; }

    let mut remaining = trait_bits as usize;
    let mut curr_byte = byte_idx;
    let mut curr_bit = bit_idx;
    let mut val_processed = 0;

    while remaining > 0 && curr_byte < 32 {
        let bits_in_byte = 8 - curr_bit;
        let set = remaining.min(bits_in_byte);
        let mask = ((1u8 << set) - 1) << curr_bit;
        let chunk = (value >> val_processed) & ((1u8 << set) - 1);
        dna[curr_byte] = (dna[curr_byte] & !mask) | (chunk << curr_bit);
        remaining -= set;
        val_processed += set;
        curr_byte += 1;
        curr_bit = 0;
    }
}
