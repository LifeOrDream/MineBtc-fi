#!/usr/bin/env node
/**
 * set_game_active.js — flip GlobalGameSate.is_active on the current cluster.
 *
 *   node set_game_active.js false   # disable rounds (no new round can start)
 *   node set_game_active.js true    # re-enable rounds
 *
 * Signs with the deployer key from config.json (must be ext_authority on chain).
 * Does NOT touch HashBeastMintConfig — NFT minting stays however it was.
 */

import { Connection, Keypair, PublicKey, Transaction, ComputeBudgetProgram, sendAndConfirmTransaction, SystemProgram } from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { setIdlAddress } from "./raydium_id_sync.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const arg = process.argv[2];
if (arg !== "true" && arg !== "false") {
  console.error("Usage: node set_game_active.js <true|false>");
  process.exit(1);
}
const desired = arg === "true";

import { loadConfig } from "./load_config.js";
const config = loadConfig(path.join(__dirname, "config.json"));
const cluster = config.network.cluster;
const deployment = JSON.parse(
  fs.readFileSync(path.join(__dirname, "deployments", `${cluster}.json`), "utf8"),
);
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
const [globalGameStatePda] = PublicKey.findProgramAddressSync([Buffer.from("global-game-state")], pid);

(async () => {
  const gc = await program.account.globalConfig.fetch(globalConfigPda);
  const gs = await program.account.globalGameSate.fetch(globalGameStatePda);

  console.log(`cluster              : ${cluster}`);
  console.log(`program              : ${pid.toBase58()}`);
  console.log(`signer (deployer)    : ${walletKeypair.publicKey.toBase58()}`);
  console.log(`on-chain ext_authority: ${gc.extAuthority.toBase58()}`);
  console.log(`current is_active    : ${gs.isActive}`);
  console.log(`desired is_active    : ${desired}`);

  if (!gc.extAuthority.equals(walletKeypair.publicKey)) {
    console.error("❌ Signer is NOT ext_authority. update_game_state will fail with Unauthorized.");
    process.exit(2);
  }
  if (gs.isActive === desired) {
    console.log("✓ already in desired state — nothing to do.");
    return;
  }

  const ix = await program.methods
    .updateGameState(desired, null)
    .accounts({
      globalGameState: globalGameStatePda,
      globalConfig: globalConfigPda,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .instruction();

  const tx = new Transaction()
    .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 80_000 }))
    .add(ix);

  const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair], { commitment: "confirmed" });
  console.log(`✅ update_game_state(is_active=${desired}) sent`);
  console.log(`   sig: ${sig}`);
  console.log(`   https://explorer.solana.com/tx/${sig}${cluster === "mainnet" ? "" : `?cluster=${cluster}`}`);

  const after = await program.account.globalGameSate.fetch(globalGameStatePda);
  console.log(`on-chain is_active   : ${after.isActive}`);
})().catch((e) => {
  console.error("❌ FATAL:", e.message);
  if (e.logs) for (const l of e.logs) console.error("   ", l);
  process.exit(1);
});
