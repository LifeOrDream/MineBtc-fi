#!/usr/bin/env node

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { Keypair } from '@solana/web3.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration
const PROD_MOONBASE_DIR = path.join(__dirname, '..', 'prod_moonbase');
const WALLET_KEYPAIR_PATH = path.join(__dirname, '..', 'wallet-keypair.json');
const ANCHOR_TOML_PATH = path.join(PROD_MOONBASE_DIR, 'Anchor.toml');
const DEPLOYMENTS_DIR = path.join(__dirname, 'deployments');

// Program configurations
const PROGRAMS = {
  moon_base: {
    name: 'moon_base',
    keypairPath: path.join(PROD_MOONBASE_DIR, 'target', 'deploy', 'moon_base-keypair.json'),
    soPath: path.join(PROD_MOONBASE_DIR, 'target', 'deploy', 'moon_base.so'),
    libPath: path.join(PROD_MOONBASE_DIR, 'programs', 'moon_base', 'src', 'lib.rs')
  },
  moon_economy: {
    name: 'moon_economy',
    keypairPath: path.join(PROD_MOONBASE_DIR, 'target', 'deploy', 'moon_economy-keypair.json'),
    soPath: path.join(PROD_MOONBASE_DIR, 'target', 'deploy', 'moon_economy.so'),
    libPath: path.join(PROD_MOONBASE_DIR, 'programs', 'moon_economy', 'src', 'lib.rs')
  }
};

// Utility functions
function runCommand(command, cwd = PROD_MOONBASE_DIR) {
  console.log(`\x1b[36m🔧 Running: ${command}\x1b[0m`);
  try {
    const result = execSync(command, { 
      cwd, 
      stdio: 'pipe', 
      encoding: 'utf8' 
    });
    return result.trim();
  } catch (error) {
    console.error(`\x1b[31m❌ Command failed: ${command}\x1b[0m`);
    console.error(`\x1b[31m${error.message}\x1b[0m`);
    if (error.stdout) console.error(`\x1b[33mSTDOUT: ${error.stdout}\x1b[0m`);
    if (error.stderr) console.error(`\x1b[33mSTDERR: ${error.stderr}\x1b[0m`);
    throw error;
  }
}

function ensureDirectoryExists(dirPath) {
  if (!fs.existsSync(dirPath)) {
    fs.mkdirSync(dirPath, { recursive: true });
    console.log(`\x1b[32m✅ Created directory: ${dirPath}\x1b[0m`);
  }
}

function generateKeypair(outputPath) {
  console.log(`\x1b[33m🔑 Generating keypair: ${outputPath}\x1b[0m`);
  
  // Ensure the target/deploy directory exists
  ensureDirectoryExists(path.dirname(outputPath));
  
  // Generate keypair using solana-keygen
  runCommand(`solana-keygen new -o ${outputPath} --force --no-bip39-passphrase`);
  
  // Read the generated keypair to get the public key
  const keypairData = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  const publicKey = keypair.publicKey.toString();
  
  console.log(`\x1b[32m✅ Generated keypair with public key: ${publicKey}\x1b[0m`);
  return publicKey;
}

function updateAnchorToml(programAddresses) {
  console.log(`\x1b[33m📝 Updating Anchor.toml with new program addresses...\x1b[0m`);
  
  // Read cluster from config.json
  const configPath = path.join(__dirname, 'config.json');
  let cluster = 'localnet'; // default
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    cluster = config.network?.cluster || 'localnet';
  } catch (error) {
    console.log(`\x1b[33m⚠️  Could not read config.json, using default cluster: ${cluster}\x1b[0m`);
  }
  
  let anchorContent = fs.readFileSync(ANCHOR_TOML_PATH, 'utf8');
  
  // Update program addresses in the [programs.{cluster}] section
  for (const [programName, address] of Object.entries(programAddresses)) {
    // Look for the program in the specific cluster section
    const sectionRegex = new RegExp(`\\[programs\\.${cluster}\\]([\\s\\S]*?)(?=\\[|$)`, 'i');
    const programRegex = new RegExp(`^(\\s*)${programName}\\s*=\\s*"[^"]*"`, 'm');
    
    const sectionMatch = anchorContent.match(sectionRegex);
    if (sectionMatch) {
      const sectionContent = sectionMatch[1];
      const replacement = `$1${programName} = "${address}"`;
      
      if (sectionContent.match(programRegex)) {
        // Update existing program entry
        const newSectionContent = sectionContent.replace(programRegex, replacement);
        anchorContent = anchorContent.replace(sectionMatch[1], newSectionContent);
        console.log(`\x1b[32m  ✅ Updated ${programName} in [programs.${cluster}]: ${address}\x1b[0m`);
      } else {
        // Add new program entry to the section
        const newEntry = `\n${programName} = "${address}"`;
        const newSectionContent = sectionContent + newEntry;
        anchorContent = anchorContent.replace(sectionMatch[1], newSectionContent);
        console.log(`\x1b[32m  ✅ Added ${programName} to [programs.${cluster}]: ${address}\x1b[0m`);
      }
    } else {
      // Create the section if it doesn't exist
      const newSection = `\n[programs.${cluster}]\n${programName} = "${address}"\n`;
      anchorContent += newSection;
      console.log(`\x1b[32m  ✅ Created [programs.${cluster}] section and added ${programName}: ${address}\x1b[0m`);
    }
  }
  
  fs.writeFileSync(ANCHOR_TOML_PATH, anchorContent);
  console.log(`\x1b[32m✅ Anchor.toml updated successfully for cluster: ${cluster}\x1b[0m`);
}

function updateDeclareId(libPath, programAddress) {
  console.log(`\x1b[33m📝 Updating declare_id! in ${libPath}...\x1b[0m`);
  
  let libContent = fs.readFileSync(libPath, 'utf8');
  
  // Update declare_id! macro
  const declareIdRegex = /declare_id!\("([^"]+)"\);/;
  const replacement = `declare_id!("${programAddress}");`;
  
  if (libContent.match(declareIdRegex)) {
    libContent = libContent.replace(declareIdRegex, replacement);
    fs.writeFileSync(libPath, libContent);
    console.log(`\x1b[32m  ✅ Updated declare_id! to: ${programAddress}\x1b[0m`);
  } else {
    console.log(`\x1b[33m  ⚠️  Could not find declare_id! in ${libPath}\x1b[0m`);
  }
}

function checkPrerequisites() {
  console.log(`\x1b[36m🔍 Checking prerequisites...\x1b[0m`);
  
  // Check if wallet keypair exists
  if (!fs.existsSync(WALLET_KEYPAIR_PATH)) {
    throw new Error(`Wallet keypair not found at: ${WALLET_KEYPAIR_PATH}`);
  }
  
  // Check if prod_moonbase directory exists
  if (!fs.existsSync(PROD_MOONBASE_DIR)) {
    throw new Error(`prod_moonbase directory not found at: ${PROD_MOONBASE_DIR}`);
  }
  
  // Check if Anchor.toml exists
  if (!fs.existsSync(ANCHOR_TOML_PATH)) {
    throw new Error(`Anchor.toml not found at: ${ANCHOR_TOML_PATH}`);
  }
  
  console.log(`\x1b[32m✅ All prerequisites met\x1b[0m`);
}

function buildPrograms() {
  console.log(`\x1b[36m🏗️  Building programs...\x1b[0m`);
  runCommand('anchor build');
  console.log(`\x1b[32m✅ Programs built successfully\x1b[0m`);
}

function deployProgram(programConfig, walletPath) {
  console.log(`\x1b[36m🚀 Deploying ${programConfig.name}...\x1b[0m`);
  
  // Read cluster configuration from config.json
  const configPath = path.join(__dirname, 'config.json');
  let clusterUrl = 'http://127.0.0.1:8899'; // default localnet
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    clusterUrl = config.network?.rpc_url || clusterUrl;
    console.log(`\x1b[33m📍 Deploying to cluster: ${config.network?.cluster || 'localnet'} (${clusterUrl})\x1b[0m`);
  } catch (error) {
    console.log(`\x1b[33m⚠️  Could not read config.json, using default cluster URL: ${clusterUrl}\x1b[0m`);
  }
  
  const deployCommand = `solana program deploy ${programConfig.soPath} --program-id ${programConfig.keypairPath} --keypair ${walletPath} --url ${clusterUrl}`;
  
  try {
    const result = runCommand(deployCommand);
    console.log(`\x1b[32m✅ Successfully deployed ${programConfig.name}\x1b[0m`);
    return result;
  } catch (error) {
    console.error(`\x1b[31m❌ Failed to deploy ${programConfig.name}\x1b[0m`);
    throw error;
  }
}

function saveDeploymentInfo(programAddresses) {
  // Read cluster from config.json
  const configPath = path.join(__dirname, 'config.json');
  let cluster = 'localnet'; // default
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    cluster = config.network?.cluster || 'localnet';
  } catch (error) {
    console.log(`\x1b[33m⚠️  Could not read config.json, using default cluster: ${cluster}\x1b[0m`);
  }
  
  // Determine deployment file based on cluster
  const deploymentFileName = `${cluster}.json`;
  const deploymentPath = path.join(DEPLOYMENTS_DIR, deploymentFileName);
  
  // Ensure deployments directory exists
  ensureDirectoryExists(DEPLOYMENTS_DIR);
  
  // Read existing deployment file or create new one
  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
    try {
      deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    } catch (error) {
      console.log(`\x1b[33m⚠️  Could not parse existing ${deploymentFileName}, creating new one\x1b[0m`);
      deploymentData = {};
    }
  }
  
  // Update program IDs
  deploymentData.MOON_BASE_PROGRAM_ID = programAddresses.moon_base;
  deploymentData.MOON_ECONOMY_PROGRAM_ID = programAddresses.moon_economy;
  
  // Update deployment timestamp
  deploymentData.last_deployment = {
    timestamp: new Date().toISOString(),
    cluster: cluster,
    programs_deployed: Object.keys(programAddresses)
  };
  
  // Save updated deployment file
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
  console.log(`\x1b[32m✅ Updated deployment file: ${deploymentPath}\x1b[0m`);
  console.log(`\x1b[32m   📍 Cluster: ${cluster}\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_BASE_PROGRAM_ID: ${programAddresses.moon_base}\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_ECONOMY_PROGRAM_ID: ${programAddresses.moon_economy}\x1b[0m`);
}

async function main() {
  try {
    console.log(`\x1b[35m🚀 Starting automated program deployment...\x1b[0m`);
    console.log(`\x1b[35m==============================================\x1b[0m`);
    
    // Step 1: Check prerequisites
    checkPrerequisites();
    
    // Step 2: Generate keypairs and collect addresses
    console.log(`\x1b[36m\n📋 Step 1: Generating program keypairs...\x1b[0m`);
    const programAddresses = {};
    
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      programAddresses[programName] = generateKeypair(config.keypairPath);
    }
    
    // Step 3: Update Anchor.toml
    console.log(`\x1b[36m\n📋 Step 2: Updating configuration files...\x1b[0m`);
    updateAnchorToml(programAddresses);
    
    // Step 4: Update declare_id! in lib.rs files
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      updateDeclareId(config.libPath, programAddresses[programName]);
    }
    
    // Step 5: Build programs
    console.log(`\x1b[36m\n📋 Step 3: Building programs...\x1b[0m`);
    buildPrograms();
    
    // Step 6: Deploy programs
    console.log(`\x1b[36m\n📋 Step 4: Deploying programs...\x1b[0m`);
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      deployProgram(config, WALLET_KEYPAIR_PATH);
    }
    
    // Step 7: Save deployment information
    console.log(`\x1b[36m\n📋 Step 5: Saving deployment information...\x1b[0m`);
    saveDeploymentInfo(programAddresses);
    
    // Success summary
    console.log(`\x1b[32m\n🎉 DEPLOYMENT COMPLETED SUCCESSFULLY! 🎉\x1b[0m`);
    console.log(`\x1b[32m==========================================\x1b[0m`);
    console.log(`\x1b[36m📊 Deployed Programs:\x1b[0m`);
    
    for (const [programName, address] of Object.entries(programAddresses)) {
      console.log(`\x1b[32m  ✅ ${programName}: ${address}\x1b[0m`);
    }
    
    console.log(`\x1b[36m\n🔗 Next Steps:\x1b[0m`);
    console.log(`\x1b[33m  1. Run initialization scripts (1_mint_moondoge.js, 2_initialize_raydium.js, etc.)\x1b[0m`);
    console.log(`\x1b[33m  2. Test the deployed programs\x1b[0m`);
    console.log(`\x1b[33m  3. Update frontend configuration with new program IDs\x1b[0m`);
    
  } catch (error) {
    console.error(`\x1b[31m\n💥 DEPLOYMENT FAILED! 💥\x1b[0m`);
    console.error(`\x1b[31m========================\x1b[0m`);
    console.error(`\x1b[31mError: ${error.message}\x1b[0m`);
    
    console.log(`\x1b[33m\n🔧 Troubleshooting:\x1b[0m`);
    console.log(`\x1b[33m  1. Make sure Solana CLI is installed and configured\x1b[0m`);
    console.log(`\x1b[33m  2. Check that your wallet has sufficient SOL for deployment\x1b[0m`);
    console.log(`\x1b[33m  3. Verify that the localnet validator is running\x1b[0m`);
    console.log(`\x1b[33m  4. Ensure all file paths are correct\x1b[0m`);
    
    process.exit(1);
  }
}

// Run the deployment if this script is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;
