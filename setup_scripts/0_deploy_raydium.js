#!/usr/bin/env node

import { execSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";
import { Keypair } from "@solana/web3.js";
import { readJsonIfExists, setIdlAddress, syncRaydiumProgramId } from "./raydium_id_sync.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT_DIR = path.join(__dirname, "..");
const RAYDIUM_DIR = path.join(ROOT_DIR, "raydium");
const WALLET_KEYPAIR_PATH = path.join(ROOT_DIR, "devnet-wallet-keypair.json");
const RAYDIUM_DEPLOY_DIR = path.join(RAYDIUM_DIR, "target", "deploy");
const RAYDIUM_KEYPAIR_PATH = path.join(
  RAYDIUM_DEPLOY_DIR,
  "raydium_cp_swap-keypair.json"
);
const RAYDIUM_SO_PATH = path.join(RAYDIUM_DEPLOY_DIR, "raydium_cp_swap.so");
const RAYDIUM_LIB_PATH = path.join(
  RAYDIUM_DIR,
  "programs",
  "cp-swap",
  "src",
  "lib.rs"
);
const RAYDIUM_BUILD_DIR = path.join(RAYDIUM_DIR, "programs", "cp-swap");
const DEPLOYMENTS_DIR = path.join(__dirname, "deployments");

function shellEscape(value) {
  return `"${String(value).replace(/(["\\$`])/g, "\\$1")}"`;
}

function stageForSolanaCli(sourcePath, stagedName = path.basename(sourcePath)) {
  const stagingDir = path.join(os.tmpdir(), "minebtc-solana-cli");
  ensureDirectoryExists(stagingDir);
  const stagedPath = path.join(stagingDir, stagedName);
  fs.copyFileSync(sourcePath, stagedPath);
  return stagedPath;
}

function runCommand(command, cwd = ROOT_DIR) {
  console.log(`\x1b[36m🔧 Running: ${command}\x1b[0m`);
  try {
    const result = execSync(command, { cwd, stdio: "pipe", encoding: "utf8" });
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
  const keypairData = JSON.parse(fs.readFileSync(WALLET_KEYPAIR_PATH, "utf8"));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
  return keypair.publicKey.toString();
}

function writeKeypair(secretKeyBytes) {
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
  fs.writeFileSync(
    RAYDIUM_KEYPAIR_PATH,
    JSON.stringify(Array.from(secretKeyBytes))
  );
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
  const configPath = path.join(__dirname, "config.json");
  const config = readJsonIfExists(configPath) || { network: { cluster: "localnet" }, raydium: {} };
  syncRaydiumProgramId({
    rootDir: ROOT_DIR,
    setupDir: __dirname,
    config,
    programId,
    deployerPubkey,
    updateConfig: true,
  });
}

function buildRaydium(secretKey) {
  console.log(`\x1b[36m🏗️  Building Raydium CP Swap...\x1b[0m`);
  console.log(`\x1b[36m🧹 Cleaning build cache...\x1b[0m`);
  try {
    runCommand("cargo clean", RAYDIUM_BUILD_DIR);
  } catch (error) {
    console.log(`\x1b[33m   ⚠️  Clean failed, continuing...\x1b[0m`);
  }

  runCommand(
    "cargo build-sbf --features devnet -- --locked",
    RAYDIUM_BUILD_DIR
  );

  // Copy to expected location
  const builtSoPath = path.join(
    RAYDIUM_DIR,
    "target",
    "deploy",
    "raydium_cp_swap.so"
  );
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
  if (!fs.existsSync(builtSoPath)) {
    // fallback to sbpf release path
    const releaseSoPath = path.join(
      RAYDIUM_DIR,
      "target",
      "sbpf-solana-solana",
      "release",
      "raydium_cp_swap.so"
    );
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
    throw new Error("Compiled .so file not found after build");
  }
}

function deployRaydium(programId) {
  console.log(`\x1b[36m🚀 Deploying Raydium CP Swap...\x1b[0m`);

  const configPath = path.join(__dirname, "config.json");
  let clusterUrl = "http://127.0.0.1:8899";

  try {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
    clusterUrl = config.network?.rpc_url || clusterUrl;
  } catch (error) {}

  const stagedSoPath = stageForSolanaCli(
    RAYDIUM_SO_PATH,
    "raydium_cp_swap.so"
  );
  const stagedProgramKeypairPath = stageForSolanaCli(
    RAYDIUM_KEYPAIR_PATH,
    "raydium_cp_swap-keypair.json"
  );
  const stagedWalletPath = stageForSolanaCli(
    WALLET_KEYPAIR_PATH,
    "devnet-wallet-keypair.json"
  );

  const deployCommand = `solana program deploy ${shellEscape(
    stagedSoPath
  )} --program-id ${shellEscape(
    stagedProgramKeypairPath
  )} --keypair ${shellEscape(stagedWalletPath)} --url ${shellEscape(clusterUrl)}`;
  runCommand(deployCommand);

  console.log(`\x1b[32m✅ Raydium deployed to: ${programId}\x1b[0m`);
}

function generateIdl(programId) {
  console.log(`\x1b[36m📝 Updating IDL...\x1b[0m`);

  // Try multiple possible IDL locations
  const possibleIdlPaths = [
    path.join(ROOT_DIR, "target", "idl", "raydium_cp_swap.json"),
    path.join(RAYDIUM_DIR, "target", "idl", "raydium_cp_swap.json"),
  ];

  let idlPath = null;
  for (const pathToCheck of possibleIdlPaths) {
    if (fs.existsSync(pathToCheck)) {
      idlPath = pathToCheck;
      break;
    }
  }

  // If IDL doesn't exist, try generating it with anchor build
  if (!idlPath) {
    console.log(
      `\x1b[33m⚠️  IDL not found, attempting to generate with Anchor...\x1b[0m`
    );
    try {
      // Try anchor build to generate IDL (will build but we won't use the .so if deployment fails)
      // Note: anchor build will try to deploy but we'll ignore that error
      try {
        runCommand("anchor build", RAYDIUM_DIR);
      } catch (buildError) {
        // Build might fail on deploy step, but IDL might still be generated
        console.log(
          `\x1b[33m   Build/deploy step had issues (expected), checking for IDL...\x1b[0m`
        );
      }

      // Check again after anchor build attempt
      for (const pathToCheck of possibleIdlPaths) {
        if (fs.existsSync(pathToCheck)) {
          idlPath = pathToCheck;
          break;
        }
      }
    } catch (error) {
      console.log(
        `\x1b[33m⚠️  Anchor build failed (this is OK if using cargo build-sbf): ${error.message}\x1b[0m`
      );
    }
  }

  if (!idlPath) {
    console.log(
      `\x1b[33m⚠️  IDL not available (Raydium uses cargo build-sbf which doesn't generate IDL)\x1b[0m`
    );
    console.log(
      `\x1b[33m   This is expected and deployment will continue without IDL\x1b[0m`
    );
    return;
  }

  try {
    const idlContent = JSON.parse(fs.readFileSync(idlPath, "utf8"));
    if (!idlContent.instructions || idlContent.instructions.length === 0) {
      throw new Error("IDL template is empty");
    }
    setIdlAddress(idlContent, programId);
    fs.writeFileSync(idlPath, JSON.stringify(idlContent, null, 2));
    console.log(`\x1b[32m✅ IDL address updated: ${programId}\x1b[0m`);
  } catch (error) {
    console.log(`\x1b[33m⚠️  Failed to update IDL: ${error.message}\x1b[0m`);
  }
}

function saveDeployment(programId) {
  const configPath = path.join(__dirname, "config.json");
  let cluster = "localnet";

  try {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
    cluster = config.network?.cluster || "localnet";
  } catch (error) {}

  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);
  ensureDirectoryExists(DEPLOYMENTS_DIR);

  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
    try {
      deploymentData = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
    } catch (error) {}
  }

  deploymentData.RAYDIUM_CP_PROGRAM_ID = programId;
  deploymentData.raydium_last_deployment = {
    timestamp: new Date().toISOString(),
    cluster: cluster,
  };

  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
  console.log(`\x1b[32m✅ Saved to: ${deploymentPath}\x1b[0m`);
}

function resetArtifacts() {
  console.log(`\x1b[36m🧹 Resetting previous artifacts...\x1b[0m`);
  const targets = [
    path.join(RAYDIUM_DIR, "target", "sbpf-solana-solana"),
    path.join(RAYDIUM_DIR, "target", "sbf"),
    RAYDIUM_SO_PATH,
    RAYDIUM_KEYPAIR_PATH,
  ];
  targets.forEach((target) => {
    if (fs.existsSync(target)) {
      fs.rmSync(target, { recursive: true, force: true });
    }
  });
  ensureDirectoryExists(RAYDIUM_DEPLOY_DIR);
}

function checkIfDeployed() {
  const configPath = path.join(__dirname, "config.json");
  let cluster = "localnet";

  try {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
    cluster = config.network?.cluster || "localnet";
  } catch (error) {}

  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);

  if (!fs.existsSync(deploymentPath)) {
    return null;
  }

  try {
    const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
    const programId = deploymentData.RAYDIUM_CP_PROGRAM_ID;

    if (!programId) {
      return null;
    }

    // Check if program exists on-chain
    let clusterUrl = "http://127.0.0.1:8899";

    try {
      const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
      clusterUrl = config.network?.rpc_url || clusterUrl;
    } catch (error) {}

    try {
      const stagedWalletPath = stageForSolanaCli(
        WALLET_KEYPAIR_PATH,
        "devnet-wallet-keypair.json"
      );
      const checkCommand = `solana program show ${programId} --keypair ${shellEscape(
        stagedWalletPath
      )} --url ${shellEscape(clusterUrl)}`;
      execSync(checkCommand, { stdio: "pipe", encoding: "utf8" });
      return programId;
    } catch (error) {
      // Program not found on-chain, return null
      return null;
    }
  } catch (error) {
    return null;
  }
}

async function main() {
  try {
    console.log(`\x1b[35m🚀 Raydium CP-Swap Program Deployment\x1b[0m`);
    console.log(`\x1b[35m======================================\x1b[0m`);

    const deployerPubkey = getDeployerPublicKey();
    console.log(`\x1b[36m📝 Deployer: ${deployerPubkey}\x1b[0m`);
    // return;

    // Check if program is already deployed
    const existingProgramId = checkIfDeployed();

    if (existingProgramId) {
      console.log(
        `\x1b[33m⚠️  Program already deployed at: ${existingProgramId}\x1b[0m`
      );
      console.log(`\x1b[36m📝 Syncing Raydium ids + regenerating IDL...\x1b[0m`);

      updateRaydiumCode(existingProgramId, deployerPubkey);
      generateIdl(existingProgramId);

      console.log(`\x1b[32m\n✅ Raydium id sync complete!\x1b[0m`);
      console.log(`\x1b[32m   Program ID: ${existingProgramId}\x1b[0m`);
      return;
    }

    // New deployment flow
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
    console.log(`\x1b[33m\n📋 Next: Run 0_deploy_game.js for minebtc\x1b[0m`);
  } catch (error) {
    console.error(`\x1b[31m💥 Deployment failed: ${error.message}\x1b[0m`);
    process.exit(1);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  main();
}

export default main;
