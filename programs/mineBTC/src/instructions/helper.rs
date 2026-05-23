use crate::errors::ErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_lang::system_program;
use anchor_lang::system_program::{create_account, transfer, CreateAccount, Transfer};
use anchor_spl::token::{self as token_standard, Burn as StandardBurn};
use anchor_spl::token_2022::{
    self,
    spl_token_2022::{
        extension::{
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
        },
        state::Mint as SplToken2022Mint,
    },
    Burn,
};

// WSOL mint address (Wrapped SOL)
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

// -----------------------------------------------------
// ------------ REFERRAL SYSTEM HELPERS ----------------
// -----------------------------------------------------

/// Derive the canonical referral rewards PDA for a referrer.
pub fn referral_rewards_pda(referrer: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[REFERRAL_REWARDS_SEED, referrer.as_ref()], &crate::id()).0
}

/// Validate that the provided referral rewards account matches the expected PDA for `referral_code`.
pub fn validate_referrer_rewards_account<'info>(
    referral_code: &Pubkey,
    referrer_rewards: Option<&Account<'info, ReferralRewards>>,
) -> Result<()> {
    let Some(referrer_rewards) = referrer_rewards else {
        return err!(ErrorCode::ReferralRewardsAccountRequired);
    };

    require_keys_eq!(
        referrer_rewards.key(),
        referral_rewards_pda(referral_code),
        ErrorCode::InvalidReferralAccount
    );
    require_keys_eq!(
        referrer_rewards.owner,
        *referral_code,
        ErrorCode::InvalidReferralAccount
    );

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token2022TransferFeeInfo {
    pub transfer_fee_basis_points: u16,
    pub max_fee: u64,
    pub fee_amount: u64,
    pub post_fee_amount: u64,
}

pub fn get_token2022_transfer_fee_info<'info>(
    mint_account_info: &AccountInfo<'info>,
    pre_fee_amount: u64,
    epoch: u64,
) -> Result<Token2022TransferFeeInfo> {
    let mint_data = mint_account_info.try_borrow_data()?;
    let mint = StateWithExtensions::<SplToken2022Mint>::unpack(&mint_data)
        .map_err(|_| ErrorCode::InvalidMint)?;
    let transfer_fee_config = <StateWithExtensions<SplToken2022Mint> as BaseStateWithExtensions<
        SplToken2022Mint,
    >>::get_extension::<TransferFeeConfig>(&mint)
    .map_err(|_| ErrorCode::InvalidMint)?;
    let epoch_fee = transfer_fee_config.get_epoch_fee(epoch);
    let fee_amount = transfer_fee_config
        .calculate_epoch_fee(epoch, pre_fee_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let post_fee_amount = pre_fee_amount
        .checked_sub(fee_amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    Ok(Token2022TransferFeeInfo {
        transfer_fee_basis_points: u16::from(epoch_fee.transfer_fee_basis_points),
        max_fee: u64::from(epoch_fee.maximum_fee),
        fee_amount,
        post_fee_amount,
    })
}

// Helper function to transfer SOL to the program's sol_treasury PDA
pub fn transfer_to_sol_treasury<'info>(
    from: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [helper.transfer_to_sol_treasury] from={} to={} amount={} SOL",
        from.key(),
        sol_treasury.key(),
        amount as f64 / 1e9
    );
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_treasury.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   ✅ transfer_to_sol_treasury complete");
    Ok(())
}

pub fn transfer_to_autominer_custody<'info>(
    from: &AccountInfo<'info>,
    autominer_custody: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [helper.transfer_to_autominer_custody] from={} to={} amount={} SOL",
        from.key(),
        autominer_custody.key(),
        amount as f64 / 1e9
    );
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: autominer_custody.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   ✅ transfer_to_autominer_custody complete");
    Ok(())
}

/// Return the native SOL that can leave an account without dropping it below
/// its current rent-exempt floor. This is zero for the zero-data system PDAs
/// used as simple SOL vaults today, but keeping the helper generic prevents a
/// future vault shape change from introducing rent-drain failures.
pub fn native_lamports_available_after_rent(account: &AccountInfo<'_>) -> Result<u64> {
    let rent_floor = Rent::get()?.minimum_balance(account.data_len());
    Ok(account.lamports().saturating_sub(rent_floor))
}

/// Close a program-owned account manually, sending every remaining lamport to
/// `destination`, assigning the account back to System Program, and shrinking
/// data to zero. Use for manually loaded sidecar PDAs in stack-sensitive ix
/// account parsers where Anchor's `close = ...` constraint would require a
/// typed `Account<T>`.
pub fn close_owned_account<'info>(
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
) -> Result<u64> {
    require_keys_eq!(*source.owner, crate::ID, ErrorCode::InvalidAccount);
    let lamports = source.lamports();
    if lamports > 0 {
        **source.try_borrow_mut_lamports()? = 0;
        **destination.try_borrow_mut_lamports()? = destination
            .lamports()
            .checked_add(lamports)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }
    source.realloc(0, false)?;
    source.assign(&system_program::ID);
    Ok(lamports)
}

pub fn transfer_from_autominer_custody<'info>(
    autominer_custody: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    custody_bump: u8,
) -> Result<u64> {
    msg!(
        "💰 [helper.transfer_from_autominer_custody] from={} to={} amount={} SOL bump={}",
        autominer_custody.key(),
        to.key(),
        amount as f64 / 1e9,
        custody_bump
    );
    let available = native_lamports_available_after_rent(autominer_custody)?;
    let actual_amount = amount.min(available);
    if actual_amount == 0 {
        msg!("   ⚠️ autominer custody has no withdrawable SOL after rent reserve");
        return Ok(0);
    }

    let seeds = &[AUTOMINER_CUSTODY_SEED.as_ref(), &[custody_bump]];
    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: autominer_custody.to_account_info(),
                to: to.to_account_info(),
            },
            &[seeds],
        ),
        actual_amount,
    )?;
    msg!(
        "   ✅ transfer_from_autominer_custody complete actual_amount={}",
        actual_amount
    );
    Ok(actual_amount)
}

// Helper function to transfer SOL to the sol_rewards_vault PDA
pub fn transfer_to_sol_rewards_vault<'info>(
    from: &AccountInfo<'info>,
    sol_rewards_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [helper.transfer_to_sol_rewards_vault] from={} to={} amount={} SOL",
        from.key(),
        sol_rewards_vault.key(),
        amount as f64 / 1e9
    );
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_rewards_vault.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   ✅ transfer_to_sol_rewards_vault complete");
    Ok(())
}

// Helper function to transfer SOL to the sol_prize_pot_vault PDA
pub fn transfer_to_sol_prize_pot_vault<'info>(
    from: &AccountInfo<'info>,
    sol_prize_pot_vault: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [helper.transfer_to_sol_prize_pot_vault] from={} to={} amount={} SOL",
        from.key(),
        sol_prize_pot_vault.key(),
        amount as f64 / 1e9
    );
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_prize_pot_vault.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   ✅ transfer_to_sol_prize_pot_vault complete");
    Ok(())
}

/// Like `init_pda_account_if_needed` but writes only the discriminator and
/// relies on `system_program::create_account` zero-filling the rest of the
/// account data. Use this when the desired initial state is "all zeros after
/// the discriminator" — saves the caller from materializing a `T` on the
/// stack just to serialize zeros.
///
/// Specifically used for large account types (e.g. `FactionWarState`) where
/// passing `&T` would push the calling function over BPF's 4096-byte stack
/// budget.
#[inline(never)]
pub fn init_pda_account_zeroed_if_needed<'info, T>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[u8]],
    space: usize,
) -> Result<bool>
where
    T: Discriminator,
{
    require!(account.is_writable, ErrorCode::InvalidAccount);

    if account.owner == &system_program::ID {
        require!(account.data_len() == 0, ErrorCode::InvalidAccount);
        let rent = Rent::get()?.minimum_balance(space);
        if account.lamports() == 0 {
            msg!(
                "   creating zeroed PDA account {} with rent={} bytes={}",
                account.key(),
                rent,
                space
            );
            create_account(
                CpiContext::new_with_signer(
                    system_program.to_account_info(),
                    CreateAccount {
                        from: payer.to_account_info(),
                        to: account.to_account_info(),
                    },
                    &[signer_seeds],
                ),
                rent,
                space as u64,
                &crate::ID,
            )?;
        } else {
            claim_prefunded_system_pda(payer, account, system_program, signer_seeds, space, rent)?;
        }
        let mut data = account.try_borrow_mut_data()?;
        require!(data.len() >= 8, ErrorCode::InvalidAccount);
        data[..8].copy_from_slice(T::DISCRIMINATOR);
        return Ok(true);
    }
    Ok(false)
}

pub fn init_pda_account_if_needed<'info, T>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[u8]],
    space: usize,
    initial_data: &T,
) -> Result<bool>
where
    T: AccountSerialize + AccountDeserialize + Discriminator + Owner + Clone,
{
    require!(account.is_writable, ErrorCode::InvalidAccount);

    msg!(
        "🧱 [init_pda_account_if_needed] account={} writable={} owner={} lamports={} data_len={} target_space={}",
        account.key(),
        account.is_writable,
        account.owner,
        account.lamports(),
        account.data_len(),
        space
    );

    if account.owner == &system_program::ID {
        require!(account.data_len() == 0, ErrorCode::InvalidAccount);
        let rent = Rent::get()?.minimum_balance(space);
        if account.lamports() == 0 {
            msg!(
                "   creating PDA account {} with rent={} bytes={}",
                account.key(),
                rent,
                space
            );
            create_account(
                CpiContext::new_with_signer(
                    system_program.to_account_info(),
                    CreateAccount {
                        from: payer.to_account_info(),
                        to: account.to_account_info(),
                    },
                    &[signer_seeds],
                ),
                rent,
                space as u64,
                &crate::ID,
            )?;
        } else {
            claim_prefunded_system_pda(payer, account, system_program, signer_seeds, space, rent)?;
        }

        let mut data = account.try_borrow_mut_data()?;
        require!(data.len() >= space, ErrorCode::InvalidAccount);
        let mut cursor = &mut data[..];
        initial_data.try_serialize(&mut cursor)?;
        msg!("   ✅ PDA account {} initialized", account.key());
        return Ok(true);
    }

    msg!(
        "   ↪️ PDA account {} already initialized; owner={} lamports={} data_len={}",
        account.key(),
        account.owner,
        account.lamports(),
        account.data_len()
    );

    Ok(false)
}

fn claim_prefunded_system_pda<'info>(
    payer: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[u8]],
    space: usize,
    rent: u64,
) -> Result<()> {
    require!(
        account.owner == &system_program::ID,
        ErrorCode::InvalidAccount
    );
    require!(account.data_len() == 0, ErrorCode::InvalidAccount);

    let current_lamports = account.lamports();
    if current_lamports < rent {
        let top_up = rent
            .checked_sub(current_lamports)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        msg!(
            "   topping up prefunded PDA {} by {} lamports before allocation",
            account.key(),
            top_up
        );
        transfer(
            CpiContext::new(
                system_program.to_account_info(),
                Transfer {
                    from: payer.to_account_info(),
                    to: account.to_account_info(),
                },
            ),
            top_up,
        )?;
    }

    msg!(
        "   claiming prefunded PDA {} with existing_lamports={} bytes={}",
        account.key(),
        current_lamports,
        space
    );
    let signer_groups: &[&[&[u8]]] = &[signer_seeds];
    invoke_signed(
        &system_instruction::allocate(account.key, space as u64),
        &[account.to_account_info()],
        signer_groups,
    )?;
    invoke_signed(
        &system_instruction::assign(account.key, &crate::ID),
        &[account.to_account_info()],
        signer_groups,
    )?;

    Ok(())
}

pub fn load_account_data<'info, T>(account: &AccountInfo<'info>) -> Result<T>
where
    T: AccountDeserialize + Owner,
{
    require!(account.owner == &T::owner(), ErrorCode::InvalidAccount);
    msg!(
        "📥 [load_account_data] account={} owner={} data_len={}",
        account.key(),
        account.owner,
        account.data_len()
    );
    let data = account.try_borrow_data()?;
    let mut data_slice: &[u8] = &data;
    T::try_deserialize(&mut data_slice)
}

/// Boxed wrapper around `load_account_data`. Marked `#[inline(never)]` so the
/// `T`-sized stack temporary lives only in this helper's frame instead of the
/// caller's. Suitable for SMALL accounts only — for accounts whose `T` itself
/// exceeds BPF's 4096-byte stack budget (e.g. `FactionWarState`), the helper's
/// own frame would still overflow. Use a specialized field-by-field loader for
/// those.
#[inline(never)]
pub fn load_account_data_boxed<'info, T>(account: &AccountInfo<'info>) -> Result<Box<T>>
where
    T: AccountDeserialize + Owner,
{
    Ok(Box::new(load_account_data::<T>(account)?))
}

/// Allocate a zero-filled `Box<T>` directly on the heap without ever
/// materializing the `T` on the stack. Used by the FactionWarState boxed
/// loader to avoid blowing BPF's stack budget on a 2.6KB struct.
///
/// # Safety
///
/// Caller must guarantee `T` has no invariants violated by an all-zeros
/// bit pattern: no enums with non-zero discriminants, no references, no
/// `NonZero*` types, no custom `Drop`. Suitable for plain numeric
/// primitives, fixed-size arrays of those, and `Pubkey = [u8; 32]`.
#[inline(never)]
pub unsafe fn alloc_zeroed_boxed<T>() -> Box<T> {
    let b: Box<core::mem::MaybeUninit<T>> = Box::new(core::mem::MaybeUninit::uninit());
    let raw = Box::into_raw(b);
    core::ptr::write_bytes(raw as *mut u8, 0u8, core::mem::size_of::<T>());
    Box::from_raw(raw as *mut T)
}

pub fn store_account_data<'info, T>(account: &AccountInfo<'info>, value: &T) -> Result<()>
where
    T: AccountSerialize + Discriminator,
{
    require!(account.is_writable, ErrorCode::InvalidAccount);
    let mut data = account.try_borrow_mut_data()?;
    require!(data.len() >= DISCRIMINATOR_SIZE, ErrorCode::InvalidAccount);
    msg!(
        "📦 [store_account_data] account={} writable={} data_len={}",
        account.key(),
        account.is_writable,
        data.len()
    );
    let mut cursor = &mut data[..];
    value.try_serialize(&mut cursor)?;
    msg!("   ✅ account {} state persisted", account.key());
    Ok(())
}

// Helper function to transfer SOL FROM sol_rewards_vault to a user
// Uses PDA signer to authorize the transfer from the System Program-owned account
pub fn transfer_from_sol_rewards_vault<'info>(
    sol_rewards_vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    vault_bump: u8,
) -> Result<u64> {
    msg!(
        "💰 [helper.transfer_from_sol_rewards_vault] from={} to={} amount={} SOL bump={}",
        sol_rewards_vault.key(),
        to.key(),
        amount as f64 / 1e9,
        vault_bump
    );
    let available = native_lamports_available_after_rent(sol_rewards_vault)?;
    let actual_amount = amount.min(available);
    if actual_amount == 0 {
        msg!("   ⚠️ sol_rewards_vault has no withdrawable SOL after rent reserve");
        return Ok(0);
    }

    let seeds = &[STAKER_SOL_REWARD_VAULT_SEED.as_ref(), &[vault_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: sol_rewards_vault.to_account_info(),
                to: to.to_account_info(),
            },
            signer_seeds,
        ),
        actual_amount,
    )?;
    msg!(
        "   ✅ transfer_from_sol_rewards_vault complete actual_amount={}",
        actual_amount
    );
    Ok(actual_amount)
}

// Helper function to transfer SOL FROM sol_prize_pot_vault to a user
// Uses PDA signer to authorize the transfer from the System Program-owned account
pub fn transfer_from_sol_prize_pot_vault<'info>(
    sol_prize_pot_vault: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
    vault_bump: u8,
) -> Result<u64> {
    msg!(
        "💰 [helper.transfer_from_sol_prize_pot_vault] from={} to={} amount={} SOL bump={}",
        sol_prize_pot_vault.key(),
        to.key(),
        amount as f64 / 1e9,
        vault_bump
    );

    let available = native_lamports_available_after_rent(sol_prize_pot_vault)?;
    let actual_amount = amount.min(available);
    if actual_amount < amount {
        msg!(
            "   ⚠️ Capping prize-pot payout: requested={} actual={} available_after_rent={}",
            amount,
            actual_amount,
            available
        );
    }
    if actual_amount == 0 {
        return Ok(0);
    }

    let seeds = &[JACKPOT_POT_VAULT_SEED.as_ref(), &[vault_bump]];
    let signer_seeds = &[&seeds[..]];

    transfer(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            Transfer {
                from: sol_prize_pot_vault.to_account_info(),
                to: to.to_account_info(),
            },
            signer_seeds,
        ),
        actual_amount,
    )?;
    msg!(
        "   ✅ transfer_from_sol_prize_pot_vault complete actual_amount={}",
        actual_amount
    );
    Ok(actual_amount)
}

// Helper function to transfer WSOL to multisig token account
// Wraps SOL to WSOL first, then transfers WSOL
pub fn transfer_wsol_to_multisig<'info>(
    from: &AccountInfo<'info>,
    multisig_wsol_account: &AccountInfo<'info>,
    from_wsol_account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    msg!(
        "💰 [helper.transfer_wsol_to_multisig] amount={} SOL",
        amount as f64 / 1e9
    );
    // Step 1: Transfer SOL to the from_wsol_account (this wraps it)
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: from_wsol_account.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   Step 1: SOL wrapped to WSOL");

    // Step 2: Sync native account to update WSOL balance
    anchor_spl::token::sync_native(CpiContext::new(
        token_program.to_account_info(),
        anchor_spl::token::SyncNative {
            account: from_wsol_account.to_account_info(),
        },
    ))?;
    msg!("   Step 2: WSOL balance synced");

    // Step 3: Transfer WSOL from from_wsol_account to multisig_wsol_account
    anchor_spl::token::transfer(
        CpiContext::new(
            token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: from_wsol_account.to_account_info(),
                to: multisig_wsol_account.to_account_info(),
                authority: from.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("   Step 3: WSOL transferred to multisig");
    msg!("   ✅ transfer_wsol_to_multisig complete");
    Ok(())
}

// Helper function to calculate multiplier
pub fn calculate_multiplier(
    lockup_duration: u64,
    min_lockup: u64,
    max_lockup: u64,
    base_multiplier: u16,
    max_multiplier: u16,
) -> Result<u16> {
    require!(min_lockup <= max_lockup, ErrorCode::InvalidParameters);
    require!(
        base_multiplier >= M_HUNDRED as u16
            && max_multiplier >= base_multiplier
            && max_multiplier <= 300,
        ErrorCode::InvalidParameters
    );
    require!(
        lockup_duration >= min_lockup && lockup_duration <= max_lockup,
        ErrorCode::InvalidParameters
    );

    let duration_range = max_lockup
        .checked_sub(min_lockup)
        .ok_or(ErrorCode::InvalidParameters)?;
    let multiplier_range = max_multiplier
        .checked_sub(base_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    // Guard: if min == max lockup, return base multiplier (avoid div by zero)
    if duration_range == 0 {
        return Ok(base_multiplier);
    }

    let duration_above_min = lockup_duration
        .checked_sub(min_lockup)
        .ok_or(ErrorCode::InvalidParameters)?;

    let multiplier_increase_u128 = (duration_above_min as u128)
        .checked_mul(multiplier_range as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(duration_range as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let multiplier_increase =
        u16::try_from(multiplier_increase_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;

    base_multiplier
        .checked_add(multiplier_increase)
        .filter(|value| *value <= max_multiplier)
        .ok_or(ErrorCode::ArithmeticOverflow.into())
}

/// Add position index to user's degenBTC positions
pub fn add_degenbtc_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    msg!(
        "🔍 [add_degenbtc_position] Adding position index: {}",
        position_index
    );
    msg!(
        "🔍 [add_degenbtc_position] MAX_ALLOWED_POSITIONS: {}",
        MAX_ALLOWED_POSITIONS
    );

    if position_index >= MAX_ALLOWED_POSITIONS {
        return Err(ErrorCode::InvalidParameters.into());
    }

    // If this position index is not already active
    if !player_ac
        .degenbtc_position_indices
        .contains(&position_index)
    {
        msg!("🔍 [add_degenbtc_position] Position index is not already active");
        // Ensure we're not exceeding the max allowed positions
        if player_ac.degenbtc_position_indices.len() >= MAX_ALLOWED_POSITIONS as usize {
            msg!("🔍 [add_degenbtc_position] Exceeding max allowed positions");
            return Err(ErrorCode::InvalidParameters.into());
        }
        player_ac.degenbtc_position_indices.push(position_index);
        msg!(
            "🔍 [add_degenbtc_position] Position index added: {}",
            position_index
        );
    }

    Ok(())
}

/// Remove position index from user's degenBTC positions
pub fn remove_degenbtc_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    crate::log_fn!("helper", "remove_degenbtc_position");
    msg!(
        "🔧 [helper.remove_degenbtc_position] position_index={} current_indices={:?}",
        position_index,
        player_ac.degenbtc_position_indices
    );
    // Find the position index in the vector
    if let Some(pos) = player_ac
        .degenbtc_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        player_ac.degenbtc_position_indices.remove(pos);
        msg!(
            "   ✅ removed position_index={} new_indices={:?}",
            position_index,
            player_ac.degenbtc_position_indices
        );
    } else {
        msg!("   ⚠️ position_index={} not found", position_index);
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(())
}

/// Add position index to user's LP positions
pub fn add_lp_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    crate::log_fn!("helper", "add_lp_position");
    msg!(
        "🔧 [helper.add_lp_position] position_index={} current_indices={:?}",
        position_index,
        player_ac.lp_position_indices
    );
    if position_index >= MAX_ALLOWED_POSITIONS {
        msg!("   ⚠️ position_index >= MAX_ALLOWED_POSITIONS");
        return Err(ErrorCode::InvalidParameters.into());
    }

    // If this position index is not already active
    if !player_ac.lp_position_indices.contains(&position_index) {
        if player_ac.lp_position_indices.len() >= MAX_ALLOWED_POSITIONS as usize {
            msg!("   ⚠️ max positions reached");
            return Err(ErrorCode::InvalidParameters.into());
        }
        player_ac.lp_position_indices.push(position_index);
        msg!(
            "   ✅ added position_index={} new_indices={:?}",
            position_index,
            player_ac.lp_position_indices
        );
    } else {
        msg!("   position_index={} already active", position_index);
    }

    Ok(())
}

/// Remove position index from user's LP positions
pub fn remove_lp_position(player_ac: &mut PlayerData, position_index: u8) -> Result<()> {
    crate::log_fn!("helper", "remove_lp_position");
    msg!(
        "🔧 [helper.remove_lp_position] position_index={} current_indices={:?}",
        position_index,
        player_ac.lp_position_indices
    );
    if let Some(pos) = player_ac
        .lp_position_indices
        .iter()
        .position(|&x| x == position_index)
    {
        player_ac.lp_position_indices.remove(pos);
        msg!(
            "   ✅ removed position_index={} new_indices={:?}",
            position_index,
            player_ac.lp_position_indices
        );
    } else {
        msg!("   ⚠️ position_index={} not found", position_index);
        return Err(ErrorCode::InvalidParameters.into());
    }

    Ok(())
}

pub fn calculate_staking_rewards(
    user_weighted_amt: u64,
    accumulated_sol_per_point: u128,
    reward_debt: u128,
) -> Result<u64> {
    crate::log_fn!("helper", "calculate_staking_rewards");
    msg!(
        "📊 [helper.calculate_staking_rewards] user_weighted_amt={} accumulated={} reward_debt={}",
        user_weighted_amt,
        accumulated_sol_per_point,
        reward_debt
    );
    let reward_diff = accumulated_sol_per_point
        .checked_sub(reward_debt)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let new_rewards = mul_div_u128(
        user_weighted_amt as u128,
        reward_diff,
        INDEX_PRECISION as u128,
    )?;
    let result = u64::try_from(new_rewards).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   reward_diff={} new_rewards={} result={}",
        reward_diff,
        new_rewards,
        result
    );
    Ok(result)
}

pub fn mul_div(a: u64, b: u64, c: u64) -> Result<u128> {
    crate::log_fn!("helper", "mul_div");
    msg!("📊 [helper.mul_div] a={} b={} c={}", a, b, c);
    let result = (a as u128)
        .checked_mul(b as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(c as u128)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   result={}", result);
    Ok(result)
}

pub fn mul_div_u128(a: u128, b: u128, c: u128) -> Result<u128> {
    crate::log_fn!("helper", "mul_div_u128");
    msg!("📊 [helper.mul_div_u128] a={} b={} c={}", a, b, c);
    let result = a
        .checked_mul(b)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(c)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   result={}", result);
    Ok(result)
}

pub fn init_position(
    position: &mut StakedPosition,
    position_type: u8,
    faction_id: u8,
    position_index: u8,
    staked_amount: u64,
    weighted_amount: u64,
    lockup_duration: u64,
    current_ts: i64,
    multiplier: u16,
    bump: u8,
) -> Result<()> {
    crate::log_fn!("helper", "init_position");
    msg!(
        "🔧 [helper.init_position] type={} faction={} index={} staked={} weighted={} lockup={}d mult={}",
        position_type,
        faction_id,
        position_index,
        staked_amount,
        weighted_amount,
        lockup_duration,
        multiplier
    );
    position.position_index = position_index;
    position.position_type = position_type;

    position.faction_id = faction_id;
    position.staked_amount = staked_amount;
    position.weighted_amount = weighted_amount;

    position.lockup_duration = lockup_duration;
    position.start_timestamp = current_ts;
    position.multiplier = multiplier;
    position.bump = bump;

    let seconds_to_add = lockup_duration
        .checked_mul(DAY_IN_SECONDS)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let seconds_to_add_i64 =
        i64::try_from(seconds_to_add).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    position.lockup_end_timestamp = current_ts
        .checked_add(seconds_to_add_i64)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!(
        "   lockup_end_ts={} (current={} + {}s)",
        position.lockup_end_timestamp,
        current_ts,
        seconds_to_add
    );

    Ok(())
}

/// Add gameplay rewards to total claimable and pending rewards.
pub fn add_to_total_claimable(
    unrefined_dbtc: &mut HodlPool,
    player_data: &mut PlayerData,
    dbtc_rewards: u64,
    user: Pubkey,
    player_data_key: Pubkey,
    source: u8,
    reference_id: u64,
) -> Result<u64> {
    // Calculate extra degenBtc rewards from the HODL tax. The global hodl_tax_index
    // is monotonically non-decreasing, so checked_sub should never fail — but we
    // validate it to surface any state corruption rather than panicking.
    let index_dif = unrefined_dbtc
        .hodl_tax_index
        .checked_sub(player_data.hodl_tax_index)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let accrued_u128 = mul_div_u128(
        player_data.pending_dbtc_rewards as u128,
        index_dif,
        INDEX_PRECISION as u128,
    )?;
    let accrued_rewards = u64::try_from(accrued_u128).map_err(|_| ErrorCode::ArithmeticOverflow)?;
    msg!("     Accrued degenBTC rewards: {}", accrued_rewards);

    let total_new = dbtc_rewards
        .checked_add(accrued_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    unrefined_dbtc.total_dbtc_claimable = unrefined_dbtc
        .total_dbtc_claimable
        .checked_add(total_new)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.hodl_tax_index = unrefined_dbtc.hodl_tax_index;
    player_data.pending_dbtc_rewards = player_data
        .pending_dbtc_rewards
        .checked_add(total_new)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    player_data.unrefined_dbtc_rewards = player_data
        .unrefined_dbtc_rewards
        .checked_add(accrued_rewards)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    if total_new > 0 {
        emit!(crate::events::MinebtcClaimableAccrued {
            user,
            player_data: player_data_key,
            source,
            reference_id,
            source_amount: dbtc_rewards,
            unrefined_bonus_amount: accrued_rewards,
            total_added: total_new,
            pending_dbtc_after: player_data.pending_dbtc_rewards,
            total_claimable_after: unrefined_dbtc.total_dbtc_claimable,
            timestamp: Clock::get()?.unix_timestamp,
        });
    }

    Ok(accrued_rewards)
}

pub fn calculate_emergency_tax(
    user_position: &StakedPosition,
    current_ts: i64,
    emergency_tax: u64,
) -> Result<u64> {
    let total_lockup_seconds = user_position
        .lockup_end_timestamp
        .checked_sub(user_position.start_timestamp)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    let remaining_seconds = user_position
        .lockup_end_timestamp
        .checked_sub(current_ts)
        .unwrap_or(0);

    // Guard: if lockup already expired, no penalty (remaining <= 0)
    if remaining_seconds <= 0 || total_lockup_seconds <= 0 {
        msg!("   Lockup expired or invalid — no penalty");
        return Ok(0);
    }

    let remaining_seconds_pct = (M_HUNDRED as i64)
        .checked_mul(remaining_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(total_lockup_seconds)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    msg!("   Lockup remaining: {}%", remaining_seconds_pct);

    // remaining_seconds_pct is guaranteed positive here, safe to cast
    let calc_penalty_pct = u64::try_from(mul_div(
        emergency_tax,
        remaining_seconds_pct as u64,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow)?;
    u64::try_from(mul_div(
        user_position.staked_amount,
        calc_penalty_pct,
        M_HUNDRED,
    )?)
    .map_err(|_| ErrorCode::ArithmeticOverflow.into())
}

/// Charge emergency tax for MINEBTC tokens by burning the full penalty amount.
/// This function handles the penalty for early withdrawal from MINEBTC staking positions.
pub fn charge_emergency_tax<'info>(
    dbtc_custodian: &AccountInfo<'info>,
    dbtc_custodian_authority: &AccountInfo<'info>,
    degenbtc_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    custodian_authority_bump: u8,
    penalty_amount: u64,
) -> Result<()> {
    msg!("💰 [charge_emergency_tax] Processing MINEBTC emergency tax");
    msg!("   Penalty amount: {} tokens", penalty_amount as f64 / 1e6);

    // Burn the full penalty amount from the custodian.
    if penalty_amount > 0 {
        msg!(
            "   Burning {} tokens from custodian...",
            penalty_amount as f64 / 1e6
        );

        // Get PDA signer seeds for the dbtc_custodian authority (global, no faction_id)
        let custodian_authority_seeds = &[
            DEGENBTC_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[custodian_authority_bump],
        ];
        let custodian_signer = &[&custodian_authority_seeds[..]];

        // Use Token-2022 burn instruction
        token_2022::burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Burn {
                    mint: degenbtc_mint.to_account_info(),
                    from: dbtc_custodian.to_account_info(),
                    authority: dbtc_custodian_authority.to_account_info(),
                },
                custodian_signer,
            ),
            penalty_amount,
        )?;
        msg!("   ✅ Burned {} tokens", penalty_amount as f64 / 1e6);
    }

    msg!("   ✅ Emergency tax charged successfully");

    Ok(())
}

/// Charge emergency tax for LP tokens: 100% burned
/// This function handles the penalty for early withdrawal from LP staking positions
/// LP tokens are fully burned with no rewards to stakers
pub fn charge_lp_emergency_tax<'info>(
    liquidity_custodian: &AccountInfo<'info>,
    liquidity_custodian_authority: &AccountInfo<'info>,
    lp_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    custodian_authority_bump: u8,
    penalty_amount: u64,
) -> Result<()> {
    msg!("💰 [charge_lp_emergency_tax] Processing LP emergency tax");
    msg!("   Penalty amount: {} tokens", penalty_amount as f64 / 1e6);
    msg!("   Burning 100% of penalty (no rewards to stakers)");

    if penalty_amount > 0 {
        // Get PDA signer seeds for the liquidity_custodian authority (global)
        let custodian_authority_seeds = &[
            LIQUIDITY_CUSTODIAN_AUTHORITY_SEED.as_ref(),
            &[custodian_authority_bump],
        ];
        let custodian_signer = &[&custodian_authority_seeds[..]];

        // Burn 100% of penalty from custodian (standard SPL Token burn)
        token_standard::burn(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                StandardBurn {
                    mint: lp_mint.to_account_info(),
                    from: liquidity_custodian.to_account_info(),
                    authority: liquidity_custodian_authority.to_account_info(),
                },
                custodian_signer,
            ),
            penalty_amount,
        )?;
        msg!("   ✅ Burned {} LP tokens", penalty_amount as f64 / 1e6);
    }

    msg!("   ✅ LP emergency tax charged successfully");

    Ok(())
}

/// Calculate number of tickets given minting price
pub fn calc_tickets_count(total_price: u64, ticket_value: u64) -> u64 {
    if ticket_value == 0 {
        return 0;
    }
    // 1.0x: users get 100% of their mint price as tickets
    total_price / ticket_value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_player_data() -> PlayerData {
        PlayerData {
            bump: 0,
            owner: Pubkey::default(),
            referral_code: Pubkey::default(),
            faction_id: 0,
            origin_faction_id: 0,
            referrer_faction_id: u8::MAX,
            degenbtc_hashpower: 0,
            degenbtc_staked: 0,
            degenbtc_degenbtc_reward_debt: 0,
            degenbtc_sol_reward_debt: 0,
            lp_hashpower: 0,
            lp_staked: 0,
            lp_sol_reward_debt: 0,
            lp_degenbtc_reward_debt: 0,
            pending_sol_rewards: 0,
            hodl_tax_index: 0,
            pending_dbtc_rewards: 0,
            pending_staking_dbtc_rewards: 0,
            unrefined_dbtc_rewards: 0,
            pending_round_claims: 0,
            pending_war_claims: 0,
            degenbtc_position_indices: Vec::new(),
            lp_position_indices: Vec::new(),
            staked_hashbeasts: Vec::new(),
            hashbeast_multiplier: BASE_MULTIPLIER as u16,
            free_tickets: Vec::new(),
            free_tickets_remaining: Vec::new(),
            gameplay_hashbeast: Pubkey::default(),
            active_multiplier: BASE_MULTIPLIER,
            gameplay_hashbeast_dna: [0u8; 32],
            gameplay_hashbeast_xp: 0,
            gameplay_unlock_request_faction_war: 0,
            current_war_score: 0,
            current_war_score_cycle_id: 0,
        }
    }

    fn blank_staked_position() -> StakedPosition {
        StakedPosition {
            position_type: 0,
            position_index: 0,
            faction_id: 0,
            staked_amount: 0,
            weighted_amount: 0,
            start_timestamp: 0,
            lockup_end_timestamp: 0,
            lockup_duration: 0,
            multiplier: 100,
            bump: 0,
        }
    }

    // ------------------------------------------------------------------------
    // calculate_multiplier
    // ------------------------------------------------------------------------

    #[test]
    fn lockup_multiplier_cannot_exceed_three_x() {
        let multiplier = calculate_multiplier(90, 7, 90, 100, 300).unwrap();
        assert_eq!(multiplier, 300);
        assert!(calculate_multiplier(91, 7, 90, 100, 300).is_err());
        assert!(calculate_multiplier(90, 7, 90, 100, 301).is_err());
    }

    #[test]
    fn lockup_multiplier_min_returns_base() {
        let m = calculate_multiplier(7, 7, 90, 100, 300).unwrap();
        assert_eq!(m, 100);
    }

    #[test]
    fn lockup_multiplier_midpoint() {
        let m = calculate_multiplier(48, 7, 90, 100, 300).unwrap();
        // 48 is halfway between 7 and 90
        // multiplier_range = 200, duration_above_min = 41, duration_range = 83
        // increase = 41 * 200 / 83 = 98 (truncated)
        assert_eq!(m, 198);
    }

    #[test]
    fn lockup_multiplier_zero_duration_range() {
        let m = calculate_multiplier(30, 30, 30, 150, 300).unwrap();
        assert_eq!(m, 150);
    }

    #[test]
    fn lockup_multiplier_invalid_bounds() {
        assert!(calculate_multiplier(5, 10, 20, 100, 300).is_err()); // lockup < min
        assert!(calculate_multiplier(25, 10, 20, 100, 300).is_err()); // lockup > max
        assert!(calculate_multiplier(15, 10, 20, 50, 300).is_err()); // base < 100
        assert!(calculate_multiplier(15, 10, 20, 100, 50).is_err()); // max < base
    }

    // ------------------------------------------------------------------------
    // mul_div / mul_div_u128
    // ------------------------------------------------------------------------

    #[test]
    fn mul_div_basic() {
        assert_eq!(mul_div(100, 50, 100).unwrap(), 50);
        assert_eq!(mul_div(0, 50, 100).unwrap(), 0);
    }

    #[test]
    fn mul_div_large_numbers() {
        assert_eq!(
            mul_div(u64::MAX, u64::MAX, u64::MAX).unwrap(),
            u64::MAX as u128
        );
    }

    #[test]
    fn mul_div_division_by_zero_errors() {
        assert!(mul_div(100, 50, 0).is_err());
    }

    #[test]
    fn mul_div_u128_basic() {
        assert_eq!(mul_div_u128(100, 50, 100).unwrap(), 50);
        assert_eq!(mul_div_u128(0, 50, 100).unwrap(), 0);
    }

    #[test]
    fn mul_div_u128_overflow_protection() {
        assert!(mul_div_u128(u128::MAX, 2, 1).is_err());
    }

    // ------------------------------------------------------------------------
    // calc_tickets_count
    // ------------------------------------------------------------------------

    #[test]
    fn tickets_count_basic() {
        assert_eq!(calc_tickets_count(1_000_000, 100_000), 10);
        assert_eq!(calc_tickets_count(1_000_000, 300_000), 3);
    }

    #[test]
    fn tickets_count_zero_ticket_value() {
        assert_eq!(calc_tickets_count(1_000_000, 0), 0);
    }

    // ------------------------------------------------------------------------
    // add/remove degenbtc position
    // ------------------------------------------------------------------------

    #[test]
    fn add_degenbtc_position_success() {
        let mut player = blank_player_data();
        add_degenbtc_position(&mut player, 3).unwrap();
        assert!(player.degenbtc_position_indices.contains(&3));
    }

    #[test]
    fn add_degenbtc_position_duplicate_is_noop() {
        let mut player = blank_player_data();
        add_degenbtc_position(&mut player, 3).unwrap();
        add_degenbtc_position(&mut player, 3).unwrap();
        assert_eq!(player.degenbtc_position_indices.len(), 1);
    }

    #[test]
    fn add_degenbtc_position_invalid_index_fails() {
        let mut player = blank_player_data();
        assert!(add_degenbtc_position(&mut player, MAX_ALLOWED_POSITIONS).is_err());
    }

    #[test]
    fn remove_degenbtc_position_success() {
        let mut player = blank_player_data();
        add_degenbtc_position(&mut player, 2).unwrap();
        add_degenbtc_position(&mut player, 5).unwrap();
        remove_degenbtc_position(&mut player, 2).unwrap();
        assert!(!player.degenbtc_position_indices.contains(&2));
        assert!(player.degenbtc_position_indices.contains(&5));
    }

    #[test]
    fn remove_degenbtc_position_missing_fails() {
        let mut player = blank_player_data();
        assert!(remove_degenbtc_position(&mut player, 2).is_err());
    }

    // ------------------------------------------------------------------------
    // add/remove lp position
    // ------------------------------------------------------------------------

    #[test]
    fn add_lp_position_success() {
        let mut player = blank_player_data();
        add_lp_position(&mut player, 4).unwrap();
        assert!(player.lp_position_indices.contains(&4));
    }

    #[test]
    fn add_lp_position_duplicate_is_noop() {
        let mut player = blank_player_data();
        add_lp_position(&mut player, 4).unwrap();
        add_lp_position(&mut player, 4).unwrap();
        assert_eq!(player.lp_position_indices.len(), 1);
    }

    #[test]
    fn remove_lp_position_success() {
        let mut player = blank_player_data();
        add_lp_position(&mut player, 1).unwrap();
        remove_lp_position(&mut player, 1).unwrap();
        assert!(player.lp_position_indices.is_empty());
    }

    #[test]
    fn remove_lp_position_missing_fails() {
        let mut player = blank_player_data();
        assert!(remove_lp_position(&mut player, 1).is_err());
    }

    // ------------------------------------------------------------------------
    // init_position
    // ------------------------------------------------------------------------

    #[test]
    fn init_position_sets_fields() {
        let mut pos = blank_staked_position();
        init_position(
            &mut pos, 0, 2, 5, 1_000_000, 2_000_000, 30, 1_000_000, 250, 42,
        )
        .unwrap();

        assert_eq!(pos.position_type, 0);
        assert_eq!(pos.faction_id, 2);
        assert_eq!(pos.position_index, 5);
        assert_eq!(pos.staked_amount, 1_000_000);
        assert_eq!(pos.weighted_amount, 2_000_000);
        assert_eq!(pos.lockup_duration, 30);
        assert_eq!(pos.start_timestamp, 1_000_000);
        assert_eq!(
            pos.lockup_end_timestamp,
            1_000_000 + (30 * DAY_IN_SECONDS as i64)
        );
        assert_eq!(pos.multiplier, 250);
        assert_eq!(pos.bump, 42);
    }

    #[test]
    fn init_position_overflow_fails() {
        let mut pos = blank_staked_position();
        assert!(init_position(&mut pos, 0, 0, 0, 0, 0, u64::MAX, 0, 100, 0).is_err());
    }

    // ------------------------------------------------------------------------
    // calculate_staking_rewards
    // ------------------------------------------------------------------------

    #[test]
    fn staking_rewards_basic() {
        let rewards = calculate_staking_rewards(1_000_000, 2_000_000, 1_000_000).unwrap();
        // reward_diff = 1_000_000
        // new_rewards = 1_000_000 * 1_000_000 / 1_000_000 = 1_000_000
        assert_eq!(rewards, 1_000_000);
    }

    #[test]
    fn staking_rewards_zero_weight() {
        let rewards = calculate_staking_rewards(0, 2_000_000, 1_000_000).unwrap();
        assert_eq!(rewards, 0);
    }

    #[test]
    fn staking_rewards_debt_greater_than_accumulated_errors() {
        assert!(calculate_staking_rewards(1_000_000, 500_000, 1_000_000).is_err());
    }

    // ------------------------------------------------------------------------
    // calculate_emergency_tax
    // ------------------------------------------------------------------------

    #[test]
    fn emergency_tax_full_remaining() {
        let pos = StakedPosition {
            start_timestamp: 0,
            lockup_end_timestamp: 100,
            staked_amount: 1_000_000,
            position_type: 0,
            position_index: 0,
            faction_id: 0,
            weighted_amount: 0,
            lockup_duration: 0,
            multiplier: 100,
            bump: 0,
        };
        let tax = calculate_emergency_tax(&pos, 0, 100).unwrap();
        assert_eq!(tax, 1_000_000); // 100% penalty
    }

    #[test]
    fn emergency_tax_half_remaining() {
        let pos = StakedPosition {
            start_timestamp: 0,
            lockup_end_timestamp: 100,
            staked_amount: 1_000_000,
            position_type: 0,
            position_index: 0,
            faction_id: 0,
            weighted_amount: 0,
            lockup_duration: 0,
            multiplier: 100,
            bump: 0,
        };
        let tax = calculate_emergency_tax(&pos, 50, 100).unwrap();
        // remaining = 50, total = 100 → 50%
        // penalty_pct = 100 * 50 / 100 = 50
        // tax = 1_000_000 * 50 / 100 = 500_000
        assert_eq!(tax, 500_000);
    }

    #[test]
    fn emergency_tax_expired_lockup() {
        let pos = StakedPosition {
            start_timestamp: 0,
            lockup_end_timestamp: 100,
            staked_amount: 1_000_000,
            position_type: 0,
            position_index: 0,
            faction_id: 0,
            weighted_amount: 0,
            lockup_duration: 0,
            multiplier: 100,
            bump: 0,
        };
        let tax = calculate_emergency_tax(&pos, 101, 100).unwrap();
        assert_eq!(tax, 0);
    }

    #[test]
    fn emergency_tax_zero_lockup() {
        let pos = StakedPosition {
            start_timestamp: 100,
            lockup_end_timestamp: 100,
            staked_amount: 1_000_000,
            position_type: 0,
            position_index: 0,
            faction_id: 0,
            weighted_amount: 0,
            lockup_duration: 0,
            multiplier: 100,
            bump: 0,
        };
        let tax = calculate_emergency_tax(&pos, 50, 100).unwrap();
        assert_eq!(tax, 0);
    }

    // ------------------------------------------------------------------------
    // referral_rewards_pda
    // ------------------------------------------------------------------------

    #[test]
    fn referral_rewards_pda_is_deterministic() {
        let pk = Pubkey::new_from_array([1u8; 32]);
        let pda1 = referral_rewards_pda(&pk);
        let pda2 = referral_rewards_pda(&pk);
        assert_eq!(pda1, pda2);
    }
}
