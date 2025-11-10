use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, transfer, Transfer};

use crate::errors::ErrorCode;
use crate::events::*;
use crate::state::*;

// -----------------------------------------------------
// ------------ REFERRAL SYSTEM HELPERS ----------------
// -----------------------------------------------------

// Helper function to transfer SOL to the program's sol_treasury PDA
pub fn transfer_to_sol_treasury<'info>(
    from: &AccountInfo<'info>,
    sol_treasury: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: sol_treasury.to_account_info(),
            },
        ),
        amount,
    )
}

/// Integer square root implementation for u64
/// Uses binary search to find the largest integer whose square is <= n
pub fn integer_sqrt(n: u64) -> u32 {
    if n == 0 {
        return 0;
    }

    let mut left = 1u32;
    let mut right = if n > u32::MAX as u64 {
        u32::MAX
    } else {
        n as u32
    };
    let mut result = 0u32;

    while left <= right {
        let mid = left + (right - left) / 2;
        let mid_squared = (mid as u64) * (mid as u64);

        if mid_squared == n {
            return mid;
        } else if mid_squared < n {
            result = mid;
            left = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            right = mid - 1;
        }
    }

    result
}
