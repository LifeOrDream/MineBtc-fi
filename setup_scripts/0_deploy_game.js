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

// Program configurations (MoonBase & MoonEconomy only)
const PROGRAMS = {
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
    // For moonbase/mooneconomy, capture IDL output from anchor idl build
    const idlOutput = runCommand(`anchor idl build -p ${programConfig.name}`, ROOT_DIR);
    
    // Extract JSON from output
    const jsonMatch = idlOutput.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      const idlPath = path.join(idlDir, `${programConfig.name}.json`);
      fs.writeFileSync(idlPath, jsonMatch[0]);
      console.log(`\x1b[32m  ✅ IDL extracted: ${idlPath}\x1b[0m`);
      return true;
    } else {
      console.log(`\x1b[33m⚠️  Could not extract IDL JSON from output\x1b[0m`);
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

function generateOrReadKeypair(outputPath) {
  ensureDirectoryExists(path.dirname(outputPath));
  
  // // Check if keypair already exists
  // if (fs.existsSync(outputPath)) {
  //   console.log(`\x1b[33m🔑 Reading existing keypair: ${outputPath}\x1b[0m`);
  //   const keypairData = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  //   const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  //   const publicKey = keypair.publicKey.toString();
  //   console.log(`\x1b[32m✅ Using existing keypair: ${publicKey}\x1b[0m`);
  //   return publicKey;
  // }
  
  console.log(`\x1b[33m🔑 Generating new keypair: ${outputPath}\x1b[0m`);
  runCommand(`solana-keygen new -o ${outputPath} --force --no-bip39-passphrase`);
  
  const keypairData = JSON.parse(fs.readFileSync(outputPath, 'utf8'));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  const publicKey = keypair.publicKey.toString();
  
  console.log(`\x1b[32m✅ Generated new keypair: ${publicKey}\x1b[0m`);
  return publicKey;
}

function updateAnchorToml(programAddresses) {
  console.log(`\x1b[33m📝 Updating Anchor.toml with new program addresses...\x1b[0m`);
  
  const configPath = path.join(__dirname, 'config.json');
  let cluster = 'localnet';
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    cluster = config.network?.cluster || 'localnet';
  } catch (error) {}
  
  let anchorContent = fs.readFileSync(ANCHOR_TOML_PATH, 'utf8');
  
  for (const [programName, address] of Object.entries(programAddresses)) {
    const sectionRegex = new RegExp(`\\[programs\\.${cluster}\\]([\\s\\S]*?)(?=\\[|$)`, 'i');
    const programRegex = new RegExp(`^(\\s*)${programName}\\s*=\\s*"[^"]*"`, 'm');
    
    const sectionMatch = anchorContent.match(sectionRegex);
    if (sectionMatch) {
      const sectionContent = sectionMatch[1];
      const replacement = `$1${programName} = "${address}"`;
      
      if (sectionContent.match(programRegex)) {
        const newSectionContent = sectionContent.replace(programRegex, replacement);
        anchorContent = anchorContent.replace(sectionMatch[1], newSectionContent);
        console.log(`\x1b[32m  ✅ Updated ${programName}: ${address}\x1b[0m`);
      } else {
        const newEntry = `\n${programName} = "${address}"`;
        const newSectionContent = sectionContent + newEntry;
        anchorContent = anchorContent.replace(sectionMatch[1], newSectionContent);
        console.log(`\x1b[32m  ✅ Added ${programName}: ${address}\x1b[0m`);
      }
    } else {
      const newSection = `\n[programs.${cluster}]\n${programName} = "${address}"\n`;
      anchorContent += newSection;
      console.log(`\x1b[32m  ✅ Created section and added ${programName}: ${address}\x1b[0m`);
    }
  }
  
  fs.writeFileSync(ANCHOR_TOML_PATH, anchorContent);
  console.log(`\x1b[32m✅ Anchor.toml updated\x1b[0m`);
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


function checkPrerequisites() {
  console.log(`\x1b[36m🔍 Checking prerequisites...\x1b[0m`);
  
  if (!fs.existsSync(WALLET_KEYPAIR_PATH)) {
    throw new Error(`Wallet keypair not found at: ${WALLET_KEYPAIR_PATH}`);
  }
  
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
  
  const programDir = path.join(programConfig.buildDir, 'programs', programConfig.name);
  runCommand('clear', programDir);

  const builtSoPath = path.join(programDir, 'target', 'deploy', `${programConfig.name}.so`);
  const targetSoPath = programConfig.soPath;

  ensureDirectoryExists(path.dirname(targetSoPath));
  if (fs.existsSync(builtSoPath)) {
    fs.copyFileSync(builtSoPath, targetSoPath);
    console.log(`\x1b[32m  ✅ Copied .so\x1b[0m`);
  }
  
  console.log(`\x1b[36m📝 Generating IDL...\x1b[0m`);
  try {
    const idlDir = path.join(ROOT_DIR, 'target', 'idl');
    ensureDirectoryExists(idlDir);
    
    const idlOutput = runCommand(`anchor idl build -p ${programConfig.name}`, ROOT_DIR);
    const jsonMatch = idlOutput.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      const idlPath = path.join(idlDir, `${programConfig.name}.json`);
      fs.writeFileSync(idlPath, jsonMatch[0]);
      console.log(`\x1b[32m  ✅ IDL generated\x1b[0m`);
    }
  } catch (error) {
    console.log(`\x1b[33m⚠️  IDL generation failed\x1b[0m`);
  }
  
  console.log(`\x1b[32m✅ ${programConfig.displayName} built\x1b[0m`);
}

function deployProgram(programConfig, walletPath) {
  console.log(`\x1b[36m🚀 Deploying ${programConfig.displayName}...\x1b[0m`);
  
  const configPath = path.join(__dirname, 'config.json');
  let clusterUrl = 'http://127.0.0.1:8899';
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    clusterUrl = config.network?.rpc_url || clusterUrl;
  } catch (error) {}
  
  const deployCommand = `solana program deploy ${programConfig.soPath} --program-id ${programConfig.keypairPath} --keypair ${walletPath} --url ${clusterUrl}`;
  
  try {
    runCommand(deployCommand);
    console.log(`\x1b[32m✅ Deployed ${programConfig.displayName}\x1b[0m`);
    
    // Update IDL with deployed address
    const keypairData = JSON.parse(fs.readFileSync(programConfig.keypairPath, 'utf8'));
    const deployedKeypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
    const deployedProgramId = deployedKeypair.publicKey.toString();
    
    const idlPath = path.join(ROOT_DIR, 'target', 'idl', `${programConfig.name}.json`);
    if (fs.existsSync(idlPath)) {
      const idlContent = JSON.parse(fs.readFileSync(idlPath, 'utf8'));
      idlContent.address = deployedProgramId;
      fs.writeFileSync(idlPath, JSON.stringify(idlContent, null, 2));
      console.log(`\x1b[32m   IDL updated: ${deployedProgramId}\x1b[0m`);
    }
  } catch (error) {
    console.error(`\x1b[31m❌ Deploy failed: ${error.message}\x1b[0m`);
    throw error;
  }
}

function saveDeploymentInfo(programAddresses) {
  const configPath = path.join(__dirname, 'config.json');
  let cluster = 'localnet';
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    cluster = config.network?.cluster || 'localnet';
  } catch (error) {}
  
  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);
  ensureDirectoryExists(DEPLOYMENTS_DIR);
  
  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
    try {
      deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    } catch (error) {}
  }
  
  deploymentData.MOON_BASE_PROGRAM_ID = programAddresses.moonbase;
  deploymentData.MOON_ECONOMY_PROGRAM_ID = programAddresses.mooneconomy;
  deploymentData.game_programs_deployment = {
    timestamp: new Date().toISOString(),
    cluster: cluster
  };
  
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
  console.log(`\x1b[32m✅ Saved deployment info\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_BASE: ${programAddresses.moonbase}\x1b[0m`);
  console.log(`\x1b[32m   🔗 MOON_ECONOMY: ${programAddresses.mooneconomy}\x1b[0m`);
}

async function main() {
  try {
    console.log(`\x1b[35m🚀 Starting automated program deployment...\x1b[0m`);
    console.log(`\x1b[35m==============================================\x1b[0m`);
    
    // Check for existing deployment
    const existingDeployment = getExistingDeployment();
    
    // Check if game programs are already deployed
    const allDeployed = existingDeployment && 
      isProgramDeployed('moonbase', existingDeployment) &&
      isProgramDeployed('mooneconomy', existingDeployment);
    
    if (allDeployed) {
      console.log(`\x1b[32m✅ Game programs already deployed!\x1b[0m`);
      console.log(`\x1b[36m   🔗 MOON_BASE: ${existingDeployment.MOON_BASE_PROGRAM_ID}\x1b[0m`);
      console.log(`\x1b[36m   🔗 MOON_ECONOMY: ${existingDeployment.MOON_ECONOMY_PROGRAM_ID}\x1b[0m`);
      console.log(`\x1b[36m\n📋 Regenerating IDL files...\x1b[0m`);
      
      extractIdlFromBinary(PROGRAMS.moonbase);
      extractIdlFromBinary(PROGRAMS.mooneconomy);
      
      console.log(`\x1b[32m\n✅ IDL files regenerated!\x1b[0m`);
      return;
    }
    
    checkPrerequisites();
    initSbpfEnvironment();
    
    console.log(`\x1b[36m\n📋 Step 1: Generating keypairs...\x1b[0m`);
    const programAddresses = {};
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      programAddresses[programName] = generateOrReadKeypair(config.keypairPath);
    }
    
    console.log(`\x1b[36m\n📋 Step 2: Updating source code...\x1b[0m`);
    for (const [programName, config] of Object.entries(PROGRAMS)) {
      updateDeclareId(config.libPath, programAddresses[programName]);
    }
    updateAnchorToml(programAddresses);
    
    console.log(`\x1b[36m\n📋 Step 3: Building programs...\x1b[0m`);
    buildProgram(PROGRAMS.moonbase);
    buildProgram(PROGRAMS.mooneconomy);
    
    console.log(`\x1b[36m\n📋 Step 4: Deploying programs...\x1b[0m`);
    deployProgram(PROGRAMS.moonbase, WALLET_KEYPAIR_PATH);
    deployProgram(PROGRAMS.mooneconomy, WALLET_KEYPAIR_PATH);
    
    console.log(`\x1b[36m\n📋 Step 5: Saving deployment...\x1b[0m`);
    saveDeploymentInfo(programAddresses);
    
    console.log(`\x1b[32m\n🎉 GAME PROGRAMS DEPLOYED! 🎉\x1b[0m`);
    console.log(`\x1b[32m==============================\x1b[0m`);
    for (const [programName, address] of Object.entries(programAddresses)) {
      console.log(`\x1b[32m  ✅ ${PROGRAMS[programName].displayName}: ${address}\x1b[0m`);
    }
    
    console.log(`\x1b[33m\n🔗 Next Steps:\x1b[0m`);
    console.log(`\x1b[33m  1. Run 1_init_mdoge_token.js\x1b[0m`);
    console.log(`\x1b[33m  2. Run 2_init_mdoge_SOL_pool.js\x1b[0m`);
    console.log(`\x1b[33m  3. Run 4_init_moonbase.js\x1b[0m`);
    console.log(`\x1b[33m  4. Run 5_init_mooneconomy.js\x1b[0m`);
    
  } catch (error) {
    console.error(`\x1b[31m\n💥 DEPLOYMENT FAILED! 💥\x1b[0m`);
    console.error(`\x1b[31m${error.message}\x1b[0m`);
    process.exit(1);
  }
}

// Run the deployment if this script is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;
