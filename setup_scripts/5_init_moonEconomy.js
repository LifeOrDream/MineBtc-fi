// Import Anchor as CommonJS package
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3 } = pkg;
import { SystemProgram } from '@solana/web3.js';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import { BorshAccountsCoder } from '@coral-xyz/anchor';
import path from 'path';
import { 
    getSolanaBalance, initializeMoonEconomyProgram, mEconomySetupDbtcVault, mEconomySetupLiquidityVaults, 
    mEconomy_claimMoonbaseSol, mEconomy_withdrawDevEarnings, updateGlobalConfig, updateMoonEconomyGlobalConfig,
    LOOT_REWARDS_SEED, LOOT_SOL_VAULT_SEED
 } from './helper.js';

// Get the current file's directory
const __dirname = new URL('.', import.meta.url).pathname;

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;

// Load deployment data
const deploymentDir = path.resolve(__dirname, './deployments');
const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);

let deploymentFile = {};
if (fs.existsSync(deploymentPath)) {
    deploymentFile = JSON.parse(fs.readFileSync(deploymentPath, 'utf-8'));
} else {
    // Create deployments directory if it doesn't exist
    if (!fs.existsSync(deploymentDir)) {
        fs.mkdirSync(deploymentDir, { recursive: true });
    }
    console.log('\x1b[33m%s\x1b[0m', '⚠️ No deployment file found. Starting fresh deployment.');
}

// Program IDs from deployment file
const ID_MOONBASE_PROGRAM = deploymentFile.MOON_BASE_PROGRAM_ID ? 
    new PublicKey(deploymentFile.MOON_BASE_PROGRAM_ID) : null;
const ID_MOON_ECONOMY_PROGRAM = deploymentFile.MOON_ECONOMY_PROGRAM_ID ? 
    new PublicKey(deploymentFile.MOON_ECONOMY_PROGRAM_ID) : null;

// Moon Economy configuration
const MOONDOGE_ALLOCATION = config.moonEconomy?.moondoge_allocation || 25;
const LIQUIDITY_ALLOCATION = config.moonEconomy?.liquidity_allocation || 25;
const GAME_ALLOCATION = config.moonEconomy?.game_allocation || 30;
const MIN_LOCKUP_DAYS = config.moonEconomy?.min_lockup_days || 1;
const MAX_LOCKUP_DAYS = config.moonEconomy?.max_lockup_days || 365;
const BASE_MULTIPLIER = config.moonEconomy?.base_multiplier || 100;
const MAX_MULTIPLIER = config.moonEconomy?.max_multiplier || 700;
const ELECTRICITY_PER_WEIGHTED_MDOGE = config.moonEconomy?.electricity_per_weighted_mdoge || 100;

// -------------------------------------------------------------------
// ==================== [ READ ::: IDL | WALLET | DEPLOYMENT ] ====================
// -------------------------------------------------------------------

// Load IDLs
const IDL_MOONBASE = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moonbase_idl), 'utf-8')
);

const IDL_MOON_ECONOMY = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moon_economy_idl), 'utf-8')
);

// Solana Connection
const connection = new Connection(RPC_URL, config.network.commitment);

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

// Create provider
const provider = new AnchorProvider(connection, wallet, { commitment: config.network.commitment });
setProvider(provider);

// Helper function to save deployment data
function saveDeploymentData() {
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentFile, null, 2));
    console.log('\x1b[32m%s\x1b[0m', '✅ Deployment file updated');
}

// ----------------------------------------------------------- 
// ==================== [ ::: MAIN SCRIPT ::: ] ====================
// ----------------------------------------------------------- 

async function main() {
    console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ DogeTech Moon Economy Initialization ================================');
    console.log('\x1b[36m%s\x1b[0m', '👤 Admin Wallet:', walletKeypair.publicKey.toString());
    console.log('\x1b[36m%s\x1b[0m', '🌐 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🔗 RPC URL:', RPC_URL);
    
    const balance = await getSolanaBalance(connection, walletKeypair.publicKey);
    console.log('\x1b[36m%s\x1b[0m', '💰 Balance:', balance / 1e9, 'SOL');

    // Verify prerequisites
    if (!ID_MOONBASE_PROGRAM) {
        console.error('\x1b[31m%s\x1b[0m', '❌ MoonBase program ID not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please deploy the MoonBase program first.');
        return;
    }

    if (!ID_MOON_ECONOMY_PROGRAM) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Moon Economy program ID not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please deploy the Moon Economy program first.');
        return;
    }
  
    console.log('\x1b[35m%s\x1b[0m', '============================== [ PROGRAMS ] ===============================');
    console.log('\x1b[36m%s\x1b[0m', '🚀 MoonBase Program ID:', ID_MOONBASE_PROGRAM.toString());
    console.log('\x1b[36m%s\x1b[0m', '🏦 Moon Economy Program ID:', ID_MOON_ECONOMY_PROGRAM.toString());

    const moonBaseProgram = new Program(IDL_MOONBASE, provider);
    const moonEconomyProgram = new Program(IDL_MOON_ECONOMY, provider);
    console.log('\x1b[32m%s\x1b[0m', '✅ Connected to programs');

    // Check program account data
    await checkProgramInfo();
    // return

    try {
        // 1. Initialize Moon Economy Program
        await initializeMoonEconomyProgramLocal(moonEconomyProgram);
        // return;
        
        // 2. Initialize MDOGE Vault
        await initializeDbtcVault(moonEconomyProgram);
        // return;
        
        // 3. Initialize Liquidity Vault
        await initializeLiquidityVault(moonEconomyProgram);
        // return;
        
        // 🔧 Configuration Updates
        await updateMoonBaseConfig(moonBaseProgram, deploymentFile.moonbase_program_initialized.moonbase_fee_collector);
        return;

        // 4. Claim MoonBase SOL
        // await claimMoonbaseSol(moonEconomyProgram);
        // return;
 
        // // 💰 Earnings Management
        // await withdrawDevEarnings(moonEconomyProgram);

        // Print completion summary
        printCompletionSummary();

        // ==================== [ ADMIN FUNCTIONS - UNCOMMENT AS NEEDED ] ====================
        
        // 📊 System Status & Utilities
        // await queryEconomyConfig(moonEconomyProgram);
        // await queryMoonBaseConfig(moonBaseProgram);
        
        
        // await updateMoonEconomyConfig(moonEconomyProgram, "new_authority_address");
        
        // 🚀 Complete Admin Examples
        // await exampleAdminOperations(moonEconomyProgram, moonBaseProgram);

    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Initialization failed:', error);
        process.exit(1);
    }
}

// ==================== [ INITIALIZATION FUNCTIONS ] ====================

async function checkProgramInfo() {
    try {
       const moonEconomyProgramInfo = await connection.getAccountInfo(ID_MOON_ECONOMY_PROGRAM);
        console.log('\x1b[36m%s\x1b[0m', '\n🔍 Moon Economy Program Info:');
        console.log('\x1b[36m%s\x1b[0m', '   Account exists:', !!moonEconomyProgramInfo);
        console.log('\x1b[36m%s\x1b[0m', '   Program size:', moonEconomyProgramInfo?.data.length || 0, 'bytes');
        console.log('\x1b[36m%s\x1b[0m', '   Is executable:', moonEconomyProgramInfo?.executable || false);
    } catch (err) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Could not fetch program info:', err.message);
    } 
}

async function initializeMoonEconomyProgramLocal(moonEconomyProgram) {
    if (deploymentFile.moonEconomy_program_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Moon Economy program already initialized. Skipping...');
        return;
    }

            console.log('\x1b[35m%s\x1b[0m', '\n====================== [ INITIALIZING MOON ECONOMY PROGRAM ] ====================');
    
    const devAddress = walletKeypair.publicKey;
    console.log('\x1b[36m%s\x1b[0m', '📝 Configuration:');
    console.log('\x1b[36m%s\x1b[0m', `   Dev Address: ${devAddress.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `   DogeBtc Allocation: ${MOONDOGE_ALLOCATION}%`);
    console.log('\x1b[36m%s\x1b[0m', `   Liquidity Allocation: ${LIQUIDITY_ALLOCATION}%`);
    console.log('\x1b[36m%s\x1b[0m', `   Lockup Days: ${MIN_LOCKUP_DAYS}-${MAX_LOCKUP_DAYS}`);
    console.log('\x1b[36m%s\x1b[0m', `   Multipliers: ${BASE_MULTIPLIER}-${MAX_MULTIPLIER}`);
    
    const result = await initializeMoonEconomyProgram(
        connection, moonEconomyProgram, wallet, walletKeypair,
        devAddress, MOONDOGE_ALLOCATION, LIQUIDITY_ALLOCATION,  
        MIN_LOCKUP_DAYS, MAX_LOCKUP_DAYS, BASE_MULTIPLIER, MAX_MULTIPLIER
    );

            if (result.success) {
                console.log('\x1b[32m%s\x1b[0m', '✅ Program initialized successfully!');
                console.log('\x1b[36m%s\x1b[0m', '🔑 Global Config Address:', result.data.globalConfig_address);
                console.log('\x1b[36m%s\x1b[0m', '🔑 Dev Earnings Address:', result.data.devEarningsCollector_address);
                console.log('\x1b[36m%s\x1b[0m', '🔑 Fee Collector Address:', result.data.feeCollector_address);

                deploymentFile.moonEconomy_program_initialized = {
                    moonEconomy_globalConfig_data_ac: result.data.globalConfig_address,
                    moonEconomy_devEarnings_data_ac: result.data.devEarningsCollector_address,
            moonEconomy_feeCollector_data_ac: result.data.feeCollector_address,
            timestamp: new Date().toISOString()
                };                
        saveDeploymentData();
            } else {
        throw new Error(`Program initialization failed: ${result.error}`);
    }
}

async function initializeDbtcVault(moonEconomyProgram) {
    if (deploymentFile.moonEconomy_mDogeVault_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ MDOGE vault already initialized. Skipping...');
            return;
        }

            console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING MOON ECONOMY MDOGE VAULT ] =================');
            
            // Get token addresses from deployment file
    const dbtcMintKey = deploymentFile.dbtc_mint_address || deploymentFile.dbtc_dbtc_mint_account_created?.dbtc_mintAddress;
    if (!dbtcMintKey) {
        throw new Error('MDOGE token mint address not found in deployment file');
    }
    
    const dbtc_TOKEN_MINT = new PublicKey(dbtcMintKey);
            console.log('\x1b[36m%s\x1b[0m', `🔑 MDOGE Token (SPL-2022): ${dbtc_TOKEN_MINT.toString()}`);
    
            const result = await mEconomySetupDbtcVault(
        connection, moonEconomyProgram, wallet, walletKeypair,
        dbtc_TOKEN_MINT, anchor_spl.TOKEN_2022_PROGRAM_ID
            );

            if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ MDOGE vault initialized successfully!');
                
                deploymentFile.moonEconomy_mDogeVault_initialized = {
                    dogebtcVault: result.data.dogebtcVaultAddress,
                    dbtcSolVault: result.data.dbtcSolVaultAddress,
                    dbtcCustodian: result.data.dbtcCustodianAddress,
            dbtcCustodianAuthority: result.data.dbtcCustodianAuthorityAddress,
            timestamp: new Date().toISOString()
                };
        saveDeploymentData();
            } else {
        throw new Error(`MDOGE vault initialization failed: ${result.error}`);
    }
}

async function initializeLiquidityVault(moonEconomyProgram) {
    if (deploymentFile.moonEconomy_liquidityVault_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Liquidity vault already initialized. Skipping...');
        return;
    }

            console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING MOON ECONOMY LIQUIDITY VAULT ] =================');
            
    // Get LP token addresses from deployment file
    const lpMintKey = deploymentFile.dbtc_sol_pool_created?.lpMintPDA;
    if (!lpMintKey) {
        throw new Error('LP token mint address not found in deployment file');
    }
    
    const LP_TOKEN_MINT = new PublicKey(lpMintKey);
            console.log('\x1b[36m%s\x1b[0m', `🔑 LP Token (standard SPL): ${LP_TOKEN_MINT.toString()}`);
            
            const result = await mEconomySetupLiquidityVaults(
        connection, moonEconomyProgram, wallet, walletKeypair,
        LP_TOKEN_MINT, anchor_spl.TOKEN_PROGRAM_ID
            );

            if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Liquidity vault initialized successfully!');
                
                deploymentFile.moonEconomy_liquidityVault_initialized = {
                    liquidityVault: result.data.liquidityVaultAddress,
                    liquiditySolVault: result.data.liquiditySolVaultAddress,
                    liquidityCustodian: result.data.liquidityCustodianAddress,
            liquidityCustodianAuthority: result.data.liquidityCustodianAuthorityAddress,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        throw new Error(`Liquidity vault initialization failed: ${result.error}`);
    }
}

async function claimMoonbaseSol(moonEconomyProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ CLAIMING MOONBASE SOL ] =================');

    // Derive loot rewards PDAs
    const [lootRewardsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(LOOT_REWARDS_SEED)], 
        ID_MOONBASE_PROGRAM
      );
      
      const [lootSolVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(LOOT_SOL_VAULT_SEED)], 
        ID_MOONBASE_PROGRAM
      );

      console.log(`DEBUG: lootRewardsPDA: ${lootRewardsPDA}`);
      console.log(`DEBUG: lootSolVaultPDA: ${lootSolVaultPDA}`);

    try {
        // Verify all required addresses exist
        const requiredAddresses = {
            'Moon Economy Global Config': deploymentFile.moonEconomy_program_initialized?.moonEconomy_globalConfig_data_ac,
            'DogeBtc Vault': deploymentFile.moonEconomy_mDogeVault_initialized?.dogebtcVault,
            'Liquidity Vault': deploymentFile.moonEconomy_liquidityVault_initialized?.liquidityVault,
            'MoonBase Global Config': deploymentFile.moonbase_program_initialized?.globalConfig_address,
            'MoonBase Treasury': deploymentFile.moonbase_program_initialized?.solTreasury_address,
            'Fee Collector': deploymentFile.moonEconomy_program_initialized?.moonEconomy_feeCollector_data_ac
        };

        console.log('\x1b[36m%s\x1b[0m', '🔍 Verifying required addresses:');
        for (const [name, address] of Object.entries(requiredAddresses)) {
            if (!address) {
                throw new Error(`${name} address not found in deployment file`);
            }
            console.log('\x1b[36m%s\x1b[0m', `   ${name}: ${address}`);
        }

                const result = await mEconomy_claimMoonbaseSol(
            connection, moonEconomyProgram, wallet, walletKeypair,
                    deploymentFile.moonEconomy_program_initialized.moonEconomy_globalConfig_data_ac,
                    deploymentFile.moonEconomy_mDogeVault_initialized.dogebtcVault,
                    deploymentFile.moonEconomy_liquidityVault_initialized.liquidityVault,
                    deploymentFile.moonEconomy_mDogeVault_initialized.dbtcSolVault,
                    deploymentFile.moonEconomy_liquidityVault_initialized.liquiditySolVault,
                    deploymentFile.moonEconomy_program_initialized.moonEconomy_devEarnings_data_ac,
            deploymentFile.moonbase_program_initialized.globalConfig_address,
            deploymentFile.moonbase_program_initialized.dogeBtcMining_address,            
            deploymentFile.moonbase_program_initialized.solTreasury_address,
                    deploymentFile.moonEconomy_program_initialized.moonEconomy_feeCollector_data_ac,
            lootSolVaultPDA,
            lootRewardsPDA,
            ID_MOONBASE_PROGRAM.toString()
                );

                if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ MoonBase SOL claimed successfully!');
                    
                    deploymentFile.moonEconomy_claimMoonbaseSol = {
                        claimMoonbaseSolTxid: result.data.claimMoonbaseSolTxid,
                timestamp: new Date().toISOString()
            };
            saveDeploymentData();
        } else {
            throw new Error(`MoonBase SOL claim failed: ${result.error}`);
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error claiming MoonBase SOL:', error.message);
        console.log('\x1b[33m%s\x1b[0m', '🔍 Debugging suggestions:');
        console.log('\x1b[33m%s\x1b[0m', '  - Check that all programs are properly initialized');
        console.log('\x1b[33m%s\x1b[0m', '  - Verify account ownership and permissions');
        console.log('\x1b[33m%s\x1b[0m', '  - Ensure PDAs are derived with correct seeds');
        throw error;
    }
}

function printCompletionSummary() {
    console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ MOON ECONOMY INITIALIZATION COMPLETE ================================');
    console.log('\x1b[32m%s\x1b[0m', '✅ All systems initialized successfully!');
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Summary:');
    console.log('\x1b[36m%s\x1b[0m', `  • Moon Economy Program: ${deploymentFile.moonEconomy_program_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • MDOGE Vault: ${deploymentFile.moonEconomy_mDogeVault_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Liquidity Vault: ${deploymentFile.moonEconomy_liquidityVault_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • MoonBase SOL Claim: ${deploymentFile.moonEconomy_claimMoonbaseSol ? '✅' : '❌'}`);
    console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
    
    if (deploymentFile.moonEconomy_program_initialized) {
        console.log('\x1b[90m%s\x1b[0m', '\n🔑 Important Addresses:');
        console.log('\x1b[90m%s\x1b[0m', `   Global Config: ${deploymentFile.moonEconomy_program_initialized.moonEconomy_globalConfig_data_ac}`);
        console.log('\x1b[90m%s\x1b[0m', `   Dev Earnings: ${deploymentFile.moonEconomy_program_initialized.moonEconomy_devEarnings_data_ac}`);
        console.log('\x1b[90m%s\x1b[0m', `   Fee Collector: ${deploymentFile.moonEconomy_program_initialized.moonEconomy_feeCollector_data_ac}`);
        if (deploymentFile.moonEconomy_mDogeVault_initialized) {
            console.log('\x1b[90m%s\x1b[0m', `   MDOGE Vault: ${deploymentFile.moonEconomy_mDogeVault_initialized.dogebtcVault}`);
        }
        if (deploymentFile.moonEconomy_liquidityVault_initialized) {
            console.log('\x1b[90m%s\x1b[0m', `   Liquidity Vault: ${deploymentFile.moonEconomy_liquidityVault_initialized.liquidityVault}`);
        }
    }
}

// ==================== [ ADMIN FUNCTIONS ] ====================
// 
// All admin functions now accept flexible parameters instead of hardcoded values:
//
// 📊 queryEconomyConfig(program) - View current economy configuration
// 📊 queryMoonBaseConfig(program) - View current moonbase configuration
// 💰 withdrawDevEarnings(program) - Withdraw accumulated dev earnings
// 🔧 updateMoonBaseConfig(program, newFeeCollectorAddress) - Update moonbase fee collector
// 🔧 updateMoonEconomyConfig(program, newAuthorityAddress) - Update economy authority
//

async function queryEconomyConfig(moonEconomyProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ QUERYING ECONOMY CONFIG ] ================');
    
    try {
        const globalConfigAddress = deploymentFile.moonEconomy_program_initialized?.moonEconomy_globalConfig_data_ac;
        if (!globalConfigAddress) {
            console.error('\x1b[31m%s\x1b[0m', '❌ Global config address not found');
            return;
        }

        const accountInfo = await connection.getAccountInfo(new PublicKey(globalConfigAddress));
        if (!accountInfo) {
            console.error('\x1b[31m%s\x1b[0m', '❌ Could not fetch account info');
            return;
        }

        const coder = new BorshAccountsCoder(IDL_MOON_ECONOMY);
        const economyConfig = coder.decode('GlobalConfig', accountInfo.data);
        
        // Convert PublicKeys to strings for display
        const displayConfig = {
            ...economyConfig,
            authority: economyConfig.authority.toBase58(),
            dev_address: economyConfig.dev_address.toBase58(),
            fee_collector: economyConfig.fee_collector.toBase58(),
            min_lockup_days: economyConfig.min_lockup_days.toNumber(),
            max_lockup_days: economyConfig.max_lockup_days.toNumber(),
            last_claim_slot: economyConfig.last_claim_slot.toNumber()
        };

        console.log('\x1b[36m%s\x1b[0m', '📊 Current Economy Configuration:');
        console.log('\x1b[36m%s\x1b[0m', `   Authority: ${displayConfig.authority}`);
        console.log('\x1b[36m%s\x1b[0m', `   Dev Address: ${displayConfig.dev_address}`);
        console.log('\x1b[36m%s\x1b[0m', `   Fee Collector: ${displayConfig.fee_collector}`);
        console.log('\x1b[36m%s\x1b[0m', `   DogeBtc Allocation: ${displayConfig.moondoge_allocation}%`);
        console.log('\x1b[36m%s\x1b[0m', `   Liquidity Allocation: ${displayConfig.liquidity_allocation}%`);
        console.log('\x1b[36m%s\x1b[0m', `   Game Allocation: ${displayConfig.game_allocation}%`);
        console.log('\x1b[36m%s\x1b[0m', `   Lockup Days: ${displayConfig.min_lockup_days}-${displayConfig.max_lockup_days}`);
        console.log('\x1b[36m%s\x1b[0m', `   Multipliers: ${displayConfig.base_multiplier}-${displayConfig.max_multiplier}`);
        console.log('\x1b[36m%s\x1b[0m', `   Last Claim Slot: ${displayConfig.last_claim_slot}`);
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to query economy config:', error);
    }
}

async function queryMoonBaseConfig(moonBaseProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ QUERYING MOONBASE CONFIG ] ================');
    
    try {
        const globalConfigAddress = deploymentFile.moonbase_program_initialized?.globalConfig_address;
        if (!globalConfigAddress) {
            console.error('\x1b[31m%s\x1b[0m', '❌ MoonBase global config address not found');
            return;
        }

        const accountInfo = await connection.getAccountInfo(new PublicKey(globalConfigAddress), 'confirmed');
        if (!accountInfo) {
            console.error('\x1b[31m%s\x1b[0m', '❌ Could not fetch MoonBase account info');
            return;
        }

        const coder = new BorshAccountsCoder(IDL_MOONBASE);
        const moonbaseConfig = coder.decode('GlobalConfig', accountInfo.data);
        
        // Convert PublicKeys to strings for display
        const displayConfig = {
            ...moonbaseConfig,
            ext_authority: moonbaseConfig.ext_authority.toBase58(),
            ext_fee_collector: moonbaseConfig.ext_fee_collector.toBase58(),
            pda_sol_treasury: moonbaseConfig.pda_sol_treasury.toBase58(),
            base_creation_cost: moonbaseConfig.base_creation_cost.toNumber()
        };

        console.log('\x1b[36m%s\x1b[0m', '📊 Current MoonBase Configuration:');
        console.log('\x1b[36m%s\x1b[0m', `   Authority: ${displayConfig.ext_authority}`);
        console.log('\x1b[36m%s\x1b[0m', `   Fee Collector: ${displayConfig.ext_fee_collector}`);
        console.log('\x1b[36m%s\x1b[0m', `   SOL Treasury: ${displayConfig.pda_sol_treasury}`);
        console.log('\x1b[36m%s\x1b[0m', `   Base Creation Cost: ${displayConfig.base_creation_cost / 1e9} SOL`);
        console.log('\x1b[36m%s\x1b[0m', `   Loot Percentage: ${displayConfig.loot_percentage}%`);
        console.log('\x1b[36m%s\x1b[0m', `   Game Active: ${displayConfig.is_game_active ? '🟢 YES' : '🔴 NO'}`);
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to query moonbase config:', error);
    }
}

async function withdrawDevEarnings(moonEconomyProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ WITHDRAWING DEV EARNINGS ] ================');
    
    try {
        const globalConfigAddress = deploymentFile.moonEconomy_program_initialized?.moonEconomy_globalConfig_data_ac;
        const devEarningsAddress = deploymentFile.moonEconomy_program_initialized?.moonEconomy_devEarnings_data_ac;
        
        if (!globalConfigAddress || !devEarningsAddress) {
            console.error('\x1b[31m%s\x1b[0m', '❌ Required addresses not found in deployment file');
            return;
        }

        console.log('\x1b[36m%s\x1b[0m', `📝 Withdrawing earnings to: ${walletKeypair.publicKey.toString()}`);
        
        const result = await mEconomy_withdrawDevEarnings(
            connection, moonEconomyProgram, wallet, walletKeypair,
            globalConfigAddress, devEarningsAddress
        );

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ Dev earnings withdrawn successfully!');
            console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
                } else {
            console.error('\x1b[31m%s\x1b[0m', '❌ Failed to withdraw dev earnings:', result.error);
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error withdrawing dev earnings:', error);
    }
}

async function updateMoonBaseConfig(moonBaseProgram, newFeeCollectorAddress) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING MOONBASE CONFIG ] ================');
    
    if (!newFeeCollectorAddress) {
        console.error('\x1b[31m%s\x1b[0m', '❌ newFeeCollectorAddress parameter is required');
        return;
    }

    try {
        const feeCollectorPubkey = new PublicKey(newFeeCollectorAddress);
        const globalConfigAddress = deploymentFile.moonbase_program_initialized?.globalConfig_address;
        const moduleConfigStoreAddress = deploymentFile.config_stores_initialized?.module_config_store;
        const dogeBtcMiningAddress = deploymentFile.moonbase_program_initialized?.dogeBtcMining_address;

        const requiredAddresses = {
            'MoonBase Global Config': globalConfigAddress,
            'Module Config Store': moduleConfigStoreAddress,
            'Doge BTC Mining State': dogeBtcMiningAddress
        };

        for (const [label, value] of Object.entries(requiredAddresses)) {
            if (!value) {
                throw new Error(`${label} address not found in deployment file`);
            }
        }

        console.log('\x1b[36m%s\x1b[0m', `📝 Setting new fee collector: ${feeCollectorPubkey.toString()}`);

        const result = await updateGlobalConfig(
            connection,
            moonBaseProgram,
            wallet,
            walletKeypair,
            globalConfigAddress,
            moduleConfigStoreAddress,
            dogeBtcMiningAddress,
            walletKeypair.publicKey.toString(),
            feeCollectorPubkey.toString(),
            null,
            null,
            null
        );

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ MoonBase config updated successfully!');
            console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid ?? result.data.updateTxid}`);

            deploymentFile.moonbase_program_initialized = {
                ...deploymentFile.moonbase_program_initialized,
                moonbase_fee_collector: feeCollectorPubkey.toString()
            };
            saveDeploymentData();
        } else {
            console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update MoonBase config:', result.error);
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error updating MoonBase config:', error);
    }
}

async function updateMoonEconomyConfig(moonEconomyProgram, newAuthorityAddress) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING MOON ECONOMY CONFIG ] ================');
    
    if (!newAuthorityAddress) {
        console.error('\x1b[31m%s\x1b[0m', '❌ newAuthorityAddress parameter is required');
        return;
    }

    try {
        console.log('\x1b[36m%s\x1b[0m', `📝 Setting new authority: ${newAuthorityAddress}`);
        
        const result = await updateMoonEconomyGlobalConfig(
            connection, moonEconomyProgram, wallet, walletKeypair,
            deploymentFile.moonbase_program_initialized?.globalConfig_address,
            deploymentFile.config_stores_initialized?.module_config_store,
            deploymentFile.moonbase_program_initialized?.dogeBtcMining_address,
            newAuthorityAddress, // new authority
            deploymentFile.moonEconomy_program_initialized?.moonEconomy_feeCollector_data_ac,
            null, null, null // other params unchanged
        );

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ Moon Economy config updated successfully!');
            console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
        } else {
            console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update Moon Economy config:', result.error);
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error updating Moon Economy config:', error);
    }
}

// Example usage functions
async function exampleAdminOperations(moonEconomyProgram, moonBaseProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ EXAMPLE ADMIN OPERATIONS ] ================');
    
    // 1. Query current configurations
    await queryEconomyConfig(moonEconomyProgram);
    await queryMoonBaseConfig(moonBaseProgram);
    
    // 2. Withdraw dev earnings (uncomment to execute)
    // await withdrawDevEarnings(moonEconomyProgram);
    
    // 3. Update configurations (uncomment and provide addresses to execute)
    // await updateMoonBaseConfig(moonBaseProgram, "new_fee_collector_pubkey_here");
    // await updateMoonEconomyConfig(moonEconomyProgram, "new_authority_pubkey_here");
    
    console.log('\x1b[32m%s\x1b[0m', '✅ Admin operations example completed!');
}

// ==================== [ QUICK ADMIN SHORTCUTS ] ====================

// Quick status check
async function quickStatusCheck(moonEconomyProgram, moonBaseProgram) {
    console.log('\x1b[33m%s\x1b[0m', '📊 Quick system status check...');
    await queryEconomyConfig(moonEconomyProgram);
    await queryMoonBaseConfig(moonBaseProgram);
}

// Quick earnings withdrawal
async function quickWithdrawEarnings(moonEconomyProgram) {
    console.log('\x1b[33m%s\x1b[0m', '💰 Quick dev earnings withdrawal...');
    await withdrawDevEarnings(moonEconomyProgram);
}

// Run the main script
main().catch(console.error);