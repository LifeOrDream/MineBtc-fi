use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_core::{
    ID as MPL_CORE_PROGRAM_ID,
    instructions::{CreateV1CpiBuilder, UpdateV1CpiBuilder, BurnV1CpiBuilder},
    types::{PluginAuthorityPair, Plugin},
};

use crate::{
    constants::*,
    errors::DragonHiveError,
    events::*,
    state::*,
    utils::*,
};

// ========================================================================================
// =============================== CREATE USER PROFILE =================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct CreateUserProfile<'info> {
    #[account(
        init,
        payer = user,
        space = UserProfile::LEN,
        seeds = [USER_PROFILE_SEED, user.key().as_ref()],
        bump
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn create_user_profile_handler(ctx: Context<CreateUserProfile>) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    let current_time = get_current_timestamp()?;

    user_profile.owner = ctx.accounts.user.key();
    user_profile.dragonbees = Vec::new();
    user_profile.total_sol_spent = 0;
    user_profile.total_breeding_fees = 0;
    user_profile.dragonbees_killed = 0;
    user_profile.honey_tokens_earned = 0;
    user_profile.created_at = current_time;
    user_profile.last_activity = current_time;
    user_profile.bump = ctx.bumps.user_profile;

    emit!(UserProfileCreated {
        owner: ctx.accounts.user.key(),
        created_at: current_time,
    });

    Ok(())
}

// ========================================================================================
// =============================== PURCHASE DRAGONBEE ==================================== 
// ========================================================================================

#[derive(Accounts)]
pub struct PurchaseDragonBee<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED, buyer.key().as_ref()],
        bump = user_profile.bump
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        init,
        payer = buyer,
        space = DragonBeeMetadata::LEN,
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.key().as_ref()],
        bump
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,

    #[account(mut)]
    pub dragonbee_mint: Signer<'info>,

    #[account(
        constraint = collection_mint.key() == global_config.collection_mint @ DragonHiveError::InvalidAccount
    )]
    pub collection_mint: UncheckedAccount<'info>,

    /// SOL treasury for fee collection
    /// CHECK: PDA will be validated by seeds
    #[account(
        mut,
        seeds = [SOL_TREASURY_SEED],
        bump = global_config.treasury_bump
    )]
    pub sol_treasury: SystemAccount<'info>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn purchase_dragonbee_handler(ctx: Context<PurchaseDragonBee>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let user_profile = &mut ctx.accounts.user_profile;
    let dragonbee_metadata = &mut ctx.accounts.dragonbee_metadata;

    // Check if program is not paused
    require!(!global_config.is_paused, DragonHiveError::ProgramPaused);

    // Check supply limit
    require!(
        global_config.total_dragonbees_minted < MAX_DRAGONBEE_SUPPLY,
        DragonHiveError::MaxSupplyReached
    );

    let nft_price = global_config.nft_price;
    let current_time = get_current_timestamp()?;
    let slot = Clock::get()?.slot;

    // Generate random genetic data
    let genes = generate_genesis_genes(
        BEE_TYPE_SOLAR + (global_config.total_dragonbees_minted % 8) as u8, // Cycle through types
        slot,
        &ctx.accounts.buyer.key(),
    );
    let initial_power = crate::state::genetics::calculate_power_from_genes(&genes);
    let bee_type = crate::state::genetics::extract_bee_type(&genes);

    // Transfer SOL payment to treasury
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.sol_treasury.to_account_info(),
            },
        ),
        nft_price,
    )?;

    // Generate NFT name and URI
    let nft_name = format!("DragonBee #{}", global_config.total_dragonbees_minted + 1);
    let nft_uri = format!("https://dragonhive.io/metadata/{}", ctx.accounts.dragonbee_mint.key());

    // Initialize DragonBee metadata
    dragonbee_metadata.mint = ctx.accounts.dragonbee_mint.key();
    dragonbee_metadata.owner = ctx.accounts.buyer.key();
    dragonbee_metadata.name = nft_name.clone();
    dragonbee_metadata.uri = nft_uri.clone();
    dragonbee_metadata.genes = genes;
    dragonbee_metadata.evolution_stage = EVOLUTION_LARVA;
    dragonbee_metadata.bee_type = bee_type;
    dragonbee_metadata.power = initial_power;
    dragonbee_metadata.generation = 0; // Genesis
    dragonbee_metadata.parent1 = None;
    dragonbee_metadata.parent2 = None;
    dragonbee_metadata.birth_time = current_time;
    dragonbee_metadata.last_breeding_time = 0;
    dragonbee_metadata.breeding_count = 0;
    dragonbee_metadata.cooldown_stage = 0;
    dragonbee_metadata.is_queen = false;
    dragonbee_metadata.queen_breeding_price = 0;
    dragonbee_metadata.game_interactions = 0;
    dragonbee_metadata.in_game = false;
    dragonbee_metadata.bump = ctx.bumps.dragonbee_metadata;

    // Create the DragonBee NFT using MPL Core
    CreateV1CpiBuilder::new(&ctx.accounts.mpl_core_program)
        .asset(&ctx.accounts.dragonbee_mint)
        .collection(Some(&ctx.accounts.collection_mint))
        .payer(&ctx.accounts.buyer)
        .authority(Some(&ctx.accounts.buyer))
        .owner(Some(&ctx.accounts.buyer))
        .system_program(&ctx.accounts.system_program)
        .name(nft_name.clone())
        .uri(nft_uri.clone())
        .plugins(vec![
            PluginAuthorityPair {
                plugin: Plugin::UpdateDelegate(mpl_core::types::UpdateDelegate {
                    additional_delegates: vec![crate::ID],
                }),
                authority: None,
            }
        ])
        .invoke()?;

    // Update user profile
    user_profile.dragonbees.push(ctx.accounts.dragonbee_mint.key());
    user_profile.total_sol_spent = user_profile.total_sol_spent
        .checked_add(nft_price)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    user_profile.last_activity = current_time;

    // Update global stats
    global_config.total_dragonbees_minted = global_config.total_dragonbees_minted
        .checked_add(1)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;
    global_config.total_sol_collected = global_config.total_sol_collected
        .checked_add(nft_price)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    // Calculate fee distribution
    let (team_portion, buyback_portion, kill_pool_portion) = calculate_fee_distribution(nft_price)?;
    global_config.kill_rewards_pool = global_config.kill_rewards_pool
        .checked_add(kill_pool_portion)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    emit!(DragonBeePurchased {
        mint: ctx.accounts.dragonbee_mint.key(),
        buyer: ctx.accounts.buyer.key(),
        price_paid: nft_price,
        total_minted: global_config.total_dragonbees_minted,
    });

    emit!(SOLFeesCollected {
        source: "nft_sale".to_string(),
        amount: nft_price,
        team_portion,
        buyback_portion,
        kill_pool_portion,
    });

    Ok(())
}

// ========================================================================================
// =============================== EVOLVE DRAGONBEE ====================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct EvolveDragonBee<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.key().as_ref()],
        bump = dragonbee_metadata.bump,
        constraint = dragonbee_metadata.owner == owner.key() @ DragonHiveError::DragonBeeNotOwnedByUser,
        constraint = !dragonbee_metadata.in_game @ DragonHiveError::DragonBeeInGame
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,

    /// CHECK: DragonBee mint account
    pub dragonbee_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn evolve_dragonbee_handler(
    ctx: Context<EvolveDragonBee>,
    dragonbee_mint: Pubkey,
) -> Result<()> {
    let dragonbee_metadata = &mut ctx.accounts.dragonbee_metadata;

    // Check if program is not paused
    require!(!ctx.accounts.global_config.is_paused, DragonHiveError::ProgramPaused);

    // Check if DragonBee can evolve
    require!(dragonbee_metadata.can_evolve(), DragonHiveError::NotReadyForEvolution);

    let old_evolution_stage = dragonbee_metadata.evolution_stage;
    let old_power = dragonbee_metadata.power;

    // Evolve genetic code
    let evolved_genes = evolve_genetic_code(&dragonbee_metadata.genes, old_evolution_stage)?;
    let new_power = crate::state::genetics::calculate_power_from_genes(&evolved_genes);

    // Update DragonBee metadata
    dragonbee_metadata.genes = evolved_genes;
    dragonbee_metadata.evolution_stage = old_evolution_stage + 1;
    dragonbee_metadata.power = new_power;

    // Generate new URI for evolved form
    let new_uri = format!(
        "https://dragonhive.io/metadata/{}/evolution/{}",
        dragonbee_mint,
        dragonbee_metadata.evolution_stage
    );
    dragonbee_metadata.uri = new_uri.clone();

    // Update NFT metadata using MPL Core
    let authority_seeds = &[
        GLOBAL_CONFIG_SEED,
        &[ctx.accounts.global_config.config_bump],
    ];

    UpdateV1CpiBuilder::new(&ctx.accounts.mpl_core_program)
        .asset(&ctx.accounts.dragonbee_mint)
        .authority(Some(&ctx.accounts.global_config.to_account_info()))
        .system_program(&ctx.accounts.system_program)
        .new_name(format!("DragonBee {} Lv.{}", dragonbee_mint, dragonbee_metadata.evolution_stage))
        .new_uri(new_uri)
        .invoke_signed(&[authority_seeds])?;

    emit!(DragonBeeEvolved {
        mint: dragonbee_mint,
        owner: ctx.accounts.owner.key(),
        old_evolution_stage,
        new_evolution_stage: dragonbee_metadata.evolution_stage,
        old_power,
        new_power,
        new_genes: evolved_genes,
    });

    Ok(())
}

// ========================================================================================
// =============================== UPDATE DRAGONBEE STATS =============================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey, power_increase: u32, new_uri: Option<String>)]
pub struct UpdateDragonBeeStats<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.key().as_ref()],
        bump = dragonbee_metadata.bump,
        constraint = dragonbee_metadata.owner == owner.key() @ DragonHiveError::DragonBeeNotOwnedByUser
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,

    /// CHECK: DragonBee mint account
    pub dragonbee_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn update_dragonbee_stats_handler(
    ctx: Context<UpdateDragonBeeStats>,
    dragonbee_mint: Pubkey,
    power_increase: u32,
    new_uri: Option<String>,
) -> Result<()> {
    let dragonbee_metadata = &mut ctx.accounts.dragonbee_metadata;

    // Check if program is not paused
    require!(!ctx.accounts.global_config.is_paused, DragonHiveError::ProgramPaused);

    // Validate power increase
    validate_power_increase(power_increase)?;

    let old_power = dragonbee_metadata.power;

    // Update power and game interactions
    dragonbee_metadata.power = dragonbee_metadata.power
        .saturating_add(power_increase);
    dragonbee_metadata.game_interactions = dragonbee_metadata.game_interactions
        .saturating_add(1);

    // Update URI if provided
    if let Some(uri) = &new_uri {
        validate_uri(uri)?;
        dragonbee_metadata.uri = uri.clone();

        // Update NFT metadata using MPL Core
        let authority_seeds = &[
            GLOBAL_CONFIG_SEED,
            &[ctx.accounts.global_config.config_bump],
        ];

        UpdateV1CpiBuilder::new(&ctx.accounts.mpl_core_program)
            .asset(&ctx.accounts.dragonbee_mint)
            .authority(Some(&ctx.accounts.global_config.to_account_info()))
            .system_program(&ctx.accounts.system_program)
            .new_uri(uri.clone())
            .invoke_signed(&[authority_seeds])?;
    }

    emit!(DragonBeeStatsUpdated {
        mint: dragonbee_mint,
        owner: ctx.accounts.owner.key(),
        old_power,
        new_power: dragonbee_metadata.power,
        power_increase,
        new_uri,
        game_interactions: dragonbee_metadata.game_interactions,
    });

    Ok(())
}

// ========================================================================================
// =============================== KILL DRAGONBEE ======================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(dragonbee_mint: Pubkey)]
pub struct KillDragonBee<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED, owner.key().as_ref()],
        bump = user_profile.bump
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [DRAGONBEE_METADATA_SEED, dragonbee_mint.key().as_ref()],
        bump = dragonbee_metadata.bump,
        constraint = dragonbee_metadata.owner == owner.key() @ DragonHiveError::DragonBeeNotOwnedByUser,
        constraint = !dragonbee_metadata.in_game @ DragonHiveError::DragonBeeInGame,
        close = owner
    )]
    pub dragonbee_metadata: Account<'info, DragonBeeMetadata>,

    /// DRAGON token vault
    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_config.vault_bump,
        constraint = honey_vault.mint == global_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    /// HONEY vault authority PDA
    /// CHECK: PDA will be validated by seeds
    #[account(
        seeds = [HONEY_VAULT_AUTHORITY_SEED],
        bump = global_config.vault_authority_bump
    )]
    pub honey_vault_authority: UncheckedAccount<'info>,

    /// User's HONEY token account
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = honey_token_mint,
        associated_token::authority = owner
    )]
    pub user_honey_account: InterfaceAccount<'info, TokenAccount>,

    /// HONEY token mint
    #[account(
        constraint = honey_token_mint.key() == global_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: DragonBee mint account
    #[account(mut)]
    pub dragonbee_mint: UncheckedAccount<'info>,

    /// CHECK: Collection mint account
    #[account(
        constraint = collection_mint.key() == global_config.collection_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub collection_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn kill_dragonbee_handler(
    ctx: Context<KillDragonBee>,
    dragonbee_mint: Pubkey,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let user_profile = &mut ctx.accounts.user_profile;
    let dragonbee_metadata = &ctx.accounts.dragonbee_metadata;

    // Check if program is not paused
    require!(!global_config.is_paused, DragonHiveError::ProgramPaused);

    // Check if kill rewards pool has tokens
    require!(global_config.kill_rewards_pool > 0, DragonHiveError::EmptyKillRewardsPool);

    // Calculate total active power (simplified - in production, calculate from all DragonBees)
    let total_active_power = 1_000_000u64; // Placeholder - should be calculated dynamically
    
    // Calculate kill reward
    let reward_amount = calculate_kill_reward(
        dragonbee_metadata.power,
        total_active_power,
        global_config.kill_rewards_pool,
    )?;

    require!(reward_amount > 0, DragonHiveError::InsufficientPowerForKill);

    // Burn the DragonBee NFT using MPL Core
    BurnV1CpiBuilder::new(&ctx.accounts.mpl_core_program)
        .asset(&ctx.accounts.dragonbee_mint)
        .collection(Some(&ctx.accounts.collection_mint))
        .payer(&ctx.accounts.owner)
        .authority(Some(&ctx.accounts.owner))
        .system_program(Some(&ctx.accounts.system_program))
        .invoke()?;

    // Transfer HONEY tokens as reward
    let authority_seeds = &[
        HONEY_VAULT_AUTHORITY_SEED,
        &[global_config.vault_authority_bump],
    ];

    anchor_spl::token_interface::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.honey_vault.to_account_info(),
                to: ctx.accounts.user_honey_account.to_account_info(),
                authority: ctx.accounts.honey_vault_authority.to_account_info(),
            },
            &[authority_seeds],
        ),
        reward_amount,
    )?;

    // Update global stats
    global_config.kill_rewards_pool = global_config.kill_rewards_pool
        .checked_sub(reward_amount)
        .ok_or(DragonHiveError::ArithmeticUnderflow)?;

    // Update user profile
    user_profile.dragonbees.retain(|&x| x != dragonbee_mint);
    user_profile.dragonbees_killed = user_profile.dragonbees_killed
        .saturating_add(1);
    user_profile.honey_tokens_earned = user_profile.honey_tokens_earned
        .checked_add(reward_amount)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    emit!(DragonBeeKilled {
        mint: dragonbee_mint,
        owner: ctx.accounts.owner.key(),
        power: dragonbee_metadata.power,
        dragon_tokens_earned: reward_amount,
        remaining_pool: global_config.kill_rewards_pool,
    });

    Ok(())
}

// Note: Standard breeding has been replaced with the queen auction system
// All breeding now happens through the auction mechanism in breeding.rs
