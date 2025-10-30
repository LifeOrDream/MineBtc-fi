#!/usr/bin/env node

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { Keypair } from '@solana/web3.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Configuration
const ROOT_DIR = path.join(__dirname, '..');
const RAYDIUM_DIR = path.join(ROOT_DIR, 'raydium');
const WALLET_KEYPAIR_PATH = path.join(ROOT_DIR, 'wallet-keypair.json');
const ANCHOR_TOML_PATH = path.join(ROOT_DIR, 'Anchor.toml');
const DEPLOYMENTS_DIR = path.join(__dirname, 'deployments');

// Program configurations
const PROGRAMS = {
  raydium_cp_swap: {
    name: 'raydium_cp_swap',
    displayName: 'Raydium CP Swap',
    keypairPath: path.join(ROOT_DIR, 'raydium', 'target', 'deploy', 'raydium_cp_swap-keypair.json'),
    soPath: path.join(ROOT_DIR, 'raydium', 'target', 'deploy', 'raydium_cp_swap.so'),
    libPath: path.join(ROOT_DIR, 'raydium', 'programs', 'cp-swap', 'src', 'lib.rs'),
    buildDir: RAYDIUM_DIR,
    needsAdminUpdate: true
  },
  moonbase: {
    name: 'moonbase',
    displayName: 'MoonBase',
    keypairPath: path.join(ROOT_DIR, 'target', 'deploy', 'moonbase-keypair.json'),
    soPath: path.join(ROOT_DIR, 'target', 'deploy', 'moonbase.so'),
    libPath: path.join(ROOT_DIR, 'programs', 'moonbase', 'src', 'lib.rs'),
    buildDir: ROOT_DIR
  },
  mooneconomy: {
    name: 'mooneconomy',
    displayName: 'MoonEconomy',
    keypairPath: path.join(ROOT_DIR, 'target', 'deploy', 'mooneconomy-keypair.json'),
    soPath: path.join(ROOT_DIR, 'target', 'deploy', 'mooneconomy.so'),
    libPath: path.join(ROOT_DIR, 'programs', 'mooneconomy', 'src', 'lib.rs'),
    buildDir: ROOT_DIR
  }
};

// Utility functions
function runCommand(command, cwd = ROOT_DIR) {
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

function getExistingDeployment() {
  const configPath = path.join(__dirname, 'config.json');
  let cluster = 'localnet';
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    cluster = config.network?.cluster || 'localnet';
  } catch (error) {
    // Use default
  }
  
  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);
  
  if (fs.existsSync(deploymentPath)) {
    try {
      return JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    } catch (error) {
      return null;
    }
  }
  
  return null;
}

function isProgramDeployed(programName, deploymentData) {
  if (!deploymentData) return false;
  
  const programIdKey = {
    'raydium_cp_swap': 'RAYDIUM_CP_PROGRAM_ID',
    'moonbase': 'MOON_BASE_PROGRAM_ID',
    'mooneconomy': 'MOON_ECONOMY_PROGRAM_ID'
  }[programName];
  
  return deploymentData[programIdKey] && deploymentData[programIdKey] !== '';
}

function extractIdlFromBinary(programConfig) {
  console.log(`\x1b[36m📝 Extracting IDL for ${programConfig.displayName}...\x1b[0m`);
  
  const idlDir = path.join(ROOT_DIR, 'target', 'idl');
  ensureDirectoryExists(idlDir);
  
  try {
    if (programConfig.name === 'raydium_cp_swap') {
      // For Raydium, build IDL from its workspace
      runCommand('anchor idl build', programConfig.buildDir);
      const idlSourcePath = path.join(programConfig.buildDir, 'target', 'idl', 'raydium_cp_swap.json');
      const idlTargetPath = path.join(idlDir, 'raydium_cp_swap.json');
      if (fs.existsSync(idlSourcePath)) {
        fs.copyFileSync(idlSourcePath, idlTargetPath);
        console.log(`\x1b[32m  ✅ IDL extracted: ${idlTargetPath}\x1b[0m`);
        return true;
      }
    } else {
      // For moonbase/mooneconomy, capture IDL output from anchor idl build
      const idlOutput = runCommand(`anchor idl build -p ${programConfig.name}`, ROOT_DIR);
      
      // Extract JSON from output (it's printed after the compilation messages)
      const jsonMatch = idlOutput.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        const idlPath = path.join(idlDir, `${programConfig.name}.json`);
        fs.writeFileSync(idlPath, jsonMatch[0]);
        console.log(`\x1b[32m  ✅ IDL extracted: ${idlPath}\x1b[0m`);
        return true;
      } else {
        console.log(`\x1b[33m⚠️  Could not extract IDL JSON from output\x1b[0m`);
      }
    }
  } catch (error) {
    console.log(`\x1b[33m⚠️  IDL extraction failed: ${error.message}\x1b[0m`);
  }
  
  return false;
}

function getDeployerPublicKey() {
  const keypairData = JSON.parse(fs.readFileSync(WALLET_KEYPAIR_PATH, 'utf8'));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  return keypair.publicKey.toString();
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
  
  // Also update Raydium's Anchor.toml
  const raydiumAnchorToml = path.join(RAYDIUM_DIR, 'Anchor.toml');
  if (fs.existsSync(raydiumAnchorToml) && programAddresses.raydium_cp_swap) {
    let raydiumContent = fs.readFileSync(raydiumAnchorToml, 'utf8');
    const raydiumProgramRegex = /raydium_cp_swap\s*=\s*"([^"]+)"/g;
    raydiumContent = raydiumContent.replace(raydiumProgramRegex, `raydium_cp_swap = "${programAddresses.raydium_cp_swap}"`);
    fs.writeFileSync(raydiumAnchorToml, raydiumContent);
    console.log(`\x1b[32m✅ Updated Raydium Anchor.toml: ${programAddresses.raydium_cp_swap}\x1b[0m`);
  }
}

function updateDeclareId(libPath, programAddress) {
  console.log(`\x1b[33m📝 Updating declare_id! in ${libPath}...\x1b[0m`);
  
  let libContent = fs.readFileSync(libPath, 'utf8');
  
  // Update declare_id! macro (handles both devnet and non-devnet versions)
  const declareIdRegex = /declare_id!\("([^"]+)"\);/g;
  let updated = false;
  
  libContent = libContent.replace(declareIdRegex, (match) => {
    updated = true;
    return `declare_id!("${programAddress}");`;
  });
  
  if (updated) {
    fs.writeFileSync(libPath, libContent);
    console.log(`\x1b[32m  ✅ Updated declare_id! to: ${programAddress}\x1b[0m`);
  } else {
    console.log(`\x1b[33m  ⚠️  Could not find declare_id! in ${libPath}\x1b[0m`);
  }
}

function updateRaydiumAdmins(libPath, deployerPubkey) {
  console.log(`\x1b[33m📝 Updating Raydium admin addresses in ${libPath}...\x1b[0m`);
  
  let libContent = fs.readFileSync(libPath, 'utf8');
  
  // Update admin::ID pubkey (devnet) - this controls who can create AMM configs
  const adminModuleRegex = /pub mod admin \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\("([^"]+)"\);/;
  const adminMatch = libContent.match(adminModuleRegex);
  
  if (adminMatch) {
    const oldAdminId = adminMatch[1];
    libContent = libContent.replace(
      adminModuleRegex,
      (match) => match.replace(oldAdminId, deployerPubkey)
    );
    console.log(`\x1b[32m  ✅ Updated admin::ID (devnet): ${oldAdminId} → ${deployerPubkey}\x1b[0m`);
  } else {
    console.log(`\x1b[33m  ⚠️  Could not find admin::ID (devnet) pattern\x1b[0m`);
  }
  
  // Update create_pool_fee_reveiver::ID pubkey (devnet) - this is where pool creation fees are sent
  const feeReceiverModuleRegex = /pub mod create_pool_fee_reveiver \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\("([^"]+)"\);/;
  const feeReceiverMatch = libContent.match(feeReceiverModuleRegex);
  
  if (feeReceiverMatch) {
    const oldFeeReceiverId = feeReceiverMatch[1];
    libContent = libContent.replace(
      feeReceiverModuleRegex,
      (match) => match.replace(oldFeeReceiverId, deployerPubkey)
    );
    console.log(`\x1b[32m  ✅ Updated create_pool_fee_reveiver::ID (devnet): ${oldFeeReceiverId} → ${deployerPubkey}\x1b[0m`);
  } else {
    console.log(`\x1b[33m  ⚠️  Could not find create_pool_fee_reveiver::ID (devnet) pattern\x1b[0m`);
  }
  
  fs.writeFileSync(libPath, libContent);
  console.log(`\x1b[32m  ✅ Raydium admin configuration updated for localnet/devnet\x1b[0m`);
}

function checkPrerequisites() {
  console.log(`\x1b[36m🔍 Checking prerequisites...\x1b[0m`);
  
  // Check if wallet keypair exists
  if (!fs.existsSync(WALLET_KEYPAIR_PATH)) {
    throw new Error(`Wallet keypair not found at: ${WALLET_KEYPAIR_PATH}`);
  }
  
  // Check if Raydium directory exists
  if (!fs.existsSync(RAYDIUM_DIR)) {
    throw new Error(`Raydium directory not found at: ${RAYDIUM_DIR}`);
  }
  
  // Check if main Anchor.toml exists
  if (!fs.existsSync(ANCHOR_TOML_PATH)) {
    throw new Error(`Anchor.toml not found at: ${ANCHOR_TOML_PATH}`);
  }
  
  console.log(`\x1b[32m✅ All prerequisites met\x1b[0m`);
}

function initSbpfEnvironment() {
  console.log(`\x1b[36m🛠  Initializing SBPF toolchain...\x1b[0m`);
  try {
    // Clean previous SBPF artifacts to avoid mismatched toolchain errors
    runCommand('rm -rf target/sbpf-solana-solana');
  } catch {}
  // Note: Ensure Solana CLI is v2.x (matching crates) before building
}

function cleanBuild(programConfig) {
  console.log(`\x1b[36m🧹 Cleaning ${programConfig.displayName}...\x1b[0m`);
  try {
    runCommand('anchor clean', programConfig.buildDir);
  } catch (error) {
    console.log(`\x1b[33m⚠️  Clean failed (may not exist yet), continuing...\x1b[0m`);
  }
}

function buildProgram(programConfig) {
  console.log(`\x1b[36m🏗️  Building ${programConfig.displayName}...\x1b[0m`);
  
  if (programConfig.name === 'raydium_cp_swap') {
    // Build Raydium in its own workspace to avoid conflicts
    // Use cargo build-sbf directly instead of anchor build
    const raydiumProgramPath = path.join(programConfig.buildDir, 'programs', 'cp-swap');
    runCommand('cargo build-sbf --features devnet -- --locked', raydiumProgramPath);
    
    // Copy the built .so file to the expected location
    const builtSoPath = path.join(raydiumProgramPath, 'target', 'deploy', 'raydium_cp_swap.so');
    const targetSoPath = programConfig.soPath;
    
    ensureDirectoryExists(path.dirname(targetSoPath));
    if (fs.existsSync(builtSoPath)) {
      fs.copyFileSync(builtSoPath, targetSoPath);
      console.log(`\x1b[32m  ✅ Copied .so to: ${targetSoPath}\x1b[0m`);
    }
    
    // Generate IDL for Raydium
    console.log(`\x1b[36m📝 Generating IDL for ${programConfig.displayName}...\x1b[0m`);
    try {
      runCommand('anchor idl build', programConfig.buildDir);
      const idlSourcePath = path.join(programConfig.buildDir, 'target', 'idl', 'raydium_cp_swap.json');
      const idlTargetPath = path.join(ROOT_DIR, 'target', 'idl', 'raydium_cp_swap.json');
      if (fs.existsSync(idlSourcePath)) {
        ensureDirectoryExists(path.dirname(idlTargetPath));
        fs.copyFileSync(idlSourcePath, idlTargetPath);
        console.log(`\x1b[32m  ✅ IDL copied to: ${idlTargetPath}\x1b[0m`);
      }
    } catch (error) {
      console.log(`\x1b[33m⚠️  IDL generation failed, continuing...\x1b[0m`);
    }
  } else {
    // Build each Anchor program in its own crate to avoid workspace conflicts
    const programDir = path.join(programConfig.buildDir, 'programs', programConfig.name);
    // Use cargo build-sbf directly (same as Anchor under the hood)
    runCommand('cargo build-sbf -- --locked', programDir);

    // Copy the built .so file to the expected root target location
    const builtSoPath = path.join(programDir, 'target', 'deploy', `${programConfig.name}.so`);
    const targetSoPath = programConfig.soPath;

    ensureDirectoryExists(path.dirname(targetSoPath));
    if (fs.existsSync(builtSoPath)) {
      fs.copyFileSync(builtSoPath, targetSoPath);
      console.log(`\x1b[32m  ✅ Copied .so to: ${targetSoPath}\x1b[0m`);
    }
    
    // Generate IDL for Anchor programs by capturing output
    console.log(`\x1b[36m📝 Generating IDL for ${programConfig.displayName}...\x1b[0m`);
    try {
      const idlDir = path.join(ROOT_DIR, 'target', 'idl');
      ensureDirectoryExists(idlDir);
      
      // Run anchor idl build and capture JSON output
      const idlOutput = runCommand(`anchor idl build -p ${programConfig.name}`, ROOT_DIR);
      
      // Extract JSON from output
      const jsonMatch = idlOutput.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        const idlPath = path.join(idlDir, `${programConfig.name}.json`);
        fs.writeFileSync(idlPath, jsonMatch[0]);
        console.log(`\x1b[32m  ✅ IDL generated at: ${idlPath}\x1b[0m`);
      } else {
        console.log(`\x1b[33m⚠️  Could not extract IDL JSON from output\x1b[0m`);
      }
    } catch (error) {
      console.log(`\x1b[33m⚠️  IDL generation failed, continuing...\x1b[0m`);
    }
  }
  
  console.log(`\x1b[32m✅ ${programConfig.displayName} built successfully\x1b[0m`);
}

function deployProgram(programConfig, walletPath) {
  console.log(`\x1b[36m🚀 Deploying ${programConfig.displayName}...\x1b[0m`);
  
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
    console.log(`\x1b[32m✅ Successfully deployed ${programConfig.displayName}\x1b[0m`);
    
    // Update IDL with actual deployed program ID
    console.log(`\x1b[36m📝 Updating IDL with deployed address for ${programConfig.displayName}...\x1b[0m`);
    try {
      const idlDir = path.join(ROOT_DIR, 'target', 'idl');
      ensureDirectoryExists(idlDir);
      const idlPath = path.join(idlDir, `${programConfig.name}.json`);
      
      // Read the deployed program ID from keypair
      const keypairData = JSON.parse(fs.readFileSync(programConfig.keypairPath, 'utf8'));
      const deployedKeypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
      const deployedProgramId = deployedKeypair.publicKey.toString();
      
      if (fs.existsSync(idlPath)) {
        // Update the address field in the IDL
        const idlContent = JSON.parse(fs.readFileSync(idlPath, 'utf8'));
        idlContent.address = deployedProgramId;
        fs.writeFileSync(idlPath, JSON.stringify(idlContent, null, 2));
        console.log(`\x1b[32m  ✅ IDL updated with deployed address: ${deployedProgramId}\x1b[0m`);
      }
    } catch (error) {
      console.log(`\x1b[33m⚠️  IDL update failed: ${error.message}\x1b[0m`);
    }
    
    return result;
  } catch (error) {
    console.error(`\x1b[31m❌ Failed to deploy ${programConfig.displayName}\x1b[0m`);
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
  deploymentData.RAYDIUM_CP_PROGRAM_ID = programAddresses.raydium_cp_swap;
  deploymentData.MOON_BASE_PROGRAM_ID = programAddresses.moonbase;
  deploymentData.MOON_ECONOMY_PROGRAM_ID = programAddresses.mooneconomy;
  
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
  console.log(`\x1b[32m   🔗 RAYDIUM_CP_PROGRAM_ID: ${programAddresses.raydium_cp_swap}\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_BASE_PROGRAM_ID: ${programAddresses.moonbase}\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_ECONOMY_PROGRAM_ID: ${programAddresses.mooneconomy}\x1b[0m`);
}

async function main() {
  try {
    console.log(`\x1b[35m🚀 Starting automated program deployment...\x1b[0m`);
    console.log(`\x1b[35m==============================================\x1b[0m`);
    
    // Check for existing deployment
    const existingDeployment = getExistingDeployment();
    
    // Check if all programs are already deployed
    const allDeployed = existingDeployment && 
      isProgramDeployed('raydium_cp_swap', existingDeployment) &&
      isProgramDeployed('moonbase', existingDeployment) &&
      isProgramDeployed('mooneconomy', existingDeployment);
    
    if (allDeployed) {
      console.log(`\x1b[32m✅ All programs already deployed!\x1b[0m`);
      console.log(`\x1b[36m   🔗 RAYDIUM_CP_PROGRAM_ID: ${existingDeployment.RAYDIUM_CP_PROGRAM_ID}\x1b[0m`);
      console.log(`\x1b[36m   🔗 MOON_BASE_PROGRAM_ID: ${existingDeployment.MOON_BASE_PROGRAM_ID}\x1b[0m`);
      console.log(`\x1b[36m   🔗 MOON_ECONOMY_PROGRAM_ID: ${existingDeployment.MOON_ECONOMY_PROGRAM_ID}\x1b[0m`);
      console.log(`\x1b[36m\n📋 Regenerating IDL files from deployed programs...\x1b[0m`);
      
      // Extract IDL from existing .so files
      extractIdlFromBinary(PROGRAMS.raydium_cp_swap);
      extractIdlFromBinary(PROGRAMS.moonbase);
      extractIdlFromBinary(PROGRAMS.mooneconomy);
      
      console.log(`\x1b[32m\n✅ IDL files regenerated successfully!\x1b[0m`);
      return;
    }
    
    // Step 1: Check prerequisites
    checkPrerequisites();
    // Reinitialize SBPF platform tools once per run
    initSbpfEnvironment();
    
    // Get deployer wallet public key
    const deployerPubkey = getDeployerPublicKey();
    console.log(`\x1b[36m📝 Deployer wallet: ${deployerPubkey}\x1b[0m`);
    
    // Step 2: Generate keypairs and collect addresses
    console.log(`\x1b[36m\n📋 Step 1: Generating program keypairs...\x1b[0m`);
    const programAddresses = {};
    
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      programAddresses[programName] = generateKeypair(config.keypairPath);
    }
    
    // Step 3: Update configuration files
    console.log(`\x1b[36m\n📋 Step 2: Updating configuration files...\x1b[0m`);
    
    // Update Raydium admin addresses first (before updating declare_id)
    const raydiumConfig = PROGRAMS.raydium_cp_swap;
    if (raydiumConfig.needsAdminUpdate) {
      updateRaydiumAdmins(raydiumConfig.libPath, deployerPubkey);
    }
    
    // Update declare_id! in all lib.rs files
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      updateDeclareId(config.libPath, programAddresses[programName]);
    }
    
    // Update Anchor.toml
    updateAnchorToml(programAddresses);
    
    // Step 4: Build programs (in order: Raydium first, then game programs)
    console.log(`\x1b[36m\n📋 Step 3: Building programs...\x1b[0m`);
    
    // Build Raydium first as it's a dependency
    buildProgram(PROGRAMS.raydium_cp_swap);
    
    // Then build game programs
    buildProgram(PROGRAMS.moonbase);
    buildProgram(PROGRAMS.mooneconomy);
    
    // Step 5: Deploy programs (in order: Raydium first)
    console.log(`\x1b[36m\n📋 Step 4: Deploying programs...\x1b[0m`);
    
    // Deploy Raydium first
    deployProgram(PROGRAMS.raydium_cp_swap, WALLET_KEYPAIR_PATH);
    
    // Then deploy game programs
    deployProgram(PROGRAMS.moonbase, WALLET_KEYPAIR_PATH);
    deployProgram(PROGRAMS.mooneconomy, WALLET_KEYPAIR_PATH);
    
    // Step 6: Save deployment information
    console.log(`\x1b[36m\n📋 Step 5: Saving deployment information...\x1b[0m`);
    saveDeploymentInfo(programAddresses);
    
    // Success summary
    console.log(`\x1b[32m\n🎉 DEPLOYMENT COMPLETED SUCCESSFULLY! 🎉\x1b[0m`);
    console.log(`\x1b[32m==========================================\x1b[0m`);
    console.log(`\x1b[36m📊 Deployed Programs:\x1b[0m`);
    
    for (const [programName, address] of Object.entries(programAddresses)) {
      const displayName = PROGRAMS[programName].displayName;
      console.log(`\x1b[32m  ✅ ${displayName}: ${address}\x1b[0m`);
    }
    
    console.log(`\x1b[36m\n📝 Admin Configuration:\x1b[0m`);
    console.log(`\x1b[33m  Raydium admin: ${deployerPubkey}\x1b[0m`);
    console.log(`\x1b[33m  Pool fee receiver: ${deployerPubkey}\x1b[0m`);
    
    console.log(`\x1b[36m\n🔗 Next Steps:\x1b[0m`);
    console.log(`\x1b[33m  1. Run 1_init_mdoge_token.js to create the game token\x1b[0m`);
    console.log(`\x1b[33m  2. Run 2_init_mdoge_SOL_pool.js to create the Raydium pool\x1b[0m`);
    console.log(`\x1b[33m  3. Run 3_init_moonbase.js and 4_init_moonEconomy.js\x1b[0m`);
    console.log(`\x1b[33m  4. Update frontend configuration with new program IDs\x1b[0m`);
    
  } catch (error) {
    console.error(`\x1b[31m\n💥 DEPLOYMENT FAILED! 💥\x1b[0m`);
    console.error(`\x1b[31m========================\x1b[0m`);
    console.error(`\x1b[31mError: ${error.message}\x1b[0m`);
    if (error.stack) {
      console.error(`\x1b[90m${error.stack}\x1b[0m`);
    }
    
    console.log(`\x1b[33m\n🔧 Troubleshooting:\x1b[0m`);
    console.log(`\x1b[33m  1. Make sure Solana CLI and Anchor are installed\x1b[0m`);
    console.log(`\x1b[33m  2. Check that your wallet has sufficient SOL for deployment\x1b[0m`);
    console.log(`\x1b[33m  3. Verify that the validator is running (solana-test-validator)\x1b[0m`);
    console.log(`\x1b[33m  4. Ensure all dependencies are installed (cargo build-sbf)\x1b[0m`);
    
    process.exit(1);
  }
}

// Run the deployment if this script is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;
