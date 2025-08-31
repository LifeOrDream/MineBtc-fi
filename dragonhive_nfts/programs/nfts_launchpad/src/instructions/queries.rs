use anchor_lang::prelude::*;
use crate::{
    constants::*,
    // errors::DragonHiveError, // Removed unused import
    state::*,
    utils::*,
};

// ========================================================================================
// =============================== GET DRAGONBEE INFO ==================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct GetDragonBeeInfo<'info> {
    #[account(
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.as_ref()],
        bump = dragonbee_metadata.bump
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,
}

pub fn get_dragonbee_info_handler(
    ctx: Context<GetDragonBeeInfo>,
    dragonbee_mint: Pubkey,
) -> Result<DragonBeeInfoResponse> {
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;
    let current_time = get_current_timestamp()?;

    Ok(DragonBeeInfoResponse {
        mint: dragonbee_mint,
        owner: dragonbee_metadata.owner,
        name: dragonbee_metadata.name.clone(),
        uri: dragonbee_metadata.uri.clone(),
        genes: dragonbee_metadata.genes,
        evolution_stage: dragonbee_metadata.evolution_stage,
        bee_type: dragonbee_metadata.bee_type,
        power: dragonbee_metadata.power,
        generation: dragonbee_metadata.generation,
        parent1: dragonbee_metadata.parent1,
        parent2: dragonbee_metadata.parent2,
        birth_time: dragonbee_metadata.birth_time,
        breeding_count: dragonbee_metadata.breeding_count,
        is_queen: dragonbee_metadata.is_queen,
        can_breed: dragonbee_metadata.can_breed_now(current_time),
        can_evolve: dragonbee_metadata.can_evolve(),
    })
}

// ========================================================================================
// =============================== GET USER DRAGONBEES =================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct GetUserDragonBees<'info> {
    #[account(
        seeds = [USER_PROFILE_SEED, user.as_ref()],
        bump = user_profile.bump
    )]
    pub user_profile: Account<'info, UserProfile>,
}

pub fn get_user_dragonbees_handler(
    ctx: Context<GetUserDragonBees>,
    user: Pubkey,
) -> Result<UserDragonBeesResponse> {
    let user_profile = &ctx.accounts.user_profile;
    
    // In a full implementation, you would load all DragonBee metadata accounts
    // For this example, we'll return a simplified response
    let mut dragonbees = Vec::new();
    let mut total_power = 0u64;

    // Note: In production, you would iterate through user_profile.dragonbees
    // and load each DragonBee metadata account to get full information
    // This would require additional accounts in the instruction

    for &dragonbee_mint in user_profile.dragonbees.iter() {
        // Placeholder - in production, load actual metadata
        let dragonbee_info = DragonBeeInfoResponse {
            mint: dragonbee_mint,
            owner: user,
            name: "DragonBee".to_string(),
            uri: "".to_string(),
            genes: [0u8; 32],
            evolution_stage: 0,
            bee_type: 1,
            power: 1000, // Placeholder
            generation: 0,
            parent1: None,
            parent2: None,
            birth_time: 0,
            breeding_count: 0,
            is_queen: false,
            can_breed: true,
            can_evolve: false,
        };

        total_power += dragonbee_info.power as u64;
        dragonbees.push(dragonbee_info);
    }

    Ok(UserDragonBeesResponse {
        owner: user,
        dragonbees,
        total_count: user_profile.dragonbees.len() as u32,
        total_power,
    })
}

// ========================================================================================
// =============================== GET GLOBAL STATS ====================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct GetGlobalStats<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GlobalStatsResponse {
    pub total_dragonbees_minted: u64,
    pub nft_price: u64,
    pub breeding_fee: u64,
    pub total_sol_collected: u64,
    pub kill_rewards_pool: u64,
    pub is_paused: bool,
    pub honey_token_mint: Pubkey,
    pub collection_mint: Pubkey,
}

pub fn get_global_stats_handler(ctx: Context<GetGlobalStats>) -> Result<GlobalStatsResponse> {
    let global_config = &ctx.accounts.global_config;

    Ok(GlobalStatsResponse {
        total_dragonbees_minted: global_config.total_dragonbees_minted,
        nft_price: global_config.nft_price,
        breeding_fee: global_config.breeding_fee,
        total_sol_collected: global_config.total_sol_collected,
        kill_rewards_pool: global_config.kill_rewards_pool,
        is_paused: global_config.is_paused,
        honey_token_mint: global_config.honey_token_mint,
        collection_mint: global_config.collection_mint,
    })
}

// ========================================================================================
// =============================== GET BREEDING AUCTION ================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(queen_mint: Pubkey)]
pub struct GetBreedingAuction<'info> {
    #[account(
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump = queen_auction.bump
    )]
    pub queen_auction: Account<'info, QueenAuctionManager>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BreedingAuctionResponse {
    pub queen_mint: Pubkey,
    pub queen_owner: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub highest_bid: u64,
    pub highest_bidder: Option<Pubkey>,
    pub finalized: bool,
    pub winner: Option<Pubkey>,
    pub breeding_price: u64,
    pub breeding_count: u32,
    pub is_active: bool,
    pub has_ended: bool,
}

pub fn get_breeding_auction_handler(
    ctx: Context<GetBreedingAuction>,
    queen_mint: Pubkey,
) -> Result<BreedingAuctionResponse> {
    let queen_auction = &ctx.accounts.queen_auction;
    let _current_time = get_current_timestamp()?;

    Ok(BreedingAuctionResponse {
        queen_mint,
        queen_owner: Pubkey::default(), // TODO: Get from leading DragonBee
        start_time: queen_auction.auction_start_epoch as i64,
        end_time: (queen_auction.auction_start_epoch + queen_auction.config.unlimited_deposit_window) as i64,
        highest_bid: queen_auction.price_to_be_queen,
        highest_bidder: Some(Pubkey::default()), // TODO: Get from leading DragonBee
        finalized: queen_auction.is_cooldown(),
        winner: Some(Pubkey::default()), // TODO: Get from leading DragonBee
        breeding_price: queen_auction.price_to_be_queen,
        breeding_count: 0, // TODO: Track breeding count
        is_active: queen_auction.are_live && !queen_auction.is_cooldown(),
        has_ended: queen_auction.is_cooldown(),
    })
}

// ========================================================================================
// =============================== GET GENETIC ANALYSIS ================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct GetGeneticAnalysis<'info> {
    #[account(
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.as_ref()],
        bump = dragonbee_metadata.bump
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GeneticAnalysisResponse {
    pub mint: Pubkey,
    pub bee_type: u8,
    pub evolution_stage: u8,
    pub appearance_traits: Vec<u8>,
    pub power_traits: Vec<u8>,
    pub calculated_power: u32,
    pub rarity_score: u32,
    pub rare_traits: Vec<String>,
    pub breeding_compatibility: Vec<u8>, // Compatible DragonBee types
}

pub fn get_genetic_analysis_handler(
    ctx: Context<GetGeneticAnalysis>,
    dragonbee_mint: Pubkey,
) -> Result<GeneticAnalysisResponse> {
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;

    let bee_type = crate::state::genetics::extract_bee_type(&dragonbee_metadata.genes);
    let evolution_stage = crate::state::genetics::extract_evolution_stage(&dragonbee_metadata.genes);
    let appearance_traits = crate::state::genetics::extract_appearance_traits(&dragonbee_metadata.genes);
    let power_traits = crate::state::genetics::extract_power_traits(&dragonbee_metadata.genes);
    let calculated_power = crate::state::genetics::calculate_power_from_genes(&dragonbee_metadata.genes);
    let rarity_score = calculate_rarity_score(&dragonbee_metadata.genes);
    let rare_traits = has_rare_traits(&dragonbee_metadata.genes);

    // Breeding compatibility - same type only
    let breeding_compatibility = vec![bee_type];

    Ok(GeneticAnalysisResponse {
        mint: dragonbee_mint,
        bee_type,
        evolution_stage,
        appearance_traits,
        power_traits,
        calculated_power,
        rarity_score,
        rare_traits,
        breeding_compatibility,
    })
}

// ========================================================================================
// =============================== GET BREEDING COOLDOWN ================================= 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct GetBreedingCooldown<'info> {
    #[account(
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.as_ref()],
        bump = dragonbee_metadata.bump
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BreedingCooldownResponse {
    pub mint: Pubkey,
    pub last_breeding_time: i64,
    pub cooldown_stage: u8,
    pub cooldown_duration: i64,
    pub cooldown_end_time: i64,
    pub can_breed_now: bool,
    pub time_until_ready: i64,
    pub breeding_count: u32,
}

pub fn get_breeding_cooldown_handler(
    ctx: Context<GetBreedingCooldown>,
    dragonbee_mint: Pubkey,
) -> Result<BreedingCooldownResponse> {
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;
    let current_time = get_current_timestamp()?;

    let cooldown_duration = dragonbee_metadata.get_breeding_cooldown();
    let cooldown_end_time = dragonbee_metadata.last_breeding_time + cooldown_duration;
    let can_breed_now = dragonbee_metadata.can_breed_now(current_time);
    let time_until_ready = if can_breed_now {
        0
    } else {
        cooldown_end_time - current_time
    };

    Ok(BreedingCooldownResponse {
        mint: dragonbee_mint,
        last_breeding_time: dragonbee_metadata.last_breeding_time,
        cooldown_stage: dragonbee_metadata.cooldown_stage,
        cooldown_duration,
        cooldown_end_time,
        can_breed_now,
        time_until_ready,
        breeding_count: dragonbee_metadata.breeding_count,
    })
}

// ========================================================================================
// =============================== GET KILL REWARD ESTIMATE ============================= 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct GetKillRewardEstimate<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.as_ref()],
        bump = dragonbee_metadata.bump
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct KillRewardEstimateResponse {
    pub mint: Pubkey,
    pub current_power: u32,
    pub total_kill_pool: u64,
    pub estimated_reward: u64,
    pub min_power_required: u32,
    pub can_kill: bool,
}

pub fn get_kill_reward_estimate_handler(
    ctx: Context<GetKillRewardEstimate>,
    dragonbee_mint: Pubkey,
) -> Result<KillRewardEstimateResponse> {
    let global_config = &ctx.accounts.global_config;
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;

    let total_kill_pool = global_config.kill_rewards_pool;
    let current_power = dragonbee_metadata.power;

    // Simplified calculation - in production, calculate actual total active power
    let total_active_power = 1_000_000u64; // Placeholder

    let estimated_reward = if current_power >= MIN_KILL_POWER_THRESHOLD {
        calculate_kill_reward(current_power, total_active_power, total_kill_pool)
            .unwrap_or(0)
    } else {
        0
    };

    let can_kill = current_power >= MIN_KILL_POWER_THRESHOLD && 
                   total_kill_pool > 0 && 
                   !dragonbee_metadata.in_game;

    Ok(KillRewardEstimateResponse {
        mint: dragonbee_mint,
        current_power,
        total_kill_pool,
        estimated_reward,
        min_power_required: MIN_KILL_POWER_THRESHOLD,
        can_kill,
    })
}
