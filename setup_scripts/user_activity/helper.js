#!/usr/bin/env node

/**
 * User Betting Helper Functions
 * 
 * Provides simple, one-line functions for all user betting operations.
 * Handles all complexity internally (PDA derivation, account setup, transactions).
 * 
 * Usage:
 *   import { initializePlayer, joinRound, joinRoundBatch, claimRewards, initAutominer, executeAutominerBet } from './helper.js';
 */

import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, Wallet } = pkg;
import { Connection, Keypair, PublicKey, SystemProgram, ComputeBudgetProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ============================================================================
// INITIALIZATION
// ============================================================================

let _connection = null;
let _program = null;
let _programId = null;
let _globalConfigPDA = null;
let _globalGameStatePDA = null;
let _solTreasuryPDA = null;
let _solPrizePotVaultPDA = null;
let _solRewardsVaultPDA = null;
let _dbtcEmissionVaultPDA = null;
let _config = null;

// Seeds
const GLOBAL_CONFIG_SEED = "global-config";
const GLOBAL_GAME_STATE_SEED = "global-game-state";
const GAME_SESSION_SEED = "game-session";
const PLAYER_DATA_SEED = "player";
const USER_GAME_BET_SEED = "user-bet";
const SOL_TREASURY_SEED = "sol-treasury";
const SOL_PRIZE_POT_VAULT_SEED = "sol-prize-pot";
const STAKER_SOL_REWARD_VAULT_SEED = "staker-sol-reward-vault";
const DBTC_EMISSION_VAULT_SEED = "dbtc-emission-vault";
const FACTION_STATE_SEED = "faction";
const AUTOMINER_SEED = "autominer";
const REFERRAL_REWARDS_SEED = "referral-rewards";

/**
 * Initialize the helper module (call once before using other functions)
 * @param {string} walletPath - Path to wallet keypair JSON file
 * @param {string} network - Network cluster (localnet, devnet, mainnet-beta)
 */
export async function init(walletPath, network = 'localnet') {
  try {
    // Load config
    const configPath = path.resolve(__dirname, '../config.json');
    _config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    
    // Override network if provided
    if (network) {
      _config.network.cluster = network;
    }
    
    const RPC_URL = _config.network.rpc_url;
    const COMMITMENT = _config.network.commitment;
    
    // Load deployment
    const deploymentPath = path.resolve(__dirname, '../deployments', `${_config.network.cluster}.json`);
    if (!fs.existsSync(deploymentPath)) {
      throw new Error(`Deployment file not found: ${deploymentPath}`);
    }
    const deployment = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    
    // Load IDL
    const idlPath = path.resolve(__dirname, '../../target/idl/moonbase.json');
    if (!fs.existsSync(idlPath)) {
      throw new Error(`IDL file not found: ${idlPath}`);
    }
    const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8'));
    
    // Load wallet
    const walletKeypair = Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
    );
    
    // Initialize connection and program
    _connection = new Connection(RPC_URL, COMMITMENT);
    const wallet = new Wallet(walletKeypair);
    const provider = new AnchorProvider(_connection, wallet, { commitment: COMMITMENT });
    _program = new Program(idl, provider);
    _programId = _program.programId;
    
    // Print wallet info
    const balance = await _connection.getBalance(walletKeypair.publicKey);
    const balanceSOL = (balance / LAMPORTS_PER_SOL).toFixed(4);
    console.log(`👛 Wallet Address: ${walletKeypair.publicKey.toString()}`);
    console.log(`💰 Wallet Balance: ${balanceSOL} SOL (${balance} lamports)`);
    
    // Derive PDAs
    [_globalConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_CONFIG_SEED)],
      _programId
    );
    
    [_globalGameStatePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_GAME_STATE_SEED)],
      _programId
    );
    
    [_solTreasuryPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(SOL_TREASURY_SEED)],
      _programId
    );
    
    [_solPrizePotVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(SOL_PRIZE_POT_VAULT_SEED)],
      _programId
    );
    
    [_solRewardsVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(STAKER_SOL_REWARD_VAULT_SEED)],
      _programId
    );
    
    [_dbtcEmissionVaultPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from(DBTC_EMISSION_VAULT_SEED)],
      _programId
    );
    
    console.log(`✅ Helper initialized for ${_config.network.cluster}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to initialize helper:`, error.message);
    throw error;
  }
}

/**
 * Get wallet keypair from file
 */
function getWallet(walletPath) {
  return Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
  );
}

/**
 * Derive PDA helper
 */
function derivePDA(seeds, programId) {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

/**
 * Get faction name from faction ID using config
 */
function getFactionName(factionId) {
  if (!_config || !_config.factions || factionId >= _config.factions.length) {
    throw new Error(`Invalid faction ID: ${factionId}. Must be 0-${(_config?.factions?.length || 12) - 1}`);
  }
  return _config.factions[factionId].name;
}

// ============================================================================
// USER BETTING FUNCTIONS
// ============================================================================

/**
 * Initialize a player account
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @param {number} factionId - Faction ID (0-11)
 * @param {string|null} referralCode - Optional referral code (public key string)
 * @returns {Promise<boolean>} Success status
 */
export async function initializePlayer(walletPath, factionId, referralCode = null) {
  try {
    const walletKeypair = getWallet(walletPath);
    const wallet = new Wallet(walletKeypair);
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const newPlayerRewardsPDA = derivePDA(
      [Buffer.from(REFERRAL_REWARDS_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const referrerRewardsPDA = referralCode
      ? derivePDA(
          [Buffer.from(REFERRAL_REWARDS_SEED), new PublicKey(referralCode).toBuffer()],
          _programId
        )
      : derivePDA(
          [Buffer.from(REFERRAL_REWARDS_SEED), SystemProgram.programId.toBuffer()],
          _programId
        );
    
    // Build transaction
    const tx = await _program.methods
      .initializePlayer(factionId, referralCode ? new PublicKey(referralCode) : null)
      .accounts({
        playerData: playerDataPDA,
        globalConfig: _globalConfigPDA,
        referrerRewards: referralCode ? referrerRewardsPDA : null,
        newPlayerRewards: newPlayerRewardsPDA,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Player initialized: ${tx}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to initialize player:`, error.message);
    return false;
  }
}

/**
 * Join a round with a single bet
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @param {number} amount - Bet amount in lamports
 * @param {Object} betType - Bet type object: { block: { blockId: number } } or { factionHighestLowest: { factionId: number, isHighest: boolean } }
 * @param {number|null} useTicket - Optional ticket type index (0-4), null to use SOL
 * @returns {Promise<boolean>} Success status
 */
export async function joinRound(walletPath, amount, betType, useTicket = null) {
  try {
    const walletKeypair = getWallet(walletPath);
    
    // Get current round ID
    const globalGameState = await _program.account.globalGameSate.fetch(_globalGameStatePDA);
    const currentRoundId = globalGameState.currentRoundId.toNumber();
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const roundIdBuffer = Buffer.allocUnsafe(8);
    roundIdBuffer.writeBigUInt64LE(BigInt(currentRoundId), 0);
    
    const gameSessionPDA = derivePDA(
      [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
      _programId
    );
    
    const userGameBetPDA = derivePDA(
      [Buffer.from(USER_GAME_BET_SEED), walletKeypair.publicKey.toBuffer(), roundIdBuffer],
      _programId
    );
    
    // Get faction state PDA (need to determine from bet type)
    let factionStatePDA;
    if (betType.block) {
      // Need to fetch game session to get faction for this block
      const gameSession = await _program.account.gameSession.fetch(gameSessionPDA);
      console.log(gameSession)


      const blockIndex = betType.block.blockId;
      console.log(`blockIndex ${blockIndex}`)
      const factionId = gameSession.blockAssignments[blockIndex];
      console.log(`factionId ${factionId}`)
      const factionName = getFactionName(factionId);
      console.log(`factionName ${factionName}`)

      factionStatePDA = derivePDA(
        [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
        _programId
      );
    } else if (betType.factionHighestLowest) {
      const factionName = getFactionName(betType.factionHighestLowest.factionId);
      factionStatePDA = derivePDA(
        [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
        _programId
      );
    } else {
      throw new Error('Invalid bet type');
    }
    
    // Build transaction
    const tx = await _program.methods
      .joinRound(new BN(amount), betType, useTicket !== null ? useTicket : null)
      .accounts({
        globalGameState: _globalGameStatePDA,
        globalConfig: _globalConfigPDA,
        playerData: playerDataPDA,
        factionState: factionStatePDA,
        gameSession: gameSessionPDA,
        userGameBet: userGameBetPDA,
        solTreasury: _solTreasuryPDA,
        solPrizePotVault: _solPrizePotVaultPDA,
        solRewardsVault: _solRewardsVaultPDA,
        userWallet: walletKeypair.publicKey,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Joined round: ${tx}`);
    return true;
  } catch (error) {
    console.error(error)
    console.error(`❌ Failed to join round:`, error.message);
    return false;
  }
}

/**
 * Join a round with multiple bets (batch)
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @param {number} amountPerBet - Bet amount per bet in lamports
 * @param {Array<Object>} betTypes - Array of bet type objects
 * @param {number|null} useTicket - Optional ticket type index (0-4), null to use SOL
 * @returns {Promise<boolean>} Success status
 */
export async function joinRoundBatch(walletPath, amountPerBet, betTypes, useTicket = null) {
  try {
    const walletKeypair = getWallet(walletPath);
    
    // Get current round ID
    const globalGameState = await _program.account.globalGameSate.fetch(_globalGameStatePDA);
    const currentRoundId = globalGameState.currentRoundId.toNumber();
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const roundIdBuffer = Buffer.allocUnsafe(8);
    roundIdBuffer.writeBigUInt64LE(BigInt(currentRoundId), 0);
    
    const gameSessionPDA = derivePDA(
      [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
      _programId
    );
    
    const userGameBetPDA = derivePDA(
      [Buffer.from(USER_GAME_BET_SEED), walletKeypair.publicKey.toBuffer(), roundIdBuffer],
      _programId
    );
    
    // Get faction state PDA (all bets must be for same faction in batch)
    const gameSession = await _program.account.gameSession.fetch(gameSessionPDA);
    let factionId;
    if (betTypes[0].block) {
      const blockIndex = betTypes[0].block.blockId - 1;
      factionId = gameSession.blockAssignments[blockIndex];
    } else if (betTypes[0].factionHighestLowest) {
      factionId = betTypes[0].factionHighestLowest.factionId;
    } else if (betTypes[0].factionBoth) {
      factionId = betTypes[0].factionBoth.factionId;
    } else {
      throw new Error('Invalid bet type');
    }
    
    const factionName = getFactionName(factionId);
    const factionStatePDA = derivePDA(
      [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
      _programId
    );
    
    // Build transaction
    const tx = await _program.methods
      .joinRoundBatch(betTypes, new BN(amountPerBet), useTicket !== null ? useTicket : null)
      .accounts({
        globalGameState: _globalGameStatePDA,
        globalConfig: _globalConfigPDA,
        playerData: playerDataPDA,
        factionState: factionStatePDA,
        gameSession: gameSessionPDA,
        userGameBet: userGameBetPDA,
        solTreasury: _solTreasuryPDA,
        solPrizePotVault: _solPrizePotVaultPDA,
        solRewardsVault: _solRewardsVaultPDA,
        userWallet: walletKeypair.publicKey,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Joined round (batch): ${tx}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to join round (batch):`, error.message);
    return false;
  }
}

/**
 * Claim rewards for a completed round
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @param {number} roundId - Round ID to claim rewards for
 * @returns {Promise<boolean>} Success status
 */
export async function claimRewards(walletPath, roundId) {
  try {
    const walletKeypair = getWallet(walletPath);
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const roundIdBuffer = Buffer.allocUnsafe(8);
    roundIdBuffer.writeBigUInt64LE(BigInt(roundId), 0);
    
    const gameSessionPDA = derivePDA(
      [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
      _programId
    );
    
    const userGameBetPDA = derivePDA(
      [Buffer.from(USER_GAME_BET_SEED), walletKeypair.publicKey.toBuffer(), roundIdBuffer],
      _programId
    );
    
    // Get faction state PDA
    const gameSession = await _program.account.gameSession.fetch(gameSessionPDA);
    const winningFactionId = gameSession.winningFactionId;
    const factionName = getFactionName(winningFactionId);
    const factionStatePDA = derivePDA(
      [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
      _programId
    );
    
    // Build transaction
    const tx = await _program.methods
      .claimRewards()
      .accounts({
        globalGameState: _globalGameStatePDA,
        globalConfig: _globalConfigPDA,
        playerData: playerDataPDA,
        gameSession: gameSessionPDA,
        userGameBet: userGameBetPDA,
        factionState: factionStatePDA,
        solPrizePotVault: _solPrizePotVaultPDA,
        dbtcEmissionVault: _dbtcEmissionVaultPDA,
        userWallet: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Rewards claimed: ${tx}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to claim rewards:`, error.message);
    return false;
  }
}

/**
 * Initialize autominer vault
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @param {Array<Object>} betTypes - Array of bet type objects
 * @param {number} betAmountPerBet - Bet amount per bet in lamports
 * @param {number} numRounds - Number of rounds to bet for
 * @returns {Promise<boolean>} Success status
 */
export async function initAutominer(walletPath, betTypes, betAmountPerBet, numRounds) {
  try {
    const walletKeypair = getWallet(walletPath);
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const autominerVaultPDA = derivePDA(
      [Buffer.from(AUTOMINER_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    // Build transaction
    const tx = await _program.methods
      .initAutominer(betTypes, new BN(betAmountPerBet), numRounds)
      .accounts({
        autominerVault: autominerVaultPDA,
        playerData: playerDataPDA,
        globalConfig: _globalConfigPDA,
        userWallet: walletKeypair.publicKey,
        authority: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Autominer initialized: ${tx}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to initialize autominer:`, error.message);
    return false;
  }
}

/**
 * Execute autominer bet (keeper instruction)
 * @param {string} walletPath - Path to user's wallet keypair JSON file
 * @returns {Promise<boolean>} Success status
 */
export async function executeAutominerBet(walletPath) {
  try {
    const walletKeypair = getWallet(walletPath);
    
    // Get current round ID
    const globalGameState = await _program.account.globalGameSate.fetch(_globalGameStatePDA);
    const currentRoundId = globalGameState.currentRoundId.toNumber();
    
    // Derive PDAs
    const playerDataPDA = derivePDA(
      [Buffer.from(PLAYER_DATA_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const autominerVaultPDA = derivePDA(
      [Buffer.from(AUTOMINER_SEED), walletKeypair.publicKey.toBuffer()],
      _programId
    );
    
    const roundIdBuffer = Buffer.allocUnsafe(8);
    roundIdBuffer.writeBigUInt64LE(BigInt(currentRoundId), 0);
    
    const gameSessionPDA = derivePDA(
      [Buffer.from(GAME_SESSION_SEED), roundIdBuffer],
      _programId
    );
    
    const userGameBetPDA = derivePDA(
      [Buffer.from(USER_GAME_BET_SEED), walletKeypair.publicKey.toBuffer(), roundIdBuffer],
      _programId
    );
    
    // Get autominer vault to determine faction
    const autominerVault = await _program.account.autominerVault.fetch(autominerVaultPDA);
    const firstBetType = autominerVault.betTypes[0];
    
    let factionId;
    if (firstBetType.block) {
      const gameSession = await _program.account.gameSession.fetch(gameSessionPDA);
      const blockIndex = firstBetType.block.blockId - 1;
      factionId = gameSession.blockAssignments[blockIndex];
    } else if (firstBetType.factionHighestLowest) {
      factionId = firstBetType.factionHighestLowest.factionId;
    } else {
      throw new Error('Invalid bet type in autominer');
    }
    
    const factionName = getFactionName(factionId);
    const factionStatePDA = derivePDA(
      [Buffer.from(FACTION_STATE_SEED), Buffer.from(factionName)],
      _programId
    );
    
    // Build transaction
    const tx = await _program.methods
      .executeAutominerBet()
      .accounts({
        autominerVault: autominerVaultPDA,
        globalGameState: _globalGameStatePDA,
        globalConfig: _globalConfigPDA,
        playerData: playerDataPDA,
        factionState: factionStatePDA,
        gameSession: gameSessionPDA,
        userGameBet: userGameBetPDA,
        solTreasury: _solTreasuryPDA,
        solPrizePotVault: _solPrizePotVaultPDA,
        solRewardsVault: _solRewardsVaultPDA,
        owner: walletKeypair.publicKey,
        caller: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log(`✅ Autominer bet executed: ${tx}`);
    return true;
  } catch (error) {
    console.error(`❌ Failed to execute autominer bet:`, error.message);
    return false;
  }
}

