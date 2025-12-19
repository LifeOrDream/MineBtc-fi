#!/usr/bin/env node

/**
 * Set Token-2022 authorities (TransferFeeConfig, WithheldWithdraw) to a new address
 * Usage: node set_token_authority.js <mint_address> <new_authority> [authority_type]
 * 
 * Authority types:
 * - transfer_fee (default): TransferFeeConfig authority
 * - withheld_withdraw: WithheldWithdraw authority
 */

import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import { setAuthority, AuthorityType, TOKEN_2022_PROGRAM_ID, getMint } from '@solana/spl-token';
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
const CLUSTER = config.network.cluster;

async function main() {
  const args = process.argv.slice(2);
  
  if (args.length < 2) {
    console.log('Usage: node set_token_authority.js <mint_address> <new_authority> [authority_type]');
    console.log('');
    console.log('Authority types:');
    console.log('  transfer_fee (default) - TransferFeeConfig authority');
    console.log('  withheld_withdraw      - WithheldWithdraw authority');
    console.log('');
    console.log('Example:');
    console.log('  node set_token_authority.js BwMCF5LSHPvrR8pLVvcsa4k1AMg4VWVnMWUiNEXMtLkE 2Xze8BhdWV3GoJUyzpQPF7d1N2KUCS1TCkdVECfkDTcd transfer_fee');
    process.exit(1);
  }

  const mintAddress = new PublicKey(args[0]);
  const newAuthorityAddress = new PublicKey(args[1]);
  const authorityTypeArg = args[2] || 'transfer_fee';

  // Map authority type string to enum
  let authorityType;
  let authorityTypeName;
  switch (authorityTypeArg.toLowerCase()) {
    case 'transfer_fee':
    case 'transferfee':
    case 'transferfeeconfig':
      authorityType = AuthorityType.TransferFeeConfig;
      authorityTypeName = 'TransferFeeConfig';
      break;
    case 'withheld_withdraw':
    case 'withheldwithdraw':
    case 'withdrawwithheld':
      authorityType = AuthorityType.WithheldWithdraw;
      authorityTypeName = 'WithheldWithdraw';
      break;
    default:
      console.error(`❌ Unknown authority type: ${authorityTypeArg}`);
      console.error('Valid types: transfer_fee, withheld_withdraw');
      process.exit(1);
  }

  // Load current authority keypair
  const keypairPath = path.resolve(__dirname, '../../mainnet-wallet-keypair.json');
  if (!fs.existsSync(keypairPath)) {
    console.error(`❌ Keypair not found at: ${keypairPath}`);
    process.exit(1);
  }

  const currentAuthorityKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, 'utf8')))
  );

  console.log('\x1b[35m%s\x1b[0m', '🔑 Setting Token Authority...');
  console.log('\x1b[36m%s\x1b[0m', `   Network: ${CLUSTER}`);
  console.log('\x1b[36m%s\x1b[0m', `   Mint Address: ${mintAddress.toBase58()}`);
  console.log('\x1b[36m%s\x1b[0m', `   Authority Type: ${authorityTypeName}`);
  console.log('\x1b[36m%s\x1b[0m', `   Current Authority: ${currentAuthorityKeypair.publicKey.toBase58()}`);
  console.log('\x1b[36m%s\x1b[0m', `   New Authority: ${newAuthorityAddress.toBase58()}`);

  const connection = new Connection(RPC_URL, COMMITMENT);

  try {
    // Verify mint exists and is Token-2022
    console.log('\n\x1b[33m%s\x1b[0m', '🔍 Verifying mint account...');
    const mintInfo = await getMint(connection, mintAddress, 'confirmed', TOKEN_2022_PROGRAM_ID);
    console.log('\x1b[32m%s\x1b[0m', '✅ Mint account verified (Token-2022)');
    console.log('\x1b[36m%s\x1b[0m', `   Decimals: ${mintInfo.decimals}`);
    console.log('\x1b[36m%s\x1b[0m', `   Supply: ${mintInfo.supply.toString()}`);

    // Transfer authority
    console.log('\n\x1b[33m%s\x1b[0m', '📡 Sending setAuthority transaction...');
    
    const signature = await setAuthority(
      connection,
      currentAuthorityKeypair, // payer
      mintAddress, // mint
      currentAuthorityKeypair, // current authority
      authorityType, // authority type
      newAuthorityAddress, // new authority
      [], // multiSigners
      { commitment: 'confirmed' }, // confirmOptions
      TOKEN_2022_PROGRAM_ID // programId
    );

    console.log('\n\x1b[32m%s\x1b[0m', '✅ Authority transferred successfully!');
    console.log('\x1b[36m%s\x1b[0m', `   Transaction: ${signature}`);
    console.log('\x1b[36m%s\x1b[0m', `   Explorer: https://explorer.solana.com/tx/${signature}?cluster=${CLUSTER === 'mainnet' ? 'mainnet-beta' : CLUSTER}`);
    console.log('\n\x1b[33m%s\x1b[0m', `🔑 ${authorityTypeName} authority is now: ${newAuthorityAddress.toBase58()}`);

    // Update deployment file if exists
    const deploymentPath = path.resolve(__dirname, '../deployments', `${CLUSTER}.json`);
    if (fs.existsSync(deploymentPath)) {
      const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
      
      if (authorityType === AuthorityType.TransferFeeConfig) {
        if (!deploymentData.transfer_fee_authority_updated_to_multisig) {
          deploymentData.transfer_fee_authority_updated_to_multisig = {};
        }
        deploymentData.transfer_fee_authority_updated_to_multisig = {
          previous_authority: currentAuthorityKeypair.publicKey.toBase58(),
          new_authority: newAuthorityAddress.toBase58(),
          signature: signature,
          timestamp: new Date().toISOString(),
        };
        
        // Also update the mint creation data
        if (deploymentData.dbtc_mint_created) {
          deploymentData.dbtc_mint_created.transfer_fee_config_authority = newAuthorityAddress.toBase58();
        }
      } else if (authorityType === AuthorityType.WithheldWithdraw) {
        if (!deploymentData.withheld_withdraw_authority_updated_to_multisig) {
          deploymentData.withheld_withdraw_authority_updated_to_multisig = {};
        }
        deploymentData.withheld_withdraw_authority_updated_to_multisig = {
          previous_authority: currentAuthorityKeypair.publicKey.toBase58(),
          new_authority: newAuthorityAddress.toBase58(),
          signature: signature,
          timestamp: new Date().toISOString(),
        };
        
        // Also update the mint creation data
        if (deploymentData.dbtc_mint_created) {
          deploymentData.dbtc_mint_created.withdraw_withheld_authority = newAuthorityAddress.toBase58();
        }
      }
      
      fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
      console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data updated');
    }

  } catch (error) {
    console.error('\n\x1b[31m%s\x1b[0m', '❌ Error setting authority:', error.message);
    if (error.logs) {
      console.error('\x1b[31m%s\x1b[0m', 'Transaction logs:');
      error.logs.forEach(log => console.error(`  ${log}`));
    }
    process.exit(1);
  }
}

main().catch(console.error);

