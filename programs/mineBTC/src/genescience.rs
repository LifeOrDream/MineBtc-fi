use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

use crate::errors::ErrorCode;
use crate::events::{DogeEvolution, DogePowerMutation, DogeVisualMutation};
use crate::state::{
    GameplayTuningConfig, BASE_MULTIPLIER, BASIS_POINTS_DENOMINATOR, FACTION_MUTATION_PENALTY_STEP,
};

// ========================================================================================
// ============================= DNA STRUCTURE & CONSTANTS ================================
// ========================================================================================
//
/// DNA Generation and Manipulation for Cyber-Doge Assets
///
/// DNA STRUCTURE (256 bits / 32 bytes):
/// 1. Faction/Family (4 bits, offset 0): Maps to Faction ID (0-11)
/// 2. Evolution Stage (3 bits, offset 4): Current level (0-7)
/// 3. Appearance Genes (105 bits, offset 7): 7 groups × 3 traits × 5 bits (0-31)
///    - Per group: [Dominant][Recessive][Minor Recessive]
/// 4. Combat Genes (60 bits, offset 112): 5 groups × 3 traits × 4 bits (0-15)
///    - Per group: [Dominant][Recessive][Minor Recessive]
/// 5. Breed/Body Type (2 bits, offset 172): Dog breed per faction (0-3). IMMUTABLE.
/// 6. Reserved (82 bits): Future use
///
/// IMMUTABLE FIELDS (never change after creation):
/// - Faction (inherited from parent faction or chosen at genesis)
/// - Breed (random at genesis, inherited from one parent during breeding)
const FACTION_TYPE_BITS: u8 = 4;
const EVOLUTION_STAGE_BITS: u8 = 3;
const APPEARANCE_TRAIT_BITS: u8 = 5; // 0-31 values
const POWER_TRAIT_BITS: u8 = 4; // 0-15 values
const BREED_BITS: u8 = 2; // 0-3 breed values per faction

const APPEARANCE_OFFSET: u8 = FACTION_TYPE_BITS + EVOLUTION_STAGE_BITS; // 7
const COMBAT_OFFSET: u8 = APPEARANCE_OFFSET + (21 * APPEARANCE_TRAIT_BITS); // 7 + 105 = 112
const BREED_OFFSET: u8 = COMBAT_OFFSET + (15 * POWER_TRAIT_BITS); // 112 + 60 = 172 (in reserved area)

// const APPEARANCE_GROUPS: usize = 7;
// const APPEARANCE_TRAITS_PER_GROUP: usize = 3;  // Dominant, Recessive, Minor Recessive
const APPEARANCE_TOTAL_TRAITS: usize = 21; // 7 × 3

// const COMBAT_GROUPS: usize = 5;
// const COMBAT_TRAITS_PER_GROUP: usize = 3;
const COMBAT_TOTAL_TRAITS: usize = 15; // 5 × 3

// const EVOLUTION_STAGES: u8 = 8;
const APPEARANCE_MAX: u8 = 31;
const COMBAT_MAX: u8 = 15;

// ========================================================================================
// ============================= PRICE CALCULATION ========================================
// ========================================================================================

/// Calculate dynamic pricing for Genetic Assets (Bonding Curve)
pub fn compute_gene_price(base_price: u64, curve_a: u64, items_minted: u64) -> Result<u64> {
    crate::log_fn!("genescience", "compute_gene_price");
    if items_minted == 0 {
        return Ok(base_price);
    }

    let items_u128 = items_minted as u128;
    let squared = items_u128
        .checked_mul(items_u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

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
    base_price
        .checked_add(price_increase)
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

// ========================================================================================
// ============================= DNA GENERATION ===========================================
// ========================================================================================

/// Generate unique DNA for a Genesis Cyber-Doge
/// Genesis constraints:
/// - All appearance traits: 0-9 (less than 10)
/// - First 3 power groups (9 traits): 0-4 (less than 5)
/// - Last 2 power groups (6 traits): 0 (locked at zero)
pub fn generate_genesis_dna(
    mint_number: u64,
    minter: &Pubkey,
    slot: u64,
    faction_id: u8,
) -> Result<[u8; 32]> {
    crate::log_fn!("genescience", "generate_genesis_dna");
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
    dna[0] &= 0x8F;

    limit_genesis_trait_ranges(&mut dna);

    // Encode Breed (2 bits at BREED_OFFSET, value 0-3 from hash randomness)
    let breed_val = (hash.to_bytes()[31] & 0x03) as u8; // Use last byte for breed randomness
    set_trait_value(&mut dna, BREED_OFFSET, BREED_BITS, 0, breed_val);

    Ok(dna)
}

/// Limit trait ranges for genesis DNA (weaker initial stats)
fn limit_genesis_trait_ranges(dna: &mut [u8; 32]) {
    // Appearance traits: 0-9 (less than 10)
    for i in 0..APPEARANCE_TOTAL_TRAITS {
        let val = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        set_trait_value(
            dna,
            APPEARANCE_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
            val % 10,
        );
    }

    // Power traits:
    // - First 3 groups (9 traits, indices 0-8): 0-4 (less than 5)
    // - Last 2 groups (6 traits, indices 9-14): 0 (locked)
    for i in 0..COMBAT_TOTAL_TRAITS {
        let val = if i < 9 {
            // First 3 groups: 0-4
            get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8) % 5
        } else {
            // Last 2 groups: locked at 0
            0
        };
        set_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, val);
    }
}

// ========================================================================================
// ============================= STORY EVENT / DNA MUTATION SYSTEM =========================
// ========================================================================================
//
// Product terminology: callers and indexers should treat these outcomes as
// story events. The contract still mutates DNA / multiplier / XP for specific
// event types because that is the compact on-chain state transition that powers
// progression. The backend is free to turn a story event into artwork, a reel,
// a character-history update, or no visual metadata change at all.

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MutationType {
    /// Evolution: generation += 1, xp = 0 (~10%)
    Evolution,
    /// Power: multiplier += 25 (0.025x since BASE_MULTIPLIER=1000) (~30%)
    Power,
    /// Trait: reroll a visual gene in dna (~60%)
    Trait,
}

#[derive(Clone, Debug)]
pub struct MutationResult {
    /// Mutation type (None if no mutation triggered)
    pub mutation_type: Option<MutationType>,
    /// XP gained from the claim's eligible SOL stake, scaled by multiplier.
    pub xp_gained: u32,
    /// Multiplier increase (0 if no mutation)
    pub multiplier_increase: u32,
    /// XP consumed by the mutation (deducted from cached XP after applying boost)
    pub xp_consumed: u32,
    /// New DNA after mutation
    pub new_dna: [u8; 32],
}

/// Calculates a claim-time mutation roll for a gameplay Doge.
///
/// **Chance Formula**:
/// `base * stake_strength * multiplier_penalty * faction_penalty * volume * cooldown * pacing * claim_boost`
///
/// **XP Gain**: Gained from the eligible SOL stake that produced the winning claim,
/// scaled down by current multiplier.
pub fn calculate_mutation_result(
    origin: u8,
    origin_id: u64,
    user_total_bet: u64,
    highest_round_bet_for_faction: u64,
    current_multiplier: u32,
    mut gameplay_doge_dna: [u8; 32],
    gameplay_doge_xp: u32,
    max_evolution_stage_unlocked: u8,
    faction_mutation_count: u8,
    faction_round_volume: u64,
    tuning: &GameplayTuningConfig,
    chance_boost_bps: u64,
    cycle_rounds_elapsed: u16,
    cycle_mutations_triggered: u16,
    recent_mutation_pressure_bps: u16,
    total_sol_bets: u64,
    total_points_bets: u64,
    total_wgtd_points_bets: u64,
    slot: u64,
    user_key: &Pubkey,
    doge_mint: &Pubkey,
) -> MutationResult {
    crate::log_fn!("genescience", "calculate_mutation_result");
    msg!("🧬 Calculating mutation result...");
    msg!(
        "   User total bet: {} SOL, Highest round bet for faction: {} SOL",
        format!("{:.4}", user_total_bet as f64 / 1e9),
        format!("{:.4}", highest_round_bet_for_faction as f64 / 1e9)
    );
    msg!(
        "   Current multiplier: {:.3}. Gameplay doge XP: {}",
        current_multiplier as f64 / 1000.0,
        gameplay_doge_xp
    );
    msg!(
        "   Total sol bets: {} SOL, Faction round volume: {} SOL, Total points bets: {}, Total wgtd points bets: {}",
        format!("{:.4}", total_sol_bets as f64 / 1e9),
        format!("{:.4}", faction_round_volume as f64 / 1e9),
        format!("{:.4}", total_points_bets as f64 / 1e9),
        format!("{:.4}", total_wgtd_points_bets as f64 / 1e9)
    );
    msg!("   Slot: {}. User key: {}", slot, user_key);

    // Derive generation from DNA (bits 4-6 of byte 0)
    let generation = get_evolution_stage(&gameplay_doge_dna);
    let evolution_allowed = generation < max_evolution_stage_unlocked;

    msg!(
        "🧬 Mutation calc: bet={} SOL, gen={}, xp={}, mult={:.3}, max_evolution_stage_unlocked={}, evolution_allowed={}",
        format!("{:.4}", user_total_bet as f64 / 1e9),
        generation,
        gameplay_doge_xp,
        current_multiplier as f64 / 1000.0,
        max_evolution_stage_unlocked,
        evolution_allowed
    );

    // --- STEP 1: CALCULATE TRIGGER CHANCE ---

    // XP gain scales inversely with multiplier: stronger doges gain XP slower.
    // At 1.0x: full rate (1 XP per 0.001 SOL)
    // At 5.0x: 1/5 rate (1 XP per 0.005 SOL)
    // At 4.2x: ~1/4.2 rate (about 1 XP per 0.0042 SOL)
    // Formula: xp = (bet / 1_000_000) × (BASE_MULTIPLIER / current_multiplier)
    let raw_xp = (user_total_bet as u128) / 1_000_000;
    let xp_gained =
        (raw_xp * BASE_MULTIPLIER as u128 / current_multiplier.max(BASE_MULTIPLIER) as u128) as u32;
    msg!(
        "   XP gained: {} (raw={}, mult_penalty={}x→{}x)",
        xp_gained,
        raw_xp,
        current_multiplier as f64 / 1000.0,
        BASE_MULTIPLIER as f64 / current_multiplier.max(BASE_MULTIPLIER) as f64
    );

    if user_total_bet == 0 {
        return MutationResult {
            mutation_type: None,
            xp_gained,
            multiplier_increase: 0,
            xp_consumed: 0,
            new_dna: gameplay_doge_dna,
        };
    }

    // A. Bet Strength (0 - 10,000 bps) -  If highest is 0 (first bet), strength is 100%. Otherwise ratio. (100% = 100,00)
    let effective_highest = if highest_round_bet_for_faction == 0 {
        user_total_bet
    } else {
        highest_round_bet_for_faction
    };
    let bet_strength = ((user_total_bet as u128)
        .saturating_mul(BASIS_POINTS_DENOMINATOR as u128)
        .checked_div(effective_highest as u128)
        .unwrap_or(BASIS_POINTS_DENOMINATOR as u128))
    .min(BASIS_POINTS_DENOMINATOR as u128) as u64;
    msg!("   Bet strength: {:.2}%", bet_strength as f64 / 100.0);

    // B. Multiplier Penalty (0 - 10,000 bps)
    // Higher multiplier = lower chance, slowing progression for advanced doges.
    let effective_multiplier = current_multiplier.max(BASE_MULTIPLIER);
    let mult_factor =
        (BASE_MULTIPLIER as u64 * BASIS_POINTS_DENOMINATOR) / effective_multiplier as u64;
    msg!("   Multiplier factor: {:.2}%", mult_factor as f64 / 100.0);

    // C. Per-Faction Penalty: each prior mutation this round makes the next harder.
    // 10000 / (10000 + count * 5000):  0 prior → 100%, 1 → 67%, 2 → 50%, 3 → 40%
    // Scaled by 10000 so this lands on the same bps basis as bet_strength and
    // mult_factor — otherwise the integer division collapses to 1 and the
    // final_chance_bps multiplication gets divided out to 0.
    let faction_penalty = (10000u64 * 10000u64)
        / (10000u64 + faction_mutation_count as u64 * FACTION_MUTATION_PENALTY_STEP);
    msg!(
        "   Faction penalty ({}): {:.2}%",
        faction_mutation_count,
        faction_penalty as f64 / 100.0
    );

    // D. Country-volume factor: more real SOL support unlocks more mutation throughput.
    let required_volume = tuning
        .faction_volume_threshold_lamports
        .saturating_add(
            tuning
                .extra_volume_threshold_per_mutation_lamports
                .saturating_mul(faction_mutation_count as u64),
        )
        .max(1);
    let volume_factor = ((faction_round_volume as u128)
        .saturating_mul(10_000)
        .checked_div(required_volume as u128)
        .unwrap_or(10_000))
    .min(10_000) as u64;
    msg!(
        "   Volume factor: {:.2}% (volume={} / required={})",
        volume_factor as f64 / 100.0,
        faction_round_volume,
        required_volume
    );

    // E. Global cooldown factor: recent mutation-heavy rounds suppress the next few rounds.
    let cooldown_factor =
        BASIS_POINTS_DENOMINATOR.saturating_sub(recent_mutation_pressure_bps as u64);
    msg!(
        "   Cooldown factor: {:.2}% (recent_pressure={} bps)",
        cooldown_factor as f64 / 100.0,
        recent_mutation_pressure_bps
    );

    // F. Cycle pacing factor: compare observed mutations so far against target-by-now.
    let progress_rounds = u64::from(cycle_rounds_elapsed).max(1);
    let target_rounds = u64::from(tuning.target_rounds_per_cycle).max(1);
    let target_mutations = u64::from(tuning.target_mutations_per_cycle).max(1);
    let expected_mutations_x100 = target_mutations
        .checked_mul(progress_rounds)
        .unwrap_or(0)
        .checked_mul(100)
        .unwrap_or(0)
        .checked_div(target_rounds)
        .unwrap_or(0);
    let actual_mutations_x100 = u64::from(cycle_mutations_triggered).saturating_mul(100);
    let mutation_gap_x100 = expected_mutations_x100 as i64 - actual_mutations_x100 as i64;
    let pacing_adjustment = ((mutation_gap_x100 as i128)
        .checked_mul(tuning.pacing_max_adjustment_bps as i128)
        .unwrap_or(0)
        .checked_div((target_mutations.saturating_mul(100)).max(100) as i128)
        .unwrap_or(0))
    .clamp(
        -(tuning.pacing_max_adjustment_bps as i128),
        tuning.pacing_max_adjustment_bps as i128,
    ) as i64;
    let max_pacing_factor =
        BASIS_POINTS_DENOMINATOR as i64 + tuning.pacing_max_adjustment_bps as i64;
    let pacing_factor =
        (BASIS_POINTS_DENOMINATOR as i64 + pacing_adjustment).clamp(0, max_pacing_factor) as u64;
    msg!(
        "   Pacing factor: {:.2}% (expected_x100={}, actual_x100={}, adjustment={} bps)",
        pacing_factor as f64 / 100.0,
        expected_mutations_x100,
        actual_mutations_x100,
        pacing_adjustment
    );

    // G. Final Chance = BASE × bet_strength × mult_factor × faction_penalty × volume × cooldown × pacing × claim boost
    let final_chance_bps = ((tuning.base_mutation_chance_bps as u128)
        * (bet_strength as u128)
        * (mult_factor as u128)
        * (faction_penalty as u128)
        * (volume_factor as u128)
        * (cooldown_factor as u128)
        * (pacing_factor as u128)
        * (chance_boost_bps as u128)
        / 10_000u128.pow(7))
    .clamp(
        tuning.mutation_chance_floor_bps as u128,
        tuning.mutation_chance_cap_bps as u128,
    ) as u64;
    msg!(
        "   Final chance: {:.2}% (claim_boost={} bps)",
        final_chance_bps as f64 / 100.0,
        chance_boost_bps
    );

    // --- STEP 2: ROLL THE DICE ---

    // Roll the dice
    let total_combined = total_sol_bets
        .saturating_add(total_points_bets)
        .saturating_add(total_wgtd_points_bets);
    let seed = keccak::hashv(&[
        &origin.to_le_bytes(),
        &origin_id.to_le_bytes(),
        &slot.to_le_bytes(),
        &total_sol_bets.to_le_bytes(),
        &total_combined.to_le_bytes(),
        user_key.as_ref(),
        doge_mint.as_ref(),
        &gameplay_doge_dna,
    ])
    .to_bytes();

    // Roll 1: Did we hit the mutation? (0-10000)
    // Use first 2 bytes for higher precision (u16)
    let roll_val = u16::from_le_bytes([seed[0], seed[1]]) as u64;
    let roll_normalized = (roll_val * 10_000) / 65535;
    msg!("   Roll normalized: {:.2}%", roll_normalized as f64 / 100.0);

    if roll_normalized >= final_chance_bps {
        return MutationResult {
            mutation_type: None,
            xp_gained,
            multiplier_increase: 0,
            xp_consumed: 0,
            new_dna: gameplay_doge_dna,
        };
    }
    msg!("   Mutation triggered!!!");

    // Mutation triggered - determine type
    let type_roll = seed[2]; // random dice roll for the type of mutation
    let evo_chance = 10 / (generation as u64 + 1); // base percentage chance (10%) for an Evolution to happen.
    let evo_threshold = if evolution_allowed {
        (255 * evo_chance) / 100 // This is the "cutoff point" on the 0-255 scale for Evolution.
    } else {
        0
    };

    // When evolution is locked, fold the evolution bucket into power mutations so the
    // overall mutation cadence stays intact while staged visuals remain under admin control.
    let power_share_pct = if evolution_allowed { 30 } else { 40 };
    // Power threshold is the "cutoff point" on the 0-255 scale for Power.
    let power_threshold = evo_threshold + ((255 * power_share_pct) / 100);
    msg!(
        "Type roll: {}, Evo Chance: {}, Evo threshold: {}, Power threshold: {}, Power share: {}%",
        type_roll,
        evo_chance,
        evo_threshold,
        power_threshold,
        power_share_pct
    );
    if !evolution_allowed {
        msg!(
            "   Evolution roll bucket is locked for stage {}. Re-routing that weight into power mutations.",
            generation
        );
    }

    let (m_type, base_boost) = if (type_roll as u64) < evo_threshold {
        evolve_stage(origin, origin_id, &mut gameplay_doge_dna, &seed, doge_mint);
        (MutationType::Evolution, 50u32)
    } else if (type_roll as u64) < power_threshold {
        mutate_power_trait(origin, origin_id, &mut gameplay_doge_dna, &seed, doge_mint);
        (MutationType::Power, 25u32)
    } else {
        mutate_visual_trait(origin, origin_id, &mut gameplay_doge_dna, &seed, doge_mint);
        (MutationType::Trait, 5u32)
    };
    msg!("   Mutation type: {:?}, Base boost: {}", m_type, base_boost);

    // XP Bonus: a small fraction of accumulated XP boosts the multiplier.
    // Kept low so the grind to the 4.2x cap takes sustained active play, not minutes.
    // Evolution: 5-10% of XP.  Power/Trait: 2-5% of XP.
    let xp_roll = seed[3] as u64;
    let (min_pct, max_pct) = if m_type == MutationType::Evolution {
        (5u64, 10u64)
    } else {
        (2u64, 5u64)
    };
    let efficiency_pct = min_pct + ((xp_roll * (max_pct - min_pct)) / 255);
    let xp_mult_boost = (gameplay_doge_xp as u64 * efficiency_pct) / 100;

    // XP consumed by the mutation:
    // Evolution consumes ALL XP (full reset).
    // Power/Trait consume what they used (the efficiency% portion).
    let xp_consumed = if m_type == MutationType::Evolution {
        gameplay_doge_xp // full reset
    } else {
        (gameplay_doge_xp as u64 * efficiency_pct / 100) as u32 // consume what was used
    };

    msg!(
        "   XP roll: {}, Efficiency: {}%, XP mult boost: {}, XP consumed: {}",
        xp_roll,
        efficiency_pct,
        xp_mult_boost,
        xp_consumed
    );

    MutationResult {
        mutation_type: Some(m_type),
        xp_gained,
        multiplier_increase: base_boost + xp_mult_boost as u32,
        xp_consumed,
        new_dna: gameplay_doge_dna,
    }
}

// ========================================================================================
// ============================= TRAIT MUTATION FUNCTIONS =================================
// ========================================================================================

/// Evolve to next Generation with guaranteed mutations
pub fn evolve_stage(
    origin: u8,
    origin_id: u64,
    dna: &mut [u8; 32],
    seed: &[u8],
    doge_mint: &Pubkey,
) -> (u8, u8, u8, u8, u8, u8, u8) {
    crate::log_fn!("genescience", "evolve_stage");
    let current_stage = (dna[0] >> 4) & 0x07;
    if current_stage >= 7 {
        return (current_stage, 0, 0, 0, 0, 0, 0);
    }

    let new_stage = current_stage + 1;
    dna[0] = (dna[0] & 0x8F) | ((new_stage & 0x07) << 4);
    msg!("🧬 EVOLUTION: Stage {} -> {}", current_stage, new_stage);

    // Sub-mutations don't emit their own events during evolution (we emit one combined event)
    let (m_index, m_current_val, m_new_val) = mutate_visual_trait_internal(dna, seed);
    let power_seed = keccak::hashv(&[seed, b"power"]).to_bytes();
    let (p_index, p_current_val, p_new_val) = mutate_power_trait_internal(dna, &power_seed);

    emit!(DogeEvolution {
        origin,
        origin_id,
        doge_mint: *doge_mint,
        new_stage,
        visual_trait_index: m_index,
        visual_old_val: m_current_val,
        visual_new_val: m_new_val,
        power_trait_index: p_index,
        power_old_val: p_current_val,
        power_new_val: p_new_val,
    });

    (
        new_stage,
        m_index,
        m_current_val,
        m_new_val,
        p_index,
        p_current_val,
        p_new_val,
    )
}

/// Mutate a random visual trait (+1 to +3, cap at 31) with event emission
pub fn mutate_visual_trait(
    origin: u8,
    origin_id: u64,
    dna: &mut [u8; 32],
    seed: &[u8],
    doge_mint: &Pubkey,
) -> (u8, u8, u8) {
    crate::log_fn!("genescience", "mutate_visual_trait");
    let (trait_index, current_val, new_val) = mutate_visual_trait_internal(dna, seed);

    emit!(DogeVisualMutation {
        origin,
        origin_id,
        doge_mint: *doge_mint,
        trait_index,
        old_val: current_val,
        new_val,
    });

    (trait_index, current_val, new_val)
}

/// Internal visual trait mutation (no event - used during evolution)
fn mutate_visual_trait_internal(dna: &mut [u8; 32], seed: &[u8]) -> (u8, u8, u8) {
    let hash = keccak::hash(seed).to_bytes();
    let trait_index = hash[0] as usize % APPEARANCE_TOTAL_TRAITS;
    let current_val = get_trait_value(
        dna,
        APPEARANCE_OFFSET,
        APPEARANCE_TRAIT_BITS,
        trait_index as u8,
    );
    let increase = (hash[1] % 3) + 1;
    let new_val = (current_val + increase).min(APPEARANCE_MAX);

    if new_val > current_val {
        set_trait_value(
            dna,
            APPEARANCE_OFFSET,
            APPEARANCE_TRAIT_BITS,
            trait_index as u8,
            new_val,
        );
        msg!(
            "🎨 Visual: Trait #{} {} -> {}",
            trait_index,
            current_val,
            new_val
        );
    }
    (trait_index as u8, current_val, new_val)
}

/// Mutate a random power trait (+1 to +3, cap at 15) with event emission
pub fn mutate_power_trait(
    origin: u8,
    origin_id: u64,
    dna: &mut [u8; 32],
    seed: &[u8],
    doge_mint: &Pubkey,
) -> (u8, u8, u8) {
    crate::log_fn!("genescience", "mutate_power_trait");
    let (trait_index, current_val, new_val) = mutate_power_trait_internal(dna, seed);

    emit!(DogePowerMutation {
        origin,
        origin_id,
        doge_mint: *doge_mint,
        trait_index,
        old_val: current_val,
        new_val,
    });

    (trait_index, current_val, new_val)
}

/// Internal power trait mutation (no event - used during evolution)
fn mutate_power_trait_internal(dna: &mut [u8; 32], seed: &[u8]) -> (u8, u8, u8) {
    let hash = keccak::hash(seed).to_bytes();
    let trait_index = hash[2] as usize % COMBAT_TOTAL_TRAITS;
    let current_val = get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, trait_index as u8);
    let increase = (hash[3] % 3) + 1;
    let new_val = (current_val + increase).min(COMBAT_MAX);

    if new_val > current_val {
        set_trait_value(
            dna,
            COMBAT_OFFSET,
            POWER_TRAIT_BITS,
            trait_index as u8,
            new_val,
        );
        msg!(
            "⚡ Power: Trait #{} {} -> {}",
            trait_index,
            current_val,
            new_val
        );
    }
    (trait_index as u8, current_val, new_val)
}

// ========================================================================================
// ============================= BREEDING SYSTEM ==========================================
// ========================================================================================

/// Breed two characters to create offspring
pub fn breed_genes(
    parent1_dna: &[u8; 32],
    parent2_dna: &[u8; 32],
    seed: &[u8],
) -> Result<[u8; 32]> {
    crate::log_fn!("genescience", "breed_genes");
    let hash = keccak::hash(seed).to_bytes();
    let mut offspring_dna = [0u8; 32];

    offspring_dna[0] = parent1_dna[0] & 0x0F; // Same faction, stage 0

    mix_appearance_traits(&mut offspring_dna, parent1_dna, parent2_dna, &hash);
    mix_power_traits(&mut offspring_dna, parent1_dna, parent2_dna, &hash);

    // Inherit breed from one parent (50/50 chance)
    let parent_breed = if (hash[31] & 1) == 0 {
        get_trait_value(parent1_dna, BREED_OFFSET, BREED_BITS, 0)
    } else {
        get_trait_value(parent2_dna, BREED_OFFSET, BREED_BITS, 0)
    };
    set_trait_value(
        &mut offspring_dna,
        BREED_OFFSET,
        BREED_BITS,
        0,
        parent_breed,
    );

    Ok(offspring_dna)
}

fn mix_appearance_traits(
    offspring: &mut [u8; 32],
    p1: &[u8; 32],
    p2: &[u8; 32],
    random: &[u8; 32],
) {
    for i in 0..APPEARANCE_TOTAL_TRAITS {
        let t1 = get_trait_value(p1, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        let t2 = get_trait_value(p2, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
        let method = random[i % 32] % 4;
        let rand = random[(i + 16) % 32];

        let selected = match method {
            0 => {
                if (rand & 1) == 0 {
                    t1
                } else {
                    t2
                }
            }
            1 => ((t1 as u16 + t2 as u16) / 2) as u8,
            2 => enhance_trait(t1, t2, rand, APPEARANCE_MAX),
            _ => mutate_trait_value(t1.max(t2), rand, APPEARANCE_MAX),
        };
        set_trait_value(
            offspring,
            APPEARANCE_OFFSET,
            APPEARANCE_TRAIT_BITS,
            i as u8,
            selected,
        );
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
        set_trait_value(
            offspring,
            COMBAT_OFFSET,
            POWER_TRAIT_BITS,
            i as u8,
            selected,
        );
    }
}

fn enhance_trait(t1: u8, t2: u8, rand: u8, max: u8) -> u8 {
    let max_t = t1.max(t2);
    let min_t = t1.min(t2);
    if rand < 48 && max_t < max {
        max_t + 1
    } else if rand < 96 && min_t < max_t {
        min_t + 1
    } else {
        max_t
    }
}

fn mutate_trait_value(base: u8, rand: u8, max: u8) -> u8 {
    if rand == 0 {
        // Wild mutation: use a pseudo-random value up to max-1
        // Since rand is 0, we use a deterministic but varied approach based on base
        let max_val = max.saturating_sub(1) as u16;
        ((base as u16 * 7 + 13) % (max_val + 1)) as u8
    } else if rand < 48 && base > 0 {
        base - 1
    } else if rand < 80 && base < max {
        base + 1
    } else {
        base
    }
}

fn synergy_boost(t1: u8, t2: u8, rand: u8, max: u8) -> u8 {
    let max_t = t1.max(t2);
    let diff = t1.abs_diff(t2);
    if diff <= 2 && rand < 64 && max_t < max {
        max_t + 1
    } else {
        max_t
    }
}

// ========================================================================================
// ============================= EVOLUTION SYSTEM =========================================
// ========================================================================================

// /// Evolve genes to next stage with probabilistic trait improvements
// pub fn evolve_genes(dna: &mut [u8; 32], seed: &[u8]) -> Result<u8> {
//     let current_stage = (dna[0] >> 4) & 0x07;
//     require!(current_stage < 7, ErrorCode::InvalidParameters);

//     let new_stage = current_stage + 1;
//     dna[0] = (dna[0] & 0x8F) | ((new_stage & 0x07) << 4);

//     let hash = keccak::hash(seed).to_bytes();
//     evolve_appearance_traits(dna, new_stage, &hash);
//     evolve_power_traits(dna, new_stage, &hash);

//     Ok(new_stage)
// }

// fn evolve_appearance_traits(dna: &mut [u8; 32], stage: u8, random: &[u8; 32]) {
//     let chance = evolution_chance(stage);
//     for i in 0..APPEARANCE_TOTAL_TRAITS {
//         if random[i % 32] < chance {
//             let current = get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
//             let evolved = evolve_single_trait(current, stage, random[(i + 16) % 32], APPEARANCE_MAX);
//             set_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8, evolved);
//         }
//     }
// }

// fn evolve_power_traits(dna: &mut [u8; 32], stage: u8, random: &[u8; 32]) {
//     let chance = evolution_chance(stage);
//     for i in 0..COMBAT_TOTAL_TRAITS {
//         if random[(i + 8) % 32] < chance {
//             let current = get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
//             let evolved = evolve_single_trait(current, stage, random[(i + 24) % 32], COMBAT_MAX);
//             set_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, evolved);
//         }
//     }
// }

// fn evolution_chance(stage: u8) -> u8 {
//     match stage { 0 => 50, 1 => 70, 2 => 90, 3 => 110, 4 => 90, 5 => 70, 6 => 50, _ => 30 }
// }

// fn evolve_single_trait(current: u8, stage: u8, rand: u8, max_cap: u8) -> u8 {
//     let max_for_stage = if stage == EVOLUTION_STAGES - 1 { max_cap } else { (max_cap * 3 / 4) + stage };
//     if current >= max_for_stage { return current; }

//     if rand < 192 { (current + 1).min(max_for_stage) }
//     else if rand < 224 && current + 2 <= max_for_stage { current + 2 }
//     else if rand < 244 && current > 0 { current - 1 }
//     else if rand < 248 && current + 3 <= max_for_stage { current + 3 }
//     else { current }
// }

// ========================================================================================
// ============================= BIT MANIPULATION HELPERS =================================
// ========================================================================================

fn get_trait_value(dna: &[u8; 32], base_offset: u8, trait_bits: u8, index: u8) -> u8 {
    let start_bit = base_offset as usize + (index as usize * trait_bits as usize);
    let byte_idx = start_bit / 8;
    let bit_idx = start_bit % 8;
    if byte_idx >= 32 {
        return 0;
    }

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
    if byte_idx >= 32 {
        return;
    }

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

// ========================================================================================
// ============================= PUBLIC DECODER FUNCTIONS (for testing) ===================
// ========================================================================================

/// Get breed value from DNA (2 bits at BREED_OFFSET)
#[allow(dead_code)]
pub fn get_breed(dna: &[u8; 32]) -> u8 {
    crate::log_fn!("genescience", "get_breed");
    get_trait_value(dna, BREED_OFFSET, BREED_BITS, 0)
}

/// Get faction/family type from DNA (first 4 bits)
pub fn get_family_type(dna: &[u8; 32]) -> u8 {
    crate::log_fn!("genescience", "get_family_type");
    dna[0] & 0x0F
}

/// Get evolution stage from DNA (bits 4-6)
pub fn get_evolution_stage(dna: &[u8; 32]) -> u8 {
    crate::log_fn!("genescience", "get_evolution_stage");
    (dna[0] >> 4) & 0x07
}

/// Decode all 21 appearance traits
pub fn decode_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
    crate::log_fn!("genescience", "decode_appearance_traits");
    (0..APPEARANCE_TOTAL_TRAITS)
        .map(|i| get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8))
        .collect()
}

/// Decode dominant appearance traits (first from each of 7 groups)
pub fn decode_dominant_appearance_traits(dna: &[u8; 32]) -> Vec<u8> {
    crate::log_fn!("genescience", "decode_dominant_appearance_traits");
    (0..7)
        .map(|i| get_trait_value(dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, (i * 3) as u8))
        .collect()
}

/// Decode all 15 power traits
pub fn decode_power_traits(dna: &[u8; 32]) -> Vec<u8> {
    crate::log_fn!("genescience", "decode_power_traits");
    (0..COMBAT_TOTAL_TRAITS)
        .map(|i| get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8))
        .collect()
}

/// Decode dominant power traits (first from each of 5 groups)
pub fn decode_dominant_power_traits(dna: &[u8; 32]) -> Vec<u8> {
    crate::log_fn!("genescience", "decode_dominant_power_traits");
    (0..5)
        .map(|i| get_trait_value(dna, COMBAT_OFFSET, POWER_TRAIT_BITS, (i * 3) as u8))
        .collect()
}

// ========================================================================================
// ============================= TESTS ====================================================
// ========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::STORY_EVENT_ORIGIN_ROUND;
    use anchor_lang::prelude::Pubkey;

    fn mock_pubkey() -> Pubkey {
        Pubkey::new_from_array([1u8; 32])
    }

    fn test_tuning() -> GameplayTuningConfig {
        let mut tuning = GameplayTuningConfig {
            rpg_progression: false,
            max_evolution_stage_unlocked: 0,
            faction_war_base_reward_bps: 0,
            faction_war_loyalty_reward_bps: 0,
            faction_war_doge_reward_bps: 0,
            faction_war_mvp_reward_bps: 0,
            base_mutation_chance_bps: 0,
            mutation_chance_floor_bps: 0,
            mutation_chance_cap_bps: 0,
            faction_volume_threshold_lamports: 0,
            extra_volume_threshold_per_mutation_lamports: 0,
            global_mutation_pressure_decay_bps: 0,
            global_mutation_pressure_per_mutation_bps: 0,
            target_mutations_per_cycle: 0,
            target_rounds_per_cycle: 0,
            pacing_max_adjustment_bps: 0,
        };
        tuning.apply_defaults();
        tuning
    }

    // --- DNA GENERATION TESTS ---

    #[test]
    fn test_generate_genesis_dna_basic() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 12345, 5).unwrap();
        println!("\n=== DNA Generation Test ===");
        println!("DNA bytes: {:?}", dna);
        println!(
            "DNA hex: {}",
            dna.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );

        // Check faction is encoded correctly (first 4 bits)
        let faction = get_family_type(&dna);
        println!("Faction: {}", faction);
        assert_eq!(faction, 5, "Faction should be 5");

        // Check evolution stage is 0 (bits 4-6)
        let stage = get_evolution_stage(&dna);
        println!("Evolution Stage: {}", stage);
        assert_eq!(stage, 0, "Evolution stage should be 0 for genesis");

        // Decode traits
        let app_traits = decode_appearance_traits(&dna);
        let pwr_traits = decode_power_traits(&dna);
        let dom_app = decode_dominant_appearance_traits(&dna);
        let dom_pwr = decode_dominant_power_traits(&dna);

        println!("\nAppearance Traits (21 total):");
        println!("  All: {:?}", app_traits);
        println!("  Dominant (7): {:?}", dom_app);

        println!("\nPower Traits (15 total):");
        println!("  All: {:?}", pwr_traits);
        println!("  Dominant (5): {:?}", dom_pwr);

        // Verify genesis constraints
        let app_valid = app_traits.iter().all(|&t| t < 10);
        let pwr_first_9_valid = pwr_traits[0..9].iter().all(|&t| t < 5);
        let pwr_last_6_zero = pwr_traits[9..15].iter().all(|&t| t == 0);

        println!("\nGenesis Constraints:");
        println!(
            "  Appearance traits < 10: {} (all: {:?})",
            app_valid,
            app_traits.iter().all(|&t| t < 10)
        );
        println!(
            "  First 9 power traits < 5: {} (values: {:?})",
            pwr_first_9_valid,
            &pwr_traits[0..9]
        );
        println!(
            "  Last 6 power traits == 0: {} (values: {:?})",
            pwr_last_6_zero,
            &pwr_traits[9..15]
        );

        assert!(app_valid, "All appearance traits must be < 10 for genesis");
        assert!(
            pwr_first_9_valid,
            "First 9 power traits must be < 5 for genesis"
        );
        assert!(pwr_last_6_zero, "Last 6 power traits must be 0 for genesis");
        println!("\n✅ All genesis constraints satisfied!");
    }

    #[test]
    fn test_generate_genesis_dna_all_factions() {
        for faction_id in 0..12 {
            let dna =
                generate_genesis_dna(faction_id as u64, &mock_pubkey(), 100, faction_id).unwrap();
            assert_eq!(
                get_family_type(&dna),
                faction_id,
                "Faction {} mismatch",
                faction_id
            );
        }
    }

    #[test]
    fn test_generate_genesis_dna_deterministic() {
        let dna1 = generate_genesis_dna(100, &mock_pubkey(), 999, 3).unwrap();
        let dna2 = generate_genesis_dna(100, &mock_pubkey(), 999, 3).unwrap();
        assert_eq!(dna1, dna2, "Same inputs should produce same DNA");
    }

    #[test]
    fn test_generate_genesis_dna_different_inputs() {
        let dna1 = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let dna2 = generate_genesis_dna(2, &mock_pubkey(), 100, 0).unwrap();
        assert_ne!(
            dna1, dna2,
            "Different mint numbers should produce different DNA"
        );
    }

    // --- TRAIT DECODING TESTS ---

    #[test]
    fn test_decode_appearance_traits_count() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let traits = decode_appearance_traits(&dna);
        assert_eq!(
            traits.len(),
            21,
            "Should have 21 appearance traits (7 groups × 3)"
        );
    }

    #[test]
    fn test_decode_power_traits_count() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let traits = decode_power_traits(&dna);
        assert_eq!(
            traits.len(),
            15,
            "Should have 15 power traits (5 groups × 3)"
        );
    }

    #[test]
    fn test_appearance_traits_in_range() {
        let dna = generate_genesis_dna(42, &mock_pubkey(), 9999, 7).unwrap();
        let traits = decode_appearance_traits(&dna);
        for (i, &t) in traits.iter().enumerate() {
            assert!(
                t <= APPEARANCE_MAX,
                "Appearance trait {} value {} exceeds max {}",
                i,
                t,
                APPEARANCE_MAX
            );
        }
    }

    #[test]
    fn test_power_traits_in_range() {
        let dna = generate_genesis_dna(42, &mock_pubkey(), 9999, 7).unwrap();
        let traits = decode_power_traits(&dna);
        for (i, &t) in traits.iter().enumerate() {
            assert!(
                t <= COMBAT_MAX,
                "Power trait {} value {} exceeds max {}",
                i,
                t,
                COMBAT_MAX
            );
        }
    }

    #[test]
    fn test_dominant_appearance_traits() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let all = decode_appearance_traits(&dna);
        let dominant = decode_dominant_appearance_traits(&dna);

        assert_eq!(
            dominant.len(),
            7,
            "Should have 7 dominant appearance traits"
        );
        for i in 0..7 {
            assert_eq!(dominant[i], all[i * 3], "Dominant trait {} mismatch", i);
        }
    }

    #[test]
    fn test_dominant_power_traits() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let all = decode_power_traits(&dna);
        let dominant = decode_dominant_power_traits(&dna);

        assert_eq!(dominant.len(), 5, "Should have 5 dominant power traits");
        for i in 0..5 {
            assert_eq!(
                dominant[i],
                all[i * 3],
                "Dominant power trait {} mismatch",
                i
            );
        }
    }

    // // --- MUTATION TESTS ---

    #[test]
    fn test_mutate_visual_trait() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let original = decode_appearance_traits(&dna);
        println!("Original appearance traits: {:?}", original);

        let seed = [1u8; 32];
        let doge_mint = mock_pubkey();
        let (trait_idx, _, _) =
            mutate_visual_trait(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);

        let mutated = decode_appearance_traits(&dna);
        println!("Mutated appearance traits: {:?}", mutated);

        // At least one trait should have changed (or stayed same if at max)
        let original_val = original[trait_idx as usize];
        let mutated_val = mutated[trait_idx as usize];
        assert!(
            mutated_val >= original_val,
            "Visual trait should increase or stay same"
        );
        assert!(mutated_val <= APPEARANCE_MAX, "Should not exceed max");
    }

    #[test]
    fn test_mutate_power_trait() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let original = decode_power_traits(&dna);
        println!("Original power traits: {:?}", original);

        let seed = [2u8; 32];
        let doge_mint = mock_pubkey();
        let (_, original_val, mutated_val) =
            mutate_power_trait(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);

        let mutated = decode_power_traits(&dna);
        println!("Mutated power traits: {:?}", mutated);

        assert!(
            mutated_val >= original_val,
            "Power trait should increase or stay same"
        );
        assert!(mutated_val <= COMBAT_MAX, "Should not exceed max");
    }

    // --- EVOLUTION TESTS ---

    #[test]
    fn test_evolve_stage() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        assert_eq!(get_evolution_stage(&dna), 0);

        let original_visual_traits = decode_appearance_traits(&dna);
        println!("Original visual traits: {:?}", original_visual_traits);
        let original = decode_power_traits(&dna);
        println!("Original power traits: {:?}", original);

        let seed = [3u8; 32];
        let doge_mint = mock_pubkey();
        let (new_stage, m_index, m_current_val, m_new_val, p_index, p_current_val, p_new_val) =
            evolve_stage(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);
        println!("New stage: {}", new_stage);
        println!(
            "Mutated traits: {:?}",
            (
                m_index,
                m_current_val,
                m_new_val,
                p_index,
                p_current_val,
                p_new_val
            )
        );

        let new_visual_traits = decode_appearance_traits(&dna);
        println!("New visual traits: {:?}", new_visual_traits);

        let new_power_traits = decode_power_traits(&dna);
        println!("New power traits: {:?}", new_power_traits);
    }

    #[test]
    fn test_evolve_multiple_stages() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let doge_mint = mock_pubkey();

        for expected_stage in 1..=7 {
            let seed = [expected_stage; 32];
            let (new_stage, _, _, _, _, _, _) =
                evolve_stage(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);
            assert_eq!(
                new_stage, expected_stage,
                "Stage mismatch at {}",
                expected_stage
            );
        }

        // Should return current stage when at max (7)
        let seed = [8u8; 32];
        let (new_stage, m_index, m_current_val, m_new_val, p_index, p_current_val, p_new_val) =
            evolve_stage(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);
        assert_eq!(new_stage, 7, "Should stay at max stage");
        assert_eq!(m_index, 0, "No mutation at max");
        assert_eq!(m_current_val, 0, "No mutation at max");
        assert_eq!(m_new_val, 0, "No mutation at max");
        assert_eq!(p_index, 0, "No mutation at max");
        assert_eq!(p_current_val, 0, "No mutation at max");
        assert_eq!(p_new_val, 0, "No mutation at max");
    }

    #[test]
    fn test_evolution_preserves_faction() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 9).unwrap();
        let original_faction = get_family_type(&dna);

        let seed = [1u8; 32];
        let doge_mint = mock_pubkey();
        let _ = evolve_stage(STORY_EVENT_ORIGIN_ROUND, 1, &mut dna, &seed, &doge_mint);

        assert_eq!(
            get_family_type(&dna),
            original_faction,
            "Faction should be preserved"
        );
    }

    // --- BREEDING TESTS ---

    #[test]
    fn test_breed_genes_basic() {
        let parent1 = generate_genesis_dna(1, &mock_pubkey(), 100, 5).unwrap();
        let parent2 = generate_genesis_dna(2, &mock_pubkey(), 200, 5).unwrap();

        let seed = b"breeding_seed_12345678901234567890";
        let offspring = breed_genes(&parent1, &parent2, seed).unwrap();

        // Offspring should have same faction as parent1
        assert_eq!(
            get_family_type(&offspring),
            5,
            "Offspring should inherit faction"
        );

        // Offspring should start at stage 0
        assert_eq!(
            get_evolution_stage(&offspring),
            0,
            "Offspring should be stage 0"
        );
    }

    #[test]
    fn test_breed_genes_deterministic() {
        let parent1 = generate_genesis_dna(1, &mock_pubkey(), 100, 3).unwrap();
        let parent2 = generate_genesis_dna(2, &mock_pubkey(), 200, 3).unwrap();

        let seed = b"test_seed_1234567890123456789012";
        let offspring1 = breed_genes(&parent1, &parent2, seed).unwrap();
        let offspring2 = breed_genes(&parent1, &parent2, seed).unwrap();

        assert_eq!(
            offspring1, offspring2,
            "Same breeding should produce same offspring"
        );
    }

    #[test]
    fn test_breed_genes_different_seeds() {
        let parent1 = generate_genesis_dna(1, &mock_pubkey(), 100, 3).unwrap();
        let parent2 = generate_genesis_dna(2, &mock_pubkey(), 200, 3).unwrap();

        let offspring1 =
            breed_genes(&parent1, &parent2, b"seed_a_12345678901234567890123456").unwrap();
        let offspring2 =
            breed_genes(&parent1, &parent2, b"seed_b_12345678901234567890123456").unwrap();

        assert_ne!(
            offspring1, offspring2,
            "Different seeds should produce different offspring"
        );
    }

    #[test]
    fn test_breed_offspring_traits_in_range() {
        let parent1 = generate_genesis_dna(10, &mock_pubkey(), 500, 2).unwrap();
        let parent2 = generate_genesis_dna(20, &mock_pubkey(), 600, 2).unwrap();

        let offspring =
            breed_genes(&parent1, &parent2, b"range_test_seed_1234567890123456").unwrap();

        for t in decode_appearance_traits(&offspring) {
            assert!(t <= APPEARANCE_MAX, "Appearance trait exceeds max");
        }
        for t in decode_power_traits(&offspring) {
            assert!(t <= COMBAT_MAX, "Power trait exceeds max");
        }
    }

    // --- MUTATION RESULT TESTS ---

    #[test]
    fn test_mutation_result_zero_bet() {
        let dna = [
            133, 68, 70, 49, 137, 148, 80, 78, 16, 104, 152, 128, 82, 0, 68, 52, 16, 64, 0, 0, 0,
            48, 171, 230, 185, 253, 209, 30, 122, 100, 207, 57,
        ];
        let doge_mint = mock_pubkey();
        let tuning = test_tuning();

        let result = calculate_mutation_result(
            STORY_EVENT_ORIGIN_ROUND,
            1,
            1_000_000_000 / 2, // 0.5 SOL bet
            1_000_000_000,
            1000,
            dna,
            100,
            1,
            0, // faction_mutation_count
            100_000_000,
            &tuning,
            10_000,
            0,
            0,
            0,
            100_000_000,
            100_000_000,
            100_000_000,
            12345,
            &mock_pubkey(),
            &doge_mint,
        );

        // Note: with 0.5 SOL bet against 1 SOL highest, there's a chance of mutation
        // This test just verifies the function runs without error
        println!(
            "Mutation result: {:?}, XP: {}",
            result.mutation_type, result.xp_gained
        );
    }

    fn find_evolution_slot(dna: [u8; 32], max_evolution_stage_unlocked: u8) -> u64 {
        let doge_mint = mock_pubkey();
        let user = mock_pubkey();
        let tuning = test_tuning();

        for slot in 1..100_000 {
            let result = calculate_mutation_result(
                STORY_EVENT_ORIGIN_ROUND,
                1,
                1_000_000_000,
                1_000_000_000,
                1000,
                dna,
                0,
                max_evolution_stage_unlocked,
                0,
                1_000_000_000,
                &tuning,
                10_000,
                0,
                0,
                0,
                1_000_000_000,
                1_000_000_000,
                1_000_000_000,
                slot,
                &user,
                &doge_mint,
            );

            if result.mutation_type == Some(MutationType::Evolution) {
                return slot;
            }
        }

        panic!("expected to find a deterministic evolution slot for test inputs");
    }

    fn count_mutations_for_cycle_progress(
        tuning: &GameplayTuningConfig,
        cycle_rounds_elapsed: u16,
        cycle_mutations_triggered: u16,
    ) -> usize {
        let doge_mint = mock_pubkey();
        let user = mock_pubkey();
        let dna = generate_genesis_dna(7, &mock_pubkey(), 777, 0).unwrap();

        (1..=5_000u64)
            .filter(|slot| {
                calculate_mutation_result(
                    STORY_EVENT_ORIGIN_ROUND,
                    1,
                    1_000_000_000,
                    1_000_000_000,
                    1000,
                    dna,
                    0,
                    1,
                    0,
                    1_000_000_000,
                    tuning,
                    10_000,
                    cycle_rounds_elapsed,
                    cycle_mutations_triggered,
                    0,
                    1_000_000_000,
                    1_000_000_000,
                    1_000_000_000,
                    *slot,
                    &user,
                    &doge_mint,
                )
                .mutation_type
                .is_some()
            })
            .count()
    }

    #[test]
    fn test_pacing_factor_boosts_mutations_when_cycle_is_behind() {
        let mut tuning = test_tuning();
        tuning.base_mutation_chance_bps = 500;
        tuning.mutation_chance_floor_bps = 0;
        tuning.mutation_chance_cap_bps = 10_000;
        tuning.target_mutations_per_cycle = 12;
        tuning.target_rounds_per_cycle = 240;
        tuning.pacing_max_adjustment_bps = 4_000;

        let on_pace_count = count_mutations_for_cycle_progress(&tuning, 120, 6);
        let behind_count = count_mutations_for_cycle_progress(&tuning, 120, 0);

        assert!(
            behind_count > on_pace_count,
            "behind pace should boost mutation throughput (behind={}, on_pace={})",
            behind_count,
            on_pace_count
        );
    }

    #[test]
    fn test_stage_zero_evolution_requires_unlock() {
        let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();
        let slot = find_evolution_slot(dna, 1);
        let doge_mint = mock_pubkey();
        let user = mock_pubkey();
        let tuning = test_tuning();

        let unlocked = calculate_mutation_result(
            STORY_EVENT_ORIGIN_ROUND,
            1,
            1_000_000_000,
            1_000_000_000,
            1000,
            dna,
            0,
            1,
            0,
            1_000_000_000,
            &tuning,
            10_000,
            0,
            0,
            0,
            1_000_000_000,
            1_000_000_000,
            1_000_000_000,
            slot,
            &user,
            &doge_mint,
        );
        assert_eq!(unlocked.mutation_type, Some(MutationType::Evolution));

        let locked = calculate_mutation_result(
            STORY_EVENT_ORIGIN_ROUND,
            1,
            1_000_000_000,
            1_000_000_000,
            1000,
            dna,
            0,
            0,
            0,
            1_000_000_000,
            &tuning,
            10_000,
            0,
            0,
            0,
            1_000_000_000,
            1_000_000_000,
            1_000_000_000,
            slot,
            &user,
            &doge_mint,
        );
        assert_eq!(locked.mutation_type, Some(MutationType::Power));
    }

    #[test]
    fn test_stage_one_evolution_requires_next_unlock() {
        let mut dna = generate_genesis_dna(2, &mock_pubkey(), 200, 1).unwrap();
        dna[0] = (dna[0] & 0x8F) | (1 << 4);

        let slot = find_evolution_slot(dna, 2);
        let doge_mint = mock_pubkey();
        let user = mock_pubkey();
        let tuning = test_tuning();

        let unlocked = calculate_mutation_result(
            STORY_EVENT_ORIGIN_ROUND,
            1,
            1_000_000_000,
            1_000_000_000,
            1000,
            dna,
            0,
            2,
            0,
            1_000_000_000,
            &tuning,
            10_000,
            0,
            0,
            0,
            1_000_000_000,
            1_000_000_000,
            1_000_000_000,
            slot,
            &user,
            &doge_mint,
        );
        assert_eq!(unlocked.mutation_type, Some(MutationType::Evolution));

        let locked = calculate_mutation_result(
            STORY_EVENT_ORIGIN_ROUND,
            1,
            1_000_000_000,
            1_000_000_000,
            1000,
            dna,
            0,
            1,
            0,
            1_000_000_000,
            &tuning,
            10_000,
            0,
            0,
            0,
            1_000_000_000,
            1_000_000_000,
            1_000_000_000,
            slot,
            &user,
            &doge_mint,
        );
        assert_eq!(locked.mutation_type, Some(MutationType::Power));
    }

    // #[test]
    // fn test_mutation_result_xp_calculation() {
    //     let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();

    //     // 1 SOL bet = 1_000_000_000 lamports -> 1000 XP (capped)
    //     let result = calculate_mutation_result(
    //         1_000_000_000,
    //         1_000_000_000,
    //         100,
    //         dna,
    //         0,
    //         1_000_000_000,
    //         0,
    //         0,
    //         12345,
    //         &mock_pubkey(),
    //     );

    //     assert_eq!(result.xp_gained, 1000, "1 SOL should give 1000 XP (capped)");
    // }

    // #[test]
    // fn test_mutation_result_xp_small_bet() {
    //     let dna = generate_genesis_dna(1, &mock_pubkey(), 100, 0).unwrap();

    //     // 0.01 SOL = 10_000_000 lamports -> 10 XP
    //     let result = calculate_mutation_result(
    //         10_000_000,
    //         1_000_000_000,
    //         100,
    //         dna,
    //         0,
    //         1_000_000_000,
    //         0,
    //         0,
    //         12345,
    //         &mock_pubkey(),
    //     );

    //     assert_eq!(result.xp_gained, 10, "0.01 SOL should give 10 XP");
    // }

    // // --- BIT MANIPULATION TESTS ---

    // #[test]
    // fn test_get_set_trait_roundtrip() {
    //     let mut dna = [0u8; 32];

    //     // Test appearance traits (5 bits)
    //     for i in 0..21 {
    //         let val = (i * 3) as u8 % 32;
    //         set_trait_value(&mut dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8, val);
    //         let got = get_trait_value(&dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
    //         assert_eq!(got, val, "Appearance trait {} roundtrip failed", i);
    //     }

    //     // Test power traits (4 bits)
    //     for i in 0..15 {
    //         let val = (i * 2) as u8 % 16;
    //         set_trait_value(&mut dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8, val);
    //         let got = get_trait_value(&dna, COMBAT_OFFSET, POWER_TRAIT_BITS, i as u8);
    //         assert_eq!(got, val, "Power trait {} roundtrip failed", i);
    //     }
    // }

    // #[test]
    // fn test_trait_isolation() {
    //     let mut dna = [0u8; 32];

    //     // Set one trait, verify others unchanged
    //     set_trait_value(&mut dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, 5, 25);

    //     for i in 0..21 {
    //         let val = get_trait_value(&dna, APPEARANCE_OFFSET, APPEARANCE_TRAIT_BITS, i as u8);
    //         if i == 5 {
    //             assert_eq!(val, 25, "Target trait should be 25");
    //         } else {
    //             assert_eq!(val, 0, "Non-target trait {} should be 0", i);
    //         }
    //     }
    // }

    // // --- PRICE CALCULATION TESTS ---

    // --- BREED TESTS ---

    #[test]
    fn test_genesis_breed_in_range() {
        // Test many genesis doges to ensure breed is always 0-3
        for i in 0..100 {
            let dna = generate_genesis_dna(i, &mock_pubkey(), i * 7 + 100, (i % 12) as u8).unwrap();
            let breed = get_breed(&dna);
            assert!(breed <= 3, "Breed {} out of range for mint {}", breed, i);
        }
    }

    #[test]
    fn test_genesis_breed_has_variety() {
        // Generate many doges and check we get all 4 breed values
        let mut seen = [false; 4];
        for i in 0..200 {
            let dna = generate_genesis_dna(i, &mock_pubkey(), i * 13 + 42, 0).unwrap();
            let breed = get_breed(&dna) as usize;
            seen[breed] = true;
        }
        assert!(
            seen.iter().all(|&s| s),
            "Should see all 4 breed values across 200 mints, saw: {:?}",
            seen
        );
    }

    #[test]
    fn test_breed_preserved_through_evolution() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 5).unwrap();
        let original_breed = get_breed(&dna);
        let doge_mint = mock_pubkey();

        // Evolve through all stages
        for stage in 1..=7 {
            let seed = [stage as u8; 32];
            evolve_stage(
                STORY_EVENT_ORIGIN_ROUND,
                stage as u64,
                &mut dna,
                &seed,
                &doge_mint,
            );
            assert_eq!(
                get_breed(&dna),
                original_breed,
                "Breed changed during evolution to stage {}",
                stage
            );
        }
    }

    #[test]
    fn test_breed_preserved_through_visual_mutation() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 3).unwrap();
        let original_breed = get_breed(&dna);
        let doge_mint = mock_pubkey();

        for i in 0..50 {
            let seed = [i as u8; 32];
            mutate_visual_trait(
                STORY_EVENT_ORIGIN_ROUND,
                i as u64,
                &mut dna,
                &seed,
                &doge_mint,
            );
            assert_eq!(
                get_breed(&dna),
                original_breed,
                "Breed changed during visual mutation {}",
                i
            );
        }
    }

    #[test]
    fn test_breed_preserved_through_power_mutation() {
        let mut dna = generate_genesis_dna(1, &mock_pubkey(), 100, 7).unwrap();
        let original_breed = get_breed(&dna);
        let doge_mint = mock_pubkey();

        for i in 0..50 {
            let seed = [i as u8; 32];
            mutate_power_trait(
                STORY_EVENT_ORIGIN_ROUND,
                i as u64,
                &mut dna,
                &seed,
                &doge_mint,
            );
            assert_eq!(
                get_breed(&dna),
                original_breed,
                "Breed changed during power mutation {}",
                i
            );
        }
    }

    #[test]
    fn test_breed_inherited_in_breeding() {
        let parent1 = generate_genesis_dna(1, &mock_pubkey(), 100, 5).unwrap();
        let parent2 = generate_genesis_dna(2, &mock_pubkey(), 200, 5).unwrap();
        let p1_breed = get_breed(&parent1);
        let p2_breed = get_breed(&parent2);

        let offspring =
            breed_genes(&parent1, &parent2, b"breed_inherit_test_1234567890123").unwrap();
        let offspring_breed = get_breed(&offspring);

        assert!(
            offspring_breed == p1_breed || offspring_breed == p2_breed,
            "Offspring breed {} must be from parent1 ({}) or parent2 ({})",
            offspring_breed,
            p1_breed,
            p2_breed
        );
    }

    #[test]
    fn test_breed_breeding_both_parents_possible() {
        // Generate parents with different breeds by trying many seeds
        let mut saw_p1 = false;
        let mut saw_p2 = false;

        for i in 0..500 {
            let p1 = generate_genesis_dna(i * 2, &mock_pubkey(), i * 3, 0).unwrap();
            let p2 = generate_genesis_dna(i * 2 + 1, &mock_pubkey(), i * 5, 0).unwrap();
            if get_breed(&p1) == get_breed(&p2) {
                continue;
            }

            let seed_bytes = format!("seed_{:032}", i);
            let offspring = breed_genes(&p1, &p2, seed_bytes.as_bytes()).unwrap();
            let ob = get_breed(&offspring);

            if ob == get_breed(&p1) {
                saw_p1 = true;
            }
            if ob == get_breed(&p2) {
                saw_p2 = true;
            }
            if saw_p1 && saw_p2 {
                break;
            }
        }
        assert!(saw_p1 && saw_p2, "Should see inheritance from both parents");
    }

    // --- PRICE CALCULATION TESTS ---

    #[test]
    fn test_compute_gene_price_zero_minted() {
        let price = compute_gene_price(1_000_000, 100, 0).unwrap();
        assert_eq!(price, 1_000_000, "Zero minted should return base price");
    }

    #[test]
    fn test_compute_gene_price_increases() {
        let base = 1_000_000u64;
        let curve = 100u64;

        let price1 = compute_gene_price(base, curve, 10).unwrap();
        let price2 = compute_gene_price(base, curve, 100).unwrap();
        let price3 = compute_gene_price(base, curve, 1000).unwrap();
        println!("Price 1: {}", price1);
        println!("Price 2: {}", price2);
        println!("Price 3: {}", price3);

        assert!(price1 == 1000400, "Price should increase with mints");
        assert!(price2 == 1002100, "Price should keep increasing");
        assert!(price3 == 1010000, "Price should keep increasing");
    }

    // --- HELPER TRAIT FUNCTION TESTS ---

    #[test]
    fn test_enhance_trait() {
        println!("\n=== Testing enhance_trait ===");

        // Test appearance traits (max = 31)
        // Case 1: rand < 48, max_t < max -> should enhance max_t
        let result1 = enhance_trait(10, 5, 10, APPEARANCE_MAX);
        println!(
            "enhance_trait(10, 5, 10, {}) = {} (should be 11)",
            APPEARANCE_MAX, result1
        );
        assert_eq!(result1, 11, "Should enhance max trait when rand < 48");

        // Case 2: max_t at max, but can enhance min_t
        let result2 = enhance_trait(31, 25, 10, APPEARANCE_MAX);
        println!(
            "enhance_trait(31, 25, 10, {}) = {} (enhances min_t)",
            APPEARANCE_MAX, result2
        );
        assert_eq!(result2, 26, "Should enhance min_t when max_t is at max");

        // Case 2b: Both at max -> should return max_t
        let result2b = enhance_trait(31, 31, 10, APPEARANCE_MAX);
        println!(
            "enhance_trait(31, 31, 10, {}) = {} (should be 31)",
            APPEARANCE_MAX, result2b
        );
        assert_eq!(result2b, APPEARANCE_MAX, "Both at max should return max");

        // Case 3: rand 48-95, min_t < max_t -> should enhance min_t
        let result3 = enhance_trait(5, 10, 60, APPEARANCE_MAX);
        println!(
            "enhance_trait(5, 10, 60, {}) = {} (should be 6)",
            APPEARANCE_MAX, result3
        );
        assert_eq!(result3, 6, "Should enhance min trait when rand 48-95");

        // Case 4: rand >= 96 -> should return max_t
        let result4 = enhance_trait(8, 12, 100, APPEARANCE_MAX);
        println!(
            "enhance_trait(8, 12, 100, {}) = {} (should be 12)",
            APPEARANCE_MAX, result4
        );
        assert_eq!(result4, 12, "Should return max_t when rand >= 96");

        // Test power traits (max = 15)
        let result5 = enhance_trait(3, 5, 20, COMBAT_MAX);
        println!(
            "enhance_trait(3, 5, 20, {}) = {} (should be 6)",
            COMBAT_MAX, result5
        );
        assert_eq!(result5, 6, "Power trait enhancement");

        let result6 = enhance_trait(15, 10, 10, COMBAT_MAX);
        println!(
            "enhance_trait(15, 10, 10, {}) = {} (enhances min_t)",
            COMBAT_MAX, result6
        );
        assert_eq!(result6, 11, "Should enhance min_t when max_t is at max");

        let result6b = enhance_trait(15, 15, 10, COMBAT_MAX);
        println!(
            "enhance_trait(15, 15, 10, {}) = {} (should be 15)",
            COMBAT_MAX, result6b
        );
        assert_eq!(result6b, COMBAT_MAX, "Both at max should return max");
    }

    #[test]
    fn test_synergy_boost() {
        println!("\n=== Testing synergy_boost ===");

        // Test power traits (max = 15)
        // Case 1: Close traits (diff <= 2), rand < 64, max_t < max -> should boost
        let result1 = synergy_boost(10, 11, 30, COMBAT_MAX);
        println!(
            "synergy_boost(10, 11, 30, {}) = {} (should be 12)",
            COMBAT_MAX, result1
        );
        assert_eq!(result1, 12, "Close traits should boost");

        // Case 2: Close traits but at max -> should not exceed
        let result2 = synergy_boost(15, 14, 30, COMBAT_MAX);
        println!(
            "synergy_boost(15, 14, 30, {}) = {} (should be 15)",
            COMBAT_MAX, result2
        );
        assert_eq!(result2, COMBAT_MAX, "Should not exceed max");

        // Case 3: Far traits (diff > 2) -> should return max_t
        let result3 = synergy_boost(5, 15, 30, COMBAT_MAX);
        println!(
            "synergy_boost(5, 15, 30, {}) = {} (should be 15)",
            COMBAT_MAX, result3
        );
        assert_eq!(result3, 15, "Far traits take max");

        // Case 4: Close traits but rand >= 64 -> should return max_t
        let result4 = synergy_boost(8, 9, 100, COMBAT_MAX);
        println!(
            "synergy_boost(8, 9, 100, {}) = {} (should be 9)",
            COMBAT_MAX, result4
        );
        assert_eq!(result4, 9, "High rand should return max_t");

        // Test appearance traits (max = 31)
        let result5 = synergy_boost(20, 22, 40, APPEARANCE_MAX);
        println!(
            "synergy_boost(20, 22, 40, {}) = {} (should be 23)",
            APPEARANCE_MAX, result5
        );
        assert_eq!(result5, 23, "Appearance trait synergy boost");

        let result6 = synergy_boost(25, 30, 50, APPEARANCE_MAX);
        println!(
            "synergy_boost(25, 30, 50, {}) = {} (should be 30)",
            APPEARANCE_MAX, result6
        );
        assert_eq!(result6, 30, "Diff > 2 should return max_t");
    }

    #[test]
    fn test_mutate_trait_value() {
        println!("\n=== Testing mutate_trait_value ===");

        // Test appearance traits (max = 31)
        // Case 1: rand == 0 -> significant mutation (but capped)
        let result1 = mutate_trait_value(15, 0, APPEARANCE_MAX);
        println!(
            "mutate_trait_value(15, 0, {}) = {} (rand==0 mutation)",
            APPEARANCE_MAX, result1
        );
        assert!(
            result1 < APPEARANCE_MAX,
            "Rand==0 should produce value <= max-1"
        );

        // Case 2: rand 1-47, base > 0 -> decrease
        let result2 = mutate_trait_value(10, 20, APPEARANCE_MAX);
        println!(
            "mutate_trait_value(10, 20, {}) = {} (should be 9)",
            APPEARANCE_MAX, result2
        );
        assert_eq!(result2, 9, "Should decrease when rand < 48 and base > 0");

        // Case 3: rand 48-79, base < max -> increase
        let result3 = mutate_trait_value(10, 60, APPEARANCE_MAX);
        println!(
            "mutate_trait_value(10, 60, {}) = {} (should be 11)",
            APPEARANCE_MAX, result3
        );
        assert_eq!(
            result3, 11,
            "Should increase when rand 48-79 and base < max"
        );

        // Case 4: rand >= 80 -> no change
        let result4 = mutate_trait_value(15, 100, APPEARANCE_MAX);
        println!(
            "mutate_trait_value(15, 100, {}) = {} (should be 15)",
            APPEARANCE_MAX, result4
        );
        assert_eq!(result4, 15, "Should not change when rand >= 80");

        // Case 5: At max -> should not exceed
        let result5 = mutate_trait_value(31, 60, APPEARANCE_MAX);
        println!(
            "mutate_trait_value(31, 60, {}) = {} (should be 31)",
            APPEARANCE_MAX, result5
        );
        assert_eq!(result5, APPEARANCE_MAX, "Should not exceed max");

        // Test power traits (max = 15)
        let result6 = mutate_trait_value(5, 30, COMBAT_MAX);
        println!(
            "mutate_trait_value(5, 30, {}) = {} (should be 4)",
            COMBAT_MAX, result6
        );
        assert_eq!(result6, 4, "Power trait decrease");

        let result7 = mutate_trait_value(8, 70, COMBAT_MAX);
        println!(
            "mutate_trait_value(8, 70, {}) = {} (should be 9)",
            COMBAT_MAX, result7
        );
        assert_eq!(result7, 9, "Power trait increase");
    }
}
