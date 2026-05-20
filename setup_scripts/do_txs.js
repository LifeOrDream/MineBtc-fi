#!/usr/bin/env node

/**
 * do_txs.js — manual cranker / operations script
 *
 * Replaces the scattered set of test_*, economy_cycle_step, game_loop, and
 * price_snapshot scripts. Each cranker is a top-level async function. Edit
 * `main()` at the bottom: uncomment the steps you want to run, save, run.
 *
 *   node do_txs.js
 *
 * Functions defined here:
 *   ── status / inspection ──
 *     printState()
 *     printGameState()
 *
 *   ── economy cranker ──
 *     sendSolToTreasury(solAmount)
 *     distributeSolFees()
 *     snapshotPrice()
 *     updateRate()
 *     addLpAndBurn()
 *     crankHarvestFees()        // pulls withheld fees from holder ATAs into mint
 *     crankDistributeTax()      // splits mint-withheld into faction/burn/recycle
 *
 *   ── game cranker ──
 *     initializeFactionWar()    // ONE-OFF per 4h cycle, before first start_round
 *     startRound(roundId)
 *     endRound()                // reveals entropy + picks winner
 *     settleRound()             // pays stakers, folds round into faction-war cycle
 *     settleFactionWar()        // permissionless after LP-burn cycle boundary
 *
 *   ── NFT marketplace cranker ──
 *     recordFloorSnapshot()
 *
 * ───────────────────────────────────────────────────────────────────
 *  LIFECYCLE — read this before editing `main()`
 * ───────────────────────────────────────────────────────────────────
 *
 *  ONE-TIME (run by 1_/2_/3_init_* scripts, not here): initialize, add_faction,
 *  initialize_war_config, initialize_mining, etc.
 *
 *  PER 4H CYCLE (war_id):
 *    1. initializeFactionWar()              // creates faction-war PDA for current war_id
 *    2. snapshotPrice()  x8                 // every ~30 min during the cycle
 *    3. updateRate()                        // after 8th snapshot
 *    4. addLpAndBurn()                      // burns LP — when lp_op_count crosses
 *                                           //   `settle_at_lp_op_count`, the contract
 *                                           //   emits CycleEndRoundSnapshotted and
 *                                           //   locks cycle_end_round_id.
 *
 *  PER ROUND (every ~2 min within the active cycle):
 *    a. startRound()                        // requires war_state for war_id
 *    b. (users place bets — join_bets)
 *    c. endRound()                          // entropy reveal, picks winner
 *    d. settleRound()                       // pays stakers + faction-war mining
 *
 *  END OF CYCLE (once cycle_end_round_id has been settled):
 *    5. settleFactionWar()                  // permissionless, finalizes ranks
 *    6. crankDistributeTax(war_id)          // splits accumulated tax to factions
 *
 *  → THEN GO BACK TO STEP 1 with the NEXT war_id.
 *
 *  Periodic (any time):
 *    distributeSolFees()                    // drain sol_treasury to buyback + dev
 *    crankHarvestFees()                     // pull withheld fees from holder ATAs
 *    recordFloorSnapshot()                  // daily, for NFT floor anchor
 */

import {
  Connection, PublicKey, Keypair, SystemProgram, Transaction,
  SYSVAR_SLOT_HASHES_PUBKEY, sendAndConfirmTransaction,
  LAMPORTS_PER_SOL, ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress, getAccount,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { resolveRaydiumProgramId, setIdlAddress } from "./raydium_id_sync.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ════════════════════════════════════════════════════════════════════
//  SETUP
// ════════════════════════════════════════════════════════════════════

const config = JSON.parse(
  fs.readFileSync(path.join(__dirname, "config.json"), "utf8"),
);
const cluster = config.network.cluster;
const deployment = JSON.parse(
  fs.readFileSync(
    path.join(__dirname, "deployments", `${cluster}.json`),
    "utf8",
  ),
);
const minebtcIdl = setIdlAddress(
  JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, config.deployment.paths.minebtc_idl),
    "utf8",
    ),
  ),
  deployment.MINE_BTC_PROGRAM_ID,
);
const raydiumProgramId = new PublicKey(
  resolveRaydiumProgramId(config, deployment, {
    requireCustomDeployment: true,
  }),
);
const raydiumIdl = setIdlAddress(
  JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, config.deployment.paths.raydium_idl),
    "utf8",
    ),
  ),
  raydiumProgramId,
);
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(
    JSON.parse(
      fs.readFileSync(
        path.resolve(__dirname, config.deployment.paths.deployer_key),
        "utf8",
      ),
    ),
  ),
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
const wallet = new Wallet(walletKeypair);
const provider = new AnchorProvider(connection, wallet, {
  commitment: config.network.commitment,
});
const program = new Program(minebtcIdl, provider);
const raydiumProgram = new Program(raydiumIdl, provider);
const pid = program.programId;

// Helius for the harvest-discovery step (Token-2022 withheld scan).
const HELIUS_RPC =
  cluster === "devnet"
    ? "https://devnet.helius-rpc.com"
    : "https://mainnet.helius-rpc.com";
const HELIUS_API_KEY =
  process.env.HELIUS_API_KEY || "00613837-c378-4a74-bc99-9dd891e24f89";

// ════════════════════════════════════════════════════════════════════
//  ADDRESSES & PDAS
// ════════════════════════════════════════════════════════════════════

const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");
const dbtcMint = new PublicKey(deployment.dbtc_mint_address);
// FEE_RECIPIENT_MULTISIG is keyed by cluster (devnet/mainnet); fall back to a
// bare string for backward compat if the config hasn't been migrated.
const feeRecipientRaw = config.deployment.FEE_RECIPIENT_MULTISIG;
const FEE_RECIPIENT = new PublicKey(
  typeof feeRecipientRaw === "string"
    ? feeRecipientRaw
    : feeRecipientRaw[cluster],
);

// MineBTC PDAs
const [globalConfigPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("global-config")], pid);
const [globalGameStatePda] = PublicKey.findProgramAddressSync(
  [Buffer.from("global-game-state")], pid);
const [mineBtcMiningPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("mine-btc-mining")], pid);
const [solTreasuryPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("sol-treasury")], pid);
const [buybacksAccountPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("buybacks")], pid);
const [buybacksSolVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("buybacks-sol-vault")], pid);
const [vaultAuthorityPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("degenBTC-vault-authority")], pid);
const [factionWarConfigPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("faction-war-config")], pid);
const [solRewardsVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("staker-sol-reward-vault")], pid);
const [solPrizePotVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("jackpot-pot")], pid);
const [taxConfigPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("tax-config")], pid);
const [withdrawWithheldAuthorityPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("withdraw-withheld-authority")], pid);
const [inventoryPoolPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("inventory-pool")], pid);
const [floorQueuePda] = PublicKey.findProgramAddressSync(
  [Buffer.from("floor-queue")], pid);
const [saleHistoryPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("sale-history")], pid);
const [floorHistoryPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("floor-history")], pid);
const [inventorySweepVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("inventory-sweep-vault")], pid);
const marketplaceConfigAddress =
  deployment.inventory_pool_initialized?.marketplace_config
  ?? deployment.degenbtc_marketplace_initialized?.marketplace_config_pda;
const marketplaceConfigPda = marketplaceConfigAddress
  ? new PublicKey(marketplaceConfigAddress)
  : null;
const marketplaceProgramAddress =
  deployment.inventory_pool_initialized?.marketplace_program
  ?? deployment.degenbtc_marketplace_initialized?.program_id;
const marketplaceProgramId = marketplaceProgramAddress
  ? new PublicKey(marketplaceProgramAddress)
  : null;
const factionWarSolVaultPda = deployment.war_sol_vault_pda
  ? new PublicKey(deployment.war_sol_vault_pda)
  : PublicKey.findProgramAddressSync([Buffer.from("faction-war-sol-vault")], pid)[0];
// `dbtc_vault` is the dBTC token vault owned by the mining authority.
const [dbtcTokenVaultPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("dbtc_vault"), mineBtcMiningPda.toBuffer()], pid);
const [dbtcVaultAuthorityPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("degenBTC-vault-authority")], pid);

// Tax-config side accounts (initialized at deploy time)
const factionTreasuryVault = new PublicKey(
  deployment.tax_config_initialized.faction_treasury_vault,
);

// Raydium pool addresses — dBTC/SOL CP-swap pool
const raydiumPoolState = new PublicKey(deployment.dbtc_sol_pool_created.poolStatePDA);
const raydiumAmmConfig = new PublicKey(deployment.raydium_amm_config_created.amm_config_pda);
const raydiumAuthority = new PublicKey(deployment.dbtc_sol_pool_created.authorityPDA);
const raydiumObservationState = new PublicKey(deployment.dbtc_sol_pool_created.observationStatePDA);
const raydiumLpMint = new PublicKey(deployment.dbtc_sol_pool_created.lpMintPDA);
const poolToken0Vault = new PublicKey(deployment.dbtc_sol_pool_created.token0VaultPDA);
const poolToken1Vault = new PublicKey(deployment.dbtc_sol_pool_created.token1VaultPDA);
const solVaultPda = deployment.dbtc_sol_pool_created.isDbtcToken0 ? poolToken1Vault : poolToken0Vault;
const dbtcVaultPda = deployment.dbtc_sol_pool_created.isDbtcToken0 ? poolToken0Vault : poolToken1Vault;

// ════════════════════════════════════════════════════════════════════
//  HELPERS
// ════════════════════════════════════════════════════════════════════

function u64Buffer(n) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n), 0);
  return b;
}
const lam = (n) => (Number(n) / LAMPORTS_PER_SOL).toFixed(6);
const dbtc = (n) => (Number(n) / 1e6).toFixed(2);
const banner = (s) => {
  const line = "═".repeat(72);
  console.log(`\n${line}\n  ${s}\n${line}`);
};
const step = (s) => console.log(`\n──▶ ${s}`);
const ok = (s) => console.log(`✅ ${s}`);
const warn = (s) => console.log(`⚠️  ${s}`);
const explorer = (sig) =>
  `https://explorer.solana.com/tx/${sig}?cluster=${cluster}`;

function deriveGameSessionPda(roundId) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("game-session"), u64Buffer(roundId)], pid)[0];
}
function deriveFactionWarStatePda(factionWarId) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("faction-war"), u64Buffer(factionWarId)], pid)[0];
}
function deriveFactionWarSettlementPda(factionWarId) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("faction-war-settlement"), u64Buffer(factionWarId)], pid)[0];
}
function deriveFactionStatePda(factionId) {
  const name = config.factions[factionId]?.name;
  if (!name) throw new Error(`Unknown faction id ${factionId}`);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("faction"), Buffer.from(name)], pid)[0];
}

function logEvents(txInfo) {
  if (!txInfo?.meta?.logMessages) return [];
  const events = [];
  for (const log of txInfo.meta.logMessages) {
    if (!log.startsWith("Program data: ")) continue;
    try {
      const decoded = program.coder.events.decode(
        log.replace("Program data: ", ""),
      );
      if (decoded) {
        events.push(decoded);
        console.log(`   ▸ event ${decoded.name}`);
        for (const [k, v] of Object.entries(decoded.data)) {
          const s = v?.toString?.() ?? v;
          console.log(`       ${k}: ${s}`);
        }
      }
    } catch { /* not an event */ }
  }
  return events;
}

async function ensureAta(owner, mint, tokenProgram = TOKEN_PROGRAM_ID, allowOffCurve = false) {
  const ata = await getAssociatedTokenAddress(mint, owner, allowOffCurve, tokenProgram);
  try {
    await getAccount(connection, ata, "confirmed", tokenProgram);
  } catch {
    const tx = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        walletKeypair.publicKey, ata, owner, mint, tokenProgram,
      ),
    );
    await sendAndConfirmTransaction(connection, tx, [walletKeypair],
      { commitment: "confirmed" });
    console.log(`   created ATA ${ata.toBase58()} for ${owner.toBase58()}`);
  }
  return ata;
}

async function send(tx, computeUnits = 400_000) {
  tx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnits }),
  );
  return sendAndConfirmTransaction(connection, tx, [walletKeypair],
    { commitment: "confirmed" });
}

async function fetchFactionWarId() {
  const cfg = await program.account.factionWarConfig.fetch(factionWarConfigPda);
  return cfg.currentWarId.toNumber();
}

// ════════════════════════════════════════════════════════════════════
//  STATUS / INSPECTION
// ════════════════════════════════════════════════════════════════════

async function printState() {
  banner("ECONOMY STATE");
  const mining = await program.account.degenBtcMining.fetch(mineBtcMiningPda);
  const buybacks = await program.account.buybacksAccount.fetch(buybacksAccountPda);
  const gc = await program.account.globalConfig.fetch(globalConfigPda);
  const fw = await program.account.factionWarConfig.fetch(factionWarConfigPda);
  const treasuryBal = await connection.getBalance(solTreasuryPda);
  const buybacksBal = await connection.getBalance(buybacksSolVaultPda);
  const sweepBal = await connection.getBalance(inventorySweepVaultPda);
  const walletBal = await connection.getBalance(walletKeypair.publicKey);

  console.log(`  cluster                : ${cluster}`);
  console.log(`  wallet                 : ${walletKeypair.publicKey.toBase58()}`);
  console.log(`  wallet bal             : ${lam(walletBal)} SOL`);
  console.log(`  --- mining ---`);
  console.log(`  price history          : ${(mining.priceHistory||[]).length}/8`);
  console.log(`  lp_operation_pending   : ${mining.lpOperationPending}`);
  console.log(`  recent_price           : ${mining.recentPrice.toString()}`);
  console.log(`  track_price            : ${mining.trackPrice.toString()}`);
  console.log(`  dbtc_per_round     : ${mining.dbtcPerRound.toString()}`);
  console.log(`  last_rate_update       : ${mining.lastRateUpdate.toString()} (${new Date(Number(mining.lastRateUpdate) * 1000).toISOString()})`);
  console.log(`  --- treasuries ---`);
  console.log(`  sol_treasury           : ${lam(treasuryBal)} SOL`);
  console.log(`  buybacks_sol_vault     : ${lam(buybacksBal)} SOL`);
  console.log(`  inventory_sweep_vault  : ${lam(sweepBal)} SOL`);
  console.log(`  buybacks.solForPol     : ${buybacks.solForPol.toString()} lamports`);
  console.log(`  --- sol_fee_config ---`);
  console.log(`  protocol_fee/buyback/stakers/cycle/nftMM = ${gc.solFeeConfig.protocolFeePct}/${gc.solFeeConfig.buybackPct}/${gc.solFeeConfig.stakersPct}/${gc.solFeeConfig.cycleSolSplitPct}/${gc.solFeeConfig.nftMarketMakingPct}%`);
  console.log(`  --- faction war ---`);
  console.log(`  current_war_id : ${fw.currentWarId.toString()}`);
}

async function printGameState() {
  banner("GAME STATE");
  const gs = await program.account.globalGameSate.fetch(globalGameStatePda);
  const cur = gs.currentRoundId?.toNumber() ?? 0;
  console.log(`  is_active              : ${gs.isActive}`);
  console.log(`  can_begin_round        : ${gs.canBeginRound}`);
  console.log(`  current_round_id       : ${cur}`);
  console.log(`  last_round_id          : ${gs.lastRoundId?.toNumber() ?? 0}`);
  console.log(`  round_duration_seconds : ${gs.roundDurationSeconds?.toNumber() ?? 0}`);
  console.log(`  winning_faction_id     : ${gs.winningFactionId ?? "—"}`);

  if (cur > 0) {
    try {
      const session = await program.account.gameSession.fetch(deriveGameSessionPda(cur));
      const endTs = Number(session.roundEndTimestamp);
      const now = Math.floor(Date.now() / 1000);
      console.log(`  round_end_timestamp    : ${endTs} (${new Date(endTs * 1000).toISOString()})`);
      console.log(`  time_remaining         : ${Math.max(0, endTs - now)}s`);
      console.log(`  stage                  : ${session.stage}`);
    } catch (e) {
      warn(`couldn't fetch current game session: ${e.message}`);
    }
  }
}

// ════════════════════════════════════════════════════════════════════
//  ECONOMY CRANKER
// ════════════════════════════════════════════════════════════════════

async function sendSolToTreasury(solAmount = 0.1) {
  banner(`SEND ${solAmount} SOL → sol_treasury`);
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: walletKeypair.publicKey,
      toPubkey: solTreasuryPda,
      lamports: Math.floor(solAmount * LAMPORTS_PER_SOL),
    }),
  );
  const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair],
    { commitment: "confirmed" });
  ok(`funded: ${sig}`);
  console.log(`   ${explorer(sig)}`);
  return sig;
}

async function distributeSolFees() {
  banner("DISTRIBUTE SOL FEES");
  const treasuryWsolAta = await getAssociatedTokenAddress(
    WSOL_MINT, solTreasuryPda, true, TOKEN_PROGRAM_ID);
  const multisigWsolAta = await ensureAta(FEE_RECIPIENT, WSOL_MINT);

  const preTreasury = await connection.getBalance(solTreasuryPda);
  const preBuybacks = await connection.getBalance(buybacksSolVaultPda);
  const preSweep    = await connection.getBalance(inventorySweepVaultPda);

  const tx = await program.methods.distributeSolFees().accounts({
    globalConfig: globalConfigPda,
    solTreasury: solTreasuryPda,
    treasuryWsolAccount: treasuryWsolAta,
    multisigWsolAccount: multisigWsolAta,
    wsolMint: WSOL_MINT,
    buybacksSolVault: buybacksSolVaultPda,
    inventorySweepVault: inventorySweepVaultPda,
    buybacksAccount: buybacksAccountPda,
    payer: walletKeypair.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 400_000);
  ok(`distribute_sol_fees: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const postTreasury = await connection.getBalance(solTreasuryPda);
  const postBuybacks = await connection.getBalance(buybacksSolVaultPda);
  const postSweep    = await connection.getBalance(inventorySweepVaultPda);
  console.log(`   sol_treasury  : ${lam(preTreasury)} → ${lam(postTreasury)}`);
  console.log(`   buybacks_vault: ${lam(preBuybacks)} → ${lam(postBuybacks)}  (+${lam(postBuybacks - preBuybacks)})`);
  console.log(`   sweep_vault   : ${lam(preSweep)} → ${lam(postSweep)}  (+${lam(postSweep - preSweep)})`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

async function snapshotPrice() {
  banner("SNAPSHOT PRICE");
  const solTokenAccount = await getAssociatedTokenAddress(
    WSOL_MINT, vaultAuthorityPda, true, TOKEN_PROGRAM_ID);

  const tx = await program.methods.snapshotPrice().accounts({
    dbtcMining: mineBtcMiningPda,
    globalConfig: globalConfigPda,
    raydiumProgram: raydiumProgramId,
    poolState: raydiumPoolState,
    ammConfig: raydiumAmmConfig,
    authorityPda: vaultAuthorityPda,
    raydiumAuthority,
    dbtcVault: dbtcVaultPda,
    solVault: solVaultPda,
    dbtcTokenAccount: dbtcTokenVaultPda,
    solTokenAccount,
    degenbtcMint: dbtcMint,
    solMint: WSOL_MINT,
    observationState: raydiumObservationState,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    tokenProgram: TOKEN_PROGRAM_ID,
    buybacksSolVault: buybacksSolVaultPda,
    buybacksAccount: buybacksAccountPda,
    systemProgram: SystemProgram.programId,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    authority: walletKeypair.publicKey,
  }).transaction();

  const sig = await send(tx, 500_000);
  ok(`snapshot_price: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

async function updateRate() {
  banner("UPDATE RATE");
  const tx = await program.methods.updateRate().accounts({
    dbtcMining: mineBtcMiningPda,
    warConfig: factionWarConfigPda,
  }).transaction();

  const sig = await send(tx, 200_000);
  ok(`update_rate: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

async function addLpAndBurn() {
  banner("ADD LP + BURN");
  const solTokenAccount = await getAssociatedTokenAddress(
    WSOL_MINT, vaultAuthorityPda, true, TOKEN_PROGRAM_ID);
  const lpTokenAccount = await getAssociatedTokenAddress(
    raydiumLpMint, vaultAuthorityPda, true, TOKEN_PROGRAM_ID);

  const tx = await program.methods.addLpAndBurn(new BN(0)).accounts({
    dbtcMining: mineBtcMiningPda,
    globalConfig: globalConfigPda,
    globalGameState: globalGameStatePda,
    warConfig: factionWarConfigPda,
    authority: walletKeypair.publicKey,
    raydiumProgram: raydiumProgramId,
    poolState: raydiumPoolState,
    authorityPda: vaultAuthorityPda,
    raydiumAuthority,
    dbtcVault: dbtcVaultPda,
    solVault: solVaultPda,
    dbtcTokenAccount: dbtcTokenVaultPda,
    solTokenAccount,
    degenbtcMint: dbtcMint,
    solMint: WSOL_MINT,
    lpTokenAccount,
    lpMint: raydiumLpMint,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    tokenProgram: TOKEN_PROGRAM_ID,
    buybacksSolVault: buybacksSolVaultPda,
    buybacksAccount: buybacksAccountPda,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 600_000);
  ok(`add_lp_and_burn: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

// crank_harvest_fees — discover holder ATAs with withheld fees via Helius DAS,
// then call program.crank_harvest_fees() with them as remaining_accounts.
// Batches at 18 ATAs/tx so we stay under the lock-set limit.
async function crankHarvestFees() {
  banner("CRANK HARVEST FEES");

  step("scanning Token-2022 accounts for withheld fees…");
  const accountsWithFees = [];
  let cursor;
  let scanned = 0;
  let totalWithheld = 0n;

  do {
    const params = { mint: dbtcMint.toBase58(), limit: 1000 };
    if (cursor) params.cursor = cursor;
    const res = await fetch(`${HELIUS_RPC}/?api-key=${HELIUS_API_KEY}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: "harvest", method: "getTokenAccounts", params }),
    });
    const data = await res.json();
    if (data.error) {
      warn(`helius error: ${JSON.stringify(data.error)}`);
      break;
    }
    const list = data.result?.token_accounts || [];
    scanned += list.length;
    for (const a of list) {
      const w = a.token_extensions?.transfer_fee_amount?.withheld_amount;
      if (w && BigInt(w) > 0n) {
        accountsWithFees.push({ address: a.address, withheld: BigInt(w) });
        totalWithheld += BigInt(w);
      }
    }
    cursor = data.result?.cursor;
  } while (cursor);

  console.log(`   scanned ${scanned} ATAs · ${accountsWithFees.length} have fees · total ${dbtc(totalWithheld)} dBTC`);
  if (accountsWithFees.length === 0) {
    warn("nothing to harvest");
    return null;
  }

  step("submitting harvest batches (18 ATAs/tx)…");
  const BATCH = 18;
  let ok_n = 0;
  let fail_n = 0;
  for (let i = 0; i < accountsWithFees.length; i += BATCH) {
    const chunk = accountsWithFees.slice(i, i + BATCH);
    const remaining = chunk.map((a) => ({
      pubkey: new PublicKey(a.address),
      isSigner: false,
      isWritable: true,
    }));
    try {
      const tx = await program.methods.crankHarvestFees().accounts({
        degenbtcMint: dbtcMint,
        tokenProgram2022: TOKEN_2022_PROGRAM_ID,
      }).remainingAccounts(remaining).transaction();
      const sig = await send(tx, 200_000);
      ok_n++;
      console.log(`   batch ${ok_n}: ${chunk.length} ATAs · ${sig}`);
    } catch (e) {
      fail_n++;
      warn(`batch ${i / BATCH + 1} failed: ${e.message}`);
    }
  }
  console.log(`   ${ok_n} batches ok · ${fail_n} failed`);
}

async function crankDistributeTax() {
  banner("CRANK DISTRIBUTE TAX");
  const factionWarId = await fetchFactionWarId();
  console.log(`   current war_id: ${factionWarId}`);

  // crank_distribute_tax now requires war_state to exist (typed
  // Box<Account<FactionWarState>>). Init lazily so the cranker is self-healing
  // across cycle boundaries — same pattern as startRound.
  await initializeFactionWar(factionWarId).catch((err) => {
    if (!/already in use|already initialized|AccountAlready/i.test(String(err?.message || ""))) {
      throw err;
    }
  });

  // Withdraw-authority ATA must exist before withdraw_withheld_tokens_from_mint.
  const withdrawAuthAta = await ensureAta(
    withdrawWithheldAuthorityPda, dbtcMint, TOKEN_2022_PROGRAM_ID, true,
  );

  const factionWarStatePda = deriveFactionWarStatePda(factionWarId);

  const tx = await program.methods.crankDistributeTax(new BN(factionWarId)).accounts({
    degenbtcMint: dbtcMint,
    withdrawWithheldAuthority: withdrawWithheldAuthorityPda,
    withdrawAuthorityTokenAccount: withdrawAuthAta,
    factionTreasuryVault,
    dbtcMining: mineBtcMiningPda,
    vaultAuthority: dbtcVaultAuthorityPda,
    dbtcTokenVault: dbtcTokenVaultPda,
    taxConfig: taxConfigPda,
    warConfig: factionWarConfigPda,
    warState: factionWarStatePda,
    caller: walletKeypair.publicKey,
    tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 400_000);
  ok(`crank_distribute_tax: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

// ════════════════════════════════════════════════════════════════════
//  GAME CRANKER
// ════════════════════════════════════════════════════════════════════

/**
 * Initialize the FactionWarState + FactionWarSettlement PDAs for the current
 * war_id. Must be called once per 4h cycle BEFORE the cycle's first
 * `start_round`, otherwise start_round errors with AccountNotInitialized on
 * `war_state` (3012 / 0xbc4). Permissionless — anyone can call.
 *
 * Idempotent in practice: if the PDAs already exist on-chain for this war_id
 * the call fails (account already initialized) — catch and continue.
 */
async function initializeFactionWar(warId) {
  if (warId == null) warId = await fetchFactionWarId();
  banner(`INIT FACTION WAR ${warId}`);

  // Skip if already initialized — avoids a wasted tx + the AccountAlreadyInitialized revert.
  try {
    const existing = await program.account.factionWarState.fetch(
      deriveFactionWarStatePda(warId)
    );
    if (existing) {
      ok(`war ${warId} already initialized — skipping`);
      return { warId, sig: null };
    }
  } catch (_) {
    // not found yet — proceed with init
  }

  const tx = await program.methods.initializeFactionWar(new BN(warId)).accounts({
    globalConfig: globalConfigPda,
    warConfig: factionWarConfigPda,
    warState: deriveFactionWarStatePda(warId),
    warSettlement: deriveFactionWarSettlementPda(warId),
    taxConfig: taxConfigPda,
    dbtcMining: mineBtcMiningPda,
    authority: walletKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 400_000);
  ok(`war ${warId} initialized: ${sig}`);
  console.log(`   ${explorer(sig)}`);
  return { warId, sig };
}

async function startRound(roundId) {
  if (!roundId) {
    const gs = await program.account.globalGameSate.fetch(globalGameStatePda);
    roundId = (gs.currentRoundId?.toNumber() ?? 0) + 1;
  }
  banner(`START ROUND ${roundId}`);

  // start_round requires war_state for war_config.current_war_id. If it's
  // missing the on-chain ix reverts AccountNotInitialized (3012). Init lazily
  // so the cranker is self-healing across cycle boundaries.
  const warId = await fetchFactionWarId();
  await initializeFactionWar(warId).catch((err) => {
    // Tolerate "already initialized" — anything else should bubble.
    if (!/already in use|already initialized|AccountAlready/i.test(String(err?.message || ""))) {
      throw err;
    }
  });

  const tx = await program.methods.startRound(new BN(roundId)).accounts({
    globalConfig: globalConfigPda,
    globalGameState: globalGameStatePda,
    gameSession: deriveGameSessionPda(roundId),
    warConfig: factionWarConfigPda,
    warState: deriveFactionWarStatePda(warId),
    authority: walletKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 500_000);
  ok(`round ${roundId} started: ${sig}`);
  console.log(`   ${explorer(sig)}`);
  return { roundId, sig };
}

async function endRound() {
  const gs = await program.account.globalGameSate.fetch(globalGameStatePda);
  const roundId = gs.currentRoundId?.toNumber() ?? 0;
  if (roundId <= 0) throw new Error("no active round to end");
  banner(`END ROUND ${roundId}`);

  const tx = await program.methods.endRound().accounts({
    gameSession: deriveGameSessionPda(roundId),
    dbtcMining: mineBtcMiningPda,
    globalGameState: globalGameStatePda,
    globalConfig: globalConfigPda,
    slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
    authority: walletKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 1_000_000);
  ok(`round ${roundId} ended: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const session = await program.account.gameSession.fetch(deriveGameSessionPda(roundId));
  console.log(`   winning_faction_id: ${session.winningFactionId}`);
  console.log(`   winning_direction : ${session.winningDirection}`);
  return { roundId, sig, session };
}

async function settleRound() {
  const gs = await program.account.globalGameSate.fetch(globalGameStatePda);
  const roundId = gs.currentRoundId?.toNumber() ?? 0;
  if (roundId <= 0) throw new Error("no active round");
  banner(`SETTLE ROUND · round ${roundId}`);

  const session = await program.account.gameSession.fetch(deriveGameSessionPda(roundId));
  if (session.winningFactionId == null) {
    throw new Error("end_round must run first — no winner picked yet");
  }
  const factionWarId = await fetchFactionWarId();
  const factionStatePda = deriveFactionStatePda(session.winningFactionId);

  const tx = await program.methods.settleRound(new BN(factionWarId)).accounts({
    globalGameState: globalGameStatePda,
    gameSession: deriveGameSessionPda(roundId),
    globalConfig: globalConfigPda,
    factionState: factionStatePda,
    solRewardsVault: solRewardsVaultPda,
    solPrizePotVault: solPrizePotVaultPda,
    warConfig: factionWarConfigPda,
    warState: deriveFactionWarStatePda(factionWarId),
    dbtcMining: mineBtcMiningPda,
    authority: walletKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 1_000_000);
  ok(`settle_round: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return { roundId, sig };
}

async function settleFactionWar() {
  banner("SETTLE FACTION WAR");
  const factionWarId = await fetchFactionWarId();
  const factionWarStatePda = deriveFactionWarStatePda(factionWarId);

  const gs = await program.account.globalGameSate.fetch(globalGameStatePda);
  const roundId = gs.currentRoundId?.toNumber() ?? 0;

  const tx = await program.methods.settleWar().accounts({
    warConfig: factionWarConfigPda,
    warState: factionWarStatePda,
    warSettlement: deriveFactionWarSettlementPda(factionWarId),
    taxConfig: taxConfigPda,
    dbtcMining: mineBtcMiningPda,
    globalConfig: globalConfigPda,
    factionWarSolVault: factionWarSolVaultPda,
    solTreasury: solTreasuryPda,
    systemProgram: SystemProgram.programId,
    cranker: walletKeypair.publicKey,
  }).transaction();

  const sig = await send(tx, 800_000);
  ok(`settle_war: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

// ════════════════════════════════════════════════════════════════════
//  NFT MARKETPLACE CRANKER
// ════════════════════════════════════════════════════════════════════

async function recordFloorSnapshot() {
  banner("RECORD FLOOR SNAPSHOT");
  if (!marketplaceConfigPda) {
    throw new Error("Marketplace config PDA missing. Run 3_init_mineBTC.js through marketplace/inventory init first.");
  }
  if (!marketplaceProgramId) {
    throw new Error("Marketplace program ID missing. Run 3_init_mineBTC.js through marketplace/inventory init first.");
  }
  const floorQueue = await program.account.floorQueue.fetch(floorQueuePda);
  const queueCount = Number(floorQueue.entriesCount ?? 0);
  let queueMedianListing = null;
  let queueMedianAsset = null;
  let queueMedianEscrow = null;
  if (queueCount > 0) {
    const entry = floorQueue.entries[Math.floor(queueCount / 2)];
    queueMedianListing = entry.listing;
    queueMedianAsset = entry.asset;
    [queueMedianEscrow] = PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), marketplaceConfigPda.toBuffer(), queueMedianAsset.toBuffer()],
      marketplaceProgramId,
    );
  }
  const tx = await program.methods.recordFloorSnapshot().accounts({
    caller: walletKeypair.publicKey,
    inventoryPool: inventoryPoolPda,
    floorQueue: floorQueuePda,
    saleHistory: saleHistoryPda,
    marketplaceConfig: marketplaceConfigPda,
    queueMedianListing,
    queueMedianAsset,
    queueMedianEscrow,
    floorHistory: floorHistoryPda,
    inventorySweepVault: inventorySweepVaultPda,
    systemProgram: SystemProgram.programId,
  }).transaction();

  const sig = await send(tx, 300_000);
  ok(`record_floor_snapshot: ${sig}`);
  console.log(`   ${explorer(sig)}`);

  const txInfo = await connection.getTransaction(sig,
    { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  logEvents(txInfo);
  return sig;
}

// ════════════════════════════════════════════════════════════════════
//  MAIN — uncomment the steps you want to run
// ════════════════════════════════════════════════════════════════════

async function main() {
  // ── status / inspection (run anytime) ──
  // await printState();
  // await printGameState();

  // ════════════════════════════════════════════════════════════════
  //  PER 4H CYCLE — call ONCE when current_war_id advances
  // ════════════════════════════════════════════════════════════════
  // await initializeFactionWar();   // creates war_state PDA for current war_id
                                     // (startRound now lazy-inits this too, but
                                     //  calling explicitly makes the cycle boundary
                                     //  observable in tx history.)

  // ════════════════════════════════════════════════════════════════
  //  PER ROUND LOOP  — repeat a → c → d every ~2 min
  // ════════════════════════════════════════════════════════════════
  // await startRound();             // a. opens new round (auto-inits war_state if needed)
  await endRound();                // c. reveal entropy + pick winner (wait for round timer)
  await settleRound();             // d. pay stakers + advance faction-war mining

  // ════════════════════════════════════════════════════════════════
  //  PRICE / EMISSION RAIL  — runs ~every 30 min inside the cycle
  // ════════════════════════════════════════════════════════════════
  // await distributeSolFees();       // drain sol_treasury → buyback + dev multisig
  // await snapshotPrice();          // 8x per cycle (every ~30 min)
  // await updateRate();             // ONCE after 8th snapshot
  // await addLpAndBurn();           // ONCE after update_rate flips lp_operation_pending
  //                                 // (crosses settle_at_lp_op_count → locks cycle_end_round_id)

  // ════════════════════════════════════════════════════════════════
  //  END OF CYCLE — runs ONCE after cycle_end_round_id is settled
  // ════════════════════════════════════════════════════════════════
  // await settleFactionWar();        // permissionless, finalizes ranks
  // await crankDistributeTax();      // split accumulated tax: 25% faction / 50% burn / 25% recycle
  //                                  // → loop back: call initializeFactionWar() for the NEW war_id

  // ════════════════════════════════════════════════════════════════
  //  PERIODIC (independent of round/cycle cadence)
  // ════════════════════════════════════════════════════════════════
  // await sendSolToTreasury(0.05);   // top up sol_treasury for testing
  // await crankHarvestFees();        // pull withheld fees from holder ATAs into mint
  // await recordFloorSnapshot();     // ~daily floor anchor for breed/sweep pricing
}

main().catch((err) => {
  console.error("\n❌ FATAL:", err.message);
  if (err.logs) {
    console.error("   logs:");
    for (const l of err.logs) console.error(`     ${l}`);
  }
  console.error(err.stack);
  process.exit(1);
});
