use anchor_lang::prelude::*;
use crate::constants::*;

// ========================================================================================
// =============================== GLOBAL STATE ========================================== 
// ========================================================================================

/// Global configuration for the DragonHive NFT program
#[account]
pub struct GlobalConfig {
    /// Program authority (can update config and mint genesis NFTs)
    pub authority: Pubkey,
    
    /// Treasury account for collecting SOL fees
    pub treasury: Pubkey,
    
    /// HONEY token mint address
    pub honey_token_mint: Pubkey,
    
    /// HONEY token vault for rewards and buybacks
    pub honey_vault: Pubkey,
    
    /// HONEY vault authority PDA
    pub honey_vault_authority: Pubkey,
    
    /// DragonBee collection mint
    pub collection_mint: Pubkey,
    
    /// Total DragonBees minted
    pub total_dragonbees_minted: u64,
    
    /// Current NFT price in lamports
    pub nft_price: u64,
    
    /// Base breeding fee in lamports
    pub breeding_fee: u64,
    
    /// Total SOL collected from sales
    pub total_sol_collected: u64,
    
    /// Total HONEY tokens in kill rewards pool
    pub kill_rewards_pool: u64,
    
    /// Whether the program is paused
    pub is_paused: bool,
    
    /// PDA bump seeds
    pub config_bump: u8,
    pub vault_bump: u8,
    pub vault_authority_bump: u8,
    pub treasury_bump: u8,
}

impl GlobalConfig {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 
        32 +  // authority
        32 +  // treasury  
                32 +  // honey_token_mint
         32 +  // honey_vault
         32 +  // honey_vault_authority
        32 +  // collection_mint
        8 +   // total_dragonbees_minted
        8 +   // nft_price
        8 +   // breeding_fee
        8 +   // total_sol_collected
        8 +   // kill_rewards_pool
        1 +   // is_paused
        1 +   // config_bump
        1 +   // vault_bump
        1 +   // vault_authority_bump
        1;    // treasury_bump
}

// ========================================================================================
// =============================== DRAGONBEE STATE ======================================= 
// ========================================================================================

/// DragonBee NFT metadata and state
#[account]
pub struct DragonBeeMetadata {
    /// The NFT mint address
    pub mint: Pubkey,
    
    /// Current owner
    pub owner: Pubkey,
    
    /// DragonBee name
    pub name: String,
    
    /// Metadata URI
    pub uri: String,
    
    /// 256-bit genetic code (32 bytes)
    pub genes: [u8; 32],
    
    /// Current evolution stage (0-7)
    pub evolution_stage: u8,
    
    /// DragonBee type (1-8)
    pub bee_type: u8,
    
    /// Current power level
    pub power: u32,
    
    /// Generation (0 for genesis, increments with breeding)
    pub generation: u32,
    
    /// Parent 1 mint (None for genesis)
    pub parent1: Option<Pubkey>,
    
    /// Parent 2 mint (None for genesis)
    pub parent2: Option<Pubkey>,
    
    /// Birth timestamp
    pub birth_time: i64,
    
    /// Last breeding timestamp
    pub last_breeding_time: i64,
    
    /// Number of times this DragonBee has bred
    pub breeding_count: u32,
    
    /// Current breeding cooldown stage (0-4)
    pub cooldown_stage: u8,
    
    /// Whether this DragonBee is currently a queen
    pub is_queen: bool,
    
    /// Queen breeding price (if queen)
    pub queen_breeding_price: u64,
    
    /// Times this DragonBee has been used in games
    pub game_interactions: u32,
    
    /// Whether the DragonBee is currently in a game
    pub in_game: bool,
    
    /// PDA bump
    pub bump: u8,
}

impl DragonBeeMetadata {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +                                    // mint
        32 +                                    // owner
        4 + MAX_NAME_LENGTH +                   // name
        4 + MAX_URI_LENGTH +                    // uri
        32 +                                    // genes
        1 +                                     // evolution_stage
        1 +                                     // bee_type
        4 +                                     // power
        4 +                                     // generation
        33 +                                    // parent1 (Option<Pubkey>)
        33 +                                    // parent2 (Option<Pubkey>)
        8 +                                     // birth_time
        8 +                                     // last_breeding_time
        4 +                                     // breeding_count
        1 +                                     // cooldown_stage
        1 +                                     // is_queen
        8 +                                     // queen_breeding_price
        4 +                                     // game_interactions
        1 +                                     // in_game
        1;                                      // bump

    /// Calculate current breeding cooldown based on breeding count
    pub fn get_breeding_cooldown(&self) -> i64 {
        match self.cooldown_stage {
            0 => BREEDING_COOLDOWN_BASE,    // 5 minutes
            1 => BREEDING_COOLDOWN_NORMAL,  // 30 minutes
            2 => BREEDING_COOLDOWN_SLOW,    // 6 hours
            3 => BREEDING_COOLDOWN_SLOWER,  // 7 days
            4 => BREEDING_COOLDOWN_SLOWEST, // 1 month
            _ => BREEDING_COOLDOWN_SLOWEST, // Default to longest
        }
    }

    /// Check if DragonBee can breed now
    pub fn can_breed_now(&self, current_time: i64) -> bool {
        let cooldown_end = self.last_breeding_time + self.get_breeding_cooldown();
        current_time >= cooldown_end
    }

    /// Calculate kill reward based on power
    pub fn calculate_kill_reward(&self, total_pool: u64, total_power: u64) -> u64 {
        if total_power == 0 {
            return 0;
        }
        
        // Reward = (individual_power / total_power) * total_pool
        ((self.power as u64 * total_pool) / total_power)
            .min(total_pool / 10) // Cap at 10% of pool
    }

    /// Check if ready for evolution
    pub fn can_evolve(&self) -> bool {
        if self.evolution_stage >= EVOLUTION_DRAGON {
            return false; // Already at max evolution
        }
        
        // Evolution requirements based on power and interactions
        let power_requirement = (self.evolution_stage as u32 + 1) * 1000;
        let interaction_requirement = (self.evolution_stage as u32 + 1) * 10;
        
        self.power >= power_requirement && self.game_interactions >= interaction_requirement
    }
}

// ========================================================================================
// =============================== USER STATE ============================================ 
// ========================================================================================

/// User profile for DragonBee ownership tracking
#[account]
pub struct UserProfile {
    /// User's wallet address
    pub owner: Pubkey,
    
    /// List of owned DragonBee mints
    pub dragonbees: Vec<Pubkey>,
    
    /// Total SOL spent on DragonBees
    pub total_sol_spent: u64,
    
    /// Total breeding fees paid
    pub total_breeding_fees: u64,
    
    /// Number of DragonBees killed for rewards
    pub dragonbees_killed: u32,
    
    /// Total HONEY tokens earned from kills
    pub honey_tokens_earned: u64,
    
    /// Profile creation time
    pub created_at: i64,
    
    /// Last activity timestamp
    pub last_activity: i64,
    
    /// PDA bump
    pub bump: u8,
}

impl UserProfile {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +                                    // owner
        4 + (MAX_USER_DRAGONBEES * 32) +        // dragonbees vec
        8 +                                     // total_sol_spent
        8 +                                     // total_breeding_fees
        4 +                                     // dragonbees_killed
        8 +                                     // dragon_tokens_earned
        8 +                                     // created_at
        8 +                                     // last_activity
        1;                                      // bump
}

// ========================================================================================
// =============================== QUEEN AUCTION STATE =================================== 
// ========================================================================================

/// Global queen auction manager - handles all queen auctions
#[account]
pub struct QueenAuctionManager {
    /// Current auction epoch/round
    pub current_auction_epoch: u64,
    
    /// Auction configuration
    pub config: AuctionConfig,
    
    /// Current auction status
    pub auction_status: u8,
    
    /// Auction start epoch
    pub auction_start_epoch: u64,
    
    /// Current price to become a queen (minimum winning bid)
    pub price_to_be_queen: u64,
    
    /// Last epoch when price was updated
    pub price_update_epoch: u64,
    
    /// Phase 2 start epoch (limited bidding)
    pub phase_2_start_epoch: u64,
    
    /// When unlimited deposits closed (timestamp)
    pub unlimited_deposits_close_ts: i64,
    
    /// Whether auctions are currently live
    pub are_live: bool,
    
    /// Total SOL collected from auctions
    pub total_sol_collected: u64,
    
    /// Energy yield accumulated from taxes
    pub energy_yield_accumulated: u64,
    
    /// PDA bump
    pub bump: u8,
}

impl QueenAuctionManager {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +     // current_auction_epoch
        AuctionConfig::LEN + // config
        1 +     // auction_status
        8 +     // auction_start_epoch
        8 +     // price_to_be_queen
        8 +     // price_update_epoch
        8 +     // phase_2_start_epoch
        8 +     // unlimited_deposits_close_ts
        1 +     // are_live
        8 +     // total_sol_collected
        8 +     // energy_yield_accumulated
        1;      // bump

    /// Check if auction is in open phase
    pub fn is_open_phase(&self) -> bool {
        self.auction_status == AUCTION_PHASE_OPEN
    }

    /// Check if auction is in limited phase
    pub fn is_limited_phase(&self) -> bool {
        self.auction_status == AUCTION_PHASE_LIMITED
    }

    /// Check if auction is in cooldown
    pub fn is_cooldown(&self) -> bool {
        self.auction_status == AUCTION_PHASE_COOLDOWN
    }
}

/// Auction configuration parameters
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AuctionConfig {
    /// Percentage increase in minimum bid each auction (5-50%)
    pub bid_increase_pct: u64,
    
    /// Percentage decrease during limited phase (max 11%)
    pub bid_decrease_pct: u64,
    
    /// Unlimited deposit window (epochs)
    pub unlimited_deposit_window: u64,
    
    /// Limited deposit window (epochs)
    pub limited_deposit_window: u64,
    
    /// Cooldown period between auctions (epochs)
    pub cooldown_period: u64,
    
    /// Maximum eggs per queen
    pub max_eggs_per_queen: u64,
    
    /// Energy tax percentage (1-15%)
    pub energy_tax: u64,
}

impl AuctionConfig {
    pub const LEN: usize = 8 + 8 + 8 + 8 + 8 + 8 + 8; // 7 u64 fields
}

/// Leading DragonBee for each family type in current auction
#[account]
pub struct LeadingDragonBee {
    /// Family type (1-8)
    pub family_type: u8,
    
    /// DragonBee mint address
    pub dragonbee_mint: Pubkey,
    
    /// Current highest bid for this family type
    pub bid_amount: u64,
    
    /// Bidder's address
    pub bidder: Pubkey,
    
    /// Bidder's username
    pub username: String,
    
    /// Auction epoch this bid belongs to
    pub auction_epoch: u64,
    
    /// PDA bump
    pub bump: u8,
}

impl LeadingDragonBee {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        1 +     // family_type
        32 +    // dragonbee_mint
        8 +     // bid_amount
        32 +    // bidder
        4 + MAX_NAME_LENGTH + // username
        8 +     // auction_epoch
        1;      // bump
}

/// User's participation in current auction
#[account]
pub struct AuctionParticipation {
    /// User's address
    pub user: Pubkey,
    
    /// DragonBee deposited for auction
    pub dragonbee_mint: Pubkey,
    
    /// Family type of deposited DragonBee
    pub family_type: u8,
    
    /// Auction epoch
    pub auction_epoch: u64,
    
    /// Total SUI bidded
    pub sui_bidded: u64,
    
    /// Tax paid
    pub tax_paid: u64,
    
    /// Username
    pub username: String,
    
    /// Whether user added to bid in limited phase
    pub limited_phase_flag: bool,
    
    /// PDA bump
    pub bump: u8,
}

impl AuctionParticipation {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // user
        32 +    // dragonbee_mint
        1 +     // family_type
        8 +     // auction_epoch
        8 +     // sui_bidded
        8 +     // tax_paid
        4 + MAX_NAME_LENGTH + // username
        1 +     // limited_phase_flag
        1;      // bump
}

/// Auction bid pool for each epoch
#[account]
pub struct AuctionBidPool {
    /// Auction epoch
    pub auction_epoch: u64,
    
    /// Total SUI available in pool
    pub sui_available: u64,
    
    /// Total SUI bidded (including taxes)
    pub total_sui_bidded: u64,
    
    /// Energy yield from taxes
    pub energy_yield: u64,
    
    /// Total HONEY energy to distribute
    pub total_honey_energy: u64,
    
    /// Total participants
    pub total_participants: u32,
    
    /// PDA bump
    pub bump: u8,
}

impl AuctionBidPool {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        8 +     // auction_epoch
        8 +     // sui_available
        8 +     // total_sui_bidded
        8 +     // energy_yield
        8 +     // total_honey_energy
        4 +     // total_participants
        1;      // bump
}

/// Breeding cooldown tracking
#[account]
pub struct BreedingCooldown {
    /// DragonBee mint
    pub dragonbee_mint: Pubkey,
    
    /// Last breeding timestamp
    pub last_breeding_time: i64,
    
    /// Current cooldown stage
    pub cooldown_stage: u8,
    
    /// PDA bump
    pub bump: u8,
}

impl BreedingCooldown {
    pub const LEN: usize = DISCRIMINATOR_SIZE +
        32 +    // dragonbee_mint
        8 +     // last_breeding_time
        1 +     // cooldown_stage
        1;      // bump
}

// ========================================================================================
// =============================== RESPONSE STRUCTS ===================================== 
// ========================================================================================

/// Response structure for DragonBee info queries
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DragonBeeInfoResponse {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub name: String,
    pub uri: String,
    pub genes: [u8; 32],
    pub evolution_stage: u8,
    pub bee_type: u8,
    pub power: u32,
    pub generation: u32,
    pub parent1: Option<Pubkey>,
    pub parent2: Option<Pubkey>,
    pub birth_time: i64,
    pub breeding_count: u32,
    pub is_queen: bool,
    pub can_breed: bool,
    pub can_evolve: bool,
}

/// Response structure for user DragonBees query
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UserDragonBeesResponse {
    pub owner: Pubkey,
    pub dragonbees: Vec<DragonBeeInfoResponse>,
    pub total_count: u32,
    pub total_power: u64,
}

// ========================================================================================
// =============================== GENETIC UTILITIES ===================================== 
// ========================================================================================

/// Genetic trait extraction and manipulation utilities
pub mod genetics {
    use super::*;

    /// Extract DragonBee type from genetic data
    pub fn extract_bee_type(genes: &[u8; 32]) -> u8 {
        genes[0] & 0x0F // First 4 bits
    }

    /// Extract evolution stage from genetic data
    pub fn extract_evolution_stage(genes: &[u8; 32]) -> u8 {
        (genes[0] >> 4) & 0x07 // Bits 4-6
    }

    /// Extract appearance traits from genetic data
    pub fn extract_appearance_traits(genes: &[u8; 32]) -> Vec<u8> {
        let mut traits = Vec::new();
        
        // Extract 28 appearance traits (7 groups × 4 traits)
        for i in 0..28 {
            let byte_idx = (APPEARANCE_OFFSET as usize + i * 5) / 8;
            let bit_offset = (APPEARANCE_OFFSET + i as u8 * 5) % 8;
            
            if byte_idx < 32 {
                let trait_value = (genes[byte_idx] >> bit_offset) & 0x1F; // 5 bits
                traits.push(trait_value);
            }
        }
        
        traits
    }

    /// Extract power traits from genetic data
    pub fn extract_power_traits(genes: &[u8; 32]) -> Vec<u8> {
        let mut traits = Vec::new();
        
        // Extract 21 power traits (7 groups × 3 traits)
        for i in 0..21 {
            let byte_idx = (POWER_OFFSET as usize + i * 4) / 8;
            let bit_offset = (POWER_OFFSET + i as u8 * 4) % 8;
            
            if byte_idx < 32 {
                let trait_value = (genes[byte_idx] >> bit_offset) & 0x0F; // 4 bits
                traits.push(trait_value);
            }
        }
        
        traits
    }

    /// Calculate total power from genetic traits
    pub fn calculate_power_from_genes(genes: &[u8; 32]) -> u32 {
        let power_traits = extract_power_traits(genes);
        let mut total_power = 0u32;
        
        for trait_value in power_traits {
            total_power = total_power.saturating_add(trait_value as u32 * 100);
        }
        
        total_power
    }

    /// Breed two genetic codes to create offspring
    pub fn breed_genes(parent1: &[u8; 32], parent2: &[u8; 32], random_seed: u64) -> [u8; 32] {
        let mut offspring = [0u8; 32];
        
        // Use simple random selection for now - in production, implement more sophisticated breeding
        for i in 0..32 {
            offspring[i] = if (random_seed >> (i % 64)) & 1 == 0 {
                parent1[i]
            } else {
                parent2[i]
            };
        }
        
        // Reset evolution stage to 0 for offspring
        offspring[0] = (offspring[0] & 0x8F) | 0x00; // Clear evolution bits, keep type
        
        offspring
    }

    /// Evolve genetic code (enhance traits)
    pub fn evolve_genes(genes: &[u8; 32], current_evolution: u8) -> [u8; 32] {
        let mut evolved = *genes;
        
        // Increase evolution stage
        let new_evolution = (current_evolution + 1).min(EVOLUTION_DRAGON);
        evolved[0] = (evolved[0] & 0x8F) | ((new_evolution & 0x07) << 4);
        
        // Enhance some traits randomly (simplified version)
        for i in 1..32 {
            if evolved[i] < 255 && (i as u64 % 7) == 0 {
                evolved[i] = evolved[i].saturating_add(1);
            }
        }
        
        evolved
    }
}
