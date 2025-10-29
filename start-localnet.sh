#!/bin/bash

# ============================================================================
# LOCALNET STARTUP SCRIPT WITH ALL REQUIRED PROGRAMS
# ============================================================================
# 
# This script starts a Solana test validator with all necessary programs
# cloned from mainnet, including:
# - Metaplex Core (for Dragon Egg NFTs)
# - Token-2022 (for DOGE_BTC token)
# 
# Usage: ./start-localnet.sh
# ============================================================================

echo "🚀 Starting Solana Test Validator with Metaplex Core..."
echo "============================================================"

# Kill any existing test validator
pkill -f solana-test-validator 2>/dev/null
sleep 2

# Start test validator with cloned programs
solana-test-validator \
  --clone CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d \
  --clone TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
  --url https://api.mainnet-beta.solana.com \
  --ledger test-ledger \
  --reset

echo ""
echo "✅ Test validator started with:"
echo "   • Metaplex Core: CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
echo "   • Token-2022: TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
echo ""
echo "🔧 Now run your deployment scripts:"
echo "   node setup_scripts/0_deploy_game.js"
echo "   node setup_scripts/1_init_mdoge_token.js"
echo "   node setup_scripts/2_init_mdoge_SOL_pool.js"
echo "   node setup_scripts/3_init_moonbase.js"
echo "   node setup_scripts/3_create_dragon_egg_collection.js"

