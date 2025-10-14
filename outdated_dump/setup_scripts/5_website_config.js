#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration paths
const CONFIG_PATH = path.join(__dirname, 'config.json');
const DEPLOYMENTS_DIR = path.join(__dirname, 'deployments');

function readConfig() {
  try {
    const configData = fs.readFileSync(CONFIG_PATH, 'utf8');
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
    const deploymentData = fs.readFileSync(deploymentPath, 'utf8');
    return JSON.parse(deploymentData);
  } catch (error) {
    console.error(`❌ Failed to read deployment file: ${error.message}`);
    process.exit(1);
  }
}

function generateWebsiteConfig(config, deployment) {
  const cluster = config.network?.cluster || 'localnet';
  
  // Extract all necessary addresses and configuration
  const websiteConfig = {
    [cluster]: {
      // ========== PROGRAM IDs ==========
      "MOON_BASE_PROGRAM_ID": deployment.MOON_BASE_PROGRAM_ID,
      "MOON_ECONOMY_PROGRAM_ID": deployment.MOON_ECONOMY_PROGRAM_ID,
      "RAYDIUM_CP_PROGRAM_ID": deployment.RAYDIUM_CP_PROGRAM_ID,
      
      // ========== NETWORK CONFIG ==========
      "rpc_url": config.network?.rpc_url || "http://127.0.0.1:8899",
      "commitment": config.network?.commitment || "confirmed",
 
      // ========== mDOGE TOKEN ==========
      "mdoge_mintAddress": deployment.mdoge_mint_address || deployment.mdoge_mint_created?.mint_address,
      "mdoge_mintAuthority": deployment.mdoge_mint_created?.mint_authority,
      "mdoge_burnTaxBps": deployment.mdoge_mint_created?.burn_tax_bps || config.token?.burn_tax_bps,
      "mdoge_maxBurnAmount": deployment.mdoge_mint_created?.max_burn_amount?.toString() || config.token?.max_burn_amount?.toString(),
      "mdoge_decimals": config.token?.decimals || 6,
      
      // ========== RAYDIUM POOL ==========
      "raydium_pool_state": deployment.mdoge_sol_pool_created?.poolStatePDA,
      "raydium_lp_mint": deployment.mdoge_sol_pool_created?.lpMintPDA,
      "raydium_token0_vault": deployment.mdoge_sol_pool_created?.token0VaultPDA, // WSOL
      "raydium_token1_vault": deployment.mdoge_sol_pool_created?.token1VaultPDA, // mDOGE
      "raydium_authority": deployment.mdoge_sol_pool_created?.authorityPDA,
      "raydium_observation_state": deployment.mdoge_sol_pool_created?.observationStatePDA,
      "raydium_amm_config": deployment.raydium_amm_config_created?.amm_config_pda,
      "is_mdoge_token0": deployment.mdoge_sol_pool_created?.isMdogeToken0 || false,
      "token0_mint": deployment.mdoge_sol_pool_created?.token0Mint, // WSOL
      "token1_mint": deployment.mdoge_sol_pool_created?.token1Mint, // mDOGE
      
      // ========== MOON BASE PROGRAM ACCOUNTS ==========
      "globalConfig_pda": deployment.moonbase_program_initialized?.globalConfig_address,
      "moonDogeMining_pda": deployment.moonbase_program_initialized?.moonDogeMining_address,
      "sol_treasury_pda": deployment.moonbase_program_initialized?.solTreasury_address,
      
      // ========== mDOGE MINING VAULT ==========
      "mdoge_token_vault": deployment.mining_vault_initialized?.vault_address,
      "mdoge_vault_authority": deployment.mining_vault_initialized?.vault_authority,
      "mining_start_timestamp": deployment.mining_vault_initialized?.start_timestamp,
      "moon_doge_per_slot": deployment.mining_vault_initialized?.moon_doge_per_slot,
      
      // ========== LP TOKEN MANAGEMENT ==========
      "lp_token_account": deployment.lp_token_accounts_initialized?.lp_token_account,
      "lp_token_mint": deployment.lp_token_accounts_initialized?.lp_mint,
      
      // ========== SYSTEM ACCOUNTS ==========
      "referral_rewards_pda": deployment.referral_system_initialized?.system_referral_pda,
      "module_config_store": deployment.config_stores_initialized?.module_config_store,
      "pvp_matchmaker_pda": deployment.pvp_matchmaker_initialized?.pvp_matchmaker_pda,
      
      // ========== LOOT REWARDS SYSTEM ==========
      "loot_rewards_pda": deployment.loot_rewards_initialized?.loot_rewards_pda,
      "loot_sol_vault": deployment.loot_rewards_initialized?.sol_vault,
      "loot_mdoge_vault": deployment.loot_rewards_initialized?.mdoge_vault,
      "loot_mdoge_vault_authority": deployment.loot_rewards_initialized?.loot_mdoge_vault_authority,
      
      // ========== LEVEL STATS ==========
      "level_stats_pda": deployment.level_stats_initialized?.level_stats_pda,
      
      // ========== GAME CONFIGURATION ==========
      "base_creation_cost": config.moonbase?.base_creation_cost || 100000000,
      "loot_percentage": config.moonbase?.loot_percentage || 10,
      
      // ========== MINING CONFIGURATION ==========
      "slots_for_swap": config.mining?.slots_for_swap || 9000,
      "initial_distribution_rate": config.mining?.moon_doge_per_slot || 1000000,
      
      // ========== PVP CONFIGURATION ==========
      "pvp_ticket_tiers": config.pvp?.ticket_tiers || [
        100000000,    // 0.1 SOL
        1000000000,   // 1 SOL
        10000000000,  // 10 SOL
        100000000000, // 100 SOL
        500000000000  // 500 SOL
      ],
      "pvp_min_hp_required": config.pvp?.min_hp_required || 1000,
      "pvp_turn_timeout_seconds": config.pvp?.turn_timeout_seconds || 300,
      "pvp_max_turns": config.pvp?.max_turns || 15,
      
      // ========== FACTION CONFIGURATION ==========
      "supported_factions": config.factions?.map(f => f.name) || [
        "United States", "China", "Russia", "Israel", "Iran", "Ukraine"
      ],
      
      // ========== EXPANSION CONFIGURATION ==========
      "expansions": config.expansions || [],
            
      // ========== AUTHORITIES ==========
      "creation_fee_recipient": config.deployment?.creation_fee_recipient,
      "transfer_fee_config_authority": config.deployment?.transfer_fee_config_authority,
                  
            // ========== MOON ECONOMY (if deployed) ==========
      ...(deployment.MOON_ECONOMY_PROGRAM_ID && {
        "moon_economy_enabled": true,
        "moondoge_allocation": config.moonEconomy?.moondoge_allocation || 33,
        "liquidity_allocation": config.moonEconomy?.liquidity_allocation || 33,
        "min_lockup_days": config.moonEconomy?.min_lockup_days || 1,
        "max_lockup_days": config.moonEconomy?.max_lockup_days || 365,
        "base_multiplier": config.moonEconomy?.base_multiplier || 100,
        "max_multiplier": config.moonEconomy?.max_multiplier || 700,
        "electricity_per_weighted_mdoge": config.moonEconomy?.electricity_per_weighted_mdoge || 100,
        "electricity_per_weighted_lp_tokens": config.moonEconomy?.electricity_per_weighted_lp_tokens || 400,
        
        // ========== MOON ECONOMY PDAs ==========
        "moonEconomy_globalConfig_pda": deployment.moonEconomy_program_initialized?.moonEconomy_globalConfig_data_ac,
        "moonEconomy_devEarnings_pda": deployment.moonEconomy_program_initialized?.moonEconomy_devEarnings_data_ac,
        "moonEconomy_feeCollector_pda": deployment.moonEconomy_program_initialized?.moonEconomy_feeCollector_data_ac,
        
        // ========== MOON ECONOMY mDOGE VAULTS ==========
        "moonEconomy_mdoge_vault": deployment.moonEconomy_mDogeVault_initialized?.moondogeVault,
        "moonEconomy_mdoge_sol_vault": deployment.moonEconomy_mDogeVault_initialized?.mdogeSolVault,
        "moonEconomy_mdoge_custodian": deployment.moonEconomy_mDogeVault_initialized?.mdogeCustodian,
        "moonEconomy_mdoge_custodian_authority": deployment.moonEconomy_mDogeVault_initialized?.mdogeCustodianAuthority,
        
        // ========== MOON ECONOMY LIQUIDITY VAULTS ==========
        "moonEconomy_liquidity_vault": deployment.moonEconomy_liquidityVault_initialized?.liquidityVault,
        "moonEconomy_liquidity_sol_vault": deployment.moonEconomy_liquidityVault_initialized?.liquiditySolVault,
        "moonEconomy_liquidity_custodian": deployment.moonEconomy_liquidityVault_initialized?.liquidityCustodian,
        "moonEconomy_liquidity_custodian_authority": deployment.moonEconomy_liquidityVault_initialized?.liquidityCustodianAuthority
      }),
      
      // ========== UI CONFIGURATION ==========
      "max_modules_per_base": 50,
      "grid_width": 20,
      "grid_height": 15,
      "max_upgrade_level": 10,
      
      // ========== FEATURE FLAGS ==========
      "features": {
        "pvp_enabled": config.moonbase?.is_game_active !== false,
        "loot_rewards_enabled": true,
        "referral_system_enabled": true,
        "level_system_enabled": true,
        "expansion_system_enabled": true,
        "moon_economy_enabled": !!deployment.MOON_ECONOMY_PROGRAM_ID
      }
    }
  };
  
  return websiteConfig;
}

function saveWebsiteConfig(websiteConfig, cluster) {
  const websitePath = path.join(DEPLOYMENTS_DIR, 'website.json');
  
  let existingConfig = {};
  if (fs.existsSync(websitePath)) {
    try {
      const existingData = fs.readFileSync(websitePath, 'utf8');
      existingConfig = JSON.parse(existingData);
    } catch (error) {
      console.log(`⚠️  Could not parse existing website.json, creating new one`);
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
    const cluster = config.network?.cluster || 'localnet';
    
    console.log(`📖 Reading configuration for cluster: ${cluster}`);
    
    // Read deployment file
    const deployment = readDeploymentFile(cluster);
    
    console.log(`🔍 Found deployment data with ${Object.keys(deployment).length} entries`);
    
    // Generate website configuration
    const websiteConfig = generateWebsiteConfig(config, deployment);
    
    console.log(`🏗️  Generated website configuration with ${Object.keys(websiteConfig[cluster]).length} properties`);
    
    // Save website configuration
    saveWebsiteConfig(websiteConfig, cluster);
    
    // Success summary
    console.log(`\n🎉 WEBSITE CONFIGURATION GENERATED SUCCESSFULLY! 🎉`);
    console.log(`================================================`);
    console.log(`📊 Configuration Summary:`);
    console.log(`  🌐 Cluster: ${cluster}`);
    console.log(`  🔗 Moon Base Program: ${websiteConfig[cluster].MOON_BASE_PROGRAM_ID}`);
    console.log(`  🔗 Moon Economy Program: ${websiteConfig[cluster].MOON_ECONOMY_PROGRAM_ID || 'Not deployed'}`);
    console.log(`  🪙 mDOGE Token: ${websiteConfig[cluster].mdoge_mintAddress}`);
    console.log(`  🏊 Raydium Pool: ${websiteConfig[cluster].raydium_pool_state}`);
    console.log(`  💰 Loot Rewards: ${websiteConfig[cluster].loot_rewards_pda}`);
    console.log(`  ⚔️  PvP Matchmaker: ${websiteConfig[cluster].pvp_matchmaker_pda}`);
    
    console.log(`\n🔗 Next Steps:`);
    console.log(`  1. Update your frontend to use the addresses from deployments/website.json`);
    console.log(`  2. Test the frontend connection to the deployed programs`);
    console.log(`  3. Verify all addresses are correct and accessible`);
    
  } catch (error) {
    console.error(`\n💥 CONFIGURATION GENERATION FAILED! 💥`);
    console.error(`======================================`);
    console.error(`Error: ${error.message}`);
    
    console.log(`\n🔧 Troubleshooting:`);
    console.log(`  1. Make sure config.json exists and is valid`);
    console.log(`  2. Ensure the deployment file exists for your cluster`);
    console.log(`  3. Check that all required deployment steps have been completed`);
    console.log(`  4. Verify file permissions in the deployments directory`);
    
    process.exit(1);
  }
}

// Run the script if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;
