use anchor_lang::prelude::*;

// ========================================================================================
// ============================== PROGRAM INITIALIZATION EVENTS ========================== 
// ========================================================================================

#[event]
pub struct ProgramInitialized {
    pub authority: Pubkey,
    pub moondoge_collection: Pubkey,
    pub dragon_egg_collection: Pubkey,
}

#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub new_authority: Option<Pubkey>,
    pub new_treasury: Option<Pubkey>,
}

// ========================================================================================
// =============================== MOONDOGE NFT EVENTS ===================================
// ========================================================================================

#[event]
pub struct DogeBtcMinted {
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub price_paid: u64,
}

#[event]
pub struct DogeBtcAttached {
    pub doge_mint: Pubkey,
    pub moonbase_owner: Pubkey,
    pub attached_at: i64,
}

#[event]
pub struct DogeBtcDetached {
    pub doge_mint: Pubkey,
    pub moonbase_owner: Pubkey,
    pub detached_at: i64,
    pub final_money: u64,
}

#[event]
pub struct DogeBtcMoneyUpdated {
    pub doge_mint: Pubkey,
    pub old_money: u64,
    pub new_money: u64,
    pub money_increase: u64,
    pub dbtc_mined: u64,
}

// ========================================================================================
// =============================== DRAGON EGG NFT EVENTS =================================
// ========================================================================================

#[event]
pub struct DragonEggMinted {
    pub mint: Pubkey,
    pub name: String,
    pub uri: String,
    pub dna: [u8; 32],
    pub initial_power: u32,
    pub price_paid: u64,
}

#[event]
pub struct DragonEggIncubated {
    pub egg_mint: Pubkey,
    pub moonbase_owner: Pubkey,
    pub incubated_at: i64,
    pub total_eggs_in_moonbase: u8,
}

#[event]
pub struct DragonEggRemoved {
    pub egg_mint: Pubkey,
    pub moonbase_owner: Pubkey,
    pub removed_at: i64,
    pub final_power: u32,
    pub remaining_eggs_in_moonbase: u8,
}

#[event]
pub struct DragonEggPowerUpdated {
    pub egg_mint: Pubkey,
    pub old_power: u32,
    pub new_power: u32,
    pub power_increase: u32,
    pub hashpower_accumulated: u64,
}

// ========================================================================================
// ============================== MOONBASE CREATION EVENTS ===============================
// ========================================================================================

#[event]
pub struct MoonbaseCreatedWithNfts {
    pub moonbase_owner: Pubkey,
    pub pricing_tier: String, // "basic", "doge", "full"
    pub sol_paid: u64,
    pub moondoge_minted: Option<Pubkey>,
    pub dragon_egg_minted: Option<Pubkey>,
}

// ========================================================================================
// =============================== BATCH OPERATIONS EVENTS ===============================
// ========================================================================================

#[event]
pub struct BatchPowerUpdate {
    pub moonbase_owner: Pubkey,
    pub eggs_updated: u8,
    pub total_power_increase: u64,
    pub time_elapsed: i64,
}

#[event]
pub struct IncubationStateUpdated {
    pub moonbase_owner: Pubkey,
    pub total_eggs: u8,
    pub total_power: u64,
    pub last_update_ts: i64,
}

// ========================================================================================
// =============================== ECONOMIC EVENTS =======================================
// ========================================================================================

#[event]
pub struct SOLFeesCollected {
    pub source: String, // "moonbase_creation", "nft_purchase"
    pub amount: u64,
    pub pricing_tier: Option<String>,
}

// ========================================================================================
// =============================== ANALYTICS EVENTS ======================================
// ========================================================================================

#[event]
pub struct CollectionStatsUpdated {
    pub total_moondoges_minted: u64,
    pub total_dragon_eggs_minted: u64,
    pub total_sol_collected: u64,
}
