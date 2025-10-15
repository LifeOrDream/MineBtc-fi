use anchor_lang::prelude::*;

use crate::state::*;
use crate::events::*;
use crate::errors::ErrorCode;
use crate::instructions::helper::{self, transfer_to_sol_treasury};
use crate::mpl_core_helpers;

use anchor_spl::token_interface::{ Mint as Mint2022 };


// ----------------------------------------------------------------------------------------
// -------------- USER FUNCTIONS :: CREATE MOON-BASE, EXPAND MOONBASE ----------------
// ----------------------------------------------------------------------------------------

/// Creates a new moon base for a user
/// This can only be called once per user
/// pricing_tier: MOONBASE_BASIC_PRICE (0.5 SOL, no NFT) or MOONBASE_EGG_PRICE (1.42 SOL, + Dragon Egg)
pub fn initialize_user_moonbase(ctx: Context<CreateUserMoonbase>, referrer: Option<Pubkey>, faction_id: u8, pricing_tier: u64) -> Result<()> {

    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let new_rewards = &mut ctx.accounts.new_user_rewards;
    let doge_btc_mining = &ctx.accounts.doge_btc_mining;
    let user = &ctx.accounts.user;

    // Get moonbase count before mutable borrow
    let moonbase_count = {
        let global_config = &ctx.accounts.global_config;
        global_config.total_moonbases_created
    };

    // Determine pricing and NFT minting
    let sol_cost: u64;
    let to_mint_dragon: bool;
    
    
    if pricing_tier == PRICE_ONE {
        sol_cost = PRICE_ONE;
        to_mint_dragon = false;
    } else if pricing_tier == PRICE_TWO {
        sol_cost = PRICE_TWO;
        to_mint_dragon = true;
    } else {
        return Err(ErrorCode::InvalidParameters.into());
    }

    let global_config = &mut ctx.accounts.global_config;

    // Increment total moonbases created and total sol spent
    global_config.total_moonbases_created = global_config.total_moonbases_created.saturating_add(1);
    global_config.total_sol_spent = global_config.total_sol_spent.saturating_add(sol_cost);
    
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
    let fee_recipient_amount = sol_cost / 2; // 50% goes to creation fee recipient
    let remaining_amount = sol_cost - fee_recipient_amount; // 50% goes through existing system
    
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
            global_config,
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
    user_moonbase.dbtc_claim_index =  doge_btc_mining.dbtc_tokens_minted_per_hashpower;
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

    // Initialize PVP and Dragon Egg fields
    user_moonbase.pvp_hp = 0;
    user_moonbase.active_game = None;
    user_moonbase.last_game_end_ts = 0;
    user_moonbase.modules_repaired_since_last_game = false;
    user_moonbase.incubated_dragon_egg = None;

    // Mint Dragon Egg NFT if tier includes it
    if to_mint_dragon {
        // Unwrap optional accounts (required for NFT minting)
        let dragon_egg_asset = ctx.accounts.dragon_egg_asset.as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let dragon_egg_collection = ctx.accounts.dragon_egg_collection.as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let mpl_core_program = ctx.accounts.mpl_core_program.as_ref()
            .ok_or(ErrorCode::InvalidAccount)?;
        let egg_metadata = ctx.accounts.dragon_egg_metadata.as_mut()
            .ok_or(ErrorCode::InvalidAccount)?;

        let name = format!("Dragon Egg #{}", moonbase_count);

        // Generate DNA for the egg
        let dna = generate_dragon_egg_dna(
            Clock::get()?.slot,
            &user.key(),
            moonbase_count,
        );

        // Get URI from global config or use fallback
        let uri = global_config.get_random_dragon_egg_uri(
            Clock::get()?.slot,
            moonbase_count,
            &dna,
        ).unwrap_or_else(|_| format!("https://arweave.net/dragonegg/{}", moonbase_count));

        // Get global config as AccountInfo before using mutable reference
        let global_config_info = global_config.to_account_info();

        // Create Dragon Egg NFT with MPL Core
        crate::mpl_core_helpers::create_mpl_core_asset(
            dragon_egg_asset,
            Some(dragon_egg_collection),
            &global_config_info,
            &user.to_account_info(),
            &user.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            mpl_core_program,
            name.clone(),
            uri.clone(),
        )?;

        // Initialize Dragon Egg metadata
        egg_metadata.mint = dragon_egg_asset.key();
        egg_metadata.power = BASE_EGG_POWER;
        egg_metadata.dna = dna;
        egg_metadata.incubated_moonbase = None;
        egg_metadata.last_update_ts = Clock::get()?.unix_timestamp;
        egg_metadata.created_at = Clock::get()?.unix_timestamp;
        egg_metadata.bump = ctx.bumps.dragon_egg_metadata.unwrap();

        // Update global dragon egg counter
        global_config.total_dragon_eggs_minted = global_config.total_dragon_eggs_minted.saturating_add(1);

        // Emit events
        emit!(DragonEggMinted {
            mint: egg_metadata.mint,
            name,
            uri,
            dna,
            initial_power: BASE_EGG_POWER,
            price_paid: 0, // Included in moonbase price
        });

        msg!("✅ Dragon Egg minted for moonbase creation");
        msg!("   Egg: {}", egg_metadata.mint);
    }

    // Emit event
    emit!(UserMoonBaseCreated {
        owner: user.key(),
        referrer,
    });

    msg!("Created new moon base for user {} with tier: {} SOL {}",
         user.key(), sol_cost as f64 / 1_000_000_000.0,
         if to_mint_dragon { "(includes Dragon Egg)" } else { "(no NFT)" });

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
 


/// Update user's available electricity based on DogeBtc staking
pub fn update_user_electricity_internal(ctx: Context<UpdateUserElectricity>, to_increase: bool, amount: u64) -> Result<()> {
    msg!("⚡ Updating user electricity - Increase: {}, Amount: {}", to_increase, amount);
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let mining_state = &mut ctx.accounts.mining_state;
    
    // Process daily login automatically (no loot/level stats for this admin function)
    let (_xp_gained, _streak) = helper::process_daily_login(user_moonbase)?;

    // Check if the authority is the fee_collector 
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
// -------------- USER :: MINING FUNCTIONS ---------------------------------------------------
// ------------------------------------------------------------------------------------------

/// Claim DogeBtc tokens that the user has mined
pub fn claim_dbtc_tokens_internal(
    ctx: Context<ClaimDogeBtc>,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let doge_btc_mining = &mut ctx.accounts.doge_btc_mining;
    let user = &ctx.accounts.user;
    
    // Ensure mining has been initialized
    require!(
        doge_btc_mining.mining_start_timestamp > 0,
        ErrorCode::MiningNotInitialized
    );

    // Process any pending mining rewards
    helper::mine_dbtc_for_user(user_moonbase, doge_btc_mining)?;

    // Get vault authority signer seeds
    let vault_seeds = &[
        DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref(),
        &[doge_btc_mining.vault_auth_bump],
    ];

    // Get account info before using doge_btc_mining as a mutable reference
    let mining_account_info = doge_btc_mining.to_account_info();
    
    // Claim tokens
    let claimed_amount = helper::claim_dogebtc_tokens(
        user_moonbase,
        doge_btc_mining,
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.token_vault.to_account_info(),
        &ctx.accounts.token_mint.to_account_info(),
        &ctx.accounts.user_token_account.to_account_info(),
        &mining_account_info,
        vault_seeds,
        Some(&ctx.accounts.loot_dbtc_vault.to_account_info()),
        Some(&mut ctx.accounts.loot_rewards),
    )?;

    // Update Dragon Egg power if one is incubated
    if let Some(egg_metadata_pubkey) = user_moonbase.incubated_dragon_egg {
        // If moonbase has an incubated egg, egg accounts MUST be provided
        let egg_metadata = ctx.accounts.dragon_egg_metadata.as_mut()
            .ok_or(ErrorCode::InvalidAccount)?;
        let incubation_state = ctx.accounts.incubation_state.as_mut()
            .ok_or(ErrorCode::InvalidAccount)?;

        // Verify this is the correct egg
        require!(
            egg_metadata.key() == egg_metadata_pubkey,
            ErrorCode::InvalidAccount
        );
        
        let current_time = Clock::get()?.unix_timestamp;
        
        // Calculate power increase based on claimed amount
        // Formula: power_increase = claimed_amount / POWER_RATE_MULTIPLIER
        let power_increase = (claimed_amount / POWER_RATE_MULTIPLIER) as u32;
        
        let old_power = egg_metadata.power;
        let new_power = old_power.saturating_add(power_increase).min(MAX_EGG_POWER);
        
        egg_metadata.power = new_power;
        egg_metadata.last_update_ts = current_time;
        
        incubation_state.total_power = new_power as u64;
        incubation_state.last_update_ts = current_time;
        
        msg!("🥚 Dragon Egg power updated: {} -> {} (+{})", old_power, new_power, power_increase);
    }

    // Process daily login and award XP based on tokens mined
    let mining_xp = helper::calculate_mining_xp(claimed_amount);
    process_daily_login_and_xp(user_moonbase, mining_xp, "Mining")?;

    // Emit event
    emit!(DogeBtcTokensClaimed { owner: user.key(), amount: claimed_amount });

    msg!("User {} claimed {} DogeBtc tokens", user.key(), claimed_amount);

    Ok(())
}



 
// ----------------------------------------------------------------------------------------
// -------------- USER :: MODULE MANAGEMENT FUNCTIONS -------------------------------------------
// ----------------------------------------------------------------------------------------



/// Buy a module - purchase and create undeployed ModuleInstance
pub fn buy_module(
    ctx: Context<BuyModule>,
    config_id: u16,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    let current_timestamp = Clock::get()?.unix_timestamp;

    msg!("🛒 Starting module purchase for user {}", user.key());
    msg!("   Config ID: {}", config_id);
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    
    // Validate module is active
    require!(module_config.is_active, ErrorCode::ModuleNotActive);
    msg!("✅ Module config {} is active", config_id);
    
    // Validate user level requirement
    require!(  user_moonbase.level >= module_config.min_level,  ErrorCode::UserLevelTooLow);
    msg!("✅ User level {} meets requirement of {}", user_moonbase.level, module_config.min_level);
    
    // Validate faction access (if faction restrictions exist)
    if !module_config.faction_ids.is_empty() {
        require!(  module_config.faction_ids.contains(&user_moonbase.faction_id),  ErrorCode::FactionNotAllowed);
        msg!("✅ User faction {} is allowed for this module", user_moonbase.faction_id);
    }
    
    // Check if user has enough SOL for the module cost
    let cost = module_config.mint_cost;
    msg!("💰 Module cost: {} SOL", cost as f64 / 1e9);
    
    // Process the payment, handle referral if applicable
    let referrer = user_moonbase.referral;
    
    // Handle the referral payment
    let (referral_fee, _) = helper::process_referral_payment(
        cost,
        &referrer,
        &user.key(),
        &user.to_account_info(),
        ctx.accounts.referral_rewards.as_ref().map(|acc| acc.to_account_info()).as_ref(),
        &mut ctx.accounts.global_config,
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    
    msg!("💰 Payment processed: {} SOL cost, {} SOL referral fee", 
         cost as f64 / 1e9, referral_fee as f64 / 1e9);
    
    // Update available modules count
    let found_entry = user_moonbase.available_modules.iter_mut()
        .find(|entry| entry.config_id == config_id);
    
    if let Some(entry) = found_entry {
        // Increment count for existing entry
        entry.count += 1;
    } else {
        // Create new entry
        let new_entry = AvailableModuleEntry { 
            config_id, 
            count: 1 
        };
        user_moonbase.available_modules.push(new_entry);
    }
    
    // Calculate electricity cost based on module type and stats
    let electricity_cost = match &module_config.stats {
        ModuleStats::Mining(stats) => stats.power_consumption as u32,
        ModuleStats::Attraction(stats) => stats.power_consumption as u32,
    };
    
    // Initialize runtime state based on module type (at full HP but not deployed)
    let runtime_state = helper::initialize_module_runtime_state(&module_config.module_type, &module_config.stats);
    
    // Create the ModuleInstance (undeployed)
    let module_instance = &mut ctx.accounts.module_instance;
    let module_index = user_moonbase.modules_count;
    
    // Initialize module instance fields (NOT DEPLOYED YET)
    module_instance.config_id = config_id;
    module_instance.upgrade_level = 0; // Start at level 0
    module_instance.index = module_index;
    module_instance.module_type = module_config.module_type.clone();
    module_instance.runtime_state = runtime_state;
    module_instance.pos_x = 0;     // Not placed yet
    module_instance.pos_y = 0;     // Not placed yet
    module_instance.width = module_config.width;
    module_instance.height = module_config.height;

    module_instance.electricity_cost = electricity_cost;
    module_instance.is_active = false; // NOT DEPLOYED YET
    module_instance.created_at = current_timestamp;
    module_instance.last_updated = current_timestamp;
    module_instance.bump = ctx.bumps.module_instance;
    
  
    // Update user_moonbase counters
    user_moonbase.modules_count += 1;

    // Process daily login and award XP for purchasing module (based on SOL spent)
    let purchase_xp = helper::calculate_sol_based_xp(cost);
    process_daily_login_and_xp(
        user_moonbase,
        purchase_xp,
        &format!("Purchase Module ({} SOL)", cost as f64 / 1e9),
    )?;
    
    // Emit event
    emit!(ModulePurchased {
        owner: user.key(),
        config_id,
        module_index,
        cost,
        referral_fee,
    });
    
    msg!("🎉 Module purchase completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Config ID: {}, Module Index: {}", config_id, module_index);
    msg!("   Cost: {} SOL, Referral Fee: {} SOL", cost as f64 / 1e9, referral_fee as f64 / 1e9);
    msg!("   Module created but not deployed (is_active: false)");
    
    Ok(())
}


/// Install/deploy an existing undeployed module to specific coordinates
pub fn install_module(
    ctx: Context<InstallModule>,
    module_index: u8,
    pos_x: u8,
    pos_y: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_instance = &mut ctx.accounts.module_instance;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    
    msg!("🏗️ Starting module installation for user {}", user.key());
    msg!("   Module Index: {}, Position: ({}, {})", module_index, pos_x, pos_y);
    
    // Validate module is not already deployed
    require!(!module_instance.is_active, ErrorCode::ModuleAlreadyActive);
    msg!("✅ Module is undeployed and ready for installation");
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    
    // Validate config_id matches the module instance
    require!(
        module_config.id == module_instance.config_id,
        ErrorCode::ModuleConfigMismatch
    );
    
    msg!("✅ Installing module at position ({}, {})", pos_x, pos_y);
    
    // Calculate electricity cost based on current upgrade level
    let electricity_cost = module_instance.electricity_cost as u64;
    
    msg!("📊 Module electricity requirement: {} units", electricity_cost);
    
    // Check electricity availability BEFORE placement
    if electricity_cost > 0 {
        let new_used_electricity = user_moonbase.used_electricity
            .checked_add(electricity_cost)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        require!(
            new_used_electricity <= user_moonbase.available_electricity,
            ErrorCode::ElectricityCapacityExceeded
        );
        
        msg!("✅ Electricity check passed: {} + {} = {} <= {}", 
             user_moonbase.used_electricity, 
             electricity_cost,
             new_used_electricity,
             user_moonbase.available_electricity);
    }
    
    // Check placement within moonbase bounds and no collision
    require!(
        helper::can_place_module_in_moonbase(
            user_moonbase,
            pos_x,
            pos_y,
            module_instance.width,
            module_instance.height,
        )?,
        ErrorCode::PlacementOutsideMoonbaseArea
    );
    
    msg!("✅ Module placement validated at ({}, {}) with size {}x{}", 
         pos_x, pos_y, module_instance.width, module_instance.height);
    
    // Get width and height before the mutable borrow
    let module_width = module_instance.width;
    let module_height = module_instance.height;
    
    // Place the module on the grid using the placement system
    helper::place_module(
        user_moonbase,
        module_instance,
        pos_x,
        pos_y,
        module_width,
        module_height,
    )?;
    
    msg!("📍 Module placed successfully on grid");
    
    // Update module instance to be deployed
    module_instance.is_active = true;
    module_instance.last_updated = Clock::get()?.unix_timestamp;
    
    // Update electricity usage
    if electricity_cost > 0 {
        user_moonbase.used_electricity = user_moonbase.used_electricity
            .checked_add(electricity_cost)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("⚡ Updated electricity usage: {} -> {} / {}", 
             user_moonbase.used_electricity - electricity_cost,
             user_moonbase.used_electricity,
             user_moonbase.available_electricity);
    }
    
    // Update total moonbase HP (add module's HP to moonbase) based on current level
    let module_max_hp = match &module_config.stats {
        ModuleStats::Mining(stats) => stats.max_hp,
        ModuleStats::Attraction(stats) => stats.max_hp,
    };
    
    user_moonbase.pvp_hp = user_moonbase.pvp_hp.saturating_add(module_max_hp);
    msg!("🛡️ Updated moonbase HP: added {} HP, total now: {}", module_max_hp, user_moonbase.pvp_hp);
    
    // Update hashpower if this is a mining module (based on current upgrade level)
    if let ModuleStats::Mining(mining_stats) = &module_config.stats {
        // Mine pending rewards BEFORE changing hashpower
        helper::mine_dbtc_for_user(user_moonbase, &mut ctx.accounts.doge_btc_mining)?;
        
        let hashpower_increase = mining_stats.current_hashpower(module_instance.upgrade_level) as u64;
        let old_hashpower = user_moonbase.active_hashpower;
        
        user_moonbase.active_hashpower = user_moonbase.active_hashpower
            .checked_add(hashpower_increase)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("⛏️ Updated hashpower: {} -> {} (+{}) at level {}", 
             old_hashpower, user_moonbase.active_hashpower, hashpower_increase, module_instance.upgrade_level);
        
        // Update global hashpower
        let mining_state = &mut ctx.accounts.doge_btc_mining;
        let old_global_hashpower = mining_state.total_active_hashpower;
        mining_state.total_active_hashpower = mining_state.total_active_hashpower
            .checked_add(hashpower_increase)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("🌐 Updated global hashpower: {} -> {} (+{})", 
             old_global_hashpower, mining_state.total_active_hashpower, hashpower_increase);
    }
    
    // Process daily login and award XP for installing module
    let installation_xp = 25; // Fixed XP for installation
    process_daily_login_and_xp(  user_moonbase,  installation_xp, "Install Module")?;
    
    // Emit event
    emit!(ModuleInstalled {
        owner: user.key(),
        config_id: module_instance.config_id,
        module_index,
        pos_x,
        pos_y,
    });
    
    msg!("🎉 Module installation completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Module Index: {}, Config ID: {}", module_index, module_instance.config_id);
    msg!("   Position: ({}, {}), Size: {}x{}", pos_x, pos_y, module_instance.width, module_instance.height);
    msg!("   Upgrade Level: {}, Electricity Used: {} / {}", 
         module_instance.upgrade_level, user_moonbase.used_electricity, user_moonbase.available_electricity);
    msg!("   Module is now deployed and active!");
    
    Ok(())
}

/// Delete a specific undeployed module instance permanently
pub fn delete_module(
    ctx: Context<DeleteModule>,
    module_index: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_instance = &ctx.accounts.module_instance;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    
    msg!("🗑️ Starting module deletion for user {}", user.key());
    msg!("   Module Index: {}, Config ID: {}", module_index, module_instance.config_id);
    
    // Verify module is undeployed before deletion
    require!(!module_instance.is_active, ErrorCode::ModuleAlreadyActive);
    msg!("✅ Module is undeployed and can be deleted");
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    let config_id = module_instance.config_id;
    
    // Find the available module entry
    let available_entry_index = user_moonbase.available_modules.iter()
        .position(|entry| entry.config_id == config_id)
        .ok_or(ErrorCode::ModuleInstanceNotFound)?;
    
    // Validate there's at least one available module of this type
    require!(
        user_moonbase.available_modules[available_entry_index].count > 0,
        ErrorCode::ModuleInstanceNotFound
    );
    
    msg!("✅ Module deletion validation passed");
    msg!("   Available modules of this type: {}", user_moonbase.available_modules[available_entry_index].count);
    
    // Decrement the available modules count
    let count = &mut user_moonbase.available_modules[available_entry_index].count;
    *count -= 1;
    if *count == 0 {
        user_moonbase.available_modules.remove(available_entry_index);
        msg!("🗑️ Removed module config {} from available modules (count reached 0)", config_id);
    } else {
        msg!("🗑️ Decremented module config {} count: {} -> {}", config_id, *count + 1, *count);
    }
    
    // Process daily login (no XP for deletion)
    let (daily_login_xp, _streak) = helper::process_daily_login(user_moonbase)?;
    if daily_login_xp > 0 {
        helper::add_xp_simple(user_moonbase, daily_login_xp, "Daily Login")?;
        msg!("🗓️ Daily login processed: {} XP gained", daily_login_xp);
    }
    
    // Emit event
    emit!(ModuleDeleted {
        owner: user.key(),
        config_id,
        remaining_count: user_moonbase.available_modules.iter().find(|entry| entry.config_id == config_id).map(|entry| entry.count).unwrap_or(0),
    });
    
    msg!("🎉 Module deletion completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Module Index: {}, Config ID: {}", module_index, config_id);
    msg!("   Module type: {} permanently deleted", module_config.name);
    msg!("   ModuleInstance account will be closed and rent returned to user");
    
    Ok(())
} 


/// Remove/undeploy a module instance from the moonbase (makes it undeployed but keeps it owned)
pub fn remove_module_internal(
    ctx: Context<RemoveModuleInstance>,
    module_index: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_instance = &mut ctx.accounts.module_instance;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    
    msg!("🗑️ Starting module removal for user {}", user.key());
    msg!("   Module index: {}, Type: {:?}", module_index, module_instance.module_type);
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    
    // Verify module is deployed before removal
    require!(module_instance.is_active, ErrorCode::ModuleNotActive);
    msg!("✅ Module is deployed and can be removed");
    
    // Clear the module's position on the grid
    helper::remove_module(user_moonbase, module_instance)?;
    
    // Set module as undeployed but keep the instance
    let module_instance = &mut ctx.accounts.module_instance;
    module_instance.is_active = false;
    module_instance.pos_x = 0; // Reset position
    module_instance.pos_y = 0; // Reset position
    module_instance.last_updated = Clock::get()?.unix_timestamp;
    
    // Reduce electricity consumption
    let electricity_reduction = module_instance.electricity_cost as u64;
    if electricity_reduction > 0 {
        user_moonbase.used_electricity = user_moonbase.used_electricity
            .saturating_sub(electricity_reduction);
        
        msg!("⚡ Reduced electricity usage: -{} units (new usage: {} / {})", 
             electricity_reduction, 
             user_moonbase.used_electricity, 
             user_moonbase.available_electricity);
    }
    
    // Update hashpower if this is a mining module
    if let ModuleStats::Mining(mining_stats) = &module_config.stats {
        // Mine pending rewards BEFORE changing hashpower
        helper::mine_dbtc_for_user(user_moonbase, &mut ctx.accounts.doge_btc_mining)?;
        
        let hashpower_reduction = mining_stats.current_hashpower(module_instance.upgrade_level) as u64;
        let old_hashpower = user_moonbase.active_hashpower;
        
        user_moonbase.active_hashpower = user_moonbase.active_hashpower
            .saturating_sub(hashpower_reduction);
        
        msg!("⛏️ Reduced hashpower: {} -> {} (-{})", 
             old_hashpower, user_moonbase.active_hashpower, hashpower_reduction);
        
        // Update global hashpower
        let mining_state = &mut ctx.accounts.doge_btc_mining;
        let old_global_hashpower = mining_state.total_active_hashpower;
        mining_state.total_active_hashpower = mining_state.total_active_hashpower
            .saturating_sub(hashpower_reduction);
        
        msg!("🌐 Reduced global hashpower: {} -> {} (-{})", 
             old_global_hashpower, mining_state.total_active_hashpower, hashpower_reduction);
    }
    
    // Update total moonbase HP (subtract module's HP from moonbase)
    let module_max_hp = match &module_config.stats {
        ModuleStats::Mining(stats) => stats.max_hp,
        ModuleStats::Attraction(stats) => stats.max_hp,
    };
    
    user_moonbase.pvp_hp = user_moonbase.pvp_hp.saturating_sub(module_max_hp);
    msg!("🛡️ Reduced moonbase HP: -{} HP, total now: {}", module_max_hp, user_moonbase.pvp_hp);
    
    // Emit event
    emit!(ModuleInstanceRemoved {
        owner: user.key(),
        module_index,
        module_type: module_instance.module_type.clone(),
        position_x: module_instance.pos_x,
        position_y: module_instance.pos_y,
        electricity_freed: electricity_reduction,
        hashpower_lost: if let ModuleStats::Mining(mining_stats) = &module_config.stats {
            mining_stats.current_hashpower(module_instance.upgrade_level) as u64
        } else {
            0
        },
    });
    
    msg!("🎉 Module removal completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Module Index: {}", module_index);
    msg!("   Module is now undeployed but still owned (is_active: false)");
    msg!("   Can be redeployed later or deleted permanently");
    
    Ok(())
}
 
/// Upgrade a module instance (deployed or undeployed)
pub fn upgrade_module_internal (
    ctx: Context<UpdateModuleInstance>,
    module_index: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_instance = &mut ctx.accounts.module_instance;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    
    msg!("⬆️ Starting module upgrade for user {}", user.key());
    msg!("   Module index: {}, Current level: {}, Deployed: {}", 
         module_index, module_instance.upgrade_level, module_instance.is_active);
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    
    // Make sure the module is not already at max level
    require!(
        module_instance.upgrade_level < module_config.max_upgrades(),
        ErrorCode::ModuleInstanceMaxLevel
    );
    
    // Calculate new upgrade level
    let new_upgrade_level = module_instance.upgrade_level + 1;
    
    // Check moonbase level requirement for this upgrade
    if let Some(required_level) = module_config.get_upgrade_level_requirement(new_upgrade_level) {
        require!(
            user_moonbase.level >= required_level,
            ErrorCode::UserLevelTooLow
        );
        msg!("✅ User level {} meets upgrade requirement of {}", user_moonbase.level, required_level);
    }
    
    // Calculate hashpower difference for mining modules BEFORE upgrade (only if deployed)
    let old_hashpower = if module_instance.is_active {
        if let ModuleStats::Mining(mining_stats) = &module_config.stats {
            if let ModuleRuntimeState::Mining { .. } = &module_instance.runtime_state {
                Some(mining_stats.current_hashpower(module_instance.upgrade_level) as u64)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    
    // Get the cost to upgrade this module (progressive pricing)
    let cost = module_config.next_upgrade_cost(module_instance.upgrade_level);
    
    msg!("💰 Progressive upgrade cost: {} SOL (base: {} SOL, level {} -> {})", 
         cost as f64 / 1e9, 
         module_config.upgrade_cost as f64 / 1e9,
         module_instance.upgrade_level,
         new_upgrade_level);
    
    // Process the payment, handle referral if applicable
    let referrer = user_moonbase.referral;
    
    // Handle the referral payment
    let (referral_fee, _) = helper::process_referral_payment(
        cost,
        &referrer,
        &user.key(),
        &user.to_account_info(),
        ctx.accounts.referral_rewards.as_ref().map(|acc| acc.to_account_info()).as_ref(),
        &mut ctx.accounts.global_config,
        &ctx.accounts.sol_treasury.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    
    msg!("💰 Payment processed: {} SOL cost, {} SOL referral fee", 
         cost as f64 / 1e9, referral_fee as f64 / 1e9);
    
    // Update the module instance
    module_instance.upgrade_level = new_upgrade_level;
    module_instance.last_updated = Clock::get()?.unix_timestamp;
    
    // Update electricity cost for the new level
    let new_electricity_cost = match &module_config.stats {
        ModuleStats::Mining(stats) => stats.power_consumption as u32,
        ModuleStats::Attraction(stats) => stats.power_consumption as u32,
    };
    module_instance.electricity_cost = new_electricity_cost;
    
    // If module is deployed, update user and global stats
    if module_instance.is_active {
        msg!("🔄 Module is deployed - updating user and global stats");
        
        // Update hashpower if this is a mining module
        if let (Some(old_hp), ModuleStats::Mining(mining_stats)) = (old_hashpower, &module_config.stats) {
            if let ModuleRuntimeState::Mining { .. } = &module_instance.runtime_state {
                // Mine pending rewards BEFORE changing hashpower
                helper::mine_dbtc_for_user(user_moonbase, &mut ctx.accounts.doge_btc_mining)?;
                
                let new_hashpower = mining_stats.current_hashpower(new_upgrade_level) as u64;
                let hashpower_increase = new_hashpower.saturating_sub(old_hp);
                
                if hashpower_increase > 0 {
                    user_moonbase.active_hashpower = user_moonbase.active_hashpower
                        .checked_add(hashpower_increase)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;
                    
                    msg!("⛏️ Updated hashpower: {} -> {} (+{})", 
                         user_moonbase.active_hashpower - hashpower_increase, 
                         user_moonbase.active_hashpower, 
                         hashpower_increase);
                         
                    // Update global hashpower for upgrades
                    let mining_state = &mut ctx.accounts.doge_btc_mining;
                    let old_global_hashpower = mining_state.total_active_hashpower;
                    mining_state.total_active_hashpower = mining_state.total_active_hashpower
                        .checked_add(hashpower_increase)
                        .ok_or(ErrorCode::ArithmeticOverflow)?;
                    
                    msg!("🌐 Updated global hashpower: {} -> {} (+{})", 
                         old_global_hashpower, mining_state.total_active_hashpower, hashpower_increase);
                }
            }
        }
    } else {
        msg!("⚠️ Module is undeployed - stats will be applied when deployed");
    }
    
    // Process daily login and award XP for upgrading module (based on SOL spent)
    let upgrade_xp = helper::calculate_sol_based_xp(cost);
    process_daily_login_and_xp(
        user_moonbase,
        upgrade_xp,
        &format!("Upgrade Module ({} SOL)", cost as f64 / 1e9),
    )?;
    
    // Emit event
    emit!(ModuleInstanceUpgraded {
        owner: user.key(),
        module_index,
        new_upgrade_level,
        cost,
        referral_fee,
    });
    
    msg!("🎉 Module upgrade completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Module Index: {}, New Level: {}", module_index, new_upgrade_level);
    msg!("   Cost: {} SOL, Referral Fee: {} SOL", cost as f64 / 1e9, referral_fee as f64 / 1e9);
    msg!("   Deployed: {}", module_instance.is_active);
    
    Ok(())
}
 


/// Claim accumulated XP from Attraction modules
pub fn claim_attraction_xp_internal(
    ctx: Context<ClaimAttractionXP>,
    module_index: u8,
) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let module_instance = &mut ctx.accounts.module_instance;
    let module_config_account = &ctx.accounts.module_config_account;
    let user = &ctx.accounts.user;
    
    msg!("🎯 Starting Attraction XP claim for user {}", user.key());
    msg!("   Module index: {}", module_index);
    
    // Get the module config from the individual PDA
    let module_config = &module_config_account.data;
    
    // Ensure this is an Attraction module
    require!(
        module_config.module_type == ModuleType::Attraction,
        ErrorCode::InvalidModuleType
    );
    
    // Get attraction stats
    let attraction_stats = match &module_config.stats {
        ModuleStats::Attraction(stats) => stats,
        _ => return Err(ErrorCode::ModuleTypeMismatch.into()),
    };
    
    // Get runtime state for Attraction module
    let (current_hp, last_xp_claim) = match &module_instance.runtime_state {
        ModuleRuntimeState::Attraction { current_hp, last_xp_claim, .. } => (*current_hp, *last_xp_claim),
        _ => return Err(ErrorCode::ModuleTypeMismatch.into()),
    };
    
    msg!("✅ Module validation passed - Attraction module with {} HP", current_hp);
    
    // Check if module is active and has HP
    require!(module_instance.is_active, ErrorCode::ModuleNotActive);
    require!(current_hp > 0, ErrorCode::ModuleDamaged);
    
    // Calculate time elapsed since last claim
    let current_time = Clock::get()?.unix_timestamp;
    let time_elapsed = current_time.saturating_sub(last_xp_claim);
    
    // Convert seconds to hours (with precision)
    let hours_elapsed = time_elapsed as f64 / 3600.0;
    
    msg!("⏰ Time calculation:");
    msg!("   Last claim: {}", last_xp_claim);
    msg!("   Current time: {}", current_time);
    msg!("   Time elapsed: {} seconds ({:.2} hours)", time_elapsed, hours_elapsed);
    
    // Return early if less than 1 minute has passed (prevent spam)
    if time_elapsed < 60 {
        msg!("⚠️ Must wait at least 1 minute between claims");
        return Ok(());
    }
    
    // Calculate current XP per hour based on upgrade level
    let base_xp_per_hour = attraction_stats.current_xp_per_hour(module_instance.upgrade_level);
    
    // Apply HP efficiency multiplier (damaged modules generate less XP)
    let hp_efficiency = module_instance.hp_efficiency_multiplier(attraction_stats.max_hp);
    let effective_xp_per_hour = (base_xp_per_hour as f64 * hp_efficiency) as u32;
    
    // Calculate total XP accumulated
    let accumulated_xp = (effective_xp_per_hour as f64 * hours_elapsed) as u32;
    
    msg!("💫 XP calculation:");
    msg!("   Base XP/hour (level {}): {}", module_instance.upgrade_level, base_xp_per_hour);
    msg!("   HP efficiency: {:.2}% ({} / {} HP)", hp_efficiency * 100.0, current_hp, attraction_stats.max_hp);
    msg!("   Effective XP/hour: {}", effective_xp_per_hour);
    msg!("   Accumulated XP: {}", accumulated_xp);
    
    // Return early if no XP to claim
    if accumulated_xp == 0 {
        msg!("⚠️ No XP accumulated yet");
        return Ok(());
    }
    
    // Update module's runtime state
    if let ModuleRuntimeState::Attraction { total_xp_generated, last_xp_claim, .. } = &mut module_instance.runtime_state {
        *total_xp_generated = total_xp_generated.saturating_add(accumulated_xp as u64);
        *last_xp_claim = current_time;
    }
    
    module_instance.last_updated = current_time;
    
    msg!("📊 Updated module state - new total XP generated: {}", 
         match &module_instance.runtime_state {
             ModuleRuntimeState::Attraction { total_xp_generated, .. } => *total_xp_generated,
             _ => 0,
         });
    
    // Process daily login and award the accumulated XP
    process_daily_login_and_xp(
        user_moonbase,
        accumulated_xp,
        &format!("Attraction Module ({:.1}h)", hours_elapsed),
    )?;
    
    // Emit event
    emit!(AttractionXPClaimed {
        owner: user.key(),
        module_index,
        xp_claimed: accumulated_xp,
        hours_elapsed: hours_elapsed,
        effective_xp_per_hour,
    });
    
    msg!("🎉 Attraction XP claim completed successfully!");
    msg!("   User: {}", user.key());
    msg!("   Module Index: {}", module_index);
    msg!("   XP Claimed: {} ({:.2} hours at {}/hour)", accumulated_xp, hours_elapsed, effective_xp_per_hour);
    
    Ok(())
}



/// Claim level-up rewards based on accumulated XP (with loot transfers)
/// This function processes any pending level-ups and awards loot rewards
pub fn claim_level_up_rewards_internal(ctx: Context<ClaimLevelUpRewards>) -> Result<()> {
    let user_moonbase = &mut ctx.accounts.user_moonbase;
    let old_level = user_moonbase.level;
    
    msg!("🎉 Processing level-ups for user {} (Level: {}, XP: {})", 
         ctx.accounts.user.key(), old_level, user_moonbase.xp);
    
    // Process any pending level-ups with loot system
    process_auto_daily_login_and_activity_xp(
        user_moonbase,
        0, // No new XP, just convert existing XP to levels
        "Level-Up Claim",
        &mut ctx.accounts.loot_rewards,
        &mut ctx.accounts.level_stats,
        &ctx.accounts.doge_btc_mining,
        &ctx.accounts.loot_sol_vault,
        &ctx.accounts.loot_dbtc_vault,
        &ctx.accounts.loot_dbtc_vault_authority,
        &ctx.accounts.user.to_account_info(),
        ctx.accounts.user_dbtc_token_account.as_ref().map(|acc| acc.to_account_info()).as_ref(),
        ctx.accounts.dbtc_token_mint.as_ref().map(|mint| mint.to_account_info()).as_ref(),
        ctx.accounts.token_program_2022.as_ref().map(|prog| prog.to_account_info()).as_ref(),
        &ctx.accounts.system_program,
    )?;
    
    let levels_gained = user_moonbase.level.saturating_sub(old_level);
    msg!("✅ Level-up complete: {} -> {} (+{} levels)", old_level, user_moonbase.level, levels_gained);
    
    Ok(())
}



// ----------------------------------------------------------------------------------------
// INTERNAL :: PROCESS DAILY LOGIN AND ACTIVITY XP ----------------
// ----------------------------------------------------------------------------------------

/// Process daily login and activity XP with full loot transfers
/// This is the central function for all user XP interactions
fn process_auto_daily_login_and_activity_xp<'info>(
    user_moonbase: &mut UserMoonBaseInstance,
    activity_xp: u32,
    activity_source: &str,
    loot_rewards: &mut LootRewards,
    level_stats: &mut LevelStats,
    doge_btc_mining: &DogeBtcMining,
    // Transfer-related accounts (required for loot transfers)
    loot_sol_vault: &AccountInfo<'info>,
    loot_dbtc_vault: &AccountInfo<'info>,
    loot_dbtc_vault_authority: &AccountInfo<'info>,
    user_account: &AccountInfo<'info>,
    user_token_account: Option<&AccountInfo<'info>>,
    token_mint: Option<&AccountInfo<'info>>,
    token_program: Option<&AccountInfo<'info>>,
    system_program: &AccountInfo<'info>,
) -> Result<()> {
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
        
        // Award all XP using enhanced system with transfers
        helper::add_xp_with_loot_transfers(
            user_moonbase,
            total_xp,
            &xp_source,
            loot_rewards,
            level_stats,
            doge_btc_mining,
            loot_sol_vault,
            loot_dbtc_vault,
            loot_dbtc_vault_authority,
            user_account,
            user_token_account,
            token_mint,
            token_program,
            system_program,
        )?;
        
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
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

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

    /// CHECK: Dragon Egg asset (optional) - will be created via CPI if tier includes egg
    #[account(mut)]
    pub dragon_egg_asset: Option<AccountInfo<'info>>,

    /// CHECK: Dragon Egg collection (optional)
    #[account(mut)]
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    /// Dragon Egg metadata (optional) - only created if tier includes egg
    #[account(
        init_if_needed,
        payer = user,
        space = DragonEggMetadata::LEN,
        seeds = [
            DRAGON_EGG_METADATA_SEED.as_ref(), 
            user.key().as_ref(),
            global_config.total_moonbases_created.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub dragon_egg_metadata: Option<Account<'info, DragonEggMetadata>>,

    /// CHECK: Metaplex Core program (optional)
    pub mpl_core_program: Option<UncheckedAccount<'info>>,

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
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub mining_state: Account<'info, DogeBtcMining>,
    
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



 

#[derive(Accounts)]
pub struct ClaimDogeBtc<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    #[account(mut)]
    /// CHECK: This is the token vault that holds all the DogeBtc tokens
    pub token_vault: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: This is the user's token account that will receive DogeBtc tokens
    pub user_token_account: UncheckedAccount<'info>,

    // Mint created under Token-2022
    #[account(mut, owner = token_program.key())]
    pub token_mint: InterfaceAccount<'info, Mint2022>,    

    /// Loot DOGE_BTC vault for distributing loot tokens (10% of mining rewards)
    #[account(
        mut,
        seeds = [LOOT_DOGE_BTC_VAULT_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Loot DOGE_BTC vault for distributing loot tokens
    pub loot_dbtc_vault: UncheckedAccount<'info>,

    /// Loot rewards tracking account (for mining loot accumulation only)
    #[account(
        mut,
        seeds = [LOOT_REWARDS_SEED.as_ref()],
        bump = loot_rewards.bump,
    )]
    pub loot_rewards: Account<'info, LootRewards>,

    // Optional Dragon Egg accounts (for power updates during claim)
    #[account(mut)]
    /// CHECK: Optional Dragon Egg NFT asset from Metaplex Core
    pub dragon_egg_asset: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    /// CHECK: Optional Dragon Egg metadata PDA
    pub dragon_egg_metadata: Option<Account<'info, DragonEggMetadata>>,

    #[account(mut)]
    /// CHECK: Optional incubation state PDA
    pub incubation_state: Option<Account<'info, IncubationState>>,

    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    
    /// SPL Token program
    /// CHECK: We know this is the correct address
    #[account(address = anchor_spl::token_2022::spl_token_2022::ID)]
    pub token_program: UncheckedAccount<'info>,
}



#[derive(Accounts)]
#[instruction(config_id: u16)]
pub struct BuyModule<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    // Access the specific module config directly via PDA
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    // Create the module instance with proper size calculation
    #[account(
        init,
        payer = user,
        space = ModuleInstance::LEN,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            user_moonbase.modules_count.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub module_instance: Account<'info, ModuleInstance>,

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
    
    #[account(
        constraint = referral_rewards.owner == user_moonbase.referral @ ErrorCode::InvalidReferralAccount,
    )]
    pub referral_rewards: Option<Account<'info, ReferralRewards>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(module_index: u8)]
pub struct InstallModule<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = module_index < user_moonbase.modules_count @ ErrorCode::ModuleInstanceNotFound
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            module_index.to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub module_instance: Account<'info, ModuleInstance>,
    
    // Access the specific module config directly via PDA using the config_id from module_instance
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_instance.config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
#[instruction(module_index: u8)]
pub struct RemoveModuleInstance<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = module_index < user_moonbase.modules_count @ ErrorCode::ModuleInstanceNotFound
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            module_index.to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub module_instance: Account<'info, ModuleInstance>,
    
    // Access the specific module config directly via PDA using the config_id from module_instance
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_instance.config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}




#[derive(Accounts)]
#[instruction(module_index: u8)]
pub struct DeleteModule<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = module_index < user_moonbase.modules_count @ ErrorCode::ModuleInstanceNotFound
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            module_index.to_le_bytes().as_ref()
        ],
        bump,
        close = user  // Close the module instance account and return lamports to user
    )]
    pub module_instance: Account<'info, ModuleInstance>,
    
    // Access the specific module config directly via PDA using the config_id from module_instance
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_instance.config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
#[instruction(module_index: u8)]
pub struct ClaimAttractionXP<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = module_index < user_moonbase.modules_count @ ErrorCode::ModuleInstanceNotFound
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            module_index.to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub module_instance: Account<'info, ModuleInstance>,
    
    // Access the specific module config directly via PDA using the config_id from module_instance
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_instance.config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


 

#[derive(Accounts)]
#[instruction(module_index: u8)]
pub struct UpdateModuleInstance<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized,
        constraint = module_index < user_moonbase.modules_count @ ErrorCode::ModuleInstanceNotFound
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,
    
    #[account(
        mut,
        seeds = [
            MODULE_INSTANCE_SEED.as_ref(), 
            user.key().as_ref(), 
            module_index.to_le_bytes().as_ref()
        ],
        bump,
    )]
    pub module_instance: Account<'info, ModuleInstance>,
    
    // Access the specific module config directly via PDA using the config_id from module_instance
    #[account(
        seeds = [MODULE_CONFIG_SEED.as_ref(), module_instance.config_id.to_le_bytes().as_ref()],
        bump = module_config_account.bump,
    )]
    pub module_config_account: Account<'info, ModuleConfigAccount>,
    
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_ref()],
        bump = global_config.bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump = doge_btc_mining.bump,
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,
    
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: This is the PDA that holds collected SOL fees
    pub sol_treasury: UncheckedAccount<'info>,
    
    #[account(
        constraint = referral_rewards.owner == user_moonbase.referral @ ErrorCode::InvalidReferralAccount,
    )]
    pub referral_rewards: Option<Account<'info, ReferralRewards>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}
 




#[derive(Accounts)]
pub struct ClaimLevelUpRewards<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,

    /// Loot rewards tracking account (required for loot transfers)
    #[account(
        mut,
        seeds = [LOOT_REWARDS_SEED.as_ref()],
        bump = loot_rewards.bump,
    )]
    pub loot_rewards: Account<'info, LootRewards>,

    /// Level statistics tracking account (required for level-up processing)
    #[account(
        mut,
        seeds = [LEVEL_STATS_SEED.as_ref()],
        bump = level_stats.bump,
    )]
    pub level_stats: Account<'info, LevelStats>,
    
    #[account(
        seeds = [DOGE_BTC_MINING_SEED.as_ref()],
        bump
    )]
    pub doge_btc_mining: Account<'info, DogeBtcMining>,

    /// Loot SOL vault for distributing SOL loot
    #[account(
        mut,
        seeds = [LOOT_SOL_VAULT_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Loot SOL vault PDA
    pub loot_sol_vault: UncheckedAccount<'info>,

    /// Loot DOGE_BTC vault for distributing DOGE_BTC loot
    #[account(
        mut,
        seeds = [LOOT_DOGE_BTC_VAULT_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Loot DOGE_BTC vault PDA
    pub loot_dbtc_vault: UncheckedAccount<'info>,

    /// Loot DOGE_BTC vault authority for signing transfers
    #[account(
        seeds = [LOOT_DOGE_BTC_VAULT_AUTHORITY_SEED.as_ref()],
        bump,
    )]
    /// CHECK: Loot DOGE_BTC vault authority PDA
    pub loot_dbtc_vault_authority: UncheckedAccount<'info>,

    /// User's DOGE_BTC token account for receiving loot tokens (optional)
    #[account(mut)]
    /// CHECK: User's DOGE_BTC token account
    pub user_dbtc_token_account: Option<UncheckedAccount<'info>>,

    /// DOGE_BTC token mint (Token-2022) (optional)
    pub dbtc_token_mint: Option<InterfaceAccount<'info, anchor_spl::token_interface::Mint>>,

    /// SPL Token-2022 program (optional)
    pub token_program_2022: Option<Program<'info, anchor_spl::token_2022::Token2022>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}


// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG NFT MANAGEMENT -----------------------------------------------
// ----------------------------------------------------------------------------------------

/// Incubate a Dragon Egg in the moonbase (max 1 per moonbase)
pub fn incubate_dragon_egg_internal(
    ctx: Context<IncubateDragonEgg>,
) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let incubation_state = &mut ctx.accounts.incubation_state;
    let user_moonbase = &mut ctx.accounts.user_moonbase;

    // Verify ownership from Metaplex Core asset
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(nft_owner == ctx.accounts.user.key(), ErrorCode::NftNotOwnedByUser);

    // Validation
    require!(
        egg_metadata.incubated_moonbase.is_none(),
        ErrorCode::EggAlreadyIncubated
    );
    require!(
        user_moonbase.incubated_dragon_egg.is_none(),
        ErrorCode::MaxEggsReached
    );

    let current_time = Clock::get()?.unix_timestamp;

    msg!("🔒 Transferring NFT to custody PDA (locking)");
    
    // Transfer NFT from user to custody PDA (this locks the NFT)
    crate::mpl_core_helpers::transfer_mpl_core_asset(
        &ctx.accounts.dragon_egg_asset.to_account_info(),
        ctx.accounts.dragon_egg_collection.as_ref().map(|c| c.to_account_info()).as_ref(),
        &ctx.accounts.user.to_account_info(),
        &ctx.accounts.user.to_account_info(), // User is current owner/authority
        &ctx.accounts.egg_custody_pda.to_account_info(), // Transfer to custody PDA
        &ctx.accounts.mpl_core_program.to_account_info(),
    )?;

    msg!("✅ NFT locked in custody PDA: {}", ctx.accounts.egg_custody_pda.key());

    // Update moonbase state
    user_moonbase.incubated_dragon_egg = Some(egg_metadata.key());

    // Add egg to incubation state
    incubation_state.incubated_egg = Some(egg_metadata.mint);
    incubation_state.last_update_ts = current_time;
    incubation_state.moonbase_owner = user_moonbase.owner;

    // Update egg metadata
    egg_metadata.incubated_moonbase = Some(user_moonbase.owner);
    egg_metadata.last_update_ts = current_time;

    msg!("✅ Dragon Egg incubated in moonbase");
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Moonbase: {}", user_moonbase.owner);

    Ok(())
}

/// Remove Dragon Egg from moonbase incubation
pub fn remove_dragon_egg_internal(
    ctx: Context<RemoveDragonEgg>,
) -> Result<()> {
    let egg_metadata = &mut ctx.accounts.dragon_egg_metadata;
    let incubation_state = &mut ctx.accounts.incubation_state;
    let user_moonbase = &mut ctx.accounts.user_moonbase;

    // Verify NFT is in custody PDA (it should be locked there)
    let nft_owner = crate::mpl_core_helpers::get_mpl_core_owner(&ctx.accounts.dragon_egg_asset)?;
    require!(nft_owner == ctx.accounts.egg_custody_pda.key(), ErrorCode::EggNotIncubated);

    require!(
        egg_metadata.incubated_moonbase.is_some(),
        ErrorCode::EggNotIncubated
    );

    let current_time = Clock::get()?.unix_timestamp;
    let final_power = egg_metadata.power;

    msg!("🔓 Transferring NFT back to user (unlocking)");
    
    // Get PDA signer seeds for custody PDA
    let custody_seeds = &[
        DRAGON_EGG_CUSTODY_SEED,
        &[ctx.bumps.egg_custody_pda],
    ];
    let signer_seeds = &[&custody_seeds[..]];

    // Transfer NFT back from custody PDA to user (unlock)
    let mut cpi_builder = mpl_core::instructions::TransferV1CpiBuilder::new(&ctx.accounts.mpl_core_program);
    cpi_builder
        .asset(&ctx.accounts.dragon_egg_asset)
        .payer(&ctx.accounts.user)
        .authority(Some(&ctx.accounts.egg_custody_pda)) // Custody PDA is authority
        .new_owner(&ctx.accounts.user);
    
    if let Some(collection) = &ctx.accounts.dragon_egg_collection {
        cpi_builder.collection(Some(collection));
    }
    
    cpi_builder.invoke_signed(signer_seeds)?;

    msg!("✅ NFT unlocked and returned to user: {}", ctx.accounts.user.key());

    // Update moonbase state
    user_moonbase.incubated_dragon_egg = None;

    // Remove egg from incubation state
    incubation_state.incubated_egg = None;
    incubation_state.last_update_ts = current_time;

    // Update egg metadata
    egg_metadata.incubated_moonbase = None;
    egg_metadata.last_update_ts = current_time;

    msg!("✅ Dragon Egg removed from incubation");
    msg!("   Egg: {}", egg_metadata.mint);
    msg!("   Final Power: {}", final_power);

    Ok(())
}
 

// ----------------------------------------------------------------------------------------
// -------------- DRAGON EGG ACCOUNT CONTEXTS ---------------------------------------------
// ----------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct IncubateDragonEgg<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,

    /// Metaplex Core asset (source of truth for ownership)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    #[account(
        init_if_needed,
        payer = user,
        space = IncubationState::LEN,
        seeds = [INCUBATION_STATE_SEED.as_ref(), user.key().as_ref()],
        bump
    )]
    pub incubation_state: Account<'info, IncubationState>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveDragonEgg<'info> {
    #[account(
        mut,
        seeds = [USER_MOONBASE_SEED.as_ref(), user.key().as_ref()],
        bump = user_moonbase.bump,
        constraint = user_moonbase.owner == user.key() @ ErrorCode::Unauthorized
    )]
    pub user_moonbase: Account<'info, UserMoonBaseInstance>,

    /// Metaplex Core asset (currently locked in custody PDA)
    #[account(mut)]
    /// CHECK: Verified via get_mpl_core_owner helper
    pub dragon_egg_asset: UncheckedAccount<'info>,

    /// Optional collection account for the Dragon Egg
    /// CHECK: Optional collection
    pub dragon_egg_collection: Option<UncheckedAccount<'info>>,

    #[account(
        mut,
        seeds = [DRAGON_EGG_METADATA_SEED.as_ref(), dragon_egg_metadata.mint.as_ref()],
        bump = dragon_egg_metadata.bump,
        constraint = dragon_egg_metadata.mint == dragon_egg_asset.key() @ ErrorCode::InvalidAccount
    )]
    pub dragon_egg_metadata: Account<'info, DragonEggMetadata>,

    #[account(
        mut,
        seeds = [INCUBATION_STATE_SEED.as_ref(), user.key().as_ref()],
        bump = incubation_state.bump,
    )]
    pub incubation_state: Account<'info, IncubationState>,

    /// PDA that holds custody of locked NFTs
    #[account(
        seeds = [DRAGON_EGG_CUSTODY_SEED],
        bump
    )]
    /// CHECK: PDA for NFT custody
    pub egg_custody_pda: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program
    pub mpl_core_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}
