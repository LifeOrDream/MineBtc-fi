use anchor_lang::prelude::*;
use crate::constants::*;

// ========================================================================================
// =============================== GLOBAL STATE ========================================== 
// ========================================================================================

/// Global configuration for the NFT Launchpad
#[account]
pub struct GlobalConfig {
    /// Program authority (can update config and mint NFTs)
    pub authority: Pubkey,

    /// Treasury account for collecting SOL fees
    pub treasury: Pubkey,

    /// Dragon Egg collection mint (Metaplex Core)
    pub dragon_egg_collection: Pubkey,

    /// Total Dragon Egg NFTs minted
    pub total_dragon_eggs_minted: u64,

    /// Total SOL collected from sales
    pub total_sol_collected: u64,

    /// Available Dragon Egg URIs (randomly selected on mint)
    pub dragon_egg_uris: Vec<String>,

    /// PDA bump seeds
    pub config_bump: u8,
    pub treasury_bump: u8,
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +  // authority
        32 +  // treasury
        32 +  // dragon_egg_collection
        8 +   // total_dragon_eggs_minted
        8 +   // total_sol_collected
        4 + (10 * (4 + MAX_URI_LENGTH)) +  // dragon_egg_uris (max 10 URIs)
        1 +   // config_bump
        1;    // treasury_bump
    
    /// Select random Dragon Egg URI based on slot, index, and DNA
    pub fn get_random_dragon_egg_uri(&self, slot: u64, index: u64, dna: &[u8; 32]) -> Result<String> {
        require!(!self.dragon_egg_uris.is_empty(), crate::errors::NftLaunchpadError::InvalidMetadata);

        let dna_seed = u64::from_le_bytes([dna[0], dna[1], dna[2], dna[3], dna[4], dna[5], dna[6], dna[7]]);
        let random_index = (slot.wrapping_add(index).wrapping_add(dna_seed)) as usize % self.dragon_egg_uris.len();
        Ok(self.dragon_egg_uris[random_index].clone())
    }
}

// ========================================================================================
// =============================== DRAGON EGG NFT STATE ==================================
// ========================================================================================

/// Dragon Egg NFT metadata (stored separately from Metaplex Core asset)
/// NOTE: Owner is NOT stored here - always derive from Metaplex Core asset (source of truth)
#[account]
pub struct DragonEggMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    
    /// Current power level
    pub power: u32,
    
    /// DNA data (32 bytes for breeding/evolution)
    pub dna: [u8; 32],
    
    /// Moonbase this egg is incubated in (if any)
    pub incubated_moonbase: Option<Pubkey>,
    
    /// Last power update timestamp
    pub last_update_ts: i64,
    
    /// Total hashpower accumulated
    pub total_hashpower_accumulated: u64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// PDA bump
    pub bump: u8,
}

impl DragonEggMetadata {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // mint
        4 +     // power
        32 +    // dna
        33 +    // incubated_moonbase (Option<Pubkey>)
        8 +     // last_update_ts
        8 +     // total_hashpower_accumulated
        8 +     // created_at
        1;      // bump
    
    /// Calculate power increase based on hashpower and time
    pub fn calculate_power_increase(
        &self,
        total_hashpower: u64,
        total_eggs: u8,
        time_elapsed: i64,
    ) -> u32 {
        if total_eggs == 0 || time_elapsed <= 0 {
            return 0;
        }
        
        // Formula: power_increase = (total_hashpower / total_eggs) * time_elapsed / POWER_RATE_MULTIPLIER
        let hashpower_per_egg = total_hashpower.saturating_div(total_eggs as u64);
        let power_increase = hashpower_per_egg
            .saturating_mul(time_elapsed as u64)
            .saturating_div(POWER_RATE_MULTIPLIER) as u32;
        
        // Cap at max power
        power_increase.min(MAX_EGG_POWER.saturating_sub(self.power))
    }
}

// ========================================================================================
// =============================== MOONBASE INCUBATION STATE =============================
// ========================================================================================

/// Incubation state for a specific moonbase (tracks all incubated eggs)
#[account]
pub struct IncubationState {
    /// Moonbase owner
    pub moonbase_owner: Pubkey,

    /// List of incubated egg mints
    pub incubated_eggs: Vec<Pubkey>,

    /// Last update timestamp
    pub last_update_ts: i64,

    /// Total power accumulated across all eggs
    pub total_power: u64,

    /// PDA bump
    pub bump: u8,
}

impl IncubationState {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // moonbase_owner
        4 + (MAX_EGGS_PER_MOONBASE as usize * 32) + // incubated_eggs vec
        8 +     // last_update_ts
        8 +     // total_power
        1;      // bump
}

// ========================================================================================
// =============================== DNA UTILITIES =========================================
// ========================================================================================

/// Generate random DNA for new dragon eggs
pub fn generate_dragon_egg_dna(slot: u64, owner: &Pubkey, index: u64) -> [u8; 32] {
    let mut dna = [0u8; 32];
    
    // Use slot, owner, and index as entropy sources
    let seed = slot
        .wrapping_add(index)
        .wrapping_mul(1103515245)
        .wrapping_add(12345);
    
    for i in 0..32 {
        let random_value = seed
            .wrapping_mul(owner.to_bytes()[i % 32] as u64)
            .wrapping_add(i as u64 * 31)
            .wrapping_mul(1103515245);
        
        dna[i] = (random_value & 0xFF) as u8;
    }
    
    dna
}

/// Calculate rarity score from DNA
pub fn calculate_dna_rarity(dna: &[u8; 32]) -> u32 {
    let mut rarity = 0u32;
    
    for &byte in dna.iter() {
        // High values are rarer
        if byte > 200 {
            rarity += 100;
        }
        if byte > 240 {
            rarity += 200;
        }
        if byte == 255 {
            rarity += 500;
        }
    }
    
    rarity
}
