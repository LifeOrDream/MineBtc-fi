use anchor_lang::prelude::*;
use anchor_lang::system_program::System;

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;

use anchor_spl::token::{Mint, Token, TokenAccount};

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{Mint as Mint2022, TokenAccount as TokenAccount2022};

// ----------------------------------------------------------------------------------------
// ------------ INITIALIZATION & UPDATES :: GLOBAL_CONFIG and VAULTS ------------
// ----------------------------------------------------------------------------------------

/// Initialize the global program configuration
/// This function can only be called once as it creates the program's configuration accounts
pub fn initialize_global_config(
    ctx: Context<InitializeGlobalConfig>,
    dev_address: Pubkey,
    dogebtc_allocation: u8,
    liquidity_allocation: u8,
    min_lockup_days: u64,
    max_lockup_days: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<()> {
    // Validate multipliers
    require!(min_lockup_days > 0, ErrorCode::InvalidLockupPeriod);
    require!(
        max_lockup_days > min_lockup_days,
        ErrorCode::InvalidLockupPeriod
    );
    require!(base_multiplier > 0, ErrorCode::InvalidMultiplier);
    require!(
        max_multiplier > base_multiplier,
        ErrorCode::InvalidMultiplier
    );

    let global_config = &mut ctx.accounts.global_config;

    // Address which can claim the dev earnings
    global_config.dev_address = dev_address;

    // Initialize GlobalConfig
    global_config.authority = ctx.accounts.authority.key();
    global_config.fee_collector = ctx.accounts.fee_collector.key();

    global_config.dogebtc_allocation = dogebtc_allocation;
    global_config.liquidity_allocation = liquidity_allocation;

    global_config.min_lockup_days = min_lockup_days;
    global_config.max_lockup_days = max_lockup_days;

    global_config.base_multiplier = base_multiplier;
    global_config.max_multiplier = max_multiplier;

    global_config.last_claim_slot = Clock::get()?.slot;

    // Initialize SOL distribution as disabled to prevent early stakers from capturing all initial SOL
    global_config.sol_distribution_enabled = false;

    // Initialize price to 0 (must be set by admin via set_dbtc_sol_price function)
    global_config.dbtc_sol_price = 0;

    global_config.bump = ctx.bumps.global_config;

    emit!(ProgramInitialized {
        dev_address: dev_address,
        dev_earnings_collector: ctx.accounts.dev_earnings_collector.key(),
        fee_collector: ctx.accounts.fee_collector.key(),
        dogebtc_allocation,
        liquidity_allocation
    });
    Ok(())
}

/// Initialize the global program configuration
/// This function can only be called once as it creates the program's configuration accounts
pub fn initialize_dbtc_vault(ctx: Context<InitializeDbtcVault>, dbtc_mint: Pubkey) -> Result<()> {
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;

    // Initialize DogeBtc Vault
    dogebtc_vault.authority = ctx.accounts.authority.key();
    dogebtc_vault.dbtc_mint = dbtc_mint;
    dogebtc_vault.dbtc_sol_vault = ctx.accounts.dbtc_sol_vault.key();
    dogebtc_vault.dbtc_custodian = ctx.accounts.dbtc_custodian.key();

    dogebtc_vault.dbtc_locked = 0;
    dogebtc_vault.weighted_dbtc_locked = 0;
    dogebtc_vault.accumulated_sol_per_point = 0;
    dogebtc_vault.total_sol_distributed = 0;
    dogebtc_vault.emergency_tax = EMERGENCY_WITHDRAWAL_PENALTY_PCT; // Default 10% tax for emergency withdrawals
    dogebtc_vault.bump = ctx.bumps.dogebtc_vault;

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
) -> Result<()> {
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // Initialize Liquidity Vault
    liquidity_vault.authority = ctx.accounts.authority.key();
    liquidity_vault.lp_token_mint = lp_token_mint;
    liquidity_vault.liquidity_sol_vault = ctx.accounts.liquidity_sol_vault.key();
    liquidity_vault.liquidity_custodian = ctx.accounts.liquidity_custodian.key();

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
    new_dogebtc_allocation: Option<u8>,
    new_liquidity_allocation: Option<u8>,
    new_electricity_per_weighted_sol: Option<u64>,
    new_emergency_tax: Option<u8>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // Update authority if provided
    if let Some(new_authority) = new_authority {
        global_config.authority = new_authority;
        dogebtc_vault.authority = new_authority;
        liquidity_vault.authority = new_authority;
        msg!("Updated authority to {}", new_authority);
    }

    // if new fee_collector is provided, update it
    if let Some(new_dev_address) = new_dev_address {
        global_config.dev_address = new_dev_address;
        msg!("Updated dev address to {}", new_dev_address);
    }

    // if dogebtc_allocation is provided, update it
    if let Some(dogebtc_allocation) = new_dogebtc_allocation {
        global_config.dogebtc_allocation = dogebtc_allocation;
        msg!("Updated moondoge allocation to {}", dogebtc_allocation);
    }
    if let Some(liquidity_allocation) = new_liquidity_allocation {
        global_config.liquidity_allocation = liquidity_allocation;
        msg!("Updated liquidity allocation to {}", liquidity_allocation);
    }

    // if electricity_per_weighted_sol is provided, update it
    if let Some(electricity_per_weighted_sol) = new_electricity_per_weighted_sol {
        global_config.electricity_per_weighted_sol = electricity_per_weighted_sol;
        msg!(
            "Updated electricity per weighted SOL to {}",
            electricity_per_weighted_sol
        );
    }

    // if emergency_tax is provided, update it (ensuring it's not more than 100%)
    if let Some(emergency_tax) = new_emergency_tax {
        require!(emergency_tax <= (M_HUNDRED as u8), ErrorCode::InvalidAmount);
        dogebtc_vault.emergency_tax = emergency_tax;
        liquidity_vault.emergency_tax = emergency_tax;
        msg!("Updated emergency withdrawal tax to {}%", emergency_tax);
    }

    // Emit update event
    emit!(ConfigUpdated {
        authority: global_config.authority,
        electricity_per_weighted_sol: global_config.electricity_per_weighted_sol,
        dogebtc_allocation: global_config.dogebtc_allocation,
        liquidity_allocation: global_config.liquidity_allocation
    });

    Ok(())
}

/// Set DOGE_BTC to SOL price (admin only)
/// Price is stored with 9-decimal precision (same as MoonBase)
/// Example: 0.001 SOL per DOGE_BTC = 1_000_000 (9-decimal precision)
pub fn set_dbtc_sol_price_internal(ctx: Context<UpdateConfig>, price: u64) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    require!(price > 0, ErrorCode::InvalidAmount);

    global_config.dbtc_sol_price = price;

    msg!(
        "✅ Set DOGE_BTC to SOL price: {} (9-decimal precision)",
        price
    );
    msg!(
        "   Actual price: {:.9} SOL per DOGE_BTC",
        price as f64 / 1_000_000_000.0
    );

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
    let cpi_accounts = moonbase::cpi::accounts::WithdrawSolFees {
        global_config: ctx.accounts.moonbase_global_config.to_account_info(),
        doge_btc_mining: ctx.accounts.moonbase_mining_state.to_account_info(),
        sol_treasury: ctx.accounts.moonbase_treasury.to_account_info(),
        fee_collector: ctx.accounts.fee_collector.to_account_info(),
        loot_sol_vault: ctx.accounts.loot_sol_vault.to_account_info(),
        loot_rewards: ctx.accounts.loot_rewards.to_account_info(),
        buybacks_sol_vault: ctx.accounts.buybacks_sol_vault.to_account_info(),
        buybacks_account: ctx.accounts.buybacks_account.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };

    // Derive PDA seeds for fee_collector signer
    let fee_collector_seeds = &[FEE_COLLECTOR_SEED.as_ref(), &[ctx.bumps.fee_collector]];
    let signer_seeds = &[&fee_collector_seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

    // Perform CPI call
    moonbase::cpi::withdraw_sol_fees(cpi_ctx)?;
    msg!("✅ CPI withdrawal successful, proceeding with vault distributions");

    // Now get the ACTUAL amount that the fee collector received
    // This is what we need to distribute, not the original treasury balance
    let fee_collector = &ctx.accounts.fee_collector;
    let fee_collector_rent = Rent::get()?.minimum_balance(fee_collector.data_len());
    let sol_for_distribution = fee_collector.lamports().saturating_sub(fee_collector_rent);
    msg!(
        "🔍 SOL for distribution from fee collector: {}",
        sol_for_distribution
    );

    // Get allocation percentages from global config
    let global_config = &ctx.accounts.global_config;
    // Check if SOL distribution is enabled
    require!(
        global_config.sol_distribution_enabled,
        ErrorCode::SolDistributionDisabled
    );

    // Calculate amounts for each vault based on what we actually have
    let mut sol_for_dbtc_stakers = (sol_for_distribution as u128)
        .checked_mul(global_config.dogebtc_allocation as u128)
        .unwrap()
        .checked_div(M_HUNDRED as u128)
        .unwrap() as u64;

    let mut sol_for_lp_stakers = (sol_for_distribution as u128)
        .checked_mul(global_config.liquidity_allocation as u128)
        .unwrap()
        .checked_div(M_HUNDRED as u128)
        .unwrap() as u64;

    msg!(
        "📊 Allocations - DogeBtc: {}, Liquidity: {}",
        sol_for_dbtc_stakers,
        sol_for_lp_stakers
    );

    // Now distribute to respective vaults
    let dogebtc_vault = &mut ctx.accounts.dogebtc_vault;
    let liquidity_vault = &mut ctx.accounts.liquidity_vault;

    // Create signer seeds for fee_collector
    let fee_collector_seeds = &[FEE_COLLECTOR_SEED.as_ref(), &[ctx.bumps.fee_collector]];
    let signer_seeds = &[&fee_collector_seeds[..]];

    if dogebtc_vault.weighted_dbtc_locked > 0 && sol_for_dbtc_stakers > 0 {
        let sol_per_point = (sol_for_dbtc_stakers as u128 * PRECISION_FACTOR as u128)
            / dogebtc_vault.weighted_dbtc_locked as u128;
        dogebtc_vault.accumulated_sol_per_point += sol_per_point;

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: fee_collector.to_account_info(),
                    to: ctx.accounts.dbtc_sol_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_dbtc_stakers,
        )?;
        msg!(
            "💰 Sent {} SOL to DogeBtc vault",
            sol_for_dbtc_stakers as f64 / 1e9
        );
    } else {
        sol_for_dbtc_stakers = 0;
    }

    if liquidity_vault.weighted_lp_locked > 0 && sol_for_lp_stakers > 0 {
        let sol_per_point = (sol_for_lp_stakers as u128 * PRECISION_FACTOR as u128)
            / liquidity_vault.weighted_lp_locked as u128;
        liquidity_vault.accumulated_sol_per_point += sol_per_point;

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: fee_collector.to_account_info(),
                    to: ctx.accounts.liquidity_sol_vault.to_account_info(),
                },
                signer_seeds,
            ),
            sol_for_lp_stakers,
        )?;
        msg!(
            "💰 Sent {} SOL to Liquidity vault",
            sol_for_lp_stakers as f64 / 1e9
        );
    } else {
        sol_for_lp_stakers = 0;
    }

    let dev_earnings =
        sol_for_distribution.saturating_sub(sol_for_dbtc_stakers + sol_for_lp_stakers);
    if dev_earnings > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: fee_collector.to_account_info(),
                    to: ctx.accounts.dev_earnings_collector.to_account_info(),
                },
                signer_seeds,
            ),
            dev_earnings,
        )?;
        msg!("👨‍💻 Sent {} SOL to Dev Earnings", dev_earnings as f64 / 1e9);
    }

    emit!(SolDistributed {
        sol_for_distribution: sol_for_distribution,
        sol_for_dbtc_stakers,
        sol_for_lp_stakers,
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
    require!(
        dev_address.key() == ctx.accounts.authority.key(),
        ErrorCode::Unauthorized
    );

    // Get the current balance of dev_earnings_collector, leaving rent exempt amount
    let dev_earnings_collector_balance = dev_earnings_collector.lamports();

    // Calculate rent based on actual account size, not assuming 0 bytes
    let rent = Rent::get()?.minimum_balance(dev_earnings_collector.data_len());
    let withdraw_amount = dev_earnings_collector_balance.saturating_sub(rent);

    // Transfer SOL from dev_earnings_collector to dev_address using system transfer
    if withdraw_amount > 0 {
        let dev_vault_seeds = &[
            DEV_EARNINGS_SEED.as_ref(),
            &[ctx.bumps.dev_earnings_collector],
        ];
        let signer_seeds = &[&dev_vault_seeds[..]];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: dev_earnings_collector.to_account_info(),
                    to: ctx.accounts.authority.to_account_info(),
                },
                signer_seeds,
            ),
            withdraw_amount,
        )?;

        msg!(
            "👨‍💻 Withdrew {} SOL to dev address",
            withdraw_amount as f64 / 1e9
        );
    }

    emit!(AdminEarningsWithdrawn {
        amount: withdraw_amount
    });

    msg!(
        "Withdrew {} lamports from admin fee collector",
        withdraw_amount
    );
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
    dogebtc_allocation: u8,
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

    /// CHECK: This is a System Account PDA that will collect admin fees which the devs can withdraw
    #[account(
        init,
        payer = authority,
        space = 0,  // 0-byte for System Account holding only lamports
        seeds = [DEV_EARNINGS_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub dev_earnings_collector: UncheckedAccount<'info>,

    /// CHECK: This is a System Account PDA that will collect total SOL fees
    #[account(
        init,
        payer = authority,
        space = 0,  // 0-byte for System Account holding only lamports
        seeds = [FEE_COLLECTOR_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
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
pub struct InitializeDbtcVault<'info> {
    // ------ Data Storage Accounts ------
    #[account(
        init,
        payer = authority,
        space = DogeBtcVault::LEN,
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,

    // ------ SOL Storage Accounts ------
    /// CHECK: This is a 0-byte System Account PDA that will store SOL to be distributed to the DogeBtc vault
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [DBTC_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
    )]
    pub dbtc_sol_vault: UncheckedAccount<'info>,

    // ------ DOGE_BTC Token Storage Account and Signer-Only PDA ------
    /// CHECK: This is the authority of the custodian of the moonDoge tokens pda
    #[account(
        init,
        payer = authority,
        space = 0,                              // no data needed
        seeds = [DBTC_CUSTODIAN_AUTHORITY_SEED.as_ref()],
        bump
    )]
    /// CHECK: signer-only PDA, no data or lamports required    
    pub dbtc_custodian_authority: UncheckedAccount<'info>,

    /// Token-2022 account to hold DOGE_BTC
    #[account(
        init,
        payer = authority,
        owner = token_program.key(),
        seeds = [DBTC_CUSTODIAN_SEED.as_ref(), dogebtc_vault.key().as_ref()],
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
    /// CHECK: This is a 0-byte System Account PDA that will store SOL to be distributed to the Liquidity vault
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [LP_SOL_VAULT_SEED.as_ref()],
        bump,
        owner = system_program.key()  // System-owned account for native SOL
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
    pub token_program: Program<'info, Token>, // SPL token program
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
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump,
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,

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
        seeds = [DOGE_BTC_VAULT_SEED.as_ref()],
        bump 
    )]
    pub dogebtc_vault: Account<'info, DogeBtcVault>,

    #[account(
        mut,
        seeds = [LIQUIDITY_VAULT_SEED.as_ref()],
        bump
    )]
    pub liquidity_vault: Account<'info, LiquidityVault>,

    #[account(
        mut,
        seeds = [DBTC_SOL_VAULT_SEED.as_ref()],
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

    /// CHECK: MoonBase's buybacks SOL vault
    #[account(mut)]
    pub buybacks_sol_vault: UncheckedAccount<'info>,

    /// CHECK: MoonBase's buybacks tracking account
    #[account(mut)]
    pub buybacks_account: UncheckedAccount<'info>,

    /// CHECK: TheMoonBaseprogram
    pub moonbase_program: Program<'info, moonbase::program::Moonbase>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ========================================================================================
// ============================= ENABLE/DISABLE SOL DISTRIBUTION ==========================
// ========================================================================================

/// Enable or disable SOL distribution from fee collector
/// This prevents early stakers from capturing all initial SOL during launch period
pub fn set_sol_distribution_enabled(
    ctx: Context<SetSolDistributionEnabled>,
    enabled: bool,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    let old_value = global_config.sol_distribution_enabled;
    global_config.sol_distribution_enabled = enabled;

    msg!(
        "🔄 SOL distribution status changed: {} -> {}",
        old_value,
        enabled
    );

    if enabled {
        msg!("✅ SOL distribution ENABLED - stakers can now claim SOL rewards");
    } else {
        msg!("⏸️  SOL distribution DISABLED - SOL will accumulate but not be claimable");
    }

    Ok(())
}

#[derive(Accounts)]
pub struct SetSolDistributionEnabled<'info> {
    #[account(
        mut,
        seeds = [ME_CONFIG_SEED.as_ref()],
        bump = global_config.bump,
        constraint = global_config.authority == authority.key() @ ErrorCode::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}
