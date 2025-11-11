/// DNA Generation and Manipulation for Dragon Eggs
///
/// Based on a 256-bit DNA structure that determines characteristics:
///
/// DNA STRUCTURE (256 bits total):
///
/// 1. Family/Type (4 bits): Dragon family (0-15), determines base appearance
/// 2. Evolutionary Stage (3 bits): Current evolution level (0-7)
/// 3. Appearance Traits (140 bits): 7 groups × 4 traits × 5 bits
///    - Each group: [Dominant][Recessive1][Recessive2][Recessive3]
///    - Examples: Wings, Horns, Eyes, Claws, Scales, Color, Pattern
/// 4. Power Traits (84 bits): 7 groups × 3 traits × 4 bits
///    - Examples: Health, Energy, Attack, Defense, Special, Ultimate, Passive
/// 5. Reserved (25 bits): For future use
use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

#[allow(dead_code)] // Functions will be used when NFT minting is enabled

// DNA field offsets
const FAMILY_TYPE_BITS: u8 = 4;
const EVOLUTIONARY_STAGE_BITS: u8 = 3;
const APPEARANCE_TRAIT_BITS: u8 = 5;
const POWER_TRAIT_BITS: u8 = 4;

const APPEARANCE_TRAITS_OFFSET: u8 = FAMILY_TYPE_BITS + EVOLUTIONARY_STAGE_BITS;
const POWER_TRAITS_OFFSET: u8 = APPEARANCE_TRAITS_OFFSET + (28 * APPEARANCE_TRAIT_BITS); // 7 groups × 4 traits

// Constants
const APPEARANCE_TRAIT_GROUPS: usize = 7;
const APPEARANCE_TRAITS_PER_GROUP: usize = 4;
const POWER_TRAIT_GROUPS: usize = 7;
const POWER_TRAITS_PER_GROUP: usize = 3;

/// Generate unique DNA for a genesis Dragon Egg
///
/// # Arguments
/// * `mint_number` - The sequential mint number of the egg
/// * `minter` - Address of the person minting the egg
/// * `slot` - Current slot for additional entropy
/// * `family_type` - Dragon family (0-15)
///
/// # Returns
/// * 32-byte array representing 256-bit DNA
pub fn generate_genesis_dna(
    mint_number: u64,
    minter: &Pubkey,
    slot: u64,
    family_type: u8,
) -> Result<[u8; 32]> {
    require!(
        family_type < 16,
        crate::errors::ErrorCode::InvalidParameters
    );

    // Create seed for randomness
    let mut seed_data = Vec::new();
    seed_data.extend_from_slice(&mint_number.to_le_bytes());
    seed_data.extend_from_slice(&minter.to_bytes());
    seed_data.extend_from_slice(&slot.to_le_bytes());

    // Hash to create base randomness
    let hash = keccak::hash(&seed_data);
    let mut dna = hash.to_bytes();

    // Set family type (first 4 bits)
    dna[0] = (dna[0] & 0xF0) | (family_type & 0x0F);

    // Set evolutionary stage to 0 (next 3 bits)
    dna[0] = dna[0] & 0x8F; // Clear evolution bits (bits 4-6)

    // Limit trait values to lower half of possible range
    // For appearance traits (5 bits, max 31): limit to 0-15
    // For power traits (4 bits, max 15): limit to 0-7
    limit_trait_ranges(&mut dna);

    Ok(dna)
}

/// Generate DNA with progressive gene windows based on tier (family)
/// Each tier gets progressively better gene value ranges
///
/// # Arguments
/// * `mint_number` - The sequential mint number
/// * `minter` - Address of minter
/// * `slot` - Current slot
/// * `gene_tier` - Tier/family (0-15)
///
/// # Returns
/// * 32-byte array representing 256-bit DNA with tier-based gene windows
pub fn generate_genesis_dna_with_tier(
    mint_number: u64,
    minter: &Pubkey,
    slot: u64,
    gene_tier: u8,
) -> Result<[u8; 32]> {
    require!(gene_tier < 16, crate::errors::ErrorCode::InvalidParameters);

    // Create seed for randomness
    let mut seed_data = Vec::new();
    seed_data.extend_from_slice(&mint_number.to_le_bytes());
    seed_data.extend_from_slice(&minter.to_bytes());
    seed_data.extend_from_slice(&slot.to_le_bytes());

    let hash = keccak::hash(&seed_data);
    let hash_bytes = hash.to_bytes();

    let mut dna = [0u8; 32];

    // Set family type (first 4 bits)
    dna[0] = gene_tier & 0x0F;

    // Set evolutionary stage to 0 (bits 4-6)
    // Already 0 from initialization

    // Generate appearance traits with progressive windows
    // Tier 0: [0,3], Tier 1: [0,4], Tier 2: [2,5], ..., Tier 15: [13,18]
    let appearance_min = if gene_tier >= 2 { gene_tier - 2 } else { 0 };
    let appearance_max = 3 + gene_tier;

    for i in 0..(APPEARANCE_TRAIT_GROUPS * APPEARANCE_TRAITS_PER_GROUP) {
        let hash_index = (i * 3) % 32;
        let random_byte = hash_bytes[hash_index];
        let range = appearance_max - appearance_min + 1;
        let trait_value = appearance_min + (random_byte % range);
        let trait_value = trait_value.min(15); // Cap at half of max (31/2 ≈ 15)

        set_trait_value(
            &mut dna,
            APPEARANCE_TRAITS_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
            trait_value,
        );
    }

    // Generate power traits with progressive windows
    // Tier 0: [0,2], Tier 1: [1,3], Tier 2: [2,4], ..., Tier 15: [14,16]
    let power_min = if gene_tier >= 1 { gene_tier - 1 } else { 0 };
    let power_max = 2 + gene_tier;

    for i in 0..(POWER_TRAIT_GROUPS * POWER_TRAITS_PER_GROUP) {
        let hash_index = ((28 + i) * 3) % 32;
        let random_byte = hash_bytes[hash_index];
        let range = power_max - power_min + 1;
        let trait_value = power_min + (random_byte % range);
        let trait_value = trait_value.min(7); // Cap at half of max (15/2 ≈ 7)

        set_trait_value(
            &mut dna,
            POWER_TRAITS_OFFSET,
            POWER_TRAIT_BITS,
            i as u8,
            trait_value,
        );
    }

    Ok(dna)
}

/// Limit trait ranges to lower half of possible values
/// Appearance traits: 0-15 (instead of 0-31)
/// Power traits: 0-7 (instead of 0-15)
fn limit_trait_ranges(dna: &mut [u8; 32]) {
    // Limit appearance traits (28 traits, 5 bits each)
    for i in 0..(APPEARANCE_TRAIT_GROUPS * APPEARANCE_TRAITS_PER_GROUP) {
        let trait_val = get_trait_value(
            dna,
            APPEARANCE_TRAITS_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
        );
        let limited_val = trait_val % 16; // 0-15
        set_trait_value(
            dna,
            APPEARANCE_TRAITS_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
            limited_val,
        );
    }

    // Limit power traits (21 traits, 4 bits each)
    for i in 0..(POWER_TRAIT_GROUPS * POWER_TRAITS_PER_GROUP) {
        let trait_val = get_trait_value(dna, POWER_TRAITS_OFFSET, POWER_TRAIT_BITS, i as u8);
        let limited_val = trait_val % 8; // 0-7
        set_trait_value(
            dna,
            POWER_TRAITS_OFFSET,
            POWER_TRAIT_BITS,
            i as u8,
            limited_val,
        );
    }
}

/// Extract a specific bit range from DNA
fn get_trait_value(dna: &[u8; 32], base_offset: u8, trait_bits: u8, trait_index: u8) -> u8 {
    let bit_offset = base_offset + (trait_index * trait_bits);
    let byte_index = (bit_offset / 8) as usize;
    let bit_in_byte = bit_offset % 8;

    if byte_index >= 32 {
        return 0;
    }

    let mut value = 0u8;
    let mut bits_remaining = trait_bits;
    let mut current_byte_index = byte_index;
    let mut current_bit_offset = bit_in_byte;

    while bits_remaining > 0 && current_byte_index < 32 {
        let bits_in_this_byte = 8 - current_bit_offset;
        let bits_to_take = bits_remaining.min(bits_in_this_byte);

        let mask = ((1u8 << bits_to_take) - 1) << current_bit_offset;
        let bits = (dna[current_byte_index] & mask) >> current_bit_offset;

        value |= bits << (trait_bits - bits_remaining);

        bits_remaining -= bits_to_take;
        current_byte_index += 1;
        current_bit_offset = 0;
    }

    value
}

/// Set a specific bit range in DNA
fn set_trait_value(
    dna: &mut [u8; 32],
    base_offset: u8,
    trait_bits: u8,
    trait_index: u8,
    value: u8,
) {
    let bit_offset = base_offset + (trait_index * trait_bits);
    let byte_index = (bit_offset / 8) as usize;
    let bit_in_byte = bit_offset % 8;

    if byte_index >= 32 {
        return;
    }

    let mut bits_remaining = trait_bits;
    let mut current_byte_index = byte_index;
    let mut current_bit_offset = bit_in_byte;
    let mut value_bits_used = 0u8;

    while bits_remaining > 0 && current_byte_index < 32 {
        let bits_in_this_byte = 8 - current_bit_offset;
        let bits_to_set = bits_remaining.min(bits_in_this_byte);

        let mask = ((1u8 << bits_to_set) - 1) << current_bit_offset;
        let value_bits = (value >> value_bits_used) & ((1u8 << bits_to_set) - 1);

        dna[current_byte_index] =
            (dna[current_byte_index] & !mask) | (value_bits << current_bit_offset);

        bits_remaining -= bits_to_set;
        value_bits_used += bits_to_set;
        current_byte_index += 1;
        current_bit_offset = 0;
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
