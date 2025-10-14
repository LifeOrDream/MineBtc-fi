use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod mpl_core_helpers;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("CG6btG2MbTDXR2Ws6Kqn24HG6VqWWFJBrfxAK7NJVyNA");

#[program]
pub mod nft_launchpad {
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

    /// Add MoonDoge URIs to the pool (admin only)
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

    /// Clear all MoonDoge URIs (admin only)
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




}
 
