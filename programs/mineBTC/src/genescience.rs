use crate::errors::ErrorCode;
use crate::state::{BASE_MULTIPLIER, MAX_BASE_CHANCE};

/// DNA Generation and Manipulation for Cyber-Doge Assets
///
/// The core genetic engine for the Bio-Doge Invasion.
/// Based on a 256-bit DNA structure that seeds the AI generation pipeline.
///
/// DNA STRUCTURE (256 bits total):
///
/// 1. Faction/Family (4 bits): Maps 1:1 to Faction ID (0-11). Determines base loyalty & breed.
/// 2. Cyber-Evolution Stage (3 bits): Current upgrade level (0-7).
/// 3. Appearance Genes (140 bits): 7 groups × 4 traits × 5 bits.
///    - Seeds the AI generation for visual traits (Fur, Armor, Cybernetics, Eyes).
/// 4. Combat Genes (84 bits): 7 groups × 3 traits × 4 bits.
///    - Determines on-chain stats (Hashpower Efficiency, Raid Attack, Defense).
/// 5. Reserved (25 bits): Future mutations.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

// --- GENE MAPPING CONSTANTS ---
const FACTION_TYPE_BITS: u8 = 4; // First 4 bits = Faction ID
const EVOLUTION_STAGE_BITS: u8 = 3; // Next 3 bits = Level (0-7)
const APPEARANCE_TRAIT_BITS: u8 = 5;
const COMBAT_TRAIT_BITS: u8 = 4;

const APPEARANCE_OFFSET: u8 = FACTION_TYPE_BITS + EVOLUTION_STAGE_BITS;
const COMBAT_OFFSET: u8 = APPEARANCE_OFFSET + (28 * APPEARANCE_TRAIT_BITS);

const APPEARANCE_GROUPS: usize = 7;
const APPEARANCE_PER_GROUP: usize = 4;
const COMBAT_GROUPS: usize = 7;
const COMBAT_PER_GROUP: usize = 3;

/// Calculate dynamic pricing for Genetic Assets (Bonding Curve)
/// Formula: price = base_price + curve_a * (minted^(2/3))
/// Creates a "FOMO Curve" where early adopters get cheaper genes.
pub fn compute_gene_price(base_price: u64, curve_a: u64, items_minted: u64) -> Result<u64> {
    if items_minted == 0 {
        return Ok(base_price);
    }

    // Calculate x^(2/3) approximation
    let items_u128 = items_minted as u128;
    let squared = items_u128
        .checked_mul(items_u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Binary search for cube root of x^2
    let mut low: u128 = 1;
    let mut high = squared.min(1_000_000_000_000_000_000);
    let mut result: u128 = 0;

    while low <= high {
        let mid = (low + high) / 2;
        let cube = mid
            .checked_mul(mid)
            .and_then(|x| x.checked_mul(mid))
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        if cube <= squared {
            result = mid;
            low = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            high = mid - 1;
        }
    }

    let exponent_component = result.min(u64::MAX as u128) as u64;
    let price_increase = curve_a
        .checked_mul(exponent_component)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let final_price = base_price
        .checked_add(price_increase)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    Ok(final_price)
}

/// Generate unique DNA for a Genesis Cyber-Doge
///
/// # Arguments
/// * `faction_id` - The Faction (0-11) this Doge belongs to.
///                  This is hard-coded into the first 4 bits of DNA.
pub fn generate_genesis_dna(
    mint_number: u64,
    minter: &Pubkey,
    slot: u64,
    faction_id: u8,
) -> Result<[u8; 32]> {
    // Ensure faction ID fits in 4 bits (0-15)
    require!(faction_id < 16, ErrorCode::InvalidFactionId);

    // 1. Seed Generation (Deterministic but unpredictable)
    let mut seed_data = Vec::new();
    seed_data.extend_from_slice(&mint_number.to_le_bytes());
    seed_data.extend_from_slice(&minter.to_bytes());
    seed_data.extend_from_slice(&slot.to_le_bytes());

    // 2. Hash for Randomness
    let hash = keccak::hash(&seed_data);
    let mut dna = hash.to_bytes();

    // 3. Encode Faction (Family) - First 4 bits
    // Clears first 4 bits, sets them to faction_id
    dna[0] = (dna[0] & 0xF0) | (faction_id & 0x0F);

    // 4. Initialize Evolution to Stage 0 - Next 3 bits
    // Clears bits 4-6
    dna[0] = dna[0] & 0x8F;

    // 5. Normalize Trait Ranges
    // Ensures generated values map correctly to our AI model's trait tables
    limit_trait_ranges(&mut dna);

    Ok(dna)
}

// --- HELPER FUNCTIONS (Bit Manipulation) ---

fn limit_trait_ranges(dna: &mut [u8; 32]) {
    // Appearance: 0-15 range
    for i in 0..(APPEARANCE_GROUPS * APPEARANCE_PER_GROUP) {
        let val = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        set_trait_value(
            dna,
            APPEARANCE_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
            val % 16,
        );
    }
    // Combat: 0-7 range
    for i in 0..(COMBAT_GROUPS * COMBAT_PER_GROUP) {
        let val = get_trait_value(dna, COMBAT_OFFSET, COMBAT_TRAIT_BITS, i as u8);
        set_trait_value(dna, COMBAT_OFFSET, COMBAT_TRAIT_BITS, i as u8, val % 8);
    }
}

fn get_trait_value(dna: &[u8; 32], base_offset: u8, trait_bits: u8, index: u8) -> u8 {
    let start_bit = base_offset + (index * trait_bits);
    let byte_idx = (start_bit / 8) as usize;
    let bit_idx = start_bit % 8;

    if byte_idx >= 32 {
        return 0;
    }

    // Logic to read bits across byte boundaries
    let mut val = 0u8;
    let mut remaining = trait_bits;
    let mut curr_byte = byte_idx;
    let mut curr_bit = bit_idx;

    while remaining > 0 && curr_byte < 32 {
        let bits_in_byte = 8 - curr_bit;
        let take = remaining.min(bits_in_byte);
        let mask = ((1u8 << take) - 1) << curr_bit;
        let bits = (dna[curr_byte] & mask) >> curr_bit;

        val |= bits << (trait_bits - remaining);

        remaining -= take;
        curr_byte += 1;
        curr_bit = 0;
    }
    val
}

fn set_trait_value(dna: &mut [u8; 32], base_offset: u8, trait_bits: u8, index: u8, value: u8) {
    let start_bit = base_offset + (index * trait_bits);
    let byte_idx = (start_bit / 8) as usize;
    let bit_idx = start_bit % 8;

    if byte_idx >= 32 {
        return;
    }

    let mut remaining = trait_bits;
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

// ========================================================================================
// ============================= INSTANT MUTATION SYSTEM ==================================
// ========================================================================================

/// Mutation types for instant mutation mechanic
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MutationType {
    /// Evolution: generation += 1, xp = 0 (~10%)
    Evolution,
    /// Power: multiplier += 25 (0.25x) (~30%)
    Power,
    /// Trait: reroll a visual gene in dna (~60%)
    Trait,
}

/// Result of mutation attempt - includes XP and multiplier gains
#[derive(Clone, Debug)]
pub struct MutationResult {
    /// Mutation type (None if no mutation triggered)
    pub mutation_type: Option<MutationType>,
    /// XP gained from betting (always > 0 for SOL bets)
    pub xp_gained: u32,
    /// Multiplier increase (0 if no Power mutation or no mutation)
    pub multiplier_increase: u32,
}

/// Calculate mutation result based on bet and game state.
/// 
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
    gameplay_egg_dna: [u8; 32],
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

    // A. Bet Strength (0 - 10,000 bps) -  If highest is 0 (first bet), strength is 100%. Otherwise ratio.
    let effective_highest = if highest_round_bet_for_faction == 0 { user_total_bet } else { highest_round_bet_for_faction };
    let bet_strength = ((user_total_bet * 10000) / effective_highest).min(10000);
    msg!("   Bet strength: {}", bet_strength as f64 / 10000.0);

    // B. Multiplier Penalty (0 - 10,000 bps)
    // Formula: (BASE_MULTIPLIER / Current) * 10,000
    // 1.0x -> 1k/1k * 1k = 1000 (100% factor)
    // 6.9x -> 1k/6900 * 1k = 144.9 (14.5% factor)
    let mult_factor = (BASE_MULTIPLIER   * 10000) / current_multiplier;
    msg!("   Multiplier factor: {}", mult_factor);

    // C. Final Chance Calculation
    // Chance = 30% * BetFactor * MultFactor
    // scale: 3000 * (0.0-1.0) * (0.14-1.0)
    let final_chance_bps = (MAX_BASE_CHANCE * bet_strength * (mult_factor as u64)) 
        / 100_000_000; // Divide by 10k * 10k
    msg!("   Final chance: {}", final_chance_bps as f64 / 100.0);

    // --- STEP 2: ROLL THE DICE ---

    // Calculate the sums first to get the values
    let total_combined_points = total_points_bets + total_wgtd_points_bets;

    // Use hashv which takes a slice of byte slices directly
    let seed = keccak::hashv(&[
        &slot.to_le_bytes(),
        &total_sol_bets.to_le_bytes(),
        &total_combined_points.to_le_bytes(),
    ]).to_bytes();

    // Roll 1: Did we hit the mutation? (0-10000)
    // Use first 2 bytes for higher precision (u16)
    let roll_val = u16::from_le_bytes([seed[0], seed[1]]) as u64;
    let roll_normalized = (roll_val * 10_000) / 65535;

    if roll_normalized < final_chance_bps as u64 {
        // SUCCESS! Mutation triggered. Now decide TYPE.
        // Use 3rd byte for type roll (0-100)
        let type_roll = seed[2]; // 0-255
        
        // D. Determine Type
        // Gen 0: Evo(10%), Power(30%), Trait(60%)
        // Gen 9: Evo(1%), Power(30%), Trait(69%)
        let evo_chance = 10 / (current_generation as u64 + 1);
        let power_chance = 30;
        
        let evo_threshold = (255 * evo_chance) / 100;
        let power_threshold = evo_threshold + ((255 * power_chance) / 100);

        let mut base_boost = 0;
        let m_type = if (type_roll as u64) < evo_threshold {
            MutationType::Evolution
        } else if (type_roll as u64) < power_threshold {
            MutationType::Power
        } else {
            MutationType::Trait
        };

        match m_type {
            MutationType::Evolution => {
                // 1. Increment Generation in DNA
                let new_gen = gameplay_egg_generation.saturating_add(1);
                base_boost = 50; // +0.50x Base for Evolving
            },
            MutationType::Power => {
                let new_gen = gameplay_egg_generation.saturating_add(1);
                base_boost = 25; // +0.25x Base for Power
            },
            MutationType::Trait => {
                let new_gen = gameplay_egg_generation.saturating_add(1);
                base_boost = 5; // +0.05x Base for Trait
            }
        }        

        // E. Calculate XP Bonus (The "Investment" Logic)
        // We use a % of current XP to boost the multiplier.
        // Evolution: Uses 50-100% of XP efficiency.
        // Others: Uses 10-50% of XP efficiency.
        let xp_roll = seed[3] as u64; // 0-255

        let (min_pct, max_pct) = if m_type == MutationType::Evolution { (50, 100) } else { (10, 50) };
        let efficiency_pct = min_pct + ((xp_roll * (max_pct - min_pct)) / 255);        

        // XP Boost Formula: (XP * Efficiency%) / 100 
        // Example: 1000 XP * 80% efficiency = +800 basis points (+0.08x)
        // We scale this down slightly so 1000 XP isn't too OP. Let's say 10 XP = 1 basis point.
        let xp_mult_boost: u64 = ((gameplay_egg_xp as u64 * efficiency_pct) / 100) / 10;
        msg!("   XP boost: {}", xp_mult_boost);

        let total_boost = base_boost + xp_mult_boost as u32;
        let new_multiplier = current_multiplier + total_boost;        
        msg!("   New multiplier: {}", new_multiplier);
    }

    MutationResult {
        mutation_type: Some(mutation_type),
        xp_gained,
        multiplier_increase,
    }
}


/// Apply mutation to egg metadata directly
/// Note: Power mutation multiplier is applied separately via UserGameBet.multiplier_increase
pub fn apply_mutation(
    egg: &mut Account<crate::state::EggMetadata>,
    mutation_type: MutationType,
    slot: u64,
    user_key: &Pubkey,
) {
    match mutation_type {
        MutationType::Evolution => {
            // Increment generation (cap at 7), reset xp
            if egg.generation < 7 {
                egg.generation += 1;
            }
            egg.xp = 0;
            // Update evolution stage in DNA (bits 4-6)
            egg.dna[0] = (egg.dna[0] & 0x8F) | ((egg.generation & 0x07) << 4);
        }
        MutationType::Power => {
            // Multiplier increase is handled via UserGameBet.multiplier_increase
            // and applied to PlayerData.active_multiplier in claim_rewards
            // No action needed here - just marks the mutation type
        }
        MutationType::Trait => {
            // Reroll a random visual gene
            let mut seed_data = Vec::with_capacity(40);
            seed_data.extend_from_slice(&slot.to_le_bytes());
            seed_data.extend_from_slice(&user_key.to_bytes());
            let hash = keccak::hash(&seed_data);
            let bytes = hash.to_bytes();

            // Pick random appearance trait (0-27)
            let trait_index = bytes[0] % 28;
            let new_value = bytes[1] % 16; // Appearance traits 0-15
            set_trait_value(&mut egg.dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, trait_index, new_value);
        }
    }
}

// /// Get family/type from DNA (first 4 bits)
// pub fn get_family_type(dna: &[u8; 32]) -> u8 {
//     dna[0] & 0x0F
// }

// /// Get evolutionary stage from DNA (bits 4-6)
// pub fn get_evolutionary_stage(dna: &[u8; 32]) -> u8 {
//     (dna[0] >> 4) & 0x07
// }

// /// Set evolutionary stage in DNA
// pub fn set_evolutionary_stage(dna: &mut [u8; 32], stage: u8) -> Result<()> {
//     require!(stage < 8, crate::errors::ErrorCode::InvalidParameters);
//     dna[0] = (dna[0] & 0x8F) | ((stage & 0x07) << 4);
//     Ok(())
// }

// /// Get all appearance traits (28 traits)
// pub fn get_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
//     let mut traits = Vec::with_capacity(28);
//     for i in 0..(APPEARANCE_TRAIT_GROUPS * APPEARANCE_TRAITS_PER_GROUP) {
//         traits.push(get_trait_value(
//             dna,
//             APPEARANCE_TRAITS_OFFSET,
//             APPEARANCE_TRAIT_BITS,
//             i as u8,
//         ));
//     }
//     traits
// }

// /// Get dominant appearance traits (7 traits - first from each group)
// pub fn get_dominant_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
//     let mut traits = Vec::with_capacity(7);
//     for i in 0..APPEARANCE_TRAIT_GROUPS {
//         let trait_index = (i * APPEARANCE_TRAITS_PER_GROUP) as u8;
//         traits.push(get_trait_value(
//             dna,
//             APPEARANCE_TRAITS_OFFSET,
//             APPEARANCE_TRAIT_BITS,
//             trait_index,
//         ));
//     }
//     traits
// }

// /// Get all power traits (21 traits)
// pub fn get_power_traits(dna: &[u8; 32]) -> Vec<u8> {
//     let mut traits = Vec::with_capacity(21);
//     for i in 0..(POWER_TRAIT_GROUPS * POWER_TRAITS_PER_GROUP) {
//         traits.push(get_trait_value(
//             dna,
//             POWER_TRAITS_OFFSET,
//             POWER_TRAIT_BITS,
//             i as u8,
//         ));
//     }
//     traits
// }

// /// Get dominant power traits (7 traits - first from each group)
// pub fn get_dominant_power_traits(dna: &[u8; 32]) -> Vec<u8> {
//     let mut traits = Vec::with_capacity(7);
//     for i in 0..POWER_TRAIT_GROUPS {
//         let trait_index = (i * POWER_TRAITS_PER_GROUP) as u8;
//         traits.push(get_trait_value(
//             dna,
//             POWER_TRAITS_OFFSET,
//             POWER_TRAIT_BITS,
//             trait_index,
//         ));
//     }
//     traits
// }

// /// Evolve DNA to next stage (increases evolutionary stage and may improve traits)
// pub fn evolve_dna(dna: &mut [u8; 32], random_seed: &[u8]) -> Result<()> {
//     let current_stage = get_evolutionary_stage(dna);
//     require!(
//         current_stage < 7,
//         crate::errors::ErrorCode::InvalidParameters
//     );

//     let new_stage = current_stage + 1;
//     set_evolutionary_stage(dna, new_stage)?;

//     // Use random seed to probabilistically improve traits
//     let hash = keccak::hash(random_seed);
//     let random_bytes = hash.to_bytes();

//     // Evolve appearance traits with ~30% chance each
//     for i in 0..(APPEARANCE_TRAIT_GROUPS * APPEARANCE_TRAITS_PER_GROUP) {
//         if random_bytes[i % 32] < 77 {
//             // 77/256 ≈ 30%
//             let current = get_trait_value(
//                 dna,
//                 APPEARANCE_TRAITS_OFFSET,
//                 APPEARANCE_TRAIT_BITS,
//                 i as u8,
//             );
//             let max_for_stage = 15 + new_stage; // Increases with evolution
//             if current < max_for_stage {
//                 set_trait_value(
//                     dna,
//                     APPEARANCE_TRAITS_OFFSET,
//                     APPEARANCE_TRAIT_BITS,
//                     i as u8,
//                     current + 1,
//                 );
//             }
//         }
//     }

//     // Evolve power traits with ~30% chance each
//     for i in 0..(POWER_TRAIT_GROUPS * POWER_TRAITS_PER_GROUP) {
//         let rand_index = (28 + i) % 32;
//         if random_bytes[rand_index] < 77 {
//             // 77/256 ≈ 30%
//             let current = get_trait_value(dna, POWER_TRAITS_OFFSET, POWER_TRAIT_BITS, i as u8);
//             let max_for_stage = 7 + new_stage; // Increases with evolution
//             if current < max_for_stage {
//                 set_trait_value(
//                     dna,
//                     POWER_TRAITS_OFFSET,
//                     POWER_TRAIT_BITS,
//                     i as u8,
//                     current + 1,
//                 );
//             }
//         }
//     }

//     Ok(())
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_dna_generation() {
//         let mint_number = 1;
//         let minter = Pubkey::new_unique();
//         let slot = 12345;
//         let family = 3;

//         let dna = generate_genesis_dna(mint_number, &minter, slot, family).unwrap();

//         // Check family type
//         assert_eq!(get_family_type(&dna), family);

//         // Check evolution stage is 0
//         assert_eq!(get_evolutionary_stage(&dna), 0);

//         // Check traits are in valid range
//         let appearance = get_appearance_traits(&dna);
//         assert_eq!(appearance.len(), 28);
//         for trait_val in appearance {
//             assert!(
//                 trait_val <= 15,
//                 "Appearance trait {} should be <= 15",
//                 trait_val
//             );
//         }

//         let power = get_power_traits(&dna);
//         assert_eq!(power.len(), 21);
//         for trait_val in power {
//             assert!(trait_val <= 7, "Power trait {} should be <= 7", trait_val);
//         }
//     }

//     #[test]
//     fn test_evolution() {
//         let mint_number = 1;
//         let minter = Pubkey::new_unique();
//         let slot = 12345;
//         let family = 5;

//         let mut dna = generate_genesis_dna(mint_number, &minter, slot, family).unwrap();

//         let seed = b"evolution_seed_1";
//         evolve_dna(&mut dna, seed).unwrap();

//         assert_eq!(get_evolutionary_stage(&dna), 1);
//     }
// }
