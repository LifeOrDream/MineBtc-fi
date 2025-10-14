use anchor_lang::prelude::*;

use crate::state::*;
use crate::events::*;
use crate::errors::ErrorCode;
use crate::instructions::helper::{self, transfer_to_sol_treasury};

use anchor_spl::token_interface::{ Mint as Mint2022 };


// ----------------------------------------------------------------------------------------
// -------------- USER FUNCTIONS :: CREATE MOON-BASE, EXPAND MOONBASE ----------------
// ----------------------------------------------------------------------------------------

/// Creates a new moon base for a user
/// This can only be called once per user
pub fn initialize_user_moonbase(ctx: Context<CreateUserMoonbase>, referrer: Option<Pubkey>, faction_id: u8) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let new_rewards = &mut ctx.accounts.new_user_rewards;
    let user = &ctx.accounts.user;
    
    // Increment total moonbases created and total sol spent
    let global_config = &mut ctx.accounts.global_config;
    global_config.total_moonbases_created = global_config.total_moonbases_created.saturating_add(1);
    global_config.total_sol_spent = global_config.total_sol_spent.saturating_add(global_config.base_creation_cost);
    
    // Validate faction_id
    require!(
        (faction_id as usize) < global_config.supported_factions.len(),
        ErrorCode::InvalidFactionId
    );
    
    msg!("Creating moonbase for user {} with faction: {} ({})", 
         user.key(), 
         faction_id, 
         global_config.supported_factions.get(faction_id as usize).unwrap_or(&"Unknown".to_string()));
    
    // --- Initialize the new user's referral rewards account ---
    new_rewards.owner = user.key();
    new_rewards.total_sol_earned = 0;
    new_rewards.sol_claimed_for_xp = 0;
    new_rewards.bump = ctx.bumps.new_user_rewards;
    new_rewards.referrals_count = 0;
    
    // Charge the creation fee with 50/50 split
    let creation_cost = global_config.base_creation_cost;
    let fee_recipient_amount = creation_cost / 2; // 50% goes to creation fee recipient
    let remaining_amount = creation_cost - fee_recipient_amount; // 50% goes through existing system
    
    // Transfer 50% directly to creation fee recipient
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: user.to_account_info(),
                to: ctx.accounts.creation_fee_recipient.to_account_info(),
            },
        ),
        fee_recipient_amount,
    )?;
    
    msg!("💰 Transferred {} SOL directly to creation fee recipient: {}", 
         fee_recipient_amount, global_config.creation_fee_recipient);
    
    // Initialize the user's moonbase
    user_moonbase.owner = user.key();
    
    // Initialize referral - make sure referrer is not the same as owner
    if let Some(ref_pubkey) = referrer {
        require!(ref_pubkey != user.key(), ErrorCode::ReferralCannotBeSameAsOwner);        
        user_moonbase.referral = ref_pubkey;
        
        // Handle referral payment with remaining 50%
        let (_, _) = helper::process_referral_payment(
            remaining_amount,
            &ref_pubkey,
            &user.key(),
            &user.to_account_info(),
            ctx.accounts.referrer_rewards.as_ref().map(|acc| acc.to_account_info()).as_ref(),
            &mut ctx.accounts.global_config,
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
        )?;
        
        // Process new referral (increment count) if referrer rewards account exists
        if let Some(referrer_rewards_account) = &mut ctx.accounts.referrer_rewards {
            referrer_rewards_account.referrals_count = referrer_rewards_account.referrals_count.saturating_add(1);
            
            msg!("🤝 New referral processed: {} total referrals for {}", 
                 referrer_rewards_account.referrals_count, ref_pubkey);
        }
    } else {
        // If no referrer, set to default (system program is a common default)
        user_moonbase.referral = ctx.accounts.system_program.key();
        
        // If no referrer, remaining 50% goes directly to treasury
        transfer_to_sol_treasury(
            &user.to_account_info(),
            &ctx.accounts.sol_treasury.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            remaining_amount,
        )?;
    }

    // Initialize remaining fields
    user_moonbase.modules_count = 0;
    user_moonbase.active_hashpower = 0;
    user_moonbase.available_electricity = 0;
    user_moonbase.used_electricity = 0;
    user_moonbase.moondoge_claim_index = 0;
    user_moonbase.bump = ctx.bumps.user_moonbase;
    user_moonbase.faction_id = faction_id;
    
    // Initialize XP and level system
    user_moonbase.level = 0;
    user_moonbase.xp = 0;
    user_moonbase.last_login_ts = Clock::get()?.unix_timestamp;
    user_moonbase.daily_login_streak = 0;
    
    // Initialize grid bitmap
    user_moonbase.occupied_bitmap = [0u8; BITMAP_SIZE];
    
    // Initialize moonbase expansion system
    helper::initialize_moonbase_dimensions(user_moonbase)?;
    
    // Emit event
    emit!(UserMoonBaseCreated {
        owner: user.key(),
        referrer,
    });
    
    msg!("Created new moon base for user {}", user.key());
    
    Ok(())
}



/// Expand a moonbase
pub fn expand_moonbase_internal(ctx: Context<ExpandMoonbase>, expansion_id: u8) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let user = &ctx.accounts.user;
    
    msg!("🚀 Starting moonbase expansion for user {}", user.key());
    msg!("   Requested expansion ID: {}", expansion_id);
    msg!("   Current moonbase size: {}x{}", user_moonbase.current_width, user_moonbase.current_height);
    msg!("   Current level: {}", user_moonbase.level);
        
    // Find the expansion configuration and clone it to avoid borrowing issues
    let expansion = ctx.accounts.global_config.expansions
        .iter()
        .find(|e| e.id == expansion_id)
        .cloned()
        .ok_or(ErrorCode::ExpansionNotFound)?;
    
    msg!("✅ Found expansion: '{}' (ID: {})", expansion.name, expansion.id);
    msg!("   Required level: {}", expansion.required_level);
    msg!("   Cost: {} SOL", expansion.cost_sol);
    msg!("   New dimensions: {}x{}", expansion.new_width, expansion.new_height);
    msg!("   Is active: {}", expansion.is_active);
    
    // Validate the expansion can be purchased using helper function
    require!(
        helper::can_purchase_expansion(user_moonbase, &expansion)?,
        ErrorCode::ExpansionNotAvailable
    );
    
    msg!("✅ Expansion validation passed");
    
    // Get the cost
    let cost = expansion.cost_sol;
    
    msg!("💰 Processing payment of {} SOL", cost);
    
    // Handle referral payment for expansions
    let referrer = user_moonbase.referral;
    msg!("🤝 Processing referral payment to: {}", referrer);
    
    let (referral_fee, treasury_amount) = helper::process_referral_payment(
        cost,
        &referrer,
        &user.key(),
        &user.to_account_info(),
        ctx.accounts.referrer_rewards.as_ref().map(|acc| acc.to_account_info()).as_ref(),
        &mut ctx.accounts.global_config,
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    
    msg!("💰 Payment processed: {} SOL to referrer, {} SOL to treasury", 
         referral_fee, treasury_amount);
    
    // Add expansion to purchased list
    msg!("📝 Adding expansion {} to purchased list", expansion.id);
    user_moonbase.purchased_expansions.push(expansion.id);
    msg!("   Total expansions purchased: {}", user_moonbase.purchased_expansions.len());
    
    // Update moonbase dimensions
    let old_width = user_moonbase.current_width;
    let old_height = user_moonbase.current_height;
    
    msg!("📐 Updating moonbase dimensions:");
    msg!("   Old size: {}x{}", old_width, old_height);
    
    user_moonbase.current_width = expansion.new_width;
    user_moonbase.current_height = expansion.new_height;
    
    msg!("   New size: {}x{}", user_moonbase.current_width, user_moonbase.current_height);
    
    // Calculate new usable area
    let old_area = (old_width as u32) * (old_height as u32);
    let new_area = (expansion.new_width as u32) * (expansion.new_height as u32);
    let area_increase = new_area - old_area;
    
    msg!("📊 Area calculations:");
    msg!("   Old area: {} tiles", old_area);
    msg!("   New area: {} tiles", new_area);
    msg!("   Area increase: {} tiles", area_increase);
    
    // Process daily login and award XP for expansion
    let expansion_xp = 100 + (expansion.required_level as u32 * 10); // More XP for higher level expansions
    
    msg!("🌟 Awarding expansion XP: {} (base 100 + {} * 10 for level requirement)", 
         expansion_xp, expansion.required_level);
    
    process_daily_login_and_xp(
        user_moonbase,
        expansion_xp,
        "Moonbase Expansion",
    )?;
    
    emit!(MoonbaseExpanded {
        owner: user_moonbase.owner,
        expansion_id: expansion.id,
        expansion_name: expansion.name.clone(),
        old_width,
        old_height,
        new_width: expansion.new_width,
        new_height: expansion.new_height,
        area_increase,
        xp_gained: expansion_xp,
        cost_paid: expansion.cost_sol,
    });
    
    msg!("🎉 Moonbase expansion completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Expansion: '{}' (ID: {})", expansion.name, expansion_id);
    msg!("   Cost paid: {} SOL", cost);
    msg!("   New size: {}x{} (+{} tiles)", 
         expansion.new_width, expansion.new_height, area_increase);
    msg!("   XP awarded: {}", expansion_xp);
    
    Ok(())
}


// ----------------------------------------------------------------------------------------
// -------------- USER FUNCTIONS :: CLAIM REFERRAL REWARDS ----------------
// ----------------------------------------------------------------------------------------
 

/// Claim referral rewards
pub fn claim_referral_rewards_internal(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    let rewards_account = &mut ctx.accounts.referral_rewards;
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let user = &ctx.accounts.user;
        
    // Calculate actual available balance in the account (this is real SOL)
    let account_balance = rewards_account.to_account_info().lamports();

    // Calculate minimum required for rent exemption
    let rent = Rent::get()?;
    let min_rent = rent.minimum_balance(rewards_account.to_account_info().data_len());
    
    // Calculate claimable amount, ensuring we maintain rent-exemption
    let claimable_amount = account_balance.saturating_sub(min_rent);
    msg!("💰 REFERRAL REWARDS: Claimable amount: {} SOL", claimable_amount);

    // Calculate NEW SOL earned since last XP claim 
    let new_sol_for_xp = rewards_account.total_sol_earned.saturating_sub(rewards_account.sol_claimed_for_xp);
    msg!("💰 REFERRAL REWARDS: New SOL for XP: {} SOL", new_sol_for_xp);
    
    // Calculate XP for new SOL earned: 500 XP per SOL (sqrt scaling)
    let sol_bonus_xp = if new_sol_for_xp > 0 {
        let sqrt_lamports = helper::integer_sqrt(new_sol_for_xp);
        sqrt_lamports * 500 / 1_000_000_000  // 500 XP per SOL (sqrt'ed)
    } else {
        0
    };
    msg!("💰 REFERRAL REWARDS: XP for new SOL: {} XP", sol_bonus_xp);

    // Process daily login and award XP for new SOL earnings
    process_daily_login_and_xp(
        user_moonbase,
        sol_bonus_xp,
        "Referral SOL Earnings",
    )?;

    // Update the tracked amount so this SOL won't give XP again (if XP was awarded)
    if sol_bonus_xp > 0 {
        rewards_account.sol_claimed_for_xp = rewards_account.total_sol_earned;
        msg!("💰 REFERRAL REWARDS: Updated SOL claimed for XP: {} SOL", rewards_account.sol_claimed_for_xp);

        // Emit enhanced referral success event
        emit!(ReferralSuccess {
            referrer: user.key(),
            referee: user.key(), // Self in this context (claiming own rewards)
            xp_bonus: sol_bonus_xp,
            sol_earned_bonus: sol_bonus_xp,
        });

        msg!("🤝 Referral XP awarded: {} XP for {} new SOL earned (Total tracked: {}, Previously claimed: {})", 
             sol_bonus_xp, new_sol_for_xp, rewards_account.total_sol_earned, rewards_account.sol_claimed_for_xp);
    } else {
        msg!("🤝 No new referral earnings for XP - {} SOL total, {} already claimed for XP", 
             rewards_account.total_sol_earned, rewards_account.sol_claimed_for_xp);
    }

    // Transfer SOL from the rewards account to the user
    **rewards_account.to_account_info().try_borrow_mut_lamports()? -= claimable_amount;
    **user.to_account_info().try_borrow_mut_lamports()? += claimable_amount;
        
    // Emit event
    emit!(ReferralRewardsClaimed {
        owner: user.key(),
        amount: claimable_amount,
    });
    
    msg!("Claimed {} lamports in referral rewards for {}", claimable_amount, user.key());
    
    Ok(())
}


// ----------------------------------------------------------------------------------------
// -------------- USER FUNCTIONS :: UPDATE USER ELECTRICITY ----------------
// ----------------------------------------------------------------------------------------
 


/// Update user's available electricity based on MoonDoge staking
pub fn update_user_electricity_internal(ctx: Context<UpdateUserElectricity>, to_increase: bool, amount: u64) -> Result<()> {
    msg!("⚡ Updating user electricity - Increase: {}, Amount: {}", to_increase, amount);
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let mining_state = &mut ctx.accounts.mining_state;
    
    // Process daily login automatically (no loot/level stats for this admin function)
    let (xp_gained, _streak) = helper::process_daily_login(user_moonbase)?;
    if xp_gained > 0 {
        msg!("🗓️ Daily login processed: {} XP gained", xp_gained);
    }

    // Check if the authority is the fee_collector 
    msg!("🔑 Checking authority - Caller: {}, Expected: {}", 
        ctx.accounts.authority.key(), 
        ctx.accounts.global_config.ext_fee_collector);
    require!(ctx.accounts.authority.key() == ctx.accounts.global_config.ext_fee_collector, ErrorCode::Unauthorized);
    
    // Update user's available electricity
    if to_increase {
        msg!("📈 Increasing user electricity by {}", amount);
        msg!("   Current electricity: {}", user_moonbase.available_electricity);
        user_moonbase.available_electricity = user_moonbase.available_electricity
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("   New electricity: {}", user_moonbase.available_electricity);
    } else {        
        msg!("📉 Decreasing user electricity by {}", amount);
        msg!("   Current electricity: {}", user_moonbase.available_electricity);
        user_moonbase.available_electricity = user_moonbase.available_electricity
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("   New electricity: {}", user_moonbase.available_electricity);
        
        // Check if the user has enough available electricity to cover the used electricity
        msg!("   Used electricity: {}", user_moonbase.used_electricity);
        require!(user_moonbase.available_electricity >= user_moonbase.used_electricity, ErrorCode::ElectricityInUse);
        msg!("   Remaining available electricity: {}", user_moonbase.available_electricity - user_moonbase.used_electricity);
    }
        
    // Update global electricity
    if to_increase {
        msg!("🌐 Updating global electricity statistics - Current total: {}", mining_state.total_active_electricity);
        mining_state.total_active_electricity = mining_state.total_active_electricity
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("   New total active electricity: {}", mining_state.total_active_electricity);
    } else {
        msg!("🌐 Updating global electricity statistics - Current total: {}", mining_state.total_active_electricity);
        mining_state.total_active_electricity = mining_state.total_active_electricity
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!("   New total active electricity: {}", mining_state.total_active_electricity);
    }
        
    emit!(ElectricityUpdated {
        owner: user_moonbase.owner,
        to_increase,
        amount,
        new_available_electricity: user_moonbase.available_electricity,
        new_total_electricity: mining_state.total_active_electricity,
    });
    
    msg!("✅ Electricity update completed successfully");
    Ok(())
}


// ----------------------------------------------------------------------------------------
// INTERNAL :: SIMPLE DAILY LOGIN AND XP FUNCTIONS ----------------
// ----------------------------------------------------------------------------------------

/// Simple daily login processing with XP award - no loot transfers
/// This can be called from any user function without complex account dependencies
fn process_daily_login_and_xp(user_moonbase: &mut UserMoonBaseInstance, activity_xp: u32, activity_source: &str) -> Result<()> {
    // Process daily login first
    let (daily_login_xp, _streak) = helper::process_daily_login(user_moonbase)?;
    
    // Calculate total XP to award
    let total_xp = daily_login_xp + activity_xp;
    
    if total_xp > 0 {
        // Determine XP source description
        let xp_source = if daily_login_xp > 0 && activity_xp > 0 {
            format!("{} + Daily Login", activity_source)
        } else if daily_login_xp > 0 {
            "Daily Login".to_string()
        } else {
            activity_source.to_string()
        };
        
        // Award XP using simple helper (no loot transfers)
        helper::add_xp_simple(user_moonbase, total_xp, &xp_source)?;
        
        if daily_login_xp > 0 && activity_xp > 0 {
            msg!("🗓️ Combined XP awarded: {} Daily Login + {} {} = {} total (Streak: {})", 
                 daily_login_xp, activity_xp, activity_source, total_xp, user_moonbase.daily_login_streak);
        } else if daily_login_xp > 0 {
            msg!("🗓️ Daily login processed: {} XP gained (Streak: {})", 
                 daily_login_xp, user_moonbase.daily_login_streak);
        }
    }
    
    Ok(())
}



// ------------------------------------------------------------------------------------------
// -------------- ACCOUNT VALIDATION STRUCTURES ----------------------------------------------
// ------------------------------------------------------------------------------------------




#[derive(Accounts)]
#[instruction(referrer: Option<Pubkey>, faction_id: u8)]
pub struct CreateUserMoonbase<'info> {
    #[account(
        init,
        payer = user,
        space = UserMoonBaseInstance::LEN,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    // Create rewards account for the new user 
    #[account(
        init,
        payer = user,
        space = ReferralRewards::LEN,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), user.key().as_ref()],
        bump
    )]
    pub new_user_rewards: Account<'info, ReferralRewards>,
    
    // Only require this account if referrer is provided
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), 
                referrer.as_ref().unwrap_or(&system_program.key()).as_ref()],
        bump,
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that holds collected SOL fees
    pub sol_treasury: UncheckedAccount<'info>,

    /// CHECK: Creation fee recipient account (from global config)
    #[account(
        mut,
        address = global_config.creation_fee_recipient
    )]
    pub creation_fee_recipient: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}






/// Account struct for purchasing an expansion
#[derive(Accounts)]
pub struct ExpandMoonbase<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that holds collected SOL fees
    pub sol_treasury: UncheckedAccount<'info>,

    /// Referrer's rewards account for handling referral payments (optional)
    #[account(
        mut,
        seeds = [REFERRAL_REWARDS_SEED.as_ref(), user_moonbase.referral.as_ref()],
        bump,
    )]
    pub referrer_rewards: Option<Account<'info, ReferralRewards>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct ClaimReferralRewards<'info> {
    #[account(
        mut,
        // Ensure the rewards account owner matches the user
        constraint = referral_rewards.owner == user.key() @ ErrorCode::InvalidReferralAccount,
    )]
    pub referral_rewards: Account<'info, ReferralRewards>,

    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
        
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct UpdateUserElectricity<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [MOON_DOGE_MINING_SEED.as_ref()],
        bump
    )]
    pub mining_state: Account<'info, MoonDogeMining>,
    
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump  
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub authority: Signer<'info>, 
    
    /// CHECK: User wallet (used for moonbase PDA)
    pub user: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}


