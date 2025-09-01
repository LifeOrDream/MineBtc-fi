use anchor_lang::prelude::*;

// ========================================================================================
// ============================== PROGRAM INITIALIZATION EVENTS ======================== 
// ========================================================================================

#[event]
pub struct ProgramInitialized {
    pub authority: Pubkey,
    pub honey_token_mint: Pubkey,
    pub collection_mint: Pubkey,
    pub nft_price: u64,
    pub breeding_fee: u64,
}

#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub new_authority: Option<Pubkey>,
    pub new_treasury: Option<Pubkey>,
    pub new_nft_price: Option<u64>,
    pub new_breeding_fee: Option<u64>,
}

// ========================================================================================
// =============================== NFT LIFECYCLE EVENTS ================================== 
// ========================================================================================

#[event]
pub struct DragonBeeGenesisMinted {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub bee_type: u8,
    pub genes: [u8; 32],
    pub initial_power: u32,
}

#[event]
pub struct DragonBeePurchased {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub price_paid: u64,
    pub total_minted: u64,
}

#[event]
pub struct DragonBeeEvolved {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub old_evolution_stage: u8,
    pub new_evolution_stage: u8,
    pub old_power: u32,
    pub new_power: u32,
    pub new_genes: [u8; 32],
}

#[event]
pub struct DragonBeeKilled {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub power: u32,
    pub dragon_tokens_earned: u64,
    pub remaining_pool: u64,
}

#[event]
pub struct DragonBeeStatsUpdated {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub old_power: u32,
    pub new_power: u32,
    pub power_increase: u32,
    pub new_uri: Option<String>,
    pub game_interactions: u32,
}

// ========================================================================================
// =============================== QUEEN AUCTION EVENTS ================================== 
// ========================================================================================

#[event]
pub struct NewBeeAddedToCompetition {
    pub trainer_addr: Pubkey,
    pub username: String,
    pub version: Pubkey, // DragonBee mint
    pub family_type: u8,
    pub bid_amt: u64,
    pub tax_amt: u64,
    pub auction_start_at: u64,
}

#[event]
pub struct BidUpdatedByUser {
    pub trainer_addr: Pubkey,
    pub username: String,
    pub bid_amt: u64,
    pub tax_amt: u64,
    pub flag: bool,
    pub auction_start_at: u64,
}

#[event]
pub struct LeadingDragonBeeUpdated {
    pub auction_start_epoch: u64,
    pub family_type: u8,
    pub version: Pubkey, // DragonBee mint
    pub bid_amount: u64,
    pub trainer_addr: Pubkey,
    pub username: String,
}

#[event]
pub struct BidsOpenForExisting {
    pub auction_start_epoch: u64,
    pub price_to_be_a_queen: u64,
    pub cur_epoch: u64,
    pub deposits_open_till: i64,
}

#[event]
pub struct BidsClosed {
    pub auction_start_epoch: u64,
}

#[event]
pub struct QueenCompetitionOver {
    pub started_at_epoch: u64,
    pub next_event_from: u64,
    pub hive_burnt_amt: u64,
    pub total_sui_bidded: u64,
    pub energy_from_queens: u64,
    pub community_energy: u64,
    pub becoming_queen_expensive_by: u64,
    pub price_to_be_a_queen: u64,
}

#[event]
pub struct DragonBeesBred {
    pub parent1_mint: Pubkey,
    pub parent2_mint: Pubkey,
    pub offspring_mint: Pubkey,
    pub breeder: Pubkey,
    pub offspring_genes: [u8; 32],
    pub offspring_power: u32,
    pub generation: u32,
    pub breeding_fee_paid: u64,
}

#[event]
pub struct QueenBeeSet {
    pub queen_mint: Pubkey,
    pub queen_owner: Pubkey,
    pub breeding_price: u64,
    pub auction_start_time: i64,
    pub auction_end_time: i64,
}

#[event]
pub struct QueenBreedingBidPlaced {
    pub queen_mint: Pubkey,
    pub bidder: Pubkey,
    pub bid_amount: u64,
    pub previous_highest_bid: u64,
    pub previous_bidder: Option<Pubkey>,
}

#[event]
pub struct QueenAuctionFinalized {
    pub queen_mint: Pubkey,
    pub winner: Pubkey,
    pub winning_bid: u64,
    pub breeding_price_set: u64,
    pub total_bids: u32,
}

#[event]
pub struct QueenBreedingCompleted {
    pub queen_mint: Pubkey,
    pub parent_mint: Pubkey,
    pub offspring_mint: Pubkey,
    pub breeder: Pubkey,
    pub queen_owner: Pubkey,
    pub breeding_price_paid: u64,
    pub queen_power_bonus: u32,
}

// ========================================================================================
// =============================== USER EVENTS =========================================== 
// ========================================================================================

#[event]
pub struct UserProfileCreated {
    pub owner: Pubkey,
    pub created_at: i64,
}

#[event]
pub struct UserProfileUpdated {
    pub owner: Pubkey,
    pub total_dragonbees: u32,
    pub total_sol_spent: u64,
    pub total_breeding_fees: u64,
    pub dragonbees_killed: u32,
    pub dragon_tokens_earned: u64,
}

// ========================================================================================
// =============================== HONEY CONFIG EVENTS ==================================== 
// ========================================================================================

#[event]
pub struct HoneyConfigInitialized {
    pub main_admin: Pubkey,
    pub ext_authority: Pubkey,
    pub honey_token_mint: Pubkey,
    pub honey_vault: Pubkey,
    pub burn_account: Pubkey,
    pub staking_rewards_account: Pubkey,
    pub initial_distribution_rate: u64,
    pub for_game_percentage: u16,
}

#[event]
pub struct HoneyConfigUpdated {
    pub main_admin: Pubkey,
    pub new_main_admin: Option<Pubkey>,
    pub new_ext_authority: Option<Pubkey>,
    pub is_paused: Option<bool>,
}

#[event]
pub struct DistributionConfigUpdated {
    pub distribution_admin: Pubkey,
    pub new_distribution_admin: Option<Pubkey>,
    pub new_distribution_rate: Option<u64>,
    pub new_game_recipient: Option<Pubkey>,
    pub new_amm_recipient: Option<Pubkey>,
    pub new_for_game_percentage: Option<u16>,
}

// ========================================================================================
// =============================== ECONOMIC EVENTS ======================================= 
// ========================================================================================

#[event]
pub struct HoneyTokensDeposited {
    pub depositor: Pubkey,
    pub amount: u64,
    pub total_vault_balance: u64,
}

#[event]
pub struct HoneyTokensAddedToBurn {
    pub user: Pubkey,
    pub amount: u64,
    pub total_burn_balance: u64,
}

#[event]
pub struct HoneyTokensBurned {
    pub caller: Pubkey,
    pub amount: u64,
    pub total_burned: u64,
    pub remaining_burn_balance: u64,
}

#[event]
pub struct HoneyTokensAddedToStaking {
    pub user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct StakingRewardsClaimed {
    pub claimer: Pubkey,
    pub amount: u64,
    pub remaining_rewards: u64,
}

#[event]
pub struct HoneyTokensDistributed {
    pub caller: Pubkey,
    pub total_distributed: u64,
    pub game_amount: u64,
    pub dev_amount: u64,
    pub amm_amount: u64,
    pub game_recipient: Pubkey,
    pub dev_recipient: Pubkey,
    pub amm_recipient: Pubkey,
    pub remaining_vault_balance: u64,
}

#[event]
pub struct SOLFeesCollected {
    pub source: String, // "nft_sale", "breeding_fee", "queen_auction"
    pub amount: u64,
    pub team_portion: u64,
    pub buyback_portion: u64,
    pub kill_pool_portion: u64,
}

#[event]
pub struct DragonTokensBought {
    pub buyer: Pubkey,
    pub sol_amount: u64,
    pub dragon_amount: u64,
    pub price_per_token: u64,
}

#[event]
pub struct KillRewardsPoolUpdated {
    pub previous_amount: u64,
    pub new_amount: u64,
    pub added_amount: u64,
    pub source: String, // "buyback", "deposit"
}

// ========================================================================================
// =============================== COOLDOWN EVENTS ======================================= 
// ========================================================================================

#[event]
pub struct BreedingCooldownStarted {
    pub dragonbee_mint: Pubkey,
    pub owner: Pubkey,
    pub cooldown_stage: u8,
    pub cooldown_duration: i64,
    pub cooldown_end_time: i64,
}

#[event]
pub struct BreedingCooldownStageIncreased {
    pub dragonbee_mint: Pubkey,
    pub owner: Pubkey,
    pub old_stage: u8,
    pub new_stage: u8,
    pub new_cooldown_duration: i64,
}

#[event]
pub struct BreedingCooldownCompleted {
    pub dragonbee_mint: Pubkey,
    pub owner: Pubkey,
    pub cooldown_stage: u8,
    pub ready_for_breeding: bool,
}

// ========================================================================================
// =============================== GENETIC EVENTS ======================================== 
// ========================================================================================

#[event]
pub struct GeneticTraitsAnalyzed {
    pub dragonbee_mint: Pubkey,
    pub bee_type: u8,
    pub evolution_stage: u8,
    pub appearance_traits: Vec<u8>,
    pub power_traits: Vec<u8>,
    pub calculated_power: u32,
}

#[event]
pub struct TraitMutationOccurred {
    pub dragonbee_mint: Pubkey,
    pub parent1_mint: Pubkey,
    pub parent2_mint: Pubkey,
    pub mutated_trait_type: String, // "appearance", "power"
    pub trait_index: u8,
    pub old_value: u8,
    pub new_value: u8,
}

#[event]
pub struct TraitEnhancementOccurred {
    pub dragonbee_mint: Pubkey,
    pub trait_type: String, // "appearance", "power"
    pub trait_index: u8,
    pub enhancement_amount: u8,
    pub new_value: u8,
}

// ========================================================================================
// =============================== COLLECTION EVENTS ===================================== 
// ========================================================================================

#[event]
pub struct CollectionStatsUpdated {
    pub total_minted: u64,
    pub total_evolved: u64,
    pub total_killed: u64,
    pub total_bred: u64,
    pub average_power: u32,
    pub queens_count: u32,
}

#[event]
pub struct RareDragonBeeDetected {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub rarity_score: u32,
    pub rare_traits: Vec<String>,
    pub estimated_value: u64,
}

// ========================================================================================
// =============================== ERROR EVENTS ========================================== 
// ========================================================================================

#[event]
pub struct BreedingFailed {
    pub parent1_mint: Pubkey,
    pub parent2_mint: Pubkey,
    pub breeder: Pubkey,
    pub failure_reason: String,
    pub compatibility_score: u16,
}

#[event]
pub struct EvolutionFailed {
    pub dragonbee_mint: Pubkey,
    pub owner: Pubkey,
    pub current_evolution_stage: u8,
    pub failure_reason: String,
    pub requirements_met: bool,
}

#[event]
pub struct AuctionBidFailed {
    pub queen_mint: Pubkey,
    pub bidder: Pubkey,
    pub bid_amount: u64,
    pub failure_reason: String,
    pub current_highest_bid: u64,
}

// ========================================================================================
// =============================== ADMIN EVENTS ========================================== 
// ========================================================================================

#[event]
pub struct ProgramPaused {
    pub authority: Pubkey,
    pub reason: String,
    pub timestamp: i64,
}

#[event]
pub struct ProgramUnpaused {
    pub authority: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyWithdrawal {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub reason: String,
}

// ========================================================================================
// =============================== MARKETPLACE EVENTS ==================================== 
// ========================================================================================

#[event]
pub struct DragonBeeListed {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub price: u64,
    pub listing_time: i64,
}

#[event]
pub struct DragonBeeUnlisted {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub unlisting_time: i64,
}

#[event]
pub struct DragonBeeSold {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub sale_price: u64,
    pub marketplace_fee: u64,
    pub sale_time: i64,
}

// ========================================================================================
// =============================== GAME INTEGRATION EVENTS =============================== 
// ========================================================================================

#[event]
pub struct DragonBeeEnteredGame {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub game_program: Pubkey,
    pub entry_time: i64,
}

#[event]
pub struct DragonBeeExitedGame {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub game_program: Pubkey,
    pub exit_time: i64,
    pub power_gained: u32,
    pub rewards_earned: u64,
}

#[event]
pub struct GameRewardsDistributed {
    pub game_program: Pubkey,
    pub total_participants: u32,
    pub total_rewards: u64,
    pub reward_type: String, // "dragon_tokens", "power_boost", "evolution_points"
}

// ========================================================================================
// =============================== ANALYTICS EVENTS ====================================== 
// ========================================================================================

#[event]
pub struct DailyStatsUpdated {
    pub date: i64,
    pub new_dragonbees_minted: u32,
    pub breeding_events: u32,
    pub evolution_events: u32,
    pub kill_events: u32,
    pub total_sol_volume: u64,
    pub active_users: u32,
}

#[event]
pub struct PowerDistributionAnalyzed {
    pub total_dragonbees: u64,
    pub average_power: u32,
    pub median_power: u32,
    pub max_power: u32,
    pub power_percentiles: Vec<u32>, // [P10, P25, P50, P75, P90, P95, P99]
}

#[event]
pub struct BreedingTrendsAnalyzed {
    pub most_popular_type: u8,
    pub average_generation: f64,
    pub breeding_success_rate: f64,
    pub average_cooldown_stage: f64,
    pub queen_utilization_rate: f64,
}
