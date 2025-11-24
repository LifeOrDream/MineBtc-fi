#!/usr/bin/env node

import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, sendAndConfirmTransaction, SystemProgram, Transaction } from '@solana/web3.js';
import { getOrCreateAssociatedTokenAccount, createTransferCheckedInstruction, getAccount, NATIVE_MINT, createSyncNativeInstruction, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import fs from 'fs';
import path from 'path';

// Load config
const configPath = path.join(process.cwd(), 'config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;

// // Only allow on localnet
// if (CLUSTER !== 'localnet') {
//   console.error('❌ This script only works on localnet for safety reasons');
//   console.error(`   Current cluster: ${CLUSTER}`);
//   process.exit(1);
// }

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

async function sendWSOL(recipientAddress, amountInSol) {
  try {
    console.log('\n🚀 Sending WSOL on Localnet...');
    console.log(`📤 From: ${walletKeypair.publicKey.toString()}`);
    console.log(`📥 To: ${recipientAddress}`);
    console.log(`💰 Amount: ${amountInSol} WSOL`);

    const recipient = new PublicKey(recipientAddress);
    const lamports = BigInt(Math.floor(amountInSol * LAMPORTS_PER_SOL));

    // Check sender SOL balance
    const senderBalance = await connection.getBalance(walletKeypair.publicKey);
    console.log(`\n💼 Sender SOL balance: ${senderBalance / LAMPORTS_PER_SOL} SOL`);

    if (senderBalance < amountInSol * LAMPORTS_PER_SOL) {
      throw new Error('Insufficient SOL balance');
    }

    // Get or create sender's WSOL token account
    console.log('📦 Setting up WSOL token accounts...');
    const senderAta = await getOrCreateAssociatedTokenAccount(
      connection,
      walletKeypair,
      NATIVE_MINT,
      walletKeypair.publicKey
    );
    console.log(`  Sender WSOL Account: ${senderAta.address.toBase58()}`);

    // Get or create recipient's WSOL token account
    const recipientAta = await getOrCreateAssociatedTokenAccount(
      connection,
      walletKeypair,
      NATIVE_MINT,
      recipient
    );
    console.log(`  Recipient WSOL Account: ${recipientAta.address.toBase58()}`);

    // Check sender WSOL balance
    const senderWSOLBalance = await getAccount(connection, senderAta.address);
    const senderWSOLBalanceSOL = Number(senderWSOLBalance.amount) / LAMPORTS_PER_SOL;
    console.log(`💼 Sender WSOL balance: ${senderWSOLBalanceSOL} WSOL`);

    // Check if we need to wrap more SOL into WSOL
    if (senderWSOLBalance.amount < lamports) {
      const neededWSOL = amountInSol - senderWSOLBalanceSOL;
      const neededLamports = BigInt(Math.ceil(neededWSOL * LAMPORTS_PER_SOL));
      
      console.log(`\n⚠️  Insufficient WSOL balance. Need ${neededWSOL.toFixed(9)} more WSOL.`);
      console.log(`🔄 Wrapping ${neededWSOL.toFixed(9)} SOL into WSOL...`);

      // Check if we have enough native SOL
      const currentSOLBalance = await connection.getBalance(walletKeypair.publicKey);
      const neededSOL = Number(neededLamports) / LAMPORTS_PER_SOL;
      
      // Add a small buffer for transaction fees (0.01 SOL)
      const totalNeededSOL = neededSOL + 0.01;
      
      if (currentSOLBalance < totalNeededSOL * LAMPORTS_PER_SOL) {
        throw new Error(
          `Insufficient SOL balance. Need ${totalNeededSOL.toFixed(9)} SOL to wrap, ` +
          `but only have ${(currentSOLBalance / LAMPORTS_PER_SOL).toFixed(9)} SOL.`
        );
      }

      // Wrap SOL into WSOL in a single transaction
      console.log(`  Sending ${neededSOL.toFixed(9)} SOL to WSOL account...`);
      const wrapTransaction = new Transaction().add(
        // Step 1: Transfer SOL to WSOL token account
        SystemProgram.transfer({
          fromPubkey: walletKeypair.publicKey,
          toPubkey: senderAta.address,
          lamports: Number(neededLamports),
        }),
        // Step 2: Sync native account to update token balance
        createSyncNativeInstruction(
          senderAta.address,
          TOKEN_PROGRAM_ID
        )
      );

      const wrapSignature = await sendAndConfirmTransaction(
        connection,
        wrapTransaction,
        [walletKeypair],
        { commitment: 'confirmed' }
      );
      console.log(`  ✅ Wrapped and synced: ${wrapSignature}`);

      // Verify the new WSOL balance
      const updatedWSOLBalance = await getAccount(connection, senderAta.address);
      const updatedWSOLBalanceSOL = Number(updatedWSOLBalance.amount) / LAMPORTS_PER_SOL;
      console.log(`  💼 Updated WSOL balance: ${updatedWSOLBalanceSOL} WSOL\n`);

      if (updatedWSOLBalance.amount < lamports) {
        throw new Error(
          `Failed to wrap enough WSOL. Current balance: ${updatedWSOLBalanceSOL} WSOL, ` +
          `needed: ${amountInSol} WSOL`
        );
      }
    }

    // Create transfer instruction
    const transferInstruction = createTransferCheckedInstruction(
      senderAta.address,
      NATIVE_MINT,
      recipientAta.address,
      walletKeypair.publicKey,
      lamports,
      9 // WSOL decimals
    );

    const transaction = new Transaction().add(transferInstruction);

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
    const newSenderWSOL = await getAccount(connection, senderAta.address);
    const newRecipientWSOL = await getAccount(connection, recipientAta.address);

    console.log(`\n💼 New sender WSOL balance: ${Number(newSenderWSOL.amount) / LAMPORTS_PER_SOL} WSOL`);
    console.log(`💼 New recipient WSOL balance: ${Number(newRecipientWSOL.amount) / LAMPORTS_PER_SOL} WSOL`);

    return {
      success: true,
      signature,
      newSenderBalance: Number(newSenderWSOL.amount) / LAMPORTS_PER_SOL,
      recipientBalance: Number(newRecipientWSOL.amount) / LAMPORTS_PER_SOL
    };

  } catch (error) {
    console.error('\n❌ Error sending WSOL:', error);
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

  // Check for -wsol flag
  const isWSOL = args.includes('-wsol');
  const filteredArgs = args.filter(arg => arg !== '-wsol');

  if (filteredArgs.length < 2) {
    console.log('\n📖 Usage:');
    console.log('  Native SOL: node send_sol_localnet.js <recipient_address> <amount_in_sol>');
    console.log('  WSOL:       node send_sol_localnet.js -wsol <recipient_address> <amount_in_sol>');
    console.log('\n📝 Examples:');
    console.log('  node send_sol_localnet.js 7xZn...abc 10');
    console.log('  node send_sol_localnet.js -wsol 7xZn...abc 10');
    process.exit(1);
  }

  const recipientAddress = filteredArgs[0];
  const amountInSol = parseFloat(filteredArgs[1]);

  if (isNaN(amountInSol) || amountInSol <= 0) {
    console.error('❌ Invalid amount. Must be a positive number.');
    process.exit(1);
  }

  if (isWSOL) {
    await sendWSOL(recipientAddress, amountInSol);
  } else {
    await sendSol(recipientAddress, amountInSol);
  }
}

main().catch(console.error);

