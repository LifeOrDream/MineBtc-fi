# Deployment Guide for MoonBase & Raydium Programs

## Overview

The `0_deploy_game.js` script has been updated to properly deploy all three programs in the correct order:

1. **Raydium CP Swap** (dependency) - AMM for token swapping
2. **MoonBase** - Main game logic
3. **MoonEconomy** - Economy and electricity management

## Key Features

### Raydium Admin Configuration

The Raydium CP Swap program has hardcoded admin addresses for different networks:

- **Admin address**: Controls AMM config updates, pool status, protocol fees
- **Pool fee receiver**: Receives fees from pool creation

For **localnet/devnet**, the script automatically updates these addresses to your deployer wallet address before building.

### Deployment Order

1. **Generate keypairs** for all three programs
2. **Update Raydium admin addresses** to deployer wallet (for devnet feature)
3. **Update `declare_id!`** macros in all programs
4. **Update Anchor.toml** with new program addresses
5. **Build Raydium** with `--features devnet` flag
6. **Build game programs** (moonbase depends on Raydium)
7. **Deploy in order**: Raydium → MoonBase → MoonEconomy
8. **Save deployment info** to `deployments/{cluster}.json`

## Usage

```bash
# Make sure you have:
# 1. Solana CLI installed
# 2. Anchor CLI installed  
# 3. Rust toolchain with BPF target
# 4. Validator running (localnet)
solana-test-validator

# Run deployment
node setup_scripts/0_deploy_game.js
```

## Configuration

Edit `setup_scripts/config.json`:

```json
{
  "network": {
    "cluster": "localnet",
    "rpc_url": "http://127.0.0.1:8899"
  }
}
```

## Output Files

### Deployment Info

Saved to `setup_scripts/deployments/localnet.json`:

```json
{
  "RAYDIUM_CP_PROGRAM_ID": "...",
  "MOON_BASE_PROGRAM_ID": "...",
  "MOON_ECONOMY_PROGRAM_ID": "...",
  "last_deployment": {
    "timestamp": "...",
    "cluster": "localnet",
    "programs_deployed": ["raydium_cp_swap", "moonbase", "mooneconomy"]
  }
}
```

### Program Keypairs

Generated at:
- `raydium/target/deploy/raydium_cp_swap-keypair.json`
- `target/deploy/moonbase-keypair.json`
- `target/deploy/mooneconomy-keypair.json`

## Important Notes

### Raydium Dependencies

**MoonBase** has a dependency on Raydium in `programs/moonbase/Cargo.toml`:

```toml
raydium-cp-swap = { path = "../../raydium/programs/cp-swap", features = ["cpi"] }
```

This means:
- Raydium MUST be deployed first
- Raydium MUST be built with the same features (devnet)
- The program ID must match in both build and deployment

### Admin Privileges

After deployment, the **deployer wallet** will have admin rights to:

1. **Create AMM configs** (set trade fees, protocol fees)
2. **Update pool status** (enable/disable pools)
3. **Collect protocol fees** from pools
4. **Create permission PDAs** (whitelist pool creators)

### Next Steps

After successful deployment:

1. **Initialize token**: `node setup_scripts/1_init_mdoge_token.js`
2. **Create AMM config**: Must be done before creating pools
3. **Create Raydium pool**: `node setup_scripts/2_init_mdoge_SOL_pool.js`
4. **Initialize game state**: `node setup_scripts/3_init_moonbase.js`
5. **Initialize economy**: `node setup_scripts/4_init_moonEconomy.js`

## Troubleshooting

### Build Failures

If Raydium build fails:
```bash
cd raydium
anchor build -- --features devnet
```

If game programs fail:
```bash
anchor build
```

### Deployment Failures

Check:
1. Validator is running: `solana logs`
2. Sufficient SOL: `solana balance`
3. Correct network: `solana config get`
4. Program sizes don't exceed limits

### Admin Address Issues

If you need to change admin address after deployment, you'll need to:
1. Update the address in `raydium/programs/cp-swap/src/lib.rs`
2. Rebuild with `anchor build -- --features devnet`
3. Upgrade the program: `solana program deploy --program-id <keypair>`

## Security Considerations

### Mainnet Deployment

For mainnet:
1. **DO NOT** use `--features devnet`
2. **DO NOT** change admin addresses
3. Use official Raydium program ID
4. Consider using a multisig for admin operations
5. Audit all custom changes

### Localnet/Devnet

- Admin = deployer wallet (single point of control)
- Pool fees go to deployer
- Suitable for testing only

## Architecture

```
┌─────────────────┐
│ Raydium CP Swap │ ← Admin controls AMM config
└────────┬────────┘
         │ (dependency)
         ↓
    ┌────────┐
    │MoonBase│ ← Uses Raydium for swaps
    └────┬───┘
         │ (CPI calls)
         ↓
  ┌──────────────┐
  │ MoonEconomy  │ ← Manages electricity/economy
  └──────────────┘
```

## Admin Functions Reference

### Raydium Admin Can:
- `create_amm_config()` - Create new AMM configurations
- `update_amm_config()` - Modify fees, owners
- `update_pool_status()` - Enable/disable pools
- `collect_protocol_fee()` - Withdraw protocol fees
- `collect_fund_fee()` - Withdraw fund fees
- `create_permission_pda()` - Whitelist creators
- `close_permission_pda()` - Remove whitelist

All these require the signer to match `crate::admin::ID`.

