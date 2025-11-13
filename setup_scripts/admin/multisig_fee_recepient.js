/**
 * Multisig Token Transfer Script (2-of-3)
 * 
 * This script transfers tokens from a multisig-controlled token account.
 * Requires 2 of 3 signatures to execute the transfer.
 * 
 * Environment Variables Required:
 * - MULTISIG1: Mnemonic phrase for signer 1
 * - MULTISIG2: Mnemonic phrase for signer 2  
 * - MULTISIG3: Mnemonic phrase for signer 3
 * - MULTISIG_ADDRESS: (Optional) Existing multisig account address
 * - MULTISIG_TOKEN_ACCOUNT: (Optional) Token account owned by multisig
 * - RECIPIENT_TOKEN_ACCOUNT: Destination token account address
 * - TRANSFER_AMOUNT: (Optional) Amount to transfer in smallest units (default: 100000000)
 * 
 * Usage:
 * 1. Set up .env file with MULTISIG1, MULTISIG2, MULTISIG3 mnemonics
 * 2. Set RECIPIENT_TOKEN_ACCOUNT to destination address
 * 3. Run: node setup_scripts/admin/multisig_fee_recepient.js
 * 
 * On first run, save the MULTISIG_ADDRESS and MULTISIG_TOKEN_ACCOUNT to .env
 */

import {
  Connection,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
  SystemProgram,
} from "@solana/web3.js";
import {
  createTransferCheckedInstruction,
  getMint,
  getAccount,
  createMultisig,
  getMultisig,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import * as bip39 from "bip39";
import { derivePath } from "ed25519-hd-key";
import dotenv from "dotenv";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

dotenv.config();

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, "../config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

// Load deployment data
const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;
const deploymentDir = path.resolve(__dirname, "../deployments");
const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);

let deploymentFile = {};
if (fs.existsSync(deploymentPath)) {
  deploymentFile = JSON.parse(fs.readFileSync(deploymentPath, "utf-8"));
} else {
  throw new Error(`Deployment file not found: ${deploymentPath}`);
}

const TOKEN_MINT_ADDRESS = new PublicKey(
  deploymentFile.dbtc_mint_created.mint_address
);

// ====================================================================
// CONFIGURATION
// ====================================================================

// The token account owned by the multisig (source)
// Set this to your multisig token account address, or leave null to create one
const MULTISIG_TOKEN_ACCOUNT = process.env.MULTISIG_TOKEN_ACCOUNT
  ? new PublicKey(process.env.MULTISIG_TOKEN_ACCOUNT)
  : null;

// The destination token account address
const RECIPIENT_TOKEN_ACCOUNT = process.env.RECIPIENT_TOKEN_ACCOUNT
  ? new PublicKey(process.env.RECIPIENT_TOKEN_ACCOUNT)
  : null;

// The amount to transfer (in token units, will be converted to smallest unit)
const TRANSFER_AMOUNT = process.env.TRANSFER_AMOUNT
  ? BigInt(process.env.TRANSFER_AMOUNT)
  : 100_000_000n; // 100 tokens (assuming 6 decimals)

// Multisig configuration (2-of-3)
const MULTISIG_M = 2;
const MULTISIG_N = 3;

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
  
//   if (!bip39.validateMnemonic(trimmedMnemonic)) {
//     throw new Error(`Invalid ${label} phrase. Make sure it's a valid BIP39 mnemonic (12 or 24 words)`);
//   }
  
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

    // Get mint info
    // const mintInfo = await getMint(connection, TOKEN_MINT_ADDRESS);
    // console.log(
    //   `Token Mint: ${TOKEN_MINT_ADDRESS.toBase58()} (Decimals: ${mintInfo.decimals})`
    // );

    // --- 3. Create or Get Multisig Account ---
    console.log("\nSetting up multisig account...");
    
    // Check if multisig already exists
    let multisigAddress = process.env.MULTISIG_ADDRESS
      ? new PublicKey(process.env.MULTISIG_ADDRESS)
      : null;

    if (!multisigAddress) {
      console.log("Creating new multisig account...");
      const signerPubkeys = [signer1.publicKey, signer2.publicKey, signer3.publicKey];
      multisigAddress = await createMultisig(
        connection,
        signer1, // payer
        signerPubkeys,
        MULTISIG_M
      );
      console.log(`  Multisig Address: ${multisigAddress.toBase58()}`);
      console.log(
        `  ⚠️  Save this address to MULTISIG_ADDRESS in .env: ${multisigAddress.toBase58()}`
      );
    } else {
      console.log(`Using existing multisig: ${multisigAddress.toBase58()}`);
      const multisigInfo = await getMultisig(connection, multisigAddress);
      console.log(`  Threshold: ${multisigInfo.m} of ${multisigInfo.n}`);
    }

    // --- 4. Get Token Account Owned by Multisig ---
    console.log("\nSetting up token account...");
    const multisigTokenAccount =
      MULTISIG_TOKEN_ACCOUNT ||
      getAssociatedTokenAddressSync(TOKEN_MINT_ADDRESS, multisigAddress);

    if (!MULTISIG_TOKEN_ACCOUNT) {
      console.log(
        `  Multisig Token Account: ${multisigTokenAccount.toBase58()}`
      );
      console.log(
        `  ⚠️  Save this address to MULTISIG_TOKEN_ACCOUNT in .env: ${multisigTokenAccount.toBase58()}`
      );

      // Check if account exists
      try {
        await getAccount(connection, multisigTokenAccount);
        console.log("  Token account already exists");
      } catch (error) {
        throw new Error(
          `Token account does not exist. Create it first or fund the multisig account.`
        );
      }
    }

    // Check balance
    const sourceAccount = await getAccount(connection, multisigTokenAccount);
    console.log(
      `  Current Balance: ${Number(sourceAccount.amount) / 10 ** mintInfo.decimals} tokens`
    );

    if (sourceAccount.amount < TRANSFER_AMOUNT) {
      throw new Error(
        `Insufficient balance. Required: ${Number(TRANSFER_AMOUNT) / 10 ** mintInfo.decimals}, Available: ${Number(sourceAccount.amount) / 10 ** mintInfo.decimals}`
      );
    }

    // --- 5. Get Recipient Token Account ---
    if (!RECIPIENT_TOKEN_ACCOUNT) {
      throw new Error(
        "RECIPIENT_TOKEN_ACCOUNT not set in .env. Set it to the destination token account address."
      );
    }

    // Verify recipient account exists
    try {
      await getAccount(connection, RECIPIENT_TOKEN_ACCOUNT);
    } catch (error) {
      throw new Error(
        `Recipient token account does not exist: ${RECIPIENT_TOKEN_ACCOUNT.toBase58()}`
      );
    }

    console.log(`  Recipient: ${RECIPIENT_TOKEN_ACCOUNT.toBase58()}`);

    // --- 6. Get Multisig Info and Verify Signers ---
    const multisigInfo = await getMultisig(connection, multisigAddress);
    
    // Find the indices of our signers in the multisig
    const signer1Index = multisigInfo.signers.findIndex(
      (pk) => pk.equals(signer1.publicKey)
    );
    const signer2Index = multisigInfo.signers.findIndex(
      (pk) => pk.equals(signer2.publicKey)
    );

    if (signer1Index === -1 || signer2Index === -1) {
      throw new Error(
        "Signers not found in multisig. Ensure MULTISIG1 and MULTISIG2 are part of the multisig."
      );
    }

    // --- 7. Build Transfer Instruction ---
    console.log("\nBuilding transfer instruction...");
    console.log(
      `  Amount: ${Number(TRANSFER_AMOUNT) / 10 ** mintInfo.decimals} tokens`
    );

    // Create transfer instruction with multisig as authority
    // Signers must be in the order they appear in the multisig account
    const multiSigners = [];
    if (signer1Index < signer2Index) {
      multiSigners.push(signer1.publicKey, signer2.publicKey);
    } else {
      multiSigners.push(signer2.publicKey, signer1.publicKey);
    }

    const transferInstruction = createTransferCheckedInstruction(
      multisigTokenAccount, // source
      TOKEN_MINT_ADDRESS, // mint
      RECIPIENT_TOKEN_ACCOUNT, // destination
      multisigAddress, // authority (multisig)
      TRANSFER_AMOUNT, // amount
      mintInfo.decimals, // decimals
      multiSigners // multisig signers in order
    );

    // --- 8. Create and Sign Transaction ---
    console.log("\nCreating transaction...");
    const transaction = new Transaction().add(transferInstruction);
    transaction.feePayer = signer1.publicKey;

    const { blockhash } = await connection.getLatestBlockhash();
    transaction.recentBlockhash = blockhash;

    // --- 9. Sign and Send Transaction ---
    console.log("Signing transaction (2 of 3 signers)...");
    // Signers must match the order in multiSigners array
    const signers = signer1Index < signer2Index 
      ? [signer1, signer2] 
      : [signer2, signer1];

    console.log("Sending and confirming transaction...");
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      signers,
      { commitment: COMMITMENT }
    );

    console.log("\n✅ Transaction Successful!");
    console.log(`   Signature: ${signature}`);
    const explorerUrl =
      CLUSTER === "mainnet-beta"
        ? `https://explorer.solana.com/tx/${signature}`
        : `https://explorer.solana.com/tx/${signature}?cluster=${CLUSTER}`;
    console.log(`   Explorer: ${explorerUrl}`);
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
