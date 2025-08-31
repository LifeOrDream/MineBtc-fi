use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
// MPL Core imports removed as they're not used in breeding functions

use crate::{
    constants::*,
    errors::DragonHiveError,
    events::*,
    state::*,
    utils::*,
};

// ========================================================================================
// =============================== INITIALIZE QUEEN AUCTION MANAGER ==================== 
// ========================================================================================

#[derive(Accounts)]
pub struct InitializeQueenAuctionManager<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ DragonHiveError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
        space = QueenAuctionManager::LEN,
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump
    )]
    pub queen_auction_manager: Account<'info, QueenAuctionManager>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_queen_auction_manager_handler(ctx: Context<InitializeQueenAuctionManager>) -> Result<()> {
    let queen_auction_manager = &mut ctx.accounts.queen_auction_manager;
    let current_slot = Clock::get()?.slot;

    queen_auction_manager.current_auction_epoch = 0;
    queen_auction_manager.config = AuctionConfig {
        bid_increase_pct: MIN_BID_INCREASE_PCT,
        bid_decrease_pct: MAX_BID_DECREASE_PCT,
        unlimited_deposit_window: UNLIMITED_DEPOSIT_WINDOW,
        limited_deposit_window: LIMITED_DEPOSIT_WINDOW,
        cooldown_period: AUCTION_COOLDOWN_PERIOD,
        max_eggs_per_queen: MAX_EGGS_PER_QUEEN,
        energy_tax: DEFAULT_AUCTION_TAX,
    };
    queen_auction_manager.auction_status = AUCTION_PHASE_COOLDOWN;
    queen_auction_manager.auction_start_epoch = current_slot / 432000; // Approximate epochs
    queen_auction_manager.price_to_be_queen = BASE_QUEEN_PRICE;
    queen_auction_manager.price_update_epoch = current_slot / 432000;
    queen_auction_manager.phase_2_start_epoch = 0;
    queen_auction_manager.unlimited_deposits_close_ts = 0;
    queen_auction_manager.are_live = false;
    queen_auction_manager.total_sol_collected = 0;
    queen_auction_manager.energy_yield_accumulated = 0;
    queen_auction_manager.bump = ctx.bumps.queen_auction_manager;

    Ok(())
}

// ========================================================================================
// =============================== UPDATE AUCTION CONFIG ================================= 
// ========================================================================================

#[derive(Accounts)]
pub struct UpdateAuctionConfig<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ DragonHiveError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump = queen_auction_manager.bump
    )]
    pub queen_auction_manager: Account<'info, QueenAuctionManager>,

    pub authority: Signer<'info>,
}

pub fn update_auction_config_handler(
    ctx: Context<UpdateAuctionConfig>,
    are_live: Option<bool>,
    price_to_be_queen: Option<u64>,
    bid_increase_pct: Option<u64>,
    bid_decrease_pct: Option<u64>,
    energy_tax: Option<u64>,
    max_eggs_per_queen: Option<u64>,
) -> Result<()> {
    let queen_auction_manager = &mut ctx.accounts.queen_auction_manager;
    let current_slot = Clock::get()?.slot;
    let current_epoch = current_slot / 432000; // Approximate epochs

    if let Some(are_live) = are_live {
        let was_live = queen_auction_manager.are_live;
        queen_auction_manager.are_live = are_live;

        // If turning on auctions, start new auction
        if !was_live && are_live {
            queen_auction_manager.auction_start_epoch = current_epoch;
            queen_auction_manager.auction_status = AUCTION_PHASE_OPEN;
            queen_auction_manager.current_auction_epoch = queen_auction_manager.current_auction_epoch + 1;
        }
    }

    if let Some(price) = price_to_be_queen {
        require!(price >= MIN_QUEEN_BID, DragonHiveError::InvalidPaymentAmount);
        queen_auction_manager.price_to_be_queen = price;
    }

    if let Some(increase_pct) = bid_increase_pct {
        require!(
            increase_pct >= MIN_BID_INCREASE_PCT && increase_pct <= MAX_BID_INCREASE_PCT,
            DragonHiveError::InvalidParameters
        );
        queen_auction_manager.config.bid_increase_pct = increase_pct;
    }

    if let Some(decrease_pct) = bid_decrease_pct {
        require!(decrease_pct <= MAX_BID_DECREASE_PCT, DragonHiveError::InvalidParameters);
        queen_auction_manager.config.bid_decrease_pct = decrease_pct;
    }

    if let Some(tax) = energy_tax {
        require!(
            tax >= MIN_AUCTION_TAX && tax <= MAX_AUCTION_TAX,
            DragonHiveError::InvalidParameters
        );
        queen_auction_manager.config.energy_tax = tax;
    }

    if let Some(max_eggs) = max_eggs_per_queen {
        queen_auction_manager.config.max_eggs_per_queen = max_eggs;
    }

    Ok(())
}

// ========================================================================================
// =============================== COMPETE TO BE QUEEN =================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey, bid_amount: u64)]
pub struct CompeteToBeQueen<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump = queen_auction_manager.bump,
        constraint = queen_auction_manager.are_live @ DragonHiveError::ProgramPaused,
        constraint = queen_auction_manager.is_open_phase() @ DragonHiveError::AuctionEnded
    )]
    pub queen_auction_manager: Account<'info, QueenAuctionManager>,

    #[account(
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.as_ref()],
        bump = dragonbee_metadata.bump,
        constraint = dragonbee_metadata.owner == user.key() @ DragonHiveError::DragonBeeNotOwnedByUser,
        constraint = !dragonbee_metadata.in_game @ DragonHiveError::DragonBeeInGame
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED, user.key().as_ref()],
        bump = user_profile.bump
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        init_if_needed,
        payer = user,
        space = AuctionParticipation::LEN,
        seeds = [AUCTION_PARTICIPATION_SEED, user.key().as_ref(), queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump
    )]
    pub auction_participation: Account<'info, AuctionParticipation>,

    #[account(
        init_if_needed,
        payer = user,
        space = LeadingDragonBee::LEN,
        seeds = [LEADING_DRAGONBEE_SEED, dragonbee_metadata.bee_type.to_le_bytes().as_ref(), queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump
    )]
    pub leading_dragonbee: Account<'info, LeadingDragonBee>,

    #[account(
        init_if_needed,
        payer = user,
        space = AuctionBidPool::LEN,
        seeds = [AUCTION_BID_POOL_SEED, queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump
    )]
    pub auction_bid_pool: Account<'info, AuctionBidPool>,

    /// SOL treasury for fee collection
    /// CHECK: PDA will be validated by seeds
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump = global_config.treasury_bump
    )]
    pub sol_treasury: SystemAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn compete_to_be_queen_handler(
    ctx: Context<CompeteToBeQueen>,
    dragonbee_mint: Pubkey,
    bid_amount: u64,
) -> Result<()> {
    let queen_auction_manager = &mut ctx.accounts.queen_auction_manager;
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;
    let _user_profile = &mut ctx.accounts.user_profile;
    let auction_participation = &mut ctx.accounts.auction_participation;
    let leading_dragonbee = &mut ctx.accounts.leading_dragonbee;
    let auction_bid_pool = &mut ctx.accounts.auction_bid_pool;

    require!(bid_amount >= MIN_QUEEN_BID, DragonHiveError::BidTooLow);

    // Check if user is already participating
    if auction_participation.auction_epoch == queen_auction_manager.current_auction_epoch {
        return Err(DragonHiveError::AlreadyParticipatingWithABee.into());
    }

    let current_time = get_current_timestamp()?;
    let tax_amount = bid_amount * queen_auction_manager.config.energy_tax / 100;
    let net_bid = bid_amount - tax_amount;

    // Transfer SOL payment
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        bid_amount,
    )?;

    // Initialize or update auction participation
    auction_participation.user = ctx.accounts.user.key();
    auction_participation.dragonbee_mint = dragonbee_mint;
    auction_participation.family_type = dragonbee_metadata.bee_type;
    auction_participation.auction_epoch = queen_auction_manager.current_auction_epoch;
    auction_participation.sui_bidded = bid_amount;
    auction_participation.tax_paid = tax_amount;
    auction_participation.username = "DragonTrainer".to_string(); // Simplified
    auction_participation.limited_phase_flag = false;
    if auction_participation.bump == 0 {
        auction_participation.bump = ctx.bumps.auction_participation;
    }

    // Initialize or update auction bid pool
    if auction_bid_pool.auction_epoch == 0 {
        auction_bid_pool.auction_epoch = queen_auction_manager.current_auction_epoch;
        auction_bid_pool.sui_available = 0;
        auction_bid_pool.total_sui_bidded = 0;
        auction_bid_pool.energy_yield = 0;
        auction_bid_pool.total_honey_energy = 0;
        auction_bid_pool.total_participants = 0;
        auction_bid_pool.bump = ctx.bumps.auction_bid_pool;
    }

    auction_bid_pool.sui_available = auction_bid_pool.sui_available.checked_add(net_bid)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_bid_pool.total_sui_bidded = auction_bid_pool.total_sui_bidded.checked_add(bid_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_bid_pool.energy_yield = auction_bid_pool.energy_yield.checked_add(tax_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_bid_pool.total_participants = auction_bid_pool.total_participants.saturating_add(1);

    // Update or initialize leading DragonBee for this family type
    if leading_dragonbee.auction_epoch == 0 || bid_amount > leading_dragonbee.bid_amount {
        leading_dragonbee.family_type = dragonbee_metadata.bee_type;
        leading_dragonbee.dragonbee_mint = dragonbee_mint;
        leading_dragonbee.bid_amount = bid_amount;
        leading_dragonbee.bidder = ctx.accounts.user.key();
        leading_dragonbee.username = "DragonTrainer".to_string();
        leading_dragonbee.auction_epoch = queen_auction_manager.current_auction_epoch;
        if leading_dragonbee.bump == 0 {
            leading_dragonbee.bump = ctx.bumps.leading_dragonbee;
        }

        emit!(LeadingDragonBeeUpdated {
            auction_start_epoch: queen_auction_manager.auction_start_epoch,
            family_type: dragonbee_metadata.bee_type,
            version: dragonbee_mint,
            bid_amount,
            trainer_addr: ctx.accounts.user.key(),
            username: "DragonTrainer".to_string(),
        });
    }

    // Update global stats
    queen_auction_manager.total_sol_collected = queen_auction_manager.total_sol_collected
        .checked_add(bid_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    queen_auction_manager.energy_yield_accumulated = queen_auction_manager.energy_yield_accumulated
        .checked_add(tax_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    emit!(NewBeeAddedToCompetition {
        trainer_addr: ctx.accounts.user.key(),
        username: "DragonTrainer".to_string(),
        version: dragonbee_mint,
        family_type: dragonbee_metadata.bee_type,
        bid_amt: bid_amount,
        tax_amt: tax_amount,
        auction_start_at: queen_auction_manager.auction_start_epoch,
    });

    // Check if we should transition to limited phase
    check_and_transition_auction_phase(queen_auction_manager, current_time)?;

    Ok(())
}

// ========================================================================================
// =============================== ADD TO BID ============================================ 
// ========================================================================================

#[derive(Accounts)]
#[instruction(additional_bid: u64)]
pub struct AddToBid<'info> {
    #[account(
        mut,
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump = queen_auction_manager.bump,
        constraint = queen_auction_manager.are_live @ DragonHiveError::ProgramPaused
    )]
    pub queen_auction_manager: Account<'info, QueenAuctionManager>,

    #[account(
        mut,
        seeds = [AUCTION_PARTICIPATION_SEED, user.key().as_ref(), queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump = auction_participation.bump,
        constraint = auction_participation.auction_epoch == queen_auction_manager.current_auction_epoch @ DragonHiveError::OldPosition
    )]
    pub auction_participation: Account<'info, AuctionParticipation>,

    #[account(
        mut,
        seeds = [LEADING_DRAGONBEE_SEED, auction_participation.family_type.to_le_bytes().as_ref(), queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump = leading_dragonbee.bump
    )]
    pub leading_dragonbee: Account<'info, LeadingDragonBee>,

    #[account(
        mut,
        seeds = [AUCTION_BID_POOL_SEED, queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump = auction_bid_pool.bump
    )]
    pub auction_bid_pool: Account<'info, AuctionBidPool>,

    /// SOL treasury for fee collection
    /// CHECK: PDA will be validated by seeds
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump
    )]
    pub sol_treasury: SystemAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn add_to_bid_handler(
    ctx: Context<AddToBid>,
    additional_bid: u64,
) -> Result<()> {
    let queen_auction_manager = &mut ctx.accounts.queen_auction_manager;
    let auction_participation = &mut ctx.accounts.auction_participation;
    let leading_dragonbee = &mut ctx.accounts.leading_dragonbee;
    let auction_bid_pool = &mut ctx.accounts.auction_bid_pool;

    // Check auction phase
    require!(
        queen_auction_manager.is_open_phase() || queen_auction_manager.is_limited_phase(),
        DragonHiveError::AuctionEnded
    );

    // If in limited phase, check constraints
    if queen_auction_manager.is_limited_phase() {
        require!(!auction_participation.limited_phase_flag, DragonHiveError::AlreadyAddedToPosition);
        
        // Calculate maximum additional bid allowed (decreasing over time)
        let current_time = get_current_timestamp()?;
        let max_additional = calculate_max_additional_bid(
            queen_auction_manager,
            auction_participation.sui_bidded,
            current_time,
        )?;
        
        require!(additional_bid <= max_additional, DragonHiveError::BidAmountExceedsLimit);
        auction_participation.limited_phase_flag = true;
    }

    let tax_amount = additional_bid * queen_auction_manager.config.energy_tax / 100;
    let net_additional = additional_bid - tax_amount;

    // Transfer additional SOL
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        additional_bid,
    )?;

    // Update participation
    auction_participation.sui_bidded = auction_participation.sui_bidded.checked_add(additional_bid)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_participation.tax_paid = auction_participation.tax_paid.checked_add(tax_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    // Update bid pool
    auction_bid_pool.sui_available = auction_bid_pool.sui_available.checked_add(net_additional)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_bid_pool.total_sui_bidded = auction_bid_pool.total_sui_bidded.checked_add(additional_bid)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    auction_bid_pool.energy_yield = auction_bid_pool.energy_yield.checked_add(tax_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    // Update leading DragonBee if this is now the highest bid
    if auction_participation.sui_bidded > leading_dragonbee.bid_amount {
        leading_dragonbee.bid_amount = auction_participation.sui_bidded;
        leading_dragonbee.bidder = ctx.accounts.user.key();

        emit!(LeadingDragonBeeUpdated {
            auction_start_epoch: queen_auction_manager.auction_start_epoch,
            family_type: auction_participation.family_type,
            version: auction_participation.dragonbee_mint,
            bid_amount: auction_participation.sui_bidded,
            trainer_addr: ctx.accounts.user.key(),
            username: auction_participation.username.clone(),
        });
    }

    emit!(BidUpdatedByUser {
        trainer_addr: ctx.accounts.user.key(),
        username: auction_participation.username.clone(),
        bid_amt: auction_participation.sui_bidded,
        tax_amt: tax_amount,
        flag: auction_participation.limited_phase_flag,
        auction_start_at: queen_auction_manager.auction_start_epoch,
    });

    // Check if we should transition auction phase
    let current_time = get_current_timestamp()?;
    check_and_transition_auction_phase(queen_auction_manager, current_time)?;

    Ok(())
}

// ========================================================================================
// =============================== FINALIZE AUCTION ====================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct FinalizeAuction<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [QUEEN_AUCTION_MANAGER_SEED],
        bump = queen_auction_manager.bump,
        constraint = queen_auction_manager.are_live @ DragonHiveError::ProgramPaused
    )]
    pub queen_auction_manager: Account<'info, QueenAuctionManager>,

    #[account(
        mut,
        seeds = [AUCTION_BID_POOL_SEED, queen_auction_manager.current_auction_epoch.to_le_bytes().as_ref()],
        bump = auction_bid_pool.bump
    )]
    pub auction_bid_pool: Account<'info, AuctionBidPool>,

    /// HONEY token vault for distributing energy rewards
    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_config.vault_bump
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    pub finalizer: Signer<'info>,
}

pub fn finalize_auction_handler(ctx: Context<FinalizeAuction>) -> Result<()> {
    let queen_auction_manager = &mut ctx.accounts.queen_auction_manager;
    let auction_bid_pool = &mut ctx.accounts.auction_bid_pool;

    let current_slot = Clock::get()?.slot;
    let current_epoch = current_slot / 432000;

    // Check if auction can be finalized
    let auction_end_epoch = queen_auction_manager.auction_start_epoch
        .checked_add(queen_auction_manager.config.unlimited_deposit_window)
        .and_then(|e| e.checked_add(queen_auction_manager.config.limited_deposit_window))
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    require!(current_epoch >= auction_end_epoch, DragonHiveError::AuctionStillActive);

    let auction_over_epoch = queen_auction_manager.current_auction_epoch;

    // Calculate price increase for next auction
    let price_increase = queen_auction_manager.price_to_be_queen
        .checked_mul(queen_auction_manager.config.bid_increase_pct)
        .and_then(|p| p.checked_div(100))
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    queen_auction_manager.price_to_be_queen = queen_auction_manager.price_to_be_queen
        .checked_add(price_increase)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    // Set next auction start
    queen_auction_manager.auction_start_epoch = current_epoch
        .checked_add(queen_auction_manager.config.cooldown_period)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    
    queen_auction_manager.auction_status = AUCTION_PHASE_COOLDOWN;

    // Finalize bid pool
    auction_bid_pool.total_honey_energy = auction_bid_pool.energy_yield; // Simplified

    emit!(QueenCompetitionOver {
        started_at_epoch: auction_over_epoch,
        next_event_from: queen_auction_manager.auction_start_epoch,
        hive_burnt_amt: 0, // No HIVE burning in this version
        total_sui_bidded: auction_bid_pool.total_sui_bidded,
        energy_from_queens: auction_bid_pool.sui_available,
        community_energy: auction_bid_pool.energy_yield,
        becoming_queen_expensive_by: price_increase,
        price_to_be_a_queen: queen_auction_manager.price_to_be_queen,
    });

    Ok(())
}

// ========================================================================================
// =============================== HELPER FUNCTIONS =================================== 
// ========================================================================================

/// Check if auction should transition to limited phase or cooldown
fn check_and_transition_auction_phase(
    queen_auction_manager: &mut QueenAuctionManager,
    current_time: i64,
) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    let current_epoch = current_slot / 432000;

    let unlimited_phase_end = queen_auction_manager.auction_start_epoch
        .checked_add(queen_auction_manager.config.unlimited_deposit_window)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    // Transition to limited phase
    if current_epoch >= unlimited_phase_end && queen_auction_manager.is_open_phase() {
        queen_auction_manager.auction_status = AUCTION_PHASE_LIMITED;
        queen_auction_manager.phase_2_start_epoch = current_epoch;
        queen_auction_manager.unlimited_deposits_close_ts = current_time;

        emit!(BidsOpenForExisting {
            auction_start_epoch: queen_auction_manager.auction_start_epoch,
            price_to_be_a_queen: queen_auction_manager.price_to_be_queen,
            cur_epoch: current_epoch,
            deposits_open_till: current_time + (queen_auction_manager.config.limited_deposit_window as i64 * 432000 * 400), // Approximate
        });
    }

    // Transition to cooldown
    let limited_phase_end = unlimited_phase_end
        .checked_add(queen_auction_manager.config.limited_deposit_window)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    if current_epoch >= limited_phase_end && queen_auction_manager.is_limited_phase() {
        queen_auction_manager.auction_status = AUCTION_PHASE_COOLDOWN;

        emit!(BidsClosed {
            auction_start_epoch: queen_auction_manager.auction_start_epoch,
        });
    }

    Ok(())
}

/// Calculate maximum additional bid allowed in limited phase
fn calculate_max_additional_bid(
    queen_auction_manager: &QueenAuctionManager,
    existing_bid: u64,
    current_time: i64,
) -> Result<u64> {
    let limited_window_duration = queen_auction_manager.config.limited_deposit_window * 432000 * 400; // Approximate
    let deposits_close_time = queen_auction_manager.unlimited_deposits_close_ts
        .checked_add(limited_window_duration as i64)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    if deposits_close_time <= current_time {
        return Ok(0);
    }

    let time_remaining = deposits_close_time - current_time;
    let max_additional_percentage = (time_remaining as u64 * 50) / limited_window_duration; // Linear decrease from 50% to 0%

    Ok(existing_bid * max_additional_percentage / 100)
}