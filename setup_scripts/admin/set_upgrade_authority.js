#!/usr/bin/env node

/**
 * Set upgrade authority for a Solana program
 * This script uses web3.js directly to avoid CLI issues with multisig addresses
 */

import web3 from '@solana/web3.js';
const {
  Connection, 
  Keypair, 
  PublicKey, 
  Transaction, 
  sendAndConfirmTransaction,
  SystemProgram,
} = web3;

// BPF Loader Upgradeable Program ID
const BPF_LOADER_UPGRADEABLE_PROGRAM_ID = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load config
const configPath = path.resolve(__dirname, '../config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

async function main() {
  const args = process.argv.slice(2);
  
  if (args.length < 2) {
    console.log('Usage: node set_upgrade_authority.js <program_id> <new_authority_address>');
    console.log('Example: node set_upgrade_authority.js Hw9uxvtmQdS57N6aNwJA5iqjSqzhRDdopCHgm8EPwkqx 2Xze8BhdWV3GoJUyzpQPF7d1N2KUCS1TCkdVECfkDTcd');
    process.exit(1);
  }

  const programId = new PublicKey(args[0]);
  const newAuthorityAddress = new PublicKey(args[1]);

  // Load current upgrade authority keypair
  const keypairPath = path.resolve(__dirname, '../../mainnet-wallet-keypair.json');
  if (!fs.existsSync(keypairPath)) {
    console.error(`❌ Keypair not found at: ${keypairPath}`);
    process.exit(1);
  }

  const currentAuthorityKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, 'utf8')))
  );

  console.log('🔍 Setting upgrade authority...');
  console.log(`   Program ID: ${programId.toBase58()}`);
  console.log(`   Current Authority: ${currentAuthorityKeypair.publicKey.toBase58()}`);
  console.log(`   New Authority: ${newAuthorityAddress.toBase58()}`);
  console.log(`   RPC URL: ${RPC_URL}`);

  const connection = new Connection(RPC_URL, COMMITMENT);

  try {
    // Get program account info
    const programInfo = await connection.getAccountInfo(programId);
    if (!programInfo) {
      throw new Error('Program not found');
    }

    // Get program data account (upgradeable programs have a separate data account)
    const [programDataAddress] = PublicKey.findProgramAddressSync(
      [programId.toBuffer()],
      BPF_LOADER_UPGRADEABLE_PROGRAM_ID
    );

    console.log(`   Program Data Address: ${programDataAddress.toBase58()}`);

    // Get current program data account to verify it exists
    const programDataInfo = await connection.getAccountInfo(programDataAddress);
    if (!programDataInfo) {
      throw new Error('Program data account not found');
    }

    // Create the SetAuthority instruction
    // BPF Loader Upgradeable instruction format:
    // - 4 bytes: instruction type (little-endian u32) - SetAuthority = 4
    // - 1 byte: Option discriminator (1 = Some, 0 = None)
    // - 32 bytes: new authority pubkey (if Some)
    const instructionData = Buffer.alloc(4 + 1 + 32);
    instructionData.writeUInt32LE(4, 0); // SetAuthority instruction = 4
    instructionData.writeUInt8(1, 4); // Some(new_authority)
    newAuthorityAddress.toBuffer().copy(instructionData, 5); // Copy pubkey bytes

    // Create transaction
    // Accounts for SetAuthority:
    // 0. [writable] ProgramData account
    // 1. [signer] Current authority
    // 2. (optional) New authority - NOT required to sign when transferring to multisig
    const transaction = new Transaction().add({
      keys: [
        { pubkey: programDataAddress, isSigner: false, isWritable: true },
        { pubkey: currentAuthorityKeypair.publicKey, isSigner: true, isWritable: false },
      ],
      programId: BPF_LOADER_UPGRADEABLE_PROGRAM_ID,
      data: instructionData,
    });

    // Send transaction
    console.log('\n📡 Sending transaction...');
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [currentAuthorityKeypair],
      { commitment: 'confirmed' }
    );

    console.log('\n✅ Upgrade authority set successfully!');
    console.log(`   Transaction: ${signature}`);
    console.log(`   Explorer: https://explorer.solana.com/tx/${signature}?cluster=mainnet`);

    // Verify the change by checking program info
    console.log('\n🔍 Verifying update...');
    console.log('   Run: solana program show', programId.toBase58(), 'to verify the new authority');
    
  } catch (error) {
    console.error('\n❌ Error setting upgrade authority:', error.message);
    if (error.logs) {
      console.error('Transaction logs:');
      error.logs.forEach(log => console.error(`  ${log}`));
    }
    process.exit(1);
  }
}

main().catch(console.error);

