// Import Anchor as CommonJS package
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3, Wallet } = pkg;
import { SystemProgram } from '@solana/web3.js';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import path from 'path';
import { 
    getSolanaBalance, initializeMoonbaseProgram, depositMDOGE, setupMiningVault, 
    initializeConfigStores, addNewModuleToConfigStore, updateModuleStatsHelper, createSystemReferralAccount,
    addFactions as addFactionsHelper, initializeLootRewards, initializeLevelStats, updateDeploymentStatus,
    updateGlobalConfigHelper, toggleGameActiveHelper, updateSlotsForSwapHelper,
    updateModuleConfigHelper, getSystemStatus, updateMdogeDistPerSlot, initializeBuybacks,
    DOGE_BTC_VAULT_SEED, DOGE_BTC_VAULT_AUTHORITY_SEED, MODULE_CONFIG_STORE_SEED,
    MODULE_CONFIG_SEED, USER_MOONBASE_SEED, REFERRAL_REWARDS_SEED, MODULE_INSTANCE_SEED,
    LOOT_REWARDS_SEED, LEVEL_STATS_SEED, PVP_MATCHMAKER_SEED
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

// Calculate amounts based on config
const MOONDOGE_TOKEN_MINT = deploymentFile.dbtc_mint_address ? 
    new PublicKey(deploymentFile.dbtc_mint_address) : null;
 

const ID_MOONBASE_PROGRAM = deploymentFile.MOON_BASE_PROGRAM_ID ? 
    new PublicKey(deploymentFile.MOON_BASE_PROGRAM_ID) : null;

// Mining configuration
const dbtc_DEPOSIT_AMOUNT = new BN(config.mining.initial_deposit);
const MINING_START_TIMESTAMP = config.mining.start_timestamp || Math.floor(Date.now() / 1000);
const MINING_doge_btc_PER_SLOT = new BN(config.mining.doge_btc_per_slot);

const RAYDIUM_PROGRAM_ID = deploymentFile.RAYDIUM_CP_PROGRAM_ID;


const COMMITMENT = config.network.commitment;

// Color constants for consistent logging
const COLOR_STEP = '\x1b[35m%s\x1b[0m';
const COLOR_INFO = '\x1b[36m%s\x1b[0m';
const COLOR_SUCCESS = '\x1b[32m%s\x1b[0m';
const COLOR_WARNING = '\x1b[33m%s\x1b[0m';
const COLOR_ERROR = '\x1b[31m%s\x1b[0m';
const COLOR_DIM = '\x1b[90m%s\x1b[0m';

// -------------------------------------------------------------------
// ==================== [ READ ::: IDL | WALLET | DEPLOYMENT ] ====================
// -------------------------------------------------------------------

// Load MoonBase Program IDL
const IDL_MOONBASE = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moonbase_idl), 'utf-8')
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
    console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ DogeTech MoonBase Initialization ================================');
    console.log('\x1b[36m%s\x1b[0m', '👤 Admin Wallet:', walletKeypair.publicKey.toString());
    console.log('\x1b[36m%s\x1b[0m', '🌐 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🔗 RPC URL:', RPC_URL);
    
    const balance = await getSolanaBalance(connection, walletKeypair.publicKey);
    console.log('\x1b[36m%s\x1b[0m', '💰 Balance:', balance / 1e9, 'SOL');
    // return

    // Verify prerequisites
    if (!MOONDOGE_TOKEN_MINT) {
        console.error('\x1b[31m%s\x1b[0m', '❌ DOGE_BTC token mint address not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please run the token deployment script first.');
        return;
    }

    if (!ID_MOONBASE_PROGRAM) {
        console.error('\x1b[31m%s\x1b[0m', '❌ MoonBase program ID not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please deploy the MoonBase program first.');
        return;
    }
  
    console.log('\x1b[35m%s\x1b[0m', '============================== [ PROGRAMS ] ===============================');
    console.log('\x1b[36m%s\x1b[0m', '🚀 MoonBase Program ID:', ID_MOONBASE_PROGRAM.toString());        
    console.log('\x1b[36m%s\x1b[0m', '🪙 DOGE_BTC Token Mint:', MOONDOGE_TOKEN_MINT.toString());
    
    const moonbaseProgram = new Program(IDL_MOONBASE, provider);
    console.log('\x1b[32m%s\x1b[0m', '✅ Connected to program:', moonbaseProgram.programId.toString());

    try {
        // 1. Initialize MoonBase Program
        await initializeMoonbaseProgramLocal(moonbaseProgram);
        // return;
        
        // 2. Initialize Mining System
        await initializeMiningSystem(moonbaseProgram);
        // return
        
        // 3. Initialize Referral System
        await initializeReferralSystem(moonbaseProgram);
        // return;
        
        // 4. Initialize Config Stores
        await initializeConfigStoresLocal(moonbaseProgram);
        // return;
        
        // 4.5. Set Raydium Pool State (security: prevents using malicious pools)
        await setRaydiumPoolState(moonbaseProgram);
        // return;
        
        // 5. Initialize Loot & Level Stats
        await initializeLootAndStats(moonbaseProgram);
        // return;
        
        // 1. Create Dragon Egg Collection (collection is automatically set in global config)
        await createDragonEggCollection(connection, walletKeypair, deploymentFile, deploymentPath);
        
        // 2. Add Dragon Egg URIs to the pool
        await addDragonEggUris(connection, walletKeypair, deploymentFile, deploymentPath);
        // return;
        
        // 7. Add Factions
        await addFactions(moonbaseProgram);
        // return;

        // 9. Add Command Center Modules (must be added before regular modules to get correct IDs)
        await addCommandCenters(moonbaseProgram);
        // return;

        
        // 8. Add Expansions
        await addExpansions(moonbaseProgram);
        // return;

        
        // 9.5. Add Regular Modules
        // await addModules(moonbaseProgram);
        // return;
        
        // 10. Deposit Mining Tokens
        await depositMiningTokens(moonbaseProgram);
        // return;

        // 11. Initialize LP Token Accounts (required for Raydium integration)
        await initializeLpTokenAccounts(moonbaseProgram);

        // 11.5. Initialize Buybacks System (required for distribution rate updates)
        await initializeBuybacksSystem(moonbaseProgram);
        // return;
        
        // // // 12. Update DOGE_BTC Distribution Rate
        // await updateDistributionRate(moonbaseProgram);
        // return;

        // Print completion summary
        // printCompletionSummary();

        // ==================== [ ADMIN FUNCTIONS - UNCOMMENT AS NEEDED ] ====================
        
        // 📊 System Status & Utilities
        // await adminUtilities(moonbaseProgram); // View current system state
        
        // 🎮 PvP Game Controls
        // await toggleGameActive(moonbaseProgram);   // Toggle current state

        // 💰 Economic Updates  
        // await updateGlobalConfig(moonbaseProgram, null, null, null, 200000000, 15); // 0.2 SOL, 15% loot
        
        // ⚡ Mining Rate Controls
        // await updateSlotsForSwap(moonbaseProgram, 10800); // Set specific value
        
        // 🏭 Module Configuration Updates
        // await updateModuleConfig(moonbaseProgram, 1, "https://arweave.net/new-url", [0, 1], 5, 180000000, 90000000);
        

    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Initialization failed:', error);
        process.exit(1);
    }
}

// ==================== [ INITIALIZATION FUNCTIONS ] ====================

async function initializeMoonbaseProgramLocal(moonbaseProgram) {
    if (deploymentFile.moonbase_program_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ MoonBase program already initialized. Skipping...');
            return;
        }

    console.log('\x1b[35m%s\x1b[0m', '\n====================== [ INITIALIZING MOONBASE PROGRAM ] ====================');
    
    const result = await initializeMoonbaseProgram(
                    connection,
                    moonbaseProgram,
                    wallet,
                    walletKeypair,
        wallet.publicKey  // Use wallet as creation fee recipient for now
                );
                
    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Program initialized successfully!');
        deploymentFile.moonbase_program_initialized = {
            globalConfig_address: result.data.globalConfig_address,
            dogeBtcMining_address: result.data.dogeBtcMining_address,
            solTreasury_address: result.data.solTreasury_address,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
                } else {
        throw new Error(`Program initialization failed: ${result.error}`);
    }
}



/**
 * Adds Dragon Egg URIs to the MoonBase program's URI pool
 */
async function addDragonEggUris(connection, deployerKeypair, deploymentData, deploymentPath) {
    if (!deploymentData.dragon_egg_collection_created) {
        console.error(COLOR_ERROR, '❌ Dragon Egg collection must be created first');
        throw new Error('Collection not created');
    }

    if (deploymentData.dragon_egg_uris_added) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg URIs already added');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ ADDING DRAGON EGG URIS ] ===================');
    
    try {
        // Load MoonBase program
        const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
        if (!fs.existsSync(moonbaseIdlPath)) {
            throw new Error(`MoonBase IDL not found at: ${moonbaseIdlPath}`);
        }
        
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const wallet = new Wallet(deployerKeypair);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
        const moonbaseProgram = new Program(moonbaseIdl, provider);
        
        // Derive Global Config PDA
        const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('global-config')],
            moonbaseProgram.programId
        );
        const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
        const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
        
        console.log(COLOR_INFO, '🔑 MoonBase Program:', moonbaseProgram.programId.toString());
        console.log(COLOR_INFO, '📝 Adding URIs:', config.dragon_eggs.uris.length);
        config.dragon_eggs.uris.forEach((uri, index) => {
            console.log(COLOR_DIM, `   ${index + 1}. ${uri}`);
        });
        
        // Call the program instruction
        const txid = await moonbaseProgram.methods
            .addDragonEggUris(config.dragon_eggs.uris)
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigStore: moduleConfigStorePDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: deployerKeypair.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();
        
        console.log(COLOR_SUCCESS, '✅ Dragon Egg URIs added successfully!');
        console.log(COLOR_DIM, '🔗 Transaction:', txid);
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/tx/${txid}?cluster=${CLUSTER}`);
        
        // Save to deployment data
        deploymentData.dragon_egg_uris_added = {
            uris: config.dragon_eggs.uris,
            tx_signature: txid,
            timestamp: new Date().toISOString()
        };
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to add Dragon Egg URIs:', error);
        throw error;
    }
}


/**
 * Sets the Dragon Egg collection address in the MoonBase program
 * NOTE: This function is now redundant as the collection is automatically set
 * in create_dragon_egg_collection_internal. Keeping for reference or manual override.
 */
async function setCollectionInMoonBase(connection, deployerKeypair, deploymentData, deploymentPath) {
    const collectionAddress = new PublicKey(deploymentData.dragon_egg_collection_created.collection_address);

    if (deploymentData.dragon_egg_collection_set_in_program) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg collection already set in MoonBase program');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ SETTING COLLECTION IN MOONBASE ] ===================');
    console.log(COLOR_WARNING, '⚠️ NOTE: Collection is automatically set during creation. This call is redundant.');
    
    try {
        // Load MoonBase program
        const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
        if (!fs.existsSync(moonbaseIdlPath)) {
            throw new Error(`MoonBase IDL not found at: ${moonbaseIdlPath}`);
        }
        
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const wallet = new Wallet(deployerKeypair);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
        const moonbaseProgram = new Program(moonbaseIdl, provider);
        
        console.log(COLOR_INFO, '🔑 MoonBase Program:', moonbaseProgram.programId.toString());
        console.log(COLOR_INFO, '🎨 Collection Address:', collectionAddress.toString());
        
        // Derive Global Config PDA (use correct seed with hyphen)
        const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('global-config')],
            moonbaseProgram.programId
        );
        const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
        const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
        
        console.log(COLOR_DIM, '🔍 Global Config PDA:', globalConfigPDA.toString());
        console.log(COLOR_INFO, '📡 Calling set_dragon_egg_collection...');
        
        // Ensure collectionAddress is a PublicKey object
        const collectionPubkey = collectionAddress instanceof PublicKey 
            ? collectionAddress 
            : new PublicKey(collectionAddress);

        // Call the program instruction
        const txid = await moonbaseProgram.methods
            .setDragonEggCollection(collectionPubkey)
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigStore: moduleConfigStorePDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: deployerKeypair.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();
        
        console.log(COLOR_SUCCESS, '✅ Dragon Egg collection set in MoonBase program!');
        console.log(COLOR_DIM, '🔗 Transaction:', txid);
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/tx/${txid}?cluster=${CLUSTER}`);
        
        // Save to deployment data
        deploymentData.dragon_egg_collection_set_in_program = {
            collection_address: collectionAddress.toString(),
            global_config_pda: globalConfigPDA.toString(),
            tx_signature: txid,
            timestamp: new Date().toISOString()
        };
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to set collection in program:', error);
        throw error;
    }
}



async function initializeMiningSystem(moonbaseProgram) {
    if (deploymentFile.mining_vault_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Mining system already initialized. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ INITIALIZING MINING SYSTEM ] ===================');
    
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created.poolStatePDA;

    if (!raydiumPoolState) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Raydium pool state not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please run the raydium deployment script first.');
        return;
    }
    
    const [vaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(DOGE_BTC_VAULT_SEED), dogeBtcMiningPDA.toBuffer()],
        moonbaseProgram.programId
    );
    const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(DOGE_BTC_VAULT_AUTHORITY_SEED)],
        moonbaseProgram.programId
    );

    console.log('\x1b[36m%s\x1b[0m', `🔑 Mining Token Vault PDA: ${vaultPDA.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔑 Vault Authority PDA: ${vaultAuthorityPDA.toString()}`);

    const result = await setupMiningVault(
        connection, moonbaseProgram, wallet, walletKeypair,
        dogeBtcMiningPDA, vaultPDA, vaultAuthorityPDA, MOONDOGE_TOKEN_MINT,
        anchor_spl.TOKEN_2022_PROGRAM_ID, MINING_START_TIMESTAMP,
        MINING_doge_btc_PER_SLOT, raydiumPoolState
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Mining system initialized!');
        deploymentFile.mining_vault_initialized = {
            vault_address: vaultPDA.toString(),
            vault_authority: vaultAuthorityPDA.toString(),
            start_timestamp: MINING_START_TIMESTAMP,
            doge_btc_per_slot: MINING_doge_btc_PER_SLOT.toString(),
            init_tx: result.data.initMiningTxid,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        throw new Error(`Mining system initialization failed: ${result.error}`);
    }
}

async function setRaydiumPoolState(moonbaseProgram) {
    if (deploymentFile.raydium_pool_state_set) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Raydium pool state already set. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ SETTING RAYDIUM POOL STATE ] ===================');
    
    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created?.poolStatePDA;

    if (!raydiumPoolState) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Raydium pool state not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please run the raydium deployment script first.');
        return;
    }

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const poolStatePubkey = new PublicKey(raydiumPoolState);

    console.log('\x1b[36m%s\x1b[0m', `🔑 Pool State Address: ${poolStatePubkey.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `🔐 Global Config PDA: ${globalConfigPDA.toString()}`);

    try {
        const tx = await moonbaseProgram.methods
            .setRaydiumPoolState(poolStatePubkey)
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigStore: moduleConfigStorePDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log('\x1b[32m%s\x1b[0m', '✅ Raydium pool state set successfully!');
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${tx}`);
        console.log('\x1b[90m%s\x1b[0m', `   Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);

        deploymentFile.raydium_pool_state_set = {
            pool_state_address: poolStatePubkey.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to set Raydium pool state:', error);
        throw error;
    }
}

async function initializeReferralSystem(moonbaseProgram) {
    if (deploymentFile.referral_system_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Referral system already initialized. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING REFERRAL SYSTEM ] ================');
            
              const [referralRewardsPDA] = PublicKey.findProgramAddressSync(
                [Buffer.from(REFERRAL_REWARDS_SEED), SystemProgram.programId.toBuffer()],
                moonbaseProgram.programId
              );            
  
    const result = await createSystemReferralAccount(
        connection, moonbaseProgram, wallet, walletKeypair, referralRewardsPDA
                );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Referral system initialized!');
        deploymentFile.referral_system_initialized = {
            system_referral_pda: referralRewardsPDA.toString(),
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        throw new Error(`Referral system initialization failed: ${result.error}`);
    }
}

async function initializeConfigStoresLocal(moonbaseProgram) {
    if (deploymentFile.config_stores_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Config stores already initialized. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING CONFIG STORES ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const [moduleConfigStorePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from(MODULE_CONFIG_STORE_SEED)],
        moonbaseProgram.programId
    );

    const result = await initializeConfigStores(
        connection, moonbaseProgram, wallet, walletKeypair,
        globalConfigPDA, moduleConfigStorePDA
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Config stores initialized!');
        deploymentFile.config_stores_initialized = {
            module_config_store: moduleConfigStorePDA.toString(),
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
                } else {
        throw new Error(`Config stores initialization failed: ${result.error}`);
                }
}

async function initializeLootAndStats(moonbaseProgram) {
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    
    // Initialize Loot Rewards
    if (!deploymentFile.loot_rewards_initialized) {
        console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING LOOT REWARDS ] ================');
        
        const result = await initializeLootRewards(
            connection, moonbaseProgram, wallet, walletKeypair,
            globalConfigPDA, MOONDOGE_TOKEN_MINT
        );

        console.log("/ Initialize Loot Rewards");
        console.log(result);

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ Loot rewards initialized!');
            deploymentFile.loot_rewards_initialized = {
                loot_rewards_pda: result.data.lootRewardsPDA,
                sol_vault: result.data.lootSolVaultPDA,
                dbtc_vault: result.data.lootMdogeVaultPDA,
                loot_dbtc_vault_authority: result.data.lootMdogeVaultAuthorityPDA,
                initTxid: result.data.initTxid,
                timestamp: new Date().toISOString()
            };
            saveDeploymentData();
        } else {
            throw new Error(`Loot rewards initialization failed: ${result.error}`);
        }
    }

    // Initialize Level Stats
    if (!deploymentFile.level_stats_initialized) {
        console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING LEVEL STATS ] ================');
            
        const result = await initializeLevelStats(
            connection, moonbaseProgram, wallet, walletKeypair, globalConfigPDA
        );

        console.log("/ Initialize Level Stats");
        console.log(result);

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', '✅ Level stats initialized!');
            deploymentFile.level_stats_initialized = {
                level_stats_pda: result.data.levelStatsPDA,
                initTxid: result.data.initTxid,
                timestamp: new Date().toISOString()
            };
            saveDeploymentData();
        } else {
            throw new Error(`Level stats initialization failed: ${result.error}`);
        }
    }
}


/**
 * Creates the Dragon Egg NFT collection using Metaplex Core
 * NOTE: The collection is automatically set in global_config during creation,
 * so no separate set_collection call is needed.
 */
async function createDragonEggCollection(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.dragon_egg_collection_created) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg collection already created');
        console.log(COLOR_INFO, '🔑 Collection Address:', deploymentData.dragon_egg_collection_created.collection_address);
        return new PublicKey(deploymentData.dragon_egg_collection_created.collection_address);
    }

    console.log(COLOR_STEP, '\n=================== [ CREATING DRAGON EGG COLLECTION ] ===================');
    
    try {
        // Load MoonBase program
        const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const wallet = new Wallet(deployer);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
        const moonbaseProgram = new Program(moonbaseIdl, provider);
        
        // Derive PDAs
        const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("global-config")],
            moonbaseProgram.programId
        );
        
        const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("collection_authority")],
            moonbaseProgram.programId
        );
        
        console.log(COLOR_INFO, '🎨 Creating Metaplex Core collection...');
        console.log(COLOR_DIM, `   Name: ${config.dragon_eggs.collection_name}`);
        console.log(COLOR_DIM, `   URI: ${config.dragon_eggs.collection_uri}`);
        console.log(COLOR_INFO, '🔐 Collection Authority PDA:', collectionAuthorityPDA.toString());
        
        // Generate a new keypair for the collection
        const collectionKeypair = Keypair.generate();
        
        // Call the MoonBase admin function to create the collection
        const tx = await moonbaseProgram.methods
            .createDragonEggCollection(
                config.dragon_eggs.collection_name,
                config.dragon_eggs.collection_uri
            )
            .accounts({
                authority: deployer.publicKey,
                globalConfig: globalConfigPDA,
                collection: collectionKeypair.publicKey,
                collectionAuthority: collectionAuthorityPDA,
                mplCoreProgram: new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"),
                systemProgram: SystemProgram.programId,
            })
            .signers([collectionKeypair])
            .rpc();
            
        const collectionPubkey = collectionKeypair.publicKey;
        
        console.log(COLOR_SUCCESS, '✅ Dragon Egg collection created successfully!');
        console.log(COLOR_INFO, '🔑 Collection Address:', collectionPubkey.toString());
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/address/${collectionPubkey.toString()}?cluster=${CLUSTER}`);
        
        console.log(COLOR_INFO, '📍 Transaction:', tx);
        
        // Verify collection was created by checking global config
        const globalConfig = await moonbaseProgram.account.globalConfig.fetch(globalConfigPDA);
        if (globalConfig.dragonEggCollection.toString() === collectionPubkey.toString()) {
            console.log(COLOR_SUCCESS, '✅ Collection verified in global config');
        }
        
        // Save to deployment data
        deploymentData.dragon_egg_collection_created = {
            collection_address: collectionPubkey.toString(),
            collection_name: config.dragon_eggs.collection_name,
            collection_uri: config.dragon_eggs.collection_uri,
            update_authority: collectionAuthorityPDA.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to create collection:', error);
        throw error;
    }
}
 

async function addFactions(moonbaseProgram) {
    if (deploymentFile.factions_added) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Factions already added. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ ADDING FACTIONS ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    
    // Collect all faction names
    const factionNames = config.factions.map(f => f.name);
    
    try {
        const result = await addFactionsHelper(
            connection, moonbaseProgram, wallet, walletKeypair,
            globalConfigPDA, moduleConfigStorePDA, dogeBtcMiningPDA,
            factionNames
        );

        if (result.success) {
            console.log('\x1b[32m%s\x1b[0m', `✅ Added ${factionNames.length} factions successfully`);
            console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.addFactionsTxid}`);
            
            deploymentFile.factions_added = {
                factions: config.factions.map(f => ({
                    name: f.name,
                    description: f.description,
                })),
                tx: result.data.addFactionsTxid,
                timestamp: new Date().toISOString()
            };
            saveDeploymentData();
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to add factions:', error);
        throw error;
    }
}

async function addExpansions(moonbaseProgram) {
    if (deploymentFile.expansions_added) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Expansions already added. Skipping...');
            return;
        }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ ADDING EXPANSIONS ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const addedExpansions = [];

    for (const expansion of config.expansions) {
        try {
            const tx = await moonbaseProgram.methods
                .addExpansion(
                    expansion.id,
                    expansion.name,
                    expansion.required_level,
                    new BN(expansion.cost_sol),
                    expansion.new_width,
                    expansion.new_height
                )
                .accounts({
                    globalConfig: globalConfigPDA,
                    authority: wallet.publicKey,
                    systemProgram: web3.SystemProgram.programId,
                })
                .rpc();

            console.log('\x1b[32m%s\x1b[0m', `✅ Added expansion: ${expansion.name}`);
            addedExpansions.push({ ...expansion, tx });
        } catch (error) {
            console.log('\x1b[33m%s\x1b[0m', `⚠️ Expansion ${expansion.name} may already exist`);
            addedExpansions.push({ ...expansion, status: 'already_exists' });
        }
    }

    deploymentFile.expansions_added = {
        expansions: addedExpansions,
        timestamp: new Date().toISOString()
    };
    saveDeploymentData();
}

async function addModules(moonbaseProgram) {
    if (deploymentFile.modules_added) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Modules already added. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ ADDING MODULES ] ================');
            
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
    const addedModules = [];

    for (let i = 0; i < config.modules.length; i++) {
        const module = config.modules[i];
        
        try {
            console.log(`\n🔧 Processing module: ${module.name}`);
            
            // Step 1: Create module config
            const result = await addNewModuleToConfigStore(
                connection, moonbaseProgram, wallet, walletKeypair,
                globalConfigPDA, moduleConfigStorePDA,
                module.name, module.image_url, module.module_type, module.stats,
                module.faction_ids, module.min_level, 10, // max_per_base default to 10
                module.width, module.height,
                new BN(module.mint_cost), new BN(module.upgrade_cost),
                module.upgrade_level_requirements || []
            );

            if (result.success) {
                console.log('\x1b[32m%s\x1b[0m', `✅ Step 1: Module config created for ${module.name} (ID: ${result.data.moduleId})`);
                
                // Step 2: Update module stats to activate it
                const statsResult = await updateModuleStatsHelper(
                    connection, moonbaseProgram, wallet, walletKeypair, globalConfigPDA,
                    result.data.moduleId, module.stats, module.module_type
                );

                if (statsResult.success) {
                    console.log('\x1b[32m%s\x1b[0m', `✅ Step 2: Module stats updated and activated for ${module.name}`);
                    addedModules.push({
                        ...module,
                        config_id: result.data.moduleId,
                        create_tx: result.data.addModuleTxid,
                        stats_tx: statsResult.data.updateStatsTxid,
                        status: 'completed'
                    });
                } else {
                    console.log('\x1b[33m%s\x1b[0m', `⚠️ Step 2 failed for ${module.name}: ${statsResult.error}`);
                    addedModules.push({
                        ...module,
                        config_id: result.data.moduleId,
                        create_tx: result.data.addModuleTxid,
                        status: 'stats_failed',
                        error: statsResult.error
                    });
                }
            } else {
                console.log('\x1b[33m%s\x1b[0m', `⚠️ Step 1 failed for ${module.name}: ${result.error}`);
                addedModules.push({
                    ...module,
                    status: 'create_failed',
                    error: result.error
                });
            }
        } catch (error) {
            console.log('\x1b[31m%s\x1b[0m', `❌ Error processing ${module.name}: ${error.message}`);
            addedModules.push({
                ...module,
                status: 'error',
                error: error.message
            });
        }
    }

    deploymentFile.modules_added = {
        modules: addedModules,
        timestamp: new Date().toISOString()
    };
    saveDeploymentData();
}

async function addCommandCenters(moonbaseProgram) {
    if (deploymentFile.command_centers_added) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Command centers already added. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ ADDING COMMAND CENTER MODULES ] ================');
    console.log('\x1b[33m%s\x1b[0m', '⚠️  Note: Command centers use auto-incrementing IDs from module config store.');
    console.log('\x1b[33m%s\x1b[0m', '⚠️  If IDs don\'t match expected values (1000-1003), update contract constants accordingly.');
            
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const moduleConfigStorePDA = new PublicKey(deploymentFile.config_stores_initialized.module_config_store);
    const addedCommandCenters = [];

    // Check if command_centers exists in config
    if (!config.command_centers || config.command_centers.length === 0) {
        console.log('\x1b[33m%s\x1b[0m', '⚠️ No command centers defined in config.json. Skipping...');
        return;
    }

    for (let i = 0; i < config.command_centers.length ; i++) {  
        const commandCenter = config.command_centers[i];
        
        try {
            console.log(`\n🏛️ Processing command center: ${commandCenter.name} (Tier ${commandCenter.tier}, Config ID: ${commandCenter.config_id})`);
            
            // Derive module config PDA with specific config_id
            const configIdBuffer = Buffer.allocUnsafe(2);
            configIdBuffer.writeUInt16LE(commandCenter.config_id, 0);
            const [moduleConfigAccountPDA] = PublicKey.findProgramAddressSync(
                [Buffer.from(MODULE_CONFIG_SEED), configIdBuffer],
                moonbaseProgram.programId
            );
            
            console.log('\x1b[36m%s\x1b[0m', `🔑 Command Center Config PDA: ${moduleConfigAccountPDA.toString()}`);
            console.log('\x1b[36m%s\x1b[0m', `🔑 Using Config ID: ${commandCenter.config_id}`);
            
            // Check if account already exists
            const existingAccount = await connection.getAccountInfo(moduleConfigAccountPDA);
            if (existingAccount) {
                console.log('\x1b[33m%s\x1b[0m', `⚠️ Command center config ${commandCenter.config_id} already exists. Skipping creation...`);
                
                // Still update stats if needed
                const statsResult = await updateModuleStatsHelper(
                    connection, moonbaseProgram, wallet, walletKeypair, globalConfigPDA,
                    commandCenter.config_id, commandCenter.stats, commandCenter.module_type
                );
                
                if (statsResult.success) {
                    console.log('\x1b[32m%s\x1b[0m', `✅ Command center stats updated for ${commandCenter.name}`);
                    addedCommandCenters.push({
                        ...commandCenter,
                        config_id: commandCenter.config_id,
                        status: 'updated_stats_only'
                    });
                }
                continue;
            }
            
            // Step 1: Create command center module config with specific config_id
            // We need to manually create the account since addModuleToBase uses nextId
            // For now, we'll use addNewModuleToConfigStore and verify the ID matches
            // Note: This requires command centers to be added before other modules use IDs 1000-1003
            const result = await addNewModuleToConfigStore(
                connection, moonbaseProgram, wallet, walletKeypair,
                globalConfigPDA, moduleConfigStorePDA,
                commandCenter.name, commandCenter.image_url, commandCenter.module_type, commandCenter.stats,
                commandCenter.faction_ids, commandCenter.min_level, 1,  
                commandCenter.width, commandCenter.height,
                new BN(commandCenter.mint_cost), new BN(commandCenter.upgrade_cost),
                commandCenter.upgrade_level_requirements || []
            );

            if (result.success) {
                const actualConfigId = result.data.moduleId;
                console.log('\x1b[32m%s\x1b[0m', `✅ Step 1: Command center config created for ${commandCenter.name} (ID: ${actualConfigId})`);
                
                // Verify the config_id matches
                if (actualConfigId !== commandCenter.config_id) {
                    console.log('\x1b[33m%s\x1b[0m', `⚠️ Warning: Config ID mismatch. Expected ${commandCenter.config_id}, got ${actualConfigId}`);
                    console.log('\x1b[33m%s\x1b[0m', `⚠️ Note: Command centers use auto-incrementing IDs. Update contract constants if needed.`);
                    console.log('\x1b[33m%s\x1b[0m', `⚠️ The PDA derivation uses the actual ID (${actualConfigId}), not the expected ID (${commandCenter.config_id}).`);
                    
                    // Update the config_id to match what was actually created
                    commandCenter.config_id = actualConfigId;
                }
                
                // Step 2: Update module stats to activate it
                const statsResult = await updateModuleStatsHelper(
                    connection, moonbaseProgram, wallet, walletKeypair, globalConfigPDA,
                    result.data.moduleId, commandCenter.stats, commandCenter.module_type
                );

                if (statsResult.success) {
                    console.log('\x1b[32m%s\x1b[0m', `✅ Step 2: Command center stats updated and activated for ${commandCenter.name}`);
                    addedCommandCenters.push({
                        ...commandCenter,
                        config_id: actualConfigId, // Use the actual ID that was created
                        expected_config_id: commandCenter.config_id, // Save original expected ID for reference
                        create_tx: result.data.addModuleTxid,
                        stats_tx: statsResult.data.updateStatsTxid,
                        status: 'completed'
                    });
                    
                    if (actualConfigId !== commandCenter.config_id) {
                        console.log('\x1b[33m%s\x1b[0m', `⚠️ IMPORTANT: Update contract constants:`);
                        console.log('\x1b[33m%s\x1b[0m', `   COMMAND_CENTER_TIER_${commandCenter.tier}_CONFIG_ID = ${actualConfigId}`);
                    }
                } else {
                    console.log('\x1b[33m%s\x1b[0m', `⚠️ Step 2 failed for ${commandCenter.name}: ${statsResult.error}`);
                    addedCommandCenters.push({
                        ...commandCenter,
                        config_id: actualConfigId,
                        expected_config_id: commandCenter.config_id,
                        create_tx: result.data.addModuleTxid,
                        status: 'stats_failed',
                        error: statsResult.error
                    });
                }
            } else {
                console.log('\x1b[33m%s\x1b[0m', `⚠️ Step 1 failed for ${commandCenter.name}: ${result.error}`);
                addedCommandCenters.push({
                    ...commandCenter,
                    status: 'create_failed',
                    error: result.error
                });
            }
        } catch (error) {
            console.log('\x1b[31m%s\x1b[0m', `❌ Error processing ${commandCenter.name}: ${error.message}`);
            addedCommandCenters.push({
                ...commandCenter,
                status: 'error',
                error: error.message
            });
        }
    }

    deploymentFile.command_centers_added = {
        command_centers: addedCommandCenters,
        timestamp: new Date().toISOString()
    };
    saveDeploymentData();
}

async function depositMiningTokens(moonbaseProgram) {
    if (deploymentFile.mining_tokens_deposited) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Mining tokens already deposited. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ DEPOSITING MINING TOKENS ] ================');
    
    const vaultPDA = new PublicKey(deploymentFile.mining_vault_initialized.vault_address);
    const vaultAuthorityPDA = new PublicKey(deploymentFile.mining_vault_initialized.vault_authority);
    
    // Get user's token account
    const userTokenAccount = await anchor_spl.getAssociatedTokenAddress(
        MOONDOGE_TOKEN_MINT, 
        wallet.publicKey, 
        false, 
        anchor_spl.TOKEN_2022_PROGRAM_ID
    );

    console.log('\x1b[36m%s\x1b[0m', `💰 Depositing ${dbtc_DEPOSIT_AMOUNT.toString()} tokens...`);

    const result = await depositMDOGE(
        connection, moonbaseProgram, wallet, walletKeypair,
        userTokenAccount, vaultPDA, vaultAuthorityPDA, MOONDOGE_TOKEN_MINT,
        anchor_spl.TOKEN_2022_PROGRAM_ID, dbtc_DEPOSIT_AMOUNT
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Mining tokens deposited successfully!');
        deploymentFile.mining_tokens_deposited = {
            amount: dbtc_DEPOSIT_AMOUNT.toString(),
            tx: result.data.depositTxid,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        throw new Error(`Token deposit failed: ${result.error}`);
    }
}

async function initializeBuybacksSystem(moonbaseProgram) {
    if (deploymentFile.buybacks_initialized) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Buybacks system already initialized. Skipping...');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING BUYBACKS SYSTEM ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    
    const result = await initializeBuybacks(
        connection, moonbaseProgram, wallet, walletKeypair,
        globalConfigPDA
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Buybacks system initialized!');
        deploymentFile.buybacks_initialized = {
            buybacks_account_pda: result.data.buybacksAccountPDA,
            buybacks_sol_vault_pda: result.data.buybacksSolVaultPDA,
            init_tx: result.data.initTxid,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        throw new Error(`Buybacks system initialization failed: ${result.error}`);
    }
}

async function initializeLpTokenAccounts(moonbaseProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ INITIALIZING LP TOKEN ACCOUNTS ] ================');
    
    try {
        // Get LP mint and vault authority from deployment file
        if (!deploymentFile.dbtc_sol_pool_created?.lpMintPDA) {
            console.log('\x1b[33m%s\x1b[0m', '⚠️ LP mint not found in deployment file. Cannot initialize LP token accounts.');
            return;
        }

        if (!deploymentFile.mining_vault_initialized?.vault_authority) {
            console.log('\x1b[33m%s\x1b[0m', '⚠️ Vault authority not found in deployment file. Cannot initialize LP token accounts.');
            return;
        }

        const lpMint = new PublicKey(deploymentFile.dbtc_sol_pool_created.lpMintPDA);
        const vaultAuthority = new PublicKey(deploymentFile.mining_vault_initialized.vault_authority);
        
        // For Raydium deposit, LP token account must be owned by vault authority (same as other token accounts)
        const lpTokenAccount = await anchor_spl.getAssociatedTokenAddress(
            lpMint,
            vaultAuthority,
            true, // allowOwnerOffCurve
            anchor_spl.TOKEN_PROGRAM_ID
        );
        
 

        // Check if LP token account already exists
        const lpAccountInfo = await connection.getAccountInfo(lpTokenAccount);
        if (lpAccountInfo) {
            console.log('\x1b[33m%s\x1b[0m', 'ℹ️ LP token accounts already initialized. Skipping...');
            return;
        }

        console.log('\x1b[36m%s\x1b[0m', '🔄 Initializing LP token accounts...');
        console.log('\x1b[90m%s\x1b[0m', `   LP Token Account (ATA): ${lpTokenAccount.toString()}`);
        console.log('\x1b[90m%s\x1b[0m', `   LP Token Owner (Vault Authority): ${vaultAuthority.toString()}`);
        console.log('\x1b[90m%s\x1b[0m', `   LP Mint: ${lpMint.toString()}`);

        // Create associated token account for LP tokens with vault authority as owner (required by Raydium)
        const createdAccount = await anchor_spl.getOrCreateAssociatedTokenAccount(
            connection,
            walletKeypair,
            lpMint,
            vaultAuthority,
            true, // allowOwnerOffCurve
            'confirmed',
            {},
            anchor_spl.TOKEN_PROGRAM_ID
        );

        console.log('\x1b[32m%s\x1b[0m', '✅ LP token accounts initialized successfully!');
        console.log('\x1b[90m%s\x1b[0m', `   LP Token Account: ${createdAccount.address.toString()}`);

        // Save to deployment file
        deploymentFile.lp_token_accounts_initialized = {
            lp_token_account: createdAccount.address.toString(),
            lp_token_owner: vaultAuthority.toString(),
            lp_mint: lpMint.toString(),
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();

    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to initialize LP token accounts:', error);
        console.log('\x1b[33m%s\x1b[0m', '   This may not be critical - LP accounts can be created on-demand');
    }
}

async function updateDistributionRate(moonbaseProgram) {

    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING DOGE_BTC DISTRIBUTION RATE ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const ammConfigPDA = new PublicKey(deploymentFile.raydium_amm_config_created.amm_config_pda);
    const solTreasuryPDA = new PublicKey(deploymentFile.moonbase_program_initialized.solTreasury_address);
    const vaultAuthorityPDA = new PublicKey(deploymentFile.mining_vault_initialized.vault_authority);
    const dbtcTokenAccount = new PublicKey(deploymentFile.mining_vault_initialized.vault_address);

    // Check if Raydium pool is available for integration
    if (!deploymentFile.dbtc_sol_pool_created) {
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Raydium pool not found in deployment file.');
        console.log('\x1b[33m%s\x1b[0m', '   This function requires a deployed Raydium pool for DOGE_BTC-SOL trading.');
        console.log('\x1b[36m%s\x1b[0m', '   Please run the Raydium deployment script first.');
        return;
    }

    // Get Raydium pool data from deployment file
    const raydiumPoolData = deploymentFile.dbtc_sol_pool_created;
    console.log('\x1b[36m%s\x1b[0m', `🔄 Updating distribution rate with Raydium integration...`);
    console.log('\x1b[36m%s\x1b[0m', `   Pool State: ${raydiumPoolData.poolStatePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `   LP Mint: ${raydiumPoolData.lpMintPDA}`);
    
    console.log('\x1b[36m%s\x1b[0m', `   Raydium Program: ${RAYDIUM_PROGRAM_ID}`);
    
    const result = await updateMdogeDistPerSlot(
        connection, moonbaseProgram, RAYDIUM_PROGRAM_ID, wallet, walletKeypair, globalConfigPDA,
        dogeBtcMiningPDA, raydiumPoolData, MOONDOGE_TOKEN_MINT, ammConfigPDA, solTreasuryPDA,
        vaultAuthorityPDA, dbtcTokenAccount
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ DOGE_BTC distribution rate updated with Raydium integration!');
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.updateDistTxid}`);
        deploymentFile.dbtc_dist_per_slot_updated = {
            dogeBtcMiningPDA: result.data.dogeBtcMiningPDA,
            vaultAuthorityPDA: result.data.vaultAuthorityPDA,
            solTreasuryPDA: result.data.solTreasuryPDA,
            poolStatePDA: result.data.poolStatePDA,
            ammConfigPDA: result.data.ammConfigPDA,
            method: 'raydium_integration',
            tx_signature: result.data.updateDistTxid,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } else {
        console.log('\x1b[31m%s\x1b[0m', '❌ Failed to update DOGE_BTC distribution rate:', result.error);
        console.log('\x1b[33m%s\x1b[0m', '   This can be retried later once the Raydium pool is properly set up.');
        console.log('\x1b[33m%s\x1b[0m', '   The mining system will continue to work with the base distribution rate.');
    }
}

function printCompletionSummary() {
    console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ INITIALIZATION COMPLETE ================================');
    console.log('\x1b[32m%s\x1b[0m', '✅ All systems initialized successfully!');
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Summary:');
    console.log('\x1b[36m%s\x1b[0m', `  • MoonBase Program: ${deploymentFile.moonbase_program_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Mining System: ${deploymentFile.mining_vault_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Referral System: ${deploymentFile.referral_system_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Config Stores: ${deploymentFile.config_stores_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Loot & Stats: ${deploymentFile.loot_rewards_initialized && deploymentFile.level_stats_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • PvP Matchmaker: ${deploymentFile.pvp_matchmaker_initialized ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Factions: ${deploymentFile.factions_added ? config.factions.length + ' added ✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Expansions: ${deploymentFile.expansions_added ? config.expansions.length + ' added ✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Modules: ${deploymentFile.modules_added ? config.modules.length + ' added ✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Command Centers: ${deploymentFile.command_centers_added ? config.command_centers.length + ' added ✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Mining Tokens: ${deploymentFile.mining_tokens_deposited ? '✅' : '❌'}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Distribution Rate: ${deploymentFile.dbtc_dist_per_slot_updated ? '✅' : '⚠️ Skipped (requires Raydium pool)'}`);
    console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
    
    if (deploymentFile.moonbase_program_initialized) {
        console.log('\x1b[90m%s\x1b[0m', '\n🔑 Important Addresses:');
        console.log('\x1b[90m%s\x1b[0m', `   Global Config: ${deploymentFile.moonbase_program_initialized.globalConfig_address}`);
        console.log('\x1b[90m%s\x1b[0m', `   Mining State: ${deploymentFile.moonbase_program_initialized.dogeBtcMining_address}`);
        console.log('\x1b[90m%s\x1b[0m', `   SOL Treasury: ${deploymentFile.moonbase_program_initialized.solTreasury_address}`);
        if (deploymentFile.mining_vault_initialized) {
            console.log('\x1b[90m%s\x1b[0m', `   Mining Vault: ${deploymentFile.mining_vault_initialized.vault_address}`);
        }
    }
}

// ==================== [ ADMIN FUNCTIONS ] ====================
// 
// All admin functions now accept flexible parameters instead of hardcoded values:
//
// 🔧 updateGlobalConfig(program, newAuthority, newFeeCollector, newCreationFeeRecipient, newBaseCreationCost, newLootPercentage)
// 🎮 toggleGameActive(program) - Toggles current PvP state
// ⚡ updateSlotsForSwap(program, newSlotsForSwap) - Set mining rate control
// 🏭 updateModuleConfig(program, moduleId, newImageUrl, newFactionIds, newMaxPerBase, newMintCost, newUpgradeCost, newUpgradeLevelRequirements, isActive)
// 📊 adminUtilities(program) - View current system state
//

async function updateGlobalConfig(
    moonbaseProgram,
    newAuthority = null,
    newFeeCollector = null,
    newCreationFeeRecipient = null,
    newBaseCreationCost = null,
    newLootPercentage = null
) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING GLOBAL CONFIG ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    
    // Log what will be updated
    console.log('\x1b[36m%s\x1b[0m', '📝 Configuration Changes:');
    if (newAuthority) console.log('\x1b[36m%s\x1b[0m', `   🔑 New Authority: ${newAuthority}`);
    if (newFeeCollector) console.log('\x1b[36m%s\x1b[0m', `   💰 New Fee Collector: ${newFeeCollector}`);
    if (newCreationFeeRecipient) console.log('\x1b[36m%s\x1b[0m', `   🎯 New Creation Fee Recipient: ${newCreationFeeRecipient}`);
    if (newBaseCreationCost) console.log('\x1b[36m%s\x1b[0m', `   💎 New Base Creation Cost: ${newBaseCreationCost / 1e9} SOL`);
    if (newLootPercentage !== null) console.log('\x1b[36m%s\x1b[0m', `   🎁 New Loot Percentage: ${newLootPercentage}%`);
    
    const result = await updateGlobalConfigHelper(
        connection, moonbaseProgram, wallet, walletKeypair,
        globalConfigPDA, newAuthority, newFeeCollector, newCreationFeeRecipient,
        newBaseCreationCost, newLootPercentage
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Global config updated successfully!');
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
    } else {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update global config:', result.error);
    }
}

async function toggleGameActive(moonbaseProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ TOGGLING GAME ACTIVE STATE ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    
    const result = await toggleGameActiveHelper(
        connection, moonbaseProgram, wallet, walletKeypair, globalConfigPDA
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Game active state toggled successfully!');
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
        console.log('\x1b[36m%s\x1b[0m', `   PvP Games are now: ${result.data.isGameActive ? '🟢 ENABLED' : '🔴 DISABLED'}`);
    } else {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to toggle game active state:', result.error);
    }
}

async function updateSlotsForSwap(moonbaseProgram, newSlotsForSwap) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING SLOTS FOR SWAP ] ================');
    
    if (!newSlotsForSwap) {
        console.error('\x1b[31m%s\x1b[0m', '❌ newSlotsForSwap parameter is required');
        return;
    }
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    
    console.log('\x1b[36m%s\x1b[0m', `📝 Updating slots for swap to: ${newSlotsForSwap}`);
    console.log('\x1b[36m%s\x1b[0m', `   Multiplier vs default (9000): ${(newSlotsForSwap/9000).toFixed(2)}x`);
    
    const result = await updateSlotsForSwapHelper(
        connection, moonbaseProgram, wallet, walletKeypair,
        globalConfigPDA, dogeBtcMiningPDA, newSlotsForSwap
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', '✅ Slots for swap updated successfully!');
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
        console.log('\x1b[36m%s\x1b[0m', `   New slots for swap: ${result.data.newSlotsForSwap}`);
    } else {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update slots for swap:', result.error);
    }
}

async function updateModuleConfig(
    moonbaseProgram,
    moduleId,
    newImageUrl = null,
    newFactionIds = null,
    newMaxPerBase = null,
    newMintCost = null,
    newUpgradeCost = null,
    newUpgradeLevelRequirements = null,
    isActive = null
) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ UPDATING MODULE CONFIG ] ================');
    
    if (!moduleId) {
        console.error('\x1b[31m%s\x1b[0m', '❌ moduleId parameter is required');
        return;
    }
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    
    // Log what will be updated
    console.log('\x1b[36m%s\x1b[0m', `📝 Updating Module ID: ${moduleId}`);
    console.log('\x1b[36m%s\x1b[0m', '   Changes to apply:');
    if (newImageUrl) console.log('\x1b[36m%s\x1b[0m', `     🖼️  New Image URL: ${newImageUrl}`);
    if (newFactionIds) console.log('\x1b[36m%s\x1b[0m', `     🏛️  New Faction IDs: [${newFactionIds.join(', ')}]`);
    if (newMaxPerBase) console.log('\x1b[36m%s\x1b[0m', `     🏭 New Max Per Base: ${newMaxPerBase}`);
    if (newMintCost) console.log('\x1b[36m%s\x1b[0m', `     💰 New Mint Cost: ${newMintCost / 1e9} SOL`);
    if (newUpgradeCost) console.log('\x1b[36m%s\x1b[0m', `     ⬆️  New Upgrade Cost: ${newUpgradeCost / 1e9} SOL`);
    if (newUpgradeLevelRequirements) console.log('\x1b[36m%s\x1b[0m', `     📈 New Level Requirements: [${newUpgradeLevelRequirements.join(', ')}]`);
    if (isActive !== null) console.log('\x1b[36m%s\x1b[0m', `     ✅ Active Status: ${isActive ? 'ENABLED' : 'DISABLED'}`);
    
    const result = await updateModuleConfigHelper(
        connection, moonbaseProgram, wallet, walletKeypair,
        globalConfigPDA, moduleId, newImageUrl, newFactionIds, newMaxPerBase,
        newMintCost, newUpgradeCost, newUpgradeLevelRequirements, isActive
    );

    if (result.success) {
        console.log('\x1b[32m%s\x1b[0m', `✅ Module ${moduleId} updated successfully!`);
        console.log('\x1b[90m%s\x1b[0m', `   Transaction: ${result.data.txid}`);
    } else {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to update module config:', result.error);
    }
}

async function adminUtilities(moonbaseProgram) {
    console.log('\x1b[35m%s\x1b[0m', '\n================ [ ADMIN UTILITIES ] ================');
    
    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    
    const result = await getSystemStatus(moonbaseProgram, globalConfigPDA, dogeBtcMiningPDA);
    
    if (result.success) {
        const data = result.data;
        console.log('\x1b[36m%s\x1b[0m', '\n📊 Current System State:');
        console.log('\x1b[36m%s\x1b[0m', `   🎮 PvP Games: ${data.isGameActive ? '🟢 ENABLED' : '🔴 DISABLED'}`);
        console.log('\x1b[36m%s\x1b[0m', `   💰 Base Creation Cost: ${data.baseCreationCost / 1e9} SOL`);
        console.log('\x1b[36m%s\x1b[0m', `   🎁 Loot Percentage: ${data.lootPercentage}%`);
        console.log('\x1b[36m%s\x1b[0m', `   🏭 Total Moonbases Created: ${data.totalMoonbasesCreated}`);
        console.log('\x1b[36m%s\x1b[0m', `   💎 Total SOL Spent: ${data.totalSolSpent / 1e9} SOL`);
        console.log('\x1b[36m%s\x1b[0m', `   🔧 Active Hashpower: ${data.totalActiveHashpower}`);
        console.log('\x1b[36m%s\x1b[0m', `   ⚡ Active Electricity: ${data.totalActiveElectricity}`);
        console.log('\x1b[36m%s\x1b[0m', `   🪙 Tokens Mined: ${data.totalTokensMined / 1e6} MDOGE`);
        console.log('\x1b[36m%s\x1b[0m', `   📈 Current Distribution Rate: ${data.currentDistRate / 1e6} MDOGE/slot`);
        console.log('\x1b[36m%s\x1b[0m', `   🔄 Slots for Swap: ${data.slotsForSwap}`);
        console.log('\x1b[36m%s\x1b[0m', `   🎯 Supported Factions: ${data.supportedFactions.length}`);
        data.supportedFactions.forEach((faction, index) => {
            console.log('\x1b[90m%s\x1b[0m', `       ${index}: ${faction}`);
        });
    } else {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to fetch system state:', result.error);
    }
}
 
 

 

 
// Run the main script
main().catch(console.error);