#!/usr/bin/env node

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration paths
const CONFIG_PATH = path.join(__dirname, "config.json");
const DEPLOYMENTS_DIR = path.join(__dirname, "deployments");

function readConfig() {
  try {
    const configData = fs.readFileSync(CONFIG_PATH, "utf8");
    return JSON.parse(configData);
  } catch (error) {
    console.error(`❌ Failed to read config.json: ${error.message}`);
    process.exit(1);
  }
}

function readDeploymentFile(cluster) {
  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);

  if (!fs.existsSync(deploymentPath)) {
    console.error(`❌ Deployment file not found: ${deploymentPath}`);
    process.exit(1);
  }

  try {
    const deploymentData = fs.readFileSync(deploymentPath, "utf8");
    return JSON.parse(deploymentData);
  } catch (error) {
    console.error(`❌ Failed to read deployment file: ${error.message}`);
    process.exit(1);
  }
}

function generateWebsiteConfig(config, deployment) {
  const cluster = config.network?.cluster || "localnet";

  // Extract all necessary addresses and configuration
  const websiteConfig = {
    [cluster]: {
      // ========== PROGRAM IDs ==========
      MINE_BTC_PROGRAM_ID: deployment.MINE_BTC_PROGRAM_ID,
      RAYDIUM_CP_PROGRAM_ID: deployment.RAYDIUM_CP_PROGRAM_ID,

      // ========== NETWORK CONFIG ==========
      rpc_url: config.network?.rpc_url || "http://127.0.0.1:8899",
      commitment: config.network?.commitment || "confirmed",

      // ========== DOGE_BTC TOKEN ==========
      dbtc_mintAddress:
        deployment.dbtc_mint_address ||
        deployment.dbtc_mint_created?.mint_address,
      dbtc_mintAuthority: deployment.dbtc_mint_created?.mint_authority,
      dbtc_burnTaxBps:
        deployment.dbtc_mint_created?.burn_tax_bps ||
        config.token?.burn_tax_bps,
      dbtc_maxBurnAmount:
        deployment.dbtc_mint_created?.max_burn_amount?.toString() ||
        config.token?.max_burn_amount?.toString(),
      dbtc_decimals: config.token?.decimals || 6,

      // ========== RAYDIUM POOL ==========
      raydium_pool_state: deployment.dbtc_sol_pool_created?.poolStatePDA,
      raydium_lp_mint: deployment.dbtc_sol_pool_created?.lpMintPDA,
      raydium_token0_vault: deployment.dbtc_sol_pool_created?.token0VaultPDA, // WSOL
      raydium_token1_vault: deployment.dbtc_sol_pool_created?.token1VaultPDA, // DOGE_BTC
      raydium_authority: deployment.dbtc_sol_pool_created?.authorityPDA,
      raydium_observation_state:
        deployment.dbtc_sol_pool_created?.observationStatePDA,
      raydium_amm_config: deployment.raydium_amm_config_created?.amm_config_pda,
      is_dbtc_token0: deployment.dbtc_sol_pool_created?.isMdogeToken0 || false,
      token0_mint: deployment.dbtc_sol_pool_created?.token0Mint, // WSOL
      token1_mint: deployment.dbtc_sol_pool_created?.token1Mint, // DOGE_BTC

      // ========== MINE-BTC PROGRAM ACCOUNTS ==========
      globalConfig_pda:
        deployment.minebtc_program_initialized?.globalConfig_address,
      mineBtcMining_pda:
        deployment.minebtc_program_initialized?.mineBtcMining_address,
      sol_treasury_pda:
        deployment.minebtc_program_initialized?.solTreasury_address,
      doges_treasury_pda:
        deployment.minebtc_program_initialized?.dogesTreasury_address,
      unrefinedRewards_pda:
        deployment.minebtc_program_initialized?.unrefinedRewards_address,
      autominerCustody_pda:
        deployment.minebtc_program_initialized?.autominerCustody_address,

      // ========== DOGE_BTC MINING VAULT ==========
      dbtc_token_vault: deployment.mining_vault_initialized?.vault_address,
      minebtc_vault_authority:
        deployment.mining_vault_initialized?.vault_authority,
      mining_start_timestamp:
        deployment.mining_vault_initialized?.start_timestamp,
      doge_btc_per_round:
        deployment.mining_vault_initialized?.doge_btc_per_round,

      // ========== HASHPOWER CONFIG ==========
      hashpowerConfig_pda:
        deployment.hashpower_config_initialized?.hashpowerConfig_pda,

      // ========== GAME STATE ==========
      global_game_state_pda:
        deployment.game_state_initialized?.global_game_state_pda,
      round_duration_seconds:
        deployment.game_state_initialized?.round_duration_seconds,

      // ========== RAYDIUM POOL STATE (Game-related vaults) ==========
      sol_rewards_vault: deployment.raydium_pool_state_set?.sol_rewards_vault,
      sol_prize_pot_vault:
        deployment.raydium_pool_state_set?.sol_prize_pot_vault,

      // ==========  DOGE COLLECTION ==========
     doge_collection:
        deployment.doge_collection_created?.collection_address,
     doge_collection_authority:
        deployment.doge_collection_created?.collection_authority,

      // ========== LP TOKEN MANAGEMENT ==========
      lp_token_account:
        deployment.lp_token_accounts_initialized?.lp_token_account,
      lp_token_mint: deployment.lp_token_accounts_initialized?.lp_mint,

      // ========== SYSTEM ACCOUNTS ==========
      referral_rewards_pda:
        deployment.system_accounts_initialized?.system_referral_rewards_pda,
      buybacks_account_pda:
        deployment.system_accounts_initialized?.buybacks_account_pda,
      buybacks_sol_vault_pda:
        deployment.system_accounts_initialized?.buybacks_sol_vault_pda,

      // ========== CUSTODIAN ACCOUNTS ==========
      minebtcCustodian_pda:
        deployment.custodian_accounts_initialized?.dbtc_custodian,
      minebtcCustodian_authority:
        deployment.custodian_accounts_initialized?.dbtc_custodian_authority,
      liquidityCustodian_pda:
        deployment.custodian_accounts_initialized?.liquidity_custodian,
      liquidityCustodian_authority:
        deployment.custodian_accounts_initialized
          ?.liquidity_custodian_authority,

      // ========== DOGE CONFIG ==========
      doge_config_pda: deployment.doge_config_initialized?.doges_config_pda,
      doge_base_price: deployment.doge_config_initialized?.base_price,
      doge_curve_a: deployment.doge_config_initialized?.curve_a,
      doge_max_supply: deployment.doge_config_initialized?.max_supply,

      // ========== TAX CONFIG ==========
      tax_config_pda: deployment.tax_config_initialized?.tax_config_pda,
      withdraw_withheld_authority:
        deployment.tax_config_initialized?.withdraw_withheld_authority,
      faction_treasury_vault:
        deployment.tax_config_initialized?.faction_treasury_vault,
      nft_floor_sweep_vault:
        deployment.tax_config_initialized?.nft_floor_sweep_vault,
      nft_sale_sol_vault: deployment.tax_config_initialized?.nft_sale_sol_vault,

      // ========== CRANKER BOTS ==========
      cranker_bots: deployment.cranker_bots_added?.bots || [],

      // ========== GAME CONFIGURATION ==========
      base_creation_cost: config.minebtc?.base_creation_cost || 100000000,
      loot_percentage: config.minebtc?.loot_percentage || 10,

      // ========== TICKET TIER CONFIGURATION ==========
      ticket_tiers:
        deployment.ticket_tier_configs_initialized?.ticket_tiers?.map(
          (tier) => ({
            tier_index: tier.tier_index,
            ticket_value: tier.ticket_value,
          })
        ) ||
        config.doges_config?.ticket_tiers ||
        [],

      // ========== FACTION CONFIGURATION ==========
      supported_factions:
        deployment.factions_added?.factions?.map((f) => f.name) ||
        config.factions?.map((f) => f.name) ||
        [],
      faction_states:
        deployment.factions_added?.factions?.map((f) => ({
          faction_id: f.faction_id,
          name: f.name,
          faction_state_pda: f.faction_state_pda,
        })) || [],

      // // ========== EXPANSION CONFIGURATION ==========
      // "expansions": config.expansions || [],

      // ========== AUTHORITIES ==========
      fee_recipient:
        deployment.minebtc_program_initialized?.FEE_RECIPIENT_MULTISIG ||
        config.deployment?.FEE_RECIPIENT_MULTISIG,
      transfer_fee_config_authority:
        config.deployment?.transfer_fee_config_authority,
    },
  };

  return websiteConfig;
}

function saveWebsiteConfig(websiteConfig, cluster) {
  const websitePath = path.join(DEPLOYMENTS_DIR, "website.json");

  let existingConfig = {};
  if (fs.existsSync(websitePath)) {
    try {
      const existingData = fs.readFileSync(websitePath, "utf8");
      existingConfig = JSON.parse(existingData);
    } catch (error) {
      console.log(
        `⚠️  Could not parse existing website.json, creating new one`
      );
    }
  }

  // Merge with existing config to support multiple clusters
  const mergedConfig = { ...existingConfig, ...websiteConfig };

  fs.writeFileSync(websitePath, JSON.stringify(mergedConfig, null, 2));
  console.log(`✅ Website configuration saved to: ${websitePath}`);
  console.log(`📍 Cluster: ${cluster}`);
}

function main() {
  console.log(`🌐 Generating website configuration...`);
  console.log(`=======================================`);

  try {
    // Read configuration
    const config = readConfig();
    const cluster = config.network?.cluster || "localnet";

    console.log(`📖 Reading configuration for cluster: ${cluster}`);

    // Read deployment file
    const deployment = readDeploymentFile(cluster);

    console.log(
      `🔍 Found deployment data with ${Object.keys(deployment).length} entries`
    );

    // Generate website configuration
    const websiteConfig = generateWebsiteConfig(config, deployment);

    console.log(
      `🏗️  Generated website configuration with ${
        Object.keys(websiteConfig[cluster]).length
      } properties`
    );

    // Save website configuration
    saveWebsiteConfig(websiteConfig, cluster);

    // Success summary
    console.log(`\n🎉 WEBSITE CONFIGURATION GENERATED SUCCESSFULLY! 🎉`);
    console.log(`================================================`);
    console.log(`📊 Configuration Summary:`);
    console.log(`  🌐 Cluster: ${cluster}`);
    console.log(
      `  🔗 MineBTC Program: ${websiteConfig[cluster].MINE_BTC_PROGRAM_ID}`
    );
    console.log(
      `  🪙 DOGE_BTC Token: ${websiteConfig[cluster].dbtc_mintAddress}`
    );
    console.log(
      `  🏊 Raydium Pool: ${websiteConfig[cluster].raydium_pool_state}`
    );
    console.log(
      `  🎮 Global Game State: ${
        websiteConfig[cluster].global_game_state_pda || "Not initialized"
      }`
    );
    console.log(
      `  🥚 Doge Collection: ${
        websiteConfig[cluster].doge_collection || "Not created"
      }`
    );
    console.log(
      `  💰 Buybacks Account: ${
        websiteConfig[cluster].buybacks_account_pda || "Not initialized"
      }`
    );
    console.log(
      `  🤖 Cranker Bots: ${
        websiteConfig[cluster].cranker_bots?.length || 0
      } bot(s)`
    );

    console.log(`\n🔗 Next Steps:`);
    console.log(
      `  1. Update your frontend to use the addresses from deployments/website.json`
    );
    console.log(`  2. Test the frontend connection to the deployed programs`);
    console.log(`  3. Verify all addresses are correct and accessible`);
  } catch (error) {
    console.error(`\n💥 CONFIGURATION GENERATION FAILED! 💥`);
    console.error(`======================================`);
    console.error(`Error: ${error.message}`);

    console.log(`\n🔧 Troubleshooting:`);
    console.log(`  1. Make sure config.json exists and is valid`);
    console.log(`  2. Ensure the deployment file exists for your cluster`);
    console.log(
      `  3. Check that all required deployment steps have been completed`
    );
    console.log(`  4. Verify file permissions in the deployments directory`);

    process.exit(1);
  }
}

// Run the script if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;
