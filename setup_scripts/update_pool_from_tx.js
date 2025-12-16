/**
 * Update mainnet.json with pool data from existing transaction
 * Derives PDAs from known mints and updates deployment file
 */

import { Connection, PublicKey } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load config
const configPath = path.resolve(__dirname, "./config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Known values from the transaction
const TX_SIGNATURE = "4AUSKzTuTHkYaFGd8xeXdridWuMbzDvQbYCQXT5nkj1H4uL7t1DiB4K9niKHgJF8BfGoxneGDwwDapBmjNhSFaQf";
const RAYDIUM_CP_PROGRAM_ID = new PublicKey("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");

// From tx logs: vault_0_amount:1000000000,vault_1_amount:99000000000
// vault_0 = 1 SOL (1,000,000,000 lamports)
// vault_1 = 99,000 DogeBTC (99,000,000,000 with 6 decimals)
const INITIAL_SOL_AMOUNT = "1000000000"; // 1 SOL
const INITIAL_DBTC_AMOUNT = "99000000000"; // 99,000 DogeBTC

async function main() {
  console.log("\x1b[35m%s\x1b[0m", "🔍 Deriving pool PDAs from known values...");
  console.log("\x1b[36m%s\x1b[0m", `Transaction: ${TX_SIGNATURE}`);
  console.log("\x1b[36m%s\x1b[0m", `Raydium Program: ${RAYDIUM_CP_PROGRAM_ID.toBase58()}`);

  const connection = new Connection(RPC_URL, COMMITMENT);

  // Load deployment data
  const deploymentDir = path.resolve(__dirname, config.deployment.paths.deployments_dir);
  const deploymentPath = path.resolve(deploymentDir, "mainnet.json");
  const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));

  // Get DogeBTC mint
  const dbtcMint = new PublicKey(deploymentData.dbtc_mint_address);
  console.log("\x1b[36m%s\x1b[0m", `DogeBTC Mint: ${dbtcMint.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `WSOL Mint: ${WSOL_MINT.toBase58()}`);

  // Determine token order (token0 < token1 by bytes)
  const dbtcMintBytes = dbtcMint.toBytes();
  const wsolMintBytes = WSOL_MINT.toBytes();
  const isMdogeToken0 = Buffer.compare(dbtcMintBytes, wsolMintBytes) < 0;

  const token0Mint = isMdogeToken0 ? dbtcMint : WSOL_MINT;
  const token1Mint = isMdogeToken0 ? WSOL_MINT : dbtcMint;

  console.log("\x1b[36m%s\x1b[0m", `\n🪙 Token Order:`);
  console.log("\x1b[36m%s\x1b[0m", `   Token0: ${token0Mint.toBase58()} ${isMdogeToken0 ? "(DogeBTC)" : "(WSOL)"}`);
  console.log("\x1b[36m%s\x1b[0m", `   Token1: ${token1Mint.toBase58()} ${!isMdogeToken0 ? "(DogeBTC)" : "(WSOL)"}`);

  // Derive AMM Config PDA (index 0 for 1% fee config on mainnet Raydium)
  const ammConfigIndex = 0; // Standard 1% fee config
  const [ammConfigPDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("amm_config"),
      Buffer.from([ammConfigIndex >> 8, ammConfigIndex & 0xff]), // u16 big-endian
    ],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `\n🔑 AMM Config PDA: ${ammConfigPDA.toBase58()}`);

  // Verify AMM config exists on-chain
  const ammConfigInfo = await connection.getAccountInfo(ammConfigPDA);
  if (!ammConfigInfo) {
    console.log("\x1b[33m%s\x1b[0m", "⚠️ AMM Config index 0 not found, trying index 2...");
    // Try index 2 (another common config)
  }

  // Derive Pool State PDA
  const [poolStatePDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("pool"),
      ammConfigPDA.toBuffer(),
      token0Mint.toBuffer(),
      token1Mint.toBuffer(),
    ],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 Pool State PDA: ${poolStatePDA.toBase58()}`);

  // Verify pool exists on-chain
  let poolStateInfo = await connection.getAccountInfo(poolStatePDA);
  let finalAmmConfigPDA = ammConfigPDA;
  let finalPoolStatePDA = poolStatePDA;
  let finalAmmConfigIndex = ammConfigIndex;
  
  if (!poolStateInfo) {
    console.log("\x1b[31m%s\x1b[0m", "❌ Pool State not found at derived PDA");
    console.log("\x1b[33m%s\x1b[0m", "⚠️ Trying to find pool with different AMM config index...");
    
    // Try different AMM config indices
    for (const idx of [1, 2, 3, 4, 5, 6]) {
      const [altAmmConfig] = PublicKey.findProgramAddressSync(
        [Buffer.from("amm_config"), Buffer.from([idx >> 8, idx & 0xff])],
        RAYDIUM_CP_PROGRAM_ID
      );
      
      const [altPoolState] = PublicKey.findProgramAddressSync(
        [Buffer.from("pool"), altAmmConfig.toBuffer(), token0Mint.toBuffer(), token1Mint.toBuffer()],
        RAYDIUM_CP_PROGRAM_ID
      );
      
      const altPoolInfo = await connection.getAccountInfo(altPoolState);
      if (altPoolInfo) {
        console.log("\x1b[32m%s\x1b[0m", `✅ Found pool with AMM config index ${idx}`);
        console.log("\x1b[36m%s\x1b[0m", `   AMM Config: ${altAmmConfig.toBase58()}`);
        console.log("\x1b[36m%s\x1b[0m", `   Pool State: ${altPoolState.toBase58()}`);
        
        // Update to use found values
        finalAmmConfigPDA = altAmmConfig;
        finalPoolStatePDA = altPoolState;
        finalAmmConfigIndex = idx;
        poolStateInfo = altPoolInfo;
        break;
      }
    }
    
    if (!poolStateInfo) {
      console.log("\x1b[31m%s\x1b[0m", "❌ Could not find pool with any AMM config index");
      process.exit(1);
    }
  } else {
    console.log("\x1b[32m%s\x1b[0m", "✅ Pool State found on-chain");
  }

  // Derive Authority PDA
  const [authorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault_and_lp_mint_auth_seed")],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 Authority PDA: ${authorityPDA.toBase58()}`);

  // Derive LP Mint PDA (using final pool state PDA)
  const [lpMintPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("pool_lp_mint"), finalPoolStatePDA.toBuffer()],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 LP Mint PDA: ${lpMintPDA.toBase58()}`);

  // Derive Token Vault PDAs (using final pool state PDA)
  const [token0VaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("pool_vault"), finalPoolStatePDA.toBuffer(), token0Mint.toBuffer()],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 Token0 Vault PDA: ${token0VaultPDA.toBase58()}`);

  const [token1VaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("pool_vault"), finalPoolStatePDA.toBuffer(), token1Mint.toBuffer()],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 Token1 Vault PDA: ${token1VaultPDA.toBase58()}`);

  // Derive Observation State PDA (using final pool state PDA)
  const [observationStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("observation"), finalPoolStatePDA.toBuffer()],
    RAYDIUM_CP_PROGRAM_ID
  );
  console.log("\x1b[36m%s\x1b[0m", `🔑 Observation State PDA: ${observationStatePDA.toBase58()}`);

  // Update deployment data
  console.log("\x1b[33m%s\x1b[0m", "\n📝 Updating deployment data...");

  deploymentData.RAYDIUM_CP_PROGRAM_ID = RAYDIUM_CP_PROGRAM_ID.toBase58();

  // AMM Config (using final found values)
  deploymentData.raydium_amm_config_created = {
    amm_config_pda: finalAmmConfigPDA.toBase58(),
    raydium_program_id: RAYDIUM_CP_PROGRAM_ID.toBase58(),
    config_index: finalAmmConfigIndex,
    trade_fee_rate: 10000, // 1% fee
    create_pool_fee: "0",
    status: "using_official_raydium_config",
    is_official_raydium: true,
    timestamp: new Date().toISOString(),
  };

  // Pool data (using final found values)
  deploymentData.dbtc_sol_pool_created = {
    poolStatePDA: finalPoolStatePDA.toBase58(),
    lpMintPDA: lpMintPDA.toBase58(),
    token0VaultPDA: token0VaultPDA.toBase58(),
    token1VaultPDA: token1VaultPDA.toBase58(),
    authorityPDA: authorityPDA.toBase58(),
    observationStatePDA: observationStatePDA.toBase58(),
    token0Mint: token0Mint.toBase58(),
    token1Mint: token1Mint.toBase58(),
    isMdogeToken0: isMdogeToken0,
    txid: TX_SIGNATURE,
    initialMdogeAmount: INITIAL_DBTC_AMOUNT,
    initialSolAmount: INITIAL_SOL_AMOUNT,
    initialMdogeReadable: "99,000 DogeBTC",
    initialSolReadable: "1 SOL",
    openTime: "0",
    timestamp: new Date().toISOString(),
  };

  // Save
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));

  console.log("\x1b[32m%s\x1b[0m", "\n✅ Deployment data updated successfully!");
  console.log("\x1b[36m%s\x1b[0m", `📁 Saved to: ${deploymentPath}`);
  
  // Print summary
  console.log("\x1b[35m%s\x1b[0m", "\n=================== POOL SUMMARY ===================");
  console.log("\x1b[36m%s\x1b[0m", `AMM Config Index: ${finalAmmConfigIndex}`);
  console.log("\x1b[36m%s\x1b[0m", `AMM Config: ${finalAmmConfigPDA.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Pool State: ${finalPoolStatePDA.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `LP Mint: ${lpMintPDA.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Token0 (${isMdogeToken0 ? "DogeBTC" : "WSOL"}): ${token0Mint.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Token1 (${!isMdogeToken0 ? "DogeBTC" : "WSOL"}): ${token1Mint.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Token0 Vault: ${token0VaultPDA.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Token1 Vault: ${token1VaultPDA.toBase58()}`);
  console.log("\x1b[36m%s\x1b[0m", `Initial Liquidity: 1 SOL + 99,000 DogeBTC`);
  console.log("\x1b[35m%s\x1b[0m", "====================================================");
}

main().catch((error) => {
  console.error("\x1b[31m%s\x1b[0m", "❌ Error:", error);
  process.exit(1);
});
