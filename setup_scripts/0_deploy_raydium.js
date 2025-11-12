#!/usr/bin/env node

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { Keypair } from '@solana/web3.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT_DIR = path.join(__dirname, '..');
const RAYDIUM_DIR = path.join(ROOT_DIR, 'raydium');
const WALLET_KEYPAIR_PATH = path.join(ROOT_DIR, 'wallet-keypair.json');
const RAYDIUM_DEPLOY_DIR = path.join(RAYDIUM_DIR, 'target', 'deploy');
const RAYDIUM_KEYPAIR_PATH = path.join(RAYDIUM_DEPLOY_DIR, 'raydium_cp_swap-keypair.json');
const RAYDIUM_SO_PATH = path.join(RAYDIUM_DEPLOY_DIR, 'raydium_cp_swap.so');
const RAYDIUM_LIB_PATH = path.join(RAYDIUM_DIR, 'programs', 'cp-swap', 'src', 'lib.rs');
const RAYDIUM_BUILD_DIR = path.join(RAYDIUM_DIR, 'programs', 'cp-swap');
const DEPLOYMENTS_DIR = path.join(__dirname, 'deployments');

function runCommand(command, cwd = ROOT_DIR) {
  console.log(`\x1b[36m🔧 Running: ${command}\x1b[0m`);
  try {
    const result = execSync(command, { cwd, stdio: 'pipe', encoding: 'utf8' });
    return result.trim();
  } catch (error) {
    console.error(`\x1b[31m❌ Command failed: ${command}\x1b[0m`);
    console.error(`\x1b[31m${error.message}\x1b[0m`);
    throw error;
  }
}

function ensureDirectoryExists(dirPath) {
  if (!fs.existsSync(dirPath)) {
    fs.mkdirSync(dirPath, { recursive: true });
  }
}

function getDeployerPublicKey() {
  const keypairData = JSON.parse(fs.readFileSync(WALLET_KEYPAIR_PATH, 'utf8'));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  return keypair.publicKey.toString();
}

function writeKeypair(secretKeyBytes) {
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
  fs.writeFileSync(RAYDIUM_KEYPAIR_PATH, JSON.stringify(Array.from(secretKeyBytes)));
}

function createNewKeypair() {
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
  const keypair = Keypair.generate();
  writeKeypair(keypair.secretKey);
  const programId = keypair.publicKey.toString();
  console.log(`\x1b[33m🔑 Generating Raydium keypair\x1b[0m`);
  console.log(`\x1b[32m   Program ID: ${programId}\x1b[0m`);
  return { programId, secretKey: keypair.secretKey };
}

function updateRaydiumCode(programId, deployerPubkey) {
  console.log(`\x1b[33m📝 Updating Raydium source code...\x1b[0m`);
  
  let libContent = fs.readFileSync(RAYDIUM_LIB_PATH, 'utf8');
  
  // Update declare_id
  libContent = libContent.replace(/declare_id!\("([^"]+)"\);/g, `declare_id!("${programId}");`);
  console.log(`\x1b[32m   ✅ Updated declare_id! to: ${programId}\x1b[0m`);
  
  // Update admin::ID for devnet
  const adminRegex = /(pub mod admin \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\(")([^"]+)("\);)/;
  if (libContent.match(adminRegex)) {
    libContent = libContent.replace(adminRegex, `$1${deployerPubkey}$3`);
    console.log(`\x1b[32m   ✅ Updated admin::ID (devnet): ${deployerPubkey}\x1b[0m`);
  }
  
  // Update create_pool_fee_reveiver::ID for devnet
  const feeRegex = /(pub mod create_pool_fee_reveiver \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\(")([^"]+)("\);)/;
  if (libContent.match(feeRegex)) {
    libContent = libContent.replace(feeRegex, `$1${deployerPubkey}$3`);
    console.log(`\x1b[32m   ✅ Updated create_pool_fee_reveiver::ID (devnet): ${deployerPubkey}\x1b[0m`);
  }
  
  fs.writeFileSync(RAYDIUM_LIB_PATH, libContent);
  
  // Also update Raydium Anchor.toml
  const anchorTomlPath = path.join(RAYDIUM_DIR, 'Anchor.toml');
  if (fs.existsSync(anchorTomlPath)) {
    let anchorContent = fs.readFileSync(anchorTomlPath, 'utf8');
    anchorContent = anchorContent.replace(/raydium_cp_swap\s*=\s*"([^"]+)"/g, `raydium_cp_swap = "${programId}"`);
    fs.writeFileSync(anchorTomlPath, anchorContent);
    console.log(`\x1b[32m   ✅ Updated Raydium Anchor.toml\x1b[0m`);
  }
}

function buildRaydium(secretKey) {
  console.log(`\x1b[36m🏗️  Building Raydium CP Swap...\x1b[0m`);
  console.log(`\x1b[36m🧹 Cleaning build cache...\x1b[0m`);
  try {
    runCommand('cargo clean', RAYDIUM_BUILD_DIR);
  } catch (error) {
    console.log(`\x1b[33m   ⚠️  Clean failed, continuing...\x1b[0m`);
  }
  
  runCommand('cargo build-sbf --features devnet -- --locked', RAYDIUM_BUILD_DIR);
  
  // Copy to expected location
  const builtSoPath = path.join(RAYDIUM_DIR, 'target', 'deploy', 'raydium_cp_swap.so');
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
  if (!fs.existsSync(builtSoPath)) {
    // fallback to sbpf release path
    const releaseSoPath = path.join(RAYDIUM_DIR, 'target', 'sbpf-solana-solana', 'release', 'raydium_cp_swap.so');
    if (fs.existsSync(releaseSoPath)) {
      fs.copyFileSync(releaseSoPath, RAYDIUM_SO_PATH);
    }
  } else if (builtSoPath !== RAYDIUM_SO_PATH) {
    fs.copyFileSync(builtSoPath, RAYDIUM_SO_PATH);
  }
  // Re-write keypair to prevent tooling from replacing it
  if (secretKey) {
    writeKeypair(secretKey);
  }

  if (fs.existsSync(RAYDIUM_SO_PATH)) {
    console.log(`\x1b[32m✅ Built and copied .so file\x1b[0m`);
  } else {
    throw new Error('Compiled .so file not found after build');
  }
}

function deployRaydium(programId) {
  console.log(`\x1b[36m🚀 Deploying Raydium CP Swap...\x1b[0m`);
  
  const configPath = path.join(__dirname, 'config.json');
  let clusterUrl = 'http://127.0.0.1:8899';
  
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    clusterUrl = config.network?.rpc_url || clusterUrl;
  } catch (error) {}
  
  const deployCommand = `solana program deploy ${RAYDIUM_SO_PATH} --program-id ${RAYDIUM_KEYPAIR_PATH} --url ${clusterUrl}`;
  runCommand(deployCommand);
  
  console.log(`\x1b[32m✅ Raydium deployed to: ${programId}\x1b[0m`);
}

function generateIdl(programId) {
  console.log(`\x1b[36m📝 Updating IDL...\x1b[0m`);
  const idlPath = path.join(ROOT_DIR, 'target', 'idl', 'raydium_cp_swap.json');
  
  if (!fs.existsSync(idlPath)) {
    console.log(`\x1b[31m❌ IDL template not found at ${idlPath}\x1b[0m`);
    return;
  }
  
  try {
    const idlContent = JSON.parse(fs.readFileSync(idlPath, 'utf8'));
    if (!idlContent.instructions || idlContent.instructions.length === 0) {
      throw new Error('IDL template is empty');
    }
    idlContent.address = programId;
    fs.writeFileSync(idlPath, JSON.stringify(idlContent, null, 2));
    console.log(`\x1b[32m✅ IDL address updated: ${programId}\x1b[0m`);
  } catch (error) {
    console.log(`\x1b[31m❌ Failed to update IDL: ${error.message}\x1b[0m`);
  }
}

function saveDeployment(programId) {
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
  
  deploymentData.RAYDIUM_CP_PROGRAM_ID = programId;
  deploymentData.raydium_last_deployment = {
    timestamp: new Date().toISOString(),
    cluster: cluster
  };
  
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
  console.log(`\x1b[32m✅ Saved to: ${deploymentPath}\x1b[0m`);
}

function resetArtifacts() {
  console.log(`\x1b[36m🧹 Resetting previous artifacts...\x1b[0m`);
  const targets = [
    path.join(RAYDIUM_DIR, 'target', 'sbpf-solana-solana'),
    path.join(RAYDIUM_DIR, 'target', 'sbf'),
    RAYDIUM_SO_PATH,
    RAYDIUM_KEYPAIR_PATH
  ];
  targets.forEach((target) => {
    if (fs.existsSync(target)) {
      fs.rmSync(target, { recursive: true, force: true });
    }
  });
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
}

async function main() {
  try {
    console.log(`\x1b[35m🚀 Raydium CP-Swap Program Deployment\x1b[0m`);
    console.log(`\x1b[35m======================================\x1b[0m`);
    
    const deployerPubkey = getDeployerPublicKey();
    console.log(`\x1b[36m📝 Deployer: ${deployerPubkey}\x1b[0m`);
    // return;
    
    resetArtifacts();
    const { programId, secretKey } = createNewKeypair();
    updateRaydiumCode(programId, deployerPubkey);
    buildRaydium(secretKey);
    writeKeypair(secretKey); // ensure deploy uses expected keypair
    deployRaydium(programId);
    generateIdl(programId);
    saveDeployment(programId);
    
    console.log(`\x1b[32m\n🎉 Raydium deployment complete!\x1b[0m`);
    console.log(`\x1b[32m   Program ID: ${programId}\x1b[0m`);
    console.log(`\x1b[33m\n📋 Next: Run 0_deploy_game.js for moonbase/mooneconomy\x1b[0m`);
    
  } catch (error) {
    console.error(`\x1b[31m💥 Deployment failed: ${error.message}\x1b[0m`);
    process.exit(1);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export default main;

