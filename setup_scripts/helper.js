import { 
  Connection, 
  PublicKey, 
  Keypair, 
  SystemProgram, 
  Transaction, 
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL
} from '@solana/web3.js';
import * as anchor_spl from '@solana/spl-token';
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
} from '@solana/spl-token';
import {
  createInitializeInstruction,
  pack,
} from '@solana/spl-token-metadata';
import * as web3 from '@solana/web3.js';
import anchorPkg from '@coral-xyz/anchor';
const { BN, Program } = anchorPkg;
import fs from 'fs';
import path from 'path';


// ----- [SEEDS] -----

// PDAs which hold GlobalConfig / DogeBtcMining state
export const GLOBAL_CONFIG_SEED = "global-config";
export const DOGE_BTC_MINING_SEED = "moon-doge-mining";

// PDAs which hold SOL collected by the program
export const SOL_TREASURY_SEED = "sol-treasury";

// MDOGE Custody PDAs: Vault Authority (signs for token account) & (vault token account custodies MDOGE tokens)
export const DOGE_BTC_VAULT_AUTHORITY_SEED = "mdoge-vault-authority";
export const DOGE_BTC_VAULT_SEED = "dbtc_vault";

// PDAs which hold ModuleConfigStore / GearConfigStore state
export const MODULE_CONFIG_STORE_SEED = "module-config-store";
export const MODULE_CONFIG_SEED = "module-config";

// PDAs which hold UserMoonBaseInstance / ReferralRewards state
export const USER_MOONBASE_SEED = "user-moonbase";
export const REFERRAL_REWARDS_SEED = "referral-rewards";

// PDAs which hold GearInstance / ModuleInstance state
export const MODULE_INSTANCE_SEED = "module-instance";

// pda which holds the mdoge nft vault state
export const dbtc_NFT_VAULT_SEED = "mdoge-nft-vault";

// PDAs for loot rewards system
export const LOOT_REWARDS_SEED = "loot-rewards";
export const LOOT_SOL_VAULT_SEED = "loot-sol-vault";
export const LOOT_DOGE_BTC_VAULT_SEED = "loot-mdoge-vault";
export const LOOT_DOGE_BTC_VAULT_AUTHORITY_SEED = "loot-mdoge-vault-authority";
export const LEVEL_STATS_SEED = "level-stats";

// PDA for PvP matchmaker
export const PVP_MATCHMAKER_SEED = "pvp-matchmaker";


export const MOON_ECONOMY_GLOBAL_CONFIG_SEED = "global_config";

export const MOON_ECONOMY_DOGE_BTC_VAULT_SEED = "dogebtc_vault";
export const MOON_ECONOMY_LIQUIDITY_VAULT_SEED = "liquidity_vault";

export const MOON_ECONOMY_DBTC_SOL_VAULT_SEED = "dogewifbtc-sol-vault";
export const MOON_ECONOMY_LP_SOL_VAULT_SEED = "lp-sol-vault";
export const MOON_ECONOMY_GAME_SOL_VAULT_SEED = "game-sol-vault";


export const MOON_ECONOMY_DBTC_CUSTODIAN_SEED = "dogewifbtc-custodian";
export const MOON_ECONOMY_DBTC_CUSTODIAN_AUTHORITY_SEED = "dogewifbtc-custodian-authority";


export const MOON_ECONOMY_LIQUIDITY_CUSTODIAN_SEED = "liquidity-custodian";
export const MOON_ECONOMY_LIQUIDITY_CUSTODIAN_AUTHORITY_SEED = "liquidity-custodian-authority";

export const MOON_ECONOMY_DEV_EARNINGS_SEED = "dev_earnings_collector";
export const MOON_ECONOMY_FEE_COLLECTOR_SEED = "fee_collector";

export const MOON_ECONOMY_USER_ELECTRICITY_SEED = "user-electricity";
export const MOON_ECONOMY_DBTC_POSITION_SEED = "dogewifbtc-position";
export const MOON_ECONOMY_LP_POSITION_SEED = "liquidity-position";

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
  const key = typeof pubkey === 'string' ? new PublicKey(pubkey) : pubkey;
  try {
    return await connection.getBalance(key);
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', `❌ Error getting SOL balance: ${error.message}`);
    throw error;
  }
}


// Add function to update deployment status
export function updateDeploymentStatus(network, status, data = {}, balance = 0) {
  const deploymentDir = path.join(process.cwd(), 'deployments');
  
  // Create deployments directory if it doesn't exist
  if (!fs.existsSync(deploymentDir)) {
      fs.mkdirSync(deploymentDir, { recursive: true });
  }
  
  const deploymentPath = path.join(deploymentDir, `${network}.json`);
  
  // Read existing deployment data or create new
  let deploymentData = {};
  if (fs.existsSync(deploymentPath)) {
      deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
  }
  
  // Update with new info
  deploymentData = {
      ...deploymentData,
      lastUpdated: new Date().toISOString(),
      lastStatus: status,
      deployerBalance: balance / 1e9,
      [status]: {
          ...data
      }
  };
  
  // Write back to file
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
  
  console.log(`Deployment status updated: ${status}`);
}




// Create mint account for a Solana Token 2022 with transfer fee (burn) extension
export async function createMintAccount(
    connection, 
    deployer, 
    moonDogeMint, 
    BURN_TAX_BPS, 
    burn_tax, 
    decimals,
    mintAuthority, 
    freezeAuthority,
    transferFeeConfigAuthority, 
    withdrawWithheldAuthority
) {
    try {
        console.log('\x1b[36m%s\x1b[0m', '📝 Creating mint account with transfer fee config...');
        
        const MAX_BURN_TAX = BigInt(burn_tax) * BigInt(10) ** BigInt(decimals);
        console.log('\x1b[36m%s\x1b[0m', `   • Max Burn Tax: ${MAX_BURN_TAX.toString()}`);
        
        // Create mint account with transfer fee extension
        const extensions = [ExtensionType.TransferFeeConfig];
        const mintLen = getMintLen(extensions);
        const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);
        
        console.log('\x1b[36m%s\x1b[0m', '🔧 Preparing instructions...');
        
        // Create account instruction
        const createAccountIx = SystemProgram.createAccount({
            fromPubkey: deployer.publicKey,
            newAccountPubkey: moonDogeMint.publicKey,
            space: mintLen,
            lamports,
            programId: TOKEN_2022_PROGRAM_ID,
        });
        
        // Initialize transfer fee config instruction
        const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
            moonDogeMint.publicKey,
            transferFeeConfigAuthority,
            withdrawWithheldAuthority,
            BURN_TAX_BPS,
            MAX_BURN_TAX,
            TOKEN_2022_PROGRAM_ID
        );
        
        // Initialize mint instruction
        const initMintIx = createInitializeMintInstruction(
            moonDogeMint.publicKey,
            decimals,
            mintAuthority,
            freezeAuthority,
            TOKEN_2022_PROGRAM_ID
        );
        
        console.log('\x1b[36m%s\x1b[0m', '📤 Building and sending transaction...');
        
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
                    [deployer, moonDogeMint],
                    {
                        commitment: 'confirmed',
                        maxRetries: 3,
                        preflightCommitment: 'confirmed'
                    }
                );
                console.log('\x1b[32m%s\x1b[0m', `✅ Token mint created successfully!`);
                console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${signature}`);
                return signature;
            } catch (error) {
                retries--;
                if (retries === 0) {
                    console.error('\x1b[31m%s\x1b[0m', `❌ Failed to create token mint: ${error.message}`);
                    throw error;
                }
                console.log('\x1b[33m%s\x1b[0m', `⚠️ Retrying transaction... (${retries} attempts remaining)`);
                await new Promise(resolve => setTimeout(resolve, 2000));
            }
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', `❌ Error in createMintAccount: ${error.message}`);
        throw error;
    }
} 

 

export async function createMintAccountWithMetadata(
  connection,
  deployer,
  moonDogeMint,
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
      console.log('\x1b[36m%s\x1b[0m', '📝 Creating mint account with transfer fee and metadata extensions (2-TX method)...');

      const MAX_BURN_TAX = BigInt(burn_tax) * BigInt(10) ** BigInt(decimals);
      console.log('\x1b[36m%s\x1b[0m', `   • Max Burn Tax: ${MAX_BURN_TAX.toString()}`);

      // 1. Define ALL extensions
      const extensions = [
          ExtensionType.MetadataPointer,
          ExtensionType.TransferFeeConfig // <-- This was missing in your original code
      ];
      const mintLen = getMintLen(extensions);

      const metadataPayloadLen = pack({
        updateAuthority: deployer.publicKey,
        mint: moonDogeMint.publicKey,
        name: metadata.name,
        symbol: metadata.symbol,
        uri: metadata.uri,
        additionalMetadata: metadata.additionalMetadata ?? [],
      }).length;
      const tlvLen = TYPE_SIZE + LENGTH_SIZE + metadataPayloadLen;

      // external metadata account (token-owned)
      const metaKp = Keypair.generate();

      console.log('\x1b[36m%s\x1b[0m', `   • Mint+Extensions Length: ${mintLen}`);
      console.log('\x1b[36m%s\x1b[0m', `   • Metadata TLV Length: ${tlvLen}`);

      console.log('\x1b[36m%s\x1b[0m', '🔧 Preparing instructions...');

      // ---------- lamports ----------
      const mintLamports = await connection.getMinimumBalanceForRentExemption(mintLen);
      const metaLamports = await connection.getMinimumBalanceForRentExemption(tlvLen);


      // === Transaction 1: Create Account + Init Native Extensions + Init Mint ===
      
      // ---------- TX 1: create + InitializeMint ----------
      const tx1 = new Transaction().add(
        // create mint (EXACT size = mintLen)
        SystemProgram.createAccount({
          fromPubkey: deployer.publicKey,
          newAccountPubkey: moonDogeMint.publicKey,
          lamports: mintLamports,
          space: mintLen,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        // initialize base mint FIRST (strict builds prefer this order)
        createInitializeMintInstruction(
          moonDogeMint.publicKey,
          decimals,
          mintAuthority,
          freezeAuthority,
          TOKEN_2022_PROGRAM_ID
        ),
      );
      let sig1 = await sendAndConfirmTransaction(connection, tx1, [deployer, moonDogeMint], {
        commitment: "confirmed", preflightCommitment: "confirmed", maxRetries: 3
      });
      console.log('\x1b[32m%s\x1b[0m', `✅ Mint initialized (tx1): ${sig1}`);

      // // 2. Init Metadata Pointer
      // const initMetadataPointerIx = createInitializeMetadataPointerInstruction(
      //     moonDogeMint.publicKey,
      //     deployer.publicKey, // update authority
      //     moonDogeMint.publicKey, // metadata stored on the mint (TLV)
      //     TOKEN_2022_PROGRAM_ID
      // );

      // // 3. Init Transfer Fee Config
      // const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
      //     moonDogeMint.publicKey,
      //     transferFeeConfigAuthority,
      //     withdrawWithheldAuthority,
      //     BURN_TAX_BPS,
      //     MAX_BURN_TAX,
      //     TOKEN_2022_PROGRAM_ID
      // );

      // // 4. Init Mint (MUST BE LAST in this group)
      // const initMintIx = createInitializeMintInstruction(
      //     moonDogeMint.publicKey,
      //     decimals,
      //     mintAuthority,
      //     freezeAuthority,
      //     TOKEN_2022_PROGRAM_ID
      // );

      // console.log('\x1b[36m%s\x1b[0m', '📤 Building and sending transaction 1 (Initialize Mint)...');
      
      // const tx1 = new Transaction()
      //     .add(createAccountIx)
      //     .add(initMetadataPointerIx)
      //     .add(initTransferFeeIx) // <-- This was the critical missing piece
      //     .add(initMintIx);
      
      // const sig1 = await sendAndConfirmTransaction(
      //     connection,
      //     tx1,
      //     [deployer, moonDogeMint],
      //     { commitment: 'confirmed', preflightCommitment: 'confirmed', maxRetries: 3 }
      // );
      // console.log('\x1b[32m%s\x1b[0m', `✅ Mint + Extensions initialized (tx1): ${sig1}`);

      // // === Transaction 2: Write Metadata using spl-token-metadata library ===

      // // 5. Init Metadata TLV
      // const initMetadataIx = createInitializeInstruction({
      //     programId: TOKEN_2022_PROGRAM_ID, // Must be 2022 program
      //     mint: moonDogeMint.publicKey,
      //     metadata: moonDogeMint.publicKey, // Write to the mint itself
      //     name: metadata.name,
      //     symbol: metadata.symbol,
      //     uri: metadata.uri,
      //     mintAuthority: deployer.publicKey,
      //     updateAuthority: deployer.publicKey,
      // });

      // console.log('\x1b[36m%s\x1b[0m', '📤 Building and sending transaction 2 (Write Metadata)...');
      
      // const tx2 = new Transaction().add(initMetadataIx);

      // const sig2 = await sendAndConfirmTransaction(
      //     connection,
      //     tx2,
      //     [deployer],
      //     { commitment: 'confirmed', preflightCommitment: 'confirmed', maxRetries: 3 }
      // );
      // console.log('\x1b[32m%s\x1b[0m', `✅ Metadata TLV initialized (tx2): ${sig2}`);
      
      // return sig2; // Return the signature for the final step

  } catch (error) {
      console.error('\x1b[31m%s\x1b[0m', `❌ Error in createMintAccountWithMetadata: ${error.message}`);
      throw error;
  }
}






export async function createMintAccount_T22_TransferFeeOnly(
  connection,
  deployer,
  moonDogeMint,
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
  console.log('\x1b[36m%s\x1b[0m', `   • Max Burn Tax: ${MAX_BURN_TAX.toString()}`);

  // We are ONLY using TransferFeeConfig at creation time
  const extensions = [ExtensionType.TransferFeeConfig];
  const mintLen = getMintLen(extensions);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

  // Create the mint account owned by Token-2022
  const createIx = SystemProgram.createAccount({
    fromPubkey: deployer.publicKey,
    newAccountPubkey: moonDogeMint.publicKey,
    space: mintLen,
    lamports,
    programId: TOKEN_2022_PROGRAM_ID,
  });

  // IMPORTANT: initialize the extension BEFORE InitializeMint
  const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
    moonDogeMint.publicKey,
    transferFeeConfigAuthority,
    withdrawWithheldAuthority,
    burnTaxBps,                                   // basis points
    MAX_BURN_TAX,                           // u64 in base units
    TOKEN_2022_PROGRAM_ID
  );

  const initMintIx = createInitializeMintInstruction(
    moonDogeMint.publicKey,
    decimals,
    mintAuthority,
    freezeAuthority,                              // can be null
    TOKEN_2022_PROGRAM_ID
  );

  const tx = new Transaction()
    .add(createIx)
    .add(initTransferFeeIx)   // <-- extension first
    .add(initMintIx);         // <-- then InitializeMint

  return await sendAndConfirmTransaction(connection, tx, [deployer, moonDogeMint], {
    commitment: "confirmed", preflightCommitment: "confirmed", maxRetries: 3,
  });
}













 
/**
 * Create a system referral account
 * @param {Connection} connection Solana connection
 * @param {Program} program MoonBase program instance
 * @param {Object} wallet Wallet instance
 * @param {Keypair} walletKeypair Wallet keypair for signing
 * @returns {Promise<Object>} Result object with success status and data
 */
export async function createSystemReferralAccount(connection, program, wallet, walletKeypair, referralRewardsPDA) { 
  try { 
            console.log('\x1b[36m%s\x1b[0m', `🔑 Referral Rewards PDA: ${referralRewardsPDA.toString()}`);
            
            // Build transaction
            const tx = await program.methods
                .createSystemReferralAccount()
                .accounts({
                    referrerRewards: referralRewardsPDA,
                    user: wallet.publicKey,
                    systemProgram: web3.SystemProgram.programId,
                })
                .transaction();
            
            // Send and confirm transaction
           console.log('\x1b[33m%s\x1b[0m', '📡 Sending create system referral account transaction...');
            const txid = await web3.sendAndConfirmTransaction(
                connection,
                tx,
                [walletKeypair]
            );
            
            console.log('\x1b[32m%s\x1b[0m', `✅ System referral account created successfully!`);
            console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${txid}`);
            console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=devnet`);
    
            return {
              success: true,
              data: {
              txid,
              referralRewards_address: referralRewardsPDA.toString(),
              }
    };
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log('\x1b[34m%s\x1b[0m', `ℹ️ System referral account already created. Skipping creation.`);
      
      // Still need to return the PDAs
      const [referralRewardsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(REFERRAL_REWARDS_SEED), Buffer.from(SystemProgram.key().toBase58())], 
        program.programId
      );
          
                return {
                  success: true,
                  data: {
                    referralRewards_address: referralRewardsPDA.toString(),
                  }
      };
            } else {
      console.error('\x1b[31m%s\x1b[0m', '❌ Error creating system referral account:', error);
                return {
                  success: false,
        error: error.toString(),
      };
    }
  }
}


/**
 * Initialize the MoonBase program
 * @param {Connection} connection Solana connection
 * @param {Program} program MoonBase program instance
 * @param {Object} wallet Wallet instance
 * @param {Keypair} walletKeypair Wallet keypair for signing
 * @param {number} baseCost Base creation cost in lamports
 * @param {PublicKey} creationFeeRecipient Address to receive creation fees
 * @returns {Promise<Object>} Result object with success status and data
 */
export async function initializeMoonbaseProgram(connection, program, wallet, walletKeypair, baseCost, creationFeeRecipient) { 
  try {
            // Define parameters
            const feeRecipient = new PublicKey(creationFeeRecipient);
            
            // Find PDAs
            const [globalConfigPDA] = PublicKey.findProgramAddressSync(  [Buffer.from(GLOBAL_CONFIG_SEED)],  program.programId);            
            const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync( [Buffer.from(DOGE_BTC_MINING_SEED)],   program.programId);            
            const [solTreasuryPDA] = PublicKey.findProgramAddressSync( [Buffer.from(SOL_TREASURY_SEED)],  program.programId);
    
            console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 Moon Doge Mining PDA: ${dogeBtcMiningPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 SOL Treasury PDA: ${solTreasuryPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 Creation Fee Recipient: ${feeRecipient.toString()}`);
            
            // Build transaction
    const tx = await program.methods
                .initialize(feeRecipient)
                .accounts({
                    globalConfig: globalConfigPDA,
                    dogeBtcMining: dogeBtcMiningPDA,
                    solTreasury: solTreasuryPDA,
                    authority: wallet.publicKey,
                    systemProgram: web3.SystemProgram.programId,
                })
                .transaction();
            
            // Send and confirm transaction
    console.log('\x1b[33m%s\x1b[0m', '📡 Sending initialize transaction...');
            const txid = await web3.sendAndConfirmTransaction(
                connection,
                tx,
                [walletKeypair]
            );
                  
          console.log('\x1b[32m%s\x1b[0m', `✅ Program initialized successfully!`);
          console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${txid}`);
          console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=devnet`);
          
            return {
              success: true,
              data: {
        txid,
                globalConfig_address: globalConfigPDA.toString(),
                dogeBtcMining_address: dogeBtcMiningPDA.toString(),
                solTreasury_address: solTreasuryPDA.toString(),
              }
    };
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log('\x1b[34m%s\x1b[0m', `ℹ️ Program already initialized. Skipping initialization.`);
      
      // Still need to return the PDAs
      const [globalConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(GLOBAL_CONFIG_SEED)], 
        program.programId
      );
      
      const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(DOGE_BTC_MINING_SEED)], 
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
                    dogeBtcMining_address: dogeBtcMiningPDA.toString(),
                    solTreasury_address: solTreasuryPDA.toString(),
                  }
      };
            } else {
      console.error('\x1b[31m%s\x1b[0m', '❌ Error initializing program:', error);
                return {
                  success: false,
        error: error.toString(),
      };
    }
  }
}



/**
 * Update the global config of the MoonBase program 
 */
export async function updateGlobalConfig(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA,
  dogeBtcMiningPDA,
  newAuthority = null,
  newFeeCollector = null,
  newCreationFeeRecipient = null,
  newBaseCreationCost = null,
  newLootPercentage = null
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating MoonBase global config...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Store PDA: ${moduleConfigStorePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DogeBtc Mining PDA: ${dogeBtcMiningPDA}`);
    
    if (newAuthority) console.log('\x1b[36m%s\x1b[0m', `   New Authority: ${newAuthority}`);
    if (newFeeCollector) console.log('\x1b[36m%s\x1b[0m', `   New Fee Collector (ext_fee_collector): ${newFeeCollector}`);
    if (newCreationFeeRecipient) console.log('\x1b[36m%s\x1b[0m', `   New Creation Fee Recipient: ${newCreationFeeRecipient}`);
    if (newBaseCreationCost) console.log('\x1b[36m%s\x1b[0m', `   New Base Creation Cost: ${newBaseCreationCost}`);
    if (newLootPercentage) console.log('\x1b[36m%s\x1b[0m', `   New Loot Percentage: ${newLootPercentage}`);
 
    const updateTx = await program.methods.updateConfig(
        newAuthority ? new PublicKey(newAuthority) : null,
        newFeeCollector ? new PublicKey(newFeeCollector) : null,
        newCreationFeeRecipient ? new PublicKey(newCreationFeeRecipient) : null,
        newBaseCreationCost ? new BN(newBaseCreationCost) : null,
        newLootPercentage
      )
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        moduleConfigStore: new PublicKey(moduleConfigStorePDA),
        dogeBtcMining: new PublicKey(dogeBtcMiningPDA),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ MoonBase global config updated`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);

    return {
      success: true,
      data: {
        updateTxid: updateTxid
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating MoonBase global config:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}



/**
 * Update the global config of the MoonEconomy program 
 */
export async function updateMoonEconomyGlobalConfig(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA,
  dogeBtcMiningPDA,
  newAuthority,
  newSolClaimer,
  newGameAuthority,
  newDogeCollection,
  newBaseCreationCost
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating global config...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Store PDA: ${moduleConfigStorePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Moon Doge Mining PDA: ${dogeBtcMiningPDA}`);
 
    const updateTx = await program.methods.updateConfig(
        new PublicKey(newAuthority),
        null,
        null,
        null,
        null
      )
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        moduleConfigStore: new PublicKey(moduleConfigStorePDA),
        dogeBtcMining: new PublicKey(dogeBtcMiningPDA),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ Global config updated`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        updateTxid: updateTxid
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating global config:', error);
    return {
      success: false,
      error: error.toString()
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
  dogeBtcMiningPDA,
  vaultPDA,
  vaultAuthorityPDA,
  tokenMint,
  token_program,
  start_timestamp,
  doge_btc_per_slot,
  raydium_pool_state
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing mining parameters...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Moon Doge Mining PDA: ${dogeBtcMiningPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Vault: ${vaultPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Vault Authority: ${vaultAuthorityPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Token Mint: ${tokenMint.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Token Program: ${token_program.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `⏰ Start Timestamp: ${start_timestamp}`);
    console.log('\x1b[90m%s\x1b[0m', `💰 Moon Doge Per Slot: ${doge_btc_per_slot.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `🔄 Raydium Pool State: ${raydium_pool_state.toString()}`);

    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_CONFIG_SEED)], 
      program.programId
    );

    const miningTx = await program.methods
      .initializeMining(
        new BN(start_timestamp),     // start_timestamp
        new BN(doge_btc_per_slot),  // doge_btc_per_slot (tokens per slot)
        new PublicKey(raydium_pool_state)  // pool_state (Raydium pool state)
      )
      .accounts({
        globalConfig: globalConfigPDA,
        dogeBtcMining: dogeBtcMiningPDA,
        vaultAuthority: vaultAuthorityPDA,
        tokenVault: vaultPDA,
        tokenMint: tokenMint,
        tokenProgram: token_program,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .transaction();
    
    const miningTxid = await web3.sendAndConfirmTransaction(connection, miningTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Mining initialized with token vault: ${vaultPDA.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${miningTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${miningTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        vaultAddress: vaultPDA.toString(),
        initMiningTxid: miningTxid,
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error setting up mining vault:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Deposit MDOGE tokens to the mining vault
 */
export async function depositMDOGE(
  connection, 
  program,
  wallet, 
  walletKeypair,
  userTokenAccount, // PublicKey of the user's token account
  vaultPDA,  // PublicKey of the mining token vault
  vaultAuthorityPDA,
  tokenMint,
  token_program,
  amount  // 21 billion with decimals
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', `📡 Depositing ${amount.toString()} MDOGE tokens to vault...`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 User Token Account: ${userTokenAccount.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Mining Token Vault: ${vaultPDA.toString()}`);
    
    // Find the moon doge mining PDA
    const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(DOGE_BTC_MINING_SEED)], 
      program.programId
    );
    
    // Create the deposit instruction
    const depositTx = await program.methods
      .depositDogeBtcTokens(amount)
      .accounts({
        depositor: wallet.publicKey,
        depositorTokenAccount: userTokenAccount,
        vaultAuthority: vaultAuthorityPDA,
        mdogeTokenVault: vaultPDA,
        tokenMint: tokenMint,
        dogeBtcMining: dogeBtcMiningPDA,
        tokenProgram: token_program,
      })
      .transaction();
    
    const depositTxid = await web3.sendAndConfirmTransaction(connection, depositTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Deposited ${amount.toString()} MDOGE tokens to vault`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${depositTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${depositTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        vaultAddress: vaultPDA.toString(),
        depositTxid: depositTxid
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error depositing MDOGE tokens:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}




/**
 * Initialize the config stores
 */
export async function initializeConfigStores(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing config stores...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Wallet: ${wallet.publicKey.toString()}`);

    const configStoreTx = await program.methods
      .initializeConfigStores()
      .accounts({
        globalConfig: globalConfigPDA,
        moduleConfigStore: moduleConfigStorePDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const configStoreTxid = await web3.sendAndConfirmTransaction(connection, configStoreTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Config stores initialized`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${configStoreTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        configStoreTxid: configStoreTxid
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error initializing config stores:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Initialize a new module config with basic info only (Step 1 of module creation)
 */
export async function addNewModuleToConfigStore(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleConfigStorePDA,
  moduleConfigName,
  moduleConfigImageUrl,
  moduleType, // "Mining", "Attraction", "Attack", "Research"
  moduleStats, // Type-specific stats object (for extracting maxHp and powerConsumption only)
  factionIds = [], // Array of faction IDs (empty = all factions)
  minLevel = 0,
  maxPerBase = 10,
  moduleConfigWidth,
  moduleConfigHeight,
  moduleConfigMintCost,
  moduleConfigUpgradeCost,
  upgradeRequirements = [] // Array of level requirements for each upgrade
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Adding NEW module config (basic info only)...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Store: ${moduleConfigStorePDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Name: ${moduleConfigName}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Type: ${moduleType}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Faction IDs: ${factionIds.join(', ') || 'All factions'}`);

    // First, get the current next_id from the module config store to derive the PDA
    const moduleConfigStoreAccount = await program.account.moduleConfigStore.fetch(moduleConfigStorePDA);
    const nextId = moduleConfigStoreAccount.nextId;
    
    // Convert nextId to buffer for PDA derivation (nextId is a regular number, not BN)
    const nextIdBuffer = Buffer.allocUnsafe(2);
    nextIdBuffer.writeUInt16LE(nextId, 0);
    
    // Derive the module config account PDA
    const [moduleConfigAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MODULE_CONFIG_SEED), nextIdBuffer],
      program.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Account PDA: ${moduleConfigAccountPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Next Module ID: ${nextId}`);

    // Map module types to integer values for contract
    // 1 = Mining, 2 = Attraction
    // Research and Attack modules map to Attraction (2)
    const moduleTypeMap = {
      'Mining': 1,
      'Attraction': 2,
      'Research': 2,  // Research modules become Attraction
      'Attack': 2     // Attack modules become Attraction
    };
    
    const moduleTypeInt = moduleTypeMap[moduleType] || 2; // Default to Attraction if unknown
    if (moduleType !== 'Mining' && moduleType !== 'Attraction') {
      console.log('\x1b[33m%s\x1b[0m', `⚠️  Module type "${moduleType}" mapped to integer ${moduleTypeInt} (Attraction)`);
    }
    
    // Ensure moduleTypeInt is a number (not string or object)
    const moduleTypeValue = typeof moduleTypeInt === 'number' ? moduleTypeInt : parseInt(moduleTypeInt, 10);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Type Integer: ${moduleTypeValue} (type: ${typeof moduleTypeValue})`);

    // Extract only maxHp and powerConsumption from stats for basic config
    const rawStats = moduleStats[moduleType.toLowerCase()] || moduleStats[moduleType] || moduleStats;
    const maxHp = rawStats.max_hp ?? rawStats.maxHp;
    const powerConsumption = rawStats.power_consumption ?? rawStats.powerConsumption;

    // Sanity check for required basic stats
    if (maxHp === undefined || powerConsumption === undefined) {
      throw new Error('Required basic stats (maxHp/powerConsumption) missing');
    }
 

      // ---- 2. BN only where u64 is required ----
      const mintCostBN    = new BN(moduleConfigMintCost);   // u64
      const upgradeCostBN = new BN(moduleConfigUpgradeCost);

      // ---- 3. Pass Vec<u8> as Buffer (Anchor serializes this as bytes) ----
      const factionVec   = Buffer.from(factionIds);      // e.g. Buffer.from([0, 3])
      const upgradeVec   = Buffer.from(upgradeRequirements); // e.g. Buffer.from([3,6,9,12...])

      console.log(`moduleConfigName = ${moduleConfigName}`);
      console.log(`moduleConfigImageUrl = ${moduleConfigImageUrl}`);
      console.log(`moduleTypeValue = ${moduleTypeValue}`);
      console.log(`factionVec = ${factionVec}`);
      console.log(`minLevel = ${minLevel}`);
      console.log(`moduleConfigWidth = ${moduleConfigWidth}`);
      console.log(`moduleConfigHeight = ${moduleConfigHeight}`);
      console.log(`mintCostBN = ${mintCostBN}`);
      console.log(`upgradeCostBN = ${upgradeCostBN}`);
      console.log(`upgradeVec = ${upgradeVec}`);


    // Prepare instruction - match exact simplified lib.rs signature
    const configStoreTx = await program.methods
      .addModuleToBase(
        moduleConfigName,          // string
        moduleConfigImageUrl,      // string
        moduleTypeValue,           // u8: 1 = Mining, 2 = Attraction
        factionVec,                // Vec<u8> - pass as plain array
        minLevel,                  // u8
        moduleConfigWidth,         // u8
        moduleConfigHeight,        // u8
        mintCostBN,                // u64
        upgradeCostBN,             // u64
        upgradeVec                 // Vec<u8> - pass as plain array
      )
      .accounts({
        globalConfig: globalConfigPDA,
        moduleConfigStore: moduleConfigStorePDA,
        moduleConfigAccount: moduleConfigAccountPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const configStoreTxid = await web3.sendAndConfirmTransaction(connection, configStoreTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Module config created (inactive until stats are set)`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${configStoreTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        addModuleTxid: configStoreTxid,
        moduleConfigAccountPDA: moduleConfigAccountPDA.toString(),
        moduleId: nextId
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error creating module config:', error);
    return {
      success: false,
      error: error.toString()
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
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating module config...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Store: ${moduleConfigStorePDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Name: ${moduleConfigName}`);

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
        dogeBtcMining: dogeBtcMiningPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const configStoreTxid = await web3.sendAndConfirmTransaction(connection, configStoreTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Module config updated`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${configStoreTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating module config:', error);
    return {
      success: false,
      error: error.toString()
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
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating gear config...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Gear Config Store: ${gearConfigStorePDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Gear Config PDA: ${gearConfigPDA.toString()}`);

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

    const configStoreTxid = await web3.sendAndConfirmTransaction(connection, configStoreTx, [walletKeypair]);
    console.log('\x1b[32m%s\x1b[0m', `✅ Gear config updated`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${configStoreTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${configStoreTxid}?cluster=devnet`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating gear config:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}
  
// ------------ xxx ------------------------ xxx ------------------------ xxx ------------------------ xxx ------------
// ------------ xxx ------------------------ xxx ------------------------ xxx ------------------------ xxx ------------
// ------------ xxx ------------ MOON ECONOMY ------------ xxx ------------
// ------------ xxx ------------------------ xxx ------------------------ xxx ------------------------ xxx ------------
// ------------ xxx ------------------------ xxx ------------------------ xxx ------------------------ xxx ------------






/**
 * Initialize the Moon Economy program
 */
export async function initializeMoonEconomyProgram(connection, program, wallet, walletKeypair, dev_address, moondoge_allocation, liquidity_allocation, min_lockup_days, max_lockup_days, base_multiplier, max_multiplier) { 

  console.log(`dev_address = ${dev_address}`);
  console.log(`moondoge_allocation = ${moondoge_allocation}`);
  console.log(`liquidity_allocation = ${liquidity_allocation}`);
  console.log(`min_lockup_days = ${min_lockup_days}`);
  console.log(`max_lockup_days = ${max_lockup_days}`);
  console.log(`base_multiplier = ${base_multiplier}`);
  console.log(`max_multiplier = ${max_multiplier}`);

  // Find PDAs
  const [globalConfigPDA] = PublicKey.findProgramAddressSync(  [Buffer.from(MOON_ECONOMY_GLOBAL_CONFIG_SEED)],  program.programId);            
  const [devEarningsPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_DEV_EARNINGS_SEED)],   program.programId);            
  const [feeCollectorPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_FEE_COLLECTOR_SEED)],  program.programId);
    
  try {

            console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 Dev Earnings PDA: ${devEarningsPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 Fee Collector PDA: ${feeCollectorPDA.toString()}`);
 
    const tx = await program.methods
                .initializeGlobalConfig(new PublicKey(dev_address), moondoge_allocation, liquidity_allocation, new BN(min_lockup_days), new BN(max_lockup_days), base_multiplier, max_multiplier)
                .accounts({
                    globalConfig: globalConfigPDA,
                    devEarningsCollector: devEarningsPDA,
                    feeCollector: feeCollectorPDA,
                    authority: wallet.publicKey,
                    systemProgram: web3.SystemProgram.programId,
                })
                .transaction();
            
            // Send and confirm transaction
    console.log('\x1b[33m%s\x1b[0m', '📡 Sending initialize transaction...');
            const txid = await web3.sendAndConfirmTransaction(
                connection,
                tx,
                [walletKeypair]
            );
            
    console.log('\x1b[32m%s\x1b[0m', `✅ Program initialized successfully!`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${txid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=devnet`);
    
            return {
              success: true,
              data: {
                txid,
                globalConfig_address: globalConfigPDA.toString(),
                devEarningsCollector_address: devEarningsPDA.toString(),
                feeCollector_address: feeCollectorPDA.toString(),
              }
    };
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log('\x1b[34m%s\x1b[0m', `ℹ️ Program already initialized. Skipping initialization.`);
      return {
        success: true,
        data: {
          globalConfig_address: globalConfigPDA.toString(),
          devEarningsCollector_address: devEarningsPDA.toString(),
          feeCollector_address: feeCollectorPDA.toString(),
        }
      };
    } else {
      console.error('\x1b[31m%s\x1b[0m', '❌ Error initializing program:', error);
      return {
        success: false,
        error: error.toString(),
      };
    }
  }
}
 


/**
 * Creates a token account for the program to use as mining vault and initializes mining parameters
 */
export async function mEconomySetupDbtcVault(
  connection, 
  program,
  wallet, 
  walletKeypair,
  mDogeMint,
  newTokenProgram,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing Moon Economy :: DOGE_BTC Vault...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Mint (SPL-2022): ${mDogeMint.toString()}`);

    console.log(`DEBUG: Program ID: ${program.programId.toString()}`);
    console.log(`DEBUG: Token-2022 Program ID: ${newTokenProgram.toString()}`);

    // Make sure parameters are PublicKeys
    const dbtcMintPubkey = new PublicKey(mDogeMint);

    console.log(`DEBUG: Converted mint addresses to PublicKeys`);
    console.log(`DEBUG: dbtcMintPubkey = ${dbtcMintPubkey.toString()}`);
    
    const [dogebtcVaultPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_DOGE_BTC_VAULT_SEED)],  program.programId );
    const [dbtcSolVaultPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_DBTC_SOL_VAULT_SEED)],  program.programId );
    const [dbtcCustodianAuthorityPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_DBTC_CUSTODIAN_AUTHORITY_SEED)],  program.programId );

    console.log(`DEBUG: PDAs generated for vaults and authorities`);
    console.log(`DEBUG: dogebtcVaultPDA = ${dogebtcVaultPDA.toString()}`);
    console.log(`DEBUG: dbtcSolVaultPDA = ${dbtcSolVaultPDA.toString()}`);
    console.log(`DEBUG: dbtcCustodianAuthorityPDA = ${dbtcCustodianAuthorityPDA.toString()}`);
 
    const [dbtcCustodianPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_DBTC_CUSTODIAN_SEED), dogebtcVaultPDA.toBytes()],  program.programId );
    
    console.log(`DEBUG: Generated custodian PDAs`);
    console.log(`DEBUG: dbtcCustodianPDA = ${dbtcCustodianPDA.toString()}`);
    
    // Create a transaction with all the accounts, ensuring rent is included
    console.log(`DEBUG: Preparing transaction accounts...`);
    const miningTx = await program.methods
        .initializeDbtcVault(
            dbtcMintPubkey
        )
        .accounts({
            dogebtcVault: dogebtcVaultPDA,
            dbtcSolVault: dbtcSolVaultPDA,
            dbtcCustodianAuthority: dbtcCustodianAuthorityPDA,
            dbtcCustodian: dbtcCustodianPDA,
            authority: wallet.publicKey,
            dbtcMint: dbtcMintPubkey,
            systemProgram: web3.SystemProgram.programId,
            tokenProgram: newTokenProgram,
            rent: web3.SYSVAR_RENT_PUBKEY
        })
        .transaction();
    
    console.log(`DEBUG: Transaction created, sending to network...`);
    
    // Send the transaction with increased confirmations to ensure it's finalized
    const miningTxid = await web3.sendAndConfirmTransaction(
        connection, 
        miningTx, 
        [walletKeypair],
        {
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            commitment: 'confirmed',
        }
    );
    
    console.log('\x1b[32m%s\x1b[0m', `✅ Vaults initialized successfully!`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${miningTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${miningTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        dogebtcVaultAddress: dogebtcVaultPDA.toString(),
        dbtcSolVaultAddress: dbtcSolVaultPDA.toString(),
        dbtcCustodianAuthorityAddress: dbtcCustodianAuthorityPDA.toString(),
        dbtcCustodianAddress: dbtcCustodianPDA.toString(),
        initMiningTxid: miningTxid,
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error setting up vaults:', error);
    // Print more detailed logs if they exist
    if (error.logs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Error logs:');
      error.logs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    } else if (error.transactionLogs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Transaction logs:');
      error.transactionLogs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    }
    return {
      success: false,
      error: error.toString()
    };
  }
}

  

export async function mEconomySetupLiquidityVaults(
  connection, 
  program,
  wallet, 
  walletKeypair,
  lpTokenMint,
  tokenProgram,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing Moon Economy :: Liquidity Vaults...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 LP Token Mint (standard SPL): ${lpTokenMint.toString()}`);
    console.log(`DEBUG: Program ID: ${program.programId.toString()}`);
    console.log(`DEBUG: Token Program ID: ${tokenProgram.toString()}`);

    // Make sure parameters are PublicKeys
    const lpTokenMintPubkey = new PublicKey(lpTokenMint);
    
    console.log(`DEBUG: Converted mint addresses to PublicKeys`);
    console.log(`DEBUG: lpTokenMintPubkey = ${lpTokenMintPubkey.toString()}`);

    const [liquidityVaultPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_LIQUIDITY_VAULT_SEED)],  program.programId );
    const [liquiditySolVaultPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_LP_SOL_VAULT_SEED)],  program.programId );
    const [liquidityCustodianAuthorityPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_LIQUIDITY_CUSTODIAN_AUTHORITY_SEED)],  program.programId );

    console.log(`DEBUG: PDAs generated for vaults and authorities`);
    console.log(`DEBUG: liquidityVaultPDA = ${liquidityVaultPDA.toString()}`);
 
    const [liquidityCustodianPDA] = PublicKey.findProgramAddressSync( [Buffer.from(MOON_ECONOMY_LIQUIDITY_CUSTODIAN_SEED), liquidityVaultPDA.toBytes()],  program.programId );
    
    console.log(`DEBUG: Generated custodian PDAs`);
    console.log(`DEBUG: liquidityCustodianPDA = ${liquidityCustodianPDA.toString()}`);
    
    // Create a transaction with all the accounts, ensuring rent is included
    console.log(`DEBUG: Preparing transaction accounts...`);
    const miningTx = await program.methods
        .initializeLiquidityVault(
            lpTokenMintPubkey
        )
        .accounts({
            liquidityVault: liquidityVaultPDA,
            liquiditySolVault: liquiditySolVaultPDA,
            liquidityCustodianAuthority: liquidityCustodianAuthorityPDA,
            liquidityCustodian: liquidityCustodianPDA,
            authority: wallet.publicKey,
            lpTokenMint: lpTokenMintPubkey,
            systemProgram: web3.SystemProgram.programId,
            tokenProgram: tokenProgram,
            rent: web3.SYSVAR_RENT_PUBKEY
        })
        .transaction();
    
    console.log(`DEBUG: Transaction created, sending to network...`);
    
    // Send the transaction with increased confirmations to ensure it's finalized
    const miningTxid = await web3.sendAndConfirmTransaction(
        connection, 
        miningTx, 
        [walletKeypair],
        {
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            commitment: 'confirmed',
        }
    );
    
    console.log('\x1b[32m%s\x1b[0m', `✅ Vaults initialized successfully!`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${miningTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${miningTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        liquidityVaultAddress: liquidityVaultPDA.toString(),
        liquiditySolVaultAddress: liquiditySolVaultPDA.toString(),
        liquidityCustodianAuthorityAddress: liquidityCustodianAuthorityPDA.toString(),
        liquidityCustodianAddress: liquidityCustodianPDA.toString(),
        initMiningTxid: miningTxid,
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error setting up vaults:', error);
    // Print more detailed logs if they exist
    if (error.logs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Error logs:');
      error.logs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    } else if (error.transactionLogs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Transaction logs:');
      error.transactionLogs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    }
    return {
      success: false,
      error: error.toString()
    };
  }
}

 

export async function mEconomy_claimMoonbaseSol(
  connection, 
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  dogebtcVaultPDA,
  liquidityVaultPDA,
  dbtcSolVaultPDA,
  liquiditySolVaultPDA,
  devEarningsPDA,
  moonbaseGlobalConfigPDA,
  moonbaseMiningStatePDA,
  moonbaseTreasuryPDA,
  feeCollectorPDA,
  lootSolVaultPDA,
  lootRewardsPDA,
  moonBaseProgramPDA,
) {

  // console.log(program);
  // console.log(program);

  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing Moon Economy :: Claim Moonbase SOL...');    
 
    
    console.log(`DEBUG: globalConfig: ${globalConfigPDA}`);
    console.log(`DEBUG: dogebtcVault: ${dogebtcVaultPDA}`);
    console.log(`DEBUG: liquidityVault: ${liquidityVaultPDA}`);
    console.log(`DEBUG: dbtcSolVault: ${dbtcSolVaultPDA}`);
    console.log(`DEBUG: liquiditySolVault: ${liquiditySolVaultPDA}`);
    console.log(`DEBUG: devEarningsPDA: ${devEarningsPDA}`);
    console.log(`DEBUG: moonbaseGlobalConfig: ${moonbaseGlobalConfigPDA}`);
    console.log(`DEBUG: moonbaseTreasury: ${moonbaseTreasuryPDA}`);
    console.log(`DEBUG: feeCollector: ${feeCollectorPDA}`);
    console.log(`DEBUG: moonBaseProgram: ${moonBaseProgramPDA}`);
    console.log(`DEBUG: lootSolVault: ${lootSolVaultPDA}`);
    console.log(`DEBUG: lootRewards: ${lootRewardsPDA}`);

    // Use the derived PDAs instead of the ones from the file
    const claimMoonbaseSolTx = await program.methods.claimMoonbaseSol()
        .accounts({
            globalConfig: new PublicKey(globalConfigPDA),
            dogebtcVault: new PublicKey(dogebtcVaultPDA),
            liquidityVault: new PublicKey(liquidityVaultPDA),
            dbtcSolVault: new PublicKey(dbtcSolVaultPDA),
            liquiditySolVault: new PublicKey(liquiditySolVaultPDA),
            devEarningsCollector: new PublicKey(devEarningsPDA),
            moonbaseGlobalConfig: new PublicKey(moonbaseGlobalConfigPDA),
            moonbaseMiningState: new PublicKey(moonbaseMiningStatePDA),
            moonbaseTreasury: new PublicKey(moonbaseTreasuryPDA),
            feeCollector: new PublicKey(feeCollectorPDA),
            lootSolVault: new PublicKey(lootSolVaultPDA),
            lootRewards: new PublicKey(lootRewardsPDA),
            moonFacilityProgram: new PublicKey(moonBaseProgramPDA),
            authority: wallet.publicKey,
            systemProgram: web3.SystemProgram.programId,
        })
        .transaction();
    
    console.log(`DEBUG: Transaction created, sending to network...`);
    
    // Send the transaction with increased confirmations to ensure it's finalized
    const claimMoonbaseSolTxid = await web3.sendAndConfirmTransaction(
        connection, 
        claimMoonbaseSolTx, 
        [walletKeypair],
        {
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            commitment: 'confirmed',
        }
    );
    
    console.log('\x1b[32m%s\x1b[0m', `✅ Moonbase SOL claimed successfully!`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${claimMoonbaseSolTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${claimMoonbaseSolTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        claimMoonbaseSolTxid: claimMoonbaseSolTxid,
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error claiming Moonbase SOL:', error);
    
    // Specific handling for SendTransactionError
    if (error.name === 'SendTransactionError') {
      console.error('\x1b[31m%s\x1b[0m', '📝 SendTransactionError detected. Getting detailed logs...');
      
      try {
        // Extract logs with getLogs() if available
        const detailedLogs = error.getLogs ? error.getLogs() : error.logs || error.transactionLogs;
        if (detailedLogs && detailedLogs.length > 0) {
          console.error('\x1b[31m%s\x1b[0m', '📝 Detailed Transaction Logs:');
          detailedLogs.forEach((log, i) => {
            console.error('\x1b[31m%s\x1b[0m', `[${i}] ${log}`);
          });
          
          // Extract Anchor program error if present
          const anchorErrorLog = detailedLogs.find(log => log.includes('AnchorError'));
          if (anchorErrorLog) {
            console.error('\x1b[33m%s\x1b[0m', '🔍 Anchor Error Found:', anchorErrorLog);
          }
        }
      } catch (logError) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to get detailed logs:', logError);
      }
    }
    // Fallback to standard log printing
    else if (error.logs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Error logs:');
      error.logs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    } else if (error.transactionLogs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Transaction logs:');
      error.transactionLogs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    }
    
    return {
      success: false,
      error: error.toString()
    };
  }
}
  



 

export async function mEconomy_withdrawDevEarnings(
  connection, 
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  devEarningsPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing Moon Economy :: Withdraw Dev Earnings...');    
    console.log(`DEBUG: Preparing transaction...`);
    
    console.log(`globalConfigPDA: ${globalConfigPDA}`);
    console.log(`devEarningsPDA: ${devEarningsPDA}`);
    
    // Use the derived PDAs instead of the ones from the file
    const withdrawDevEarningsTx = await program.methods.withdrawDevEarnings()
        .accounts({
            global_config: new PublicKey(globalConfigPDA),
            dev_earnings_collector: new PublicKey(devEarningsPDA),
            authority: wallet.publicKey,
            system_program: web3.SystemProgram.programId,
        })
        .transaction();
    
    console.log(`DEBUG: Transaction created, sending to network...`);
    
    // Send the transaction with increased confirmations to ensure it's finalized
    const withdrawDevEarningsTxid = await web3.sendAndConfirmTransaction(
        connection, 
        withdrawDevEarningsTx, 
        [walletKeypair],
        {
            skipPreflight: false,
            preflightCommitment: 'confirmed',
            commitment: 'confirmed',
        }
    );
    
    console.log('\x1b[32m%s\x1b[0m', `✅ Dev Earnings withdrawn successfully!`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${withdrawDevEarningsTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${withdrawDevEarningsTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        withdrawDevEarningsTxid: withdrawDevEarningsTxid,
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error withdrawing Dev Earnings:', error);
    
    // Specific handling for SendTransactionError
    if (error.name === 'SendTransactionError') {
      console.error('\x1b[31m%s\x1b[0m', '📝 SendTransactionError detected. Getting detailed logs...');
      
      try {
        // Extract logs with getLogs() if available
        const detailedLogs = error.getLogs ? error.getLogs() : error.logs || error.transactionLogs;
        if (detailedLogs && detailedLogs.length > 0) {
          console.error('\x1b[31m%s\x1b[0m', '📝 Detailed Transaction Logs:');
          detailedLogs.forEach((log, i) => {
            console.error('\x1b[31m%s\x1b[0m', `[${i}] ${log}`);
          });
          
          // Extract Anchor program error if present
          const anchorErrorLog = detailedLogs.find(log => log.includes('AnchorError'));
          if (anchorErrorLog) {
            console.error('\x1b[33m%s\x1b[0m', '🔍 Anchor Error Found:', anchorErrorLog);
          }
        }
      } catch (logError) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to get detailed logs:', logError);
      }
    }
    // Fallback to standard log printing
    else if (error.logs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Error logs:');
      error.logs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    } else if (error.transactionLogs) {
      console.error('\x1b[31m%s\x1b[0m', '📝 Transaction logs:');
      error.transactionLogs.forEach(log => console.error('\x1b[31m%s\x1b[0m', log));
    }
    
    return {
      success: false,
      error: error.toString()
    };
  }
}
  


// ----------------------------------------------------------------------------------------
// ------------ USER FUNCTIONS -------------------------------------------------------------
// ----------------------------------------------------------------------------------------

export async function createUserMoonbase(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  solTreasuryPDA,
  systemReferralPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Creating user moonbase...');

    // Derive PDAs properly
    const [userMoonbasePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(USER_MOONBASE_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );

    const [newUserRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(REFERRAL_REWARDS_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );

    // Derive the referrer rewards PDA using SystemProgram.programId as fallback
    // This matches what your Rust code does when no referrer is provided
    const [referrerRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(REFERRAL_REWARDS_SEED), web3.SystemProgram.programId.toBuffer()],
      program.programId
    );
     

    console.log('\x1b[36m%s\x1b[0m', `🔑 User Moonbase PDA: ${userMoonbasePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 New User Rewards PDA: ${newUserRewardsPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Referrer Rewards Key: ${systemReferralPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config Key: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Sol Treasury Key: ${solTreasuryPDA}`);
    console.log("Referrer Rewards PDA (final):", referrerRewardsPDA.toBase58());
    console.log("web3.SystemProgram.programId:",  web3.SystemProgram.programId.toBase58());
    // return;

    const updateTx = await program.methods.createUserMoonbase(new PublicKey("11111111111111111111111111111111")).accounts({
      user_moonbase: userMoonbasePDA,
      new_user_rewards: newUserRewardsPDA,
      referrerRewards: referrerRewardsPDA,
      global_config: new PublicKey(globalConfigPDA),
      sol_treasury: new PublicKey(solTreasuryPDA),
      user: wallet.publicKey,
      systemProgram: web3.SystemProgram.programId,
    })
    .transaction();
    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ User moonbase created`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error creating user moonbase:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}
 

export async function stakeMDOGE(
  connection,
  program,
  wallet,
  walletKeypair,
  amount,
  lockup_duration,
  index,
  mDogeMint,
  globalConfigPDA,
  dogebtcVaultPDA,
  dbtcCustodianPDA,
  moonbaseGlobalConfigPDA,
  UserMoonbaseInstancePDA,
  facilityMiningStatePDA,
  feeCollectorPDA,
  moonFacilityProgramPDA,
  tokenProgramPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Staking DOGE_BTC...');
    
    const amountNumber = new BN(amount);
    const lockupNumber = new BN(lockup_duration);
    const indexNumber = Number(index);

    let test = "ZMgahU99BRMx9mjUngYyZSSZDREGizKaN4qT3KMJyeY";

    let [userElectricityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_USER_ELECTRICITY_SEED), new PublicKey(test).toBuffer()], 
      program.programId
    );
    console.log(`userElectricityPDA: ${userElectricityPDA}`);

    return;

    let [userPositionPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_DBTC_POSITION_SEED), wallet.publicKey.toBuffer(), Buffer.from([indexNumber])], 
      program.programId
    );
    console.log(`userPositionPDA: ${userPositionPDA}`);

    console.log(`mDogeMint: ${mDogeMint}`);
    console.log(`tokenProgramPDA: ${tokenProgramPDA}`);

    // Fetch the user mdoge account PDA
    let userMdogeAccountPDA = await anchor_spl.getAssociatedTokenAddress(
      new PublicKey(mDogeMint), 
      wallet.publicKey, 
      false, 
      new PublicKey(tokenProgramPDA)
    );
    console.log('\x1b[36m%s\x1b[0m', `🔑 User DOGE_BTC Account PDA: ${userMdogeAccountPDA}`);

    const tokenAccount = await anchor_spl.getAccount(
      connection, 
      userMdogeAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available DOGE_BTC: ${tokenAccount.amount}`);

    console.log(`amountNumber: ${amountNumber}`);
    console.log(`lockupNumber: ${lockupNumber}`);
    console.log(`indexNumber: ${indexNumber}`);

    console.log(`globalConfigPDA: ${globalConfigPDA}`);
    console.log(`dogebtcVaultPDA: ${dogebtcVaultPDA}`);
    console.log(`userElectricityPDA: ${userElectricityPDA}`);
    console.log(`userPositionPDA: ${userPositionPDA}`);
    console.log(`userMdogeAccountPDA: ${userMdogeAccountPDA}`);
    console.log(`dbtcCustodianPDA: ${dbtcCustodianPDA}`);
    console.log(`moonbaseGlobalConfigPDA: ${moonbaseGlobalConfigPDA}`);
    console.log(`UserMoonbaseInstancePDA: ${UserMoonbaseInstancePDA}`);
    console.log(`facilityMiningStatePDA: ${facilityMiningStatePDA}`);
    console.log(`feeCollectorPDA: ${feeCollectorPDA}`);
    console.log(`moonFacilityProgramPDA: ${moonFacilityProgramPDA}`);
    console.log(`tokenProgramPDA: ${tokenProgramPDA}`);
    
    
    

    // Use the proper BN values in the transaction
    const updateTx = await program.methods.stakeMoondoge(
      amountNumber,                // Use BN for u64 
      lockupNumber,        // Use BN for u64
      indexNumber                 // Use Number for u8
    ).accounts({
      globalConfig: new PublicKey(globalConfigPDA),
      dogebtcVault: new PublicKey(dogebtcVaultPDA),
      electricityAc: new PublicKey(userElectricityPDA),
      userPosition: new PublicKey(userPositionPDA),
      dbtcMint: new PublicKey(mDogeMint),
      userMdogeAccount: new PublicKey(userMdogeAccountPDA),
      dbtcCustodian: new PublicKey(dbtcCustodianPDA),
      moonbaseGlobalConfig: new PublicKey(moonbaseGlobalConfigPDA),
      facilityUserMoonbase: new PublicKey(UserMoonbaseInstancePDA),
      facilityMiningState: new PublicKey(facilityMiningStatePDA),
      feeCollector: new PublicKey(feeCollectorPDA),
      moonFacilityProgram: new PublicKey(moonFacilityProgramPDA),
      authority: wallet.publicKey,
      systemProgram: web3.SystemProgram.programId,
      tokenProgram: new PublicKey(tokenProgramPDA),
    })
    .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ DOGE_BTC staked`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error staking DOGE_BTC:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}




export async function unStakeMDOGE(
  connection,
  program,
  wallet,
  walletKeypair,
  index,
  mDogeMint,
  globalConfigPDA,
  dogebtcVaultPDA,
  dbtcCustodianPDA,
  dbtcCustodianAuthorityPDA,
  moonbaseGlobalConfigPDA,
  UserMoonbaseInstancePDA,
  facilityMiningStatePDA,
  feeCollectorPDA,
  moonFacilityProgramPDA,
  tokenProgramPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Unstaking DOGE_BTC...');    
    const indexNumber = Number(index);

    let [userElectricityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_USER_ELECTRICITY_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );
    console.log(`userElectricityPDA: ${userElectricityPDA}`);

    let [userPositionPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_DBTC_POSITION_SEED), wallet.publicKey.toBuffer(), Buffer.from([indexNumber])], 
      program.programId
    );
    console.log(`userPositionPDA: ${userPositionPDA}`);
 

    // Fetch the user mdoge account PDA
    let userMdogeAccountPDA = await anchor_spl.getAssociatedTokenAddress(
      new PublicKey(mDogeMint), 
      wallet.publicKey, 
      false, 
      new PublicKey(tokenProgramPDA)
    );
    console.log('\x1b[36m%s\x1b[0m', `🔑 User DOGE_BTC Account PDA: ${userMdogeAccountPDA}`);

    const tokenAccount = await anchor_spl.getAccount(
      connection, 
      userMdogeAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available DOGE_BTC: ${tokenAccount.amount}`);
 
    // Use the proper BN values in the transaction
    const updateTx = await program.methods.unstakeMoondoge(indexNumber).accounts({
      globalConfig: new PublicKey(globalConfigPDA),
      dogebtcVault: new PublicKey(dogebtcVaultPDA),
      electricityAc: new PublicKey(userElectricityPDA),
      userPosition: new PublicKey(userPositionPDA),
      dbtcMint: new PublicKey(mDogeMint),
      userMdogeAccount: new PublicKey(userMdogeAccountPDA),
      dbtcCustodian: new PublicKey(dbtcCustodianPDA),
      dbtcCustodianAuthority: new PublicKey(dbtcCustodianAuthorityPDA),
      moonbaseGlobalConfig: new PublicKey(moonbaseGlobalConfigPDA),
      facilityUserMoonbase: new PublicKey(UserMoonbaseInstancePDA),
      facilityMiningState: new PublicKey(facilityMiningStatePDA),
      feeCollector: new PublicKey(feeCollectorPDA),
      moonFacilityProgram: new PublicKey(moonFacilityProgramPDA),
      authority: wallet.publicKey,
      systemProgram: web3.SystemProgram.programId,
      tokenProgram: new PublicKey(tokenProgramPDA),
    })
    .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ DOGE_BTC unstaked`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    const _tokenAccount = await anchor_spl.getAccount(
      connection, 
      userMdogeAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available DOGE_BTC (after unstake): ${_tokenAccount.amount}`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error unstaking DOGE_BTC:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}



export async function stakeLP(
  connection,
  program,
  wallet,
  walletKeypair,
  amount,
  lockup_duration,
  index,
  lpMint,
  globalConfigPDA,
  liquidityVaultPDA,
  liquidityCustodianPDA,
  moonbaseGlobalConfigPDA,
  UserMoonbaseInstancePDA,
  facilityMiningStatePDA,
  feeCollectorPDA,
  moonBaseProgramPDA,
  tokenProgramPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Staking LP...');
    
    const amountNumber = new BN(amount);
    const lockupNumber = new BN(lockup_duration);
    const indexNumber = Number(index);

    let [userElectricityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_USER_ELECTRICITY_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );
    console.log(`userElectricityPDA: ${userElectricityPDA}`);

    let [userPositionPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_LP_POSITION_SEED), wallet.publicKey.toBuffer(), Buffer.from([indexNumber])], 
      program.programId
    );
    console.log(`userPositionPDA: ${userPositionPDA}`);

    console.log(`lpMint: ${lpMint}`);
    console.log(`tokenProgramPDA: ${tokenProgramPDA}`);

    // Fetch the user mdoge account PDA
    let userLpAccountPDA = await anchor_spl.getAssociatedTokenAddress(
      new PublicKey(lpMint), 
      wallet.publicKey, 
      false, 
      new PublicKey(tokenProgramPDA)
    );
    console.log('\x1b[36m%s\x1b[0m', `🔑 User LP Account PDA: ${userLpAccountPDA}`);

    const tokenAccount = await anchor_spl.getAccount(
      connection, 
      userLpAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available LP: ${tokenAccount.amount}`);

 
    // Use the proper BN values in the transaction
    const updateTx = await program.methods.stakeLpTokens(
      amountNumber,                // Use BN for u64 
      lockupNumber,        // Use BN for u64
      indexNumber                 // Use Number for u8
    ).accounts({
      globalConfig: new PublicKey(globalConfigPDA),
      liquidityVault: new PublicKey(liquidityVaultPDA),
      electricityAc: new PublicKey(userElectricityPDA),
      userPosition: new PublicKey(userPositionPDA),
      userLpAccount: new PublicKey(userLpAccountPDA),
      liquidityCustodian: new PublicKey(liquidityCustodianPDA),
      moonbaseGlobalConfig: new PublicKey(moonbaseGlobalConfigPDA),
      facilityUserMoonbase: new PublicKey(UserMoonbaseInstancePDA),
      facilityMiningState: new PublicKey(facilityMiningStatePDA),
      feeCollector: new PublicKey(feeCollectorPDA),
      moonbaseProgram: new PublicKey(moonBaseProgramPDA),
      authority: wallet.publicKey,
      systemProgram: web3.SystemProgram.programId,
      tokenProgram: new PublicKey(tokenProgramPDA),
    })
    .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ LP staked`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error staking LP:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}




export async function unStakeLP(
  connection,
  program,
  wallet,
  walletKeypair,
  index,
  lpMint,
  globalConfigPDA,
  liquidityVaultPDA,
  liquidityCustodianPDA,
  liquidityCustodianAuthorityPDA,
  moonbaseGlobalConfigPDA,
  UserMoonbaseInstancePDA,
  facilityMiningStatePDA,
  feeCollectorPDA,
  moonBaseProgramPDA,
  tokenProgramPDA,
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Unstaking LP...');    
    const indexNumber = Number(index);

    let [userElectricityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_USER_ELECTRICITY_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );
    console.log(`userElectricityPDA: ${userElectricityPDA}`);

    let [userPositionPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MOON_ECONOMY_LP_POSITION_SEED), wallet.publicKey.toBuffer(), Buffer.from([indexNumber])], 
      program.programId
    );
    console.log(`userPositionPDA: ${userPositionPDA}`);
 

    // Fetch the user mdoge account PDA
    let userLpAccountPDA = await anchor_spl.getAssociatedTokenAddress(
      new PublicKey(lpMint), 
      wallet.publicKey, 
      false, 
      new PublicKey(tokenProgramPDA)
    );
    console.log('\x1b[36m%s\x1b[0m', `🔑 User LP Account PDA: ${userLpAccountPDA}`);

    const tokenAccount = await anchor_spl.getAccount(
      connection, 
      userLpAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available LP: ${tokenAccount.amount}`);
 
    // Use the proper BN values in the transaction
    const updateTx = await program.methods.unstakeLpTokens(indexNumber).accounts({
      globalConfig: new PublicKey(globalConfigPDA),
      liquidityVault: new PublicKey(liquidityVaultPDA),
      electricityAc: new PublicKey(userElectricityPDA),
      userPosition: new PublicKey(userPositionPDA),
      userLpAccount: new PublicKey(userLpAccountPDA),
      liquidityCustodian: new PublicKey(liquidityCustodianPDA),
      liquidityCustodianAuthority: new PublicKey(liquidityCustodianAuthorityPDA),
      lpMint: new PublicKey(lpMint),
      moonbaseGlobalConfig: new PublicKey(moonbaseGlobalConfigPDA),
      facilityUserMoonbase: new PublicKey(UserMoonbaseInstancePDA),
      facilityMiningState: new PublicKey(facilityMiningStatePDA),
      feeCollector: new PublicKey(feeCollectorPDA),
      moonbaseProgram: new PublicKey(moonBaseProgramPDA),
      authority: wallet.publicKey,
      systemProgram: web3.SystemProgram.programId,
      tokenProgram: new PublicKey(tokenProgramPDA),
    })
    .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ LP unstaked`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    const _tokenAccount = await anchor_spl.getAccount(
      connection, 
      userLpAccountPDA, 
      undefined, 
      tokenProgramPDA
    );
    console.log(`available LP (after unstake): ${_tokenAccount.amount}`);

    return {
      success: true,
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error unstaking LP:', error);
    return {
      success: false,
      error: error.toString()
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
  const mintInfo = await anchor_spl.getMint(connection, new PublicKey(mintAddress), undefined, TOKEN_2022_PROGRAM_ID);
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

















export async function debugPDAs(program, networkConfig) {
  console.log('\x1b[35m%s\x1b[0m', '\n================ [ DEBUG: PDA DERIVATION ] =================');
  
  // Derive PDAs using the current program
  const [derivedGlobalConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_GLOBAL_CONFIG_SEED)], 
    program.programId
  );
  
  const [derivedMoondogeVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_DOGE_BTC_VAULT_SEED)], 
    program.programId
  );
  
  const [derivedLiquidityVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_LIQUIDITY_VAULT_SEED)], 
    program.programId
  );
  
  const [derivedMdogeSolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_DBTC_SOL_VAULT_SEED)], 
    program.programId
  );
  
  const [derivedLiquiditySolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_LP_SOL_VAULT_SEED)], 
    program.programId
  );
  
  const [derivedGameSolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_GAME_SOL_VAULT_SEED)], 
    program.programId
  );
  
  const [derivedDevEarningsPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_DEV_EARNINGS_SEED)], 
    program.programId
  );
  
  const [derivedFeeCollectorPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(MOON_ECONOMY_FEE_COLLECTOR_SEED)], 
    program.programId
  );
  
  // Compare derived addresses with deployment file addresses
  const comparePDAs = (derived, deployed, name) => {
    const match = derived.toString() === deployed;
    console.log(
      match ? '\x1b[32m%s\x1b[0m' : '\x1b[31m%s\x1b[0m',
      `${name}: ${match ? '✓ MATCH' : '✗ MISMATCH'}`
    );
    console.log('\x1b[36m%s\x1b[0m', `  Derived: ${derived.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `  Deployed: ${deployed}`);
    if (!match) {
      // Try deriving with different seed variations to help diagnose the issue
      const seedVariations = [
        { name: "with underscore", seed: Buffer.from(name.toLowerCase().replace(/-/g, '_')) },
        { name: "with hyphen", seed: Buffer.from(name.toLowerCase().replace(/_/g, '-')) },
        { name: "lowercase", seed: Buffer.from(name.toLowerCase()) },
        { name: "uppercase", seed: Buffer.from(name.toUpperCase()) },
      ];
      
      console.log('\x1b[33m%s\x1b[0m', '  Diagnostic attempts with different seeds:');
      for (const variation of seedVariations) {
        try {
          const [testPDA] = PublicKey.findProgramAddressSync(
            [variation.seed], 
            program.programId
          );
          const testMatch = testPDA.toString() === deployed;
          console.log(
            testMatch ? '\x1b[32m%s\x1b[0m' : '\x1b[90m%s\x1b[0m',
            `  - Using seed ${variation.name}: ${testPDA.toString()}${testMatch ? ' ✓ MATCH!' : ''}`
          );
        } catch (err) {
          console.log('\x1b[31m%s\x1b[0m', `  - Error with ${variation.name}: ${err.message}`);
        }
      }
    }
    return match;
  };
  
  // Compare all PDAs
  console.log('\n\x1b[33m%s\x1b[0m', '🔍 Comparing derived vs deployed PDAs:');
  
  const results = {
    globalConfig: comparePDAs(
      derivedGlobalConfigPDA, 
      networkConfig.moonEconomy_program_initialized?.globalConfig_data_ac || '', 
      'Global Config'
    ),
    dogebtcVault: comparePDAs(
      derivedMoondogeVaultPDA, 
      networkConfig.moonEconomy_mDogeVault_initialized?.dogebtcVault || '', 
      'DogeBtc Vault'
    ),
    liquidityVault: comparePDAs(
      derivedLiquidityVaultPDA, 
      networkConfig.moonEconomy_liquidityVault_initialized?.liquidityVault || '', 
      'Liquidity Vault'
    ),
    dbtcSolVault: comparePDAs(
      derivedMdogeSolVaultPDA, 
      networkConfig.moonEconomy_mDogeVault_initialized?.dbtcSolVault || '', 
      'DogeBtc SOL Vault'
    ),
    liquiditySolVault: comparePDAs(
      derivedLiquiditySolVaultPDA, 
      networkConfig.moonEconomy_liquidityVault_initialized?.liquiditySolVault || '', 
      'Liquidity SOL Vault'
    ),
    devEarningsCollector: comparePDAs(
      derivedDevEarningsPDA, 
      networkConfig.moonEconomy_program_initialized?.devEarnings_data_ac || '', 
      'Dev Earnings Collector'
    ),
    feeCollector: comparePDAs(
      derivedFeeCollectorPDA, 
      networkConfig.moonEconomy_program_initialized?.feeCollector_data_ac || '', 
      'Fee Collector'
    ),
  };
  
  // Summary
  const matchCount = Object.values(results).filter(r => r).length;
  const totalCount = Object.values(results).length;
  
  console.log('\n\x1b[35m%s\x1b[0m', `📊 PDA Match Summary: ${matchCount}/${totalCount} PDAs match`);
  
  if (matchCount < totalCount) {
    console.log('\x1b[33m%s\x1b[0m', '⚠️ Some PDAs do not match their deployed counterparts.');
    console.log('\x1b[33m%s\x1b[0m', '   This could indicate seed mismatches between deployment and current code.');
  } else {
    console.log('\x1b[32m%s\x1b[0m', '✅ All PDAs match their deployed counterparts.');
  }
  
  // Additional debug info about seed strings
  console.log('\n\x1b[36m%s\x1b[0m', '🧪 Seed strings used for derivation:');
  console.log('\x1b[36m%s\x1b[0m', `  - Global Config: "${MOON_ECONOMY_GLOBAL_CONFIG_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - DogeBtc Vault: "${MOON_ECONOMY_DOGE_BTC_VAULT_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - Liquidity Vault: "${MOON_ECONOMY_LIQUIDITY_VAULT_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - DogeBtc SOL Vault: "${MOON_ECONOMY_DBTC_SOL_VAULT_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - Liquidity SOL Vault: "${MOON_ECONOMY_LP_SOL_VAULT_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - Game SOL Vault: "${MOON_ECONOMY_GAME_SOL_VAULT_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - Dev Earnings Collector: "${MOON_ECONOMY_DEV_EARNINGS_SEED}"`);
  console.log('\x1b[36m%s\x1b[0m', `  - Fee Collector: "${MOON_ECONOMY_FEE_COLLECTOR_SEED}"`);
  
  return {
    matchCount,
    totalCount,
    details: results
  };
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

  const txid = await web3.sendAndConfirmTransaction(connection, tx, [walletKeypair]);
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

  const txid = await web3.sendAndConfirmTransaction(connection, tx, [walletKeypair]);
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
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [walletKeypair]);
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
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [walletKeypair]);
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
  const txid = await web3.sendAndConfirmTransaction(connection, tx, [walletKeypair]);
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
  dogeBtcMiningPDA,
  newSlotsPerHour
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating slots per hour configuration...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Moon Doge Mining PDA: ${dogeBtcMiningPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `⏰ New slots per hour: ${newSlotsPerHour}`);

    const updateTx = await program.methods.updateSlotsPerHour(
        new BN(newSlotsPerHour)
      )
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        dogeBtcMining: new PublicKey(dogeBtcMiningPDA),
        authority: wallet.publicKey,
      })
      .transaction();

    const updateTxid = await web3.sendAndConfirmTransaction(connection, updateTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ Slots per hour updated to ${newSlotsPerHour}`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        updateTxid: updateTxid,
        newSlotsPerHour: newSlotsPerHour
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating slots per hour:', error);
    return {
      success: false,
      error: error.toString()
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
  dogeBtcMiningPDA,
  factionNames
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Adding factions...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🏴 Factions: ${factionNames.join(', ')}`);

    const addFactionsTxid = await program.methods.addFactions(factionNames)
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        moduleConfigStore: new PublicKey(moduleConfigStorePDA),
        dogeBtcMining: new PublicKey(dogeBtcMiningPDA),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log('\x1b[32m%s\x1b[0m', `✅ Added ${factionNames.length} factions successfully`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${addFactionsTxid}`);

    return {
      success: true,
      data: {
        addFactionsTxid: addFactionsTxid,
        factionNames: factionNames
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error adding factions:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Create user moonbase with faction support
 */
export async function createUserMoonbaseWithFaction(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  solTreasuryPDA,
  systemReferralPDA,
  factionId,
  referrer = null
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Creating user moonbase with faction...');
    console.log('\x1b[36m%s\x1b[0m', `🏴 Faction ID: ${factionId}`);

    // Derive PDAs properly
    const [userMoonbasePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(USER_MOONBASE_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );

    const [newUserRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(REFERRAL_REWARDS_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );

    // Derive the referrer rewards PDA
    const referrerKey = referrer || web3.SystemProgram.programId;
    const [referrerRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(REFERRAL_REWARDS_SEED), referrerKey.toBuffer()],
      program.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 User Moonbase PDA: ${userMoonbasePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 New User Rewards PDA: ${newUserRewardsPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Referrer Rewards PDA: ${referrerRewardsPDA}`);

    const createTx = await program.methods.createUserMoonbase(referrer, factionId)
      .accounts({
        userMoonbase: userMoonbasePDA,
        newUserRewards: newUserRewardsPDA,
        referrerRewards: referrerRewardsPDA,
        globalConfig: new PublicKey(globalConfigPDA),
        solTreasury: new PublicKey(solTreasuryPDA),
        user: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const createTxid = await web3.sendAndConfirmTransaction(connection, createTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ User moonbase created with faction ID ${factionId}`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${createTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${createTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        createTxid: createTxid,
        userMoonbasePDA: userMoonbasePDA.toString(),
        factionId: factionId
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error creating user moonbase with faction:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Process daily login and award XP if eligible
 */
export async function dailyLogin(
  connection,
  program,
  wallet,
  walletKeypair
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Processing daily login...');

    // Derive user moonbase PDA
    const [userMoonbasePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(USER_MOONBASE_SEED), wallet.publicKey.toBuffer()], 
      program.programId
    );

    const dailyLoginTx = await program.methods.dailyLogin()
      .accounts({
        userMoonbase: userMoonbasePDA,
        user: wallet.publicKey,
      })
      .transaction();

    const dailyLoginTxid = await web3.sendAndConfirmTransaction(connection, dailyLoginTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ Daily login processed`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${dailyLoginTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${dailyLoginTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        dailyLoginTxid: dailyLoginTxid
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error processing daily login:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Calculate XP required for a specific level
 * Formula: required_xp = 100 + (level ^ 2 × 20)
 */
export function calculateRequiredXP(level) {
  return 100 + (level * level * 20);
}

/**
 * Calculate referral XP based on both number of referrals and total SOL earned
 * Formula: (referrals_count * 100) + (sqrt(total_sol_earned_lamports) / 1000)
 */
export function calculateReferralXP(referrals_count, total_sol_earned_lamports) {
  // Base XP from referral count (100 XP per referral)
  const base_xp = referrals_count * 100;
  
  // Bonus XP from total SOL earned (square root to prevent excessive scaling)
  const sol_bonus_xp = total_sol_earned_lamports > 0 
    ? Math.floor(Math.sqrt(total_sol_earned_lamports) / 1000)
    : 0;
  
  return base_xp + sol_bonus_xp;
}

/**
 * Get user level and XP information
 */
export async function getUserLevelInfo(
  connection,
  program,
  userPublicKey
) {
  try {
    // Derive user moonbase PDA
    const [userMoonbasePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(USER_MOONBASE_SEED), userPublicKey.toBuffer()], 
      program.programId
    );

    // Fetch user moonbase account
    const userMoonbaseAccount = await program.account.userMoonBaseInstance.fetch(userMoonbasePDA);
    
    const currentLevel = userMoonbaseAccount.level;
    const currentXP = userMoonbaseAccount.xp;
    const requiredXPForNextLevel = calculateRequiredXP(currentLevel + 1);
    const xpToNextLevel = requiredXPForNextLevel - currentXP;
    
    return {
      success: true,
      data: {
        level: currentLevel,
        xp: currentXP,
        requiredXPForNextLevel: requiredXPForNextLevel,
        xpToNextLevel: Math.max(0, xpToNextLevel),
        lastLoginTs: userMoonbaseAccount.lastLoginTs,
        dailyLoginStreak: userMoonbaseAccount.dailyLoginStreak,
        factionId: userMoonbaseAccount.factionId
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error getting user level info:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Initialize the loot rewards system (admin only)
 */
export async function initializeLootRewards(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  dbtcMint
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing loot rewards system...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 DOGE_BTC Mint: ${dbtcMint}`);

    // Derive loot rewards PDAs
    const [lootRewardsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(LOOT_REWARDS_SEED)], 
      program.programId
    );
    
    const [lootSolVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(LOOT_SOL_VAULT_SEED)], 
      program.programId
    );
    
    const [lootMdogeVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(LOOT_DOGE_BTC_VAULT_SEED)], 
      program.programId
    );
    
    const [lootMdogeVaultAuthorityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(LOOT_DOGE_BTC_VAULT_AUTHORITY_SEED)], 
      program.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 Loot Rewards PDA: ${lootRewardsPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Loot SOL Vault PDA: ${lootSolVaultPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Loot DOGE_BTC Vault PDA: ${lootMdogeVaultPDA}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Loot DOGE_BTC Vault Authority PDA: ${lootMdogeVaultAuthorityPDA}`);

    const initTx = await program.methods.initializeLootRewards()
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        lootRewards: lootRewardsPDA,
        lootSolVault: lootSolVaultPDA,
        lootMdogeVault: lootMdogeVaultPDA,
        lootMdogeVaultAuthority: lootMdogeVaultAuthorityPDA,
        dbtcMint: new PublicKey(dbtcMint),
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
        tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .transaction();

    const initTxid = await web3.sendAndConfirmTransaction(connection, initTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ Loot rewards system initialized`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${initTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${initTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        initTxid: initTxid,
        lootRewardsPDA: lootRewardsPDA.toString(),
        lootSolVaultPDA: lootSolVaultPDA.toString(),
        lootMdogeVaultPDA: lootMdogeVaultPDA.toString(),
        lootMdogeVaultAuthorityPDA: lootMdogeVaultAuthorityPDA.toString(),
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error initializing loot rewards system:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

/**
 * Initialize level statistics tracking (admin only)
 */
export async function initializeLevelStats(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Initializing level statistics tracking...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Global Config PDA: ${globalConfigPDA}`);

    // Derive level stats PDA
    const [levelStatsPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(LEVEL_STATS_SEED)], 
      program.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 Level Stats PDA: ${levelStatsPDA}`);

    const initTx = await program.methods.initializeLevelStats()
      .accounts({
        globalConfig: new PublicKey(globalConfigPDA),
        levelStats: levelStatsPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const initTxid = await web3.sendAndConfirmTransaction(connection, initTx, [walletKeypair]);

    console.log('\x1b[32m%s\x1b[0m', `✅ Level statistics tracking initialized`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${initTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${initTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        initTxid: initTxid,
        levelStatsPDA: levelStatsPDA.toString(),
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error initializing level statistics tracking:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
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
                dogeBtcMining: null,
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
        const globalConfig = await program.account.globalConfig.fetch(globalConfigPDA);

        return { 
            success: true, 
            data: { 
                txid: tx,
                isGameActive: globalConfig.isGameActive
            } 
        };
    } catch (error) {
        return { success: false, error: error.toString() };
    }
}

/**
 * Helper function to update slots for swap
 */
export async function updateSlotsForSwapHelper(
    connection,
    program,
    wallet,
    walletKeypair,
    globalConfigPDA,
    dogeBtcMiningPDA,
    newSlotsForSwap
) {
    try {
        const tx = await program.methods
            .updateSlotsForSwap(new BN(newSlotsForSwap))
            .accounts({
                globalConfig: globalConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: wallet.publicKey,
            })
            .rpc();

        return { success: true, data: { txid: tx, newSlotsForSwap } };
    } catch (error) {
        return { success: false, error: error.toString() };
    }
}

/**
 * Helper function to update module configuration
 */
export async function updateModuleConfigHelper(
    connection,
    program,
    wallet,
    walletKeypair,
    globalConfigPDA,
    moduleId,
    newImageUrl = null,
    newFactionIds = null,
    newMaxPerBase = null,
    newMintCost = null,
    newUpgradeCost = null,
    newUpgradeLevelRequirements = null,
    isActive = null
) {
    try {
        // Derive module config account PDA
        const moduleIdBuffer = Buffer.allocUnsafe(2);
        moduleIdBuffer.writeUInt16LE(moduleId, 0);
        
        const [moduleConfigAccountPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from(MODULE_CONFIG_SEED), moduleIdBuffer],
            program.programId
        );

        const tx = await program.methods
            .updateModule(
                moduleId,
                newImageUrl,
                newFactionIds ? Buffer.from(newFactionIds) : null,
                newMaxPerBase,
                newMintCost ? new BN(newMintCost) : null,
                newUpgradeCost ? new BN(newUpgradeCost) : null,
                newUpgradeLevelRequirements ? Buffer.from(newUpgradeLevelRequirements) : null,
                isActive
            )
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigAccount: moduleConfigAccountPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        return { success: true, data: { txid: tx, moduleId } };
    } catch (error) {
        return { success: false, error: error.toString() };
    }
}

/**
 * Helper function to fetch and display system status
 */
export async function getSystemStatus(program, globalConfigPDA, dogeBtcMiningPDA) {
    try {
        const globalConfig = await program.account.globalConfig.fetch(globalConfigPDA);
        const dogeBtcMining = await program.account.dogeBtcMining.fetch(dogeBtcMiningPDA);
        
        return {
            success: true,
            data: {
                isGameActive: globalConfig.isGameActive,
                baseCreationCost: globalConfig.baseCreationCost,
                lootPercentage: globalConfig.lootPercentage,
                totalMoonbasesCreated: globalConfig.totalMoonbasesCreated,
                totalSolSpent: globalConfig.totalSolSpent,
                totalActiveHashpower: dogeBtcMining.totalActiveHashpower,
                totalActiveElectricity: dogeBtcMining.totalActiveElectricity,
                totalTokensMined: dogeBtcMining.totalTokensMined,
                currentDistRate: dogeBtcMining.currentDistRate,
                slotsForSwap: dogeBtcMining.slotsForSwap,
                supportedFactions: globalConfig.supportedFactions
            }
        };
    } catch (error) {
        return { success: false, error: error.toString() };
    }
}
  

export async function updateMdogeDistPerSlot(
  connection,
  program,
  raydiumProgramId,
  wallet,
  walletKeypair,
  globalConfigPDA,
  dogeBtcMiningPDA,
  raydiumPoolData,
  dbtcMint,
  ammConfigPDA,
  solTreasuryPDA,
  vaultAuthorityPDA,
  dbtcTokenAccount
  
) {
  try {
    console.log('\x1b[36m%s\x1b[0m', '🔄 Updating DOGE_BTC distribution per slot...');

    // Extract pool data from deployment info
    const poolStatePDA = new PublicKey(raydiumPoolData.poolStatePDA);
    const lpMintPDA = new PublicKey(raydiumPoolData.lpMintPDA);
    const authorityPDA = new PublicKey(raydiumPoolData.authorityPDA);
    const observationStatePDA = new PublicKey(raydiumPoolData.observationStatePDA);
    const solMint = new PublicKey("So11111111111111111111111111111111111111112"); // WSOL
    
    // Determine correct vault assignment based on token ordering
    const isMdogeToken0 = raydiumPoolData.isMdogeToken0;
    const token0VaultPDA = new PublicKey(raydiumPoolData.token0VaultPDA); // This is WSOL vault (token0)
    const token1VaultPDA = new PublicKey(raydiumPoolData.token1VaultPDA); // This is DOGE_BTC vault (token1)
    
    // Assign vaults based on actual token ordering
    const solVaultPDA = isMdogeToken0 ? token1VaultPDA : token0VaultPDA;   // WSOL vault
    const dbtcVaultPDA = isMdogeToken0 ? token0VaultPDA : token1VaultPDA; // DOGE_BTC vault
    
    console.log('\x1b[90m%s\x1b[0m', `   Token ordering: isMdogeToken0=${isMdogeToken0}`);
    console.log('\x1b[90m%s\x1b[0m', `   WSOL vault (solVaultPDA): ${solVaultPDA.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `   DOGE_BTC vault (dbtcVaultPDA): ${dbtcVaultPDA.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `   token0VaultPDA: ${token0VaultPDA.toString()}`);
    console.log('\x1b[90m%s\x1b[0m', `   token1VaultPDA: ${token1VaultPDA.toString()}`)

    // First, try to get the Raydium pool data to determine if it's properly set up
    const poolData = await connection.getAccountInfo(poolStatePDA);
    if (!poolData) {
      console.log('\x1b[33m%s\x1b[0m', '⚠️ Raydium pool state not found. Pool may not be properly initialized.');
      return {
        success: false,
        error: "Raydium pool state not found - pool may not be properly initialized"
      };
    }

    // For Raydium deposit, the LP token account must be owned by the same authority that owns the other token accounts
    // This should be the vault authority PDA, not the LP token authority PDA
    const lpTokenAccount = await anchor_spl.getAssociatedTokenAddress(
      lpMintPDA,
      vaultAuthorityPDA,
      true, // allowOwnerOffCurve
      anchor_spl.TOKEN_PROGRAM_ID
    );
    
    // Get LP token authority PDA for burning later
    const [lpTokenAuthorityPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("lp-token-authority")],
      program.programId
    );

 

    // For SOL, we need an ATA for the vault authority to receive swapped SOL
    const solTokenAccount = await anchor_spl.getAssociatedTokenAddress(
      solMint,
      vaultAuthorityPDA,
      true, // allowOwnerOffCurve
      anchor_spl.TOKEN_PROGRAM_ID
    );

    // Check if SOL token account exists, create if it doesn't
    try {
      const solAccountInfo = await connection.getAccountInfo(solTokenAccount);
      if (!solAccountInfo) {
        console.log('\x1b[33m%s\x1b[0m', '⚠️ SOL token account does not exist, creating it...');
        
        // Create the associated token account for SOL
        await anchor_spl.getOrCreateAssociatedTokenAccount(
          connection,
          walletKeypair,
          solMint,
          vaultAuthorityPDA,
          true, // allowOwnerOffCurve
          'confirmed',
          {},
          anchor_spl.TOKEN_PROGRAM_ID
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ SOL token account created');
      }
    } catch (error) {
      console.log('\x1b[33m%s\x1b[0m', '⚠️ Could not check/create SOL token account:', error.message);
    }

    // console.log('\x1b[90m%s\x1b[0m', `   Moon Doge Mining PDA: ${dogeBtcMiningPDA.toString()}`);
    // console.log('\x1b[90m%s\x1b[0m', `   Raydium Pool State: ${poolStatePDA.toString()}`);
    // console.log('\x1b[90m%s\x1b[0m', `   Vault Authority PDA: ${vaultAuthorityPDA.toString()}`);
    // console.log('\x1b[90m%s\x1b[0m', `   AMM Config: ${ammConfigPDA.toString()}`);
    // console.log('\x1b[90m%s\x1b[0m', `   DOGE_BTC Vault (Token Account): ${dbtcTokenAccount.toString()}`);
    // console.log('\x1b[90m%s\x1b[0m', `   SOL Token Account: ${solTokenAccount.toString()}`);


    console.log(`globalConfig: ${globalConfigPDA.toString()}`);
    console.log(`dogeBtcMining: ${dogeBtcMiningPDA.toString()}`);
    console.log(`raydiumProgram: ${raydiumProgramId.toString()}`);
    console.log(`poolState: ${poolStatePDA.toString()}`);
    console.log(`ammConfig: ${ammConfigPDA.toString()}`);
    console.log(`authorityPda: ${vaultAuthorityPDA.toString()}`);
    console.log(`raydiumAuthority: ${authorityPDA.toString()}`);
    console.log(`dbtcVault: ${dbtcVaultPDA.toString()}`);
    console.log(`solVault: ${solVaultPDA.toString()}`);
    console.log(`dbtcTokenAccount: ${dbtcTokenAccount.toString()}`);
    console.log(`solTokenAccount: ${solTokenAccount.toString()}`);
    console.log(`dbtcMint: ${dbtcMint.toString()}`);
    console.log(`solMint: ${solMint.toString()}`);
    console.log(`observationState: ${observationStatePDA.toString()}`);
    console.log(`solTreasury: ${solTreasuryPDA.toString()}`);
    console.log(`lpTokenAccount: ${lpTokenAccount.toString()}`);
    console.log(`lpMint: ${lpMintPDA.toString()}`);
    console.log(`tokenProgram2022: ${anchor_spl.TOKEN_2022_PROGRAM_ID.toString()}`);
    console.log(`tokenProgram: ${anchor_spl.TOKEN_PROGRAM_ID.toString()}`);
 


    const tx = await program.methods
      .updateDbtcDistPerSlot(new BN(0))
      .accounts({
        globalConfig: globalConfigPDA,
        dogeBtcMining: dogeBtcMiningPDA,
        raydiumProgram: raydiumProgramId,
        poolState: poolStatePDA,
        ammConfig: ammConfigPDA,
        authorityPda: vaultAuthorityPDA,
        raydiumAuthority: authorityPDA,
        dbtcVault: dbtcVaultPDA, // DOGE_BTC vault in Raydium pool
        solVault: solVaultPDA,     // SOL vault in Raydium pool
        dbtcTokenAccount: dbtcTokenAccount,
        solTokenAccount: solTokenAccount,
        dbtcMint: dbtcMint,
        solMint: solMint,
        observationState: observationStatePDA,
        solTreasury: solTreasuryPDA,
        lpTokenAccount: lpTokenAccount,
        lpMint: lpMintPDA,
        tokenProgram2022: anchor_spl.TOKEN_2022_PROGRAM_ID,
        tokenProgram: anchor_spl.TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log('\x1b[32m%s\x1b[0m', '✅ DOGE_BTC distribution per slot updated successfully!');
    console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${tx}`);

    return {
      success: true,
      data: {
        updateDistTxid: tx,
        dogeBtcMiningPDA: dogeBtcMiningPDA.toString(),
        vaultAuthorityPDA: vaultAuthorityPDA.toString(),
        solTreasuryPDA: solTreasuryPDA.toString(),
        poolStatePDA: poolStatePDA.toString(),
        ammConfigPDA: ammConfigPDA.toString()
      }
    };

  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update DOGE_BTC distribution per slot:', error);
    return {
      success: false,
      error: error.message
    };
  }
}

/**
 * Update module stats to activate the module (Step 2 of module creation)
 */
export async function updateModuleStatsHelper(
  connection,
  program,
  wallet,
  walletKeypair,
  globalConfigPDA,
  moduleId,
  moduleStats,
  moduleType
) {
  try {
    console.log('\x1b[33m%s\x1b[0m', '📡 Updating module stats...');
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module ID: ${moduleId}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Type: ${moduleType}`);

    // Derive module config account PDA
    const moduleIdBuffer = Buffer.allocUnsafe(2);
    moduleIdBuffer.writeUInt16LE(moduleId, 0);
    
    const [moduleConfigAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(MODULE_CONFIG_SEED), moduleIdBuffer],
      program.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 Module Config Account PDA: ${moduleConfigAccountPDA.toString()}`);

    // Extract stats based on module type
    let maxHp = 0;
    let powerConsumption = 0;
    let baseHashpower = 0;
    let baseXpPerHour = 0;

    const rawStats = moduleStats[moduleType.toLowerCase()] || moduleStats[moduleType] || moduleStats;

    switch (moduleType) {
      case 'Mining':
        maxHp = rawStats.max_hp ?? rawStats.maxHp;
        powerConsumption = rawStats.power_consumption ?? rawStats.powerConsumption;
        baseHashpower = rawStats.base_hashpower ?? rawStats.baseHashpower;
        break;
      case 'Attraction':
        maxHp = rawStats.max_hp ?? rawStats.maxHp;
        powerConsumption = rawStats.power_consumption ?? rawStats.powerConsumption;
        baseXpPerHour = rawStats.base_xp_per_hour ?? rawStats.baseXpPerHour;
        break;
      default:
        throw new Error(`Unsupported module type: ${moduleType}`);
    }

    console.log('\x1b[36m%s\x1b[0m', '📊 Stats to update:');
    console.log('\x1b[36m%s\x1b[0m', `   maxHp: ${maxHp}`);
    console.log('\x1b[36m%s\x1b[0m', `   powerConsumption: ${powerConsumption}`);
    console.log('\x1b[36m%s\x1b[0m', `   baseHashpower: ${baseHashpower}`);
    console.log('\x1b[36m%s\x1b[0m', `   baseXpPerHour: ${baseXpPerHour}`);

    console.log(`globalConfig: ${globalConfigPDA.toString()}`);
    console.log(`moduleConfigAccount: ${moduleConfigAccountPDA.toString()}`);
    console.log(`authority: ${wallet.publicKey.toString()}`);
    console.log(`systemProgram: ${web3.SystemProgram.programId.toString()}`);

    const updateStatsTx = await program.methods
      .updateModuleStats(
        moduleId,
        new BN(maxHp),
        new BN(powerConsumption),
        new BN(baseHashpower),
        new BN(baseXpPerHour),
      )
      .accounts({
        globalConfig: globalConfigPDA,
        moduleConfigAccount: moduleConfigAccountPDA,
        authority: wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .transaction();

    const updateStatsTxid = await web3.sendAndConfirmTransaction(connection, updateStatsTx, [walletKeypair]);
    
    console.log('\x1b[32m%s\x1b[0m', `✅ Module stats updated and activated`);
    console.log('\x1b[90m%s\x1b[0m', `🔗 Transaction ID: ${updateStatsTxid}`);
    console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${updateStatsTxid}?cluster=devnet`);

    return {
      success: true,
      data: {
        updateStatsTxid: updateStatsTxid,
        moduleConfigAccountPDA: moduleConfigAccountPDA.toString(),
        moduleId: moduleId
      }
    };
  } catch (error) {
    console.error('\x1b[31m%s\x1b[0m', '❌ Error updating module stats:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}
 