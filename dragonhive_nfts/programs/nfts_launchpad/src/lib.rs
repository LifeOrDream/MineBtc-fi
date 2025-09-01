use anchor_lang::prelude::*;
use crate::state::{DragonBeeInfoResponse, UserDragonBeesResponse};

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("Xic5yxcsGtWRuWcmxN1cK8GQ3ddt3BD3osoBaY1a8j9");

#[program]
pub mod dragonhive_nfts {
    use super::*;

    // ========================================================================================
    // ================================ ADMIN FUNCTIONS ======================================
    // ========================================================================================

    // /// Initialize the DragonHive NFT program with global configuration
    // /// Creates the main program state, DRAGON token vault, and NFT collection
    // pub fn initialize(
    //     ctx: Context<Initialize>,
    //     collection_name: String,
    //     collection_symbol: String,
    //     collection_uri: String,
    //     honey_token_mint: Pubkey,
    // ) -> Result<()> {
    //     instructions::admin::initialize_handler(ctx, collection_name, collection_symbol, collection_uri, honey_token_mint)
    // }

    // /// Update global configuration (admin only)
    // pub fn update_config(
    //     ctx: Context<UpdateConfig>,
    //     new_authority: Option<Pubkey>,
    //     new_treasury: Option<Pubkey>,
    //     new_nft_price: Option<u64>,
    //     new_breeding_fee: Option<u64>,
    // ) -> Result<()> {
    //     instructions::admin::update_config_handler(ctx, new_authority, new_treasury, new_nft_price, new_breeding_fee)
    // }

    // /// Mint genesis DragonBee NFTs (admin only) - for initial sale of 15,000 NFTs
    // pub fn mint_genesis_dragonbee(
    //     ctx: Context<MintGenesisDragonBee>,
    //     name: String,
    //     uri: String,
    //     bee_type: u8,
    //     initial_genes: [u8; 32], // 256-bit genetic code
    // ) -> Result<()> {
    //     instructions::admin::mint_genesis_dragonbee_handler(ctx, name, uri, bee_type, initial_genes)
    // }

    // /// Deposit HONEY tokens to the program vault (admin only)
    // pub fn deposit_honey_tokens(
    //     ctx: Context<DepositHoneyTokens>,
    //     amount: u64,
    // ) -> Result<()> {
    //     instructions::admin::deposit_honey_tokens_handler(ctx, amount)
    // }

    // Note: set_queen_bee function moved to breeding module as part of queen auction system

    // ========================================================================================
    // ================================= USER FUNCTIONS ======================================
    // ========================================================================================

    // /// Purchase a DragonBee NFT from the program (1 SOL each)
    // pub fn purchase_dragonbee(ctx: Context<PurchaseDragonBee>) -> Result<()> {
    //     instructions::user::purchase_dragonbee_handler(ctx)
    // }

    // /// Evolve a DragonBee NFT (increases power and changes appearance)
    // pub fn evolve_dragonbee(
    //     ctx: Context<EvolveDragonBee>,
    //     dragonbee_mint: Pubkey,
    // ) -> Result<()> {
    //     instructions::user::evolve_dragonbee_handler(ctx, dragonbee_mint)
    // }

    // /// Kill/Burn a DragonBee NFT to claim HONEY tokens
    // pub fn kill_dragonbee(
    //     ctx: Context<KillDragonBee>,
    //     dragonbee_mint: Pubkey,
    // ) -> Result<()> {
    //     instructions::user::kill_dragonbee_handler(ctx, dragonbee_mint)
    // }

    // /// Update DragonBee metadata (for game interactions)
    // pub fn update_dragonbee_stats(
    //     ctx: Context<UpdateDragonBeeStats>,
    //     dragonbee_mint: Pubkey,
    //     power_increase: u32,
    //     new_uri: Option<String>,
    // ) -> Result<()> {
    //     instructions::user::update_dragonbee_stats_handler(ctx, dragonbee_mint, power_increase, new_uri)
    // }

    // /// Create user profile for DragonBee ownership tracking
    // pub fn create_user_profile(ctx: Context<CreateUserProfile>) -> Result<()> {
    //     instructions::user::create_user_profile_handler(ctx)
    // }

    // ========================================================================================
    // =============================== QUEEN AUCTION SYSTEM ================================== 
    // ========================================================================================

    // /// Initialize the queen auction manager (admin only)
    // pub fn initialize_queen_auction_manager(
    //     ctx: Context<InitializeQueenAuctionManager>,
    // ) -> Result<()> {
    //     instructions::breeding::initialize_queen_auction_manager_handler(ctx)
    // }

    // /// Update auction configuration (admin only)
    // pub fn update_auction_config(
    //     ctx: Context<UpdateAuctionConfig>,
    //     are_live: Option<bool>,
    //     price_to_be_queen: Option<u64>,
    //     bid_increase_pct: Option<u64>,
    //     bid_decrease_pct: Option<u64>,
    //     energy_tax: Option<u64>,
    //     max_eggs_per_queen: Option<u64>,
    // ) -> Result<()> {
    //     instructions::breeding::update_auction_config_handler(
    //         ctx, are_live, price_to_be_queen, bid_increase_pct, 
    //         bid_decrease_pct, energy_tax, max_eggs_per_queen
    //     )
    // }

    // /// Compete to become a queen in the current auction
    // pub fn compete_to_be_queen(
    //     ctx: Context<CompeteToBeQueen>,
    //     dragonbee_mint: Pubkey,
    //     bid_amount: u64,
    // ) -> Result<()> {
    //     instructions::breeding::compete_to_be_queen_handler(ctx, dragonbee_mint, bid_amount)
    // }

    // /// Add additional SUI to existing bid (limited phase only)
    // pub fn add_to_bid(
    //     ctx: Context<AddToBid>,
    //     additional_bid: u64,
    // ) -> Result<()> {
    //     instructions::breeding::add_to_bid_handler(ctx, additional_bid)
    // }

    // /// Finalize the current auction and make winners into queens
    // pub fn finalize_auction(
    //     ctx: Context<FinalizeAuction>,
    // ) -> Result<()> {
    //     instructions::breeding::finalize_auction_handler(ctx)
    // }

    // ========================================================================================
    // ================================ QUERY FUNCTIONS ====================================== 
    // ========================================================================================

    // /// Get DragonBee information
    // pub fn get_dragonbee_info(
    //     ctx: Context<GetDragonBeeInfo>,
    //     dragonbee_mint: Pubkey,
    // ) -> Result<DragonBeeInfoResponse> {
    //     instructions::queries::get_dragonbee_info_handler(ctx, dragonbee_mint)
    // }

    // /// Get user's DragonBee collection
    // pub fn get_user_dragonbees(
    //     ctx: Context<GetUserDragonBees>,
    //     user: Pubkey,
    // ) -> Result<UserDragonBeesResponse> {
    //     instructions::queries::get_user_dragonbees_handler(ctx, user)
    // }
}
