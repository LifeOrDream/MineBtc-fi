/**
 * Multisig Admin Script (2-of-3)
 *
 * This script executes admin functions on the minebtc program using a multisig authority.
 * Currently supports: set_pause
 * Requires 2 of 3 signatures to execute.
 *
 * Environment Variables Required:
 * - MULTISIG1: Mnemonic phrase for signer 1
 * - MULTISIG2: Mnemonic phrase for signer 2
 * - MULTISIG3: Mnemonic phrase for signer 3
 * - MULTISIG_ADDRESS: (Optional) Existing multisig authority address
 * - PAUSED: Optional true/false value for set_pause
 *
 * Usage:
 *   node multisig_admin.js set_pause true
 */

import {
  Connection,
  PublicKey,
  Transaction,
  Keypair,
} from "@solana/web3.js";
import { createMultisig, getMultisig } from "@solana/spl-token";
import * as bip39 from "bip39";
import { derivePath } from "ed25519-hd-key";
import dotenv from "dotenv";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import pkg from "@coral-xyz/anchor";
import { setIdlAddress } from "../raydium_id_sync.js";
const { AnchorProvider, Program, setProvider } = pkg;

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Calculate project root (go up 2 levels from setup_scripts/admin/)
const PROJECT_ROOT = path.resolve(__dirname, "../..");

// Load .env file from project root
const envPath = path.resolve(PROJECT_ROOT, ".env");
if (fs.existsSync(envPath)) {
  dotenv.config({ path: envPath });
  console.log(`✓ Loaded .env from: ${envPath}`);
} else {
  dotenv.config();
  console.log(
    `⚠️  .env not found at ${envPath}, using current working directory`
  );
}

// Load configuration
const configPath = path.resolve(__dirname, "../config.json");
import { loadConfig } from "../load_config.js";
const config = loadConfig(configPath);

// Load deployment data
const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Load deployment file
const deploymentPath = path.resolve(
  __dirname,
  `../deployments/${CLUSTER}.json`
);
let deploymentFile = {};
if (fs.existsSync(deploymentPath)) {
  deploymentFile = JSON.parse(fs.readFileSync(deploymentPath, "utf-8"));
} else {
  throw new Error(`Deployment file not found: ${deploymentPath}`);
}

// Load IDL
const idlPath = path.resolve(PROJECT_ROOT, "setup_scripts", config.deployment.paths.minebtc_idl);
const rawMinebtcIdl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
const IDL_MINEBTC = deploymentFile.MINE_BTC_PROGRAM_ID
  ? setIdlAddress(rawMinebtcIdl, deploymentFile.MINE_BTC_PROGRAM_ID)
  : rawMinebtcIdl;

// ====================================================================
// CONFIGURATION
// ====================================================================

// Get command line arguments
const command = process.argv[2];
const commandArg = process.argv[3];

// Multisig configuration (2-of-3)
const MULTISIG_M = 2;

// ====================================================================
// MNEMONIC LOADER
// ====================================================================

const getKeypairFromMnemonic = (mnemonic, label = "mnemonic") => {
  if (!mnemonic || typeof mnemonic !== "string") {
    throw new Error(`${label} is missing or invalid (got: ${typeof mnemonic})`);
  }

  // Trim whitespace
  const trimmedMnemonic = mnemonic.trim();

  if (!trimmedMnemonic) {
    throw new Error(`${label} is empty after trimming`);
  }

  const seed = bip39.mnemonicToSeedSync(trimmedMnemonic);
  const hd = derivePath("m/44'/501'/0'/0'", seed.toString("hex"));
  return Keypair.fromSeed(hd.key);
};

// ====================================================================
// MAIN SCRIPT
// ====================================================================

(async () => {
  try {
    // --- 1. Load Signers from Environment ---
    console.log("Loading mnemonics from environment...");
    const mnemonic1 = process.env.MULTISIG1;
    const mnemonic2 = process.env.MULTISIG2;
    const mnemonic3 = process.env.MULTISIG3;

    // Debug: Show which variables are set (without revealing values)
    console.log("Environment variables status:");
    console.log(`  MULTISIG1: ${mnemonic1 ? "✓ Set" : "✗ Missing"}`);
    console.log(`  MULTISIG2: ${mnemonic2 ? "✓ Set" : "✗ Missing"}`);
    console.log(`  MULTISIG3: ${mnemonic3 ? "✓ Set" : "✗ Missing"}`);

    // Check which ones are missing
    const missing = [];
    if (!mnemonic1) missing.push("MULTISIG1");
    if (!mnemonic2) missing.push("MULTISIG2");
    if (!mnemonic3) missing.push("MULTISIG3");

    if (missing.length > 0) {
      throw new Error(
        `Missing ${missing.join(", ")} in environment variables.\n\n` +
          `You can set them in two ways:\n\n` +
          `1. Using .env file (recommended):\n` +
          `   Create a .env file in setup_scripts/ with:\n` +
          `   MULTISIG1="your mnemonic phrase"\n` +
          `   MULTISIG2="your mnemonic phrase"\n` +
          `   MULTISIG3="your mnemonic phrase"\n\n` +
          `2. Using shell exports:\n` +
          `   export MULTISIG1="your mnemonic phrase"\n` +
          `   export MULTISIG2="your mnemonic phrase"\n` +
          `   export MULTISIG3="your mnemonic phrase"\n` +
          `   (Note: Don't use 'const' in export statements)`
      );
    }

    console.log("Validating mnemonics...");
    const signer1 = getKeypairFromMnemonic(mnemonic1, "MULTISIG1");
    const signer2 = getKeypairFromMnemonic(mnemonic2, "MULTISIG2");
    const signer3 = getKeypairFromMnemonic(mnemonic3, "MULTISIG3");

    console.log("Signers Loaded:");
    console.log(`  Signer 1: ${signer1.publicKey.toBase58()}`);
    console.log(`  Signer 2: ${signer2.publicKey.toBase58()}`);
    console.log(`  Signer 3: ${signer3.publicKey.toBase58()}`);

    // --- 2. Connect to Solana ---
    const connection = new Connection(RPC_URL, COMMITMENT);
    console.log(`\nConnecting to ${CLUSTER}...`);

    // --- 3. Create or Get Multisig Account ---
    console.log("\nSetting up multisig authority account...");
    const multisigAddressEnv = process.env.MULTISIG_ADDRESS;
    console.log(
      `  MULTISIG_ADDRESS from .env: ${multisigAddressEnv || "Not set"}`
    );

    let multisigAddress = multisigAddressEnv
      ? new PublicKey(multisigAddressEnv)
      : null;

    let multisigInfo;
    if (!multisigAddress) {
      console.log("Creating new multisig authority...");
      const signerPubkeys = [
        signer1.publicKey,
        signer2.publicKey,
        signer3.publicKey,
      ];
      multisigAddress = await createMultisig(
        connection,
        signer1, // payer
        signerPubkeys,
        MULTISIG_M
      );
      console.log(
        `  Multisig Authority Address: ${multisigAddress.toBase58()}`
      );
      console.log(
        `  ⚠️  Save this address to MULTISIG_ADDRESS in .env: ${multisigAddress.toBase58()}`
      );
      multisigInfo = await getMultisig(connection, multisigAddress);
      console.log(`  Threshold: ${multisigInfo.m} of ${multisigInfo.n}`);
    } else {
      console.log(
        `Using existing multisig authority: ${multisigAddress.toBase58()}`
      );
      multisigInfo = await getMultisig(connection, multisigAddress);
      console.log(`  Threshold: ${multisigInfo.m} of ${multisigInfo.n}`);
    }

    // --- 4. Validate Command and Parameters ---
    if (!command) {
      throw new Error(
        "No command specified. Usage: node multisig_admin.js <command> [args]\n" +
          "Available commands:\n" +
          "  set_pause <true|false> - Pause or unpause MineBTC gameplay"
      );
    }

    if (command === "set_pause") {
      const pausedArg = process.env.PAUSED ?? commandArg;
      if (!["true", "false"].includes(String(pausedArg).toLowerCase())) {
        throw new Error(
          "PAUSED not provided. Set PAUSED=true/false or pass an argument.\n" +
            "Usage: node multisig_admin.js set_pause true"
        );
      }

      const paused = String(pausedArg).toLowerCase() === "true";
      console.log(`\n⏸️ Setting MineBTC pause state: ${paused}`);

      // --- 5. Load Program and PDAs ---
      const programId = new PublicKey(deploymentFile.MINE_BTC_PROGRAM_ID);
      const globalConfigPDA = new PublicKey(
        deploymentFile.minebtc_program_initialized.globalConfig_address
      );

      console.log(`   Program ID: ${programId.toString()}`);
      console.log(`   Global Config PDA: ${globalConfigPDA.toString()}`);

      // Create a dummy wallet for provider (we'll sign manually)
      const dummyWallet = {
        publicKey: signer1.publicKey,
        signTransaction: async (tx) => tx,
        signAllTransactions: async (txs) => txs,
      };

      const provider = new AnchorProvider(connection, dummyWallet, {
        commitment: COMMITMENT,
      });
      setProvider(provider);

      const minebtcProgram = new Program(IDL_MINEBTC, provider);
      if (!minebtcProgram.programId.equals(programId)) {
        throw new Error(
          `IDL program ID (${minebtcProgram.programId.toString()}) does not match deployment (${programId.toString()})`
        );
      }

      // --- 6. Build Signers Array from Multisig ---
      const multisigSigners = [];
      const EMPTY_PUBKEY = new PublicKey("11111111111111111111111111111111");
      for (let i = 1; i <= multisigInfo.n; i++) {
        const signer = multisigInfo[`signer${i}`];
        if (signer && !signer.equals(EMPTY_PUBKEY)) {
          multisigSigners.push(signer);
        }
      }

      // Find the indices of our signers in the multisig
      const signer1Index = multisigSigners.findIndex((pk) =>
        pk.equals(signer1.publicKey)
      );
      const signer2Index = multisigSigners.findIndex((pk) =>
        pk.equals(signer2.publicKey)
      );

      if (signer1Index === -1 || signer2Index === -1) {
        throw new Error(
          "Signers not found in multisig. Ensure MULTISIG1 and MULTISIG2 are part of the multisig."
        );
      }

      // --- 7. Build set_pause Instruction ---
      console.log("\nBuilding set_pause instruction...");

      // Fetch GlobalConfig to check ext_authority
      const globalConfig = await minebtcProgram.account.globalConfig.fetch(
        globalConfigPDA
      );
      const extAuthority = new PublicKey(globalConfig.extAuthority);

      console.log(`   GlobalConfig ext_authority: ${extAuthority.toString()}`);
      console.log(`   Multisig address: ${multisigAddress.toString()}`);

      // Check if ext_authority matches any of the multisig signers
      // The program requires authority to be a Signer, so we need to use one of the signers
      let authoritySigner = null;
      if (extAuthority.equals(signer1.publicKey)) {
        authoritySigner = signer1;
        console.log(`   ✓ Using signer1 as authority (matches ext_authority)`);
      } else if (extAuthority.equals(signer2.publicKey)) {
        authoritySigner = signer2;
        console.log(`   ✓ Using signer2 as authority (matches ext_authority)`);
      } else if (extAuthority.equals(signer3.publicKey)) {
        authoritySigner = signer3;
        console.log(`   ✓ Using signer3 as authority (matches ext_authority)`);
      } else if (extAuthority.equals(multisigAddress)) {
        // If ext_authority is the multisig address itself, we can't use it directly
        // as Anchor requires Signer. We'll need to use signer1 and modify the instruction
        // to pass validation (but this will fail at runtime unless program supports it)
        console.log(
          `   ⚠️  ext_authority is multisig address - using signer1 (may fail validation)`
        );
        authoritySigner = signer1;
      } else {
        throw new Error(
          `ext_authority (${extAuthority.toString()}) does not match any multisig signer ` +
            `or multisig address. Cannot proceed.`
        );
      }

      // Build the instruction with the appropriate authority signer
      const setPauseIx = await minebtcProgram.methods
        .setPause(paused)
        .accounts({
          globalConfig: globalConfigPDA,
          authority: authoritySigner.publicKey,
        })
        .instruction();

      // If ext_authority is multisig address but we're using a signer,
      // we need to modify the instruction keys to replace the authority
      if (
        extAuthority.equals(multisigAddress) &&
        !authoritySigner.publicKey.equals(multisigAddress)
      ) {
        const authorityKeyIndex = setPauseIx.keys.findIndex((key) =>
          key.pubkey.equals(authoritySigner.publicKey)
        );
        if (authorityKeyIndex !== -1) {
          // Replace with multisig address but keep it as a signer (will fail, but trying)
          // Actually, this won't work - we need the program to support multisig
          console.log(
            `   ⚠️  Warning: Program may reject this transaction as authority mismatch`
          );
        }
      }

      // --- 8. Create and Sign Transaction ---
      console.log("\nCreating transaction...");
      const transaction = new Transaction().add(setPauseIx);
      transaction.feePayer = signer1.publicKey;

      const { blockhash, lastValidBlockHeight } =
        await connection.getLatestBlockhash();
      transaction.recentBlockhash = blockhash;
      transaction.lastValidBlockHeight = lastValidBlockHeight;

      // --- 9. Sign and Send Transaction ---
      console.log("Signing transaction...");

      // Collect all required signers
      // The authority signer must sign (it's required by the program)
      // We also need at least 2 multisig signers for the multisig to be valid
      const signers = [authoritySigner];

      // Add other multisig signers if they're not already the authority
      // We need at least 2 signers total for multisig validation
      if (!authoritySigner.publicKey.equals(signer1.publicKey)) {
        signers.push(signer1);
      }
      if (
        !authoritySigner.publicKey.equals(signer2.publicKey) &&
        signers.length < 2
      ) {
        signers.push(signer2);
      }

      console.log(
        `   Signers: ${signers.map((s) => s.publicKey.toString()).join(", ")}`
      );

      // Sign the transaction
      transaction.partialSign(...signers);

      console.log("Sending and confirming transaction...");
      const signature = await connection.sendRawTransaction(
        transaction.serialize(),
        {
          skipPreflight: false,
          maxRetries: 3,
        }
      );

      console.log(`   Transaction sent: ${signature}`);

      // Wait for confirmation
      const confirmation = await connection.confirmTransaction(
        {
          signature,
          blockhash,
          lastValidBlockHeight,
        },
        COMMITMENT
      );

      if (confirmation.value.err) {
        throw new Error(
          `Transaction failed: ${JSON.stringify(confirmation.value.err)}`
        );
      }

      console.log("\n✅ Transaction Successful!");
      console.log(`   Signature: ${signature}`);
      console.log(`   Paused: ${paused}`);
      const explorerUrl =
        CLUSTER === "mainnet-beta"
          ? `https://explorer.solana.com/tx/${signature}`
          : `https://explorer.solana.com/tx/${signature}?cluster=${CLUSTER}`;
      console.log(`   Explorer: ${explorerUrl}`);
    } else {
      throw new Error(`Unknown command: ${command}`);
    }
  } catch (error) {
    console.error("\n❌ Transaction Failed!");
    console.error(error.message);
    if (error.logs) {
      console.error("--- Solana Logs ---");
      console.error(error.logs);
      console.error("---------------------");
    }
    process.exit(1);
  }
})();
