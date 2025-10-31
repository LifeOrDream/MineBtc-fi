#!/usr/bin/env node

import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, sendAndConfirmTransaction, SystemProgram, Transaction } from '@solana/web3.js';
import fs from 'fs';
import path from 'path';

// Load config
const configPath = path.join(process.cwd(), 'config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;

// Only allow on localnet
if (CLUSTER !== 'localnet') {
  console.error('❌ This script only works on localnet for safety reasons');
  console.error(`   Current cluster: ${CLUSTER}`);
  process.exit(1);
}

// Load wallet keypair (sender)
const walletPath = path.join(process.cwd(), config.deployment.paths.deployer_key);
const walletKeypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
);

const connection = new Connection(RPC_URL, 'confirmed');

async function sendSol(recipientAddress, amountInSol) {
  try {
    console.log('\n🚀 Sending SOL on Localnet...');
    console.log(`📤 From: ${walletKeypair.publicKey.toString()}`);
    console.log(`📥 To: ${recipientAddress}`);
    console.log(`💰 Amount: ${amountInSol} SOL`);

    // Check sender balance
    const senderBalance = await connection.getBalance(walletKeypair.publicKey);
    console.log(`\n💼 Sender balance: ${senderBalance / LAMPORTS_PER_SOL} SOL`);

    if (senderBalance < amountInSol * LAMPORTS_PER_SOL) {
      throw new Error('Insufficient balance');
    }

    // Create transaction
    const recipient = new PublicKey(recipientAddress);
    const lamports = amountInSol * LAMPORTS_PER_SOL;

    const transaction = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: walletKeypair.publicKey,
        toPubkey: recipient,
        lamports: lamports,
      })
    );

    // Send transaction
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [walletKeypair],
      { commitment: 'confirmed' }
    );

    console.log('\n✅ Transfer successful!');
    console.log(`🔗 Signature: ${signature}`);

    // Check new balances
    const newSenderBalance = await connection.getBalance(walletKeypair.publicKey);
    const recipientBalance = await connection.getBalance(recipient);

    console.log(`\n💼 New sender balance: ${newSenderBalance / LAMPORTS_PER_SOL} SOL`);
    console.log(`💼 Recipient balance: ${recipientBalance / LAMPORTS_PER_SOL} SOL`);

    return {
      success: true,
      signature,
      newSenderBalance: newSenderBalance / LAMPORTS_PER_SOL,
      recipientBalance: recipientBalance / LAMPORTS_PER_SOL
    };

  } catch (error) {
    console.error('\n❌ Error sending SOL:', error);
    return {
      success: false,
      error: error.toString()
    };
  }
}

// Main function
async function main() {
  console.log('🌐 Network:', CLUSTER);
  console.log('🔗 RPC URL:', RPC_URL);

  // Get command line arguments
  const args = process.argv.slice(2);

  if (args.length < 2) {
    console.log('\n📖 Usage: node send_sol_localnet.js <recipient_address> <amount_in_sol>');
    console.log('\n📝 Example: node send_sol_localnet.js 7xZn...abc 10');
    process.exit(1);
  }

  const recipientAddress = args[0];
  const amountInSol = parseFloat(args[1]);

  if (isNaN(amountInSol) || amountInSol <= 0) {
    console.error('❌ Invalid amount. Must be a positive number.');
    process.exit(1);
  }

  await sendSol(recipientAddress, amountInSol);
}

main().catch(console.error);

