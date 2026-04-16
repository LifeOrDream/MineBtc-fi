#!/usr/bin/env node

/**
 * Round cranker for the live MineBTC country-arena game.
 *
 * The contract moved off a manual commit/reveal scheme; entropy now comes
 * from the SlotHashes sysvar at a scheduled post-round slot. Each iteration:
 *
 *   1. Read on-chain state (current round, round end timestamp).
 *   2. If the current round has ended, call `end_round` (reveals via
 *      SlotHashes) and `end_round_faction_rewards` (pays stakers, advances
 *      rebase mining pool).
 *   3. Start the next round with `start_round(round_id)`.
 *
 * Cycle settlement (settle_rebase) is driven by the LP-burn count so the
 * economy loop handles it, not this script.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_SLOT_HASHES_PUBKEY,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ============================================================================
// CONFIGURATION & INITIALIZATION
// ============================================================================

const LOOP_INTERVAL_MS = 60000; // 60 seconds

const configPath = path.join(__dirname, "../config.json");
if (!fs.existsSync(configPath)) {
  console.error(`❌ Config not found at: ${configPath}`);
  process.exit(1);
}
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));

const deploymentPath = path.join(
  __dirname,
  "../deployments",
  `${config.network.cluster}.json`
);
if (!fs.existsSync(deploymentPath)) {
  console.error(`❌ Deployment file not found at: ${deploymentPath}`);
  process.exit(1);
}
const deployment = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));

const minebtcIdlPath = path.resolve(__dirname, "../../target/idl/minebtc.json");
if (!fs.existsSync(minebtcIdlPath)) {
  console.error(`❌ MineBTC IDL not found at: ${minebtcIdlPath}`);
  process.exit(1);
}
const minebtcIdl = JSON.parse(fs.readFileSync(minebtcIdlPath, "utf8"));

const walletPath = path.resolve(__dirname, "../../game_keypair.json");
if (!fs.existsSync(walletPath)) {
  console.error(`❌ Wallet keypair not found at: ${walletPath}`);
  process.exit(1);
}
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf8")))
);

const connection = new Connection(
  config.network.rpc_url,
  config.network.commitment
);
const wallet = new Wallet(walletKeypair);
const provider = new AnchorProvider(connection, wallet, {
  commitment: config.network.commitment,
});
const mineBTCProgram = new Program(minebtcIdl, provider);
const mineBTCProgramId = mineBTCProgram.programId;

// PDA seeds (match programs/mineBTC/src/state.rs)
const GLOBAL_CONFIG_SEED = "global-config";
const GLOBAL_GAME_STATE_SEED = "global-game-state";
const GAME_SESSION_SEED = "game-session";
const FACTION_STATE_SEED = "faction";
const REBASE_CONFIG_SEED = "rebase-config";
const REBASE_STATE_SEED = "rebase";
const STAKER_SOL_REWARD_VAULT_SEED = "staker-sol-reward-vault";
const MINE_BTC_MINING_SEED = "mine-btc-mining";

const [globalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_CONFIG_SEED)],
  mineBTCProgramId
);
const [globalGameStatePDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_GAME_STATE_SEED)],
  mineBTCProgramId
);
const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(MINE_BTC_MINING_SEED)],
  mineBTCProgramId
);
const [solRewardsVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(STAKER_SOL_REWARD_VAULT_SEED)],
  mineBTCProgramId
);
const [rebaseConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(REBASE_CONFIG_SEED)],
  mineBTCProgramId
);

// ============================================================================
// UTILITIES
// ============================================================================

function u64Buffer(value) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value), 0);
  return buffer;
}

function deriveGameSessionPDA(roundId) {
  const [gameSessionPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(GAME_SESSION_SEED), u64Buffer(roundId)],
    mineBTCProgramId
  );
  return gameSessionPDA;
}

function deriveRebaseStatePDA(rebaseId) {
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from(REBASE_STATE_SEED), u64Buffer(rebaseId)],
    mineBTCProgramId
  );
  return pda;
}

function deriveFactionStatePDA(factionId) {
  // Contract seeds: [b"faction", faction_name.as_bytes()]
  // Faction names come from config.factions (order = faction_id).
  const factionName = config.factions[factionId]?.name;
  if (!factionName) {
    throw new Error(`Unknown faction ID ${factionId} — not in config.factions`);
  }
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
    mineBTCProgramId
  );
  return pda;
}

// ============================================================================
// ON-CHAIN STATE READERS
// ============================================================================

async function getGlobalGameState() {
  const state = await mineBTCProgram.account.globalGameSate.fetch(
    globalGameStatePDA
  );

  let roundEndTimestamp = 0;
  const currentRoundId = state.currentRoundId ? state.currentRoundId.toNumber() : 0;
  if (currentRoundId > 0) {
    try {
      const gameSession = await mineBTCProgram.account.gameSession.fetch(
        deriveGameSessionPDA(currentRoundId)
      );
      roundEndTimestamp = Number(gameSession.roundEndTimestamp);
    } catch (error) {
      console.warn(
        `⚠️  Could not fetch current game session: ${error.message}`
      );
    }
  }

  return {
    isActive: state.isActive,
    canBeginRound: state.canBeginRound,
    currentRoundId,
    lastRoundId: state.lastRoundId ? state.lastRoundId.toNumber() : 0,
    roundDurationSeconds: state.roundDurationSeconds
      ? state.roundDurationSeconds.toNumber()
      : 0,
    winningFactionId: state.winningFactionId ?? null,
    roundEndTimestamp,
  };
}

async function getCurrentGameSession(roundId) {
  if (!roundId || roundId <= 0) return null;
  try {
    return await mineBTCProgram.account.gameSession.fetch(
      deriveGameSessionPDA(roundId)
    );
  } catch {
    return null;
  }
}

// ============================================================================
// ROUND OPERATIONS
// ============================================================================

async function startRound(roundId) {
  console.log(`\n🎮 Starting round ${roundId}...`);

  const tx = await mineBTCProgram.methods
    .startRound(new BN(roundId))
    .accounts({
      globalGameState: globalGameStatePDA,
      gameSession: deriveGameSessionPDA(roundId),
      rebaseConfig: rebaseConfigPDA,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .transaction();

  tx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 })
  );

  const signature = await sendAndConfirmTransaction(
    connection,
    tx,
    [walletKeypair],
    { commitment: "confirmed" }
  );
  console.log(`✅ Round ${roundId} started: ${signature}`);
  return { success: true, signature };
}

async function endRound(roundId) {
  console.log(`\n🏁 Ending round ${roundId}...`);
  const gameSessionPDA = deriveGameSessionPDA(roundId);

  // STAGE 1 — reveal entropy via SlotHashes and pick the winner.
  const endRoundTx = await mineBTCProgram.methods
    .endRound()
    .accounts({
      gameSession: gameSessionPDA,
      mineBtcMining: mineBtcMiningPDA,
      globalGameState: globalGameStatePDA,
      globalConfig: globalConfigPDA,
      slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .transaction();
  endRoundTx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })
  );
  const endSignature = await sendAndConfirmTransaction(
    connection,
    endRoundTx,
    [walletKeypair],
    { commitment: "confirmed" }
  );

  const gameSession = await mineBTCProgram.account.gameSession.fetch(gameSessionPDA);
  const winningFactionId = gameSession.winningFactionId;
  const winningDirection = gameSession.winningDirection;
  console.log(
    `   Winning country: ${winningFactionId}, direction: ${winningDirection}`
  );

  // STAGE 2 — faction rewards + rebase mining accounting.
  const rebaseConfig = await mineBTCProgram.account.rebaseConfig.fetch(
    rebaseConfigPDA
  );
  const rebaseStatePDA = deriveRebaseStatePDA(
    rebaseConfig.currentRebaseId.toNumber()
  );
  const factionStatePDA = deriveFactionStatePDA(winningFactionId);

  const rewardsTx = await mineBTCProgram.methods
    .endRoundFactionRewards()
    .accounts({
      globalGameState: globalGameStatePDA,
      gameSession: gameSessionPDA,
      mineBtcMining: mineBtcMiningPDA,
      factionState: factionStatePDA,
      solRewardsVault: solRewardsVaultPDA,
      rebaseConfig: rebaseConfigPDA,
      rebaseState: rebaseStatePDA,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .transaction();
  rewardsTx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })
  );
  const rewardsSignature = await sendAndConfirmTransaction(
    connection,
    rewardsTx,
    [walletKeypair],
    { commitment: "confirmed" }
  );

  console.log(
    `✅ Round ${roundId} finalized: ${endSignature} / ${rewardsSignature}`
  );
  return { success: true, endSignature, rewardsSignature };
}

// ============================================================================
// MAIN LOOP
// ============================================================================

async function processRound(onChainState) {
  const { currentRoundId, roundEndTimestamp } = onChainState;
  const now = Math.floor(Date.now() / 1000);

  console.log("\n📊 Round Status:");
  console.log(`   Current Round: ${currentRoundId}`);
  console.log(
    `   Round End: ${new Date(roundEndTimestamp * 1000).toISOString()}`
  );
  console.log(
    `   Time Remaining: ${Math.max(0, roundEndTimestamp - now)}s`
  );

  if (currentRoundId > 0 && now >= roundEndTimestamp) {
    const session = await getCurrentGameSession(currentRoundId);
    if (session?.stage === 2) {
      // already fully settled — just proceed to the next round
    } else {
      const result = await endRound(currentRoundId);
      if (!result.success) {
        console.log("⚠️  Failed to end round, will retry");
        return false;
      }
    }
  }

  const nextRoundId = currentRoundId + 1;
  const startResult = await startRound(nextRoundId);
  if (!startResult.success) {
    console.log("⚠️  Failed to start round, will retry");
    return false;
  }
  return true;
}

async function runLoop() {
  console.log("\n🎮 Starting game loop...");
  console.log(`📡 Network: ${config.network.cluster}`);
  console.log(`🔗 RPC: ${config.network.rpc_url}`);
  console.log(`👛 Keeper: ${walletKeypair.publicKey.toString()}`);
  console.log(`⏰ Interval: ${LOOP_INTERVAL_MS / 1000}s\n`);

  const initialState = await getGlobalGameState();
  if (!initialState.isActive) {
    console.log("⚠️  Game is not active. Waiting for activation...");
  }

  let iteration = 0;
  while (true) {
    iteration++;
    console.log(`\n${"=".repeat(60)}`);
    console.log(`🔄 Iteration #${iteration} - ${new Date().toISOString()}`);
    console.log(`${"=".repeat(60)}`);

    try {
      const onChainState = await getGlobalGameState();

      if (!onChainState.isActive) {
        console.log("⏸️  Game is paused, waiting...");
        await new Promise((r) => setTimeout(r, LOOP_INTERVAL_MS));
        continue;
      }
      if (!onChainState.canBeginRound) {
        console.log(
          "⏳ canBeginRound=false — awaiting previous round settlement"
        );
      }

      await processRound(onChainState);
    } catch (error) {
      console.error(`❌ Error in loop iteration:`, error.message);
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
      }
    }

    console.log(`\n⏳ Waiting ${LOOP_INTERVAL_MS / 1000}s...`);
    await new Promise((r) => setTimeout(r, LOOP_INTERVAL_MS));
  }
}

process.on("SIGINT", () => {
  console.log("\n\n🛑 Received SIGINT, shutting down gracefully...");
  process.exit(0);
});
process.on("SIGTERM", () => {
  console.log("\n\n🛑 Received SIGTERM, shutting down gracefully...");
  process.exit(0);
});

runLoop().catch((error) => {
  console.error("❌ Fatal error:", error);
  process.exit(1);
});
