# Starting Localnet with Metaplex Core

## Problem
Metaplex Core program is not available on localnet by default.

## Solution 1: Clone from Mainnet (Recommended)

Start your test validator with the MPL Core program cloned from mainnet:

```bash
solana-test-validator \
  --clone CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d \
  --url https://api.mainnet-beta.solana.com \
  --reset
```

This will:
- Clone the Metaplex Core program from mainnet
- Make it available on your localnet at the same address
- Allow your Dragon Egg collection script to work

## Solution 2: Use Existing Localnet

If your test validator is already running, you can deploy MPL Core manually:

```bash
# 1. Download MPL Core program from mainnet
solana program dump -u m CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d /tmp/mpl_core.so

# 2. Deploy to localnet with a new address
solana program deploy /tmp/mpl_core.so -u localhost

# 3. Note the deployed program ID and update your script
```

**Note**: If you use Solution 2, you'll need to update the `MPL_CORE_PROGRAM_ID` constant in your script to the new address.

## Recommended Approach

**Use Solution 1** - it keeps the program at the standard address so all tooling/explorers work correctly.

### Complete Startup Command:

```bash
solana-test-validator \
  --clone CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d \
  --url https://api.mainnet-beta.solana.com \
  --ledger test-ledger \
  --reset
```

Then run your scripts:
```bash
node setup_scripts/0_deploy_game.js
node setup_scripts/1_init_mdoge_token.js
node setup_scripts/2_init_mdoge_SOL_pool.js
node setup_scripts/3_init_moonbase.js
node setup_scripts/3_create_dragon_egg_collection.js  # ← Now this will work!
```

