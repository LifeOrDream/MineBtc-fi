#!/usr/bin/env node
/**
 * rotate_admin.js — initiate GlobalConfig.ext_authority transfer.
 *
 *   node rotate_admin.js <NEW_AUTHORITY_PUBKEY>
 *
 * This is the FIRST step of a 2-step rotation:
 *   1) Deployer (current ext_authority) calls update_config(new_authority).
 *      That sets pending_authority on chain. Old authority still in effect.
 *   2) New authority calls accept_authority. Finalizes the rotation.
 *
 * Until step 2, the deployer can cancel by re-running with a different
 * pubkey or calling cancel_authority_transfer.
 */

import { Keypair, Connection, PublicKey, Transaction, ComputeBudgetProgram, sendAndConfirmTransaction, SystemProgram } from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { setIdlAddress } from "./raydium_id_sync.js";
import { loadConfig } from "./load_config.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const arg = process.argv[2];
if (!arg) {
  console.error("Usage: node rotate_admin.js <NEW_AUTHORITY_PUBKEY>");
  process.exit(1);
}
let newAuthority;
try {
  newAuthority = new PublicKey(arg);
} catch (e) {
  console.error(`Invalid pubkey: ${arg}`);
  process.exit(1);
}

const config = loadConfig(path.join(__dirname, "config.json"));
const cluster = config.network.cluster;
const deployment = JSON.parse(fs.readFileSync(path.join(__dirname, "deployments", `${cluster}.json`), "utf8"));
const idl = setIdlAddress(
  JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.minebtc_idl), "utf8")),
  deployment.MINE_BTC_PROGRAM_ID,
);
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.deployer_key), "utf8"))),
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
const provider = new AnchorProvider(connection, new Wallet(walletKeypair), { commitment: config.network.commitment });
const program = new Program(idl, provider);
const pid = program.programId;

const [globalConfigPda] = PublicKey.findProgramAddressSync([Buffer.from("global-config")], pid);

(async () => {
  const gc = await program.account.globalConfig.fetch(globalConfigPda);
  console.log(`cluster                : ${cluster}`);
  console.log(`program                : ${pid.toBase58()}`);
  console.log(`signer (deployer)      : ${walletKeypair.publicKey.toBase58()}`);
  console.log(`current ext_authority  : ${gc.extAuthority.toBase58()}`);
  console.log(`current pending        : ${gc.pendingAuthority.toBase58()}`);
  console.log(`new pending (target)   : ${newAuthority.toBase58()}`);

  if (!gc.extAuthority.equals(walletKeypair.publicKey)) {
    console.error("❌ Signer is NOT current ext_authority. update_config will fail.");
    process.exit(2);
  }

  const ix = await program.methods
    .updateConfig(newAuthority, null)
    .accounts({
      globalConfig: globalConfigPda,
      authority: walletKeypair.publicKey,
    })
    .instruction();

  const tx = new Transaction()
    .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 80_000 }))
    .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5_000 }))
    .add(ix);

  const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair], { commitment: "confirmed" });
  console.log("");
  console.log(`✅ update_config(new_authority=${newAuthority.toBase58()}) sent`);
  console.log(`   sig: ${sig}`);
  console.log(`   https://explorer.solana.com/tx/${sig}`);

  const after = await program.account.globalConfig.fetch(globalConfigPda);
  console.log(`pending_authority now  : ${after.pendingAuthority.toBase58()}`);
  console.log("");
  console.log("⚠️  ROTATION INCOMPLETE — step 2 of 2 required:");
  console.log(`    The new authority (${newAuthority.toBase58()}) must call`);
  console.log(`    accept_authority via Squads to finalize.`);
})().catch((e) => {
  console.error("❌ FATAL:", e.message);
  if (e.logs) for (const l of e.logs) console.error("   ", l);
  process.exit(1);
});
