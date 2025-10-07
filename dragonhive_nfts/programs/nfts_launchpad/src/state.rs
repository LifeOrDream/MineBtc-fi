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
    
    /// MoonDoge collection mint (Metaplex Core)
    pub moondoge_collection: Pubkey,
    
    /// Dragon Egg collection mint (Metaplex Core)
    pub dragon_egg_collection: Pubkey,
    
    /// Total MoonDoge NFTs minted
    pub total_moondoges_minted: u64,
    
    /// Total Dragon Egg NFTs minted
    pub total_dragon_eggs_minted: u64,
    
    /// Total SOL collected from sales
    pub total_sol_collected: u64,
    
    /// Whether the program is paused
    pub is_paused: bool,
    
    /// PDA bump seeds
    pub config_bump: u8,
    pub treasury_bump: u8,
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 
        32 +  // authority
        32 +  // treasury  
        32 +  // moondoge_collection
        32 +  // dragon_egg_collection
        8 +   // total_moondoges_minted
        8 +   // total_dragon_eggs_minted
        8 +   // total_sol_collected
        1 +   // is_paused
        1 +   // config_bump
        1;    // treasury_bump
}

// ========================================================================================
// =============================== MOONDOGE NFT STATE ====================================
// ========================================================================================

/// MoonDoge NFT metadata (stored separately from Metaplex Core asset)
#[account]
pub struct MoonDogeMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    
    /// Current owner
    pub owner: Pubkey,
    
    /// Money accumulated (increases with mDOGE mining)
    pub money: u64,
    
    /// Moonbase this doge is attached to (if any)
    pub attached_moonbase: Option<Pubkey>,
    
    /// Last update timestamp
    pub last_update_ts: i64,
    
    /// Total mDOGE mined while this doge was attached
    pub total_mdoge_mined: u64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// PDA bump
    pub bump: u8,
}

impl MoonDogeMetadata {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // mint
        32 +    // owner
        8 +     // money
        33 +    // attached_moonbase (Option<Pubkey>)
        8 +     // last_update_ts
        8 +     // total_mdoge_mined
        8 +     // created_at
        1;      // bump
    
    /// Calculate money accumulation based on mDOGE mined
    pub fn calculate_money_increase(&self, mdoge_mined: u64) -> u64 {
        // Formula: money_increase = (mdoge_mined * MONEY_RATE_MULTIPLIER) / 1_000_000
        mdoge_mined
            .saturating_mul(MONEY_RATE_MULTIPLIER)
            .saturating_div(1_000_000)
            .min(MAX_DOGE_MONEY.saturating_sub(self.money))
    }
}

// ========================================================================================
// =============================== DRAGON EGG NFT STATE ==================================
// ========================================================================================

/// Dragon Egg NFT metadata (stored separately from Metaplex Core asset)
#[account]
pub struct DragonEggMetadata {
    /// The NFT mint address (Metaplex Core asset)
    pub mint: Pubkey,
    
    /// Current owner
    pub owner: Pubkey,
    
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
        32 +    // owner
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
// =============================== DOGE ATTACHMENT STATE =================================
// ========================================================================================

/// Doge attachment state for a specific moonbase (1 doge max per moonbase)
#[account]
pub struct DogeAttachment {
    /// Moonbase owner
    pub moonbase_owner: Pubkey,
    
    /// Attached MoonDoge mint
    pub doge_mint: Pubkey,
    
    /// Last update timestamp
    pub last_update_ts: i64,
    
    /// Last recorded mDOGE balance (for tracking delta)
    pub last_mdoge_balance: u64,
    
    /// PDA bump
    pub bump: u8,
}

impl DogeAttachment {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // moonbase_owner
        32 +    // doge_mint
        8 +     // last_update_ts
        8 +     // last_mdoge_balance
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
