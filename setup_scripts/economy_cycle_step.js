#!/usr/bin/env node

/**
 * Economy Cycle Step Runner
 *
 * Each invocation performs ONE step of the economy cycle:
 *   - If price history < 8: fund treasury + distribute + snapshot
 *   - If price history == 8: update_rate
 *   - If lp_operation_pending: add_lp_and_burn
 *
 * Run repeatedly with ~5 min intervals to complete full cycles.
 * State is tracked via a JSON file so progress persists across invocations.
 */

import {
  Connection, PublicKey, Keypair, SystemProgram, Transaction,
  sendAndConfirmTransaction, LAMPORTS_PER_SOL, ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress, getAccount, createAssociatedTokenAccountInstruction,
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
const minebtcIdl = JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.minebtc_idl), "utf8"));
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.deployer_key), "utf8")))
);
const connection = new Connection(config.network.rpc_url, config.network.commitment);
const wallet = new Wallet(walletKeypair);
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
const mineBTCProgram = new Program(minebtcIdl, provider);
const pid = mineBTCProgram.programId;

// ============================================================
// PDAs
// ============================================================

const [globalConfigPDA] = PublicKey.findProgramAddressSync([Buffer.from("global-config")], pid);
const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync([Buffer.from("mine-btc-mining")], pid);
const [solTreasuryPDA] = PublicKey.findProgramAddressSync([Buffer.from("sol-treasury")], pid);
const [dogesTreasuryPDA] = PublicKey.findProgramAddressSync([Buffer.from("doges-treasury")], pid);
const [buybacksAccountPDA] = PublicKey.findProgramAddressSync([Buffer.from("buybacks")], pid);
const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync([Buffer.from("buybacks-sol-vault")], pid);
const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync([Buffer.from("minebtc-vault-authority")], pid);

const minebtcMint = new PublicKey(deployment.dbtc_mint_address);
const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");
const FEE_RECIPIENT = new PublicKey(config.deployment.FEE_RECIPIENT_MULTISIG);

// Raydium
const raydiumProgramId = new PublicKey(deployment.RAYDIUM_CP_PROGRAM_ID);
const raydiumPoolState = new PublicKey(deployment.dbtc_sol_pool_created.poolStatePDA);
const raydiumAmmConfig = new PublicKey(deployment.raydium_amm_config_created.amm_config_pda);
const raydiumAuthority = new PublicKey(deployment.dbtc_sol_pool_created.authorityPDA);
const raydiumObservationState = new PublicKey(deployment.dbtc_sol_pool_created.observationStatePDA);
const raydiumLpMint = new PublicKey(deployment.dbtc_sol_pool_created.lpMintPDA);
const solVaultPDA = new PublicKey(deployment.dbtc_sol_pool_created.token0VaultPDA);
const dbtcVaultPDA = new PublicKey(deployment.dbtc_sol_pool_created.token1VaultPDA);

// Derived accounts
const [dbtcTokenAccountPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("minebtc_vault"), mineBtcMiningPDA.toBuffer()], pid
);

// ============================================================
// State file for tracking cycle progress
// ============================================================

const STATE_FILE = path.join(__dirname, "economy_cycle_state.json");

function loadState() {
  try {
    return JSON.parse(fs.readFileSync(STATE_FILE, "utf8"));
  } catch {
    return { cycle: 1, completedCycles: 0, txHistory: [] };
  }
}

function saveState(state) {
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

// ============================================================
// Fetch on-chain state
// ============================================================

async function fetchState() {
  const mining = await mineBTCProgram.account.mineBtcMining.fetch(mineBtcMiningPDA);
  const buybacks = await mineBTCProgram.account.buybacksAccount.fetch(buybacksAccountPDA);
  const globalConfig = await mineBTCProgram.account.globalConfig.fetch(globalConfigPDA);
  const buybacksVaultBal = await connection.getBalance(buybacksSolVaultPDA);
  const solTreasuryBal = await connection.getBalance(solTreasuryPDA);
  const dogesTreasuryBal = await connection.getBalance(dogesTreasuryPDA);
  const walletBal = await connection.getBalance(walletKeypair.publicKey);

  return {
    priceHistoryLen: (mining.priceHistory || []).length,
    lpOperationPending: mining.lpOperationPending,
    lastRateUpdate: Number(mining.lastRateUpdate.toString()),
    recentPrice: mining.recentPrice.toString(),
    mineBtcPerRound: mining.mineBtcPerRound.toString(),
    trackPrice: mining.trackPrice.toString(),
    solForPol: buybacks.solForPol.toString(),
    totalSolAccumulated: buybacks.totalSolAccumulated.toString(),
    buybacksVaultBal,
    solTreasuryBal,
    dogesTreasuryBal,
    walletBal,
    buybackPct: globalConfig.solFeeConfig.buybackPct,
    priceHistory: (mining.priceHistory || []).map(e => ({
      ts: e.timestamp.toString(),
      price: e.price.toString(),
    })),
  };
}

function printState(label, state) {
  console.log(`\n  ${label}:`);
  console.log(`    Wallet: ${(state.walletBal / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`    Price history: ${state.priceHistoryLen}/8`);
  console.log(`    LP pending: ${state.lpOperationPending}`);
  console.log(`    Recent price: ${state.recentPrice}`);
  console.log(`    Track price: ${state.trackPrice}`);
  console.log(`    Mine BTC/round: ${state.mineBtcPerRound}`);
  console.log(`    SOL treasury: ${(state.solTreasuryBal / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`    Doges treasury: ${(state.dogesTreasuryBal / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`    Buybacks vault: ${(state.buybacksVaultBal / LAMPORTS_PER_SOL).toFixed(6)} SOL`);
  console.log(`    SOL for POL: ${state.solForPol} lamports`);
  console.log(`    Total accumulated: ${state.totalSolAccumulated} lamports`);
  if (state.priceHistory.length > 0) {
    console.log(`    Price history entries:`);
    state.priceHistory.forEach((e, i) => {
      const priceSOL = Number(e.price) / 1e9;
      console.log(`      [${i}] price=${e.price} (${priceSOL.toFixed(9)} SOL/MBTC)`);
    });
  }
}

// ============================================================
// Parse & decode events
// ============================================================

function parseAndLogEvents(txInfo) {
  if (!txInfo?.meta?.logMessages) return [];

  const events = [];
  const logs = txInfo.meta.logMessages;

  // Print relevant program logs
  const importantLogs = logs.filter(l =>
    l.includes("Program log:") && (
      l.includes("Transferred") || l.includes("Withdrew") || l.includes("Swap") ||
      l.includes("COMPLETE") || l.includes("Rate") || l.includes("LP") ||
      l.includes("Price") || l.includes("POL") || l.includes("Earnmark") ||
      l.includes("Available") || l.includes("Instruction:") || l.includes("burned") ||
      l.includes("minted") || l.includes("consumed") || l.includes("received") ||
      l.includes("MINE_BTC") || l.includes("snapshot") || l.includes("Conditions") ||
      l.includes("changed") || l.includes("CHANGED") || l.includes("unchanged")
    )
  );
  if (importantLogs.length > 0) {
    console.log(`\n  Key program logs:`);
    importantLogs.forEach(l => console.log(`    ${l.replace("Program log: ", "")}`));
  }

  // Decode anchor events
  for (const log of logs) {
    if (log.startsWith("Program data: ")) {
      try {
        const raw = log.replace("Program data: ", "");
        const decoded = mineBTCProgram.coder.events.decode(raw);
        if (decoded) {
          events.push(decoded);
          console.log(`\n  EVENT: ${decoded.name}`);
          const data = decoded.data;
          for (const [key, val] of Object.entries(data)) {
            const v = typeof val === "object" && val.toString ? val.toString() : val;
            console.log(`    ${key}: ${v}`);
          }
        }
      } catch { /* not an event */ }
    }
  }
  return events;
}

// ============================================================
// Step A: Fund treasury + distribute_sol_fees
// ============================================================

async function fundAndDistribute() {
  console.log("\n  --- Funding treasuries (0.01 SOL each) ---");

  const fundTx = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: walletKeypair.publicKey, toPubkey: solTreasuryPDA, lamports: 0.01 * LAMPORTS_PER_SOL }),
    SystemProgram.transfer({ fromPubkey: walletKeypair.publicKey, toPubkey: dogesTreasuryPDA, lamports: 0.01 * LAMPORTS_PER_SOL }),
  );
  const fundSig = await sendAndConfirmTransaction(connection, fundTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  Fund TX: ${fundSig}`);

  // Ensure multisig WSOL ATA exists
  const multisigWsolAta = await getAssociatedTokenAddress(WSOL_MINT, FEE_RECIPIENT, true, TOKEN_PROGRAM_ID);
  try {
    await getAccount(connection, multisigWsolAta);
  } catch {
    console.log(`  Creating multisig WSOL ATA...`);
    const createTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(walletKeypair.publicKey, multisigWsolAta, FEE_RECIPIENT, WSOL_MINT, TOKEN_PROGRAM_ID)
    );
    await sendAndConfirmTransaction(connection, createTx, [walletKeypair], { commitment: "confirmed" });
  }

  console.log("\n  --- Executing distribute_sol_fees ---");
  const preSolTreasury = await connection.getBalance(solTreasuryPDA);
  const preBuybacksVault = await connection.getBalance(buybacksSolVaultPDA);

  const distributeTx = await mineBTCProgram.methods.distributeSolFees().accounts({
    globalConfig: globalConfigPDA,
    solTreasury: solTreasuryPDA,
    treasuryWsolAccount: await getAssociatedTokenAddress(WSOL_MINT, solTreasuryPDA, true, TOKEN_PROGRAM_ID),
    multisigWsolAccount: multisigWsolAta,
    wsolMint: WSOL_MINT,
    dogesTreasury: dogesTreasuryPDA,
    buybacksSolVault: buybacksSolVaultPDA,
    buybacksAccount: buybacksAccountPDA,
    payer: walletKeypair.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  }).transaction();
  distributeTx.instructions.unshift(ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 }));

  const distSig = await sendAndConfirmTransaction(connection, distributeTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  Distribute TX: ${distSig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${distSig}?cluster=devnet`);

  const postSolTreasury = await connection.getBalance(solTreasuryPDA);
  const postBuybacksVault = await connection.getBalance(buybacksSolVaultPDA);
  console.log(`  SOL Treasury: ${preSolTreasury / LAMPORTS_PER_SOL} -> ${postSolTreasury / LAMPORTS_PER_SOL} SOL`);
  console.log(`  Buybacks Vault: ${preBuybacksVault / LAMPORTS_PER_SOL} -> ${postBuybacksVault / LAMPORTS_PER_SOL} SOL`);

  // Verify event
  const txInfo = await connection.getTransaction(distSig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const events = parseAndLogEvents(txInfo);

  // Verify business logic
  const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
  const available = preSolTreasury - rentExempt;
  const state = await fetchState();
  const expectedBuybacks = Math.floor(available * state.buybackPct / 100);
  console.log(`\n  Verification: available=${available}, expected_buybacks=${expectedBuybacks}, actual_vault_increase=${postBuybacksVault - preBuybacksVault}`);
  const match = Math.abs((postBuybacksVault - preBuybacksVault) - expectedBuybacks) < expectedBuybacks * 0.5; // doge treasury also contributes
  console.log(`  Buybacks routing: ${match ? "CORRECT" : "CHECK LOGS"}`);

  return { fundSig, distSig, events };
}

// ============================================================
// Step B: snapshot_price
// ============================================================

async function doSnapshot() {
  console.log("\n  --- Executing snapshot_price ---");

  const preState = await fetchState();
  const solTokenAccount = await getAssociatedTokenAddress(WSOL_MINT, vaultAuthorityPDA, true, TOKEN_PROGRAM_ID);

  const snapshotTx = await mineBTCProgram.methods.snapshotPrice().accounts({
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
    solTokenAccount,
    minebtcMint,
    solMint: WSOL_MINT,
    observationState: raydiumObservationState,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    tokenProgram: TOKEN_PROGRAM_ID,
    buybacksSolVault: buybacksSolVaultPDA,
    buybacksAccount: buybacksAccountPDA,
    systemProgram: SystemProgram.programId,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    authority: walletKeypair.publicKey,
  }).transaction();
  snapshotTx.instructions.unshift(ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 }));

  const sig = await sendAndConfirmTransaction(connection, snapshotTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  Snapshot TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  const postState = await fetchState();

  // Verify
  console.log(`\n  Verification:`);
  console.log(`    Price history: ${preState.priceHistoryLen} -> ${postState.priceHistoryLen} ${postState.priceHistoryLen === preState.priceHistoryLen + 1 ? "CORRECT" : "UNEXPECTED"}`);

  const polBefore = Number(preState.solForPol);
  const polAfter = Number(postState.solForPol);
  console.log(`    POL earnmarked: ${polBefore} -> ${polAfter} (+${polAfter - polBefore} lamports)`);

  const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
  const availablePreSwap = preState.buybacksVaultBal - rentExempt - polBefore;
  const expectedSwap = Math.floor(availablePreSwap / 10);
  const expectedPol = Math.floor(availablePreSwap / 10);
  console.log(`    Available for swap: ${availablePreSwap} lamports`);
  console.log(`    Expected swap (10%): ${expectedSwap}, Expected POL (10%): ${expectedPol}`);
  console.log(`    Actual POL increase: ${polAfter - polBefore}`);
  console.log(`    POL match: ${Math.abs((polAfter - polBefore) - expectedPol) < 100 ? "CORRECT" : "CHECK"}`);

  // Parse events
  const txInfo = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const events = parseAndLogEvents(txInfo);

  // Check PriceSnapshotTaken event
  const snapshotEvent = events.find(e => e.name === "priceSnapshotTaken");
  if (snapshotEvent) {
    const d = snapshotEvent.data;
    const solSwapped = Number(d.solSwapped.toString());
    const mbtcReceived = Number(d.minebtcReceived.toString());
    const price = Number(d.currentPrice.toString());
    const wAvg = Number(d.weightedAvgPrice.toString());
    console.log(`\n    Event verified:`);
    console.log(`      Snapshot #${d.snapshotNumber} of ${d.priceHistoryCount}`);
    console.log(`      SOL swapped: ${solSwapped} (${solSwapped / LAMPORTS_PER_SOL} SOL)`);
    console.log(`      MBTC received: ${mbtcReceived} (${mbtcReceived / 1e6} MBTC)`);
    console.log(`      Price: ${price} (${price / 1e9} SOL/MBTC)`);
    console.log(`      Weighted avg: ${wAvg} (${wAvg / 1e9} SOL/MBTC)`);
    console.log(`      Swap price match: ${Math.abs(solSwapped - expectedSwap) < 100 ? "CORRECT" : "CHECK"}`);
  }

  return { sig, events, postState };
}

// ============================================================
// Step C: update_rate
// ============================================================

async function doUpdateRate() {
  console.log("\n  --- Executing update_rate ---");

  const preState = await fetchState();

  const updateTx = await mineBTCProgram.methods.updateRate().accounts({
    mineBtcMining: mineBtcMiningPDA,
  }).transaction();
  updateTx.instructions.unshift(ComputeBudgetProgram.setComputeUnitLimit({ units: 200000 }));

  const sig = await sendAndConfirmTransaction(connection, updateTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  UpdateRate TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  const postState = await fetchState();

  console.log(`\n  Verification:`);
  console.log(`    Price history: ${preState.priceHistoryLen} -> ${postState.priceHistoryLen} (should be 0 after clear)`);
  console.log(`    LP pending: ${preState.lpOperationPending} -> ${postState.lpOperationPending} (should be true)`);
  console.log(`    Rate: ${preState.mineBtcPerRound} -> ${postState.mineBtcPerRound}`);
  console.log(`    Track price: ${preState.trackPrice} -> ${postState.trackPrice}`);
  console.log(`    Recent price: ${preState.recentPrice} -> ${postState.recentPrice}`);

  const txInfo = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const events = parseAndLogEvents(txInfo);

  const rateEvent = events.find(e => e.name === "distributionRateUpdated");
  if (rateEvent) {
    const d = rateEvent.data;
    console.log(`\n    DistributionRateUpdated event:`);
    console.log(`      Old rate: ${d.oldRate.toString()}`);
    console.log(`      New rate: ${d.newRate.toString()}`);
    console.log(`      Price change %: ${d.priceChangePct}`);
    console.log(`      Rate changed: ${d.rateChanged}`);
    console.log(`      Current price: ${d.currentPrice.toString()}`);
    console.log(`      Track price: ${d.trackPrice.toString()}`);
  }

  return { sig, events, postState };
}

// ============================================================
// Step D: add_lp_and_burn
// ============================================================

async function doAddLpAndBurn() {
  console.log("\n  --- Executing add_lp_and_burn ---");

  const preState = await fetchState();
  const solTokenAccount = await getAssociatedTokenAddress(WSOL_MINT, vaultAuthorityPDA, true, TOKEN_PROGRAM_ID);
  const lpTokenAccount = await getAssociatedTokenAddress(raydiumLpMint, vaultAuthorityPDA, true, TOKEN_PROGRAM_ID);

  // Check if WSOL ATA exists; if closed by previous cycle, need to re-init
  let wsolExists = false;
  try {
    await getAccount(connection, solTokenAccount);
    wsolExists = true;
  } catch { /* will be created by init_if_needed or we pass it */ }

  const addLpTx = await mineBTCProgram.methods.addLpAndBurn(new BN(0)).accounts({
    mineBtcMining: mineBtcMiningPDA,
    globalConfig: globalConfigPDA,
    authority: null,
    raydiumProgram: raydiumProgramId,
    poolState: raydiumPoolState,
    authorityPda: vaultAuthorityPDA,
    raydiumAuthority: raydiumAuthority,
    minebtcVault: dbtcVaultPDA,
    solVault: solVaultPDA,
    minebtcTokenAccount: dbtcTokenAccountPDA,
    solTokenAccount,
    minebtcMint,
    solMint: WSOL_MINT,
    lpTokenAccount,
    lpMint: raydiumLpMint,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    tokenProgram: TOKEN_PROGRAM_ID,
    buybacksSolVault: buybacksSolVaultPDA,
    buybacksAccount: buybacksAccountPDA,
    systemProgram: SystemProgram.programId,
  }).transaction();
  addLpTx.instructions.unshift(ComputeBudgetProgram.setComputeUnitLimit({ units: 600000 }));

  const sig = await sendAndConfirmTransaction(connection, addLpTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  AddLpAndBurn TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  const postState = await fetchState();

  console.log(`\n  Verification:`);
  console.log(`    LP pending: ${preState.lpOperationPending} -> ${postState.lpOperationPending} (should be false)`);
  console.log(`    SOL for POL: ${preState.solForPol} -> ${postState.solForPol} (should decrease by SOL consumed)`);

  const txInfo = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const events = parseAndLogEvents(txInfo);

  const lpAddedEvent = events.find(e => e.name === "liquidityAdded");
  if (lpAddedEvent) {
    const d = lpAddedEvent.data;
    console.log(`\n    LiquidityAdded event:`);
    console.log(`      SOL added: ${d.solAmount.toString()} (${Number(d.solAmount.toString()) / LAMPORTS_PER_SOL} SOL)`);
    console.log(`      MBTC added: ${d.minebtcAmount.toString()} (${Number(d.minebtcAmount.toString()) / 1e6} MBTC)`);
    console.log(`      LP minted: ${d.lpTokensMinted.toString()}`);
    console.log(`      LP price: ${d.lpTokenPrice.toString()} (${Number(d.lpTokenPrice.toString()) / 1e9} SOL/LP)`);
  }

  const burnEvent = events.find(e => e.name === "lpTokensBurned");
  if (burnEvent) {
    const d = burnEvent.data;
    console.log(`\n    LpTokensBurned event:`);
    console.log(`      LP burned: ${d.lpTokensBurned.toString()}`);
    console.log(`      Total LP burnt: ${d.totalLpBurnt.toString()}`);
    console.log(`      SOL added: ${d.solAmountAdded.toString()}`);
    console.log(`      MBTC added: ${d.minebtcAmountAdded.toString()}`);
  }

  return { sig, events, postState };
}

// ============================================================
// Main: decide which step to execute
// ============================================================

async function main() {
  const localState = loadState();
  const now = new Date().toISOString();

  console.log("╔══════════════════════════════════════════════════════════════════════╗");
  console.log(`║  ECONOMY CYCLE STEP - ${now}  ║`);
  console.log(`║  Cycle #${localState.cycle} | Completed: ${localState.completedCycles}                                        ║`);
  console.log("╚══════════════════════════════════════════════════════════════════════╝");

  if (localState.completedCycles >= 2) {
    console.log("\n  2 full cycles completed! Printing summary and exiting.");
    console.log("\n  === FULL TX HISTORY ===");
    localState.txHistory.forEach((entry, i) => {
      console.log(`    [${i + 1}] ${entry.action} | cycle=${entry.cycle} | sig=${entry.sig}`);
      console.log(`        https://explorer.solana.com/tx/${entry.sig}?cluster=devnet`);
    });
    console.log("\n  DONE. All cycles completed successfully.");
    process.exit(0);
  }

  const chainState = await fetchState();
  printState("Current on-chain state", chainState);

  // Check time constraint
  const currentUnix = Math.floor(Date.now() / 1000);
  const timeSinceLastUpdate = currentUnix - chainState.lastRateUpdate;
  console.log(`\n  Time since last update: ${timeSinceLastUpdate}s (need 300s)`);

  let result;
  let action;

  try {
    if (chainState.lpOperationPending) {
      // LP operation is pending - do add_lp_and_burn
      action = "add_lp_and_burn";
      console.log(`\n  ACTION: add_lp_and_burn (LP operation pending)`);
      result = await doAddLpAndBurn();

      // After add_lp_and_burn completes a cycle
      localState.completedCycles++;
      console.log(`\n  CYCLE ${localState.cycle} COMPLETE! (${localState.completedCycles}/2)`);
      localState.cycle++;

    } else if (chainState.priceHistoryLen >= 8) {
      // 8 snapshots collected - do update_rate
      action = "update_rate";
      console.log(`\n  ACTION: update_rate (8 snapshots collected)`);
      result = await doUpdateRate();

    } else if (timeSinceLastUpdate < 300) {
      // Too early for next snapshot
      const wait = 300 - timeSinceLastUpdate;
      action = "WAIT";
      console.log(`\n  ACTION: WAIT - ${wait}s until next snapshot allowed`);
      console.log(`  (snapshot interval is 300s, only ${timeSinceLastUpdate}s elapsed)`);
      localState.txHistory.push({ action: "WAIT", cycle: localState.cycle, sig: "n/a", ts: now, waitSeconds: wait });
      saveState(localState);
      process.exit(0);

    } else {
      // Fund + distribute + snapshot
      action = "fund_distribute_snapshot";
      console.log(`\n  ACTION: fund + distribute_sol_fees + snapshot_price (snapshot ${chainState.priceHistoryLen + 1}/8)`);

      // Fund and distribute
      const distResult = await fundAndDistribute();
      localState.txHistory.push({ action: "fund_treasuries", cycle: localState.cycle, sig: distResult.fundSig, ts: now });
      localState.txHistory.push({ action: "distribute_sol_fees", cycle: localState.cycle, sig: distResult.distSig, ts: now });

      // Snapshot
      result = await doSnapshot();
      action = "snapshot_price";
    }

    if (result) {
      localState.txHistory.push({
        action,
        cycle: localState.cycle <= 2 ? (action === "add_lp_and_burn" ? localState.cycle - 1 : localState.cycle) : localState.cycle,
        sig: result.sig,
        ts: now,
      });
    }

  } catch (error) {
    console.error(`\n  ERROR: ${error.message}`);
    if (error.logs) {
      console.error("\n  Transaction logs:");
      error.logs.forEach(l => console.error(`    ${l}`));
    }
    localState.txHistory.push({ action: action || "ERROR", cycle: localState.cycle, sig: "FAILED", ts: now, error: error.message });
  }

  saveState(localState);

  // Print next step
  const postState = await fetchState();
  printState("Post-step state", postState);

  if (localState.completedCycles >= 2) {
    console.log("\n  All 2 cycles now complete!");
  } else if (postState.lpOperationPending) {
    console.log("\n  NEXT: add_lp_and_burn (can run immediately)");
  } else if (postState.priceHistoryLen >= 8) {
    console.log("\n  NEXT: update_rate (can run immediately)");
  } else {
    console.log(`\n  NEXT: snapshot ${postState.priceHistoryLen + 1}/8 (wait ~5 min)`);
  }
}

main().catch(err => {
  console.error("FATAL:", err);
  process.exit(1);
});
