use anchor_lang::prelude::*;
use anchor_spl::{
    token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked, burn_checked, TransferChecked, BurnChecked},
};

use crate::{
    constants::*,
    errors::DragonHiveError,
    events::*,
    state::*,
    utils::*,
};

// ========================================================================================
// =============================== INITIALIZE HONEY CONFIG =============================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(
    honey_token_mint: Pubkey,
    honey_distribution_admin: Pubkey,
    game_recipient_address: Pubkey,
    amm_recipient_address: Pubkey,
    dev_recipient_address: Pubkey,
    staking_rewards_claim_account: Pubkey,
)]
pub struct InitializeHoneyConfig<'info> {
    #[account(
        init,
        payer = main_admin,
        space = GlobalHoneyConfig::LEN,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    /// HONEY token vault for storing 100B tokens
    #[account(
        init,
        payer = main_admin,
        seeds = [HONEY_VAULT_SEED],
        bump,
        token::mint = honey_token_mint,
        token::authority = honey_vault_authority,
        token::token_program = token_program
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// HONEY vault authority PDA
    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// HONEY burn account
    #[account(
        init,
        payer = main_admin,
        seeds = [HONEY_BURN_ACCOUNT_SEED],
        bump,
        token::mint = honey_token_mint,
        token::authority = honey_burn_authority,
        token::token_program = token_program
    )]
    pub honey_burn_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY burn account authority PDA
    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_BURN_AUTHORITY_SEED],
        bump
    )]
    pub honey_burn_authority: UncheckedAccount<'info>,

    /// Staking rewards account
    #[account(
        init,
        payer = main_admin,
        seeds = [STAKING_REWARDS_ACCOUNT_SEED],
        bump,
        token::mint = honey_token_mint,
        token::authority = staking_rewards_authority,
        token::token_program = token_program
    )]
    pub staking_rewards_account: InterfaceAccount<'info, TokenAccount>,

    /// Staking rewards account authority PDA
    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [STAKING_REWARDS_AUTHORITY_SEED],
        bump
    )]
    pub staking_rewards_authority: UncheckedAccount<'info>,

    /// HONEY token mint
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub main_admin: Signer<'info>,

    /// External authority (can be same as main_admin initially)
    /// CHECK: Will be validated by the main_admin
    pub ext_authority: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_honey_config_handler(
    ctx: Context<InitializeHoneyConfig>,
    honey_token_mint: Pubkey,
    honey_distribution_admin: Pubkey,
    game_recipient_address: Pubkey,
    amm_recipient_address: Pubkey,
    dev_recipient_address: Pubkey,
    staking_rewards_claim_account: Pubkey,
    initial_distribution_rate: u64,
    for_game_percentage: u16,
    dev_split_percentage: u16,
    min_distribution_interval: i64,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;
    
    // Validate inputs
    require!(for_game_percentage <= 10000, DragonHiveError::InvalidParameters);
    require!(dev_split_percentage <= 10000, DragonHiveError::InvalidParameters);
    require!(for_game_percentage + dev_split_percentage <= 10000, DragonHiveError::InvalidParameters);
    require!(initial_distribution_rate > 0, DragonHiveError::InvalidParameters);
    require!(min_distribution_interval > 0, DragonHiveError::InvalidParameters);

    let current_time = get_current_timestamp()?;

    // Initialize global HONEY configuration
    global_honey_config.main_admin = ctx.accounts.main_admin.key();
    global_honey_config.ext_authority = ctx.accounts.ext_authority.key();
    global_honey_config.honey_token_mint = honey_token_mint;
    global_honey_config.honey_vault = ctx.accounts.honey_vault.key();
    global_honey_config.honey_vault_authority = ctx.accounts.honey_vault_authority.key();
    global_honey_config.burn_account = ctx.accounts.honey_burn_account.key();
    global_honey_config.burn_account_authority = ctx.accounts.honey_burn_authority.key();
    global_honey_config.staking_rewards_account = ctx.accounts.staking_rewards_account.key();
    global_honey_config.staking_rewards_account_authority = ctx.accounts.staking_rewards_authority.key();
    global_honey_config.staking_rewards_claim_account = staking_rewards_claim_account;

    // Initialize distribution configuration
    global_honey_config.distribution_config = HoneyDistributionConfig {
        honey_distribution_admin,
        cur_distribution_rate: initial_distribution_rate,
        game_recipient_address,
        amm_recipient_address,
        dev_recipient_address,
        for_game_percentage,
        dev_split_percentage,
        min_distribution_interval,
        claimable_game_amount: 0,
        claimable_dev_amount: 0,
        claimable_amm_amount: 0,
    };

    // Initialize counters
    global_honey_config.total_honey_distributed = 0;
    global_honey_config.total_honey_burned = 0;
    global_honey_config.last_distribution_time = current_time;
    global_honey_config.is_paused = false;

    // Set bump seeds
    global_honey_config.config_bump = ctx.bumps.global_honey_config;
    global_honey_config.vault_bump = ctx.bumps.honey_vault;
    global_honey_config.vault_authority_bump = ctx.bumps.honey_vault_authority;
    global_honey_config.burn_account_bump = ctx.bumps.honey_burn_account;
    global_honey_config.burn_authority_bump = ctx.bumps.honey_burn_authority;
    global_honey_config.staking_rewards_bump = ctx.bumps.staking_rewards_account;
    global_honey_config.staking_rewards_authority_bump = ctx.bumps.staking_rewards_authority;

    emit!(HoneyConfigInitialized {
        main_admin: ctx.accounts.main_admin.key(),
        ext_authority: ctx.accounts.ext_authority.key(),
        honey_token_mint,
        honey_vault: ctx.accounts.honey_vault.key(),
        burn_account: ctx.accounts.honey_burn_account.key(),
        staking_rewards_account: ctx.accounts.staking_rewards_account.key(),
        initial_distribution_rate,
        for_game_percentage,
    });

    Ok(())
}

// ========================================================================================
// =============================== UPDATE HONEY CONFIG =================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct UpdateHoneyConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = global_honey_config.main_admin == main_admin.key() @ DragonHiveError::Unauthorized
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    pub main_admin: Signer<'info>,
}

pub fn update_honey_config_handler(
    ctx: Context<UpdateHoneyConfig>,
    new_main_admin: Option<Pubkey>,
    new_ext_authority: Option<Pubkey>,
    new_distribution_admin: Option<Pubkey>,
    new_game_recipient: Option<Pubkey>,
    new_amm_recipient: Option<Pubkey>,
    new_dev_recipient: Option<Pubkey>,
    new_for_game_percentage: Option<u16>,
    new_dev_split_percentage: Option<u16>,
    new_min_distribution_interval: Option<i64>,
    is_paused: Option<bool>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;

    if let Some(new_main_admin) = new_main_admin {
        global_honey_config.main_admin = new_main_admin;
    }

    if let Some(new_ext_authority) = new_ext_authority {
        global_honey_config.ext_authority = new_ext_authority;
    }

    if let Some(new_distribution_admin) = new_distribution_admin {
        global_honey_config.distribution_config.honey_distribution_admin = new_distribution_admin;
    }

    if let Some(new_game_recipient) = new_game_recipient {
        global_honey_config.distribution_config.game_recipient_address = new_game_recipient;
    }

    if let Some(new_amm_recipient) = new_amm_recipient {
        global_honey_config.distribution_config.amm_recipient_address = new_amm_recipient;
    }

    if let Some(new_dev_recipient) = new_dev_recipient {
        global_honey_config.distribution_config.dev_recipient_address = new_dev_recipient;
    }

    if let Some(new_for_game_percentage) = new_for_game_percentage {
        require!(new_for_game_percentage <= 10000, DragonHiveError::InvalidParameters);
        // Validate that game + dev percentages don't exceed 100%
        let dev_percentage = global_honey_config.distribution_config.dev_split_percentage;
        require!(new_for_game_percentage + dev_percentage <= 10000, DragonHiveError::InvalidParameters);
        global_honey_config.distribution_config.for_game_percentage = new_for_game_percentage;
    }

    if let Some(new_dev_split_percentage) = new_dev_split_percentage {
        require!(new_dev_split_percentage <= 10000, DragonHiveError::InvalidParameters);
        // Validate that game + dev percentages don't exceed 100%
        let game_percentage = global_honey_config.distribution_config.for_game_percentage;
        require!(game_percentage + new_dev_split_percentage <= 10000, DragonHiveError::InvalidParameters);
        global_honey_config.distribution_config.dev_split_percentage = new_dev_split_percentage;
    }

    if let Some(new_min_distribution_interval) = new_min_distribution_interval {
        require!(new_min_distribution_interval > 0, DragonHiveError::InvalidParameters);
        global_honey_config.distribution_config.min_distribution_interval = new_min_distribution_interval;
    }

    if let Some(is_paused) = is_paused {
        global_honey_config.is_paused = is_paused;
    }

    emit!(HoneyConfigUpdated {
        main_admin: ctx.accounts.main_admin.key(),
        new_main_admin,
        new_ext_authority,
        is_paused,
    });

    Ok(())
}

// ========================================================================================
// =============================== UPDATE DISTRIBUTION CONFIG ============================ 
// ========================================================================================

#[derive(Accounts)]
pub struct UpdateDistributionConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = global_honey_config.distribution_config.honey_distribution_admin == distribution_admin.key() @ DragonHiveError::Unauthorized
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    pub distribution_admin: Signer<'info>,
}

pub fn update_distribution_rate_handler(
    ctx: Context<UpdateDistributionConfig>,
    new_distribution_rate: Option<u64>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;

    if let Some(new_distribution_rate) = new_distribution_rate {
        require!(new_distribution_rate > 0, DragonHiveError::InvalidParameters);
        global_honey_config.distribution_config.cur_distribution_rate = new_distribution_rate;
    }

    emit!(DistributionConfigUpdated {
        distribution_admin: ctx.accounts.distribution_admin.key(),
        new_distribution_admin: None,
        new_distribution_rate,
        new_game_recipient: None,
        new_amm_recipient: None,
        new_for_game_percentage: None,
    });

    Ok(())
}

// ========================================================================================
// =============================== DEPOSIT HONEY TOKENS ================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct DepositHoneyTokens<'info> {
    #[account(
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_honey_config.vault_bump,
        constraint = honey_vault.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = depositor_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = depositor_token_account.owner == depositor.key() @ DragonHiveError::Unauthorized
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn deposit_honey_tokens_handler(
    ctx: Context<DepositHoneyTokens>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, DragonHiveError::InvalidPaymentAmount);

    // Transfer HONEY tokens to vault
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                to: ctx.accounts.honey_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
        ),
        amount,
        HONEY_DECIMALS,
    )?;

    let vault_balance = ctx.accounts.honey_vault.amount;

    emit!(HoneyTokensDeposited {
        depositor: ctx.accounts.depositor.key(),
        amount,
        total_vault_balance: vault_balance,
    });

    Ok(())
}

// ========================================================================================
// =============================== BURN HONEY TOKENS ===================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct AddToBurnAccount<'info> {
    #[account(
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_BURN_ACCOUNT_SEED],
        bump = global_honey_config.burn_account_bump,
        constraint = burn_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub burn_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = user_token_account.owner == user.key() @ DragonHiveError::Unauthorized
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn add_to_burn_account_handler(
    ctx: Context<AddToBurnAccount>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, DragonHiveError::InvalidPaymentAmount);

    // Transfer HONEY tokens to burn account
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.burn_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
        ),
        amount,
        HONEY_DECIMALS,
    )?;

    emit!(HoneyTokensAddedToBurn {
        user: ctx.accounts.user.key(),
        amount,
        total_burn_balance: ctx.accounts.burn_account.amount,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct BurnHoneyTokens<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_BURN_ACCOUNT_SEED],
        bump = global_honey_config.burn_account_bump,
        constraint = burn_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub burn_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_BURN_AUTHORITY_SEED],
        bump = global_honey_config.burn_authority_bump
    )]
    pub burn_authority: UncheckedAccount<'info>,

    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// Anyone can call this function to burn tokens
    pub caller: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn burn_honey_tokens_handler(
    ctx: Context<BurnHoneyTokens>,
    amount: Option<u64>, // If None, burn all tokens in the account
) -> Result<()> {
    let burn_amount = amount.unwrap_or(ctx.accounts.burn_account.amount);
    
    require!(burn_amount > 0, DragonHiveError::InvalidPaymentAmount);
    require!(
        burn_amount <= ctx.accounts.burn_account.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    let global_honey_config = &mut ctx.accounts.global_honey_config;

    // Get PDA signer seeds for the burn authority
    let authority_seeds = &[
        HONEY_BURN_AUTHORITY_SEED,
        &[global_honey_config.burn_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Burn HONEY tokens
    burn_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            BurnChecked {
                mint: ctx.accounts.honey_token_mint.to_account_info(),
                from: ctx.accounts.burn_account.to_account_info(),
                authority: ctx.accounts.burn_authority.to_account_info(),
            },
            signer,
        ),
        burn_amount,
        HONEY_DECIMALS,
    )?;

    // Update global stats
    global_honey_config.total_honey_burned = global_honey_config.total_honey_burned
        .checked_add(burn_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    emit!(HoneyTokensBurned {
        caller: ctx.accounts.caller.key(),
        amount: burn_amount,
        total_burned: global_honey_config.total_honey_burned,
        remaining_burn_balance: ctx.accounts.burn_account.amount.saturating_sub(burn_amount),
    });

    Ok(())
}

// ========================================================================================
// =============================== STAKING REWARDS ==================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct AddToStakingRewards<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [STAKING_REWARDS_ACCOUNT_SEED],
        bump = global_honey_config.staking_rewards_bump,
        constraint = staking_rewards_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub staking_rewards_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = user_token_account.owner == user.key() @ DragonHiveError::Unauthorized
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn add_to_staking_rewards_handler(
    ctx: Context<AddToStakingRewards>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, DragonHiveError::InvalidPaymentAmount);

    let global_honey_config = &mut ctx.accounts.global_honey_config;

    // Transfer HONEY tokens to staking rewards account
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.staking_rewards_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
        ),
        amount,
        HONEY_DECIMALS,
    )?;

    emit!(HoneyTokensAddedToStaking {
        user: ctx.accounts.user.key(),
        amount
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimStakingRewards<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused,
        constraint = global_honey_config.staking_rewards_claim_account == claimer.key() @ DragonHiveError::Unauthorized
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [STAKING_REWARDS_ACCOUNT_SEED],
        bump = global_honey_config.staking_rewards_bump,
        constraint = staking_rewards_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub staking_rewards_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [STAKING_REWARDS_AUTHORITY_SEED],
        bump = global_honey_config.staking_rewards_authority_bump
    )]
    pub staking_rewards_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = claimer_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = claimer_token_account.owner == claimer.key() @ DragonHiveError::Unauthorized
    )]
    pub claimer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub claimer: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_staking_rewards_handler(
    ctx: Context<ClaimStakingRewards>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, DragonHiveError::InvalidPaymentAmount);
    require!(
        amount <= ctx.accounts.staking_rewards_account.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    let global_honey_config = &mut ctx.accounts.global_honey_config;

    // Get PDA signer seeds for the staking rewards authority
    let authority_seeds = &[
        STAKING_REWARDS_AUTHORITY_SEED,
        &[global_honey_config.staking_rewards_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Transfer HONEY tokens to claimer
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.staking_rewards_account.to_account_info(),
                to: ctx.accounts.claimer_token_account.to_account_info(),
                authority: ctx.accounts.staking_rewards_authority.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
            signer,
        ),
        amount,
        HONEY_DECIMALS,
    )?;

    emit!(StakingRewardsClaimed {
        claimer: ctx.accounts.claimer.key(),
        amount,
        remaining_rewards: ctx.accounts.staking_rewards_account.amount.saturating_sub(amount),
    });

    Ok(())
}

// ========================================================================================
// =============================== DISTRIBUTE HONEY TOKENS =============================== 
// ========================================================================================

#[derive(Accounts)]
pub struct DistributeHoneyTokens<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_honey_config.vault_bump,
        constraint = honey_vault.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump = global_honey_config.vault_authority_bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// Dev recipient token account
    #[account(
        mut,
        constraint = dev_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = dev_token_account.owner == global_honey_config.distribution_config.dev_recipient_address @ DragonHiveError::Unauthorized
    )]
    pub dev_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// Anyone can call this function
    pub caller: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn distribute_honey_tokens_handler(
    ctx: Context<DistributeHoneyTokens>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;
    let current_time = get_current_timestamp()?;

    // Check if enough time has passed since last distribution
    let time_since_last = current_time - global_honey_config.last_distribution_time;
    require!(
        time_since_last >= global_honey_config.distribution_config.min_distribution_interval,
        DragonHiveError::OperationTooEarly
    );

    // Get distribution amount based on time elapsed
    // cur_distribution_rate is tokens per second, multiply by elapsed seconds
    let distribution_amount = global_honey_config.distribution_config.cur_distribution_rate
        .checked_mul(time_since_last as u64)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    require!(distribution_amount > 0, DragonHiveError::InvalidParameters);
    require!(
        distribution_amount <= ctx.accounts.honey_vault.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    // Calculate splits
    let game_percentage = global_honey_config.distribution_config.for_game_percentage;
    let dev_percentage = global_honey_config.distribution_config.dev_split_percentage;
    let _amm_percentage = 10000 - game_percentage - dev_percentage; // Remaining goes to AMM

    let game_amount = (distribution_amount * game_percentage as u64) / 10000;
    let dev_amount = (distribution_amount * dev_percentage as u64) / 10000;
    let amm_amount = distribution_amount - game_amount - dev_amount; // Ensure exact total

    // Get PDA signer seeds for the vault authority
    let authority_seeds = &[
        HONEY_VAULT_AUTHORITY_SEED,
        &[global_honey_config.vault_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Accumulate all amounts for later claiming (no immediate transfers)
    if game_amount > 0 {
        global_honey_config.distribution_config.claimable_game_amount = global_honey_config
            .distribution_config
            .claimable_game_amount
            .checked_add(game_amount)
            .ok_or(DragonHiveError::ArithmeticOverflow)?;
        
        msg!("🎮 Game amount accumulated: {} (Total claimable: {})", 
             game_amount, 
             global_honey_config.distribution_config.claimable_game_amount);
    }

    if dev_amount > 0 {
        global_honey_config.distribution_config.claimable_dev_amount = global_honey_config
            .distribution_config
            .claimable_dev_amount
            .checked_add(dev_amount)
            .ok_or(DragonHiveError::ArithmeticOverflow)?;
        
        msg!("👨‍💻 Dev amount accumulated: {} (Total claimable: {})", 
             dev_amount, 
             global_honey_config.distribution_config.claimable_dev_amount);

    }

    if amm_amount > 0 {
        global_honey_config.distribution_config.claimable_amm_amount = global_honey_config
            .distribution_config
            .claimable_amm_amount
            .checked_add(amm_amount)
            .ok_or(DragonHiveError::ArithmeticOverflow)?;
        
        msg!("🏦 AMM amount accumulated: {} (Total claimable: {})", 
             amm_amount, 
             global_honey_config.distribution_config.claimable_amm_amount);
    }

    // Update global stats
    global_honey_config.total_honey_distributed = global_honey_config.total_honey_distributed
        .checked_add(distribution_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    
    global_honey_config.last_distribution_time = current_time;

    emit!(HoneyTokensDistributed {
        caller: ctx.accounts.caller.key(),
        total_distributed: distribution_amount,
        game_amount,
        dev_amount,
        amm_amount,
        game_recipient: global_honey_config.distribution_config.game_recipient_address,
        dev_recipient: global_honey_config.distribution_config.dev_recipient_address,
        amm_recipient: global_honey_config.distribution_config.amm_recipient_address,
        remaining_vault_balance: ctx.accounts.honey_vault.amount, // No tokens transferred yet, just accumulated
    });

    Ok(())
}


// ========================================================================================
// =============================== CLAIM DEV TOKENS ====================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct ClaimDevTokens<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_honey_config.vault_bump,
        constraint = honey_vault.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump = global_honey_config.vault_authority_bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// Dev recipient token account
    #[account(
        mut,
        constraint = dev_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = dev_token_account.owner == dev_recipient.key() @ DragonHiveError::Unauthorized
    )]
    pub dev_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// Dev recipient (must be the configured dev address)
    #[account(
        constraint = dev_recipient.key() == global_honey_config.distribution_config.dev_recipient_address @ DragonHiveError::Unauthorized
    )]
    pub dev_recipient: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_dev_tokens_handler(
    ctx: Context<ClaimDevTokens>,
    amount: Option<u64>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;
    
    // Get amount to claim (all available if not specified)
    let claimable_amount = global_honey_config.distribution_config.claimable_dev_amount;
    let claim_amount = amount.unwrap_or(claimable_amount);
    
    // Validate claim amount
    require!(claim_amount > 0, DragonHiveError::InvalidParameters);
    require!(claim_amount <= claimable_amount, DragonHiveError::InsufficientHoneyTokens);
    require!(
        claim_amount <= ctx.accounts.honey_vault.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    msg!("🎯 Dev claiming HONEY tokens");
    msg!("   Claimable amount: {}", claimable_amount);
    msg!("   Claiming amount: {}", claim_amount);
    msg!("   Dev recipient: {}", ctx.accounts.dev_recipient.key());

    // Get PDA signer seeds for the vault authority
    let authority_seeds = &[
        HONEY_VAULT_AUTHORITY_SEED,
        &[global_honey_config.vault_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Transfer HONEY tokens to dev
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.honey_vault.to_account_info(),
                to: ctx.accounts.dev_token_account.to_account_info(),
                authority: ctx.accounts.honey_vault_authority.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
            signer,
        ),
        claim_amount,
        HONEY_DECIMALS,
    )?;

    // Update claimable amount
    global_honey_config.distribution_config.claimable_dev_amount = global_honey_config
        .distribution_config
        .claimable_dev_amount
        .checked_sub(claim_amount)
        .ok_or(DragonHiveError::ArithmeticUnderflow)?;

    msg!("✅ Dev tokens claimed successfully");
    msg!("   Amount claimed: {}", claim_amount);
    msg!("   Remaining claimable: {}", global_honey_config.distribution_config.claimable_dev_amount);

    emit!(DevTokensClaimed {
        dev_recipient: ctx.accounts.dev_recipient.key(),
        amount_claimed: claim_amount,
        remaining_claimable: global_honey_config.distribution_config.claimable_dev_amount,
    });

    Ok(())
}

// ========================================================================================
// =============================== CLAIM GAME TOKENS ===================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct ClaimGameTokens<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_honey_config.vault_bump,
        constraint = honey_vault.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump = global_honey_config.vault_authority_bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// Game recipient token account
    #[account(
        mut,
        constraint = game_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = game_token_account.owner == game_recipient.key() @ DragonHiveError::Unauthorized
    )]
    pub game_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// Game recipient (must be the configured game address)
    #[account(
        constraint = game_recipient.key() == global_honey_config.distribution_config.game_recipient_address @ DragonHiveError::Unauthorized
    )]
    pub game_recipient: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_game_tokens_handler(
    ctx: Context<ClaimGameTokens>,
    amount: Option<u64>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;
    
    // Get amount to claim (all available if not specified)
    let claimable_amount = global_honey_config.distribution_config.claimable_game_amount;
    let claim_amount = amount.unwrap_or(claimable_amount);
    
    // Validate claim amount
    require!(claim_amount > 0, DragonHiveError::InvalidParameters);
    require!(claim_amount <= claimable_amount, DragonHiveError::InsufficientHoneyTokens);
    require!(
        claim_amount <= ctx.accounts.honey_vault.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    msg!("🎮 Game claiming HONEY tokens");
    msg!("   Claimable amount: {}", claimable_amount);
    msg!("   Claiming amount: {}", claim_amount);

    // Get PDA signer seeds for the vault authority
    let authority_seeds = &[
        HONEY_VAULT_AUTHORITY_SEED,
        &[global_honey_config.vault_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Transfer HONEY tokens to game
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.honey_vault.to_account_info(),
                to: ctx.accounts.game_token_account.to_account_info(),
                authority: ctx.accounts.honey_vault_authority.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
            signer,
        ),
        claim_amount,
        HONEY_DECIMALS,
    )?;

    // Update claimable amount
    global_honey_config.distribution_config.claimable_game_amount = global_honey_config
        .distribution_config
        .claimable_game_amount
        .checked_sub(claim_amount)
        .ok_or(DragonHiveError::ArithmeticUnderflow)?;

    msg!("✅ Game tokens claimed successfully");
    msg!("   Amount claimed: {}", claim_amount);
    msg!("   Remaining claimable: {}", global_honey_config.distribution_config.claimable_game_amount);

    emit!(GameTokensClaimed {
        game_recipient: ctx.accounts.game_recipient.key(),
        amount_claimed: claim_amount,
        remaining_claimable: global_honey_config.distribution_config.claimable_game_amount,
    });

    Ok(())
}

// ========================================================================================
// =============================== CLAIM AMM TOKENS ====================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct ClaimAmmTokens<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_HONEY_CONFIG_SEED],
        bump = global_honey_config.config_bump,
        constraint = !global_honey_config.is_paused @ DragonHiveError::ProgramPaused
    )]
    pub global_honey_config: Account<'info, GlobalHoneyConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_honey_config.vault_bump,
        constraint = honey_vault.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump = global_honey_config.vault_authority_bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// AMM recipient token account
    #[account(
        mut,
        constraint = amm_token_account.mint == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = amm_token_account.owner == amm_recipient.key() @ DragonHiveError::Unauthorized
    )]
    pub amm_token_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_honey_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// AMM recipient (must be the configured AMM address)
    #[account(
        constraint = amm_recipient.key() == global_honey_config.distribution_config.amm_recipient_address @ DragonHiveError::Unauthorized
    )]
    pub amm_recipient: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_amm_tokens_handler(
    ctx: Context<ClaimAmmTokens>,
    amount: Option<u64>,
) -> Result<()> {
    let global_honey_config = &mut ctx.accounts.global_honey_config;
    
    // Get amount to claim (all available if not specified)
    let claimable_amount = global_honey_config.distribution_config.claimable_amm_amount;
    let claim_amount = amount.unwrap_or(claimable_amount);
    
    // Validate claim amount
    require!(claim_amount > 0, DragonHiveError::InvalidParameters);
    require!(claim_amount <= claimable_amount, DragonHiveError::InsufficientHoneyTokens);
    require!(
        claim_amount <= ctx.accounts.honey_vault.amount,
        DragonHiveError::InsufficientHoneyTokens
    );

    msg!("🏦 AMM claiming HONEY tokens");
    msg!("   Claimable amount: {}", claimable_amount);
    msg!("   Claiming amount: {}", claim_amount);

    // Get PDA signer seeds for the vault authority
    let authority_seeds = &[
        HONEY_VAULT_AUTHORITY_SEED,
        &[global_honey_config.vault_authority_bump]
    ];
    let signer = &[&authority_seeds[..]];

    // Transfer HONEY tokens to AMM
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.honey_vault.to_account_info(),
                to: ctx.accounts.amm_token_account.to_account_info(),
                authority: ctx.accounts.honey_vault_authority.to_account_info(),
                mint: ctx.accounts.honey_token_mint.to_account_info(),
            },
            signer,
        ),
        claim_amount,
        HONEY_DECIMALS,
    )?;

    // Update claimable amount
    global_honey_config.distribution_config.claimable_amm_amount = global_honey_config
        .distribution_config
        .claimable_amm_amount
        .checked_sub(claim_amount)
        .ok_or(DragonHiveError::ArithmeticUnderflow)?;

    msg!("✅ AMM tokens claimed successfully");
    msg!("   Amount claimed: {}", claim_amount);
    msg!("   Remaining claimable: {}", global_honey_config.distribution_config.claimable_amm_amount);

    emit!(AmmTokensClaimed {
        amm_recipient: ctx.accounts.amm_recipient.key(),
        amount_claimed: claim_amount,
        remaining_claimable: global_honey_config.distribution_config.claimable_amm_amount,
    });

    Ok(())
}
