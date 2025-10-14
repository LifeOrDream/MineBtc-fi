use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, transfer, Transfer};

use crate::state::*;
use crate::events::*;
use crate::errors::ErrorCode;


/// Calculate mining XP based on mDOGE tokens mined
/// Awards 15 XP per 1000 mDOGE mined
pub fn calculate_mining_xp(tokens_mined: u64) -> u32 {
    let thousands_mined = tokens_mined / 1_000_000_000; // Assuming 9 decimals, so 1000 tokens = 1000 * 10^9
    (thousands_mined as u32) * XP_MINING_1000_MDOGE
}



// Helper function to transfer SOL to the program's sol_treasury PDA
pub fn transfer_to_sol_treasury<'info>(
    from: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_treasury.to_account_info(),
            },
        ),
        amount,
    )
}

/// Helper function to process referral payments
/// 
/// Takes a cost amount, calculates referral fee (if applicable), 
/// transfers SOL to referrer's rewards account and treasury,
/// increments referral count, and returns the referral fee and treasury amount
pub fn process_referral_payment<'info>(
    cost: u64,
    referrer: &Pubkey,
    user_key: &Pubkey,
    user_account_info: &AccountInfo<'info>,
    referrer_rewards: Option<&AccountInfo<'info>>,
    global_config: &mut GlobalConfig,
    sol_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> Result<(u64, u64)> {
    let mut referral_fee = 0;
    let mut treasury_amount = cost;
    
    // Check if we have a referral that's not the default (system program)
    if referrer != &system_program::ID {
        // Calculate referral fee (15% of cost) & treasury amount
        referral_fee = cost.checked_mul(REFERRAL_FEE).unwrap().checked_div(100).unwrap();
        treasury_amount = cost.checked_sub(referral_fee).unwrap();

        // If referrer rewards account is provided, update and transfer
        if let Some(rewards_account_info) = referrer_rewards {
            // Access the data to update the total_sol_earned
            let mut rewards_data = rewards_account_info.try_borrow_mut_data()?;
            let mut rewards = ReferralRewards::try_deserialize(&mut rewards_data.as_ref())?;
            
            // Update the referrer's earned amount
            rewards.total_sol_earned = rewards.total_sol_earned
                .checked_add(referral_fee)
                .unwrap();
                
            // Serialize back to the account
            rewards.try_serialize(&mut *rewards_data)?;

            // Increment total referral sol paid
            global_config.total_referral_sol_paid = global_config.total_referral_sol_paid
                .checked_add(referral_fee)
                .unwrap();
            
            // Transfer SOL to the referrer's PDA account
            transfer(
                CpiContext::new(
                    system_program.to_account_info(),
                    Transfer { 
                        from: user_account_info.to_account_info(), 
                        to: rewards_account_info.to_account_info() 
                    },
                ),
                referral_fee,
            )?;
            
            // Emit event for the referral
            emit!(ReferralRewardsAdded { 
                referrer: *referrer,
                referred_user: *user_key,
                amount: referral_fee 
            });
            
            msg!("Transferred {} lamports to referrer's rewards account", referral_fee);
        } else {
            // No referrer rewards account found, all goes to treasury
            treasury_amount = cost;
            referral_fee = 0;
            msg!("Referrer rewards account not found, all fees go to treasury");
        }
    }
    
    // Transfer the remaining amount to treasury
    transfer_to_sol_treasury(
        user_account_info,
        sol_treasury,
        system_program,
        treasury_amount,
    )?;
    
    // Track total SOL spent by users
    global_config.total_sol_spent = global_config.total_sol_spent
        .checked_add(cost)
        .unwrap_or(global_config.total_sol_spent);
    
    Ok((referral_fee, treasury_amount))
}

/// Update the mining state and distribute MoonDoge tokens
/// This function should be called whenever global hashpower changes
pub fn update_mining_state(
    moon_doge_mining: &mut MoonDogeMining,
) -> Result<()> {
    // Get the current slot
    let current_slot = Clock::get()?.slot;
    
    // If mining hasn't started yet, just update the last slot
    if moon_doge_mining.mining_start_timestamp == 0 {
        moon_doge_mining.last_slot = current_slot;
        return Ok(());
    }
    
    // Calculate slots since last update
    if current_slot <= moon_doge_mining.last_slot {
        // No slots have passed, nothing to update
        return Ok(());
    }
    
    let slots_passed = current_slot - moon_doge_mining.last_slot;
    
    // Calculate current reward rate using dynamic distribution
    let current_reward_rate = calculate_current_reward_rate(moon_doge_mining);
    
    // Calculate new tokens mined in this period
    let new_tokens_mined = slots_passed.checked_mul(current_reward_rate).unwrap_or(0);
    
    // Update total tokens mined
    moon_doge_mining.total_tokens_mined = moon_doge_mining.total_tokens_mined
        .checked_add(new_tokens_mined)
        .unwrap_or(moon_doge_mining.total_tokens_mined);
    
    // Update last processed slot
    moon_doge_mining.last_slot = current_slot;
    
    msg!("Mining state updated: {} new tokens mined, total: {}", 
         new_tokens_mined, moon_doge_mining.total_tokens_mined);
    
    Ok(())
}


// ========== XP AND LEVEL SYSTEM HELPERS ========== //

/// Integer square root implementation for u64
/// Uses binary search to find the largest integer whose square is <= n
pub fn integer_sqrt(n: u64) -> u32 {
    if n == 0 {
        return 0;
    }
    
    let mut left = 1u32;
    let mut right = if n > u32::MAX as u64 { u32::MAX } else { n as u32 };
    let mut result = 0u32;
    
    while left <= right {
        let mid = left + (right - left) / 2;
        let mid_squared = (mid as u64) * (mid as u64);
        
        if mid_squared == n {
            return mid;
        } else if mid_squared < n {
            result = mid;
            left = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            right = mid - 1;
        }
    }
    
    result
}

/// Calculate the current reward rate based on dynamic distribution
fn calculate_current_reward_rate(moon_doge_mining: &MoonDogeMining) -> u64 {
    // If mining hasn't started, return 0
    if moon_doge_mining.mining_start_timestamp == 0 {
        return 0;
    }
    
    // Use the current dynamic distribution rate if available, otherwise fall back to original rate
    if moon_doge_mining.current_dist_rate > 0 {
        moon_doge_mining.current_dist_rate
    } else {
        moon_doge_mining.moon_doge_per_slot
    }
}



/// Initialize default moonbase dimensions for a new user
pub fn initialize_moonbase_dimensions(user_moonbase: &mut UserMoonBaseInstance) -> Result<()> {
    user_moonbase.current_width = DEFAULT_MOONBASE_WIDTH;
    user_moonbase.current_height = DEFAULT_MOONBASE_HEIGHT;
    user_moonbase.purchased_expansions = Vec::new();
    
    msg!("🏗️ Initialized moonbase dimensions: {}x{} ({} tiles)", 
         DEFAULT_MOONBASE_WIDTH, DEFAULT_MOONBASE_HEIGHT, 
         (DEFAULT_MOONBASE_WIDTH as u32) * (DEFAULT_MOONBASE_HEIGHT as u32));
    
    Ok(())
}

// ========== MOONBASE EXPANSION SYSTEM HELPERS ========== //

/// Check if a user can purchase a specific expansion
pub fn can_purchase_expansion(
    user_moonbase: &UserMoonBaseInstance,
    expansion: &ExpansionConfig,
) -> Result<bool> {
    // Check level requirement
    if user_moonbase.level < expansion.required_level {
        return Ok(false);
    }
    
    // Check if already purchased
    if user_moonbase.purchased_expansions.contains(&expansion.id) {
        return Ok(false);
    }
    
    // Check if expansion is active
    if !expansion.is_active {
        return Ok(false);
    }
    
    Ok(true)
}


// --------------------------------------- //
// ========== ENHANCED XP & LEVEL UP SYSTEM =========== //
// --------------------------------------- //

/// Calculate required XP for a specific level using exponential curve: 120 × (1.35^level)
/// Rounds to nearest 10 for clean numbers
pub fn required_xp_new(level: u8) -> u64 {
    msg!("📊 Calculating required XP for level {}:", level);
    
    let mut num: u64 = 1;
    let mut den: u64 = 1;
    
    // Calculate exponential growth
    for i in 0..level {
        num = num.saturating_mul(XP_CURVE_NUM);
        den = den.saturating_mul(XP_CURVE_DEN);
        msg!("   Step {}: {} / {} = {}", 
             i + 1, 
             num, 
             den, 
             (num as f64 / den as f64) * XP_BASE as f64);
    }
    
    // Calculate final XP requirement
    let raw_xp = ((XP_BASE * num) / den + 5) / 10 * 10;   // round to nearest 10
    
    msg!("   Base XP: {}", XP_BASE);
    msg!("   Final calculation: ({} * {}) / {} = {} (rounded to nearest 10)", 
         XP_BASE, num, den, raw_xp);
    
    raw_xp
}


/// Add XP to user moonbase without level progression (XP accumulation only)
/// Level-ups should be handled separately with loot transfers via process_auto_daily_login_and_activity_xp
pub fn add_xp_simple(user_moonbase: &mut UserMoonBaseInstance, xp_amount: u32, source: &str) -> Result<()> {
    if xp_amount == 0 {
        return Ok(());
    }
    
    msg!("🌟 Adding {} XP from: {}", xp_amount, source);
    
    let old_xp = user_moonbase.xp;
    
    // Add XP (convert to u64 for calculation to prevent overflow, then back to u32)
    let new_xp = (user_moonbase.xp as u64).saturating_add(xp_amount as u64);
    user_moonbase.xp = new_xp.min(u32::MAX as u64) as u32;
    
    // Check if user has enough XP for level-ups (but don't actually level up)
    let current_required = required_xp_new(user_moonbase.level);
    if (user_moonbase.xp as u64) >= current_required {
        msg!("💡 User has accumulated enough XP for level-up! Current: {} XP, Required: {} XP", 
             user_moonbase.xp, current_required);
        msg!("   Use claim_level_up_rewards() or similar function to claim level-up with loot transfers");
    } else {
        let remaining = current_required.saturating_sub(user_moonbase.xp as u64);
        msg!("📈 XP progress: {} -> {} (Level {} - need {} more for next level)", 
             old_xp, user_moonbase.xp, user_moonbase.level, remaining);
    }
    
    Ok(())
}




// --------------------------------------- //
// ========== DAILY LOGIN SYSTEM =========== //
// --------------------------------------- //

/// Process daily login and award XP if eligible
/// Returns (xp_gained, new_streak) if login reward was given, or (0, current_streak) if not
pub fn process_daily_login(user: &mut UserMoonBaseInstance) -> Result<(u32, u16)> {
    let current_timestamp = Clock::get()?.unix_timestamp;
    let one_day_seconds = 24 * 60 * 60; // 86400 seconds
    
    // Check if it's been at least 24 hours since last login
    let time_since_last_login = current_timestamp - user.last_login_ts;
    
    if time_since_last_login >= one_day_seconds {
        // Check if the streak should continue (within 48 hours) or reset
        let two_days_seconds = 2 * one_day_seconds;
        
        if time_since_last_login <= two_days_seconds && user.last_login_ts > 0 {
            // Continue streak
            user.daily_login_streak = user.daily_login_streak.saturating_add(1);
        } else {
            // Reset streak (first login or gap > 48 hours)
            user.daily_login_streak = 1;
        }
        
        // Update last login timestamp
        user.last_login_ts = current_timestamp;
        
        // 🔥🔥🔥 DEGEN STREAK XP CALCULATION 🔥🔥🔥
        let base_xp = XP_DAILY_LOGIN; // 10 XP base
        let streak = user.daily_login_streak;
        
        let xp_gained = match streak {
            // Week 1: Build the habit (10-20 XP)
            1..=7 => base_xp + (streak as u32), // Day 7 = 17 XP
            
            // Week 2: Getting serious (21-35 XP) 
            8..=14 => base_xp + 10 + (streak as u32), // Day 14 = 34 XP
            
            // Week 3-4: Degen territory (36-60 XP)
            15..=30 => base_xp + 20 + (streak as u32), // Day 30 = 60 XP
            
            // Month 2: Diamond hands (61-80 XP)
            31..=60 => base_xp + 40 + ((streak - 20) as u32), // Day 60 = 90 XP
            
            // Month 3+: Legendary status (81-100 XP max)
            _ => {
                let capped_streak = std::cmp::min(streak, 90); // Cap at day 90 scaling
                base_xp + 50 + ((capped_streak - 30) as u32) // Day 90+ = 100 XP max
            }
        };
        
        // 🎰 MILESTONE STREAK BONUSES (REASONABLE BUT EXCITING) 🎰
        let milestone_bonus = match streak {
            7 => 50,     // Week milestone: +50 XP bonus
            14 => 75,    // 2 weeks: +75 XP bonus  
            30 => 100,   // Month: +100 XP bonus
            50 => 125,   // 50 days: +125 XP bonus
            69 => 150,   // Nice: +150 XP bonus 😏
            100 => 200,  // 100 days: +200 XP bonus
            150 => 250,  // 150 days: +250 XP bonus
            200 => 300,  // 200 days: +300 XP bonus
            365 => 500,  // 1 YEAR: +500 XP MEGA BONUS!
            500 => 750,  // 500 days: +750 XP bonus
            1000 => 1000, // 1000 days: +1000 XP LEGENDARY BONUS!
            _ => 0,
        };
        
        let total_xp = xp_gained + milestone_bonus;
        user.xp = user.xp.saturating_add(total_xp);
        
        // 🎉 Enhanced logging for degen streaks
        if milestone_bonus > 0 {
            msg!("🔥🔥🔥 MILESTONE STREAK BONUS! 🔥🔥🔥");
            msg!("🗓️ Day {} streak achieved: {} base XP + {} BONUS = {} TOTAL XP!", 
                 streak, xp_gained, milestone_bonus, total_xp);
            msg!("🎯 Keep the streak alive for exponential gains!");
        } else if streak >= 100 {
            msg!("👑 LEGENDARY DEGEN: Day {} streak = {} XP! You're a daily login WHALE! 🐋", 
                 streak, total_xp);
        } else if streak >= 50 {
            msg!("💎 DIAMOND HANDS: Day {} streak = {} XP! Almost to legendary status!", 
                 streak, total_xp);
        } else if streak >= 30 {
            msg!("🚀 MONTH STREAK: Day {} = {} XP! You're entering degen territory!", 
                 streak, total_xp);
        } else if streak >= 14 {
            msg!("⚡ 2+ WEEK STREAK: Day {} = {} XP! The gains are exponential now!", 
                 streak, total_xp);
        } else if streak >= 7 {
            msg!("🔥 WEEK STREAK: Day {} = {} XP! Habit formed, gains accelerating!", 
                 streak, total_xp);
        } else {
            msg!("🗓️ Daily login: Day {} streak = {} XP (Building momentum...)", 
                 streak, total_xp);
        }
        
        // Emit events
        emit!(DailyLoginReward {
            owner: user.owner,
            streak: user.daily_login_streak,
            xp_gained: total_xp,
        });
        
        emit!(XpGained {
            owner: user.owner,
            xp_amount: total_xp,
            xp_source: if milestone_bonus > 0 {
                format!("Daily Login (Day {} + {} Milestone Bonus)", streak, milestone_bonus)
            } else {
                format!("Daily Login (Day {} Streak)", streak)
            },
            total_xp: user.xp,
        });
        
        Ok((total_xp, user.daily_login_streak))
    } else {
        // Not eligible for daily login reward yet
        Ok((0, user.daily_login_streak))
    }
}


/// Process the mining for a specific user
/// This function should be called whenever a user's hashpower changes
pub fn process_user_mining(
    user_moonbase: &mut UserMoonBaseInstance,
    moon_doge_mining: &mut MoonDogeMining,
) -> Result<()> {
    // First update the global mining state to ensure it's current
    update_mining_state(moon_doge_mining)?;
    
    // If there's no global hashpower, nothing to distribute
    if moon_doge_mining.total_active_hashpower == 0 {
        return Ok(());
    }
    
    // Calculate the user's share of global hashpower (as a proportion)
    // We use u128 for precision in intermediate calculations
    let user_hashpower = user_moonbase.active_hashpower as u128;
    let global_hashpower = moon_doge_mining.total_active_hashpower as u128;
    
    // If user has no hashpower, nothing to mine
    if user_hashpower == 0 {
        return Ok(());
    }
    
    // Calculate tokens mined since last claim
    let current_slot = Clock::get()?.slot;
    let slots_since_last_claim = current_slot.saturating_sub(user_moonbase.moondoge_claim_index);
    
    // User's share as a proportion of total (using 10^12 precision)
    let precision = 1_000_000_000_000u128;
    let user_share_precision = user_hashpower.checked_mul(precision).unwrap_or(0) / global_hashpower;
    
    // Calculate tokens mined in this period
    let slots = slots_since_last_claim as u128;
    let rate = calculate_current_reward_rate(moon_doge_mining) as u128;
    let tokens_mined = if let Some(slot_rewards) = slots.checked_mul(rate) {
        if let Some(total_rewards) = slot_rewards.checked_mul(user_share_precision) {
            total_rewards / precision
        } else {
            0
        }
    } else {
        0
    };
    
    // Update the user's claim index to the current slot
    user_moonbase.moondoge_claim_index = current_slot;
    
    // Log the mining activity
    msg!("User mining processed: {} tokens earned with hashpower {} out of global {}",
         tokens_mined, user_hashpower, global_hashpower);
    
    Ok(())
}

/// Transfer claimed MoonDoge tokens to the user (with optional loot rewards)
pub fn claim_moondoge_tokens<'info>(
    user_moonbase: &mut UserMoonBaseInstance,
    moon_doge_mining: &mut MoonDogeMining,
    token_program: &AccountInfo<'info>,
    token_vault: &AccountInfo<'info>,
    token_mint: &AccountInfo<'info>,
    user_token_account: &AccountInfo<'info>,
    vault_authority: &AccountInfo<'info>,
    vault_authority_seeds: &[&[u8]],
    loot_mdoge_vault: Option<&AccountInfo<'info>>,
    loot_rewards: Option<&mut LootRewards>,
) -> Result<u64> {
    // Process mining to ensure up-to-date calculations
    process_user_mining(user_moonbase, moon_doge_mining)?;
    
    // Calculate claimable amount based on hashpower share
    let user_hashpower = user_moonbase.active_hashpower as u128;
    let global_hashpower = moon_doge_mining.total_active_hashpower as u128;
    
    // If user or global hashpower is zero, nothing to claim
    if user_hashpower == 0 || global_hashpower == 0 {
        return Ok(0);
    }
    
    // Calculate the user's share of tokens mined since last claim
    let precision = 1_000_000_000_000u128;
    let user_share_precision = user_hashpower.checked_mul(precision).unwrap_or(0) / global_hashpower;
    
    let current_slot = Clock::get()?.slot;
    let slots_since_last_claim = current_slot.saturating_sub(user_moonbase.moondoge_claim_index);
    
    // Calculate tokens mined in this period
    let slots = slots_since_last_claim as u128;
    let rate = calculate_current_reward_rate(moon_doge_mining) as u128;
    let tokens_mined = if let Some(slot_rewards) = slots.checked_mul(rate) {
        if let Some(total_rewards) = slot_rewards.checked_mul(user_share_precision) {
            total_rewards / precision
        } else {
            0
        }
    } else {
        0
    };
    
    // Ensure we don't claim more than available
    let claimable_amount = tokens_mined.min(u64::MAX as u128) as u64;
    
    // If there's nothing to claim, return early
    if claimable_amount == 0 {
        msg!("No tokens to claim");
        return Ok(0);
    }
    
    // Calculate loot rewards (10% of claimable amount)
    let loot_amount = claimable_amount.checked_mul(LOOT_REWARDS_PERCENTAGE).unwrap().checked_div(100).unwrap();
    let user_amount = claimable_amount.checked_sub(loot_amount).unwrap();
    
    // Transfer tokens from vault to user (90% of claimable amount)
    let ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
        &anchor_spl::token_2022::spl_token_2022::ID,           // program_id
        &token_vault.key(),            // source (vault)
        &token_mint.key(),             // mint            ▲ NEW
        &user_token_account.key(),     // destination
        &vault_authority.key(),        // authority
        &[],                           // signer_pubkeys (vault PDA is a signer later)
        user_amount,                   // amount (90% of total)
        MDOGE_DECIMALS,                             // decimals        ▲ NEW
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            token_program.clone(),
            token_vault.clone(),
            user_token_account.clone(),
            vault_authority.clone(),
        ],
        &[vault_authority_seeds],
    )?;
    
    // Transfer loot rewards to loot vault (10% of claimable amount)
    if loot_amount > 0 && loot_mdoge_vault.is_some() {
        transfer_to_loot_mdoge_vault(
            token_program,
            token_vault,
            loot_mdoge_vault.unwrap(),
            vault_authority,
            token_mint,
            vault_authority_seeds,
            claimable_amount, // Pass total amount, function will calculate 10%
        )?;
        
        // Update loot rewards tracking
        if let Some(loot_rewards_account) = loot_rewards {
            update_loot_rewards_accumulation(loot_rewards_account, claimable_amount, 0)?;
        }
    }
    
    // Update the user's claim index to the current slot
    user_moonbase.moondoge_claim_index = current_slot;
    
    // Log the claim
    msg!("Claimed {} MoonDoge tokens", claimable_amount);
    
    // Return the amount claimed
    Ok(claimable_amount)
}



// ========== LOOT REWARDS SYSTEM HELPERS ========== //

/// Transfer mDOGE tokens to loot rewards vault (10% of distributions)
pub fn transfer_to_loot_mdoge_vault<'info>(
    token_program: &AccountInfo<'info>,
    from_vault: &AccountInfo<'info>,
    to_vault: &AccountInfo<'info>,
    vault_authority: &AccountInfo<'info>,
    token_mint: &AccountInfo<'info>,
    vault_authority_seeds: &[&[u8]],
    amount: u64,
) -> Result<()> {
    let loot_amount = amount.checked_mul(LOOT_REWARDS_PERCENTAGE).unwrap().checked_div(100).unwrap();
    
    if loot_amount > 0 {
        // Transfer mDOGE tokens using Token-2022 instruction directly
        let ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
            &anchor_spl::token_2022::spl_token_2022::ID,
            &from_vault.key(),
            &token_mint.key(),
            &to_vault.key(),
            &vault_authority.key(),
            &[],
            loot_amount,
            MDOGE_DECIMALS, // decimals
        )?;
        
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                token_program.clone(),
                from_vault.clone(),
                to_vault.clone(),
                vault_authority.clone(),
                token_mint.clone(),
            ],
            &[vault_authority_seeds],
        )?;
        
        msg!("🎁 Transferred {} mDOGE tokens to loot vault ({}% of {})", 
             loot_amount, LOOT_REWARDS_PERCENTAGE, amount);
    }
    
    Ok(())
}



/// Update loot rewards accumulation tracking
pub fn update_loot_rewards_accumulation(
    loot_rewards: &mut LootRewards,
    mdoge_amount: u64,
    sol_amount: u64,
) -> Result<()> {
    let mdoge_loot = mdoge_amount.checked_mul(LOOT_REWARDS_PERCENTAGE).unwrap().checked_div(100).unwrap();
    let sol_loot = sol_amount.checked_mul(LOOT_REWARDS_PERCENTAGE).unwrap().checked_div(100).unwrap();
    
    loot_rewards.total_mdoge_accumulated = loot_rewards.total_mdoge_accumulated.checked_add(mdoge_loot).unwrap();
    loot_rewards.total_sol_accumulated = loot_rewards.total_sol_accumulated.checked_add(sol_loot).unwrap();
    
    emit!(LootRewardsAccumulated {
        mdoge_amount: mdoge_loot,
        sol_amount: sol_loot,
        total_mdoge_accumulated: loot_rewards.total_mdoge_accumulated,
        total_sol_accumulated: loot_rewards.total_sol_accumulated,
    });
    
    msg!("🎁 Loot rewards accumulated: {} mDOGE, {} SOL", mdoge_loot, sol_loot);
    
    Ok(())
}



/// Initialize runtime state for a new module instance based on its type
pub fn initialize_module_runtime_state(module_type: &ModuleType, stats: &ModuleStats) -> ModuleRuntimeState {
    match (module_type, stats) {
        (ModuleType::Mining, ModuleStats::Mining(mining_stats)) => {
            ModuleRuntimeState::Mining {
                current_hp: mining_stats.max_hp,
                total_mined: 0,
            }
        },
        (ModuleType::Attraction, ModuleStats::Attraction(attraction_stats)) => {
            ModuleRuntimeState::Attraction {
                current_hp: attraction_stats.max_hp,
                total_xp_generated: 0,
                last_xp_claim: Clock::get().unwrap().unix_timestamp,
            }
        },
        _ => {
            // Fallback for mismatched types (shouldn't happen with proper validation)
            ModuleRuntimeState::Mining {
                current_hp: 100, // Default HP value
                total_mined: 0,
            }
        }
    }
}

// ========== GRID PLACEMENT SYSTEM HELPERS ========== //

/// Check if a module can be placed at the given coordinates
pub fn can_place_module(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<bool> {
    // 1. Bounds check
    if x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? > GRID_WIDTH {
        return Ok(false);
    }
    if y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? > GRID_HEIGHT {
        return Ok(false);
    }
    
    // 2. Overlap check
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            if is_tile_occupied(user_moonbase, tile_x, tile_y)? {
                return Ok(false);
            }
        }
    }
    
    Ok(true)
}

/// Check if a specific tile is occupied
pub fn is_tile_occupied(user_moonbase: &UserMoonBaseInstance, x: u8, y: u8) -> Result<bool> {
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return Err(ErrorCode::InvalidTileIndex.into());
    }
    
    let idx = (y as usize) * (GRID_WIDTH as usize) + (x as usize);
    let byte_idx = idx / 8;
    let bit_idx = idx % 8;
    
    if byte_idx >= BITMAP_SIZE {
        return Err(ErrorCode::InvalidTileIndex.into());
    }
    
    let is_occupied = (user_moonbase.occupied_bitmap[byte_idx] & (1 << bit_idx)) != 0;
    Ok(is_occupied)
}

/// Mark tiles as occupied for a module
pub fn mark_tiles_occupied(
    user_moonbase: &mut UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // Bounds check
    require!(
        x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_WIDTH,
        ErrorCode::InvalidTileIndex
    );
    require!(
        y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_HEIGHT,
        ErrorCode::InvalidTileIndex
    );
    
    // Mark all tiles as occupied
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            let idx = (tile_y as usize) * (GRID_WIDTH as usize) + (tile_x as usize);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            
            if byte_idx >= BITMAP_SIZE {
                return Err(ErrorCode::InvalidTileIndex.into());
            }
            
            user_moonbase.occupied_bitmap[byte_idx] |= 1 << bit_idx;
        }
    }
    
    msg!("🏗️ Marked tiles occupied: ({}, {}) to ({}, {})", 
         x, y, x + width - 1, y + height - 1);
    
    Ok(())
}

/// Clear tiles (mark as unoccupied) for a module
pub fn clear_tiles(
    user_moonbase: &mut UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // Bounds check
    require!(
        x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_WIDTH,
        ErrorCode::InvalidTileIndex
    );
    require!(
        y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? <= GRID_HEIGHT,
        ErrorCode::InvalidTileIndex
    );
    
    // Clear all tiles
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            let idx = (tile_y as usize) * (GRID_WIDTH as usize) + (tile_x as usize);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            
            if byte_idx >= BITMAP_SIZE {
                return Err(ErrorCode::InvalidTileIndex.into());
            }
            
            user_moonbase.occupied_bitmap[byte_idx] &= !(1 << bit_idx);
        }
    }
    
    msg!("🧹 Cleared tiles: ({}, {}) to ({}, {})", 
         x, y, x + width - 1, y + height - 1);
    
    Ok(())
}

/// Place a module at the given coordinates
pub fn place_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &mut ModuleInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<()> {
    // 1. Check if placement is valid
    require!(
        can_place_module(user_moonbase, x, y, width, height)?,
        ErrorCode::TileAlreadyOccupied
    );
    
    // 2. Mark tiles as occupied
    mark_tiles_occupied(user_moonbase, x, y, width, height)?;
    
    // 3. Save coordinates on the module instance
    module_instance.pos_x = x;
    module_instance.pos_y = y;
    module_instance.width = width;
    module_instance.height = height;
    
    msg!("📍 Module placed at ({}, {}) with size {}x{}", x, y, width, height);
    
    Ok(())
}

/// Move a module to new coordinates
pub fn move_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &mut ModuleInstance,
    new_x: u8,
    new_y: u8,
) -> Result<()> {
    // 1. Clear current tiles
    clear_tiles(
        user_moonbase,
        module_instance.pos_x,
        module_instance.pos_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    // 2. Check if new placement is valid
    require!(
        can_place_module(user_moonbase, new_x, new_y, module_instance.width, module_instance.height)?,
        ErrorCode::TileAlreadyOccupied
    );
    
    // 3. Mark new tiles as occupied
    mark_tiles_occupied(
        user_moonbase,
        new_x,
        new_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    // 4. Update coordinates
    let old_x = module_instance.pos_x;
    let old_y = module_instance.pos_y;
    module_instance.pos_x = new_x;
    module_instance.pos_y = new_y;
    module_instance.last_updated = Clock::get()?.unix_timestamp;
    
    msg!("🚚 Module moved from ({}, {}) to ({}, {})", old_x, old_y, new_x, new_y);
    
    Ok(())
}

/// Remove a module and clear its tiles
pub fn remove_module(
    user_moonbase: &mut UserMoonBaseInstance,
    module_instance: &ModuleInstance,
) -> Result<()> {
    // Clear the tiles occupied by this module
    clear_tiles(
        user_moonbase,
        module_instance.pos_x,
        module_instance.pos_y,
        module_instance.width,
        module_instance.height,
    )?;
    
    msg!("🗑️ Module removed from ({}, {}) with size {}x{}", 
         module_instance.pos_x, module_instance.pos_y, 
         module_instance.width, module_instance.height);
    
    Ok(())
}

/// Get the total number of occupied tiles
pub fn get_occupied_tile_count(user_moonbase: &UserMoonBaseInstance) -> u32 {
    let mut count = 0;
    for byte in &user_moonbase.occupied_bitmap {
        count += byte.count_ones();
    }
    count
}

/// Get available tiles count
pub fn get_available_tile_count(user_moonbase: &UserMoonBaseInstance) -> u32 {
    TOTAL_TILES as u32 - get_occupied_tile_count(user_moonbase)
}


/// Calculate XP based on SOL spent using sqrt scaling for diminishing returns
/// Formula: sqrt(lamports) * 500 / 1_000_000_000 (where 1 SOL = 500 XP base)
/// This gives reasonable XP rewards that scale with investment but with diminishing returns
pub fn calculate_sol_based_xp(lamports: u64) -> u32 {
    if lamports == 0 {
        return 0;
    }
    
    // Use sqrt scaling for diminishing returns
    let sqrt_lamports = integer_sqrt(lamports);
    
    // Calculate XP: 500 XP per SOL (sqrt'ed)
    // sqrt_lamports is in sqrt(lamports), so we scale by 500 and divide by sqrt(1e9)
    let xp = sqrt_lamports * 500 / integer_sqrt(1_000_000_000);
    
    // Ensure we give at least 1 XP for any non-zero SOL spent
    xp.max(1)
}

/// Get the current moonbase dimensions
pub fn get_current_moonbase_dimensions(user_moonbase: &UserMoonBaseInstance) -> (u8, u8) {
    (user_moonbase.current_width, user_moonbase.current_height)
}

/// Get the current usable tile count for a user's moonbase
pub fn get_current_usable_tiles(user_moonbase: &UserMoonBaseInstance) -> u32 {
    (user_moonbase.current_width as u32) * (user_moonbase.current_height as u32)
}

/// Check if a module placement is within the user's current moonbase boundaries
pub fn can_place_module_in_moonbase(
    user_moonbase: &UserMoonBaseInstance,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
) -> Result<bool> {
    // 1. Check if within current moonbase bounds (not full grid)
    if x.checked_add(width).ok_or(ErrorCode::ArithmeticOverflow)? > user_moonbase.current_width {
        return Ok(false);
    }
    if y.checked_add(height).ok_or(ErrorCode::ArithmeticOverflow)? > user_moonbase.current_height {
        return Ok(false);
    }
    
    // 2. Check overlap with existing modules
    for dy in 0..height {
        for dx in 0..width {
            let tile_x = x.checked_add(dx).ok_or(ErrorCode::ArithmeticOverflow)?;
            let tile_y = y.checked_add(dy).ok_or(ErrorCode::ArithmeticOverflow)?;
            
            if is_tile_occupied(user_moonbase, tile_x, tile_y)? {
                return Ok(false);
            }
        }
    }
    
    Ok(true)
}
