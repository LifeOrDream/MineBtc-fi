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
  SYSVAR_SLOT_HASHES_PUBKEY
} from '@solana/web3.js';
import anchorPkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, Wallet } = anchorPkg;
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import crypto from 'crypto';
// Note: For keccak256 hashing (matching Rust's keccak::hash)
// Install with: npm install js-sha3
// Then import: import { keccak256 } from 'js-sha3';
// For now using SHA256 as placeholder - MUST be changed to keccak256 for production

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load config
const configPath = path.join(__dirname, '../config.json');
if (!fs.existsSync(configPath)) {
  console.error(`❌ Config not found at: ${configPath}`);
  process.exit(1);
}
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

// Load deployment info
const deploymentPath = path.join(__dirname, '../deployments', `${config.network.cluster}.json`);
if (!fs.existsSync(deploymentPath)) {
  console.error(`❌ Deployment file not found at: ${deploymentPath}`);
  process.exit(1);
}
const deployment = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));

// Load MoonBase IDL
const moonbaseIdlPath = path.resolve(__dirname, "../../target/idl/moonbase.json",);
if (!fs.existsSync(moonbaseIdlPath)) {
  console.error(`❌ MoonBase IDL not found at: ${moonbaseIdlPath}`);
  process.exit(1);
}
const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));

// Load wallet keypair
const walletPath = path.resolve(__dirname, "../../game_keypair.json");
if (!fs.existsSync(walletPath)) {
  console.error(`❌ Wallet keypair not found at: ${walletPath}`);
  process.exit(1);
}
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);

// Create wallet object
const wallet = new Wallet(walletKeypair);

// Initialize program (programId comes from IDL)
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
const moonBaseProgram = new Program(moonbaseIdl, provider);
const moonBaseProgramId = moonBaseProgram.programId;
console.log(`moonBaseProgramId ${moonBaseProgramId.toString()}`);

// Seeds
const GLOBAL_CONFIG_SEED = "global-config";
const GLOBAL_GAME_STATE_SEED = "global-game-state";
const GAME_SESSION_SEED = "game-session";
const DOGE_BTC_MINING_SEED = "moon-doge-mining";
const SOL_PRIZE_POT_VAULT_SEED = "sol-prize-pot";
const DBTC_EMISSION_VAULT_SEED = "dbtc-emission-vault";
const FACTION_STATE_SEED = "faction";

// Derive PDAs
const [globalConfigPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_CONFIG_SEED)],
  moonBaseProgramId
);

const [globalGameStatePDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(GLOBAL_GAME_STATE_SEED)],
  moonBaseProgramId
);

const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(DOGE_BTC_MINING_SEED)],
  moonBaseProgramId
);

const [solPrizePotVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(SOL_PRIZE_POT_VAULT_SEED)],
  moonBaseProgramId
);

const [dbtcEmissionVaultPDA] = PublicKey.findProgramAddressSync(
  [Buffer.from(DBTC_EMISSION_VAULT_SEED)],
  moonBaseProgramId
);

/**
 * Generate a random 32-byte seed
 */
function generateRandomSeed() {
  return crypto.randomBytes(32);
}

/**
 * Hash a seed using keccak256 (for commit-reveal scheme)
 * IMPORTANT: Must match Rust's keccak::hash function
 * Install keccak library: npm install keccak
 * Then use: const keccak = require('keccak'); keccak('keccak256').update(seed).digest();
 * 
 * For now using SHA256 as placeholder - MUST be changed to keccak256!
 */
function hashSeed(seed) {
  // TODO: Replace with keccak256 to match Rust implementation
  // const keccak = require('keccak');
  // return keccak('keccak256').update(seed).digest();
  
  // Temporary: using SHA256 (WILL FAIL commit-reveal verification!)
  return crypto.createHash('sha256').update(seed).digest();
}

/**
 * Get global game state and return decoded/human-readable format
 */
async function getGlobalGameState() {
  try {
    const globalState = await moonBaseProgram.account.globalGameSate.fetch(globalGameStatePDA);
    
    // Decode to human-readable format
    const decoded = {
      bump: globalState.bump,
      isActive: globalState.isActive,
      totalSolBets: globalState.totalSolBets ? globalState.totalSolBets.toString() : '0',
      totalSolBetsSOL: globalState.totalSolBets 
        ? (Number(globalState.totalSolBets) / LAMPORTS_PER_SOL).toFixed(4)
        : '0.0000',
      totalGlobalPassiveHashpower: globalState.totalGlobalPassiveHashpower 
        ? globalState.totalGlobalPassiveHashpower.toString() 
        : '0',
      currentRoundId: globalState.currentRoundId ? globalState.currentRoundId.toNumber() : 0,
      roundEndTimestamp: globalState.roundEndTimestamp ? globalState.roundEndTimestamp.toNumber() : 0,
      roundEndDate: globalState.roundEndTimestamp 
        ? new Date(globalState.roundEndTimestamp.toNumber() * 1000).toISOString()
        : null,
      roundDurationSeconds: globalState.roundDurationSeconds ? globalState.roundDurationSeconds.toNumber() : 0,
      roundDurationMinutes: globalState.roundDurationSeconds 
        ? (globalState.roundDurationSeconds.toNumber() / 60).toFixed(2)
        : '0.00',
      lastRoundId: globalState.lastRoundId ? globalState.lastRoundId.toNumber() : 0,
      winningFactionId: globalState.winningFactionId !== undefined ? globalState.winningFactionId : null,
      currentRoundCommit: globalState.currentRoundCommit 
        ? Buffer.from(globalState.currentRoundCommit).toString('hex')
        : null,
      currentRoundSeed: globalState.currentRoundSeed 
        ? Buffer.from(globalState.currentRoundSeed).toString('hex')
        : null,
      nextRoundCommit: globalState.nextRoundCommit 
        ? Buffer.from(globalState.nextRoundCommit).toString('hex')
        : null,
      crankerBots: globalState.crankerBots 
        ? globalState.crankerBots.map(pk => pk.toString())
        : [],
      crankerBotsCount: globalState.crankerBots ? globalState.crankerBots.length : 0,
    };
    
    return decoded;
  } catch (error) {
    console.error(`❌ Error fetching global game state:`, error.message);
    throw error;
  }
}

/**
 * Derive game session PDA for a given round ID
 */
function deriveGameSessionPDA(roundId) {
  const roundIdBuffer = Buffer.allocUnsafe(8);
  roundIdBuffer.writeBigUInt64LE(BigInt(roundId), 0);
  
  const [gameSessionPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
    moonBaseProgramId
  );
  
  return gameSessionPDA;
}

/**
 * Derive faction state PDA for a given faction ID
 */
function deriveFactionStatePDA(factionId) {
  const factionIdBuffer = Buffer.from([factionId]);
  
  const [factionStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from(FACTION_STATE_SEED), factionIdBuffer],
    moonBaseProgramId
  );
  
  return factionStatePDA;
}

/**
 * Execute start round transaction
 */
async function executeStartRound(roundId, commitHash) {
  try {
    console.log(`\n🎮 Starting round ${roundId}...`);
    
    const gameSessionPDA = deriveGameSessionPDA(roundId);
    
    const startRoundTx = await moonBaseProgram.methods
      .startRound(new BN(roundId), commitHash ? Array.from(commitHash) : null)
      .accounts({
        globalGameState: globalGameStatePDA,
        gameSession: gameSessionPDA,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .transaction();

    // Add compute unit limit instruction at the beginning
    startRoundTx.instructions.unshift(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 500000 })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      startRoundTx,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Round ${roundId} started: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error starting round:`, error.message);
    if (error.logs) {
      console.error('Transaction logs:', error.logs);
    }
    return { success: false, error: error.message };
  }
}

/**
 * Try to determine winning faction from game session block assignments
 * This is an estimate - the actual winning faction is determined during end_round
 * Returns faction 0 as default (will be validated and retried if wrong)
 */
async function estimateWinningFaction(currentRoundId) {
  try {
    const gameSessionPDA = deriveGameSessionPDA(currentRoundId);
    const gameSession = await moonBaseProgram.account.gameSession.fetch(gameSessionPDA);
    
    // Find the block with the most bets (as a heuristic)
    const blockAssignments = gameSession.blockAssignments;
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
      const estimatedFaction = blockAssignments[estimatedBlock];
      console.log(`   Estimated winning faction: ${estimatedFaction} (from block ${estimatedBlock + 1} with ${maxUsers} users)`);
      return estimatedFaction;
    }
    
    return 0; // Fallback to faction 0
  } catch (error) {
    console.error(`   Error estimating winning faction:`, error.message);
    return 0; // Fallback to faction 0
  }
}

/**
 * Execute end round transaction
 */
async function executeEndRound(revealedSeed, nextRoundCommit) {
  try {
    console.log(`\n🏁 Ending round...`);
    
    // Get current global state
    const globalState = await getGlobalGameState();
    const currentRoundId = globalState.currentRoundId.toNumber();
    
    const gameSessionPDA = deriveGameSessionPDA(currentRoundId);
    
    // Try to estimate winning faction from game session
    // Note: The actual winning faction is determined during end_round execution,
    // so this is just a best guess. If wrong, we'll retry with all factions.
    let winningFactionId = await estimateWinningFaction(currentRoundId);
    const winningFactionStatePDA = deriveFactionStatePDA(winningFactionId);
    
    console.log(`   Using estimated faction state for faction ${winningFactionId}`);
    
    const endRoundTx = await moonBaseProgram.methods
      .endRound(
        Array.from(revealedSeed),
        Array.from(nextRoundCommit)
      )
      .accounts({
        globalGameState: globalGameStatePDA,
        gameSession: gameSessionPDA,
        globalConfig: globalConfigPDA,
        dogeBtcMining: dogeBtcMiningPDA,
        winningFactionState: winningFactionStatePDA,
        solPrizePotVault: solPrizePotVaultPDA,
        dbtcEmissionVault: dbtcEmissionVaultPDA,
        slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .transaction();

    // Add compute unit limit instruction at the beginning
    endRoundTx.instructions.unshift(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1000000 })
    );

    const signature = await sendAndConfirmTransaction(
      connection,
      endRoundTx,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log(`✅ Round ${currentRoundId} ended: ${signature}`);
    return { success: true, signature };
  } catch (error) {
    console.error(`❌ Error ending round:`, error.message);
    if (error.logs) {
      console.error('Transaction logs:', error.logs);
    }
    
    // If error is about invalid faction, try all factions
    if (error.message.includes('InvalidFactionId') || error.message.includes('faction')) {
      console.log('   Invalid faction detected, trying all factions...');
      const globalState = await getGlobalGameState();
      const currentRoundId = globalState.currentRoundId.toNumber();
      const gameSessionPDA = deriveGameSessionPDA(currentRoundId);
      
      // Try each faction (0-11)
      for (let factionId = 0; factionId < 12; factionId++) {
        try {
          console.log(`   Trying faction ${factionId}...`);
          const factionStatePDA = deriveFactionStatePDA(factionId);
          
          const endRoundTx = await moonBaseProgram.methods
            .endRound(
              Array.from(revealedSeed),
              Array.from(nextRoundCommit)
            )
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

          endRoundTx.instructions.unshift(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1000000 })
          );

          const signature = await sendAndConfirmTransaction(
            connection,
            endRoundTx,
            [walletKeypair],
            { commitment: 'confirmed' }
          );

          console.log(`✅ Round ${currentRoundId} ended with faction ${factionId}: ${signature}`);
          return { success: true, signature };
        } catch (retryError) {
          // Continue to next faction
          if (retryError.message.includes('InvalidFactionId')) {
            continue;
          }
          // If it's a different error, break
          throw retryError;
        }
      }
      
      console.error('   Failed to find correct winning faction after trying all 12 factions');
    }
    
    return { success: false, error: error.message };
  }
}

/**
 * Main loop function
 */
async function runLoop() {
  console.log('\n🎮 Starting game loop...');
  console.log(`📡 Network: ${config.network.cluster}`);
  console.log(`🔗 RPC: ${config.network.rpc_url}`);
  console.log(`👛 Wallet: ${walletKeypair.publicKey.toString()}`);
  console.log(`⏰ Interval: 60 seconds\n`);
//   return;

  // Store seeds for commit-reveal scheme
  // currentRevealedSeed: seed for current round (to reveal when ending current round)
  // nextCommitHash: commit hash for next round (hash of seed for next round)
  // nextRoundSeed: seed for next round (to reveal when ending next round)
  let currentRevealedSeed = null;
  let nextCommitHash = null;
  let nextRoundSeed = null;

  let iteration = 0;

  while (true) {
    iteration++;
    console.log(`\n${'='.repeat(60)}`);
    console.log(`🔄 Iteration #${iteration} - ${new Date().toISOString()}`);
    console.log(`${'='.repeat(60)}`);

    try {
      // Get current global game state
      const globalState = await getGlobalGameState();
      const currentRoundId = globalState.currentRoundId;
      const roundEndTimestamp = globalState.roundEndTimestamp;
      const roundDurationSeconds = globalState.roundDurationSeconds;
      const currentTimestamp = Math.floor(Date.now() / 1000);
      
      console.log(`📊 Global Game State:`);
      console.log(`   Active: ${globalState.isActive ? '✅ Yes' : '❌ No'}`);
      console.log(`   Current Round ID: ${currentRoundId}`);
      console.log(`   Last Round ID: ${globalState.lastRoundId}`);
      console.log(`   Winning Faction (last round): ${globalState.winningFactionId !== null ? globalState.winningFactionId : 'N/A'}`);
      console.log(`   Total SOL Bets: ${globalState.totalSolBetsSOL} SOL (${globalState.totalSolBets} lamports)`);
      console.log(`   Total Passive Hashpower: ${globalState.totalGlobalPassiveHashpower}`);
      console.log(`   Round Duration: ${globalState.roundDurationMinutes} minutes (${roundDurationSeconds} seconds)`);
      console.log(`   Round End: ${globalState.roundEndDate || 'N/A'}`);
      console.log(`   Current Timestamp: ${new Date(currentTimestamp * 1000).toISOString()}`);
      console.log(`   Cranker Bots: ${globalState.crankerBotsCount} whitelisted`);
      if (globalState.crankerBotsCount > 0) {
        globalState.crankerBots.forEach((bot, idx) => {
          console.log(`     ${idx + 1}. ${bot}`);
        });
      }

      return;

      // Check if round has ended or needs to be started
      if (currentRoundId === 0 || currentTimestamp >= roundEndTimestamp) {
        // Round has ended or no round started yet - end current round first (if exists)
        if (currentRoundId > 0 && currentRevealedSeed) {
          console.log(`\n🏁 Round ${currentRoundId} has ended, ending round...`);
          
          // Generate seed and commit hash for round N+2 (after the next round)
          const seedForRoundNPlus2 = generateRandomSeed();
          const commitForRoundNPlus2 = hashSeed(seedForRoundNPlus2);
          
          // End current round: reveal seed for current round, commit hash for next round
          // currentRevealedSeed = seed for round N (to reveal)
          // nextCommitHash = commit hash for round N+1 (hash of nextRoundSeed)
          const endResult = await executeEndRound(currentRevealedSeed, nextCommitHash);
          if (!endResult.success) {
            console.log('⚠️  End round failed, will retry next iteration');
            await new Promise(resolve => setTimeout(resolve, 60000));
            continue;
          }
          
          // After ending round N:
          // - nextRoundSeed becomes the seed for round N+1 (to reveal when ending round N+1)
          // - commitForRoundNPlus2 becomes the commit for round N+2 (to use when starting round N+2)
          currentRevealedSeed = nextRoundSeed; // Seed for round N+1 (to reveal)
          nextRoundSeed = seedForRoundNPlus2; // Seed for round N+2 (to reveal later)
          nextCommitHash = commitForRoundNPlus2; // Commit for round N+2
          
          console.log(`🔐 Prepared: seed for round ${currentRoundId + 1} (to reveal), commit for round ${currentRoundId + 2}`);
        } else {
          // First round or no revealed seed - check if next_round_commit exists in global state
          if (!nextCommitHash) {
            // Use next_round_commit from global state if available
            if (globalState.nextRoundCommit && Array.isArray(globalState.nextRoundCommit) && globalState.nextRoundCommit.length === 32) {
              nextCommitHash = Buffer.from(globalState.nextRoundCommit);
              console.log(`🔐 Using next_round_commit from global state`);
              // We don't have the seed for this commit, so generate new seed for round 1
              const seedForRound1 = generateRandomSeed();
              currentRevealedSeed = seedForRound1;
              // Generate seed and commit for round 2
              const seedForRound2 = generateRandomSeed();
              nextRoundSeed = seedForRound2;
              nextCommitHash = hashSeed(seedForRound2);
              console.log(`🔐 Generated seed for round 1 and commit for round 2`);
            } else {
              // Generate initial seed and commit for round 1
              const seedForRound1 = generateRandomSeed();
              currentRevealedSeed = seedForRound1;
              nextCommitHash = hashSeed(seedForRound1);
              // Generate seed for round 2
              const seedForRound2 = generateRandomSeed();
              nextRoundSeed = seedForRound2;
              console.log(`🔐 Generated initial seed for round 1 and seed for round 2`);
            }
          }
        }
        
        // Start next round
        const nextRoundId = currentRoundId + 1;
        console.log(`\n🎮 Starting round ${nextRoundId}...`);
        
        const startResult = await executeStartRound(nextRoundId, nextCommitHash ? Array.from(nextCommitHash) : null);
        if (!startResult.success) {
          console.log('⚠️  Start round failed, will retry next iteration');
        } else {
          // After starting round N+1:
          // - We used nextCommitHash (commit for round N+1 = hash of nextRoundSeed)
          // - currentRevealedSeed is the seed for round N+1 (to reveal when ending round N+1)
          // - nextRoundSeed is already set (seed for round N+1, which we'll reveal)
          // - We need to generate seed and commit for round N+2
          const seedForRoundNPlus2 = generateRandomSeed();
          const commitForRoundNPlus2 = hashSeed(seedForRoundNPlus2);
          
          // Update: currentRevealedSeed stays as seed for round N+1 (to reveal)
          // nextRoundSeed becomes seed for round N+2 (to reveal when ending round N+2)
          // nextCommitHash becomes commit for round N+2
          nextRoundSeed = seedForRoundNPlus2;
          nextCommitHash = commitForRoundNPlus2;
          
          console.log(`🔐 Prepared: seed for round ${nextRoundId} (to reveal), commit for round ${nextRoundId + 1}`);
        }
      } else {
        // Round is still active
        const timeRemaining = roundEndTimestamp - currentTimestamp;
        console.log(`⏳ Round ${currentRoundId} is still active (${timeRemaining} seconds remaining)`);
        console.log(`   Waiting for round to end...`);
      }

    } catch (error) {
      console.error(`❌ Error in loop iteration:`, error.message);
    }

    // Wait 60 seconds before next iteration
    console.log(`\n⏳ Waiting 60 seconds before next iteration...`);
    await new Promise(resolve => setTimeout(resolve, 60000));
  }
}

// Handle graceful shutdown
process.on('SIGINT', () => {
  console.log('\n\n🛑 Received SIGINT, shutting down gracefully...');
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\n\n🛑 Received SIGTERM, shutting down gracefully...');
  process.exit(0);
});

// Start the loop
runLoop().catch(error => {
  console.error('❌ Fatal error:', error);
  process.exit(1);
});

