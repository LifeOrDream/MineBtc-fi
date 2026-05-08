use anchor_lang::prelude::*;
use mpl_core::{instructions::TransferV1CpiBuilder, ID as MPL_CORE_PROGRAM_ID};

use crate::errors::MarketError;

/// Transfer an mpl-core asset via `TransferV1` CPI.
///
/// Mirrors the helper in `mineBTC` so the two programs stay independent — no
/// shared crate, just a verified-good copy. `signer_seeds` is `Some` when the
/// current authority is a PDA (e.g. our escrow PDA returning the asset).
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

    // The `mpl_core_program` AccountInfo must actually be the canonical program.
    // We additionally verify it equals the cached `config.mpl_core_program` at
    // each callsite, but this is the floor check.
    require!(
        mpl_core_program.key() == MPL_CORE_PROGRAM_ID,
        MarketError::InvalidMplCoreProgram
    );

    let mut cpi_builder = TransferV1CpiBuilder::new(mpl_core_program);

    cpi_builder
        .asset(asset)
        .payer(payer)
        .authority(Some(authority))
        .new_owner(new_owner);

    // mpl-core errors out with IncorrectAccount if the asset belongs to a
    // collection and `collection` isn't passed. Always thread it through.
    if let Some(collection_account) = collection {
        msg!("📚 Collection: {}", collection_account.key());
        cpi_builder.collection(Some(collection_account));
    }

    msg!("🚀 Invoking Metaplex Core TransferV1 CPI...");
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
