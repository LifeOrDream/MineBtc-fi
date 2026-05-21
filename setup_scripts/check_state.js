#!/usr/bin/env node
/**
 * check_state.js — read-only inspector. Prints the on-chain flags that
 * control "is the game live" without sending any tx.
 *
 *   node check_state.js
 *
 * Uses the cluster declared in config.json (network.cluster). For mainnet
 * that's the mainnet program; for devnet, devnet. No signing happens.
 */

import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { setIdlAddress } from "./raydium_id_sync.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

import { loadConfig } from "./load_config.js";
const config = loadConfig(path.join(__dirname, "config.json"));
const cluster = config.network.cluster;
const deployment = JSON.parse(
  fs.readFileSync(
    path.join(__dirname, "deployments", `${cluster}.json`),
    "utf8",
  ),
);
const idl = setIdlAddress(
  JSON.parse(
    fs.readFileSync(
      path.resolve(__dirname, config.deployment.paths.minebtc_idl),
      "utf8",
    ),
  ),
  deployment.MINE_BTC_PROGRAM_ID,
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
// Dummy wallet — we never sign anything in this script.
const wallet = new Wallet(Keypair.generate());
const provider = new AnchorProvider(connection, wallet, {
  commitment: config.network.commitment,
});
const program = new Program(idl, provider);
const pid = program.programId;

function pda(seedStrs) {
  const seeds = seedStrs.map((s) => (typeof s === "string" ? Buffer.from(s) : s));
  return PublicKey.findProgramAddressSync(seeds, pid)[0];
}

const globalConfigPda = pda(["global-config"]);
const globalGameStatePda = pda(["global-game-state"]);
const factionWarConfigPda = pda(["faction-war-config"]);
const hashBeastConfigPda = pda(["hashbeast-config"]);
const hashBeastMintConfigPda = pda(["hashbeast-mint-config"]);

const banner = (s) => {
  const line = "═".repeat(72);
  console.log(`\n${line}\n  ${s}\n${line}`);
};

async function safeFetch(accountName, pdaAddr) {
  try {
    return await program.account[accountName].fetch(pdaAddr);
  } catch (e) {
    console.log(`  ⚠️  ${accountName} @ ${pdaAddr.toBase58()} not initialized (${e.message})`);
    return null;
  }
}

(async () => {
  banner(`ON-CHAIN STATE — cluster=${cluster} program=${pid.toBase58()}`);

  const gc = await safeFetch("globalConfig", globalConfigPda);
  if (gc) {
    console.log(`  ── GlobalConfig @ ${globalConfigPda.toBase58()} ──`);
    console.log(`    is_paused              : ${gc.isPaused}        ← global kill switch (blocks bets+mints+breeds)`);
    console.log(`    ext_authority          : ${gc.extAuthority.toBase58()}`);
    console.log(`    pending_authority      : ${gc.pendingAuthority.toBase58()}`);
    console.log(`    rpg_progression        : ${gc.gameplayTuning.rpgProgression}`);
    console.log(`    max_evolution_stage    : ${gc.gameplayTuning.maxEvolutionStageUnlocked}`);
  }

  const gs = await safeFetch("globalGameSate", globalGameStatePda);
  if (gs) {
    console.log(`  ── GlobalGameSate @ ${globalGameStatePda.toBase58()} ──`);
    console.log(`    is_active              : ${gs.isActive}        ← if true, rounds can run`);
    console.log(`    can_begin_round        : ${gs.canBeginRound}`);
    console.log(`    current_round_id       : ${gs.currentRoundId.toString()}`);
    console.log(`    last_round_id          : ${gs.lastRoundId.toString()}`);
    console.log(`    round_duration_seconds : ${gs.roundDurationSeconds.toString()}`);
  }

  const fw = await safeFetch("factionWarConfig", factionWarConfigPda);
  if (fw) {
    console.log(`  ── FactionWarConfig @ ${factionWarConfigPda.toBase58()} ──`);
    console.log(`    is_active              : ${fw.isActive}`);
    console.log(`    current_war_id         : ${fw.currentWarId.toString()}`);
  }

  const hbc = await safeFetch("hashBeastConfig", hashBeastConfigPda);
  if (hbc) {
    console.log(`  ── HashBeastConfig @ ${hashBeastConfigPda.toBase58()} ──`);
    console.log(`    breeding_allowed       : ${hbc.breedingAllowed}`);
    console.log(`    total_hashbeasts_minted: ${hbc.totalHashbeastsMinted?.toString()}`);
  }

  const hbm = await safeFetch("hashBeastMintConfig", hashBeastMintConfigPda);
  if (hbm) {
    console.log(`  ── HashBeastMintConfig @ ${hashBeastMintConfigPda.toBase58()} ──`);
    console.log(`    is_active              : ${hbm.isActive}        ← if true, mint is LIVE`);
    console.log(`    base_price             : ${hbm.basePrice.toString()}`);
    console.log(`    curve_a                : ${hbm.curveA.toString()}`);
    console.log(`    genesis_mint_limit     : ${hbm.genesisMintLimit.toString()}`);
    console.log(`    genesis_mints (sold)   : ${hbm.genesisMints.toString()}`);
    console.log(`    max_per_faction        : ${hbm.maxGenesisMintsPerFaction}`);
  }

  console.log("");
})();
