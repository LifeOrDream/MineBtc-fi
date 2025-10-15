use anchor_lang::prelude::*;
use anchor_lang::system_program::System;

use crate::state::*;
use crate::errors::ErrorCode;
use crate::events::*;

use anchor_spl::token::{Mint, Token, TokenAccount};

use anchor_spl::token_interface::{
    Mint as Mint2022,
    TokenAccount as TokenAccount2022
};
use anchor_spl::token_2022::Token2022;



// ----------------------------------------------------------------------------------------
// ------------ INITIALIZATION & UPDATES :: GLOBAL_CONFIG and VAULTS ------------
// ----------------------------------------------------------------------------------------

/// Initialize the global program configuration
/// This function can only be called once as it creates the program's configuration accounts
pub fn initialize_global_config(
    ctx: Context<InitializeGlobalConfig>,
    dev_address: Pubkey,
    moondoge_allocation: u8,
    liquidity_allocation: u8,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
    ) -> Result<()> {
    // Validate multipliers
    require!(min_lockup_days > 0, ErrorCode::InvalidLockupPeriod);
    require!(max_lockup_days > min_lockup_days, ErrorCode::InvalidLockupPeriod);
    require!(base_multiplier > 0, ErrorCode::InvalidMultiplier);
    require!(max_multiplier > base_multiplier, ErrorCode::InvalidMultiplier);

    let global_config = &mut ctx.accounts.global_config;
    
    // Address which can claim the dev earnings
    global_config.dev_address = dev_address;
    
    // Initialize GlobalConfig
    global_config.authority = ctx.accounts.authority.key();
    global_config.fee_collector = ctx.accounts.fee_collector.key();
    
    global_config.moondoge_allocation = moondoge_allocation;
    global_config.liquidity_allocation = liquidity_allocation;

    global_config.min_lockup_days = min_lockup_days;
    global_config.max_lockup_days = max_lockup_days;

    global_config.base_multiplier = base_multiplier;
    global_config.max_multiplier = max_multiplier;
    
    global_config.last_claim_slot = Clock::get()?.slot;    
    global_config.bump = ctx.bumps.global_config;

    emit!(ProgramInitialized {
        dev_address: dev_address,
        dev_earnings_collector: ctx.accounts.dev_earnings_collector.key(),
        fee_collector: ctx.accounts.fee_collector.key(),
        moondoge_allocation,
        liquidity_allocation
    });
    Ok(())
}

/// Initialize the global program configuration
/// This function can only be called once as it creates the program's configuration accounts
pub fn initialize_dbtc_vault(
    ctx: Context<InitializeMdogeVault>,
    dbtc_mint: Pubkey,
    electricity_per_weighted_moondoge: u64,
) -> Result<()> {
    let moondoge_vault = &mut ctx.accounts.moondoge_vault;

    // Initialize DogeBtc Vault
    moondoge_vault.authority = ctx.accounts.authority.key();
    moondoge_vault.dbtc_mint = dbtc_mint;
    moondoge_vault.dbtc_sol_vault = ctx.accounts.dbtc_sol_vault.key();    
    moondoge_vault.dbtc_custodian = ctx.accounts.dbtc_custodian.key();

    moondoge_vault.electricity_per_weighted_moondoge = electricity_per_weighted_moondoge;
    moondoge_vault.dbtc_locked = 0;
    moondoge_vault.weighted_dbtc_locked = 0;
    moondoge_vault.accumulated_sol_per_point = 0;
    moondoge_vault.total_sol_distributed = 0;
    moondoge_vault.emergency_tax = EMERGENCY_WITHDRAWAL_PENALTY_PCT; // Default 10% tax for emergency withdrawals
    moondoge_vault.bump = ctx.bumps.moondoge_vault;

    emit!(MDogeVaultsInitialized {
        dbtc_sol_vault: ctx.accounts.dbtc_sol_vault.key(),
        dbtc_mint: dbtc_mint,
        dbtc_custodian: ctx.accounts.dbtc_custodian.key(),
    });
    
    Ok(())
}

/// Initialize the global program configuration
/// This function can only be called once as it creates the program's configuration accounts
pub fn initialize_liquidity_vault(
    ctx: Context<InitializeLiquidityVault>,
    lp_token_mint: Pubkey,
    electricity_per_weighted_lp_tokens: u64,
) -> Result<()> {
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // Initialize Liquidity Vault
    liquidity_vault.authority = ctx.accounts.authority.key();
    liquidity_vault.lp_token_mint = lp_token_mint;
    liquidity_vault.liquidity_sol_vault = ctx.accounts.liquidity_sol_vault.key();
    liquidity_vault.liquidity_custodian = ctx.accounts.liquidity_custodian.key();

    liquidity_vault.electricity_per_weighted_lp_tokens = electricity_per_weighted_lp_tokens;
    liquidity_vault.lp_tokens_locked = 0;
    liquidity_vault.weighted_lp_locked = 0;
    liquidity_vault.accumulated_sol_per_point = 0;
    liquidity_vault.total_sol_distributed = 0;
    liquidity_vault.emergency_tax = EMERGENCY_WITHDRAWAL_PENALTY_PCT; // Default 10% tax for emergency withdrawals

    liquidity_vault.bump = ctx.bumps.liquidity_vault;
    emit!(LiquidityVaultsInitialized {
        liquidity_sol_vault: ctx.accounts.liquidity_sol_vault.key(),
        lp_token_mint: lp_token_mint,
        liquidity_custodian: ctx.accounts.liquidity_custodian.key(),
    });
    
    Ok(())
}



/// Update the global configuration parameters
/// Can only be called by the current authority
pub fn internal_update_configuration(
    ctx: Context<UpdateConfig>,
    new_authority: Option<Pubkey>,
    new_dev_address: Option<Pubkey>,
    new_moondoge_allocation: Option<u8>,
    new_liquidity_allocation: Option<u8>, 
    new_electricity_per_weighted_moondoge: Option<u64>,
    new_electricity_per_weighted_lp_tokens: Option<u64>,
    new_emergency_tax: Option<u8>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let moondoge_vault = &mut ctx.accounts.moondoge_vault;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;
    
    // Update authority if provided
    if let Some(new_authority) = new_authority {
        global_config.authority = new_authority;
        moondoge_vault.authority = new_authority;
        liquidity_vault.authority = new_authority;
        msg!("Updated authority to {}", new_authority);
    }

    // if new fee_collector is provided, update it
    if let Some(new_dev_address) = new_dev_address {
        global_config.dev_address = new_dev_address;
        msg!("Updated dev address to {}", new_dev_address);
    }

    // if moondoge_allocation is provided, update it
    if let Some(moondoge_allocation) = new_moondoge_allocation {
        global_config.moondoge_allocation = moondoge_allocation;
        msg!("Updated moondoge allocation to {}", moondoge_allocation);
    }
    if let Some(liquidity_allocation) = new_liquidity_allocation {
        global_config.liquidity_allocation = liquidity_allocation;
        msg!("Updated liquidity allocation to {}", liquidity_allocation);
    }

    // if electricity_per_weighted_moondoge is provided, update it
    if let Some(electricity_per_weighted_moondoge) = new_electricity_per_weighted_moondoge {
        moondoge_vault.electricity_per_weighted_moondoge = electricity_per_weighted_moondoge;
        msg!("Updated electricity per weighted DogeBtc to {}", electricity_per_weighted_moondoge);
    }

    // if electricity_per_weighted_lp_tokens is provided, update it
    if let Some(electricity_per_weighted_lp_tokens) = new_electricity_per_weighted_lp_tokens {
        liquidity_vault.electricity_per_weighted_lp_tokens = electricity_per_weighted_lp_tokens;
        msg!("Updated electricity per weighted LP tokens to {}", electricity_per_weighted_lp_tokens);
    }
    
    // if emergency_tax is provided, update it (ensuring it's not more than 100%)
    if let Some(emergency_tax) = new_emergency_tax {
        require!(emergency_tax <= (M_HUNDRED as u8), ErrorCode::InvalidAmount);
        moondoge_vault.emergency_tax = emergency_tax;
        liquidity_vault.emergency_tax = emergency_tax;
        msg!("Updated emergency withdrawal tax to {}%", emergency_tax);
    }
         
    // Emit update event
    emit!(ConfigUpdated {
        authority: global_config.authority,
        electricity_per_weighted_moondoge: moondoge_vault.electricity_per_weighted_moondoge,
        electricity_per_weighted_lp_tokens: liquidity_vault.electricity_per_weighted_lp_tokens,
        moondoge_allocation: global_config.moondoge_allocation,
        liquidity_allocation: global_config.liquidity_allocation
    });
    
    Ok(())
}

// ----------------------------------------------------------------------------------------
// ------------ DISTRIBUTE SOL FEES FROM MOONFACILITY TO RESPECTIVE VAULTS ------------
// ----------------------------------------------------------------------------------------

/// Distribute SOL fromMoonBaseto respective vaults
pub fn internal_claim_moonbase_sol(ctx: Context<ClaimMoonBaseSOL>) -> Result<()> {
 

    // CPI: Call MoonBase.withdraw_sol_fees, fee_collector must sign
    msg!("🚀 Performing CPI call to MoonBase::withdraw_sol_fees...");
    let cpi_program = ctx.accounts.moonbase_program.to_account_info();
    let cpi_accounts = moon_base::cpi::accounts::WithdrawSolFees {
        global_config: ctx.accounts.moonbase_global_config.to_account_info(),
        doge_btc_mining: ctx.accounts.moonbase_mining_state.to_account_info(),
        sol_treasury: ctx.accounts.moonbase_treasury.to_account_info(),
        fee_collector: ctx.accounts.fee_collector.to_account_info(),
        loot_sol_vault: ctx.accounts.loot_sol_vault.to_account_info(),
        loot_rewards: ctx.accounts.loot_rewards.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };

    // Derive PDA seeds for fee_collector signer
    let fee_collector_seeds = &[FEE_COLLECTOR_SEED.as_ref(), &[ctx.bumps.fee_collector]];
    let signer_seeds = &[&fee_collector_seeds[..]];    
    
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
    
    // Perform CPI call
    moon_base::cpi::withdraw_sol_fees(cpi_ctx)?;
    msg!("✅ CPI withdrawal successful, proceeding with vault distributions");

    // Now get the ACTUAL amount that the fee collector received
    // This is what we need to distribute, not the original treasury balance
    let fee_collector = &ctx.accounts.fee_collector;
    let fee_collector_rent = Rent::get()?.minimum_balance(fee_collector.data_len());
    let available_for_distribution = fee_collector.lamports().saturating_sub(fee_collector_rent);
    msg!("🔍 Available for distribution from fee collector: {}", available_for_distribution);

    // Get allocation percentages from global config
    let global_config = &ctx.accounts.global_config;
    
    // Calculate amounts for each vault based on what we actually have
    let mut moondoge_amount = (available_for_distribution as u128).checked_mul(global_config.moondoge_allocation as u128)
                                                        .unwrap().checked_div(M_HUNDRED as u128).unwrap() as u64;
        
    let mut liquidity_amount = (available_for_distribution as u128).checked_mul(global_config.liquidity_allocation as u128)
                                                        .unwrap().checked_div(M_HUNDRED as u128).unwrap() as u64;
        
    msg!("📊 Allocations - DogeBtc: {}, Liquidity: {}", moondoge_amount, liquidity_amount);

    // Now distribute to respective vaults
    let moondoge_vault = &mut ctx.accounts.moondoge_vault;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    if moondoge_vault.weighted_dbtc_locked > 0 {
        let sol_per_point = (moondoge_amount as u128 * PRECISION_FACTOR as u128)
            / moondoge_vault.weighted_dbtc_locked as u128;
        moondoge_vault.accumulated_sol_per_point += sol_per_point;
        **fee_collector.try_borrow_mut_lamports()? -= moondoge_amount;
        **ctx.accounts.dbtc_sol_vault.try_borrow_mut_lamports()? += moondoge_amount;
        msg!("💰 Sent {} to DogeBtc vault", moondoge_amount);
    } else {
        moondoge_amount = 0;
    }

    if liquidity_vault.weighted_lp_locked > 0 {
        let sol_per_point = (liquidity_amount as u128 * PRECISION_FACTOR as u128)
            / liquidity_vault.weighted_lp_locked as u128;
        liquidity_vault.accumulated_sol_per_point += sol_per_point;
        **fee_collector.try_borrow_mut_lamports()? -= liquidity_amount;
        **ctx.accounts.liquidity_sol_vault.try_borrow_mut_lamports()? += liquidity_amount;
        msg!("💰 Sent {} to Liquidity vault", liquidity_amount);
    } else {
        liquidity_amount = 0;
    }

    let dev_earnings = available_for_distribution.saturating_sub(moondoge_amount + liquidity_amount);
    if dev_earnings > 0 {
        **fee_collector.try_borrow_mut_lamports()? -= dev_earnings;
        **ctx.accounts.dev_earnings_collector.try_borrow_mut_lamports()? += dev_earnings;
        msg!("👨‍💻 Sent {} to Dev Earnings", dev_earnings);
    }

    emit!(SolDistributed {
        total_amount: available_for_distribution,
        moondoge_amount,
        liquidity_amount,
        dev_earnings,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
 

// ----------------------------------------------------------------------------------------
// ------------ WITHDRAW COLLECTED SOL FEES (admin can only call this) ------------
// ----------------------------------------------------------------------------------------

/// Withdraw SOL fees allocated for the game development
/// Can only be called by the authority
pub fn withdraw_dev_earnings(ctx: Context<WithdrawDevEarnings>) -> Result<()> {
    let dev_earnings_collector = &ctx.accounts.dev_earnings_collector;
    let dev_address = &ctx.accounts.global_config.dev_address;

    // check if the fee collector is the same as the authority which is the only one allowed to call this
    require!(dev_address.key() == ctx.accounts.authority.key(), ErrorCode::Unauthorized);
    
    // Get the current balance of dev_earnings_collector, leaving rent exempt amount
    let dev_earnings_collector_balance = dev_earnings_collector.lamports();

    // Calculate rent based on actual account size, not assuming 0 bytes
    let rent = Rent::get()?.minimum_balance(dev_earnings_collector.data_len());
    let withdraw_amount = dev_earnings_collector_balance.saturating_sub(rent);

    // Transfer SOL from dev_earnings_collector to dev_address
    let receiver = &ctx.accounts.authority;
    **dev_earnings_collector.try_borrow_mut_lamports()? = rent;
    **receiver.try_borrow_mut_lamports()? += withdraw_amount;

    emit!(AdminEarningsWithdrawn { amount: withdraw_amount });

    msg!("Withdrew {} lamports from admin fee collector", withdraw_amount);
    Ok(())
}

 



// ----------------------------------------------------------------------------------------
// ------------ ACCOUNT STRUCTS ----------------------------------------------------------
// ----------------------------------------------------------------------------------------

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --- INITIALIZE GLOBAL CONFIG ------ 
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

/// Initialize the global config and other program state
#[derive(Accounts)]
#[instruction(
    dev_address: Pubkey,
    moondoge_allocation: u8,
    liquidity_allocation: u8,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16
)]
pub struct InitializeGlobalConfig<'info> {
    // account to hold the global config state
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    /// CHECK: This is a PDA owned by the program that will collect admin fees which the devs can withdraw
    #[account(
        init,
        payer = authority,
        space = 8,
        seeds = [DEV_EARNINGS_SEED.as_ref()],
        bump
    )]
    pub dev_earnings_collector: UncheckedAccount<'info>,

    /// CHECK: This is a PDA owned by the program that will collect total SOL fees
    #[account(
        init,
        payer = authority,
        space = 8,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump
    )]
    pub fee_collector: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,    
    
    pub system_program: Program<'info, System>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --- INITIALIZE VAULTS ------------ 
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

/// Initialize the DogeBtc vault
#[derive(Accounts)]
pub struct InitializeMdogeVault<'info> {

    // ------ Data Storage Accounts ------

    #[account(
        init,
        payer = authority,
        space = DogeBtcVault::LEN,
        seeds = [doge_btc_VAULT_SEED.as_ref()],
        bump
    )]
    pub moondoge_vault: Account<'info, DogeBtcVault>,

    // ------ SOL Storage Accounts ------

    /// CHECK: This is a 0-byte PDA owned by the program that will store SOL to be distributed to the DogeBtc vault
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [dbtc_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub dbtc_sol_vault: UncheckedAccount<'info>,

    // ------ DOGE_BTC Token Storage Account and Signer-Only PDA ------

    /// CHECK: This is the authority of the custodian of the moonDoge tokens pda
    #[account(
        init,
        payer = authority,
        space = 0,                              // no data needed
        seeds = [dbtc_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required    
    pub dbtc_custodian_authority: UncheckedAccount<'info>,

    /// Token-2022 account to hold DOGE_BTC
    #[account(
        init,
        payer = authority,
        owner = token_program.key(),
        seeds = [dbtc_CUSTODIAN_SEED.as_ref(), moondoge_vault.key().as_ref()],
        token::mint = dbtc_mint,
        token::authority = dbtc_custodian_authority,
        bump
    )]
    pub dbtc_custodian: InterfaceAccount<'info, TokenAccount2022>,
   
    // -------- Signer & Mints --------
    #[account(mut)]
    pub authority: Signer<'info>,

    /// DOGE_BTC mint (SPL-2022)
    pub dbtc_mint: InterfaceAccount<'info, Mint2022>,
    
    // -------- Programs & Sysvars --------
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>, // SPL-2022 program
    pub rent: Sysvar<'info, Rent>,
}

/// Initialize the Liquidity vault
#[derive(Accounts)]
pub struct InitializeLiquidityVault<'info> {

    // ------ Data Storage Accounts ------

    #[account(
        init,
        payer = authority,
        space = LiquidityVault::LEN,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,

    // ------ SOL Storage Accounts ------

    /// CHECK: This is a 0-byte PDA owned by the program that will store SOL to be distributed to the Liquidity vault
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [LP_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    pub liquidity_sol_vault: UncheckedAccount<'info>,

    // ------ LP Token Storage Account and Signer-Only PDA ------

    /// CHECK: This is the authority of the custodian of the LP tokens pda
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required    
    pub liquidity_custodian_authority: UncheckedAccount<'info>,

    /// SPL Token account to hold LP tokens
    #[account(
        init,
        payer = authority,
        owner = token_program.key(),
        token::mint = lp_token_mint,
        token::authority = liquidity_custodian_authority,
        seeds = [LIQUIDITY_CUSTODIAN_SEED.as_ref(), liquidity_vault.key().as_ref()],
        bump
    )]
    pub liquidity_custodian: Account<'info, TokenAccount>,
        
    // -------- Signer & Mints --------
    #[account(mut)]
    pub authority: Signer<'info>,

    /// LP token mint (standard SPL)
    pub lp_token_mint: Account<'info, Mint>,
    
    // -------- Programs & Sysvars --------
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,      // SPL token program
    pub rent: Sysvar<'info, Rent>,
}


// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --- UPDATE CONFIG ---------------- 
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

/// Update the global config parameters
#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump,
        constraint = global_config.authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [doge_btc_VAULT_SEED.as_ref()],
        bump,
    )]
    pub moondoge_vault: Account<'info, DogeBtcVault>,

    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump,
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,

    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --- WITHDRAW ADMIN EARNINGS -------- 
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
 
/// Account struct for withdrawing game SOL fees
#[derive(Accounts)]
pub struct WithdrawDevEarnings<'info> {
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump,
        constraint = global_config.dev_address == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DEV_EARNINGS_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that holds SOL which the devs can withdraw
    pub dev_earnings_collector: UncheckedAccount<'info>,
 
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
// --- CLAIM MOONBASE SOL ------------ 
// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
 
/// Account struct for distributing SOL from MoonFacility
#[derive(Accounts)]
pub struct ClaimMoonBaseSOL<'info> {
    #[account(
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [doge_btc_VAULT_SEED.as_ref()],
        bump 
    )]
    pub moondoge_vault: Account<'info, DogeBtcVault>,

    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,

    #[account(
        mut,
        seeds = [dbtc_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that custodies SOL for DogeBtc stakers
    pub dbtc_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LP_SOL_VAULT_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that custodies SOL for LP stakers
    pub liquidity_sol_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [DEV_EARNINGS_SEED.as_ref()],
        bump
    )]
    /// CHECK: This is the PDA that will store SOL for game development
    pub dev_earnings_collector: UncheckedAccount<'info>,

    /// CHECK: MoonBase's global config
    pub moonbase_global_config: UncheckedAccount<'info>,

    /// CHECK: MoonBase's mining state
    pub moonbase_mining_state: UncheckedAccount<'info>,

    /// CHECK: MoonBase's treasury
    #[account(mut)]
    pub moonbase_treasury: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
    )]
    /// CHECK: This is our program's PDA that is authorized inMoonBaseto withdraw
    pub fee_collector: UncheckedAccount<'info>,

    /// CHECK: MoonBase's loot SOL vault
    #[account(mut)]
    pub loot_sol_vault: UncheckedAccount<'info>,

    /// CHECK: MoonBase's loot rewards tracking account
    #[account(mut)]
    pub loot_rewards: UncheckedAccount<'info>,

    /// CHECK: TheMoonBaseprogram
    pub moonbase_program: Program<'info, moon_base::program::MoonBase>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
} 
 