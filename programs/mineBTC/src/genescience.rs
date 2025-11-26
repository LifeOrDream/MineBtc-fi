use crate::errors::ErrorCode;
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
