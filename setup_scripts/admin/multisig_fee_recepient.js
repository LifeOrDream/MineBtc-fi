/**
 * Multisig WSOL (Wrapped SOL) Transfer Script (2-of-3)
 *
 * This script transfers WSOL from a token account owned by a multisig
 * authority to a recipient's token account.
 * Requires 2 of 3 signatures to execute.
 *
 * Environment Variables Required:
 * - MULTISIG1: Mnemonic phrase for signer 1
 * - MULTISIG2: Mnemonic phrase for signer 2
 * - MULTISIG3: Mnemonic phrase for signer 3
 * - MULTISIG_ADDRESS: (Optional) Existing multisig authority address
 * - RECIPIENT_OWNER_ADDRESS: The *wallet address* of the person you are sending to
 * - TRANSFER_AMOUNT_SOL: (Optional) Amount to transfer in SOL (default: 1.0 SOL)
 */

import {
  Connection,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  createMultisig,
  getMultisig,
  createTransferCheckedInstruction,
  getOrCreateAssociatedTokenAccount,
  getAccount,
  NATIVE_MINT,
} from "@solana/spl-token";
import * as bip39 from "bip39";
import { derivePath } from "ed25519-hd-key";
import dotenv from "dotenv";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

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
  console.log(`⚠️  .env not found at ${envPath}, using current working directory`);
}

// Load configuration
const configPath = path.resolve(__dirname, "../config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

// Load deployment data
const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// ====================================================================
// CONFIGURATION
// ====================================================================

// The recipient's *main wallet address*
const RECIPIENT_OWNER_ADDRESS = process.env.RECIPIENT_OWNER_ADDRESS
  ? new PublicKey(process.env.RECIPIENT_OWNER_ADDRESS)
  : null;

// The amount to transfer (in SOL, converted to lamports)
const TRANSFER_AMOUNT_SOL = process.env.TRANSFER_AMOUNT_SOL
  ? parseFloat(process.env.TRANSFER_AMOUNT_SOL)
  : 1.0; // Default: 1 SOL

const TRANSFER_AMOUNT_LAMPORTS = BigInt(
  Math.floor(TRANSFER_AMOUNT_SOL * LAMPORTS_PER_SOL)
);

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
      console.log(`  Multisig Authority Address: ${multisigAddress.toBase58()}`);
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

    // --- 4. Get/Create WSOL Token Accounts ---
    if (!RECIPIENT_OWNER_ADDRESS) {
      throw new Error(
        "RECIPIENT_OWNER_ADDRESS not set in .env. Set it to the destination *wallet* address."
      );
    }
    console.log("\nSetting up WSOL token accounts...");

    // Get the multisig's own WSOL token account
    const sourceAta = await getOrCreateAssociatedTokenAccount(
      connection,
      signer1, // Payer (pays rent if account needs to be created)
      NATIVE_MINT, // Mint (WSOL)
      multisigAddress, // Owner (the multisig authority)
      true // allowOwnerOffCurve = true (multisig is a PDA, so yes)
    );
    console.log(`  Multisig WSOL Vault: ${sourceAta.address.toBase58()}`);

    // Get the recipient's WSOL token account
    const destinationAta = await getOrCreateAssociatedTokenAccount(
      connection,
      signer1, // Payer (pays rent if account needs to be created)
      NATIVE_MINT, // Mint (WSOL)
      RECIPIENT_OWNER_ADDRESS // Owner (the recipient's wallet)
    );
    console.log(`  Recipient WSOL Account: ${destinationAta.address.toBase58()}`);

    // --- 5. Check Multisig WSOL Balance ---
    console.log("\nChecking multisig WSOL balance...");
    const sourceAccountInfo = await getAccount(connection, sourceAta.address);
    const balance = sourceAccountInfo.amount;
    const balanceSOL = Number(balance) / LAMPORTS_PER_SOL;

    console.log(`  Current Balance: ${balanceSOL} WSOL`);

    if (balance < TRANSFER_AMOUNT_LAMPORTS) {
      console.error("\n❌ INSUFFICIENT FUNDS ❌");
      console.error(`  Your multisig's WSOL vault has ${balanceSOL} WSOL.`);
      console.error(`  You are trying to send ${TRANSFER_AMOUNT_SOL} WSOL.`);
      console.error("\n  HOW TO FIX:");
      console.error(`  1. Get the address of your multisig's WSOL vault:`);
      console.error(`     ${sourceAta.address.toBase58()}`);
      console.error(
        `  2. Send ${TRANSFER_AMOUNT_SOL} SOL (or more) to this address.`
      );
      console.error(
        `  3. Your wallet (Phantom, etc.) will automatically wrap it into WSOL.`
      );
      console.error("  4. Rerun this script.");
      throw new Error("Insufficient WSOL balance.");
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
    const signer1Index = multisigSigners.findIndex(
      (pk) => pk.equals(signer1.publicKey)
    );
    const signer2Index = multisigSigners.findIndex(
      (pk) => pk.equals(signer2.publicKey)
    );

    if (signer1Index === -1 || signer2Index === -1) {
      throw new Error(
        "Signers not found in multisig. Ensure MULTISIG1 and MULTISIG2 are part of the multisig."
      );
    }

    // --- 7. Build WSOL Transfer Instruction ---
    console.log("\nBuilding WSOL transfer instruction...");
    console.log(
      `  Amount: ${TRANSFER_AMOUNT_SOL} WSOL (${TRANSFER_AMOUNT_LAMPORTS} lamports)`
    );

    // For multisig WSOL transfers, signers must be in the order they appear in the multisig account
    const multiSigners = [];
    if (signer1Index < signer2Index) {
      multiSigners.push(signer1.publicKey, signer2.publicKey);
    } else {
      multiSigners.push(signer2.publicKey, signer1.publicKey);
    }

    const transferInstruction = createTransferCheckedInstruction(
      sourceAta.address, // Source (the multisig's token vault)
      NATIVE_MINT, // Mint (WSOL)
      destinationAta.address, // Destination
      multisigAddress, // Owner (the multisig authority account)
      TRANSFER_AMOUNT_LAMPORTS, // Amount
      9, // Decimals (WSOL always has 9)
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
    console.log(`   Transferred: ${TRANSFER_AMOUNT_SOL} WSOL`);
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
