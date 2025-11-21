#!/usr/bin/env node

import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
  SYSVAR_SLOT_HASHES_PUBKEY,
} from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import crypto from "crypto";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ============================================================================
// CONFIGURATION & INITIALIZATION
// ============================================================================

const STATE_FILE = path.join(__dirname, "game_loop_state.json");
const LOOP_INTERVAL_MS = 60000; // 60 seconds

// Load config
const configPath = path.join(__dirname, "../config.json");
if (!fs.existsSync(configPath)) {
  console.error(`❌ Config not found at: ${configPath}`);
  process.exit(1);
}
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));

// Load deployment info
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

// Load MineBTC IDL
const minebtcIdlPath = path.resolve(__dirname, "../../target/idl/minebtc.json");
if (!fs.existsSync(minebtcIdlPath)) {
  console.error(`❌ MineBTC IDL not found at: ${minebtcIdlPath}`);
  process.exit(1);
}
const minebtcIdl = JSON.parse(fs.readFileSync(minebtcIdlPath, "utf8"));

// Load wallet keypair
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

// Seeds
const GLOBAL_CONFIG_SEED = "global-config";
const GLOBAL_GAME_STATE_SEED = "global-game-state";
const GAME_SESSION_SEED = "game-session";
const FACTION_STATE_SEED = "faction";

// Derive PDAs
const [globalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_CONFIG_SEED)],
  mineBTCProgramId
);

const [globalGameStatePDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_GAME_STATE_SEED)],
  mineBTCProgramId
);

const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("mine-btc-mining")],
  mineBTCProgramId
);

const [solPrizePotVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("sol-prize-pot")],
  mineBTCProgramId
);

const [dbtcEmissionVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from("dbtc-emission-vault")],
  mineBTCProgramId
);

// ============================================================================
// STATE PERSISTENCE
// ============================================================================

/**
 * Load persisted state from file
 */
function loadState() {
  if (!fs.existsSync(STATE_FILE)) {
    return {
      seeds: {}, // roundId -> seed (hex string)
      commits: {}, // roundId -> commit hash (hex string)
      lastSyncedRound: 0,
    };
  }

  try {
    const data = fs.readFileSync(STATE_FILE, "utf8");
    return JSON.parse(data);
  } catch (error) {
    console.warn(
      `⚠️  Failed to load state file, starting fresh:`,
      error.message
    );
    return {
      seeds: {},
      commits: {},
      lastSyncedRound: 0,
    };
  }
}

/**
 * Save state to file
 */
function saveState(state) {
  try {
    fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
  } catch (error) {
    console.error(`❌ Failed to save state:`, error.message);
  }
}

/**
 * Convert buffer to hex string for JSON storage
 */
function bufferToHex(buffer) {
  return Buffer.from(buffer).toString("hex");
}

/**
 * Convert hex string to buffer
 */
function hexToBuffer(hex) {
  return Buffer.from(hex, "hex");
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

function generateRandomSeed() {
  return crypto.randomBytes(32);
}

function hashSeed(seed) {
  // TODO: Replace with keccak256 to match Rust implementation
  return crypto.createHash("sha256").update(seed).digest();
}

async function getGlobalGameState() {
  const globalState = await mineBTCProgram.account.globalGameSate.fetch(
    globalGameStatePDA
  );

  return {
    isActive: globalState.isActive,
    currentRoundId: globalState.currentRoundId
      ? globalState.currentRoundId.toNumber()
      : 0,
    lastRoundId: globalState.lastRoundId
      ? globalState.lastRoundId.toNumber()
      : 0,
    roundEndTimestamp: globalState.roundEndTimestamp
      ? globalState.roundEndTimestamp.toNumber()
      : 0,
    roundDurationSeconds: globalState.roundDurationSeconds
      ? globalState.roundDurationSeconds.toNumber()
      : 0,
    currentRoundCommit: globalState.currentRoundCommit
      ? Buffer.from(globalState.currentRoundCommit).toString("hex")
      : null,
    currentRoundSeed: globalState.currentRoundSeed
      ? Buffer.from(globalState.currentRoundSeed).toString("hex")
      : null,
    nextRoundCommit: globalState.nextRoundCommit
      ? Buffer.from(globalState.nextRoundCommit).toString("hex")
      : null,
  };
}

function deriveGameSessionPDA(roundId) {
  const roundIdBuffer = Buffer.allocUnsafe(8);
  roundIdBuffer.writeBigUInt64LE(BigInt(roundId), 0);

  const [gameSessionPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
    mineBTCProgramId
  );

  return gameSessionPDA;
}

function deriveFactionStatePDA(factionId) {
  const factionIdBuffer = Buffer.from([factionId]);

  const [factionStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(FACTION_STATE_SEED), factionIdBuffer],
    mineBTCProgramId
  );

  return factionStatePDA;
}

// ============================================================================
// ROUND OPERATIONS
// ============================================================================

async function startRound(roundId, commitHash) {
  console.log(`\n🎮 Starting round ${roundId}...`);

  const gameSessionPDA = deriveGameSessionPDA(roundId);

  const tx = await mineBTCProgram.methods
    .startRound(new BN(roundId), commitHash ? Array.from(commitHash) : null)
    .accounts({
      globalGameState: globalGameStatePDA,
      gameSession: gameSessionPDA,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .transaction();

  tx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
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

async function endRound(roundId, revealedSeed, nextRoundCommit) {
  console.log(`\n🏁 Ending round ${roundId}...`);

  const gameSessionPDA = deriveGameSessionPDA(roundId);

  // Try to estimate winning faction
  let winningFactionId = await estimateWinningFaction(roundId);

  // Try ending with estimated faction
  try {
    return await tryEndRound(
      roundId,
      gameSessionPDA,
      revealedSeed,
      nextRoundCommit,
      winningFactionId
    );
  } catch (error) {
    // If wrong faction, try all factions
    if (
      error.message.includes("InvalidFactionId") ||
      error.message.includes("faction")
    ) {
      console.log(`   Retrying with all factions...`);
      for (let factionId = 0; factionId < 12; factionId++) {
        try {
          return await tryEndRound(
            roundId,
            gameSessionPDA,
            revealedSeed,
            nextRoundCommit,
            factionId
          );
        } catch (retryError) {
          if (!retryError.message.includes("InvalidFactionId")) {
            throw retryError;
          }
        }
      }
      throw new Error("Failed to find correct winning faction");
    }
    throw error;
  }
}

async function tryEndRound(
  roundId,
  gameSessionPDA,
  revealedSeed,
  nextRoundCommit,
  factionId
) {
  const factionStatePDA = deriveFactionStatePDA(factionId);

  const tx = await mineBTCProgram.methods
    .endRound(Array.from(revealedSeed), Array.from(nextRoundCommit))
    .accounts({
      globalGameState: globalGameStatePDA,
      gameSession: gameSessionPDA,
      globalConfig: globalConfigPDA,
      dogeBtcMining: dogeBtcMiningPDA,
      winningFactionState: factionStatePDA,
      solPrizePotVault: solPrizePotVaultPDA,
      dbtcEmissionVault: dbtcEmissionVaultPDA,
      slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
      authority: walletKeypair.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .transaction();

  tx.instructions.unshift(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1000000 })
  );

  const signature = await sendAndConfirmTransaction(
    connection,
    tx,
    [walletKeypair],
    { commitment: "confirmed" }
  );

  console.log(
    `✅ Round ${roundId} ended with faction ${factionId}: ${signature}`
  );
  return { success: true, signature };
}

async function estimateWinningFaction(roundId) {
  try {
    const gameSessionPDA = deriveGameSessionPDA(roundId);
    const gameSession = await mineBTCProgram.account.gameSession.fetch(
      gameSessionPDA
    );

    const userBlockIndexes = gameSession.userBlockIndexes;
    let maxUsers = 0;
    let estimatedBlock = 0;

    for (let i = 0; i < userBlockIndexes.length; i++) {
      if (userBlockIndexes[i] > maxUsers) {
        maxUsers = userBlockIndexes[i];
        estimatedBlock = i;
      }
    }

    if (maxUsers > 0) {
      const estimatedFaction = gameSession.blockAssignments[estimatedBlock];
      console.log(
        `   Estimated winning faction: ${estimatedFaction} (block ${
          estimatedBlock + 1
        })`
      );
      return estimatedFaction;
    }

    return 0;
  } catch (error) {
    console.warn(`   Could not estimate faction:`, error.message);
    return 0;
  }
}

// ============================================================================
// STATE SYNCHRONIZATION
// ============================================================================

/**
 * Sync local state with on-chain state
 * Ensures we have seeds and commits for all rounds we need
 */
async function syncStateWithChain(state, onChainState) {
  const currentRoundId = onChainState.currentRoundId;
  const lastRoundId = onChainState.lastRoundId;

  console.log(`\n🔄 Syncing state with chain...`);
  console.log(
    `   On-chain: Round ${currentRoundId}, Last completed: ${lastRoundId}`
  );
  console.log(`   Local: Last synced round ${state.lastSyncedRound}`);

  // If we have a current round commit on-chain, store it
  if (onChainState.currentRoundCommit && currentRoundId > 0) {
    state.commits[currentRoundId] = onChainState.currentRoundCommit;
    console.log(`   ✓ Stored commit for round ${currentRoundId} from chain`);
  }

  // If we have a next round commit on-chain, store it
  if (onChainState.nextRoundCommit) {
    const nextRoundId = currentRoundId + 1;
    state.commits[nextRoundId] = onChainState.nextRoundCommit;
    console.log(`   ✓ Stored commit for round ${nextRoundId} from chain`);
  }

  // If we have a revealed seed on-chain, store it
  if (onChainState.currentRoundSeed && currentRoundId > 0) {
    state.seeds[currentRoundId] = onChainState.currentRoundSeed;
    console.log(`   ✓ Stored seed for round ${currentRoundId} from chain`);
  }

  // Generate missing seeds/commits for future rounds
  const roundsToPrepare = Math.max(
    currentRoundId + 2,
    state.lastSyncedRound + 1
  );

  for (
    let roundId = state.lastSyncedRound + 1;
    roundId <= roundsToPrepare;
    roundId++
  ) {
    if (!state.seeds[roundId]) {
      const seed = generateRandomSeed();
      state.seeds[roundId] = bufferToHex(seed);
      console.log(`   ✓ Generated seed for round ${roundId}`);
    }

    if (!state.commits[roundId + 1]) {
      const seed = hexToBuffer(state.seeds[roundId]);
      const commit = hashSeed(seed);
      state.commits[roundId + 1] = bufferToHex(commit);
      console.log(`   ✓ Generated commit for round ${roundId + 1}`);
    }
  }

  state.lastSyncedRound = roundsToPrepare;
  saveState(state);

  return state;
}

/**
 * Get seed for a round (from state or generate if missing)
 */
function getSeedForRound(state, roundId) {
  if (state.seeds[roundId]) {
    return hexToBuffer(state.seeds[roundId]);
  }

  // Generate if missing
  const seed = generateRandomSeed();
  state.seeds[roundId] = bufferToHex(seed);
  saveState(state);
  return seed;
}

/**
 * Get commit hash for a round (from state or generate if missing)
 */
function getCommitForRound(state, roundId) {
  if (state.commits[roundId]) {
    return hexToBuffer(state.commits[roundId]);
  }

  // Generate from seed if we have it
  if (state.seeds[roundId - 1]) {
    const seed = hexToBuffer(state.seeds[roundId - 1]);
    const commit = hashSeed(seed);
    state.commits[roundId] = bufferToHex(commit);
    saveState(state);
    return commit;
  }

  // Generate new seed and commit
  const seed = generateRandomSeed();
  state.seeds[roundId - 1] = bufferToHex(seed);
  const commit = hashSeed(seed);
  state.commits[roundId] = bufferToHex(commit);
  saveState(state);
  return commit;
}

// ============================================================================
// MAIN LOOP
// ============================================================================

async function processRound(state, onChainState) {
  const currentRoundId = onChainState.currentRoundId;
  const roundEndTimestamp = onChainState.roundEndTimestamp;
  const currentTimestamp = Math.floor(Date.now() / 1000);
  const roundHasEnded = currentTimestamp >= roundEndTimestamp;

  console.log(`\n📊 Round Status:`);
  console.log(`   Current Round: ${currentRoundId}`);
  console.log(
    `   Round End: ${new Date(roundEndTimestamp * 1000).toISOString()}`
  );
  console.log(
    `   Time Remaining: ${Math.max(
      0,
      roundEndTimestamp - currentTimestamp
    )} seconds`
  );

  // If round has ended or no round exists, end current round first
  if (roundHasEnded && currentRoundId > 0) {
    const revealedSeed = getSeedForRound(state, currentRoundId);
    const nextRoundCommit = getCommitForRound(state, currentRoundId + 1);

    const result = await endRound(
      currentRoundId,
      revealedSeed,
      nextRoundCommit
    );
    if (!result.success) {
      console.log(`⚠️  Failed to end round, will retry`);
      return false;
    }

    console.log(`✅ Round ${currentRoundId} ended successfully`);
  }

  // Start next round if needed
  const nextRoundId = currentRoundId + 1;
  const nextRoundCommit = getCommitForRound(state, nextRoundId);

  const result = await startRound(nextRoundId, nextRoundCommit);
  if (!result.success) {
    console.log(`⚠️  Failed to start round, will retry`);
    return false;
  }

  console.log(`✅ Round ${nextRoundId} started successfully`);
  return true;
}

async function runLoop() {
  console.log("\n🎮 Starting game loop...");
  console.log(`📡 Network: ${config.network.cluster}`);
  console.log(`🔗 RPC: ${config.network.rpc_url}`);
  console.log(`👛 Wallet: ${walletKeypair.publicKey.toString()}`);
  console.log(`⏰ Interval: ${LOOP_INTERVAL_MS / 1000} seconds\n`);

  // Load persisted state
  let state = loadState();
  console.log(`📁 Loaded state from ${STATE_FILE}`);

  // Sync with on-chain state
  const onChainState = await getGlobalGameState();
  state = await syncStateWithChain(state, onChainState);

  // Check if game is active
  if (!onChainState.isActive) {
    console.log(`\n⚠️  Game is not active. Waiting for activation...`);
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
        console.log(`⏸️  Game is paused, waiting...`);
        await new Promise((resolve) => setTimeout(resolve, LOOP_INTERVAL_MS));
        continue;
      }

      // Sync state
      state = await syncStateWithChain(state, onChainState);

      // Process round
      await processRound(state, onChainState);
    } catch (error) {
      console.error(`❌ Error in loop iteration:`, error.message);
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
      }
    }

    // Wait before next iteration
    console.log(`\n⏳ Waiting ${LOOP_INTERVAL_MS / 1000} seconds...`);
    await new Promise((resolve) => setTimeout(resolve, LOOP_INTERVAL_MS));
  }
}

// Handle graceful shutdown
process.on("SIGINT", () => {
  console.log("\n\n🛑 Received SIGINT, shutting down gracefully...");
  process.exit(0);
});

process.on("SIGTERM", () => {
  console.log("\n\n🛑 Received SIGTERM, shutting down gracefully...");
  process.exit(0);
});

// Start the loop
runLoop().catch((error) => {
  console.error("❌ Fatal error:", error);
  process.exit(1);
});
