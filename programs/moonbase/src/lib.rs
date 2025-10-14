use anchor_lang::prelude::*;
use anchor_lang::system_program;
mod state;
mod errors;
mod events;
pub mod instructions;

pub use instructions::admin::*;
// pub use instructions::user::*;
// pub use instructions::game::*;

declare_id!("76bGWqGdzwR13FSd1TDwanK7GFDHcunKh6WGbzAW1PjU");

#[program]
pub mod moonbase {
    use super::*;
    use crate::instructions::admin::{self};

    // ----------------------------------------------------------------------------------------
    // ------------ GLOBAL_CONFIG (ADMIN) :: UPDATES, ADDING FACTIONS / EXPANSIONS ------------
    // ----------------------------------------------------------------------------------------

    /// Initialize the global program configuration
    /// This function can only be called once as it creates the program's configuration accounts
    /// It will fail if the accounts already exist
    pub fn initialize(ctx: Context<Initialize>, base_creation_cost: u64, creation_fee_recipient: Pubkey) -> Result<()> {
        admin::internal_initialize(ctx, base_creation_cost, creation_fee_recipient)         
    }
    
    /// Update the global configuration parameters
    /// Can only be called by the current authority
    pub fn update_config(
        ctx: Context<UpdateConfigAc>,
        new_authority: Option<Pubkey>,
        new_fee_collector: Option<Pubkey>,
        new_creation_fee_recipient: Option<Pubkey>,
        new_base_creation_cost: Option<u64>,
        new_loot_percentage: Option<u8>,
    ) -> Result<()> {
        admin::update_config_internal(
            ctx,
            new_authority,
            new_fee_collector,
            new_creation_fee_recipient,
            new_base_creation_cost,
            new_loot_percentage,
        )
    }

    /// Add a new expansion configuration (admin only)
    pub fn add_expansion(
        ctx: Context<AddExpansion>,
        id: u8,
        name: String,
        required_level: u8,
        cost_sol: u64,
        new_width: u8,
        new_height: u8,
    ) -> Result<()> {
        admin::add_expansion_internal(ctx, id, name, required_level, cost_sol, new_width, new_height)
    }

    
}

 