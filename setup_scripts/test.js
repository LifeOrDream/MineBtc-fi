import {
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    SystemProgram,
    Transaction,
} from "@solana/web3.js";
import { AnchorProvider, Program, EventParser } from "@coral-xyz/anchor";
import fs from 'fs';
import path from 'path';

// Get the current file's directory
const __dirname = new URL('.', import.meta.url).pathname;

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

// Load wallet keypair
const walletKeypair = (() => {
    try {
          const walletPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
      return Keypair.fromSecretKey(
              new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf-8')))
      );
    } catch (e) {
      console.error('\x1b[31m%s\x1b[0m', "❌ Failed to load wallet keypair:", e);
      console.error('\x1b[31m%s\x1b[0m', `   Expected path: ${path.resolve(__dirname, config.deployment.paths.deployer_key || 'undefined')}`);
      throw e;
    }
  })();

// Create wallet interface
const wallet = {
    publicKey: walletKeypair.publicKey,
    signTransaction: async (tx) => {
      tx.partialSign(walletKeypair);
      return tx;
    },
    signAllTransactions: async (txs) => {
      return txs.map(tx => {
        tx.partialSign(walletKeypair);
        return tx;
      });
    }
  };
  

// Setup
const connection = new Connection("https://api.devnet.solana.com");
const provider = new AnchorProvider(connection, wallet, {});

let programId = new PublicKey("5V8t8SG3bSdMmCcJeAW5Xv1n1r8uUsukn48x4NPVkgKH");
// Load MoonBase Program IDL
const IDL_MOONBASE = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moonbase_idl), 'utf-8')
); 
// Load Raydium Program IDL
const IDL_RAYDIUM = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.raydium_idl), 'utf-8')
); 
// Load Moon Economy Program IDL
const IDL_MOON_ECONOMY = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moon_economy_idl), 'utf-8')
); 


const program = new Program(IDL_RAYDIUM, provider);

// Get transaction logs
const txSig = "scHNbfy51xvsdDBVm7eA1M9hHENnk7ePV6MfZyGrZH95Cs6bDiHu7d31zHx69obXdPEeTUfd82E6aXxWMtNTHcS";
const tx = await connection.getTransaction(txSig, {
  commitment: "confirmed",
  maxSupportedTransactionVersion: 0,
});

console.log(tx);
const logs = tx?.meta?.logMessages ?? [];

console.log("program.programId");
console.log(program.programId);
console.log("program.coder");
console.log(program.coder);

// Parse logs to extract events
const parser = new EventParser(program.programId, program.coder);
console.log("parser");
console.log(parser);
// parser.parseLogs(logs, (event) => {
//   console.log("Parsed event:", event.name, event.data);
// });

// Method 2: Manual parsing of program data logs
console.log("\n🔧 Manual parsing of program data logs:");

for (const log of logs) {
  if (log.startsWith('Program data: ')) {
    const base64Data = log.replace('Program data: ', '');
    
    try {
      // Decode base64 to buffer
      const dataBuffer = Buffer.from(base64Data, 'base64');
      console.log(`Raw data buffer length: ${dataBuffer.length} bytes`);
      console.log(`Raw data hex: ${dataBuffer.toString('hex')}`);
      
      // The first 8 bytes are the event discriminator
      const discriminator = dataBuffer.slice(0, 8);
      console.log(`Event discriminator: ${discriminator.toString('hex')}`);
      
      // Try to decode as SwapEvent
      try {
        const eventData = program.coder.events.decode(dataBuffer);
        if (eventData) {
          console.log("\n🎉 Successfully decoded SwapEvent:");
          console.log(`Event Name: ${eventData.name}`);
          console.log(`Event Data:`, eventData.data);
          
          // Parse specific fields from SwapEvent
          const swapData = eventData.data;
          console.log("\n📊 Swap Details:");
          console.log(`Pool ID: ${swapData.poolId}`);
          console.log(`Input Amount: ${swapData.inputAmount}`);
          console.log(`Output Amount: ${swapData.outputAmount}`);
          console.log(`Input Vault Before: ${swapData.inputVaultBefore}`);
          console.log(`Output Vault Before: ${swapData.outputVaultBefore}`);
          console.log(`Input Transfer Fee: ${swapData.inputTransferFee}`);
          console.log(`Output Transfer Fee: ${swapData.outputTransferFee}`);
          console.log(`Is Base Input: ${swapData.baseInput}`);
        }
      } catch (decodeError) {
        console.log(`Failed to decode event: ${decodeError.message}`);
        
        // If automatic decoding fails, let's try manual decoding
        console.log("\n🔨 Attempting manual decoding...");
        
        // if (dataBuffer.length >= 8) {
        //   // Skip the 8-byte discriminator and manually parse the SwapEvent structure
        //   // Based on the SwapEvent structure in the Rust code:
        //   // pub struct SwapEvent {
        //   //     pub pool_id: Pubkey,                    // 32 bytes
        //   //     pub input_vault_before: u64,            // 8 bytes  
        //   //     pub output_vault_before: u64,           // 8 bytes
        //   //     pub input_amount: u64,                  // 8 bytes
        //   //     pub output_amount: u64,                 // 8 bytes
        //   //     pub input_transfer_fee: u64,            // 8 bytes
        //   //     pub output_transfer_fee: u64,           // 8 bytes
        //   //     pub base_input: bool,                   // 1 byte
        //   // }
          
        //   let offset = 8; // Skip discriminator
          
        //   // Pool ID (32 bytes)
        //   const poolId = new PublicKey(dataBuffer.slice(offset, offset + 32));
        //   offset += 32;
          
        //   // Read u64 values (8 bytes each, little-endian)
        //   const inputVaultBefore = dataBuffer.readBigUInt64LE(offset); offset += 8;
        //   const outputVaultBefore = dataBuffer.readBigUInt64LE(offset); offset += 8;
        //   const inputAmount = dataBuffer.readBigUInt64LE(offset); offset += 8;
        //   const outputAmount = dataBuffer.readBigUInt64LE(offset); offset += 8;
        //   const inputTransferFee = dataBuffer.readBigUInt64LE(offset); offset += 8;
        //   const outputTransferFee = dataBuffer.readBigUInt64LE(offset); offset += 8;
          
        //   // Base input (1 byte boolean)
        //   const baseInput = dataBuffer.readUInt8(offset) === 1;
          
        //   console.log("\n📈 Manually Decoded SwapEvent:");
        //   console.log(`Pool ID: ${poolId.toString()}`);
        //   console.log(`Input Vault Before: ${inputVaultBefore.toString()}`);
        //   console.log(`Output Vault Before: ${outputVaultBefore.toString()}`);
        //   console.log(`Input Amount: ${inputAmount.toString()}`);
        //   console.log(`Output Amount: ${outputAmount.toString()}`);
        //   console.log(`Input Transfer Fee: ${inputTransferFee.toString()}`);
        //   console.log(`Output Transfer Fee: ${outputTransferFee.toString()}`);
        //   console.log(`Is Base Input: ${baseInput}`);
          
        //   // Calculate price impact
        //   const priceBeforeSwap = Number(inputVaultBefore) / Number(outputVaultBefore);
        //   const priceAfterSwap = Number(inputVaultBefore + inputAmount) / Number(outputVaultBefore - outputAmount);
        //   const priceImpact = ((priceAfterSwap - priceBeforeSwap) / priceBeforeSwap) * 100;
          
        //   console.log("\n💹 Price Analysis:");
        //   console.log(`Price Before Swap: ${priceBeforeSwap.toFixed(10)}`);
        //   console.log(`Price After Swap: ${priceAfterSwap.toFixed(10)}`);
        //   console.log(`Price Impact: ${priceImpact.toFixed(4)}%`);
        // }
      }
    } catch (error) {
      console.log(`Error processing program data: ${error.message}`);
    }
    
    break; // Only process the first program data log
  }
}