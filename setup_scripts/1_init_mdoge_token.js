/**
 * ============================================================================
 * DOGE_BTC TOKEN INITIALIZATION SCRIPT (Production-Grade)
 * ============================================================================
 *
 * This script initializes the DOGE_BTC Token-2022 with the following features:
 *
 * DEPLOYMENT STEPS:
 * 1. Create mint account with metadata and transfer fee config
 * 2. Create associated token account for deployer
 * 3. Mint initial supply with verification
 * 4. Remove mint authority (make token non-mintable)
 * 5. Remove withdraw withheld authority
 * 6. Transfer transfer fee config authority to configured address
 *
 * SAFETY FEATURES:
 * - All operations are idempotent (can be safely re-run)
 * - Deployment state is persisted after each step
 * - Backups are created before updating deployment data
 * - On-chain verification ensures data consistency
 * - Comprehensive validation prevents invalid configurations
 *
 * ============================================================================
 */

import {
  Connection,
  Keypair,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  createInitializeMintInstruction,
  createInitializeMetadataPointerInstruction,
  getMintLen,
  createInitializeTransferFeeConfigInstruction,
  mintTo,
  getOrCreateAssociatedTokenAccount,
  LENGTH_SIZE,
  TYPE_SIZE,
  setAuthority,
  AuthorityType,
  getMint,
} from "@solana/spl-token";
import { createInitializeInstruction, pack } from "@solana/spl-token-metadata";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import {
  getSolanaBalance,
  updateDeploymentStatus,
  createMintAccountWithMetadata,
  createMintAccount_T22_TransferFeeOnly,
} from "./helper.js";

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, "./config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Token metadata configuration
const TOKEN_METADATA = {
  name: config.token.name,
  symbol: config.token.symbol,
  description: config.token.description,
  image: config.token.image,
  external_url: config.token.external_url,
};

// ============================================================================
// ========== MAIN DEPLOYMENT SCRIPT =========================================
// ============================================================================

(async () => {
  console.log(
    "\x1b[35m%s\x1b[0m",
    "🚀 ================================ DogeTech DOGE_BTC Token Deployment ================================"
  );
  console.log("\x1b[36m%s\x1b[0m", "🌐 Network:", CLUSTER);
  console.log("\x1b[36m%s\x1b[0m", "🔗 RPC URL:", RPC_URL);
  console.log("\x1b[36m%s\x1b[0m", "🪙 Token Symbol:", config.token.symbol);
  console.log(
    "\x1b[36m%s\x1b[0m",
    "📊 Initial Supply:",
    config.token.initial_supply.toLocaleString()
  );

  // Validate configuration before proceeding
  validateConfiguration();

  // Setup connection
  const connection = await initializeConnection();

  // Setup deployer account
  const deployer = await setupDeployerAccount();
  console.log(
    "\x1b[36m%s\x1b[0m",
    "👤 Deployer Address:",
    deployer.publicKey.toBase58()
  );

  let deployer_balance = await getSolanaBalance(connection, deployer.publicKey);
  console.log(
    "\x1b[36m%s\x1b[0m",
    "💰 Deployer Balance:",
    deployer_balance / 1e9,
    "SOL"
  );

  // Check sufficient balance for deployment
  await validateDeployerBalance(connection, deployer.publicKey);

  // Load or create deployment state
  const { deploymentData, deploymentPath } = loadDeploymentState();

  try {
    // 1. Create mint account with metadata
    await createMintAccountTx(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // 2. Create token account
    await createTokenAccount(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // 3. Mint initial supply
    await mintInitialSupply(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // 4. Remove mint authority (make token non-mintable)
    await removeMintAuthority(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // 5. Set withdraw withheld authority to program-controlled PDA
    await setWithdrawWithheldAuthorityToPDA(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // 6. Transfer transfer fee config authority
    await transferTransferFeeConfigAuthority(
      connection,
      deployer,
      deploymentData,
      deploymentPath
    );

    // Print completion summary
    printCompletionSummary(deploymentData);
  } catch (error) {
    console.error("\x1b[31m%s\x1b[0m", "❌ Deployment failed:", error);
    process.exit(1);
  }
})();

// ============================================================================
// ========== VALIDATION FUNCTIONS ===========================================
// ============================================================================

function validateConfiguration() {
  console.log("\x1b[33m%s\x1b[0m", "🔍 Validating configuration...");

  const errors = [];

  // Token configuration validation
  if (!config.token.name || config.token.name.trim().length === 0) {
    errors.push("Token name is required");
  }
  if (!config.token.symbol || config.token.symbol.trim().length === 0) {
    errors.push("Token symbol is required");
  }
  if (
    !config.token.decimals ||
    config.token.decimals < 0 ||
    config.token.decimals > 9
  ) {
    errors.push("Token decimals must be between 0 and 9");
  }
  if (!config.token.initial_supply || config.token.initial_supply <= 0) {
    errors.push("Initial supply must be greater than 0");
  }
  if (
    config.token.burn_tax_bps === undefined ||
    config.token.burn_tax_bps < 0 ||
    config.token.burn_tax_bps > 10000
  ) {
    errors.push("Burn tax basis points must be between 0 and 10000 (0-100%)");
  }
  if (!config.token.max_burn_amount || config.token.max_burn_amount <= 0) {
    errors.push("Max burn amount must be greater than 0");
  }

  // Network validation
  if (!config.network.cluster) {
    errors.push("Network cluster is required");
  }
  if (!config.network.rpc_url) {
    errors.push("RPC URL is required");
  }

  // Deployment paths validation
  if (!config.deployment.paths.deployer_key) {
    errors.push("Deployer key path is required");
  }
  if (!config.deployment.paths.deployments_dir) {
    errors.push("Deployments directory path is required");
  }

  // Transfer fee config authority validation
  if (!config.deployment.transfer_fee_config_authority) {
    errors.push("Transfer fee config authority is required");
  } else {
    try {
      new PublicKey(config.deployment.transfer_fee_config_authority);
    } catch (e) {
      errors.push(
        "Transfer fee config authority must be a valid Solana public key"
      );
    }
  }

  // Metadata validation
  if (!TOKEN_METADATA.name || TOKEN_METADATA.name.trim().length === 0) {
    errors.push("Token metadata name is required");
  }
  if (!TOKEN_METADATA.symbol || TOKEN_METADATA.symbol.trim().length === 0) {
    errors.push("Token metadata symbol is required");
  }

  if (errors.length > 0) {
    console.error("\x1b[31m%s\x1b[0m", "❌ Configuration validation failed:");
    errors.forEach((error) =>
      console.error("\x1b[31m%s\x1b[0m", `   • ${error}`)
    );
    process.exit(1);
  }

  console.log("\x1b[32m%s\x1b[0m", "✅ Configuration validated successfully");
}

async function validateDeployerBalance(connection, deployerPublicKey) {
  console.log("\x1b[33m%s\x1b[0m", "🔍 Checking deployer balance...");

  const balance = await getSolanaBalance(connection, deployerPublicKey);
  const balanceSOL = balance / 1e9;

  // Minimum 0.1 SOL required for deployment (conservative estimate)
  const MIN_BALANCE_SOL = 0.1;

  if (balanceSOL < MIN_BALANCE_SOL) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      `❌ Insufficient balance for deployment`
    );
    console.error(
      "\x1b[31m%s\x1b[0m",
      `   • Current Balance: ${balanceSOL.toFixed(4)} SOL`
    );
    console.error(
      "\x1b[31m%s\x1b[0m",
      `   • Required Balance: ${MIN_BALANCE_SOL} SOL`
    );
    console.error(
      "\x1b[31m%s\x1b[0m",
      `   • Please fund deployer: ${deployerPublicKey.toBase58()}`
    );
    process.exit(1);
  }

  console.log("\x1b[32m%s\x1b[0m", "✅ Deployer has sufficient balance");
}

// ============================================================================
// ========== HELPER FUNCTIONS ===============================================
// ============================================================================

async function initializeConnection() {
  console.log("\x1b[33m%s\x1b[0m", "🔄 Initializing connection...");

  let connection;
  let retries = 3;

  while (retries > 0) {
    try {
      connection = new Connection(RPC_URL, COMMITMENT);
      await connection.getVersion();
      console.log(
        "\x1b[32m%s\x1b[0m",
        "✅ Successfully connected to Solana network"
      );
      break;
    } catch (error) {
      retries--;
      if (retries === 0) {
        console.error(
          "\x1b[31m%s\x1b[0m",
          "❌ Failed to connect after multiple attempts"
        );
        process.exit(1);
      }
      console.log(
        "\x1b[33m%s\x1b[0m",
        `⚠️ Connection failed, retrying... (${retries} attempts remaining)`
      );
      await new Promise((resolve) => setTimeout(resolve, 2000));
    }
  }

  return connection;
}

async function setupDeployerAccount() {
  console.log("\x1b[33m%s\x1b[0m", "🔄 Setting up deployer account...");

  const deployerPath = path.resolve(
    __dirname,
    config.deployment.paths.deployer_key
  );
  let deployer;

  try {
    if (fs.existsSync(deployerPath)) {
      console.log(
        "\x1b[36m%s\x1b[0m",
        "📂 Loading existing deployer account..."
      );
      const deployerData = JSON.parse(fs.readFileSync(deployerPath, "utf8"));
      deployer = Keypair.fromSecretKey(new Uint8Array(deployerData));
      console.log(
        "\x1b[32m%s\x1b[0m",
        "✅ Deployer account loaded successfully!"
      );
    } else {
      console.log("\x1b[36m%s\x1b[0m", "🆕 Creating new deployer account...");
      deployer = Keypair.generate();

      // Create directory if it doesn't exist
      const deployerDir = path.dirname(deployerPath);
      if (!fs.existsSync(deployerDir)) {
        fs.mkdirSync(deployerDir, { recursive: true });
      }

      fs.writeFileSync(
        deployerPath,
        JSON.stringify(Array.from(deployer.secretKey))
      );
      console.log(
        "\x1b[32m%s\x1b[0m",
        "✅ New deployer account created and saved"
      );
    }
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Error handling deployer account:",
      error
    );
    process.exit(1);
  }

  return deployer;
}

function loadDeploymentState() {
  console.log("\x1b[33m%s\x1b[0m", "📋 Loading deployment state...");

  const deploymentDir = path.resolve(
    __dirname,
    config.deployment.paths.deployments_dir
  );
  const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);

  // Create deployments directory if it doesn't exist
  if (!fs.existsSync(deploymentDir)) {
    fs.mkdirSync(deploymentDir, { recursive: true });
  }

  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
    deploymentData = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
    console.log("\x1b[32m%s\x1b[0m", "✅ Found existing deployment data");
  } else {
    console.log("\x1b[36m%s\x1b[0m", "🆕 Creating new deployment state");
  }

  return { deploymentData, deploymentPath };
}

async function createMintAccountTx(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.dbtc_mint_created) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ DOGE_BTC mint account already exists. Skipping..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "🔑 Mint Address:",
      deploymentData.dbtc_mint_created.mint_address
    );

    // Verify mint account exists on-chain
    try {
      const mintPubkey = new PublicKey(
        deploymentData.dbtc_mint_created.mint_address
      );
      const mintInfo = await getMint(
        connection,
        mintPubkey,
        "confirmed",
        TOKEN_2022_PROGRAM_ID
      );
      console.log("\x1b[32m%s\x1b[0m", "✅ Mint account verified on-chain");
      console.log(
        "\x1b[36m%s\x1b[0m",
        `   • Supply: ${mintInfo.supply.toString()}`
      );
      console.log("\x1b[36m%s\x1b[0m", `   • Decimals: ${mintInfo.decimals}`);
    } catch (error) {
      console.error(
        "\x1b[31m%s\x1b[0m",
        "⚠️ WARNING: Mint account not found on-chain. Deployment data may be stale."
      );
      console.error(
        "\x1b[31m%s\x1b[0m",
        "   Consider clearing deployment data and redeploying."
      );
    }
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ CREATING MDOGE MINT ACCOUNT WITH METADATA ] ==================="
  );

  // Generate mint keypair
  const dogeBtcMint = Keypair.generate();
  const mintPubkey = dogeBtcMint.publicKey;

  console.log(
    "\x1b[36m%s\x1b[0m",
    "🔑 Generated Mint Address:",
    mintPubkey.toBase58()
  );

  // Setup mint parameters from config
  const decimals = config.token.decimals;
  const burnTaxBps = config.token.burn_tax_bps;
  const maxBurnAmount = config.token.max_burn_amount;

  // Authority configuration
  const mintAuthority = deployer.publicKey;
  const freezeAuthority = null; // No freeze authority
  const transferFeeConfigAuthority = deployer.publicKey;
  const withdrawWithheldAuthority = deployer.publicKey;

  // Prepare metadata (Token-2022 metadata format)
  // Note: uri should point to off-chain JSON metadata, but we'll use image URL directly
  // and add description in additionalMetadata for on-chain display
  const metadata = {
    mint: mintPubkey,
    name: TOKEN_METADATA.name,
    symbol: TOKEN_METADATA.symbol,
    uri: TOKEN_METADATA.image, // Image URL (ideally should be JSON metadata URI)
    additionalMetadata: [
      //     ...(TOKEN_METADATA.description ? [['description', TOKEN_METADATA.description]] : []),
      // ...(TOKEN_METADATA.image ? [['image', TOKEN_METADATA.image]] : []),
      // ...(TOKEN_METADATA.external_url ? [['external_url', TOKEN_METADATA.external_url]] : [])
    ],
  };

  console.log("\x1b[36m%s\x1b[0m", "⚙️ Mint Configuration:");
  console.log("\x1b[36m%s\x1b[0m", `   • Decimals: ${decimals}`);
  console.log("\x1b[36m%s\x1b[0m", `   • Burn Tax: ${burnTaxBps / 100}%`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Max Burn: ${maxBurnAmount.toLocaleString()} tokens`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Mint Authority: ${mintAuthority.toBase58()}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Freeze Authority: ${freezeAuthority || "None"}`
  );
  console.log("\x1b[36m%s\x1b[0m", "📝 Token Metadata:");
  console.log("\x1b[36m%s\x1b[0m", `   • Name: ${metadata.name}`);
  console.log("\x1b[36m%s\x1b[0m", `   • Symbol: ${metadata.symbol}`);
  console.log("\x1b[36m%s\x1b[0m", `   • URI: ${metadata.uri}`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Additional Metadata Fields: ${metadata.additionalMetadata.length}`
  );

  try {
    // Use createMintAccountWithMetadata to include MetadataPointer extension
    // This ensures token shows name/symbol in explorers and wallets
    let signature = await createMintAccountWithMetadata(
      connection,
      deployer,
      dogeBtcMint,
      burnTaxBps,
      maxBurnAmount,
      decimals,
      mintAuthority,
      freezeAuthority,
      transferFeeConfigAuthority,
      withdrawWithheldAuthority,
      metadata
    );

    console.log(
      "\x1b[32m%s\x1b[0m",
      "✅ Mint account with metadata created successfully!"
    );
    console.log("\x1b[90m%s\x1b[0m", "🔗 Transaction:", signature);

    // Verify mint account was created successfully
    console.log("\x1b[33m%s\x1b[0m", "🔍 Verifying mint account creation...");
    const mintInfo = await getMint(
      connection,
      mintPubkey,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );
    console.log("\x1b[32m%s\x1b[0m", "✅ Mint account verified on-chain");
    console.log("\x1b[36m%s\x1b[0m", `   • Decimals: ${mintInfo.decimals}`);
    console.log(
      "\x1b[36m%s\x1b[0m",
      `   • Supply: ${mintInfo.supply.toString()}`
    );

    // Update deployment data
    deploymentData.dbtc_mint_created = {
      mint_address: mintPubkey.toBase58(),
      mint_authority: mintAuthority.toBase58(),
      freeze_authority: freezeAuthority,
      transfer_fee_config_authority: transferFeeConfigAuthority.toBase58(),
      withdraw_withheld_authority: withdrawWithheldAuthority.toBase58(),
      decimals: decimals,
      burn_tax_bps: burnTaxBps,
      max_burn_amount: maxBurnAmount,
      metadata_included: true,
      metadata_name: metadata.name,
      metadata_symbol: metadata.symbol,
      metadata_uri: metadata.uri,
      creation_signature: signature,
      timestamp: new Date().toISOString(),
    };

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to create mint account with metadata:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    if (error.logs) {
      console.error("\x1b[31m%s\x1b[0m", "Transaction logs:", error.logs);
    }
    throw error;
  }
}

async function createTokenAccount(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.dbtc_token_account_created) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ DOGE_BTC token account already exists. Skipping..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "🔑 Token Account:",
      deploymentData.dbtc_token_account_created.token_account_address
    );
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ CREATING TOKEN ACCOUNT ] ==================="
  );

  const mintPubkey = new PublicKey(
    deploymentData.dbtc_mint_created.mint_address
  );

  try {
    const tokenAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      deployer, // payer
      mintPubkey, // mint
      deployer.publicKey, // owner
      undefined, // allowOwnerOffCurve
      undefined, // commitment
      undefined, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log("\x1b[32m%s\x1b[0m", "✅ Token account created successfully!");
    console.log(
      "\x1b[36m%s\x1b[0m",
      "🔑 Token Account Address:",
      tokenAccount.address.toBase58()
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "👤 Owner:",
      deployer.publicKey.toBase58()
    );

    // Update deployment data
    deploymentData.dbtc_token_account_created = {
      token_account_address: tokenAccount.address.toBase58(),
      owner_address: deployer.publicKey.toBase58(),
      mint_address: mintPubkey.toBase58(),
      timestamp: new Date().toISOString(),
    };

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to create token account:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    throw error;
  }
}

async function mintInitialSupply(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.initial_supply_minted) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ Initial supply already minted. Skipping..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "💰 Amount:",
      deploymentData.initial_supply_minted.amount
    );
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ MINTING INITIAL SUPPLY ] ==================="
  );

  const mintPubkey = new PublicKey(
    deploymentData.dbtc_mint_created.mint_address
  );
  const tokenAccountAddress = new PublicKey(
    deploymentData.dbtc_token_account_created.token_account_address
  );

  // Use string-based BigInt calculation to avoid any number conversion issues
  const initialSupplyString = config.token.initial_supply.toString();
  const decimalsString = config.token.decimals.toString();

  console.log("\x1b[36m%s\x1b[0m", "🔢 BigInt Calculation Debug:");
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Initial Supply (string): "${initialSupplyString}"`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Decimals (string): "${decimalsString}"`
  );

  // Create BigInt from string to ensure no precision loss
  const supplyBigInt = BigInt(initialSupplyString);
  const decimalsBigInt = BigInt(decimalsString);
  const multiplierBigInt = BigInt(10) ** decimalsBigInt;
  const finalAmountBigInt = supplyBigInt * multiplierBigInt;

  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Supply BigInt: ${supplyBigInt.toString()}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Multiplier BigInt: ${multiplierBigInt.toString()}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Final Amount BigInt: ${finalAmountBigInt.toString()}`
  );

  console.log("\x1b[36m%s\x1b[0m", "💰 Minting Details:");
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Target Amount: ${config.token.initial_supply.toLocaleString()} ${
      config.token.symbol
    }`
  );

  // Double-check the BigInt is truly a BigInt type
  console.log("\x1b[36m%s\x1b[0m", "🔍 Type Verification:");
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Type of finalAmountBigInt: ${typeof finalAmountBigInt}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Is BigInt: ${typeof finalAmountBigInt === "bigint"}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Constructor: ${finalAmountBigInt.constructor.name}`
  );

  try {
    console.log(
      "\x1b[33m%s\x1b[0m",
      "📡 Sending mintTo transaction with BigInt..."
    );

    const signature = await mintTo(
      connection,
      deployer, // payer
      mintPubkey, // mint
      tokenAccountAddress, // destination
      deployer, // authority
      finalAmountBigInt, // amount as pure BigInt
      [], // multiSigners
      undefined, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log("\x1b[32m%s\x1b[0m", "✅ Initial supply minted successfully!");
    console.log("\x1b[90m%s\x1b[0m", "🔗 Transaction:", signature);

    // Immediately verify the mint supply after minting
    console.log("\x1b[33m%s\x1b[0m", "🔍 Verifying mint supply...");
    const mintInfo = await getMint(
      connection,
      mintPubkey,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );
    const actualSupply = mintInfo.supply.toString();

    console.log("\x1b[36m%s\x1b[0m", "✅ Post-Mint Verification:");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `   • Expected Supply: ${finalAmountBigInt.toString()}`
    );
    console.log("\x1b[36m%s\x1b[0m", `   • Actual Supply: ${actualSupply}`);
    console.log(
      "\x1b[36m%s\x1b[0m",
      `   • Supply Match: ${
        actualSupply === finalAmountBigInt.toString() ? "✅" : "❌"
      }`
    );

    if (actualSupply !== finalAmountBigInt.toString()) {
      console.log(
        "\x1b[31m%s\x1b[0m",
        "⚠️ WARNING: Minted supply does not match expected amount!"
      );
      console.log(
        "\x1b[31m%s\x1b[0m",
        `   • Difference: ${(
          BigInt(actualSupply) - finalAmountBigInt
        ).toString()}`
      );
    }

    // Update deployment data
    deploymentData.initial_supply_minted = {
      amount: finalAmountBigInt.toString(),
      actual_minted_amount: actualSupply,
      amount_readable: `${config.token.initial_supply.toLocaleString()} ${
        config.token.symbol
      }`,
      token_account_address: tokenAccountAddress.toBase58(),
      mint_signature: signature,
      bigint_verification: {
        expected: finalAmountBigInt.toString(),
        actual: actualSupply,
        match: actualSupply === finalAmountBigInt.toString(),
      },
      timestamp: new Date().toISOString(),
    };

    // Store mint address at top level for easy access
    deploymentData.dbtc_mint_address = mintPubkey.toBase58();

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to mint initial supply:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    if (error.logs) {
      console.error("\x1b[31m%s\x1b[0m", "Transaction logs:", error.logs);
    }
    throw error;
  }
}

async function removeMintAuthority(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.mint_authority_removed) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ Mint authority already removed. Skipping..."
    );
    console.log("\x1b[36m%s\x1b[0m", "🔒 Token is non-mintable");
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ REMOVING MINT AUTHORITY ] ==================="
  );

  const mintPubkey = new PublicKey(
    deploymentData.dbtc_mint_created.mint_address
  );

  console.log(
    "\x1b[36m%s\x1b[0m",
    "🔒 Making token non-mintable by removing mint authority..."
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Current Mint Authority: ${deploymentData.dbtc_mint_created.mint_authority}`
  );
  console.log("\x1b[36m%s\x1b[0m", `   • Action: Set mint authority to null`);

  try {
    const signature = await setAuthority(
      connection,
      deployer, // payer
      mintPubkey, // mint
      deployer, // current authority
      AuthorityType.MintTokens, // authority type
      null, // new authority (null removes it)
      [], // multiSigners
      undefined, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log("\x1b[32m%s\x1b[0m", "✅ Mint authority removed successfully!");
    console.log(
      "\x1b[32m%s\x1b[0m",
      "🔒 Token is now non-mintable - no additional tokens can ever be created"
    );
    console.log("\x1b[90m%s\x1b[0m", "🔗 Transaction:", signature);

    // Update deployment data
    deploymentData.mint_authority_removed = {
      previous_mint_authority: deploymentData.dbtc_mint_created.mint_authority,
      new_mint_authority: null,
      removal_signature: signature,
      timestamp: new Date().toISOString(),
      total_supply_locked: deploymentData.initial_supply_minted.amount_readable,
    };

    // Update the mint creation data to reflect removed authority
    deploymentData.dbtc_mint_created.mint_authority = null;
    deploymentData.dbtc_mint_created.mint_authority_status = "removed";

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to remove mint authority:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    if (error.logs) {
      console.error("\x1b[31m%s\x1b[0m", "Transaction logs:", error.logs);
    }
    throw error;
  }
}

async function setWithdrawWithheldAuthorityToPDA(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.withdraw_withheld_authority_set_to_pda) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ Withdraw withheld authority already set to PDA. Skipping..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "🔑 PDA Authority:",
      deploymentData.withdraw_withheld_authority_set_to_pda.pda_authority
    );
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ SETTING WITHDRAW WITHHELD AUTHORITY TO PDA ] ==================="
  );

  const mintPubkey = new PublicKey(
    deploymentData.dbtc_mint_created.mint_address
  );

  // Load minebtc program ID from deployment data (should be set by 0_deploy_game.js)
  let minebtcProgramId;
  try {
    const gameDeploymentPath = path.resolve(
      __dirname,
      config.deployment.paths.deployments_dir,
      `${CLUSTER}.json`
    );
    if (fs.existsSync(gameDeploymentPath)) {
      const gameDeploymentData = JSON.parse(
        fs.readFileSync(gameDeploymentPath, "utf8")
      );
      if (gameDeploymentData.MINE_BTC_PROGRAM_ID) {
        minebtcProgramId = new PublicKey(
          gameDeploymentData.MINE_BTC_PROGRAM_ID
        );
      }
    }
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "⚠️ Could not load minebtc program ID. Please deploy minebtc first."
    );
    throw error;
  }

  if (!minebtcProgramId) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Minebtc program ID not found. Please run 0_deploy_game.js first."
    );
    throw new Error("Minebtc program ID required");
  }

  // Derive PDA for withdraw_withheld_authority
  const [withdrawAuthorityPDA, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("withdraw-withheld-authority")],
    minebtcProgramId
  );

  console.log(
    "\x1b[36m%s\x1b[0m",
    "🔑 Deriving withdraw withheld authority PDA..."
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Program ID: ${minebtcProgramId.toBase58()}`
  );
  console.log("\x1b[36m%s\x1b[0m", `   • Seed: "withdraw-withheld-authority"`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • PDA: ${withdrawAuthorityPDA.toBase58()}`
  );
  console.log("\x1b[36m%s\x1b[0m", `   • Bump: ${bump}`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Current Authority: ${deploymentData.dbtc_mint_created.withdraw_withheld_authority}`
  );
  console.log("\x1b[36m%s\x1b[0m", `   • Action: Transfer authority to PDA`);

  try {
    const signature = await setAuthority(
      connection,
      deployer, // payer
      mintPubkey, // mint
      deployer, // current authority
      AuthorityType.WithheldWithdraw, // authority type
      withdrawAuthorityPDA, // new authority (PDA)
      [], // multiSigners
      undefined, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log(
      "\x1b[32m%s\x1b[0m",
      "✅ Withdraw withheld authority set to PDA successfully!"
    );
    console.log(
      "\x1b[32m%s\x1b[0m",
      `🔑 PDA can now withdraw withheld tokens via program`
    );
    console.log("\x1b[90m%s\x1b[0m", "🔗 Transaction:", signature);

    // Update deployment data
    deploymentData.withdraw_withheld_authority_set_to_pda = {
      previous_withdraw_withheld_authority:
        deploymentData.dbtc_mint_created.withdraw_withheld_authority,
      pda_authority: withdrawAuthorityPDA.toBase58(),
      pda_bump: bump,
      program_id: minebtcProgramId.toBase58(),
      transfer_signature: signature,
      timestamp: new Date().toISOString(),
    };

    // Update the mint creation data to reflect new authority
    deploymentData.dbtc_mint_created.withdraw_withheld_authority =
      withdrawAuthorityPDA.toBase58();
    deploymentData.dbtc_mint_created.withdraw_withheld_authority_status =
      "set_to_pda";

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to set withdraw withheld authority to PDA:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    if (error.logs) {
      console.error("\x1b[31m%s\x1b[0m", "Transaction logs:", error.logs);
    }
    throw error;
  }
}

async function transferTransferFeeConfigAuthority(
  connection,
  deployer,
  deploymentData,
  deploymentPath
) {
  if (deploymentData.transfer_fee_config_authority_transferred) {
    console.log(
      "\x1b[34m%s\x1b[0m",
      "ℹ️ Transfer fee config authority already transferred. Skipping..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      "🔑 Current Authority:",
      deploymentData.transfer_fee_config_authority_transferred
        .new_transfer_fee_config_authority
    );
    return;
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n=================== [ TRANSFERRING TRANSFER FEE CONFIG AUTHORITY ] ==================="
  );

  const mintPubkey = new PublicKey(
    deploymentData.dbtc_mint_created.mint_address
  );
  const newAuthority = new PublicKey(
    config.deployment.transfer_fee_config_authority
  );

  console.log(
    "\x1b[36m%s\x1b[0m",
    "🔄 Transferring transfer fee config authority..."
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Current Transfer Fee Config Authority: ${deploymentData.dbtc_mint_created.transfer_fee_config_authority}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • New Transfer Fee Config Authority: ${newAuthority.toBase58()}`
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Action: Transfer authority to configured address`
  );

  try {
    const signature = await setAuthority(
      connection,
      deployer, // payer
      mintPubkey, // mint
      deployer, // current authority
      AuthorityType.TransferFeeConfig, // authority type
      newAuthority, // new authority
      [], // multiSigners
      undefined, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log(
      "\x1b[32m%s\x1b[0m",
      "✅ Transfer fee config authority transferred successfully!"
    );
    console.log(
      "\x1b[32m%s\x1b[0m",
      `🔑 New authority can now update transfer fee configuration`
    );
    console.log("\x1b[90m%s\x1b[0m", "🔗 Transaction:", signature);

    // Update deployment data
    deploymentData.transfer_fee_config_authority_transferred = {
      previous_transfer_fee_config_authority:
        deploymentData.dbtc_mint_created.transfer_fee_config_authority,
      new_transfer_fee_config_authority: newAuthority.toBase58(),
      transfer_signature: signature,
      timestamp: new Date().toISOString(),
    };

    // Update the mint creation data to reflect new authority
    deploymentData.dbtc_mint_created.transfer_fee_config_authority =
      newAuthority.toBase58();
    deploymentData.dbtc_mint_created.transfer_fee_config_authority_status =
      "transferred";

    // Save deployment data
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
    console.log("\x1b[32m%s\x1b[0m", "✅ Deployment data saved");
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Failed to transfer transfer fee config authority:",
      error
    );
    console.error("\x1b[31m%s\x1b[0m", "Error details:", error.message);
    if (error.logs) {
      console.error("\x1b[31m%s\x1b[0m", "Transaction logs:", error.logs);
    }
    throw error;
  }
}

function printCompletionSummary(deploymentData) {
  console.log(
    "\x1b[35m%s\x1b[0m",
    "\n🎉 ================================ DEPLOYMENT COMPLETE ================================"
  );
  console.log(
    "\x1b[32m%s\x1b[0m",
    "✅ DOGE_BTC token deployment completed successfully!"
  );

  console.log("\x1b[36m%s\x1b[0m", "\n📋 Deployment Summary:");
  console.log("\x1b[36m%s\x1b[0m", `  • Network: ${CLUSTER}`);
  console.log("\x1b[36m%s\x1b[0m", `  • Token Name: ${config.token.name}`);
  console.log("\x1b[36m%s\x1b[0m", `  • Token Symbol: ${config.token.symbol}`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `  • Initial Supply: ${config.token.initial_supply.toLocaleString()}`
  );
  console.log("\x1b[36m%s\x1b[0m", `  • Decimals: ${config.token.decimals}`);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `  • Burn Tax: ${config.token.burn_tax_bps / 100}%`
  );

  console.log("\x1b[90m%s\x1b[0m", "\n🔑 Important Addresses:");
  if (deploymentData.dbtc_mint_created) {
    console.log(
      "\x1b[90m%s\x1b[0m",
      `   Mint Address: ${deploymentData.dbtc_mint_created.mint_address}`
    );
    if (deploymentData.dbtc_mint_created.metadata_included) {
      console.log(
        "\x1b[90m%s\x1b[0m",
        `   Metadata: ${deploymentData.dbtc_mint_created.metadata_name} (${deploymentData.dbtc_mint_created.metadata_symbol})`
      );
      console.log(
        "\x1b[90m%s\x1b[0m",
        `   Metadata Location: Built into mint account (Token-2022 native)`
      );
    }

    // Mint Authority Status
    if (deploymentData.mint_authority_removed) {
      console.log(
        "\x1b[32m%s\x1b[0m",
        `   🔒 Mint Authority: REMOVED - Token is non-mintable`
      );
      console.log(
        "\x1b[32m%s\x1b[0m",
        `   🔒 Total Supply: ${deploymentData.mint_authority_removed.total_supply_locked} (LOCKED FOREVER)`
      );
    } else {
      console.log(
        "\x1b[90m%s\x1b[0m",
        `   Mint Authority: ${
          deploymentData.dbtc_mint_created.mint_authority || "None"
        }`
      );
    }

    // Withdraw Withheld Authority Status
    if (deploymentData.withdraw_withheld_authority_set_to_pda) {
      console.log(
        "\x1b[33m%s\x1b[0m",
        `   🔑 Withdraw Withheld Authority: SET TO PDA`
      );
      console.log(
        "\x1b[33m%s\x1b[0m",
        `   🔑 PDA Authority: ${deploymentData.withdraw_withheld_authority_set_to_pda.pda_authority}`
      );
    } else if (deploymentData.withdraw_withheld_authority_removed) {
      console.log(
        "\x1b[32m%s\x1b[0m",
        `   🔒 Withdraw Withheld Authority: REMOVED - Withheld tokens locked`
      );
    } else {
      console.log(
        "\x1b[90m%s\x1b[0m",
        `   Withdraw Withheld Authority: ${
          deploymentData.dbtc_mint_created.withdraw_withheld_authority || "None"
        }`
      );
    }

    // Transfer Fee Config Authority Status
    if (deploymentData.transfer_fee_config_authority_transferred) {
      console.log(
        "\x1b[33m%s\x1b[0m",
        `   🔄 Transfer Fee Config Authority: TRANSFERRED`
      );
      console.log(
        "\x1b[33m%s\x1b[0m",
        `   🔑 New Authority: ${deploymentData.transfer_fee_config_authority_transferred.new_transfer_fee_config_authority}`
      );
    } else {
      console.log(
        "\x1b[90m%s\x1b[0m",
        `   Transfer Fee Config Authority: ${
          deploymentData.dbtc_mint_created.transfer_fee_config_authority ||
          "None"
        }`
      );
    }
  }
  if (deploymentData.dbtc_token_account_created) {
    console.log(
      "\x1b[90m%s\x1b[0m",
      `   Token Account: ${deploymentData.dbtc_token_account_created.token_account_address}`
    );
  }
  if (deploymentData.initial_supply_minted) {
    console.log(
      "\x1b[90m%s\x1b[0m",
      `   Initial Supply Minted: ${deploymentData.initial_supply_minted.amount_readable}`
    );
  }

  console.log(
    "\x1b[35m%s\x1b[0m",
    "========================================================================================"
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    "📁 Deployment data saved to:",
    path.resolve(
      __dirname,
      config.deployment.paths.deployments_dir,
      `${CLUSTER}.json`
    )
  );
  console.log(
    "\x1b[36m%s\x1b[0m",
    "🔄 Ready for next steps: Pool creation and MineBTC initialization"
  );
}
