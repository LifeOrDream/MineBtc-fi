#!/usr/bin/env node

/**
 * User Betting Test Script
 * 
 * Simple test script demonstrating how to use the betting helper functions.
 * 
 * Usage:
 *   node bets.js
 * 
 * Make sure to:
 * 1. Create wallets using: node create_wallet.js <name>
 * 2. Fund wallets with SOL
 * 3. Update wallet paths below
 */

import { 
  init,
  initializePlayer,
  joinRound,
  joinRoundBatch,
  claimRewards,
  initAutominer,
  executeAutominerBet
} from './helper.js';

// ============================================================================
// CONFIGURATION
// ============================================================================

// Update these paths to your wallet files
const USER_WALLET = './user_activity/user1.json';
const NETWORK = 'localnet'; // 'localnet', 'devnet', or 'mainnet-beta'

// ============================================================================
// TEST FUNCTIONS
// ============================================================================

async function testInitializePlayer() {
  console.log('\n📝 Testing initializePlayer...');
  const success = await initializePlayer(USER_WALLET, 0, null); // Faction 0, no referral
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testJoinRoundBlock() {
  console.log('\n📝 Testing joinRound (block bet)...');
  const betType = { block: { blockId: 5 } }; // Bet on block 5
  const amount = 100000000; // 0.1 SOL
  const success = await joinRound(USER_WALLET, amount, betType, null);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testJoinRoundFaction() {
  console.log('\n📝 Testing joinRound (faction bet)...');
  const betType = { 
    factionHighestLowest: { 
      factionId: 0, 
      isHighest: true 
    } 
  }; // Bet on faction 0, highest block
  const amount = 100000000; // 0.1 SOL
  const success = await joinRound(USER_WALLET, amount, betType, null);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testJoinRoundBatch() {
  console.log('\n📝 Testing joinRoundBatch...');
  const betTypes = [
    { block: { blockId: 1 } },
    { block: { blockId: 5 } },
    { block: { blockId: 10 } },
    { block: { blockId: 15 } }
  ]; // Bet on multiple blocks
  const amountPerBet = 50000000; // 0.05 SOL per bet
  const success = await joinRoundBatch(USER_WALLET, amountPerBet, betTypes, null);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testJoinRoundBatchFactions() {
  console.log('\n📝 Testing joinRoundBatch (factions)...');
  const betTypes = [
    { factionHighestLowest: { factionId: 0, isHighest: true } },
    { factionHighestLowest: { factionId: 0, isHighest: false } },
    { factionBoth: { factionId: 1 } }
  ]; // Bet on multiple faction options
  const amountPerBet = 50000000; // 0.05 SOL per bet
  const success = await joinRoundBatch(USER_WALLET, amountPerBet, betTypes, null);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testClaimRewards() {
  console.log('\n📝 Testing claimRewards...');
  const roundId = 1; // Claim rewards for round 1
  const success = await claimRewards(USER_WALLET, roundId);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testInitAutominer() {
  console.log('\n📝 Testing initAutominer...');
  const betTypes = [
    { block: { blockId: 5 } },
    { block: { blockId: 10 } }
  ]; // Autominer will bet on blocks 5 and 10 each round
  const betAmountPerBet = 100000000; // 0.1 SOL per bet
  const numRounds = 10; // Bet for 10 rounds
  const success = await initAutominer(USER_WALLET, betTypes, betAmountPerBet, numRounds);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testInitAutominerRandom() {
  console.log('\n📝 Testing initAutominer (random blocks)...');
  const betTypes = [
    { randomBlock: {} },
    { randomBlock: {} },
    { randomBlock: {} }
  ]; // Autominer will bet on 3 random blocks each round
  const betAmountPerBet = 100000000; // 0.1 SOL per bet
  const numRounds = 5; // Bet for 5 rounds
  const success = await initAutominer(USER_WALLET, betTypes, betAmountPerBet, numRounds);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

async function testExecuteAutominerBet() {
  console.log('\n📝 Testing executeAutominerBet...');
  const success = await executeAutominerBet(USER_WALLET);
  console.log(`Result: ${success ? '✅ Success' : '❌ Failed'}`);
  return success;
}

// ============================================================================
// MAIN
// ============================================================================

async function main() {
  console.log('🎮 User Betting Test Script');
  console.log(`📁 Wallet: ${USER_WALLET}`);
  console.log(`🌐 Network: ${NETWORK}\n`);
  
  try {
    // Initialize helper (this will print wallet address and balance)
    console.log('🔧 Initializing helper...');
    await init(USER_WALLET, NETWORK);
    
    // Run tests (uncomment the ones you want to test)
    
    // await testInitializePlayer();
    // await testJoinRoundBlock();
    // await testJoinRoundFaction();

    await testJoinRoundBatch();
    // await testJoinRoundBatchFactions();

    // await testClaimRewards();
    // await testInitAutominer();
    // await testInitAutominerRandom();
    // await testExecuteAutominerBet();
    
    console.log('\n✅ All tests completed!');
  } catch (error) {
    console.error('\n❌ Error:', error.message);
    process.exit(1);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

