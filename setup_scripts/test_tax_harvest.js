#!/usr/bin/env node

/**
 * Tax Harvest Test Script
 *
 * Flow:
 * 1. Buy DegenBTC from Raydium pool (SOL → DegenBTC swap)
 * 2. Transfer DegenBTC between wallets to generate withheld fees (0.1% tax)
 * 3. Query Helius to discover accounts with withheld fees
 * 4. Harvest withheld fees to mint
 * 5. Verify mint withheld_amount increased
 * 6. Run crankDistributeTax to split the tax
 */

import {
  Connection, PublicKey, Keypair, SystemProgram, Transaction,
  sendAndConfirmTransaction, LAMPORTS_PER_SOL, ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress, getAssociatedTokenAddressSync, getAccount,
  createAssociatedTokenAccountInstruction, createTransferCheckedInstruction,
  createHarvestWithheldTokensToMintInstruction,
  getMint, getTransferFeeAmount, unpackAccount,
} from "@solana/spl-token";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ── Setup ────────────────────────────────────────────────────────────────────
const config = JSON.parse(fs.readFileSync(path.join(__dirname, "config.json"), "utf8"));
const deployment = JSON.parse(fs.readFileSync(path.join(__dirname, "deployments", `${config.network.cluster}.json`), "utf8"));
const minebtcIdl = JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.minebtc_idl), "utf8"));
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.deployer_key), "utf8")))
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
const wallet = new Wallet(walletKeypair);
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
const program = new Program(minebtcIdl, provider);
const pid = program.programId;

const HELIUS_API_KEY = "00613837-c378-4a74-bc99-9dd891e24f89"; // from backend .env

// ── Addresses ────────────────────────────────────────────────────────────────
const dbtcMint = new PublicKey(deployment.dbtc_mint_address);
const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");

// Raydium
const raydiumProgramId = new PublicKey(deployment.RAYDIUM_CP_PROGRAM_ID);
const raydiumPoolState = new PublicKey(deployment.dbtc_sol_pool_created.poolStatePDA);
const raydiumAmmConfig = new PublicKey(deployment.raydium_amm_config_created.amm_config_pda);
const raydiumAuthority = new PublicKey(deployment.dbtc_sol_pool_created.authorityPDA);
const raydiumObservationState = new PublicKey(deployment.dbtc_sol_pool_created.observationStatePDA);
const raydiumToken0Vault = new PublicKey(deployment.dbtc_sol_pool_created.token0VaultPDA);
const raydiumToken1Vault = new PublicKey(deployment.dbtc_sol_pool_created.token1VaultPDA);

// Tax PDAs
const taxConfigPda = new PublicKey(deployment.tax_config_initialized.tax_config_pda);
const withdrawWithheldAuthority = new PublicKey(deployment.tax_config_initialized.withdraw_withheld_authority);
const factionTreasuryVault = new PublicKey(deployment.tax_config_initialized.faction_treasury_vault);
const nftFloorSweepVault = new PublicKey(deployment.tax_config_initialized.nft_floor_sweep_vault);

// token0 = WSOL, token1 = DBTC (from devnet.json: isMdogeToken0 = false)
const solVault = raydiumToken0Vault;
const dbtcVault = raydiumToken1Vault;

// Raydium IDL for swap
const raydiumIdlPath = path.resolve(__dirname, config.deployment.paths.raydium_idl);
const raydiumIdl = JSON.parse(fs.readFileSync(raydiumIdlPath, "utf8"));
const raydiumProgram = new Program(raydiumIdl, provider);

// ── Helpers ──────────────────────────────────────────────────────────────────

const fmtDbtc = (raw) => (Number(raw) / 1e6).toFixed(2);
const fmtSol = (lam) => (Number(lam) / LAMPORTS_PER_SOL).toFixed(6);

function separator(title) {
  console.log(`\n${"═".repeat(70)}`);
  console.log(`  ${title}`);
  console.log(`${"═".repeat(70)}`);
}

// ── Step 1: Buy DegenBTC from Raydium ─────────────────────────────────────────

async function buyDegenBtc(solAmount = 0.01) {
  separator("STEP 1: Buy DegenBTC from Raydium pool");

  const lamports = Math.floor(solAmount * LAMPORTS_PER_SOL);
  console.log(`  Swapping ${solAmount} SOL → DegenBTC`);

  // Get or create deployer's DBTC ATA (Token-2022)
  const deployerDbtcAta = getAssociatedTokenAddressSync(dbtcMint, walletKeypair.publicKey, false, TOKEN_2022_PROGRAM_ID);
  let dbtcBefore = BigInt(0);
  try {
    const acc = await getAccount(connection, deployerDbtcAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    dbtcBefore = acc.amount;
    console.log(`  Existing DBTC balance: ${fmtDbtc(dbtcBefore)} DBTC`);
  } catch {
    console.log(`  Creating deployer DBTC ATA...`);
    const createTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(walletKeypair.publicKey, deployerDbtcAta, walletKeypair.publicKey, dbtcMint, TOKEN_2022_PROGRAM_ID)
    );
    await sendAndConfirmTransaction(connection, createTx, [walletKeypair], { commitment: "confirmed" });
    console.log(`  Created: ${deployerDbtcAta.toBase58()}`);
  }

  // Get or create deployer's WSOL ATA
  const deployerWsolAta = getAssociatedTokenAddressSync(WSOL_MINT, walletKeypair.publicKey, false, TOKEN_PROGRAM_ID);
  try {
    await getAccount(connection, deployerWsolAta, "confirmed", TOKEN_PROGRAM_ID);
  } catch {
    const createTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(walletKeypair.publicKey, deployerWsolAta, walletKeypair.publicKey, WSOL_MINT, TOKEN_PROGRAM_ID)
    );
    await sendAndConfirmTransaction(connection, createTx, [walletKeypair], { commitment: "confirmed" });
  }

  // Wrap SOL → WSOL
  const wrapTx = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: walletKeypair.publicKey, toPubkey: deployerWsolAta, lamports }),
    // Sync native to update WSOL balance
    { keys: [{ pubkey: deployerWsolAta, isSigner: false, isWritable: true }],
      programId: TOKEN_PROGRAM_ID, data: Buffer.from([17]) } // SyncNative instruction
  );
  await sendAndConfirmTransaction(connection, wrapTx, [walletKeypair], { commitment: "confirmed" });

  // Swap SOL → DegenBTC via Raydium
  const swapTx = new Transaction();
  swapTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 }));

  const swapIx = await raydiumProgram.methods.swapBaseInput(
    new BN(lamports), new BN(0) // min_out = 0 for testing
  ).accounts({
    payer: walletKeypair.publicKey,
    authority: raydiumAuthority,
    ammConfig: raydiumAmmConfig,
    poolState: raydiumPoolState,
    inputTokenAccount: deployerWsolAta,
    outputTokenAccount: deployerDbtcAta,
    inputVault: solVault,
    outputVault: dbtcVault,
    inputTokenProgram: TOKEN_PROGRAM_ID,
    outputTokenProgram: TOKEN_2022_PROGRAM_ID,
    inputTokenMint: WSOL_MINT,
    outputTokenMint: dbtcMint,
    observationState: raydiumObservationState,
  }).instruction();
  swapTx.add(swapIx);

  const sig = await sendAndConfirmTransaction(connection, swapTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  Swap TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  // Check new balance
  const acc = await getAccount(connection, deployerDbtcAta, "confirmed", TOKEN_2022_PROGRAM_ID);
  const dbtcAfter = acc.amount;
  const dbtcReceived = dbtcAfter - dbtcBefore;
  console.log(`  DegenBTC received: ${fmtDbtc(dbtcReceived)} DBTC`);
  console.log(`  Total DBTC balance: ${fmtDbtc(dbtcAfter)} DBTC`);

  return { sig, dbtcReceived, dbtcBalance: dbtcAfter };
}

// ── Step 2: Transfer DegenBTC to generate withheld fees ───────────────────────

async function generateWithheldFees(amount) {
  separator("STEP 2: Transfer DegenBTC to generate withheld fees (0.1% tax)");

  // Create a secondary keypair to transfer to
  const recipient = Keypair.generate();
  console.log(`  Recipient (temp): ${recipient.publicKey.toBase58()}`);

  // Fund recipient for rent
  const fundTx = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: walletKeypair.publicKey, toPubkey: recipient.publicKey, lamports: 0.005 * LAMPORTS_PER_SOL })
  );
  await sendAndConfirmTransaction(connection, fundTx, [walletKeypair], { commitment: "confirmed" });

  // Create recipient DBTC ATA
  const recipientAta = getAssociatedTokenAddressSync(dbtcMint, recipient.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const createAtaTx = new Transaction().add(
    createAssociatedTokenAccountInstruction(walletKeypair.publicKey, recipientAta, recipient.publicKey, dbtcMint, TOKEN_2022_PROGRAM_ID)
  );
  await sendAndConfirmTransaction(connection, createAtaTx, [walletKeypair], { commitment: "confirmed" });

  // Transfer DegenBTC (triggers 0.1% tax withheld on sender side)
  const deployerAta = getAssociatedTokenAddressSync(dbtcMint, walletKeypair.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const transferAmount = amount || BigInt(100_000_000); // 100 DBTC default

  console.log(`  Transferring ${fmtDbtc(transferAmount)} DBTC...`);
  console.log(`  Expected tax withheld (0.1%): ~${fmtDbtc(transferAmount / BigInt(1000))} DBTC`);

  const transferIx = createTransferCheckedInstruction(
    deployerAta, dbtcMint, recipientAta,
    walletKeypair.publicKey, transferAmount, 6, // decimals
    [], TOKEN_2022_PROGRAM_ID
  );

  const transferTx = new Transaction().add(transferIx);
  const sig = await sendAndConfirmTransaction(connection, transferTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`  Transfer TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  // Check withheld amounts on both accounts
  const senderAcc = await getAccount(connection, deployerAta, "confirmed", TOKEN_2022_PROGRAM_ID);
  const recipientAcc = await getAccount(connection, recipientAta, "confirmed", TOKEN_2022_PROGRAM_ID);

  // Transfer the received DBTC back to generate more fees on the recipient account
  const recipientBalance = recipientAcc.amount;
  if (recipientBalance > BigInt(1_000_000)) { // > 1 DBTC
    console.log(`  Transferring back ${fmtDbtc(recipientBalance)} DBTC from recipient...`);
    const backTransferIx = createTransferCheckedInstruction(
      recipientAta, dbtcMint, deployerAta,
      recipient.publicKey, recipientBalance, 6,
      [], TOKEN_2022_PROGRAM_ID
    );
    const backTx = new Transaction().add(backTransferIx);
    const backSig = await sendAndConfirmTransaction(connection, backTx, [recipient], { commitment: "confirmed" });
    console.log(`  Back-transfer TX: ${backSig}`);
  }

  return { sig, recipientAta, recipientKeypair: recipient };
}

// ── Step 3: Query Helius for accounts with withheld fees ─────────────────────

async function queryWithheldFees() {
  separator("STEP 3: Query accounts with withheld fees (Helius DAS API)");

  const accountsWithFees = [];
  let cursor = undefined;
  let totalAccounts = 0;
  let totalWithheld = BigInt(0);

  do {
    const params = { mint: dbtcMint.toBase58(), limit: 1000 };
    if (cursor) params.cursor = cursor;

    const response = await fetch(
      `https://devnet.helius-rpc.com/?api-key=${HELIUS_API_KEY}`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0", id: "harvest-check", method: "getTokenAccounts", params,
        }),
      }
    );

    const data = await response.json();
    if (data.error) {
      console.error(`  Helius error:`, data.error);
      break;
    }

    const accounts = data.result?.token_accounts || [];
    totalAccounts += accounts.length;

    for (const acc of accounts) {
      const withheld = acc.token_extensions?.transfer_fee_amount?.withheld_amount;
      if (withheld && BigInt(withheld) > BigInt(0)) {
        accountsWithFees.push({
          address: acc.address,
          owner: acc.owner,
          amount: acc.amount,
          withheld: BigInt(withheld),
        });
        totalWithheld += BigInt(withheld);
      }
    }

    cursor = data.result?.cursor;
    if (data.result?.total && totalAccounts >= data.result.total) break;
  } while (cursor);

  console.log(`  Total token accounts scanned: ${totalAccounts}`);
  console.log(`  Accounts with withheld fees: ${accountsWithFees.length}`);
  console.log(`  Total withheld: ${fmtDbtc(totalWithheld)} DBTC`);

  // Sort by withheld desc
  accountsWithFees.sort((a, b) => (b.withheld > a.withheld ? 1 : -1));

  if (accountsWithFees.length > 0) {
    console.log(`\n  Top accounts:`);
    for (const acc of accountsWithFees.slice(0, 10)) {
      console.log(`    ${acc.address}: ${fmtDbtc(acc.withheld)} DBTC withheld (balance: ${fmtDbtc(acc.amount)} DBTC)`);
    }
  }

  return { accountsWithFees, totalWithheld };
}

// ── Step 4: Harvest withheld fees to mint ─────────────────────────────────────

async function harvestFees(accountsWithFees) {
  separator("STEP 4: Harvest withheld fees to mint");

  if (accountsWithFees.length === 0) {
    console.log("  No accounts to harvest.");
    return null;
  }

  // Check mint withheld_amount before
  const mintInfoBefore = await getMint(connection, dbtcMint, "confirmed", TOKEN_2022_PROGRAM_ID);
  const mintExtensions = mintInfoBefore.tlvData;
  console.log(`  Mint supply: ${fmtDbtc(mintInfoBefore.supply)} DBTC`);

  const addresses = accountsWithFees.map(a => new PublicKey(a.address));
  const BATCH_SIZE = 20;
  let harvestedBatches = 0;

  for (let i = 0; i < addresses.length; i += BATCH_SIZE) {
    const batch = addresses.slice(i, i + BATCH_SIZE);
    const batchNum = Math.floor(i / BATCH_SIZE) + 1;
    const totalBatches = Math.ceil(addresses.length / BATCH_SIZE);

    const tx = new Transaction().add(
      createHarvestWithheldTokensToMintInstruction(dbtcMint, batch, TOKEN_2022_PROGRAM_ID)
    );

    try {
      const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair], {
        commitment: "confirmed",
      });
      harvestedBatches++;
      console.log(`  Batch ${batchNum}/${totalBatches} harvested: ${sig}`);
      console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
    } catch (e) {
      console.error(`  Batch ${batchNum} failed: ${e.message}`);
    }
  }

  console.log(`  Harvested ${harvestedBatches} batches (${addresses.length} accounts)`);

  // Verify: re-query to confirm accounts are now 0
  const postHarvest = await queryWithheldFeesQuiet();
  console.log(`  Post-harvest: ${postHarvest.accountsWithFees.length} accounts still have fees (${fmtDbtc(postHarvest.totalWithheld)} DBTC)`);

  return { harvestedBatches };
}

// Quiet version of queryWithheldFees (no separator/logging)
async function queryWithheldFeesQuiet() {
  const accountsWithFees = [];
  let totalWithheld = BigInt(0);
  let cursor = undefined;

  do {
    const params = { mint: dbtcMint.toBase58(), limit: 1000 };
    if (cursor) params.cursor = cursor;

    const res = await fetch(`https://devnet.helius-rpc.com/?api-key=${HELIUS_API_KEY}`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: "q", method: "getTokenAccounts", params }),
    });
    const data = await res.json();
    if (data.error) break;

    for (const acc of (data.result?.token_accounts || [])) {
      const w = acc.token_extensions?.transfer_fee_amount?.withheld_amount;
      if (w && BigInt(w) > BigInt(0)) {
        accountsWithFees.push({ address: acc.address, withheld: BigInt(w), amount: acc.amount });
        totalWithheld += BigInt(w);
      }
    }
    cursor = data.result?.cursor;
  } while (cursor);

  return { accountsWithFees, totalWithheld };
}

// ── Step 5: Crank distribute tax ─────────────────────────────────────────────

async function crankDistributeTax() {
  separator("STEP 5: Crank distribute tax (split withheld from mint)");

  // Fetch tax config
  const taxConfig = await program.account.taxConfig.fetch(taxConfigPda);
  console.log(`  Tax Config:`);
  console.log(`    NFT Floor Sweep: ${taxConfig.nftFloorSweepPct}%`);
  console.log(`    Faction Treasury: ${taxConfig.factionTreasuryPct}%`);
  console.log(`    Burn: ${100 - taxConfig.nftFloorSweepPct - taxConfig.factionTreasuryPct}% (remainder)`);
  console.log(`    Total burnt so far: ${fmtDbtc(taxConfig.totalBurnt)} DBTC`);
  console.log(`    Round active: ${taxConfig.roundActive}`);

  // Need withdraw authority ATA
  const withdrawAuthAta = getAssociatedTokenAddressSync(dbtcMint, withdrawWithheldAuthority, true, TOKEN_2022_PROGRAM_ID);

  // Create if needed
  try {
    await getAccount(connection, withdrawAuthAta, "confirmed", TOKEN_2022_PROGRAM_ID);
  } catch {
    console.log(`  Creating withdraw authority ATA...`);
    const createTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(walletKeypair.publicKey, withdrawAuthAta, withdrawWithheldAuthority, dbtcMint, TOKEN_2022_PROGRAM_ID)
    );
    await sendAndConfirmTransaction(connection, createTx, [walletKeypair], { commitment: "confirmed" });
  }

  // Get the mining vault PDA for the "remainder" return
  const [mineBtcMiningPda] = PublicKey.findProgramAddressSync([Buffer.from("mine-btc-mining")], pid);
  const [vaultAuthorityPda] = PublicKey.findProgramAddressSync([Buffer.from("minebtc-vault-authority")], pid);
  const [minebtcTokenVault] = PublicKey.findProgramAddressSync(
    [Buffer.from("minebtc_vault"), mineBtcMiningPda.toBuffer()], pid
  );

  // Pre-balances
  const factionVaultBefore = await getAccount(connection, factionTreasuryVault, "confirmed", TOKEN_2022_PROGRAM_ID).catch(() => ({ amount: BigInt(0) }));
  const nftVaultBefore = await getAccount(connection, nftFloorSweepVault, "confirmed", TOKEN_2022_PROGRAM_ID).catch(() => ({ amount: BigInt(0) }));

  console.log(`\n  Pre-distribute balances:`);
  console.log(`    Faction Treasury Vault: ${fmtDbtc(factionVaultBefore.amount)} DBTC`);
  console.log(`    NFT Floor Sweep Vault: ${fmtDbtc(nftVaultBefore.amount)} DBTC`);

  // Execute crank
  const distributeTx = await program.methods.crankDistributeTax().accounts({
    withdrawWithheldAuthority: withdrawWithheldAuthority,
    minebtcMint: dbtcMint,
    withdrawAuthorityTokenAccount: withdrawAuthAta,
    nftFloorSweepVault: nftFloorSweepVault,
    factionTreasuryVault: factionTreasuryVault,
    minebtcTokenVault: minebtcTokenVault,
    taxConfig: taxConfigPda,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
  }).transaction();

  distributeTx.instructions.unshift(ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 }));

  const sig = await sendAndConfirmTransaction(connection, distributeTx, [walletKeypair], { commitment: "confirmed" });
  console.log(`\n  Distribute TX: ${sig}`);
  console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

  // Post-balances
  const factionVaultAfter = await getAccount(connection, factionTreasuryVault, "confirmed", TOKEN_2022_PROGRAM_ID).catch(() => ({ amount: BigInt(0) }));
  const nftVaultAfter = await getAccount(connection, nftFloorSweepVault, "confirmed", TOKEN_2022_PROGRAM_ID).catch(() => ({ amount: BigInt(0) }));
  const taxConfigAfter = await program.account.taxConfig.fetch(taxConfigPda);

  console.log(`\n  Post-distribute balances:`);
  console.log(`    Faction Treasury: ${fmtDbtc(factionVaultBefore.amount)} → ${fmtDbtc(factionVaultAfter.amount)} DBTC (+${fmtDbtc(factionVaultAfter.amount - factionVaultBefore.amount)})`);
  console.log(`    NFT Floor Sweep:  ${fmtDbtc(nftVaultBefore.amount)} → ${fmtDbtc(nftVaultAfter.amount)} DBTC (+${fmtDbtc(nftVaultAfter.amount - nftVaultBefore.amount)})`);
  console.log(`    Total burnt:      ${fmtDbtc(taxConfigAfter.totalBurnt)} DBTC`);

  // Parse events
  const txInfo = await connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  if (txInfo?.meta?.logMessages) {
    for (const log of txInfo.meta.logMessages) {
      if (log.includes("Program log:") && (log.includes("Tax") || log.includes("burn") || log.includes("sweep") || log.includes("faction") || log.includes("Distribute"))) {
        console.log(`    ${log.replace("Program log: ", "")}`);
      }
    }
  }

  return { sig };
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  separator("TAX HARVEST TEST");
  console.log(`  Network: ${config.network.cluster}`);
  console.log(`  Wallet: ${walletKeypair.publicKey.toBase58()}`);
  console.log(`  DegenBTC Mint: ${dbtcMint.toBase58()}`);
  console.log(`  Tax Config: ${taxConfigPda.toBase58()}`);

  const walletBalance = await connection.getBalance(walletKeypair.publicKey);
  console.log(`  Wallet balance: ${fmtSol(walletBalance)} SOL`);

  try {
    // Step 1: Buy DegenBTC
    const buyResult = await buyDegenBtc(0.01);

    // Step 2: Transfer to generate fees
    if (buyResult.dbtcReceived > BigInt(0)) {
      // Use half of what we bought for transfers
      const transferAmount = buyResult.dbtcReceived / BigInt(2);
      await generateWithheldFees(transferAmount);
    }

    // Step 3: Query withheld fees
    const { accountsWithFees, totalWithheld } = await queryWithheldFees();

    // Step 4: Harvest
    if (accountsWithFees.length > 0) {
      await harvestFees(accountsWithFees);
    }

    // Step 5: Distribute tax
    await crankDistributeTax();

    separator("TEST COMPLETE");
    console.log(`  All steps executed successfully.`);

  } catch (error) {
    console.error(`\n  FATAL ERROR: ${error.message}`);
    if (error.logs) {
      console.error("\n  Transaction logs:");
      error.logs.forEach(l => console.error(`    ${l}`));
    }
    console.error(error.stack);
    process.exit(1);
  }
}

main();
