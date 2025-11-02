use anchor_lang::prelude::*;
use crate::state::ModuleType;

// ------------------------------
// User management events
// ------------------------------

#[event]
pub struct UserMoonBaseCreated {
    pub owner: Pubkey,
    pub referrer: Option<Pubkey>,
}

#[event]
pub struct ReferralRewardsAdded {
    pub referrer: Pubkey,
    pub referred_user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ReferralRewardsClaimed {
    pub owner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ElectricityUpdated {
    pub owner: Pubkey,
    pub to_increase: bool,
    pub amount: u64,
    pub new_available_electricity: u64,
    pub new_total_electricity: u64,
}

// ------------------------------
// Module management events
// ------------------------------

#[event]
pub struct ModuleInstanceCreated {
    pub owner: Pubkey,
    pub config_id: u16,
    pub module_index: u8,
    pub cost: u64,
    pub referral_fee: u64,
}

#[event]
pub struct ModuleInstanceUpgraded {
    pub owner: Pubkey,
    pub module_index: u8,
    pub new_upgrade_level: u8,
    pub cost: u64,
    pub referral_fee: u64,
}

#[event]
pub struct ModuleInstanceRemoved {
    pub owner: Pubkey,
    pub module_index: u8,
    pub module_type: ModuleType,
    pub position_x: u8,
    pub position_y: u8,
    pub electricity_freed: u64,
    pub hashpower_lost: u64,
}

#[event]
pub struct ModuleInstanceReinstalled {
    pub owner: Pubkey,
    pub module_index: u8,
    pub config_id: u16,
    pub position_x: u8,
    pub position_y: u8,
    pub electricity_used: u64,
    pub hashpower_restored: u64,
    pub upgrade_level: u8,
}

#[event]
pub struct ModulePurchased {
    pub owner: Pubkey,
    pub config_id: u16,
    pub module_index: u8,
    pub cost: u64,
    pub referral_fee: u64,
}

#[event]
pub struct ModuleInstalled {
    pub owner: Pubkey,
    pub config_id: u16,
    pub module_index: u8,
    pub pos_x: u8,
    pub pos_y: u8,
}

#[event]
pub struct ModuleDeleted {
    pub owner: Pubkey,
    pub config_id: u16,
    pub remaining_count: u8,
}

// ------------------------------
// Mining and facility events
// ------------------------------

#[event]
pub struct FacilityCreated {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
}

#[event]
pub struct FacilityUpgraded {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
    pub new_size: u8,
}

#[event]
pub struct DogeAssigned {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
    pub doge_mint: Pubkey,
}

#[event]
pub struct DogeUnassigned {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
    pub doge_mint: Pubkey,
}

#[event]
pub struct MiningRigPurchased {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
    pub rig_type: u8,
    pub rig_id: u32,
    pub price: u64,
    
}

#[event]
pub struct MiningRigDeployed {
    /// Auto-indexed field
    pub owner: Pubkey,
    pub facility: Pubkey,
    pub rig_id: u32,
    pub rig_type: u8,
    pub x: u8,
    pub y: u8,
    
}

#[event]
pub struct MiningRigListed {
    /// Auto-indexed field
    pub seller: Pubkey,
    pub facility: Pubkey,
    pub position: u8,
    pub rig_id: u32,
    pub price: u64,
    
}

#[event]
pub struct MiningRigUnlisted {
    /// Auto-indexed field
    pub seller: Pubkey,
    pub facility: Pubkey,
    pub position: u8,
    pub rig_id: u32,
    
}

#[event]
pub struct MiningRigSold {
    /// Auto-indexed field
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub facility: Pubkey,
    pub position: u8,
    pub rig_id: u32,
    pub price: u64,
    
}

#[event]
pub struct MiningInitialized {
    pub token_mint: Pubkey,
    pub initial_reward_per_block: u64,
    pub halving_interval: i64,
    pub reward_period: i64,
    
}

#[event]
pub struct SolFeesWithdrawn {
    pub fee_collector: Pubkey,
    pub amount: u64,
    pub loot_amount: u64,
}

// ------------------------------
// Config management events
// ------------------------------

#[event]
pub struct NewModuleConfigCreated {
    pub id: u16,
    pub name: String,    
}

#[event]
pub struct ModuleConfigUpdated {
    pub id: u16,
    pub name: String,    
}


#[event]
pub struct MiningTokenVaultSet {
    /// The authority that set the token vault
    pub authority: Pubkey,
    /// The token vault address
    pub token_vault: Pubkey,
    /// The token vault authority address
    pub token_vault_authority: Pubkey,
    /// Timestamp of when mining started
    pub mining_start_timestamp: u64,
}

// Global Config events
#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub sol_claimer: Pubkey,
}
 
#[event]
pub struct DogeBtcTokensClaimed {
    pub owner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct MiningRewardsProcessed {
    pub owner: Pubkey,
    pub hashpower: u64,
    pub tokens_earned: u64,
}

#[event]
pub struct MiningHalveningOccurred {
    pub slot: u64,
    pub new_rate: u64,
    pub next_halvening_slot: u64,
}

#[event]
pub struct UserElectricityUpdated {
    pub user: Pubkey,
    pub previous_amount: u64,
    pub new_amount: u64,
    pub is_increase: bool,
}



// ------------------------------
// Dynamic distribution events
// ------------------------------

#[event]
pub struct RaydiumPoolSet {
    pub authority: Pubkey,
    pub pool_state: Pubkey,
}

#[event]
pub struct DistributionRateUpdated {
    pub old_rate: u64,
    pub new_rate: u64,
    pub price_change_pct: i32,
    pub current_price: u64,
    pub avg_price_4h: u64,
    pub track_price: u64,
    pub recent_price: u64,
    pub rate_changed: bool,
    pub sol_received: u64,
    pub timestamp: i64,
}

#[event]
pub struct SlotsPerHourUpdated {
    pub authority: Pubkey,
    pub old_slots_for_swap: u64,
    pub new_slots_for_swap: u64,
}

#[event]
pub struct LpTokensBurned {
    pub lp_tokens_burned: u64,
    pub total_lp_burnt: u64,
    pub dbtc_amount_added: u64,
    pub sol_amount_added: u64,
    pub timestamp: i64,
}

// ------------------------------
// XP and Level system events
// ------------------------------

#[event]
pub struct LevelUp {
    pub owner: Pubkey,
    pub new_level: u8,
    pub total_xp: u32,
}

#[event]
pub struct XpGained {
    pub owner: Pubkey,
    pub xp_amount: u32,
    pub xp_source: String,
    pub total_xp: u32,
}

#[event]
pub struct DailyLoginReward {
    pub owner: Pubkey,
    pub streak: u16,
    pub xp_gained: u32,
}

// ========== LOOT REWARDS EVENTS ========== //

#[event]
pub struct LootRewardsAccumulated {
    pub dbtc_amount: u64,
    pub sol_amount: u64,
    pub total_dbtc_accumulated: u64,
    pub total_sol_accumulated: u64,
}
 

#[event]
pub struct LootRewardsInitialized {
    pub loot_rewards_pda: Pubkey,
    pub sol_vault_pda: Pubkey,
    pub dbtc_vault_pda: Pubkey,
}
 

#[event]
pub struct LevelStatsUpdated {
    pub user: Pubkey,
    pub old_level: u8,
    pub new_level: u8,
    pub total_users: u32,
    pub users_at_new_level: u32,
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
pub struct DragonEggCollectionCreated {
    pub collection: Pubkey,
    pub update_authority: Pubkey,
    pub name: String,
    pub uri: String,
}

#[event]
pub struct LevelStatsInitialized {
    pub level_stats_pda: Pubkey,
    pub tracked_levels: u8,
}

// ========== MOONBASE EXPANSION EVENTS ========== //

#[event]
pub struct MoonbaseExpanded {
    pub owner: Pubkey,
    pub expansion_id: u8,
    pub expansion_name: String,
    pub old_width: u8,
    pub old_height: u8,
    pub new_width: u8,
    pub new_height: u8,
    pub area_increase: u32,
    pub xp_gained: u32,
    pub cost_paid: u64,
}

#[event]
pub struct ExpansionAdded {
    pub authority: Pubkey,
    pub expansion_id: u8,
    pub expansion_name: String,
    pub required_level: u8,
    pub cost_sol: u64,
    pub new_width: u8,
    pub new_height: u8,
}

// ------------------------------
// Enhanced Loot System Events
// ------------------------------

#[event]
pub struct LootWon {
    pub owner: Pubkey,
    pub level: u8,
    pub sol: u64,
    pub mdoge: u64,
    pub loot_tier: String,        // "minor", "rare", "legendary"
    pub exclusivity_rank: u8,     // 0 = first, 1-2 = top3, etc.
    pub chance_percentage: u32,   // Actual chance they had (in basis points)
}

#[event]
pub struct ReferralSuccess {
    pub referrer: Pubkey,
    pub referee: Pubkey,
    pub xp_bonus: u32,
    pub sol_earned_bonus: u32,    // Additional XP from total SOL earned
}

// ========== PVP GAME EVENTS ========== //
 
#[event]
pub struct AttractionXPClaimed {
    pub owner: Pubkey,
    pub module_index: u8,
    pub xp_claimed: u32,
    pub hours_elapsed: f64,
    pub effective_xp_per_hour: u32,
}

 