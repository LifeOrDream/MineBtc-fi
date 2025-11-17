#!/usr/bin/env node

/**
 * Create Wallet Script
 * 
 * Generates a random Solana keypair and saves it to a JSON file.
 * 
 * Usage:
 *   node create_wallet.js <filename>
 * 
 * Example:
 *   node create_wallet.js game_bot
 *   This will create a file named game_bot.json in the user_activity directory
 * 
 * The keypair will be saved as an array of numbers (secret key format)
 * compatible with Solana's Keypair.fromSecretKey() method.
 */

import { Keypair } from '@solana/web3.js';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Get filename from command line arguments
const args = process.argv.slice(2);

if (args.length === 0) {
  console.error('❌ Error: Please provide a filename for the wallet');
  console.log('\nUsage:');
  console.log('  node create_wallet.js <filename>');
  console.log('\nExample:');
  console.log('  node create_wallet.js game_bot');
  console.log('  This will create game_bot.json in the user_activity directory');
  process.exit(1);
}

const filename = args[0];

// Validate filename (basic validation)
if (!/^[a-zA-Z0-9_-]+$/.test(filename)) {
  console.error('❌ Error: Filename can only contain letters, numbers, underscores, and hyphens');
  process.exit(1);
}

// Generate new keypair
console.log('🔐 Generating new Solana keypair...');
const keypair = Keypair.generate();

// Convert keypair secret key to array format (for JSON storage)
const secretKeyArray = Array.from(keypair.secretKey);

// Determine output path (in user_activity directory)
const outputPath = path.join(__dirname, `${filename}.json`);

// Check if file already exists
if (fs.existsSync(outputPath)) {
  console.error(`❌ Error: File ${filename}.json already exists at:`);
  console.error(`   ${outputPath}`);
  console.log('\nTo overwrite, delete the existing file first or use a different filename.');
  process.exit(1);
}

// Write keypair to file
try {
  fs.writeFileSync(outputPath, JSON.stringify(secretKeyArray, null, 2), 'utf8');
  
  console.log('✅ Wallet created successfully!');
  console.log(`\n📁 File: ${outputPath}`);
  console.log(`🔑 Public Key: ${keypair.publicKey.toString()}`);
  console.log(`\n💡 To load this keypair in your scripts:`);
  console.log(`   const keypair = Keypair.fromSecretKey(`);
  console.log(`     new Uint8Array(JSON.parse(fs.readFileSync('${path.relative(process.cwd(), outputPath)}', 'utf8')))`);
  console.log(`   );`);
  console.log(`\n⚠️  Keep this file secure! Anyone with access to it can control the wallet.`);
} catch (error) {
  console.error('❌ Error writing wallet file:', error.message);
  process.exit(1);
}

