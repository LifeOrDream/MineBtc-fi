#!/usr/bin/env node

import { Connection, PublicKey } from '@solana/web3.js';
import { BorshAccountsCoder } from '@coral-xyz/anchor';
import fs from 'fs';
import path from 'path';

// Load IDL
const idlPath = path.join(process.cwd(), '../prod_moonbase/target/idl/moon_base.json');
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

// Test with module ID 20 (the one we just created)
async function main() {
  console.log('🚀 Testing Module Config Reading...');
  
  // Test the module we just created
  const result = await getModuleConfig(1);
  
  if (result && result.stats) {
    console.log('\n🎉 SUCCESS! Stats values are:');
    console.log(`Max HP: ${result.stats.max_hp}`);
    console.log(`Cooldown Sec: ${result.stats.cooldown_sec}`);
    console.log(`Max Reward: ${result.stats.max_reward}`);
    console.log(`Probability: ${result.stats.probability}`);
    console.log(`Power Consumption: ${result.stats.power_consumption}`);
  } else {
    console.log('❌ Failed to read stats');
  }
}

main().catch(console.error); 

//  anchor upgrade target/deploy/moon_base.so --program-id 3VWMZMjJZm5jjwWUZM1i8JPGYRMVtFuJTc9SUasyDVSB --provider.cluster localnet