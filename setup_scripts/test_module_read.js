#!/usr/bin/env node

import { Connection, PublicKey } from '@solana/web3.js';
import { BorshAccountsCoder } from '@coral-xyz/anchor';
import fs from 'fs';
import path from 'path';

// Load IDL
const idlPath = path.join(process.cwd(), '../target/idl/moonbase.json');
const moonBaseIdl = JSON.parse(fs.readFileSync(idlPath, 'utf8'));

// Load config
const configPath = path.join(process.cwd(), 'deployments/localnet.json');
const deployment = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const connection = new Connection("http://127.0.0.1:8899", 'confirmed'); // Use localnet
const programId = new PublicKey(deployment.MOON_BASE_PROGRAM_ID);

// Helper function to convert BN to number safely
function convertBNToNumber(value) {
  if (value && typeof value.toNumber === 'function') {
    return value.toNumber();
  }
  return value;
}

// Helper function to convert BN to string for large numbers
function convertBNToString(value) {
  if (value && typeof value.toString === 'function') {
    return value.toString();
  }
  return value;
}

async function getModuleConfig(configId) {
  try {
    console.log(`\n🔍 Reading Module Config ID: ${configId}`);
    
    // Derive the ModuleConfigAccount PDA
    const configIdBuffer = Buffer.alloc(2);
    configIdBuffer.writeUInt16LE(configId, 0);
    
    const [moduleConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("module-config"), configIdBuffer], 
      programId
    );
    
    console.log(`📍 Module Config PDA: ${moduleConfigPda}`);
    
    const accountInfo = await connection.getAccountInfo(moduleConfigPda);
    
    if (!accountInfo) {
      console.log('❌ Account not found');
      return null;
    }

    const coder = new BorshAccountsCoder(moonBaseIdl);
    const moduleConfigAccount = coder.decode('ModuleConfigAccount', accountInfo.data);
    
    console.log('\n📊 Raw Module Config Account:');
    console.log('Module ID:', moduleConfigAccount.data.id);
    console.log('Name:', moduleConfigAccount.data.name);
    console.log('Module Type:', moduleConfigAccount.data.module_type);
    console.log('Raw Stats Object:', moduleConfigAccount.data.stats);
    
    // **CORRECT ENUM HANDLING**
    const stats = moduleConfigAccount.data.stats;
    const moduleType = moduleConfigAccount.data.module_type;
    
    console.log('\n🔧 Processing Stats...');
    console.log('Module Type Object:', moduleType);
    console.log('Stats Object Keys:', Object.keys(stats));
    
    // Process stats based on the variant that exists
    let processedStats = null;
    
    if (stats.Mining) {
      console.log('📊 Processing Mining Stats...');
      const miningData = stats.Mining['0'] || stats.Mining;
      processedStats = {
        type: 'Mining',
        max_hp: convertBNToNumber(miningData.max_hp),
        base_hashpower: convertBNToNumber(miningData.base_hashpower),
        power_consumption: convertBNToNumber(miningData.power_consumption)
      };
    } else if (stats.Attraction) {
      console.log('📊 Processing Attraction Stats...');
      const attractionData = stats.Attraction['0'] || stats.Attraction;
      processedStats = {
        type: 'Attraction',
        max_hp: convertBNToNumber(attractionData.max_hp),
        base_xp_per_hour: convertBNToNumber(attractionData.base_xp_per_hour),
        power_consumption: convertBNToNumber(attractionData.power_consumption)
      };
    } else if (stats.Attack) {
      console.log('📊 Processing Attack Stats...');
      const attackData = stats.Attack['0'] || stats.Attack;
      processedStats = {
        type: 'Attack',
        max_hp: convertBNToNumber(attackData.max_hp),
        base_damage: convertBNToNumber(attackData.base_damage),
        base_missiles_per_load: convertBNToNumber(attackData.base_missiles_per_load),
        reload_time_seconds: convertBNToNumber(attackData.reload_time_seconds),
        power_consumption: convertBNToNumber(attackData.power_consumption)
      };
    } else if (stats.Research) {
      console.log('📊 Processing Research Stats...');
      console.log('Raw Research Stats:', stats.Research);
      
      // Handle both direct object and array-wrapped object
      const researchData = stats.Research['0'] || stats.Research;
      
      processedStats = {
        type: 'Research',
        max_hp: convertBNToNumber(researchData.max_hp),
        cooldown_sec: convertBNToNumber(researchData.cooldown_sec),
        max_reward: convertBNToString(researchData.max_reward), // Use string for large numbers
        probability: convertBNToNumber(researchData.probability),
        power_consumption: convertBNToNumber(researchData.power_consumption)
      };
    }
    
    console.log('\n✅ Processed Stats:', processedStats);
    
    // Process other fields
    const processedConfig = {
      id: moduleConfigAccount.data.id,
      name: moduleConfigAccount.data.name,
      image_url: moduleConfigAccount.data.image_url,
      module_type: moduleType,
      stats: processedStats,
      faction_ids: Array.from(moduleConfigAccount.data.faction_ids || []),
      min_level: moduleConfigAccount.data.min_level,
      max_per_base: moduleConfigAccount.data.max_per_base,
      width: moduleConfigAccount.data.width,
      height: moduleConfigAccount.data.height,
      mint_cost: convertBNToString(moduleConfigAccount.data.mint_cost),
      upgrade_cost: convertBNToString(moduleConfigAccount.data.upgrade_cost),
      upgrade_level_requirements: Array.from(moduleConfigAccount.data.upgrade_level_requirements || []),
      is_active: moduleConfigAccount.data.is_active
    };
    
    console.log('\n🎯 Final Processed Config:');
    console.log(JSON.stringify(processedConfig, null, 2));
    
    return processedConfig;
    
  } catch (error) {
    console.error('❌ Error reading module config:', error);
    return null;
  }
}

// Get DogeBtcMining state
async function getDogeBtcMining() {
  try {
    console.log('\n🔍 Reading DogeBtcMining state...');
    
    // Use stored address from deployment if available
    let miningPda;
    if (deployment.moonbase_program_initialized?.dogeBtcMining_address) {
      miningPda = new PublicKey(deployment.moonbase_program_initialized.dogeBtcMining_address);
      console.log(`📍 Using stored DogeBtcMining address: ${miningPda}`);
    } else {
      // Fallback: Derive the DogeBtcMining PDA
      [miningPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("doge-btc-mining")],
        programId
      );
      console.log(`📍 Derived DogeBtcMining PDA: ${miningPda}`);
    }
    
    const accountInfo = await connection.getAccountInfo(miningPda);
    
    if (!accountInfo) {
      console.log('❌ DogeBtcMining account not found');
      return null;
    }

    const coder = new BorshAccountsCoder(moonBaseIdl);
    const miningData = coder.decode('DogeBtcMining', accountInfo.data);
    
    console.log('\n📊 Raw DogeBtcMining Account fields:');
    console.log('Available keys:', Object.keys(miningData));
    
    // Convert Pubkeys to strings (using snake_case field names from IDL)
    const processedData = {
      dbtc_token_vault: miningData.dbtc_token_vault?.toString() || 'N/A',
      raydium_pool_state: miningData.raydium_pool_state?.toString() || 'N/A',
      mining_start_timestamp: convertBNToString(miningData.mining_start_timestamp),
      doge_btc_per_round: convertBNToString(miningData.doge_btc_per_round),
      last_slot: convertBNToString(miningData.last_slot),
      total_active_hashpower: convertBNToString(miningData.total_active_hashpower),
      total_active_electricity: convertBNToString(miningData.total_active_electricity),
      total_tokens_mined: convertBNToString(miningData.total_tokens_mined),
      dbtc_tokens_minted_per_hashpower: convertBNToString(miningData.dbtc_tokens_minted_per_hashpower),
      bump: miningData.bump,
      vault_auth_bump: miningData.vault_auth_bump,
      last_rate_update: convertBNToString(miningData.last_rate_update),
      current_dist_rate: convertBNToString(miningData.current_dist_rate),
      recent_price: convertBNToString(miningData.recent_price),
      track_price: convertBNToString(miningData.track_price),
      sol_for_pol: convertBNToString(miningData.sol_for_pol),
      lp_token_price_in_sol: convertBNToString(miningData.lp_token_price_in_sol)
    };
    
    // Process price history array
    if (miningData.price_history && Array.isArray(miningData.price_history)) {
      processedData.price_history = miningData.price_history.map((entry) => ({
        timestamp: convertBNToString(entry.timestamp),
        price: convertBNToString(entry.price)
      }));
    }
    
    // Process POL stats
    if (miningData.pol_stats) {
      processedData.pol_stats = {
        total_lp_burnt: convertBNToString(miningData.pol_stats.total_lp_burnt),
        total_sol_added: convertBNToString(miningData.pol_stats.total_sol_added),
        total_dbtc_added: convertBNToString(miningData.pol_stats.total_dbtc_added),
        lp_operations_count: convertBNToNumber(miningData.pol_stats.lp_operations_count)
      };
    }
    
    console.log('\n✅ Processed DogeBtcMining Data:');
    console.log(JSON.stringify(processedData, null, 2));
    
    return processedData;
    
  } catch (error) {
    console.error('❌ Error reading DogeBtcMining state:', error);
    console.error('Error details:', error.message);
    if (error.logs) {
      console.error('Logs:', error.logs);
    }
    return null;
  }
}

// Test with module ID 20 (the one we just created)
async function main() {
  console.log('🚀 Testing Module Config Reading...');
  
  // // Test the module we just created
  // const moduleResult = await getModuleConfig(1);
  
  // if (moduleResult && moduleResult.stats) {
  //   console.log('\n🎉 SUCCESS! Module Stats values are:');
  //   console.log(`Max HP: ${moduleResult.stats.max_hp}`);
  //   console.log(`Cooldown Sec: ${moduleResult.stats.cooldown_sec}`);
  //   console.log(`Max Reward: ${moduleResult.stats.max_reward}`);
  //   console.log(`Probability: ${moduleResult.stats.probability}`);
  //   console.log(`Power Consumption: ${moduleResult.stats.power_consumption}`);
  // } else {
  //   console.log('❌ Failed to read module stats');
  // }

  // Test DogeBtcMining state reading
  const miningResult = await getDogeBtcMining();
  
  if (miningResult) {
    console.log('\n🎉 SUCCESS! DogeBtcMining data retrieved');
    console.log(`Total Active Hashpower: ${miningResult.total_active_hashpower}`);
    console.log(`Total Active Electricity: ${miningResult.total_active_electricity}`);
    console.log(`Current Distribution Rate: ${miningResult.current_dist_rate}`);
    console.log(`SOL for POL: ${miningResult.sol_for_pol}`);
  } else {
    console.log('❌ Failed to read DogeBtcMining state');
  }
}

main().catch(console.error);