#!/usr/bin/env node

import { 
  Connection, 
  PublicKey, 
  Keypair, 
  SystemProgram, 
  Transaction, 
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram
} from '@solana/web3.js';
import { 
  TOKEN_PROGRAM_ID, 
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress 
} from '@solana/spl-token';
import anchorPkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load config
const configPath = path.join(__dirname, 'config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

// Load deployment info
const deploymentPath = path.join(__dirname, 'deployments', `${config.network.cluster}.json`);
const deployment = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));

// Load MoonBase IDL
const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
if (!fs.existsSync(moonbaseIdlPath)) {
  console.error(`❌ MoonBase IDL not found at: ${moonbaseIdlPath}`);
  process.exit(1);
}
const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));

// Load MoonEconomy IDL
const moonEconomyIdlPath = path.resolve(__dirname, config.deployment.paths.moon_economy_idl);
if (!fs.existsSync(moonEconomyIdlPath)) {
  console.error(`❌ MoonEconomy IDL not found at: ${moonEconomyIdlPath}`);
  process.exit(1);
}
const moonEconomyIdl = JSON.parse(fs.readFileSync(moonEconomyIdlPath, 'utf8'));

// Load wallet keypair
const walletPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
if (!fs.existsSync(walletPath)) {
  console.error(`❌ Wallet keypair not found at: ${walletPath}`);
  process.exit(1);
}
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);

// Create wallet object
const wallet = new Wallet(walletKeypair);

// Initialize programs (programId comes from IDL)
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
const moonBaseProgram = new Program(moonbaseIdl, provider);
const moonBaseProgramId = moonBaseProgram.programId;

const moonEconomyProgram = new Program(moonEconomyIdl, provider);
const moonEconomyProgramId = moonEconomyProgram.programId;

// Seeds
const GLOBAL_CONFIG_SEED = "global-config";
const DOGE_BTC_MINING_SEED = "moon-doge-mining";
const SOL_TREASURY_SEED = "sol-treasury";
const BUYBACKS_SEED = "buybacks";
const BUYBACKS_SOL_VAULT_SEED = "buybacks-sol-vault";
const DOGE_BTC_VAULT_SEED = "dbtc_vault";
const VAULT_AUTHORITY_SEED = "mdoge-vault-authority";

// Derive PDAs
const [globalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_CONFIG_SEED)],
  moonBaseProgramId
);

const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(DOGE_BTC_MINING_SEED)],
  moonBaseProgramId
);

const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(SOL_TREASURY_SEED)],
  moonBaseProgramId
);

const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(BUYBACKS_SEED)],
  moonBaseProgramId
);

const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(BUYBACKS_SOL_VAULT_SEED)],
  moonBaseProgramId
);

const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(VAULT_AUTHORITY_SEED)],
  moonBaseProgramId
);

// Token mints
const dbtcMint = new PublicKey(deployment.dbtc_mint_address);
const solMint = new PublicKey("So11111111111111111111111111111111111111112"); // WSOL

// Raydium addresses from deployment
const raydiumProgramId = new PublicKey(deployment.RAYDIUM_CP_PROGRAM_ID);
const raydiumPoolState = new PublicKey(deployment.dbtc_sol_pool_created.poolStatePDA);
const raydiumAmmConfig = new PublicKey(deployment.raydium_amm_config_created.amm_config_pda);
const raydiumAuthority = new PublicKey(deployment.dbtc_sol_pool_created.authorityPDA);
const raydiumObservationState = new PublicKey(deployment.dbtc_sol_pool_created.observationStatePDA);
const raydiumLpMint = new PublicKey(deployment.dbtc_sol_pool_created.lpMintPDA);
const raydiumToken0Vault = new PublicKey(deployment.dbtc_sol_pool_created.token0VaultPDA);
const raydiumToken1Vault = new PublicKey(deployment.dbtc_sol_pool_created.token1VaultPDA);

// Determine vault assignment (token0 = WSOL, token1 = DBTC)
const solVaultPDA = raydiumToken0Vault;
const dbtcVaultPDA = raydiumToken1Vault;

// MoonEconomy seeds
const MOON_ECONOMY_GLOBAL_CONFIG_SEED = "global_config";
const MOON_ECONOMY_DOGE_BTC_VAULT_SEED = "dogebtc_vault";
const MOON_ECONOMY_LIQUIDITY_VAULT_SEED = "liquidity_vault";
const MOON_ECONOMY_DBTC_SOL_VAULT_SEED = "dogewifbtc-sol-vault";
const MOON_ECONOMY_LP_SOL_VAULT_SEED = "lp-sol-vault";
const MOON_ECONOMY_DEV_EARNINGS_SEED = "dev_earnings_collector";
const MOON_ECONOMY_FEE_COLLECTOR_SEED = "fee_collector";

// Derive MoonEconomy PDAs
const [moonEconomyGlobalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_GLOBAL_CONFIG_SEED)],
  moonEconomyProgramId
);

const [moonEconomyDogebtcVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_DOGE_BTC_VAULT_SEED)],
  moonEconomyProgramId
);

const [moonEconomyLiquidityVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_LIQUIDITY_VAULT_SEED)],
  moonEconomyProgramId
);

const [moonEconomyDbtcSolVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_DBTC_SOL_VAULT_SEED)],
  moonEconomyProgramId
);

const [moonEconomyLiquiditySolVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_LP_SOL_VAULT_SEED)],
  moonEconomyProgramId
);

const [moonEconomyDevEarningsPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_DEV_EARNINGS_SEED)],
  moonEconomyProgramId
);

const [moonEconomyFeeCollectorPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MOON_ECONOMY_FEE_COLLECTOR_SEED)],
  moonEconomyProgramId
);

// MoonBase addresses from deployment
const moonbaseGlobalConfigPDA = new PublicKey(deployment.moonbase_program_initialized.globalConfig_address);
const moonbaseMiningStatePDA = new PublicKey(deployment.moonbase_program_initialized.dogeBtcMining_address);
const moonbaseTreasuryPDA = new PublicKey(deployment.moonbase_program_initialized.solTreasury_address);
const lootSolVaultPDA = new PublicKey(deployment.loot_rewards_initialized.sol_vault);
const lootRewardsPDA = new PublicKey(deployment.loot_rewards_initialized.loot_rewards_pda);

/**
 * Send 1 SOL to sol_treasury account
 */
async function sendSolToTreasury() {
  try {
    const amountLamports = 1 * LAMPORTS_PER_SOL;
    
    const transaction = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: walletKeypair.publicKey,
        toPubkey: solTreasuryPDA,
        lamports: amountLamports,
      })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Sent 1 SOL to treasury: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error sending SOL to treasury:`, error.message);
    return { success: false, error: error.message };
  }
}

/**
 * Get price history length from dogeBtcMining account
 */
async function getPriceHistoryLength() {
  try {
    const miningAccount = await moonBaseProgram.account.dogeBtcMining.fetch(dogeBtcMiningPDA);
    // console.log(miningAccount)
    const priceHistory = miningAccount.priceHistory || [];
    const length = priceHistory.length;
    
    console.log(`📊 Price history length: ${length}/8`);
    return length;
  } catch (error) {
    console.error(`❌ Error fetching price history:`, error.message);
    throw error;
  }
}

/**
 * Execute snapshot price transaction
 */
async function executeSnapshotPrice() {
  try {
    console.log('\n📸 Executing price snapshot...');
    
    // Get associated token accounts
    const solTokenAccount = await getAssociatedTokenAddress(
      solMint,
      vaultAuthorityPDA,
      true, // allowOwnerOffCurve
      TOKEN_PROGRAM_ID
    );

    // Derive dbtc_token_account PDA
    const [dbtcTokenAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(DOGE_BTC_VAULT_SEED), dogeBtcMiningPDA.toBuffer()],
      moonBaseProgramId
    );

    const snapshotTx = await moonBaseProgram.methods
      .snapshotPrice()
      .accounts({
        dogeBtcMining: dogeBtcMiningPDA,
        globalConfig: globalConfigPDA,
        raydiumProgram: raydiumProgramId,
        poolState: raydiumPoolState,
        ammConfig: raydiumAmmConfig,
        authorityPda: vaultAuthorityPDA,
        raydiumAuthority: raydiumAuthority,
        dbtcVault: dbtcVaultPDA,
        solVault: solVaultPDA,
        dbtcTokenAccount: dbtcTokenAccountPDA,
        solTokenAccount: solTokenAccount,
        dbtcMint: dbtcMint,
        solMint: solMint,
        observationState: raydiumObservationState,
        tokenProgram2022: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        buybacksSolVault: buybacksSolVaultPDA,
        buybacksAccount: buybacksAccountPDA,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        authority: walletKeypair.publicKey,
      })
      .transaction();

    // Add compute unit limit instruction at the beginning
    snapshotTx.instructions.unshift(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      snapshotTx,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Price snapshot executed: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error executing price snapshot:`, error.message);
    if (error.logs) {
      console.error('Transaction logs:', error.logs);
    }
    return { success: false, error: error.message };
  }
}

/**
 * Execute claim moonbase SOL transaction
 */
async function executeClaimMoonbaseSol() {
  try {
    console.log('\n💰 Executing claim moonbase SOL...');

    const claimMoonbaseSolTx = await moonEconomyProgram.methods
      .claimMoonbaseSol()
      .accounts({
        globalConfig: moonEconomyGlobalConfigPDA,
        dogebtcVault: moonEconomyDogebtcVaultPDA,
        liquidityVault: moonEconomyLiquidityVaultPDA,
        dbtcSolVault: moonEconomyDbtcSolVaultPDA,
        liquiditySolVault: moonEconomyLiquiditySolVaultPDA,
        devEarningsCollector: moonEconomyDevEarningsPDA,
        moonbaseGlobalConfig: moonbaseGlobalConfigPDA,
        moonbaseMiningState: moonbaseMiningStatePDA,
        moonbaseTreasury: moonbaseTreasuryPDA,
        feeCollector: moonEconomyFeeCollectorPDA,
        lootSolVault: lootSolVaultPDA,
        lootRewards: lootRewardsPDA,
        buybacksSolVault: buybacksSolVaultPDA,
        buybacksAccount: buybacksAccountPDA,
        moonFacilityProgram: moonBaseProgramId,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .transaction();

    // Add compute unit limit instruction at the beginning
    claimMoonbaseSolTx.instructions.unshift(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      claimMoonbaseSolTx,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Claim moonbase SOL executed: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error executing claim moonbase SOL:`, error.message);
    if (error.logs) {
      console.error('Transaction logs:', error.logs);
    }
    return { success: false, error: error.message };
  }
}

/**
 * Execute update rate and add LP transaction
 */
async function executeUpdateRateAndAddLp() {
  try {
    console.log('\n🔄 Executing update rate and add LP...');
    
    // Get associated token accounts
    const lpTokenAccount = await getAssociatedTokenAddress(
      raydiumLpMint,
      vaultAuthorityPDA,
      true, // allowOwnerOffCurve
      TOKEN_PROGRAM_ID
    );

    const solTokenAccount = await getAssociatedTokenAddress(
      solMint,
      vaultAuthorityPDA,
      true, // allowOwnerOffCurve
      TOKEN_PROGRAM_ID
    );

    // Derive dbtc_token_account PDA
    const [dbtcTokenAccountPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(DOGE_BTC_VAULT_SEED), dogeBtcMiningPDA.toBuffer()],
      moonBaseProgramId
    );

    const updateRateTx = await moonBaseProgram.methods
      .updateRateAndAddLp(new BN(0)) // 0 = automatic calculation mode
      .accounts({
        dogeBtcMining: dogeBtcMiningPDA,
        globalConfig: globalConfigPDA,
        raydiumProgram: raydiumProgramId,
        poolState: raydiumPoolState,
        authorityPda: vaultAuthorityPDA,
        raydiumAuthority: raydiumAuthority,
        dbtcVault: dbtcVaultPDA,
        solVault: solVaultPDA,
        dbtcTokenAccount: dbtcTokenAccountPDA,
        solTokenAccount: solTokenAccount,
        dbtcMint: dbtcMint,
        solMint: solMint,
        lpTokenAccount: lpTokenAccount,
        lpMint: raydiumLpMint,
        tokenProgram2022: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        buybacksSolVault: buybacksSolVaultPDA,
        buybacksAccount: buybacksAccountPDA,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        authority: walletKeypair.publicKey,
      })
      .transaction();

    // Add compute unit limit instruction at the beginning
    updateRateTx.instructions.unshift(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      updateRateTx,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Update rate and add LP executed: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error executing update rate and add LP:`, error.message);
    if (error.logs) {
      console.error('Transaction logs:', error.logs);
    }
    return { success: false, error: error.message };
  }
}

/**
 * Main loop function
 */
async function runLoop() {
  console.log('\n🚀 Starting price snapshot loop...');
  console.log(`📡 Network: ${config.network.cluster}`);
  console.log(`🔗 RPC: ${config.network.rpc_url}`);
  console.log(`👛 Wallet: ${walletKeypair.publicKey.toString()}`);
  console.log(`💰 SOL Treasury: ${solTreasuryPDA.toString()}`);
  console.log(`⏰ Interval: 7 seconds\n`);

  let iteration = 0;

  while (true) {
    iteration++;
    console.log(`\n${'='.repeat(60)}`);
    console.log(`🔄 Iteration #${iteration} - ${new Date().toISOString()}`);
    console.log(`${'='.repeat(60)}`);

    try {
      // Step 1: Send 1 SOL to treasury
      const sendResult = await sendSolToTreasury();
      if (!sendResult.success) {
        console.log('⚠️  Continuing despite SOL send failure...');
      }

      // Step 2: Claim moonbase SOL for distribution
      const claimResult = await executeClaimMoonbaseSol();
      if (!claimResult.success) {
        console.log('⚠️  Claim moonbase SOL failed, will retry next iteration');
      }

      // Step 3: Get price history length
      const priceHistoryLength = await getPriceHistoryLength();
    //   console.log(priceHistoryLength)

      // Step 4: Execute appropriate transaction based on price history
      if (priceHistoryLength < 8) {
        console.log(`📸 Price history < 8, executing snapshot...`);
        const snapshotResult = await executeSnapshotPrice();
        if (!snapshotResult.success) {
          console.log('⚠️  Snapshot failed, will retry next iteration');
        }
      } else if (priceHistoryLength === 8) {
        console.log(`🔄 Price history = 8, executing update rate and add LP...`);
        const updateResult = await executeUpdateRateAndAddLp();
        if (!updateResult.success) {
          console.log('⚠️  Update rate failed, will retry next iteration');
        }
      } else {
        console.log(`ℹ️  Price history > 8 (${priceHistoryLength}), waiting for next cycle...`);
      }

    } catch (error) {
      console.error(`❌ Error in loop iteration:`, error.message);
    }

    // Wait 7 seconds before next iteration
    console.log(`\n⏳ Waiting 7 seconds before next iteration...`);
    await new Promise(resolve => setTimeout(resolve, 7000));
  }
}

// Handle graceful shutdown
process.on('SIGINT', () => {
  console.log('\n\n🛑 Received SIGINT, shutting down gracefully...');
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\n\n🛑 Received SIGTERM, shutting down gracefully...');
  process.exit(0);
});

// Start the loop
runLoop().catch(error => {
  console.error('❌ Fatal error:', error);
  process.exit(1);
});

