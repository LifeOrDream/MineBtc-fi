use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod mpl_core_helpers;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("Xic5yxcsGtWRuWcmxN1cK8GQ3ddt3BD3osoBaY1a8j9");

#[program]
pub mod nfts_launchpad {
    use super::*;

    // ========================================================================================
    // ================================ ADMIN FUNCTIONS ======================================
    // ========================================================================================

    /// Initialize the NFT Launchpad program with both collections
    pub fn initialize(
        ctx: Context<Initialize>,
        moondoge_collection_name: String,
        moondoge_collection_symbol: String,
        moondoge_collection_uri: String,
        dragon_egg_collection_name: String,
        dragon_egg_collection_symbol: String,
        dragon_egg_collection_uri: String,
    ) -> Result<()> {
        instructions::admin::initialize_handler(
            ctx,
            moondoge_collection_name,
            moondoge_collection_symbol,
            moondoge_collection_uri,
            dragon_egg_collection_name,
            dragon_egg_collection_symbol,
            dragon_egg_collection_uri,
        )
    }

    /// Update global configuration (admin only)
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_authority: Option<Pubkey>,
    ) -> Result<()> {
        instructions::admin::update_config_handler(ctx, new_authority)
    }

    /// Pause/unpause the program (admin only)
    pub fn pause_program(
        ctx: Context<PauseProgram>,
        is_paused: bool,
    ) -> Result<()> {
        instructions::admin::pause_program_handler(ctx, is_paused)
    }

    /// Add DogeBtc URIs to the pool (admin only)
    pub fn add_moondoge_uris(
        ctx: Context<UpdateConfig>,
        uris: Vec<String>,
    ) -> Result<()> {
        instructions::admin::add_moondoge_uris_handler(ctx, uris)
    }

    /// Add Dragon Egg URIs to the pool (admin only)
    pub fn add_dragon_egg_uris(
        ctx: Context<UpdateConfig>,
        uris: Vec<String>,
    ) -> Result<()> {
        instructions::admin::add_dragon_egg_uris_handler(ctx, uris)
    }

    /// Clear all DogeBtc URIs (admin only)
    pub fn clear_moondoge_uris(
        ctx: Context<UpdateConfig>,
    ) -> Result<()> {
        instructions::admin::clear_moondoge_uris_handler(ctx)
    }

    /// Clear all Dragon Egg URIs (admin only)
    pub fn clear_dragon_egg_uris(
        ctx: Context<UpdateConfig>,
    ) -> Result<()> {
        instructions::admin::clear_dragon_egg_uris_handler(ctx)
    }

    // ========================================================================================
    // ======================== MOONBASE CREATION WITH NFTS ==================================
    // ========================================================================================

    /// Mint NFTs based on moonbase creation tier (called by moonbase program)
    /// pricing_tier: MOONBASE_BASIC_PRICE (0.25), MOONBASE_DOGE_PRICE (0.5), or MOONBASE_FULL_PRICE (1.0)
    pub fn mint_nfts_for_moonbase(
        ctx: Context<MintNftsForMoonbase>,
        pricing_tier: u64,
    ) -> Result<()> {
        instructions::user::mint_nfts_for_moonbase_handler(ctx, pricing_tier)
    }

    // ========================================================================================
    // ======================== INDIVIDUAL NFT PURCHASES =====================================
    // ========================================================================================

    /// Purchase a DogeBtc NFT (0.5 SOL)
    pub fn purchase_moondoge(
        ctx: Context<PurchaseDogeBtc>,
    ) -> Result<()> {
        instructions::user::purchase_moondoge_handler(ctx)
    }

    /// Purchase a Dragon Egg NFT (0.5 SOL)
    pub fn purchase_dragon_egg(
        ctx: Context<PurchaseDragonEgg>,
    ) -> Result<()> {
        instructions::user::purchase_dragon_egg_handler(ctx)
    }

    // ========================================================================================
    // ======================== MOONDOGE ATTACHMENT ==========================================
    // ========================================================================================

    /// Attach DogeBtc to moonbase (1 per moonbase max)
    pub fn attach_moondoge(
        ctx: Context<AttachDogeBtc>,
    ) -> Result<()> {
        instructions::user::attach_moondoge_handler(ctx)
    }

    /// Detach DogeBtc from moonbase
    pub fn detach_moondoge(
        ctx: Context<DetachDogeBtc>,
    ) -> Result<()> {
        instructions::user::detach_moondoge_handler(ctx)
    }

    /// Update DogeBtc money based on DOGE_BTC mined (called periodically by backend)
    pub fn update_moondoge_money(
        ctx: Context<UpdateDogeBtcMoney>,
        dbtc_mined: u64,
    ) -> Result<()> {
        instructions::user::update_moondoge_money_handler(ctx, dbtc_mined)
    }

    // ========================================================================================
    // ======================== DRAGON EGG INCUBATION ========================================
    // ========================================================================================

    /// Add Dragon Egg to moonbase incubation
    pub fn incubate_dragon_egg(
        ctx: Context<IncubateDragonEgg>,
    ) -> Result<()> {
        instructions::user::incubate_dragon_egg_handler(ctx)
    }

    /// Remove Dragon Egg from moonbase incubation
    pub fn remove_dragon_egg(
        ctx: Context<RemoveDragonEgg>,
    ) -> Result<()> {
        instructions::user::remove_dragon_egg_handler(ctx)
    }

    /// Update Dragon Egg power based on hashpower (called periodically by backend)
    pub fn update_dragon_egg_power(
        ctx: Context<UpdateDragonEggPower>,
        total_hashpower: u64,
    ) -> Result<()> {
        instructions::user::update_dragon_egg_power_handler(ctx, total_hashpower)
    }
}
