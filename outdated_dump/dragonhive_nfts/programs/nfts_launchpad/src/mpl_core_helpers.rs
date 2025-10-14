use anchor_lang::prelude::*;
use mpl_core::{
    instructions::{CreateV1CpiBuilder, TransferV1CpiBuilder},
    types::{DataState, UpdateAuthority},
    ID as MPL_CORE_PROGRAM_ID,
};

/// Create a Metaplex Core NFT asset via CPI
pub fn create_mpl_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: Option<&AccountInfo<'info>>,
    authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    name: String,
    uri: String,
) -> Result<()> {
    // Validate Metaplex Core program
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        crate::errors::NftLaunchpadError::InvalidMplCoreProgram
    );
    
    // Build CreateV1 CPI
    let mut cpi_builder = CreateV1CpiBuilder::new(mpl_core_program);
    
    cpi_builder
        .asset(asset)
        .payer(payer)
        .system_program(Some(system_program))
        .name(name)
        .uri(uri)
        .owner(Some(owner))
        .update_authority(Some(UpdateAuthority::Address(*authority.key)));
    
    // Add collection if provided
    if let Some(collection_account) = collection {
        cpi_builder.collection(Some(collection_account));
    }
    
    // Execute CPI
    cpi_builder.invoke()?;
    
    Ok(())
}

/// Transfer a Metaplex Core NFT asset via CPI
pub fn transfer_mpl_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: Option<&AccountInfo<'info>>,
    payer: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    new_owner: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
) -> Result<()> {
    // Validate Metaplex Core program
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        crate::errors::NftLaunchpadError::InvalidMplCoreProgram
    );
    
    // Build TransferV1 CPI
    let mut cpi_builder = TransferV1CpiBuilder::new(mpl_core_program);
    
    cpi_builder
        .asset(asset)
        .payer(payer)
        .authority(Some(authority))
        .new_owner(new_owner);
    
    // Add collection if provided
    if let Some(collection_account) = collection {
        cpi_builder.collection(Some(collection_account));
    }
    
    // Execute CPI
    cpi_builder.invoke()?;
    
    Ok(())
}

/// Get NFT owner from Metaplex Core asset account
pub fn get_mpl_core_owner(asset_account: &AccountInfo) -> Result<Pubkey> {
    // Metaplex Core V1 stores owner at bytes 8-40 (after discriminator)
    let data = asset_account.try_borrow_data()?;
    
    require!(
        data.len() >= 40,
        crate::errors::NftLaunchpadError::InvalidAccount
    );
    
    let owner_bytes = &data[8..40];
    let owner = Pubkey::try_from(owner_bytes)
        .map_err(|_| crate::errors::NftLaunchpadError::InvalidAccount)?;
    
    Ok(owner)
}

