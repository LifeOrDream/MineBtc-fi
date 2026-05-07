import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as anchor_spl from "@solana/spl-token";
import {
  TOKEN_2022_PROGRAM_ID,
  getOrCreateAssociatedTokenAccount,
  createTransferInstruction,
  ExtensionType,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createInitializeMetadataPointerInstruction,
  TYPE_SIZE,
  LENGTH_SIZE,
} from "@solana/spl-token";
import {
  createInitializeInstruction,
  createUpdateFieldInstruction,
  pack,
} from "@solana/spl-token-metadata";
import * as web3 from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { BN, Program } = anchorPkg;
import fs from "fs";
import path from "path";

// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / DegenBtcMining state
export const GLOBAL_CONFIG_SEED = "global-config";
export const DEGEN_BTC_MINING_SEED = "mine-btc-mining";

// PDAs which hold SOL collected by the program
export const SOL_TREASURY_SEED = "sol-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
export const DEGEN_BTC_VAULT_AUTHORITY_SEED = "minebtc-vault-authority";
export const DEGEN_BTC_VAULT_SEED = "minebtc_vault";

// PDAs which hold ModuleConfigStore / GearConfigStore state
export const MODULE_CONFIG_STORE_SEED = "module-config-store";
export const MODULE_CONFIG_SEED = "module-config";

// PDAs which hold UserMineBTCInstance / ReferralRewards state
export const USER_MineBTC_SEED = "user-minebtc";
export const REFERRAL_REWARDS_SEED = "referral-rewards";

// PDAs which hold GearInstance / ModuleInstance state
export const MODULE_INSTANCE_SEED = "module-instance";

// pda which holds the mdoge nft vault state
export const dbtc_NFT_VAULT_SEED = "mdoge-nft-vault";

// PDAs for loot rewards system
export const LOOT_REWARDS_SEED = "loot-rewards";
export const LOOT_SOL_VAULT_SEED = "loot-sol-vault";
export const LOOT_DEGEN_BTC_VAULT_SEED = "loot-mdoge-vault";
export const LOOT_DEGEN_BTC_VAULT_AUTHORITY_SEED = "loot-minebtc-vault-authority";
export const LEVEL_STATS_SEED = "level-stats";
export const BUYBACKS_SEED = "buybacks";
export const BUYBACKS_SOL_VAULT_SEED = "buybacks-sol-vault";


// =================== [ RAYDIUM CP-SWAP HELPERS ] ===================

// PDA seed for Raydium CP-Swap AMM config
export const CP_AMM_CONFIG_SEED = "amm_config";

/**
 * Return balance (lamports) for provided public key.
 * @param {Connection} connection Solana connection
 * @param {PublicKey|string} pubkey Public key or string
 * @returns {Promise<number>} balance in lamports
 */
export async function getSolanaBalance(connection, pubkey) {
  const key = typeof pubkey === "string" ? new PublicKey(pubkey) : pubkey;
  try {
    return await connection.getBalance(key);
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      `❌ Error getting SOL balance: ${error.message}`
    );
    throw error;
  }
}

// Add function to update deployment status
export function updateDeploymentStatus(
  network,
  status,
  data = {},
  balance = 0
) {
  const deploymentDir = path.join(process.cwd(), "deployments");

  // Create deployments directory if it doesn't exist
  if (!fs.existsSync(deploymentDir)) {
    fs.mkdirSync(deploymentDir, { recursive: true });
  }

  const deploymentPath = path.join(deploymentDir, `${network}.json`);

  // Read existing deployment data or create new
  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
    deploymentData = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
  }

  // Update with new info
  deploymentData = {
    ...deploymentData,
    lastUpdated: new Date().toISOString(),
    lastStatus: status,
    deployerBalance: balance / 1e9,
    [status]: {
      ...data,
    },
  };

  // Write back to file
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));

  console.log(`Deployment status updated: ${status}`);
}

// Create mint account for a Solana Token 2022 with transfer-tax extension
export async function createMintAccount(
  connection,
  deployer,
  degenBtcMint,
  BURN_TAX_BPS,
  burn_tax,
  decimals,
  mintAuthority,
  freezeAuthority,
  transferFeeConfigAuthority,
  withdrawWithheldAuthority
) {
  try {
    console.log(
      "\x1b[36m%s\x1b[0m",
      "📝 Creating mint account with transfer fee config..."
    );

    const MAX_BURN_TAX = BigInt(burn_tax) * BigInt(10) ** BigInt(decimals);
    console.log(
      "\x1b[36m%s\x1b[0m",
      `   • Max Transfer Fee: ${MAX_BURN_TAX.toString()}`
    );

    // Create mint account with transfer fee extension
    const extensions = [ExtensionType.TransferFeeConfig];
    const mintLen = getMintLen(extensions);
    const lamports = await connection.getMinimumBalanceForRentExemption(
      mintLen
    );

    console.log("\x1b[36m%s\x1b[0m", "🔧 Preparing instructions...");

    // Create account instruction
    const createAccountIx = SystemProgram.createAccount({
      fromPubkey: deployer.publicKey,
      newAccountPubkey: degenBtcMint.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    });

    // Initialize transfer fee config instruction
    const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
      degenBtcMint.publicKey,
      transferFeeConfigAuthority,
      withdrawWithheldAuthority,
      BURN_TAX_BPS,
      MAX_BURN_TAX,
      TOKEN_2022_PROGRAM_ID
    );

    // Initialize mint instruction
    const initMintIx = createInitializeMintInstruction(
      degenBtcMint.publicKey,
      decimals,
      mintAuthority,
      freezeAuthority,
      TOKEN_2022_PROGRAM_ID
    );

    console.log("\x1b[36m%s\x1b[0m", "📤 Building and sending transaction...");

    const tx = new Transaction()
      .add(createAccountIx)
      .add(initTransferFeeIx)
      .add(initMintIx);

    // Add retry mechanism for transaction confirmation
    let retries = 3;
    while (retries > 0) {
      try {
        const signature = await sendAndConfirmTransaction(
          connection,
          tx,
          [deployer, degenBtcMint],
          {
            commitment: "confirmed",
            maxRetries: 3,
            preflightCommitment: "confirmed",
          }
        );
        console.log("\x1b[32m%s\x1b[0m", `✅ Token mint created successfully!`);
        console.log("\x1b[90m%s\x1b[0m", `   Transaction: ${signature}`);
        return signature;
      } catch (error) {
        retries--;
        if (retries === 0) {
          console.error(
            "\x1b[31m%s\x1b[0m",
            `❌ Failed to create token mint: ${error.message}`
          );
          throw error;
        }
        console.log(
          "\x1b[33m%s\x1b[0m",
          `⚠️ Retrying transaction... (${retries} attempts remaining)`
        );
        await new Promise((resolve) => setTimeout(resolve, 2000));
      }
    }
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      `❌ Error in createMintAccount: ${error.message}`
    );
    throw error;
  }
}

export async function createMintAccountWithMetadata(
  connection,
  deployer,
  degenBtcMint,
  BURN_TAX_BPS,
  burn_tax,
  decimals,
  mintAuthority,
  freezeAuthority,
  transferFeeConfigAuthority,
  withdrawWithheldAuthority,
  metadata
) {
  try {
    console.log("\x1b[36m%s\x1b[0m", "📝 Creating mint with MetadataPointer + TransferFeeConfig extensions...");

    const MAX_BURN_TAX = BigInt(burn_tax) * BigInt(10) ** BigInt(decimals);
    console.log("\x1b[36m%s\x1b[0m", `   • Max Transfer Fee: ${MAX_BURN_TAX.toString()}`);

    // Extensions: MetadataPointer + TransferFeeConfig
    const extensions = [ExtensionType.MetadataPointer, ExtensionType.TransferFeeConfig];
    const mintLen = getMintLen(extensions);

    console.log("\x1b[36m%s\x1b[0m", `   • Mint Length (extensions only): ${mintLen}`);

    // TX1: Create account with ONLY extension space, init extensions, init mint
    const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

    const tx1 = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: deployer.publicKey,
        newAccountPubkey: degenBtcMint.publicKey,
        lamports,
        space: mintLen,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeMetadataPointerInstruction(
        degenBtcMint.publicKey,
        deployer.publicKey,
        degenBtcMint.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeTransferFeeConfigInstruction(
        degenBtcMint.publicKey,
        transferFeeConfigAuthority,
        withdrawWithheldAuthority,
        BURN_TAX_BPS,
        MAX_BURN_TAX,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        degenBtcMint.publicKey,
        decimals,
        mintAuthority,
        freezeAuthority,
        TOKEN_2022_PROGRAM_ID
      )
    );

    console.log("\x1b[36m%s\x1b[0m", "📤 Sending TX1: Create + Extensions + InitMint...");
    const sig1 = await sendAndConfirmTransaction(connection, tx1, [deployer, degenBtcMint], {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
      maxRetries: 3,
    });
    console.log("\x1b[32m%s\x1b[0m", `✅ TX1 confirmed: ${sig1}`);

    // Calculate additional rent needed for metadata TLV
    const metadataPayload = pack({
      updateAuthority: deployer.publicKey,
      mint: degenBtcMint.publicKey,
      name: metadata.name,
      symbol: metadata.symbol,
      uri: metadata.uri,
      additionalMetadata: metadata.additionalMetadata ?? [],
    });
    const metadataTlvLen = TYPE_SIZE + LENGTH_SIZE + metadataPayload.length;
    const totalSpace = mintLen + metadataTlvLen;
    const totalLamports = await connection.getMinimumBalanceForRentExemption(totalSpace);
    const additionalLamports = totalLamports - lamports;

    console.log("\x1b[36m%s\x1b[0m", `   • Metadata TLV: ${metadataTlvLen} bytes`);
    console.log("\x1b[36m%s\x1b[0m", `   • Additional rent needed: ${additionalLamports} lamports`);

    // TX2: Transfer additional rent + Initialize metadata
    const metadataFieldInstructions = (metadata.additionalMetadata ?? []).map(
      ([field, value]) =>
        createUpdateFieldInstruction({
          programId: TOKEN_2022_PROGRAM_ID,
          metadata: degenBtcMint.publicKey,
          updateAuthority: deployer.publicKey,
          field,
          value,
        })
    );

    const tx2 = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: deployer.publicKey,
        toPubkey: degenBtcMint.publicKey,
        lamports: additionalLamports,
      }),
      createInitializeInstruction({
        programId: TOKEN_2022_PROGRAM_ID,
        mint: degenBtcMint.publicKey,
        metadata: degenBtcMint.publicKey,
        name: metadata.name,
        symbol: metadata.symbol,
        uri: metadata.uri,
        mintAuthority: mintAuthority,
        updateAuthority: deployer.publicKey,
      }),
      ...metadataFieldInstructions
    );

    console.log("\x1b[36m%s\x1b[0m", "📤 Sending TX2: Fund rent + Initialize Metadata...");
    const sig2 = await sendAndConfirmTransaction(connection, tx2, [deployer], {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
      maxRetries: 3,
    });
    console.log("\x1b[32m%s\x1b[0m", `✅ TX2 confirmed: ${sig2}`);
    console.log("\x1b[32m%s\x1b[0m", `✅ Token created with metadata: ${metadata.name} (${metadata.symbol})`);

    return sig2;
  } catch (error) {
    console.error("\x1b[31m%s\x1b[0m", `❌ Error in createMintAccountWithMetadata: ${error.message}`);
    throw error;
  }
}

export async function createMintAccount_T22_TransferFeeOnly(
  connection,
  deployer,
  degenBtcMint,
  decimals,
  mintAuthority,
  freezeAuthority,
  transferFeeConfigAuthority,
  withdrawWithheldAuthority,
  burnTaxBps,
  maxBurnTokens
) {
  // Convert max burn into base units
  // const maxBurnInBaseUnits = maxBurnTokens * (10n ** BigInt(decimals));
  const MAX_BURN_TAX = BigInt(maxBurnTokens) * BigInt(10) ** BigInt(decimals);
  console.log(
    "\x1b[36m%s\x1b[0m",
    `   • Max Transfer Fee: ${MAX_BURN_TAX.toString()}`
  );

  // We are ONLY using TransferFeeConfig at creation time
  const extensions = [ExtensionType.TransferFeeConfig];
  const mintLen = getMintLen(extensions);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

  // Create the mint account owned by Token-2022
  const createIx = SystemProgram.createAccount({
    fromPubkey: deployer.publicKey,
    newAccountPubkey: degenBtcMint.publicKey,
    space: mintLen,
    lamports,
    programId: TOKEN_2022_PROGRAM_ID,
  });

  // IMPORTANT: initialize the extension BEFORE InitializeMint
  const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
    degenBtcMint.publicKey,
    transferFeeConfigAuthority,
    withdrawWithheldAuthority,
    burnTaxBps, // basis points
    MAX_BURN_TAX, // u64 in base units
    TOKEN_2022_PROGRAM_ID
  );

  const initMintIx = createInitializeMintInstruction(
    degenBtcMint.publicKey,
    decimals,
    mintAuthority,
    freezeAuthority, // can be null
    TOKEN_2022_PROGRAM_ID
  );

  const tx = new Transaction()
    .add(createIx)
    .add(initTransferFeeIx) // <-- extension first
    .add(initMintIx); // <-- then InitializeMint

  return await sendAndConfirmTransaction(
    connection,
    tx,
    [deployer, degenBtcMint],
    {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
      maxRetries: 3,
    }
  );
}

/**
 * Create the system referral sentinel account used for players without a referrer
 * @param {Connection} connection Solana connection
 * @param {Program} program MineBTC program instance
 * @param {Object} wallet Wallet instance
 * @param {Keypair} walletKeypair Wallet keypair for signing
 * @returns {Promise<Object>} Result object with success status and data
 */
export async function initializeSystemAccounts(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA
) {
  try {
    console.log(
      "\x1b[33m%s\x1b[0m",
      "📡 Initializing system accounts (referral + buybacks)..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Global Config PDA: ${globalConfigPDA.toString()}`
    );

    // Derive PDAs
    const [systemReferralRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(REFERRAL_REWARDS_SEED), SystemProgram.programId.toBuffer()],
      program.programId
    );

    const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(BUYBACKS_SEED)],
      program.programId
    );

    const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(BUYBACKS_SOL_VAULT_SEED)],
      program.programId
    );

    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 System Referral Rewards PDA: ${systemReferralRewardsPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Buybacks Account PDA: ${buybacksAccountPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Buybacks SOL Vault PDA: ${buybacksSolVaultPDA.toString()}`
    );

    // Build transaction
    const tx = await program.methods
      .initializeSystemAccounts()
      .accounts({
        globalConfig: globalConfigPDA,
        systemReferralRewards: systemReferralRewardsPDA,
        buybacksAccount: buybacksAccountPDA,
        buybacksSolVault: buybacksSolVaultPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .transaction();

    // Send and confirm transaction
    console.log(
      "\x1b[33m%s\x1b[0m",
      "📡 Sending initialize system accounts transaction..."
    );
    const txid = await web3.sendAndConfirmTransaction(connection, tx, [
      walletKeypair,
    ]);

    console.log(
      "\x1b[32m%s\x1b[0m",
      `✅ System accounts initialized successfully!`
    );
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${txid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=devnet`
    );

    return {
      success: true,
      data: {
        txid,
        systemReferralRewards_address: systemReferralRewardsPDA.toString(),
        buybacksAccount_address: buybacksAccountPDA.toString(),
        buybacksSolVault_address: buybacksSolVaultPDA.toString(),
      },
    };
  } catch (error) {
    if (
      error.toString().includes("already in use") ||
      error.toString().includes("AccountAlreadyInUse")
    ) {
      console.log(
        "\x1b[34m%s\x1b[0m",
        `ℹ️ System accounts already initialized. Skipping creation.`
      );

      // Still need to return the PDAs
      const [systemReferralRewardsPDA] = PublicKey.findProgramAddressSync(
        [
          Buffer.from(REFERRAL_REWARDS_SEED),
          SystemProgram.programId.toBuffer(),
        ],
        program.programId
      );

      const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(BUYBACKS_SEED)],
        program.programId
      );

      const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(BUYBACKS_SOL_VAULT_SEED)],
        program.programId
      );

      return {
        success: true,
        data: {
          systemReferralRewards_address: systemReferralRewardsPDA.toString(),
          buybacksAccount_address: buybacksAccountPDA.toString(),
          buybacksSolVault_address: buybacksSolVaultPDA.toString(),
        },
      };
    } else {
      console.error(
        "\x1b[31m%s\x1b[0m",
        "❌ Error initializing system accounts:",
        error
      );
      return {
        success: false,
        error: error.toString(),
      };
    }
  }
}

/**
 * Initialize the MineBTC program
 * @param {Connection} connection Solana connection
 * @param {Program} program MineBTC program instance
 * @param {Object} wallet Wallet instance
 * @param {Keypair} walletKeypair Wallet keypair for signing
 * @param {number} baseCost Base creation cost in lamports
 * @param {PublicKey} creationFeeRecipient Address to receive creation fees
 * @returns {Promise<Object>} Result object with success status and data
 */
export async function initializeMinebtcProgram(
  connection,
  program,
  wallet,
  walletKeypair,
  creationFeeRecipient
) {
  try {
    // Define parameters
    const feeRecipient = new PublicKey(creationFeeRecipient);

    // Find PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_CONFIG_SEED)],
      program.programId
    );
    const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(DEGEN_BTC_MINING_SEED)],
      program.programId
    );
    const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(SOL_TREASURY_SEED)],
      program.programId
    );

    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Global Config PDA: ${globalConfigPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DegenBTC Mining PDA: ${mineBtcMiningPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 SOL Treasury PDA: ${solTreasuryPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Creation Fee Recipient: ${feeRecipient.toString()}`
    );

    // Build transaction
    const tx = await program.methods
      .initialize(feeRecipient)
      .accounts({
        globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
        solTreasury: solTreasuryPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    // Send and confirm transaction
    console.log("\x1b[33m%s\x1b[0m", "📡 Sending initialize transaction...");
    const txid = await web3.sendAndConfirmTransaction(connection, tx, [
      walletKeypair,
    ]);

    console.log("\x1b[32m%s\x1b[0m", `✅ Program initialized successfully!`);
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${txid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=devnet`
    );

    return {
      success: true,
      data: {
        txid,
        globalConfig_address: globalConfigPDA.toString(),
        mineBtcMining_address: mineBtcMiningPDA.toString(),
        solTreasury_address: solTreasuryPDA.toString(),
      },
    };
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log(
        "\x1b[34m%s\x1b[0m",
        `ℹ️ Program already initialized. Skipping initialization.`
      );

      // Still need to return the PDAs
      const [globalConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(GLOBAL_CONFIG_SEED)],
        program.programId
      );

      const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(DEGEN_BTC_MINING_SEED)],
        program.programId
      );

      const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(SOL_TREASURY_SEED)],
        program.programId
      );

      return {
        success: true,
        data: {
          globalConfig_address: globalConfigPDA.toString(),
          mineBtcMining_address: mineBtcMiningPDA.toString(),
          solTreasury_address: solTreasuryPDA.toString(),
        },
      };
    } else {
      console.error(
        "\x1b[31m%s\x1b[0m",
        "❌ Error initializing program:",
        error
      );
      return {
        success: false,
        error: error.toString(),
      };
    }
  }
}

/**
 * Update the global config of the MineBTC program
 */
export async function updateGlobalConfig(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA,
  mineBtcMiningPDA,
  newAuthority = null,
  newFeeCollector = null,
  newCreationFeeRecipient = null,
  newBaseCreationCost = null,
  newLootPercentage = null
) {
  try {
    console.log("\x1b[33m%s\x1b[0m", "📡 Updating MineBTC global config...");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Global Config PDA: ${globalConfigPDA}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Module Config Store PDA: ${moduleConfigStorePDA}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DegenBtc Mining PDA: ${mineBtcMiningPDA}`
    );

    if (newAuthority)
      console.log("\x1b[36m%s\x1b[0m", `   New Authority: ${newAuthority}`);
    if (newFeeCollector)
      console.log(
        "\x1b[36m%s\x1b[0m",
        `   New Fee Collector (ext_fee_collector): ${newFeeCollector}`
      );
    if (newCreationFeeRecipient)
      console.log(
        "\x1b[36m%s\x1b[0m",
        `   New Creation Fee Recipient: ${newCreationFeeRecipient}`
      );
    if (newBaseCreationCost)
      console.log(
        "\x1b[36m%s\x1b[0m",
        `   New Base Creation Cost: ${newBaseCreationCost}`
      );
    if (newLootPercentage)
      console.log(
        "\x1b[36m%s\x1b[0m",
        `   New Loot Percentage: ${newLootPercentage}`
      );

    const updateTx = await program.methods
      .updateConfig(
        newAuthority ? new PublicKey(newAuthority) : null,
        newFeeCollector ? new PublicKey(newFeeCollector) : null,
        newCreationFeeRecipient ? new PublicKey(newCreationFeeRecipient) : null,
        newLootPercentage
      )
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        moduleConfigStore: new PublicKey(moduleConfigStorePDA),
        mineBtcMining: new PublicKey(mineBtcMiningPDA),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(
      connection,
      updateTx,
      [walletKeypair]
    );

    console.log("\x1b[32m%s\x1b[0m", `✅ MineBTC global config updated`);
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${updateTxid}`);

    return {
      success: true,
      data: {
        updateTxid: updateTxid,
      },
    };
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Error updating MineBTC global config:",
      error
    );
    return {
      success: false,
      error: error.toString(),
    };
  }
}
 
/**
 * Creates a token account for the program to use as mining vault and initializes mining parameters
 */
export async function setupMiningVault(
  connection,
  program,
  wallet,
  walletKeypair,
  mineBtcMiningPDA,
  vaultPDA,
  vaultAuthorityPDA,
  tokenMint,
  token_program,
  start_timestamp,
  degen_btc_per_round,
  raydium_pool_state
) {
  try {
    console.log("\x1b[33m%s\x1b[0m", "📡 Initializing mining parameters...");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGENBTC Mining PDA: ${mineBtcMiningPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGEN_BTC Vault: ${vaultPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGEN_BTC Vault Authority: ${vaultAuthorityPDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGEN_BTC Token Mint: ${tokenMint.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGEN_BTC Token Program: ${token_program.toString()}`
    );
    console.log("\x1b[90m%s\x1b[0m", `⏰ Start Timestamp: ${start_timestamp}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `💰 DEGENBTC Per Slot: ${degen_btc_per_round.toString()}`
    );
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔄 Raydium Pool State: ${raydium_pool_state.toString()}`
    );

    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_CONFIG_SEED)],
      program.programId
    );

    const miningTx = await program.methods
      .initializeMining(
        new BN(start_timestamp), // start_timestamp
        new BN(degen_btc_per_round), // degen_btc_per_round (tokens per slot)
        new PublicKey(raydium_pool_state) // pool_state (Raydium pool state)
      )
      .accounts({
        globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
        vaultAuthority: vaultAuthorityPDA,
        tokenVault: vaultPDA,
        tokenMint: tokenMint,
        tokenProgram: token_program,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .transaction();

    const miningTxid = await web3.sendAndConfirmTransaction(
      connection,
      miningTx,
      [walletKeypair]
    );
    console.log(
      "\x1b[32m%s\x1b[0m",
      `✅ Mining initialized with token vault: ${vaultPDA.toString()}`
    );
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${miningTxid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${miningTxid}?cluster=devnet`
    );

    return {
      success: true,
      data: {
        vaultAddress: vaultPDA.toString(),
        initMiningTxid: miningTxid,
      },
    };
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Error setting up mining vault:",
      error
    );
    return {
      success: false,
      error: error.toString(),
    };
  }
}
 
 

/**
 * Update an existing module config
 */
export async function updateModuleConfig(
  connection,
  program,
  wallet,
  walletKeypair,
  moduleConfigStorePDA,
  moduleConfigName,
  moduleConfigImageUrl,
  moduleConfigMintCost,
  moduleConfigUpgradeCost,
  moduleConfigMaxUpgrades,
  moduleConfigMaxDoges
) {
  try {
    console.log("\x1b[33m%s\x1b[0m", "📡 Updating module config...");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Module Config Store: ${moduleConfigStorePDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Module Config Name: ${moduleConfigName}`
    );

    const configStoreTx = await program.methods
      .updateModule(
        moduleConfigName,
        moduleConfigImageUrl,
        moduleConfigMintCost,
        moduleConfigUpgradeCost,
        moduleConfigMaxUpgrades,
        moduleConfigMaxDoges
      )
      .accounts({
        globalConfig: globalConfigPDA,
        moduleConfigStore: moduleConfigStorePDA,
        gearConfigStore: gearConfigStorePDA,
        mineBtcMining: mineBtcMiningPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const configStoreTxid = await web3.sendAndConfirmTransaction(
      connection,
      configStoreTx,
      [walletKeypair]
    );
    console.log("\x1b[32m%s\x1b[0m", `✅ Module config updated`);
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${configStoreTxid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`
    );

    return {
      success: true,
    };
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Error updating module config:",
      error
    );
    return {
      success: false,
      error: error.toString(),
    };
  }
}

/**
 * Update an existing gear config
 */
export async function updateGearConfig(
  connection,
  program,
  wallet,
  walletKeypair,
  gearConfigStorePDA,
  gearConfigPDA,
  gearConfigName,
  gearConfigImageUrl,
  gearConfigType,
  gearConfigCompatibleModules,
  gearConfigInitHashpower,
  gearConfigInitElectricity,
  gearConfigPrice,
  gearConfigMaxUpgrades,
  gearConfigAvailableCount,
  gearConfigIsTradable
) {
  try {
    console.log("\x1b[33m%s\x1b[0m", "📡 Updating gear config...");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Gear Config Store: ${gearConfigStorePDA.toString()}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Gear Config PDA: ${gearConfigPDA.toString()}`
    );

    const configStoreTx = await program.methods
      .updateGear(
        gearConfigName,
        gearConfigImageUrl,
        gearConfigType,
        gearConfigCompatibleModules,
        gearConfigInitHashpower,
        gearConfigInitElectricity,
        gearConfigPrice,
        gearConfigMaxUpgrades,
        gearConfigAvailableCount,
        gearConfigIsTradable
      )
      .accounts({
        globalConfig: globalConfigPDA,
        gearConfigStore: gearConfigStorePDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const configStoreTxid = await web3.sendAndConfirmTransaction(
      connection,
      configStoreTx,
      [walletKeypair]
    );
    console.log("\x1b[32m%s\x1b[0m", `✅ Gear config updated`);
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${configStoreTxid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`
    );

    return {
      success: true,
    };
  } catch (error) {
    console.error("\x1b[31m%s\x1b[0m", "❌ Error updating gear config:", error);
    return {
      success: false,
      error: error.toString(),
    };
  }
}


export async function sendSPL22Token({
  connection,
  payer, // Keypair paying for tx and fees
  senderTokenAccount,
  recipientWallet,
  mintAddress,
  amount,
}) {
  console.log(`senderTokenAccount: ${senderTokenAccount}`);
  console.log(`recipientWallet: ${recipientWallet}`);
  console.log(`mintAddress: ${mintAddress}`);
  console.log(`amount: ${amount}`);
  // return;

  // Get mint info to know decimals
  const mintInfo = await anchor_spl.getMint(
    connection,
    new PublicKey(mintAddress),
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  const decimals = mintInfo.decimals;
  // console.log(`decimals: ${decimals}`);
  // console.log(mintInfo);
  // return

  // Get or create recipient token account
  const recipientTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    new PublicKey(mintAddress),
    new PublicKey(recipientWallet),
    false, // allowOwnerOffCurve
    undefined, // commitment
    undefined, // confirmOptions
    TOKEN_2022_PROGRAM_ID // Important for SPL-2022!
  );
  // console.log(`recipientTokenAccount`);
  // console.log(recipientTokenAccount);
  // return;

  console.log(`senderTokenAccount: ${senderTokenAccount}`);
  console.log(`recipientTokenAccount: ${recipientTokenAccount.address}`);
  console.log(`payer.publicKey: ${payer.publicKey}`);
  console.log(`amount: ${amount}`);
  console.log(`TOKEN_2022_PROGRAM_ID: ${TOKEN_2022_PROGRAM_ID}`);
  // return;

  // Transfer
  const tx = new Transaction().add(
    anchor_spl.createTransferCheckedInstruction(
      new PublicKey(senderTokenAccount),
      new PublicKey(mintAddress),
      recipientTokenAccount.address,
      payer.publicKey,
      amount,
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID
    )
  );

  const signature = await sendAndConfirmTransaction(connection, tx, [payer]);
  console.log("✅ Sent SPL-2022 token:", signature);
}

export async function sendSPLToken({
  connection,
  payer, // Keypair paying for tx and fees
  senderTokenAccount,
  recipientWallet,
  mintAddress,
  amount,
}) {
  console.log(`senderTokenAccount: ${senderTokenAccount}`);
  console.log(`recipientWallet: ${recipientWallet}`);
  console.log(`mintAddress: ${mintAddress}`);
  console.log(`amount: ${amount}`);

  // Get or create recipient token account for regular SPL token
  const recipientTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    new PublicKey(mintAddress),
    new PublicKey(recipientWallet),
    false, // allowOwnerOffCurve
    undefined, // commitment
    undefined, // confirmOptions
    TOKEN_PROGRAM_ID // Regular SPL Token Program
  );

  console.log(`senderTokenAccount: ${senderTokenAccount}`);
  console.log(`recipientTokenAccount: ${recipientTokenAccount.address}`);
  console.log(`payer.publicKey: ${payer.publicKey}`);
  console.log(`amount: ${amount}`);
  console.log(`TOKEN_PROGRAM_ID: ${TOKEN_PROGRAM_ID}`);

  // Transfer using regular transfer instruction
  const tx = new Transaction().add(
    anchor_spl.createTransferInstruction(
      new PublicKey(senderTokenAccount),
      recipientTokenAccount.address,
      payer.publicKey,
      amount,
      [],
      TOKEN_PROGRAM_ID
    )
  );

  const signature = await sendAndConfirmTransaction(connection, tx, [payer]);
  console.log("✅ Sent SPL token:", signature);
}

 
// ------------------------------------------------------------------
// Raydium CP-Swap helpers (create config, pool ops, swap)
// ------------------------------------------------------------------

/**
 * Derive AmmConfig PDA (matches Rust seed [AMM_CONFIG_SEED, index.to_be_bytes()])
 */
function deriveAmmConfigPDA(index, programId) {
  const buf = Buffer.alloc(2);
  buf.writeUInt16BE(index, 0); // big-endian
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from(CP_AMM_CONFIG_SEED), buf],
    programId
  );
  return pda;
}

/**
 * Create a new Raydium AMM config.
 * @returns {Promise<{ammConfigPDA:string, txid:string}>}
 */
export async function cpCreateAmmConfig(
  connection,
  cpProgram,
  wallet,
  walletKeypair,
  { index, tradeFeeRate, protocolFeeRate, fundFeeRate, createPoolFee }
) {
  const ammConfigPDA = deriveAmmConfigPDA(index, cpProgram.programId);

  const tx = await cpProgram.methods
    .createAmmConfig(
      index,
      new BN(tradeFeeRate),
      new BN(protocolFeeRate),
      new BN(fundFeeRate),
      new BN(createPoolFee)
    )
    .accounts({
      owner: wallet.publicKey,
      ammConfig: ammConfigPDA,
      systemProgram: SystemProgram.programId,
    })
    .transaction();

  const txid = await web3.sendAndConfirmTransaction(connection, tx, [
    walletKeypair,
  ]);
  console.log("✅ AmmConfig created", txid);
  return { ammConfigPDA: ammConfigPDA.toString(), txid };
}

/**
 * Initialize a CP-Swap pool (wrapper for initialize instruction).
 * `accounts` should contain the full account map as per IDL.
 */
export async function cpInitializePool(
  connection,
  cpProgram,
  walletKeypair,
  { initAmount0, initAmount1, openTime, accounts }
) {
  const tx = await cpProgram.methods
    .initialize(new BN(initAmount0), new BN(initAmount1), new BN(openTime))
    .accounts(accounts)
    .transaction();

  const txid = await web3.sendAndConfirmTransaction(connection, tx, [
    walletKeypair,
  ]);
  console.log("✅ Pool initialized", txid);
  return { txid };
}

/** Deposit liquidity to pool */
export async function cpDepositLiquidity(
  connection,
  cpProgram,
  walletKeypair,
  { lpAmount, maxToken0, maxToken1, accounts }
) {
  const tx = await cpProgram.methods
    .deposit(new BN(lpAmount), new BN(maxToken0), new BN(maxToken1))
    .accounts(accounts)
    .transaction();
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [
    walletKeypair,
  ]);
  console.log("✅ Liquidity deposited", txid);
  return { txid };
}

/** Withdraw liquidity from pool */
export async function cpWithdrawLiquidity(
  connection,
  cpProgram,
  walletKeypair,
  { lpAmount, minToken0, minToken1, accounts }
) {
  const tx = await cpProgram.methods
    .withdraw(new BN(lpAmount), new BN(minToken0), new BN(minToken1))
    .accounts(accounts)
    .transaction();
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [
    walletKeypair,
  ]);
  console.log("✅ Liquidity withdrawn", txid);
  return { txid };
}

/** Swap tokens (base input) */
export async function cpSwapBaseInput(
  connection,
  cpProgram,
  walletKeypair,
  { amountIn, minAmountOut, accounts }
) {
  const tx = await cpProgram.methods
    .swapBaseInput(new BN(amountIn), new BN(minAmountOut))
    .accounts(accounts)
    .transaction();
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [
    walletKeypair,
  ]);
  console.log("✅ Swap executed", txid);
  return { txid };
}

/**
 * Update slots per hour configuration (admin only)
 */
export async function updateSlotsPerHour(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  mineBtcMiningPDA,
  newSlotsPerHour
) {
  try {
    console.log(
      "\x1b[33m%s\x1b[0m",
      "📡 Updating slots per hour configuration..."
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Global Config PDA: ${globalConfigPDA}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 DEGENBTC Mining PDA: ${mineBtcMiningPDA}`
    );
    console.log(
      "\x1b[36m%s\x1b[0m",
      `⏰ New slots per hour: ${newSlotsPerHour}`
    );

    const updateTx = await program.methods
      .updateSlotsPerHour(new BN(newSlotsPerHour))
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        mineBtcMining: new PublicKey(mineBtcMiningPDA),
        authority: wallet.publicKey,
      })
      .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(
      connection,
      updateTx,
      [walletKeypair]
    );

    console.log(
      "\x1b[32m%s\x1b[0m",
      `✅ Slots per hour updated to ${newSlotsPerHour}`
    );
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${updateTxid}`);
    console.log(
      "\x1b[90m%s\x1b[0m",
      `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`
    );

    return {
      success: true,
      data: {
        updateTxid: updateTxid,
        newSlotsPerHour: newSlotsPerHour,
      },
    };
  } catch (error) {
    console.error(
      "\x1b[31m%s\x1b[0m",
      "❌ Error updating slots per hour:",
      error
    );
    return {
      success: false,
      error: error.toString(),
    };
  }
}

/**
 * Add factions to the supported factions list (admin only)
 */
export async function addFactions(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA,
  mineBtcMiningPDA,
  factionNames
) {
  try {
    console.log("\x1b[33m%s\x1b[0m", "📡 Adding factions...");
    console.log(
      "\x1b[36m%s\x1b[0m",
      `🔑 Global Config PDA: ${globalConfigPDA}`
    );
    console.log("\x1b[36m%s\x1b[0m", `🏴 Factions: ${factionNames.join(", ")}`);

    const addFactionsTxid = await program.methods
      .addFactions(factionNames)
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        moduleConfigStore: new PublicKey(moduleConfigStorePDA),
        mineBtcMining: new PublicKey(mineBtcMiningPDA),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log(
      "\x1b[32m%s\x1b[0m",
      `✅ Added ${factionNames.length} factions successfully`
    );
    console.log("\x1b[90m%s\x1b[0m", `🔗 Transaction ID: ${addFactionsTxid}`);

    return {
      success: true,
      data: {
        addFactionsTxid: addFactionsTxid,
        factionNames: factionNames,
      },
    };
  } catch (error) {
    console.error("\x1b[31m%s\x1b[0m", "❌ Error adding factions:", error);
    return {
      success: false,
      error: error.toString(),
    };
  }
}

  
// initializeBuybacks is now merged into initializeSystemAccounts
// Keeping this function for backward compatibility but it calls initializeSystemAccounts
export async function initializeBuybacks(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA
) {
  console.log(
    "\x1b[33m%s\x1b[0m",
    "⚠️ initializeBuybacks is deprecated. Use initializeSystemAccounts instead."
  );
  return initializeSystemAccounts(
    connection,
    program,
    wallet,
    walletKeypair,
    globalConfigPDA
  );
}

// ==================== [ ADMIN HELPER FUNCTIONS ] ====================

/**
 * Helper function to update global configuration
 */
export async function updateGlobalConfigHelper(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  newAuthority = null,
  newFeeCollector = null,
  newCreationFeeRecipient = null,
  newBaseCreationCost = null,
  newLootPercentage = null
) {
  try {
    const tx = await program.methods
      .updateConfig(
        newAuthority,
        newFeeCollector,
        newCreationFeeRecipient,
        newBaseCreationCost ? new BN(newBaseCreationCost) : null,
        newLootPercentage
      )
      .accounts({
        globalConfig: globalConfigPDA,
        moduleConfigStore: null,
        mineBtcMining: null,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    return { success: true, data: { txid: tx } };
  } catch (error) {
    return { success: false, error: error.toString() };
  }
}

/**
 * Helper function to toggle game active state
 */
export async function toggleGameActiveHelper(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA
) {
  try {
    const tx = await program.methods
      .toggleGameActive()
      .accounts({
        globalConfig: globalConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Fetch the updated state
    const globalConfig = await program.account.globalConfig.fetch(
      globalConfigPDA
    );

    return {
      success: true,
      data: {
        txid: tx,
        isGameActive: globalConfig.isGameActive,
      },
    };
  } catch (error) {
    return { success: false, error: error.toString() };
  }
}


/**
 * Helper function to fetch and display system status
 */
export async function getSystemStatus(
  program,
  globalConfigPDA,
  mineBtcMiningPDA
) {
  try {
    const globalConfig = await program.account.globalConfig.fetch(
      globalConfigPDA
    );
    const mineBtcMining = await program.account.mineBtcMining.fetch(
      mineBtcMiningPDA
    );

    return {
      success: true,
      data: {
        isGameActive: globalConfig.isGameActive,
        baseCreationCost: globalConfig.baseCreationCost,
        lootPercentage: globalConfig.lootPercentage,
        totalMinebtcsCreated: globalConfig.totalMinebtcsCreated,
        totalSolSpent: globalConfig.totalSolSpent,
        totalActiveHashpower: mineBtcMining.totalActiveHashpower,
        totalActiveElectricity: mineBtcMining.totalActiveElectricity,
        totalTokensMined: mineBtcMining.totalTokensMined,
        currentDistRate: mineBtcMining.currentDistRate,
        slotsForSwap: mineBtcMining.slotsForSwap,
        supportedFactions: globalConfig.supportedFactions,
      },
    };
  } catch (error) {
    return { success: false, error: error.toString() };
  }
}
 
