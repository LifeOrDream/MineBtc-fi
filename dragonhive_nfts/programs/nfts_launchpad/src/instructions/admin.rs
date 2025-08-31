use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::{Token2022, Mint as Mint2022, TokenAccount as TokenAccount2022},
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_core::{
    ID as MPL_CORE_PROGRAM_ID,
    instructions::CreateV1CpiBuilder,
    types::{DataState, PluginAuthorityPair, Plugin},
};

use crate::{
    constants::*,
    errors::DragonHiveError,
    events::*,
    state::*,
    utils::*,
};

// ========================================================================================
// =============================== INITIALIZE PROGRAM ==================================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(
    collection_name: String,
    collection_symbol: String,
    collection_uri: String,
    honey_token_mint: Pubkey,
)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [GLOBAL_CONFIG_SEED],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// HONEY token vault for storing rewards
    #[account(
        init,
        payer = authority,
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

    /// SOL treasury for collecting fees
    /// CHECK: PDA will be validated by seeds
    #[account(
        init,
        payer = authority,
        space = 0,
        seeds = [SOL_TREASURY_SEED],
        bump
    )]
    pub sol_treasury: SystemAccount<'info>,

    /// DragonBee collection mint (MPL Core)
    #[account(mut)]
    pub collection_mint: Signer<'info>,

    /// DRAGON token mint
    pub honey_token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(
    ctx: Context<Initialize>,
    collection_name: String,
    collection_symbol: String,
    collection_uri: String,
    honey_token_mint: Pubkey,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    
    // Validate inputs
    validate_name(&collection_name)?;
    validate_uri(&collection_uri)?;

    // Initialize global configuration
    global_config.authority = ctx.accounts.authority.key();
    global_config.treasury = ctx.accounts.sol_treasury.key();
    global_config.honey_token_mint = honey_token_mint;
    global_config.honey_vault = ctx.accounts.honey_vault.key();
    global_config.honey_vault_authority = ctx.accounts.honey_vault_authority.key();
    global_config.collection_mint = ctx.accounts.collection_mint.key();
    global_config.total_dragonbees_minted = 0;
    global_config.nft_price = DRAGONBEE_PRICE;
    global_config.breeding_fee = BASE_BREEDING_FEE;
    global_config.total_sol_collected = 0;
    global_config.kill_rewards_pool = 0;
    global_config.is_paused = false;
    global_config.config_bump = ctx.bumps.global_config;
    global_config.vault_bump = ctx.bumps.dragon_vault;
    global_config.vault_authority_bump = ctx.bumps.dragon_vault_authority;
    global_config.treasury_bump = ctx.bumps.sol_treasury;

    // Create the DragonBee collection using MPL Core
    CreateV1CpiBuilder::new(&ctx.accounts.mpl_core_program)
        .asset(&ctx.accounts.collection_mint)
        .collection(Some(&ctx.accounts.collection_mint))
        .payer(&ctx.accounts.authority)
        .authority(Some(&ctx.accounts.authority))
        .system_program(&ctx.accounts.system_program)
        .name(collection_name.clone())
        .uri(collection_uri.clone())
        .plugins(vec![
            PluginAuthorityPair {
                plugin: Plugin::UpdateDelegate {
                    additional_delegates: vec![],
                },
                authority: Some(mpl_core::types::Authority::UpdateAuthority),
            }
        ])
        .invoke()?;

    emit!(ProgramInitialized {
        authority: ctx.accounts.authority.key(),
        honey_token_mint,
        collection_mint: ctx.accounts.collection_mint.key(),
        nft_price: DRAGONBEE_PRICE,
        breeding_fee: BASE_BREEDING_FEE,
    });

    Ok(())
}

// ========================================================================================
// =============================== UPDATE CONFIG ========================================= 
// ========================================================================================

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ DragonHiveError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    new_authority: Option<Pubkey>,
    new_treasury: Option<Pubkey>,
    new_nft_price: Option<u64>,
    new_breeding_fee: Option<u64>,
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    if let Some(new_authority) = new_authority {
        global_config.authority = new_authority;
    }

    if let Some(new_treasury) = new_treasury {
        global_config.treasury = new_treasury;
    }

    if let Some(new_nft_price) = new_nft_price {
        require!(new_nft_price > 0, DragonHiveError::InvalidPaymentAmount);
        global_config.nft_price = new_nft_price;
    }

    if let Some(new_breeding_fee) = new_breeding_fee {
        require!(new_breeding_fee > 0, DragonHiveError::InvalidPaymentAmount);
        global_config.breeding_fee = new_breeding_fee;
    }

    emit!(ConfigUpdated {
        authority: ctx.accounts.authority.key(),
        new_authority,
        new_treasury,
        new_nft_price,
        new_breeding_fee,
    });

    Ok(())
}

// ========================================================================================
// =============================== MINT GENESIS DRAGONBEE =============================== 
// ========================================================================================

#[derive(Accounts)]
#[instruction(name: String, uri: String, bee_type: u8, initial_genes: [u8; 32])]
pub struct MintGenesisDragonBee<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == authority.key() @ DragonHiveError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
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

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: MPL Core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn mint_genesis_dragonbee_handler(
    ctx: Context<MintGenesisDragonBee>,
    name: String,
    uri: String,
    bee_type: u8,
    initial_genes: [u8; 32],
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;
    let dragonbee_metadata = &mut ctx.accounts.dragonbee_metadata;

    // Validate inputs
    validate_name(&name)?;
    validate_uri(&uri)?;
    validate_bee_type(bee_type)?;
    validate_genetic_data(&initial_genes)?;

    // Check supply limit
    require!(
        global_config.total_dragonbees_minted < MAX_DRAGONBEE_SUPPLY,
        DragonHiveError::MaxSupplyReached
    );

    let current_time = get_current_timestamp()?;
    let initial_power = crate::state::genetics::calculate_power_from_genes(&initial_genes);

    // Initialize DragonBee metadata
    dragonbee_metadata.mint = ctx.accounts.dragonbee_mint.key();
    dragonbee_metadata.owner = ctx.accounts.authority.key();
    dragonbee_metadata.name = name.clone();
    dragonbee_metadata.uri = uri.clone();
    dragonbee_metadata.genes = initial_genes;
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
        .payer(&ctx.accounts.authority)
        .authority(Some(&ctx.accounts.authority))
        .owner(&ctx.accounts.authority)
        .system_program(&ctx.accounts.system_program)
        .name(name.clone())
        .uri(uri.clone())
        .plugins(vec![
            PluginAuthorityPair {
                plugin: Plugin::UpdateDelegate {
                    additional_delegates: vec![ctx.accounts.authority.key()],
                },
                authority: Some(mpl_core::types::Authority::UpdateAuthority),
            }
        ])
        .invoke()?;

    // Update global stats
    global_config.total_dragonbees_minted = global_config.total_dragonbees_minted
        .checked_add(1)
        .ok_or(DragonHiveError::ArithmeticOverflow)?;

    emit!(DragonBeeGenesisMinted {
        mint: ctx.accounts.dragonbee_mint.key(),
        authority: ctx.accounts.authority.key(),
        name,
        bee_type,
        genes: initial_genes,
        initial_power,
    });

    Ok(())
}

// ========================================================================================
// =============================== DEPOSIT DRAGON TOKENS ================================= 
// ========================================================================================

#[derive(Accounts)]
pub struct DepositHoneyTokens<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.config_bump,
        constraint = global_config.authority == depositor.key() @ DragonHiveError::Unauthorized
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [HONEY_VAULT_SEED],
        bump = global_config.vault_bump,
        constraint = honey_vault.mint == global_config.honey_token_mint @ DragonHiveError::InvalidTokenMint
    )]
    pub honey_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = depositor_token_account.mint == global_config.honey_token_mint @ DragonHiveError::InvalidTokenMint,
        constraint = depositor_token_account.owner == depositor.key() @ DragonHiveError::Unauthorized
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount>,

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
    anchor_spl::token_interface::transfer(
        anchor_spl::token_interface::Transfer {
            from: ctx.accounts.depositor_token_account.to_account_info(),
            to: ctx.accounts.honey_vault.to_account_info(),
            authority: ctx.accounts.depositor.to_account_info(),
        },
        &ctx.accounts.token_program,
        amount,
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
// =============================== QUEEN AUCTION MANAGEMENT ============================== 
// ========================================================================================

/// Admin function to manage queen auction system - moved to breeding.rs
