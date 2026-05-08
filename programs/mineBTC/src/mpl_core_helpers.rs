use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseAssetV1,
    instructions::{BurnV1CpiBuilder, CreateV1CpiBuilder, TransferV1CpiBuilder},
    ID as MPL_CORE_PROGRAM_ID,
};

/// Create a Metaplex Core NFT asset via CPI
/// Note: Uses multiple lifetimes to support mixing remaining_accounts with ctx.accounts
pub fn create_mpl_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: Option<&AccountInfo<'info>>, // Use 'info here too
    authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    name: String,
    uri: String,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    msg!("🎨 MPL Core Helper: create_mpl_core_asset");
    msg!("   Asset: {}", asset.key());
    msg!("   Owner: {}", owner.key());
    msg!("   Authority: {}", authority.key());
    msg!("   Name: {}", name);

    // Validate Metaplex Core program
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        crate::errors::ErrorCode::InvalidMplCoreProgram
    );
    msg!(
        "✅ Metaplex Core program validated: {}",
        mpl_core_program.key()
    );

    // Build CreateV1 CPI
    let mut cpi_builder = CreateV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .asset(asset)
        .payer(payer)
        .system_program(system_program)
        .name(name.clone())
        .uri(uri.clone())
        .owner(Some(owner))
        // Set the signer authority. This fixes the compiler error.
        .authority(Some(authority));
    // DO NOT set .update_authority(). This fixes the 0x1d runtime error.

    // Add collection if provided
    if let Some(collection_account) = collection {
        msg!("📚 Adding to collection: {}", collection_account.key());
        cpi_builder.collection(Some(collection_account));
    }

    msg!("🚀 Invoking Metaplex Core CreateV1 CPI...");
    // Execute CPI with or without signer seeds
    if let Some(seeds) = signer_seeds {
        cpi_builder.invoke_signed(seeds)?;
    } else {
        cpi_builder.invoke()?;
    }

    msg!("✅ Metaplex Core asset created successfully");

    Ok(())
}

/// Transfer a Metaplex Core NFT asset via CPI
pub fn transfer_mpl_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: Option<&AccountInfo<'info>>,
    payer: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    new_owner: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    msg!("🔄 MPL Core Helper: transfer_mpl_core_asset");
    msg!("   Asset: {}", asset.key());
    msg!("   From authority: {}", authority.key());
    msg!("   To new owner: {}", new_owner.key());

    // Validate Metaplex Core program
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        crate::errors::ErrorCode::InvalidMplCoreProgram
    );
    msg!("✅ Metaplex Core program validated");

    // Build TransferV1 CPI
    let mut cpi_builder = TransferV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .asset(asset)
        .payer(payer)
        .authority(Some(authority))
        .new_owner(new_owner);

    // Add collection if provided
    if let Some(collection_account) = collection {
        msg!("📚 Collection: {}", collection_account.key());
        cpi_builder.collection(Some(collection_account));
    }

    msg!("🚀 Invoking Metaplex Core TransferV1 CPI...");
    // Execute CPI with or without signer seeds
    if let Some(seeds) = signer_seeds {
        msg!("   Using PDA signer seeds");
        cpi_builder.invoke_signed(seeds)?;
    } else {
        msg!("   Using regular signer");
        cpi_builder.invoke()?;
    }

    msg!("✅ NFT transferred successfully");

    Ok(())
}

/// Burn a Metaplex Core NFT asset via CPI.
///
/// Used by the `rebirth_hashbeast` cascade when the country lootbox queue is full —
/// the user still receives their `accumulated_val` payout, but the asset is
/// destroyed rather than added to inventory (no listing fallback).
pub fn burn_mpl_core_asset<'info>(
    asset: &AccountInfo<'info>,
    collection: Option<&AccountInfo<'info>>,
    payer: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    msg!("🔥 MPL Core Helper: burn_mpl_core_asset");
    msg!("   Asset: {}", asset.key());
    msg!("   Authority: {}", authority.key());

    // Validate Metaplex Core program
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        crate::errors::ErrorCode::InvalidMplCoreProgram
    );
    msg!("✅ Metaplex Core program validated");

    // Build BurnV1 CPI
    let mut cpi_builder = BurnV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .asset(asset)
        .payer(payer)
        .authority(Some(authority));

    // Add collection if provided
    if let Some(collection_account) = collection {
        msg!("📚 Collection: {}", collection_account.key());
        cpi_builder.collection(Some(collection_account));
    }

    msg!("🚀 Invoking Metaplex Core BurnV1 CPI...");
    // Execute CPI with or without signer seeds
    if let Some(seeds) = signer_seeds {
        msg!("   Using PDA signer seeds");
        cpi_builder.invoke_signed(seeds)?;
    } else {
        msg!("   Using regular signer");
        cpi_builder.invoke()?;
    }

    msg!("✅ NFT burnt successfully");

    Ok(())
}

/// Get NFT owner from Metaplex Core asset account
pub fn get_mpl_core_owner(asset_account: &AccountInfo) -> Result<Pubkey> {
    msg!("🔍 Getting MPL Core owner");
    msg!("   Asset account: {}", asset_account.key());
    msg!("   Owner: {}", asset_account.owner);

    // Ensure you're actually looking at a Core account
    require_keys_eq!(
        *asset_account.owner,
        MPL_CORE_PROGRAM_ID,
        crate::errors::ErrorCode::InvalidMplCoreProgram
    );

    let data = asset_account.try_borrow_data()?;
    let mut data_ref: &[u8] = &data;

    // This reads the full Core account (including the Core discriminator/header),
    // not Anchor’s discriminator. Adjust the type to AssetV1 if that’s your asset type.
    let asset = BaseAssetV1::deserialize(&mut data_ref)
        .map_err(|_| crate::errors::ErrorCode::InvalidAccount)?;

    // Depending on your Core version, owner is either Pubkey or [u8;32]
    #[allow(unused_mut)]
    let owner: Pubkey = {
        // If asset.owner is already Pubkey:
        // asset.owner

        // If it's [u8; 32], do:
        Pubkey::new_from_array(asset.owner.to_bytes())
    };

    Ok(owner)
}
