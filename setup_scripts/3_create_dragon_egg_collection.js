/**
 * ============================================================================
 * DRAGON EGG NFT COLLECTION CREATION SCRIPT
 * ============================================================================
 * 
 * This script creates the Metaplex Core NFT collection for Dragon Eggs
 * and configures the MoonBase program to use it.
 * 
 * Steps:
 * 1. Create Metaplex Core collection for Dragon Eggs
 * 2. Set collection address in MoonBase program
 * 3. Add Dragon Egg URIs to the URI pool
 * 
 * Configuration Source: setup_scripts/config.json
 * State Management: setup_scripts/deployments/{cluster}.json
 * 
 * @requires @metaplex-foundation/mpl-core
 * @requires @solana/web3.js
 * @requires @coral-xyz/anchor
 * ============================================================================
 */

import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js';
import { 
    createCollectionV1, 
    fetchCollectionV1,
    mplCore
} from '@metaplex-foundation/mpl-core';
import { generateSigner, signerIdentity, createSignerFromKeypair } from '@metaplex-foundation/umi';
import { createUmi } from '@metaplex-foundation/umi-bundle-defaults';
import { fromWeb3JsKeypair, toWeb3JsPublicKey, fromWeb3JsPublicKey } from '@metaplex-foundation/umi-web3js-adapters';
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, Program, Wallet } = pkg;
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Color constants for consistent logging
const COLOR_STEP = '\x1b[35m%s\x1b[0m';
const COLOR_INFO = '\x1b[36m%s\x1b[0m';
const COLOR_SUCCESS = '\x1b[32m%s\x1b[0m';
const COLOR_WARNING = '\x1b[33m%s\x1b[0m';
const COLOR_ERROR = '\x1b[31m%s\x1b[0m';
const COLOR_DIM = '\x1b[90m%s\x1b[0m';

// MPL Core Program ID
const MPL_CORE_PROGRAM_ID = new PublicKey('CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d');

// ============================================================================
// ========== MAIN SCRIPT ====================================================
// ============================================================================

(async () => {
    console.log(COLOR_STEP, '\n🚀 ========== DRAGON EGG COLLECTION CREATION ==========');
    console.log(COLOR_INFO, '🌐 Network:', CLUSTER);
    console.log(COLOR_INFO, '🔗 RPC URL:', RPC_URL);
    console.log(COLOR_INFO, '🎨 Collection Type: Metaplex Core');

    // Validate configuration
    validateConfiguration();

    // Setup connection and deployer
    const connection = await initializeConnection();
    const deployer = await setupDeployerAccount(connection);
    
    // Load deployment state
    const { deploymentData, deploymentPath } = loadDeploymentState();
    
    // Validate prerequisites
    validatePrerequisites(deploymentData);
    
    try {
        // 1. Create Dragon Egg Collection
        const collectionAddress = await createDragonEggCollection(connection, deployer, deploymentData, deploymentPath);
        
        // // 2. Set collection in MoonBase program
        // await setCollectionInMoonBase(connection, deployer, deploymentData, deploymentPath, collectionAddress);
        
        // // 3. Add Dragon Egg URIs
        // await addDragonEggUris(connection, deployer, deploymentData, deploymentPath);
        
        // // Print completion summary
        // printCompletionSummary(deploymentData);
        
        console.log(COLOR_SUCCESS, '\n✅ Dragon Egg collection setup complete!');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Collection creation failed:', error.message);
        console.error(COLOR_ERROR, '❌ Error stack:', error.stack);
        process.exit(1);
    }
})();

// ============================================================================
// ========== HELPER FUNCTIONS ===============================================
// ============================================================================

/**
 * Validates all required configuration parameters from config.json
 */
function validateConfiguration() {
    console.log(COLOR_WARNING, '🔍 Validating configuration...');
    
    const errors = [];
    
    // Network configuration
    if (!config.network?.cluster) {
        errors.push('network.cluster is required');
    }
    if (!config.network?.rpc_url) {
        errors.push('network.rpc_url is required');
    }
    
    // Dragon Egg configuration
    if (!config.dragon_eggs) {
        errors.push('dragon_eggs configuration is missing');
    } else {
        if (!config.dragon_eggs.collection_name) {
            errors.push('dragon_eggs.collection_name is required');
        }
        if (!config.dragon_eggs.collection_uri) {
            errors.push('dragon_eggs.collection_uri is required');
        }
        if (!config.dragon_eggs.uris || !Array.isArray(config.dragon_eggs.uris)) {
            errors.push('dragon_eggs.uris must be an array');
        }
    }
    
    // Deployment paths
    if (!config.deployment?.paths?.deployer_key) {
        errors.push('deployment.paths.deployer_key is required');
    }
    if (!config.deployment?.paths?.moonbase_idl) {
        errors.push('deployment.paths.moonbase_idl is required');
    }
    
    if (errors.length > 0) {
        console.error(COLOR_ERROR, '❌ Configuration validation failed:');
        errors.forEach(error => console.error(COLOR_ERROR, `   • ${error}`));
        console.log(COLOR_WARNING, '⚠️ Please check your config.json file');
        process.exit(1);
    }
    
    console.log(COLOR_SUCCESS, '✅ Configuration validated successfully');
}

/**
 * Initializes connection to Solana RPC with retry logic
 */
async function initializeConnection() {
    console.log(COLOR_WARNING, '🔄 Initializing connection...');
    
    let connection;
    let retries = 3;
    
    while (retries > 0) {
        try {
            connection = new Connection(RPC_URL, COMMITMENT);
            await connection.getVersion();
            console.log(COLOR_SUCCESS, '✅ Successfully connected to Solana network');
            return connection;
        } catch (error) {
            retries--;
            if (retries === 0) {
                throw new Error(`Failed to connect to ${RPC_URL}: ${error.message}`);
            }
            console.log(COLOR_WARNING, `⚠️ Connection failed, retrying... (${retries} attempts left)`);
            await new Promise(resolve => setTimeout(resolve, 2000));
        }
    }
    
    return connection;
}

/**
 * Loads and validates the deployer account
 */
async function setupDeployerAccount(connection) {
    console.log(COLOR_WARNING, '🔄 Setting up deployer account...');
    
    const deployerPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
    let deployer;
    
    try {
        if (fs.existsSync(deployerPath)) {
            console.log(COLOR_INFO, '📂 Loading existing deployer account...');
            const deployerData = JSON.parse(fs.readFileSync(deployerPath, 'utf8'));
            deployer = Keypair.fromSecretKey(new Uint8Array(deployerData));
            
            const balance = await connection.getBalance(deployer.publicKey);
            console.log(COLOR_SUCCESS, '✅ Deployer account loaded successfully!');
            console.log(COLOR_INFO, '👤 Deployer Address:', deployer.publicKey.toString());
            console.log(COLOR_INFO, '💰 Deployer Balance:', balance / 1e9, 'SOL');
            
            if (balance < 0.1 * 1e9) {
                console.error(COLOR_ERROR, '❌ Insufficient balance. Need at least 0.1 SOL for collection creation.');
                process.exit(1);
            }
        } else {
            throw new Error(`Deployer keypair not found at: ${deployerPath}`);
        }
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Error loading deployer:', error.message);
        throw error;
    }
    
    return deployer;
}

/**
 * Loads existing deployment state
 */
function loadDeploymentState() {
    console.log(COLOR_WARNING, '📋 Loading deployment state...');
    
    const deploymentDir = path.resolve(__dirname, config.deployment.paths.deployments_dir);
    const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);
    
    if (!fs.existsSync(deploymentPath)) {
        console.error(COLOR_ERROR, '❌ Deployment file not found. Please run previous deployment scripts first.');
        process.exit(1);
    }
    
    const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    console.log(COLOR_SUCCESS, '✅ Deployment data loaded successfully');
    
    return { deploymentData, deploymentPath };
}

/**
 * Validates that all prerequisites are met
 */
function validatePrerequisites(deploymentData) {
    console.log(COLOR_WARNING, '🔍 Validating prerequisites...');
    
    const errors = [];
    
    // if (!deploymentData.MOON_BASE_PROGRAM_ID) {
    //     errors.push('MoonBase program not deployed - run 0_deploy_game.js first');
    // }
    
    // if (!deploymentData.moonbase_program_initialized) {
    //     errors.push('MoonBase program not initialized - run 3_init_moonbase.js first');
    // }
    
    if (errors.length > 0) {
        console.error(COLOR_ERROR, '❌ Prerequisites not met:');
        errors.forEach(error => console.error(COLOR_ERROR, `   • ${error}`));
        console.log(COLOR_WARNING, '⚠️ Please run previous deployment scripts first.');
        process.exit(1);
    }
    
    console.log(COLOR_SUCCESS, '✅ All prerequisites validated');
}

/**
 * Creates the Dragon Egg NFT collection using Metaplex Core
 */
async function createDragonEggCollection(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.dragon_egg_collection_created) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg collection already created');
        console.log(COLOR_INFO, '🔑 Collection Address:', deploymentData.dragon_egg_collection_created.collection_address);
        return new PublicKey(deploymentData.dragon_egg_collection_created.collection_address);
    }

    console.log(COLOR_STEP, '\n=================== [ CREATING DRAGON EGG COLLECTION ] ===================');
    
    try {
        // Create UMI instance
        const umi = createUmi(RPC_URL);
        
        // Convert web3.js keypair to UMI signer
        const umiKeypair = umi.eddsa.createKeypairFromSecretKey(deployer.secretKey);
        const umiSigner = createSignerFromKeypair(umi, umiKeypair);
        
        umi.use(signerIdentity(umiSigner));
        umi.use(mplCore());
        
        console.log(COLOR_INFO, '🎨 Creating Metaplex Core collection...');
        console.log(COLOR_DIM, `   Name: ${config.dragon_eggs.collection_name}`);
        console.log(COLOR_DIM, `   URI: ${config.dragon_eggs.collection_uri}`);
        
        // Generate collection address
        const collection = generateSigner(umi);
        
        // Create the collection
        await createCollectionV1(umi, {
            collection,
            name: config.dragon_eggs.collection_name,
            uri: config.dragon_eggs.collection_uri,
        }).sendAndConfirm(umi);
        
        const collectionPubkey = toWeb3JsPublicKey(collection.publicKey);
        
        console.log(COLOR_SUCCESS, '✅ Dragon Egg collection created successfully!');
        console.log(COLOR_INFO, '🔑 Collection Address:', collectionPubkey.toString());
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/address/${collectionPubkey.toString()}?cluster=${CLUSTER}`);
        
        // Verify collection was created
        try {
            const collectionData = await fetchCollectionV1(umi, collection.publicKey);
            console.log(COLOR_SUCCESS, '✅ Collection verified on-chain');
            console.log(COLOR_DIM, `   Update Authority: ${collectionData.updateAuthority}`);
            console.log(COLOR_DIM, `   Num Minted: ${collectionData.numMinted}`);
            console.log(COLOR_DIM, `   Current Size: ${collectionData.currentSize}`);
        } catch (error) {
            console.log(COLOR_WARNING, '⚠️ Could not verify collection:', error.message);
        }
        
        // Save to deployment data
        deploymentData.dragon_egg_collection_created = {
            collection_address: collectionPubkey.toString(),
            collection_name: config.dragon_eggs.collection_name,
            collection_uri: config.dragon_eggs.collection_uri,
            update_authority: deployer.publicKey.toString(),
            timestamp: new Date().toISOString()
        };
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        
        return collectionPubkey;
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to create collection:', error);
        throw error;
    }
}

/**
 * Sets the Dragon Egg collection address in the MoonBase program
 */
async function setCollectionInMoonBase(connection, deployer, deploymentData, deploymentPath, collectionAddress) {
    if (deploymentData.dragon_egg_collection_set_in_program) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg collection already set in MoonBase program');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ SETTING COLLECTION IN MOONBASE ] ===================');
    
    try {
        // Load MoonBase program
        const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
        if (!fs.existsSync(moonbaseIdlPath)) {
            throw new Error(`MoonBase IDL not found at: ${moonbaseIdlPath}`);
        }
        
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const wallet = new Wallet(deployer);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
        const moonbaseProgram = new Program(moonbaseIdl, provider);
        
        console.log(COLOR_INFO, '🔑 MoonBase Program:', moonbaseProgram.programId.toString());
        console.log(COLOR_INFO, '🎨 Collection Address:', collectionAddress.toString());
        
        // Derive Global Config PDA
        const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('global_config')],
            moonbaseProgram.programId
        );
        
        console.log(COLOR_DIM, '🔍 Global Config PDA:', globalConfigPDA.toString());
        console.log(COLOR_INFO, '📡 Calling set_dragon_egg_collection...');
        
        // Call the program instruction
        const txid = await moonbaseProgram.methods
            .setDragonEggCollection(collectionAddress)
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigStore: null,
                dogeBtcMining: null,
                authority: deployer.publicKey,
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

/**
 * Adds Dragon Egg URIs to the MoonBase program
 */
async function addDragonEggUris(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.dragon_egg_uris_added) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg URIs already added');
        console.log(COLOR_INFO, `🔑 Total URIs: ${deploymentData.dragon_egg_uris_added.uris_count}`);
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ ADDING DRAGON EGG URIS ] ===================');
    
    try {
        // Load MoonBase program
        const moonbaseIdlPath = path.resolve(__dirname, config.deployment.paths.moonbase_idl);
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const wallet = new Wallet(deployer);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
        const moonbaseProgram = new Program(moonbaseIdl, provider);
        
        // Derive Global Config PDA
        const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('global_config')],
            moonbaseProgram.programId
        );
        
        console.log(COLOR_INFO, `🎨 Adding ${config.dragon_eggs.uris.length} Dragon Egg URIs...`);
        
        // Add URIs (Metaplex URIs that point to Dragon Egg metadata)
        const txid = await moonbaseProgram.methods
            .addDragonEggUris(config.dragon_eggs.uris)
            .accounts({
                globalConfig: globalConfigPDA,
                moduleConfigStore: null,
                dogeBtcMining: null,
                authority: deployer.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();
        
        console.log(COLOR_SUCCESS, '✅ Dragon Egg URIs added successfully!');
        console.log(COLOR_DIM, '🔗 Transaction:', txid);
        console.log(COLOR_DIM, `   URIs added: ${config.dragon_eggs.uris.length}`);
        
        // Save to deployment data
        deploymentData.dragon_egg_uris_added = {
            uris_count: config.dragon_eggs.uris.length,
            uris: config.dragon_eggs.uris,
            tx_signature: txid,
            timestamp: new Date().toISOString()
        };
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to add URIs:', error);
        throw error;
    }
}

/**
 * Prints completion summary
 */
function printCompletionSummary(deploymentData) {
    console.log(COLOR_STEP, '\n🎉 ========== DRAGON EGG COLLECTION SETUP COMPLETE ==========');
    console.log(COLOR_SUCCESS, '✅ All Dragon Egg collection configuration completed!');
    
    console.log(COLOR_INFO, '\n📋 Summary:');
    console.log(COLOR_INFO, `  • Network: ${CLUSTER}`);
    
    if (deploymentData.dragon_egg_collection_created) {
        console.log(COLOR_INFO, `  • Collection Address: ${deploymentData.dragon_egg_collection_created.collection_address}`);
        console.log(COLOR_INFO, `  • Collection Name: ${deploymentData.dragon_egg_collection_created.collection_name}`);
        console.log(COLOR_INFO, `  • Update Authority: ${deploymentData.dragon_egg_collection_created.update_authority}`);
    }
    
    if (deploymentData.dragon_egg_uris_added) {
        console.log(COLOR_INFO, `  • Dragon Egg URIs: ${deploymentData.dragon_egg_uris_added.uris_count} added`);
    }
    
    console.log(COLOR_INFO, '\n🔗 Next Steps:');
    console.log(COLOR_WARNING, '  1. Users can now create moonbases with Dragon Egg NFTs');
    console.log(COLOR_WARNING, '  2. Each Dragon Egg will be minted from this collection');
    console.log(COLOR_WARNING, '  3. Dragon Eggs can be incubated in moonbases to gain power');
    console.log(COLOR_STEP, '============================================================\n');
}

// Run the script
// main().catch(console.error);

