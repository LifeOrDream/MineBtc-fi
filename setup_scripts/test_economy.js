#!/usr/bin/env node

/**
 * Economy.rs Transaction Tester
 *
 * Tests:
 * 1. Send SOL to sol_treasury PDA
 * 2. Execute distribute_sol_fees instruction
 * 3. Execute snapshot_price instruction
 * 4. Verify events and business logic for both
 */

import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  getAccount,
} from "@solana/spl-token";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ============================================================
// Setup
// ============================================================

const configPath = path.join(__dirname, "config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));

const deploymentPath = path.join(__dirname, "deployments", `${config.network.cluster}.json`);
const deployment = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));

const minebtcIdlPath = path.resolve(__dirname, config.deployment.paths.minebtc_idl);
const minebtcIdl = JSON.parse(fs.readFileSync(minebtcIdlPath, "utf8"));

const walletPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf8")))
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
const wallet = new Wallet(walletKeypair);
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
const mineBTCProgram = new Program(minebtcIdl, provider);
const mineBTCProgramId = mineBTCProgram.programId;

// ============================================================
// Seeds & PDAs
// ============================================================

const GLOBAL_CONFIG_SEED = "global-config";
const MINE_BTC_MINING_SEED = "mine-btc-mining";
const SOL_TREASURY_SEED = "sol-treasury";
const BUYBACKS_SEED = "buybacks";
const BUYBACKS_SOL_VAULT_SEED = "buybacks-sol-vault";
const DOGE_BTC_VAULT_SEED = "minebtc_vault";
const VAULT_AUTHORITY_SEED = "minebtc-vault-authority";

const [globalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_CONFIG_SEED)], mineBTCProgramId
);
const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MINE_BTC_MINING_SEED)], mineBTCProgramId
);
const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(SOL_TREASURY_SEED)], mineBTCProgramId
);
const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(BUYBACKS_SEED)], mineBTCProgramId
);
const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(BUYBACKS_SOL_VAULT_SEED)], mineBTCProgramId
);
const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(VAULT_AUTHORITY_SEED)], mineBTCProgramId
);

// Token mints
const minebtcMint = new PublicKey(deployment.dbtc_mint_address);
const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");

// Raydium addresses
const raydiumProgramId = new PublicKey(deployment.RAYDIUM_CP_PROGRAM_ID);
const raydiumPoolState = new PublicKey(deployment.dbtc_sol_pool_created.poolStatePDA);
const raydiumAmmConfig = new PublicKey(deployment.raydium_amm_config_created.amm_config_pda);
const raydiumAuthority = new PublicKey(deployment.dbtc_sol_pool_created.authorityPDA);
const raydiumObservationState = new PublicKey(deployment.dbtc_sol_pool_created.observationStatePDA);
const raydiumToken0Vault = new PublicKey(deployment.dbtc_sol_pool_created.token0VaultPDA);
const raydiumToken1Vault = new PublicKey(deployment.dbtc_sol_pool_created.token1VaultPDA);

// token0 = WSOL, token1 = DBTC
const solVaultPDA = raydiumToken0Vault;
const dbtcVaultPDA = raydiumToken1Vault;

const FEE_RECIPIENT_MULTISIG = new PublicKey(config.deployment.FEE_RECIPIENT_MULTISIG);

// ============================================================
// Helper: Parse events from transaction logs
// ============================================================

function parseEventsFromLogs(logs) {
  const events = [];
  for (const log of logs) {
    if (log.startsWith("Program data: ")) {
      try {
        const data = Buffer.from(log.replace("Program data: ", ""), "base64");
        events.push(data);
      } catch (e) {
        // skip non-parseable
      }
    }
  }
  return events;
}

function decodeAnchorEvent(program, eventData) {
  try {
    // Anchor event discriminator is first 8 bytes
    const discriminator = eventData.slice(0, 8);

    // Try to decode each event type
    for (const eventDef of program.idl.events || []) {
      const eventName = eventDef.name;
      try {
        const decoded = program.coder.events.decode(eventData.toString("base64"));
        if (decoded) {
          return decoded;
        }
      } catch (e) {
        // not this event
      }
    }
  } catch (e) {
    // fallback
  }
  return null;
}

// ============================================================
// Step 1: Send SOL to treasuries
// ============================================================

async function sendSolToTreasuries() {
  console.log("\n" + "=".repeat(70));
  console.log("STEP 1: Send SOL to Treasury PDA");
  console.log("=".repeat(70));

  const solAmount = 0.01 * LAMPORTS_PER_SOL; // 0.01 SOL = 10_000_000 lamports

  // Check pre-balance
  const preSolTreasuryBal = await connection.getBalance(solTreasuryPDA);
  console.log(`\n  Pre-balance:`);
  console.log(`    SOL Treasury (${solTreasuryPDA.toBase58()}): ${preSolTreasuryBal / LAMPORTS_PER_SOL} SOL`);

  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: walletKeypair.publicKey,
      toPubkey: solTreasuryPDA,
      lamports: solAmount,
    })
  );

  const signature = await sendAndConfirmTransaction(connection, tx, [walletKeypair], {
    commitment: "confirmed",
  });

  console.log(`\n  TX Signature: ${signature}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);

  // Check post-balance
  const postSolTreasuryBal = await connection.getBalance(solTreasuryPDA);
  console.log(`\n  Post-balance:`);
  console.log(`    SOL Treasury: ${postSolTreasuryBal / LAMPORTS_PER_SOL} SOL (+${(postSolTreasuryBal - preSolTreasuryBal) / LAMPORTS_PER_SOL} SOL)`);

  return { signature, postSolTreasuryBal };
}

// ============================================================
// Step 2: Execute distribute_sol_fees
// ============================================================

async function executeDistributeSolFees() {
  console.log("\n" + "=".repeat(70));
  console.log("STEP 2: Execute distribute_sol_fees");
  console.log("=".repeat(70));

  // Pre-state: fetch account balances
  const preSolTreasuryBal = await connection.getBalance(solTreasuryPDA);
  const preBuybacksVaultBal = await connection.getBalance(buybacksSolVaultPDA);

  console.log(`\n  Pre-balances:`);
  console.log(`    SOL Treasury: ${preSolTreasuryBal / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Buybacks SOL Vault: ${preBuybacksVaultBal / LAMPORTS_PER_SOL} SOL`);

  // Fetch global config to get fee percentages
  const globalConfig = await mineBTCProgram.account.globalConfig.fetch(globalConfigPDA);
  const buybackPct = globalConfig.solFeeConfig.buybackPct;
  console.log(`\n  Fee Config:`);
  console.log(`    Buyback %: ${buybackPct}`);
  console.log(`    Fee Recipient (Multisig): ${globalConfig.feeRecipient.toBase58()}`);

  // Fetch buybacks account pre-state
  const preBuybacksAccount = await mineBTCProgram.account.buybacksAccount.fetch(buybacksAccountPDA);
  console.log(`    Pre Buybacks total_sol_accumulated: ${preBuybacksAccount.totalSolAccumulated.toString()}`);

  // Need multisig WSOL ATA
  const multisigWsolAta = await getAssociatedTokenAddress(
    WSOL_MINT,
    FEE_RECIPIENT_MULTISIG,
    true, // allowOwnerOffCurve for multisig
    TOKEN_PROGRAM_ID
  );
  console.log(`    Multisig WSOL ATA: ${multisigWsolAta.toBase58()}`);

  // Check if multisig WSOL ATA exists
  let multisigWsolExists = false;
  try {
    await getAccount(connection, multisigWsolAta);
    multisigWsolExists = true;
    console.log(`    Multisig WSOL ATA exists: true`);
  } catch {
    console.log(`    Multisig WSOL ATA exists: false - will need to create it first`);
  }

  // If multisig WSOL ATA doesn't exist, create it
  if (!multisigWsolExists) {
    console.log(`\n  Creating multisig WSOL ATA...`);
    const { createAssociatedTokenAccountInstruction } = await import("@solana/spl-token");
    const createAtaTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        walletKeypair.publicKey,
        multisigWsolAta,
        FEE_RECIPIENT_MULTISIG,
        WSOL_MINT,
        TOKEN_PROGRAM_ID,
      )
    );
    const createSig = await sendAndConfirmTransaction(connection, createAtaTx, [walletKeypair], {
      commitment: "confirmed",
    });
    console.log(`  Created multisig WSOL ATA: ${createSig}`);
  }

  // Build distribute_sol_fees transaction
  const distributeTx = await mineBTCProgram.methods
    .distributeSolFees()
    .accounts({
      globalConfig: globalConfigPDA,
      solTreasury: solTreasuryPDA,
      treasuryWsolAccount: await getAssociatedTokenAddress(WSOL_MINT, solTreasuryPDA, true, TOKEN_PROGRAM_ID),
      multisigWsolAccount: multisigWsolAta,
      wsolMint: WSOL_MINT,
      buybacksSolVault: buybacksSolVaultPDA,
      buybacksAccount: buybacksAccountPDA,
      payer: walletKeypair.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .transaction();

  // Add compute budget
  distributeTx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 })
  );

  const signature = await sendAndConfirmTransaction(connection, distributeTx, [walletKeypair], {
    commitment: "confirmed",
  });

  console.log(`\n  TX Signature: ${signature}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);

  // Post-state
  const postSolTreasuryBal = await connection.getBalance(solTreasuryPDA);
  const postBuybacksVaultBal = await connection.getBalance(buybacksSolVaultPDA);

  console.log(`\n  Post-balances:`);
  console.log(`    SOL Treasury: ${postSolTreasuryBal / LAMPORTS_PER_SOL} SOL (delta: ${(postSolTreasuryBal - preSolTreasuryBal) / LAMPORTS_PER_SOL} SOL)`);
  console.log(`    Buybacks SOL Vault: ${postBuybacksVaultBal / LAMPORTS_PER_SOL} SOL (delta: ${(postBuybacksVaultBal - preBuybacksVaultBal) / LAMPORTS_PER_SOL} SOL)`);

  // Fetch buybacks account post-state
  const postBuybacksAccount = await mineBTCProgram.account.buybacksAccount.fetch(buybacksAccountPDA);
  console.log(`    Post Buybacks total_sol_accumulated: ${postBuybacksAccount.totalSolAccumulated.toString()}`);

  // Parse transaction logs for events
  console.log(`\n  Fetching transaction logs...`);
  const txInfo = await connection.getTransaction(signature, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });

  if (txInfo && txInfo.meta && txInfo.meta.logMessages) {
    console.log(`\n  Transaction Logs:`);
    for (const log of txInfo.meta.logMessages) {
      if (log.includes("Program log:") || log.includes("Program data:")) {
        console.log(`    ${log}`);
      }
    }

    // Parse events
    const eventDataList = parseEventsFromLogs(txInfo.meta.logMessages);
    console.log(`\n  Events found: ${eventDataList.length}`);
    for (const eventData of eventDataList) {
      const decoded = decodeAnchorEvent(mineBTCProgram, eventData);
      if (decoded) {
        console.log(`    Event: ${decoded.name}`);
        console.log(`    Data:`, JSON.stringify(decoded.data, (key, value) =>
          typeof value === "bigint" ? value.toString() : value, 2));
      }
    }
  }

  // Business logic verification
  console.log(`\n  === Business Logic Verification ===`);

  // The rent-exempt minimum for a system account (0 data) is ~890880 lamports
  const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
  const availableSol = preSolTreasuryBal - rentExempt;
  const expectedBuybacks = Math.floor(availableSol * buybackPct / 100);
  const expectedDevEarnings = availableSol - expectedBuybacks;

  console.log(`    Available SOL from treasury: ${availableSol / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Expected buybacks (${buybackPct}%): ${expectedBuybacks / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Expected dev earnings: ${expectedDevEarnings / LAMPORTS_PER_SOL} SOL`);

  const actualBuybacksIncrease = postBuybacksVaultBal - preBuybacksVaultBal;
  console.log(`    Expected buybacks vault increase: ${expectedBuybacks / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Actual buybacks vault increase: ${actualBuybacksIncrease / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Match: ${Math.abs(expectedBuybacks - actualBuybacksIncrease) < 1000 ? "YES" : "NO (check logs)"}`);

  return { signature, txInfo };
}

// ============================================================
// Step 3: Execute snapshot_price
// ============================================================

async function executeSnapshotPrice() {
  console.log("\n" + "=".repeat(70));
  console.log("STEP 3: Execute snapshot_price");
  console.log("=".repeat(70));

  // Pre-state
  const miningAccount = await mineBTCProgram.account.mineBtcMining.fetch(mineBtcMiningPDA);
  const priceHistoryLen = (miningAccount.priceHistory || []).length;
  const preBuybacksVaultBal = await connection.getBalance(buybacksSolVaultPDA);
  const preBuybacksAccount = await mineBTCProgram.account.buybacksAccount.fetch(buybacksAccountPDA);

  console.log(`\n  Pre-state:`);
  console.log(`    Price history length: ${priceHistoryLen}/8`);
  console.log(`    Last rate update: ${miningAccount.lastRateUpdate.toString()}`);
  console.log(`    LP operation pending: ${miningAccount.lpOperationPending}`);
  console.log(`    Recent price: ${miningAccount.recentPrice.toString()}`);
  console.log(`    Mine BTC per round: ${miningAccount.mineBtcPerRound.toString()}`);
  console.log(`    Buybacks vault balance: ${preBuybacksVaultBal / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Buybacks sol_for_pol: ${preBuybacksAccount.solForPol.toString()}`);
  console.log(`    Snapshot interval: ${miningAccount.snapshotInterval || 'unknown'} seconds`);

  if (miningAccount.lpOperationPending) {
    console.log(`\n  WARNING: LP operation is pending. snapshot_price will fail.`);
    console.log(`  Need to call update_rate + add_lp_and_burn first.`);
    return null;
  }

  if (priceHistoryLen >= 8) {
    console.log(`\n  WARNING: Price history is full (8/8). Need to call update_rate first.`);
    return null;
  }

  // Check if buybacks vault has SOL to swap
  const buybacksRent = await connection.getMinimumBalanceForRentExemption(0);
  const availableSolForSwap = preBuybacksVaultBal - buybacksRent - Number(preBuybacksAccount.solForPol.toString());
  console.log(`    Available SOL for swap: ${availableSolForSwap / LAMPORTS_PER_SOL} SOL`);

  if (availableSolForSwap <= 0) {
    console.log(`\n  WARNING: No SOL available in buybacks vault for swap.`);
    console.log(`  The snapshot will succeed but record 0 price.`);
  }

  // Get associated token accounts
  const solTokenAccount = await getAssociatedTokenAddress(
    WSOL_MINT, vaultAuthorityPDA, true, TOKEN_PROGRAM_ID
  );

  const [dbtcTokenAccountPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(DOGE_BTC_VAULT_SEED), mineBtcMiningPDA.toBuffer()],
    mineBTCProgramId
  );

  console.log(`\n  Accounts:`);
  console.log(`    WSOL Token Account (vault auth ATA): ${solTokenAccount.toBase58()}`);
  console.log(`    DBTC Token Account (PDA): ${dbtcTokenAccountPDA.toBase58()}`);

  // Build snapshot_price transaction
  const snapshotTx = await mineBTCProgram.methods
    .snapshotPrice()
    .accounts({
      mineBtcMining: mineBtcMiningPDA,
      globalConfig: globalConfigPDA,
      raydiumProgram: raydiumProgramId,
      poolState: raydiumPoolState,
      ammConfig: raydiumAmmConfig,
      authorityPda: vaultAuthorityPDA,
      raydiumAuthority: raydiumAuthority,
      minebtcVault: dbtcVaultPDA,
      solVault: solVaultPDA,
      minebtcTokenAccount: dbtcTokenAccountPDA,
      solTokenAccount: solTokenAccount,
      minebtcMint: minebtcMint,
      solMint: WSOL_MINT,
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

  snapshotTx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
  );

  const signature = await sendAndConfirmTransaction(connection, snapshotTx, [walletKeypair], {
    commitment: "confirmed",
  });

  console.log(`\n  TX Signature: ${signature}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);

  // Post-state
  const postMiningAccount = await mineBTCProgram.account.mineBtcMining.fetch(mineBtcMiningPDA);
  const postBuybacksVaultBal = await connection.getBalance(buybacksSolVaultPDA);
  const postBuybacksAccount = await mineBTCProgram.account.buybacksAccount.fetch(buybacksAccountPDA);

  console.log(`\n  Post-state:`);
  console.log(`    Price history length: ${(postMiningAccount.priceHistory || []).length}/8`);
  console.log(`    Recent price: ${postMiningAccount.recentPrice.toString()}`);
  console.log(`    Last rate update: ${postMiningAccount.lastRateUpdate.toString()}`);
  console.log(`    Buybacks vault balance: ${postBuybacksVaultBal / LAMPORTS_PER_SOL} SOL`);
  console.log(`    Buybacks sol_for_pol: ${postBuybacksAccount.solForPol.toString()}`);

  if (postMiningAccount.priceHistory && postMiningAccount.priceHistory.length > 0) {
    const lastEntry = postMiningAccount.priceHistory[postMiningAccount.priceHistory.length - 1];
    console.log(`\n    Latest price entry:`);
    console.log(`      Timestamp: ${lastEntry.timestamp.toString()}`);
    console.log(`      Price (raw): ${lastEntry.price.toString()}`);
    console.log(`      Price (SOL per MINEBTC): ${Number(lastEntry.price.toString()) / 1e9}`);
  }

  // Parse transaction logs for events
  console.log(`\n  Fetching transaction logs...`);
  const txInfo = await connection.getTransaction(signature, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });

  if (txInfo && txInfo.meta && txInfo.meta.logMessages) {
    console.log(`\n  Transaction Logs (filtered):`);
    for (const log of txInfo.meta.logMessages) {
      if (log.includes("Program log:") || log.includes("Program data:")) {
        console.log(`    ${log}`);
      }
    }

    // Parse events
    const eventDataList = parseEventsFromLogs(txInfo.meta.logMessages);
    console.log(`\n  Events found: ${eventDataList.length}`);
    for (const eventData of eventDataList) {
      const decoded = decodeAnchorEvent(mineBTCProgram, eventData);
      if (decoded) {
        console.log(`    Event: ${decoded.name}`);
        console.log(`    Data:`, JSON.stringify(decoded.data, (key, value) =>
          typeof value === "bigint" ? value.toString() : value, 2));
      }
    }
  }

  // Business logic verification
  console.log(`\n  === Business Logic Verification ===`);
  console.log(`    Price history increased: ${priceHistoryLen} -> ${(postMiningAccount.priceHistory || []).length} ${(postMiningAccount.priceHistory || []).length === priceHistoryLen + 1 ? "CORRECT" : "UNEXPECTED"}`);

  const polIncrease = Number(postBuybacksAccount.solForPol.toString()) - Number(preBuybacksAccount.solForPol.toString());
  console.log(`    POL earnmarked increase: ${polIncrease} lamports (${polIncrease / LAMPORTS_PER_SOL} SOL)`);

  if (availableSolForSwap > 0) {
    const expectedSwapAmt = Math.floor(availableSolForSwap / 10);
    const expectedPolAmt = Math.floor(availableSolForSwap / 10);
    console.log(`    Expected swap amount (10%): ${expectedSwapAmt / LAMPORTS_PER_SOL} SOL`);
    console.log(`    Expected POL earnmark (10%): ${expectedPolAmt / LAMPORTS_PER_SOL} SOL`);
    console.log(`    Actual POL increase matches expected: ${Math.abs(polIncrease - expectedPolAmt) < 1000 ? "YES" : "NO (check logs)"}`);
  }

  return { signature, txInfo };
}

// ============================================================
// Main
// ============================================================

async function main() {
  console.log("=".repeat(70));
  console.log("MineBTC Economy.rs Transaction Tester");
  console.log("=".repeat(70));
  console.log(`Network: ${config.network.cluster}`);
  console.log(`RPC: ${config.network.rpc_url}`);
  console.log(`Wallet: ${walletKeypair.publicKey.toBase58()}`);
  console.log(`Program ID: ${mineBTCProgramId.toBase58()}`);
  console.log(`SOL Treasury PDA: ${solTreasuryPDA.toBase58()}`);
  console.log(`Buybacks SOL Vault PDA: ${buybacksSolVaultPDA.toBase58()}`);

  // Check wallet balance
  const walletBalance = await connection.getBalance(walletKeypair.publicKey);
  console.log(`\nWallet balance: ${walletBalance / LAMPORTS_PER_SOL} SOL`);

  if (walletBalance < 0.05 * LAMPORTS_PER_SOL) {
    console.error("ERROR: Wallet needs at least 0.05 SOL for tests");
    process.exit(1);
  }

  try {
    // Step 1: Send SOL to treasuries
    const step1Result = await sendSolToTreasuries();
    console.log(`\n  Step 1 COMPLETE`);

    // Step 2: Distribute SOL fees
    const step2Result = await executeDistributeSolFees();
    console.log(`\n  Step 2 COMPLETE`);

    // Step 3: Snapshot price
    const step3Result = await executeSnapshotPrice();
    if (step3Result) {
      console.log(`\n  Step 3 COMPLETE`);
    } else {
      console.log(`\n  Step 3 SKIPPED (preconditions not met)`);
    }

    // Summary
    console.log("\n" + "=".repeat(70));
    console.log("SUMMARY");
    console.log("=".repeat(70));
    console.log(`  Step 1 (Send SOL to treasuries): ${step1Result.signature}`);
    console.log(`    https://explorer.solana.com/tx/${step1Result.signature}?cluster=devnet`);
    console.log(`  Step 2 (distribute_sol_fees): ${step2Result.signature}`);
    console.log(`    https://explorer.solana.com/tx/${step2Result.signature}?cluster=devnet`);
    if (step3Result) {
      console.log(`  Step 3 (snapshot_price): ${step3Result.signature}`);
      console.log(`    https://explorer.solana.com/tx/${step3Result.signature}?cluster=devnet`);
    }

  } catch (error) {
    console.error(`\nFATAL ERROR: ${error.message}`);
    if (error.logs) {
      console.error("\nTransaction logs:");
      for (const log of error.logs) {
        console.error(`  ${log}`);
      }
    }
    console.error(error.stack);
    process.exit(1);
  }
}

main();
