# Instruction Surface

Canonical source: `programs/mineBTC/src/lib.rs` and `programs/mineBTC/src/instructions/`.

## Admin and Configuration

- `initialize(fee_recipient)` - creates global config, mining state, HODL pool, SOL treasury, autominer custody.
- `set_raydium_pool_state(pool_state)` - stores authorized Raydium pool and initializes SOL rewards/prize pot vaults.
- `add_faction(faction_name, faction_id)` - sequential faction setup and `FactionState` creation.
- `initialize_system_accounts()` - system referral sentinel, buybacks account, buybacks SOL vault.
- `update_config(new_authority, new_fee_recipient)` - starts authority transfer and/or updates fee recipient.
- `cancel_authority_transfer()` and `accept_authority()` - two-step authority transfer.
- `update_fees(...)` - SOL fee split, degenBTC round distribution, HODL tax, snapshot interval, referral split, cycle SOL split, NFT market-making pct.
- `update_rpg_progression(enabled)`, `set_pause(paused)`, `update_evolution_unlock_stage(max_stage)`, `update_gameplay_tuning(args)` - live gameplay controls.
- `update_breeding_config(...)`, `update_emission_params(...)`.

## Token, Mining, HashBeast, and Custodian Setup

- `initialize_mining(start_timestamp, dbtc_per_round, pool_state)` - token vault and emission state.
- `deposit_dbtc_tokens(amount)` - deposits pre-minted degenBTC into mining vault.
- `initialize_hashpower_config(...)`, `initialize_custodian_accounts()`.
- `initialize_hashbeast_config()`, `initialize_hashbeast_mint_config(...)`.
- `update_hashbeast_config(...)`, `update_hashbeast_mint_config(...)`, `switch_hashbeast_mining()`.
- `create_hashbeast_collection(name, uri)`, `init_hashbeast_royalties(...)`, `add_collection_delegate(...)`, `update_collection_info(...)`.
- `add_ticket_tier_config(index, ticket_value)`, `set_hashbeast_free_mint_allowance(user, remaining_free_mints)`.

## Economy and Tax Cranks

- `distribute_sol_fees()` - moves accumulated SOL fee balances into buybacks (`buyback_pct`), `inventory_sweep_vault` (`nft_market_making_pct`, default 3%), and dev (residual via WSOL).
- `snapshot_price()` - swaps/observes pool to append price snapshot and earmark POL SOL.
- `update_rate()` - adjusts emission rate and faction-war multiplier based on price movement.
- `add_lp_and_burn(lp_token_amount)` - adds liquidity and burns LP, completing a POL operation/cycle step.
- `initialize_tax_config(treasury_pct, burn_pct)`, `update_tax_config(treasury_pct, burn_pct)` — NFT floor sweep is no longer in the tax split.
- `crank_harvest_fees(...)`, `crank_distribute_tax(...)`, `claim_faction_treasury_for_faction_war(...)`.

## NFT Marketplace (Permissionless Market Maker)

- `list_user_nft(price)`, `cancel_user_listing()`, `update_user_listing_price(new_price)` — user-signed wrappers that keep the on-chain `FloorQueue` in sync with `degenbtc_market` listings.
- `buy_user_listing()` — permissionless wrapper around `degenbtc_market::buy_listing`; records qualifying user-to-user sales into `SaleHistory` (5-min minimum listing age).
- `register_floor_listing()` — anyone can push an existing user listing into the sorted floor queue (top 20 cheapest, program listings excluded).
- `sweep_floor_lowest()` — anyone can buy queue.entries[0]; the contract auto-disposes the asset (push to country lootbox queue if space, else relist at formula markup, else burn if 7-day floor crashed below threshold) and pays a keeper bounty out of `inventory_sweep_vault`.
- `record_floor_snapshot()` — daily snapshot ix; computes anchor as median of qualifying user-to-user sales, falling back to floor-queue median if low volume.
- `expire_program_listing()` — 7-day TTL re-disposition of stuck program-owned listings; progressive markup discount per strike, force-burn after `MAX_EXPIRES`.
- `handle_inventory_proceeds()` — permissionless 50/50 split of accrued inventory_pda lamports between `inventory_sweep_vault` and `sol_treasury`.
- `inventory_finalize_sale()` — permissionless cleanup; verifies asset's owner is no longer `inventory_pda` via on-chain mpl-core read, closes the `RebornEntry`.
- `claim_lootbox_nft()` — delivers a reserved loser-roll HashBeast to its recorded winner.

## Faction-War Cycle

- `initialize_war_config()` - starts config at current faction-war ID 1.
- `update_war_config(is_active)`.
- `settle_war()` - closes active cycle once linked LP operation target is reached.
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

## Gameplay HashBeasts

- `use_hashbeast_for_gameplay()` - locks one HashBeast for active gameplay multiplier and story events.
- `request_hashbeast_gameplay_unlock()` - marks unlock request for current/next faction-war boundary.
- `withdraw_hashbeast_from_gameplay()` - withdraws once cycle condition permits and syncs cached HashBeast state.

## Staking and Claims

- `stake_degenbtc(amount, lockup_days, position_index)`, `unstake_degenbtc(position_index)`.
- `stake_lp_tokens(amount, lockup_days, position_index)`, `unstake_lp_tokens(position_index)`.
- `claim_staking_rewards()` - SOL and unrefined degenBTC accrual.
- `withdraw_dbtc_rewards()` - withdraws degenBTC rewards with HODL tax when applicable.
- `claim_referral_rewards()` - SOL referral rewards.

## HashBeast Minting and Lifecycle

- `simulate_purchase_cost(...)` - return-data helper for mint price.
- `admin_mint_hashbeast(...)`, `whitelist_mint_hashbeast(...)`, `batch_mint_hashbeasts(...)`.
- `stake_hashbeast()`, `unstake_hashbeast()` - passive staking multiplier path.
- `breed_hashbeasts()` - post-genesis-sellout, same-country / same-rebirth-level breeding, priced at `max(curve_price, 1.5× current_floor_anchor)`; payment is 50% SOL (25% to fee_recipient, 75% to sol_treasury) and 50% dbTC (50% burned, 50% to mining vault). Cooldown table per parent: 0/24h/72h/120h/336h, max 5 breeds per parent.
- `rebirth_hashbeast()` - claims accumulated_val; max 7 rebirths per asset (encoded in DNA bits at offset 174). Asset goes to its country's lootbox queue if space, else burned. Resets multiplier, XP, breed_count, cooldown, accumulated_val.
- `get_gene_breakdown(dna)` - return-data helper for genetics display/debugging.
