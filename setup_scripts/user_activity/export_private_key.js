#!/usr/bin/env node

/**
 * Export Private Key Script
 * 
 * Converts a Solana keypair JSON file to Base58 format for Phantom wallet import
 */

import { Keypair } from '@solana/web3.js';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import bs58 from 'bs58';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.join(__dirname, '..', '..');

// Get filename from command line arguments or use default
const args = process.argv.slice(2);
const filename = args.length > 0 ? args[0] : 'devnet-wallet-keypair';

// Determine file path
const keypairPath = filename === 'devnet-wallet-keypair'
  ? path.join(ROOT_DIR, `${filename}.json`)
  : path.join(__dirname, `${filename}.json`);

if (!fs.existsSync(keypairPath)) {
  console.error(`❌ Error: File not found: ${keypairPath}`);
  process.exit(1);
}

try {
  const keypairData = JSON.parse(fs.readFileSync(keypairPath, 'utf8'));
  const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));

  console.log('🔑 Wallet Address (Public Key):');
  console.log(`   ${keypair.publicKey.toString()}`);
  console.log('');
  console.log('🔐 Private Key (Base58 - for Phantom import):');
  const privateKeyBase58 = bs58.encode(keypair.secretKey);
  console.log(`   ${privateKeyBase58}`);
  console.log('');
  console.log('💡 To import into Phantom wallet:');
  console.log('   1. Open Phantom wallet extension');
  console.log('   2. Click on the menu (three lines)');
  console.log('   3. Go to Settings');
  console.log('   4. Click "Import Private Key"');
  console.log('   5. Paste the Base58 private key above');
  console.log('   6. Enter a password to secure your wallet');
  console.log('');
  console.log('⚠️  Keep this private key secure! Anyone with it can control your wallet.');
} catch (error) {
  console.error('❌ Error:', error.message);
  process.exit(1);
}

