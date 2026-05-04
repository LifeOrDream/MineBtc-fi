// One-off: end round 4 on devnet using the upgraded program.
// Expected: scheduled entropy slot has aged out → fallback path returns instantly.

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  ComputeBudgetProgram,
  Transaction,
  sendAndConfirmTransaction,
  SYSVAR_SLOT_HASHES_PUBKEY,
} from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const cfg = JSON.parse(fs.readFileSync(path.join(__dirname, "config.json"), "utf8"));
const dep = JSON.parse(fs.readFileSync(path.join(__dirname, "deployments", "devnet.json"), "utf8"));
const idl = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, cfg.deployment.paths.minebtc_idl), "utf8")
);
const wallet = Keypair.fromSecretKey(
  new Uint8Array(
    JSON.parse(
      fs.readFileSync(path.resolve(__dirname, cfg.deployment.paths.deployer_key), "utf8")
    )
  )
);

const conn = new Connection(cfg.network.rpc_url, "confirmed");
const provider = new AnchorProvider(conn, new Wallet(wallet), { commitment: "confirmed" });
const program = new Program(idl, provider);
const PID = program.programId;

const [globalConfig] = PublicKey.findProgramAddressSync([Buffer.from("global-config")], PID);
const [mineBtcMining] = PublicKey.findProgramAddressSync([Buffer.from("mine-btc-mining")], PID);
const [globalGameState] = PublicKey.findProgramAddressSync(
  [Buffer.from("global-game-state")],
  PID
);

const gs = await program.account.globalGameSate.fetch(globalGameState);
const currentRoundId = Number(gs.currentRoundId.toString());
console.log(`current_round_id on-chain: ${currentRoundId}`);

const [gameSession] = PublicKey.findProgramAddressSync(
  [Buffer.from("game-session"), gs.currentRoundId.toArrayLike(Buffer, "le", 8)],
  PID
);
console.log(`game_session PDA: ${gameSession.toString()}`);

// Pull the round to inspect scheduled entropy slot
const session = await program.account.gameSession.fetch(gameSession);
console.log(`round_id:                 ${session.roundId.toString()}`);
console.log(`stage:                    ${session.stage}`);
console.log(`round_start_slot:         ${session.roundStartSlot.toString()}`);
console.log(`scheduled_entropy_slot:   ${session.scheduledEntropySlot.toString()}`);
const slot = await conn.getSlot();
console.log(`current cluster slot:     ${slot}`);
console.log(`age (slots):              ${slot - Number(session.scheduledEntropySlot.toString())}`);
console.log("");
console.log("Sending endRound...");

const tx = await program.methods
  .endRound()
  .accounts({
    gameSession,
    mineBtcMining,
    globalGameState,
    globalConfig,
    slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
    authority: wallet.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 })])
  .rpc({ commitment: "confirmed" });
console.log(`✅ endRound tx: ${tx}`);
console.log(`https://explorer.solana.com/tx/${tx}?cluster=devnet`);
