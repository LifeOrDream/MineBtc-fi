# Instruction Surface

Canonical source: `programs/mineBTC/src/lib.rs` and `programs/mineBTC/src/instructions/`.

## Admin and Configuration

- `initialize(fee_recipient)` - creates global config, mining state, HODL pool, SOL treasury, autominer custody.
- `set_raydium_pool_state(pool_state)` - stores authorized Raydium pool and initializes SOL rewards/prize pot vaults.
- `add_faction(faction_name, faction_id)` - sequential faction setup and `FactionState` creation.
- `initialize_system_accounts()` - system referral sentinel, buybacks account, buybacks SOL vault.
- `update_config(new_authority, new_fee_recipient)` - starts authority transfer and/or updates fee recipient.
- `cancel_authority_transfer()` and `accept_authority()` - two-step authority transfer.
- `update_fees(...)` - SOL fee split, dogeBTC round distribution, HODL tax, snapshot interval, referral split, cycle SOL split.
- `update_rpg_progression(enabled)`, `set_pause(paused)`, `update_evolution_unlock_stage(max_stage)`, `update_gameplay_tuning(args)` - live gameplay controls.
- `update_breeding_config(...)`, `update_emission_params(...)`.

## Token, Mining, Doge, and Custodian Setup

- `initialize_mining(start_timestamp, mine_btc_per_round, pool_state)` - token vault and emission state.
- `deposit_mine_btc_tokens(amount)` - deposits pre-minted dogeBTC into mining vault.
- `initialize_hashpower_config(...)`, `initialize_custodian_accounts()`.
- `initialize_doge_config(max_supply)`, `initialize_doge_mint_config(...)`.
- `update_doge_config(...)`, `update_doge_mint_config(...)`, `switch_doge_mining()`.
- `create_doge_collection(name, uri)`, `init_doge_royalties(...)`, `add_collection_delegate(...)`, `update_collection_info(...)`.
- `add_ticket_tier_config(index, ticket_value)`, `set_doge_free_mint_allowance(user, remaining_free_mints)`.

## Economy and Tax Cranks

- `distribute_sol_fees()` - moves accumulated SOL fee balances into buybacks/staker/dev buckets.
- `snapshot_price()` - swaps/observes pool to append price snapshot and earmark POL SOL.
- `update_rate()` - adjusts emission rate and faction-war multiplier based on price movement.
- `add_lp_and_burn(lp_token_amount)` - adds liquidity and burns LP, completing a POL operation/cycle step.
- `initialize_tax_config(...)`, `update_tax_config(...)`, `update_nft_floor_sweep_whitelist(...)`.
- `crank_harvest_fees(...)`, `crank_distribute_tax(...)`, `claim_faction_treasury_for_faction_war(...)`, `withdraw_nft_floor_sweep_funds(...)`.

## Faction-War Cycle

- `initialize_faction_war_config()` - starts config at current faction-war ID 1.
- `update_faction_war_config(is_active)`.
- `settle_faction_war()` - closes active cycle once linked LP operation target is reached.
- `claim_faction_war_rewards()` - user cycle reward claim.

## Rounds and Bets

- `start_round(round_id)` - permissionless keeper can start when game active and allowed.
- `end_round()` - settles winner with slot-hash entropy.
- `end_round_faction_rewards(...)` - finalizes reward indexes/pools after winner selection.
- `initialize_player(faction_id, referral_code)` - creates player and referral-reward PDA.
- `join_bets(...)` - places one or more faction-direction positions, including SOL and/or ticket/points.
- `claim_round_rewards(round_id)` - claims per-round reward and may process mutation/faction-war accumulation.

## Autominer

- `init_autominer(...)`, `execute_autominer_bet(...)`, `update_autominer(...)`, `stop_autominer()`.
- `claim_autominer_rewards(...)` - claims eligible autominer round rewards.

## Gameplay Doges

- `use_doge_for_gameplay()` - locks one Doge for active gameplay multiplier and story events.
- `request_doge_gameplay_unlock()` - marks unlock request for current/next faction-war boundary.
- `withdraw_doge_from_gameplay()` - withdraws once cycle condition permits and syncs cached Doge state.

## Staking and Claims

- `stake_minebtc(amount, lockup_days, position_index)`, `unstake_minebtc(position_index)`.
- `stake_lp_tokens(amount, lockup_days, position_index)`, `unstake_lp_tokens(position_index)`.
- `claim_staking_rewards()` - SOL and unrefined dogeBTC accrual.
- `withdraw_dbtc_rewards()` - withdraws dogeBTC rewards with HODL tax when applicable.
- `claim_referral_rewards()` - SOL referral rewards.

## Doge Minting and Lifecycle

- `simulate_purchase_cost(...)` - return-data helper for mint price.
- `admin_mint_doge(...)`, `whitelist_mint_doge(...)`, `batch_mint_doges(...)`.
- `stake_doge()`, `unstake_doge()` - passive staking multiplier path.
- `breed_doges()`, `send_to_heaven()` - breeding and accumulated-value burn/claim path.
- `get_gene_breakdown(dna)` - return-data helper for genetics display/debugging.
