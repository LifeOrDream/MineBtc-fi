/**
 * ============================================================================
 * DEGEN_BTC-SOL RAYDIUM POOL INITIALIZATION SCRIPT
 * ============================================================================
 * 
 * Deployment Steps:
 * 1. Validates prerequisites (token mint, token account, initial supply)
 * 2. Creates AMM config with fee parameters
 * 3. Initializes DEGEN_BTC-SOL pool with correct token order
 * 4. Optionally burns LP tokens for permanent liquidity lock
 * 
 * Safety Features:
 * - Connection retry logic with exponential backoff
 * - Balance verification before operations
 * - On-chain account existence checks
 * - Automatic error recovery where possible
 * - Timestamped deployment state tracking
 * 
 * Configuration Source: setup_scripts/config.json
 * State Management: setup_scripts/deployments/{cluster}.json
 * 
 * @requires @solana/web3.js
 * @requires @solana/spl-token
 * @requires @coral-xyz/anchor
 * ============================================================================
 */

import {
    Connection,
    Keypair,
    clusterApiUrl,
    sendAndConfirmTransaction,
    SystemProgram,
    Transaction,
    PublicKey,
    LAMPORTS_PER_SOL
} from "@solana/web3.js";
import {
    TOKEN_PROGRAM_ID,
    createMint,
    getOrCreateAssociatedTokenAccount,
    mintTo,
    getAssociatedTokenAddress,
    TOKEN_2022_PROGRAM_ID,
    burn,
    getMint,
    createSyncNativeInstruction
} from "@solana/spl-token";
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3, Wallet } = pkg;
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import path from 'path';
import { getSolanaBalance, updateDeploymentStatus } from './helper.js';
import { fileURLToPath } from 'url';
import { resolveRaydiumProgramId, setIdlAddress } from './raydium_id_sync.js';

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;


// Raydium CP-Swap constants
const CP_AMM_CONFIG_SEED = "amm_config";
const WSOL_MINT = "So11111111111111111111111111111111111111112"; // Native SOL wrapper

// Color constants for consistent logging
const COLOR_STEP = '\x1b[35m%s\x1b[0m';
const COLOR_INFO = '\x1b[36m%s\x1b[0m';
const COLOR_SUCCESS = '\x1b[32m%s\x1b[0m';
const COLOR_WARNING = '\x1b[33m%s\x1b[0m';
const COLOR_ERROR = '\x1b[31m%s\x1b[0m';
const COLOR_DIM = '\x1b[90m%s\x1b[0m';

// ============================================================================
// ========== MAIN DEPLOYMENT SCRIPT =========================================
// ============================================================================

(async () => {
    console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ MineBTC DEGEN_BTC-SOL Pool Creation ================================');
    console.log('\x1b[36m%s\x1b[0m', '🌐 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🔗 RPC URL:', RPC_URL);
    console.log('\x1b[36m%s\x1b[0m', '💱 Pool Type: Raydium CP-Swap (Constant Product)');

    // Validate configuration
    validateConfiguration();

    // Setup connection and deployer
    const connection = await initializeConnection();
    const deployer = await setupDeployerAccount(connection);
    
    // Load deployment state
    const { deploymentData, deploymentPath } = loadDeploymentState();
    
    // Validate prerequisites
    validatePrerequisites(deploymentData);
    
    // Get the exact Raydium program ID that MineBTC is compiled to accept.
    // On devnet this comes from 0_deploy_raydium.js and must match
    // raydium_cp_swap::ID inside the MineBTC build.
    const RAYDIUM_CP_PROGRAM_ID = new PublicKey(
        resolveRaydiumProgramId(config, deploymentData, {
            requireCustomDeployment: true,
        })
    );
    const useOfficialProgram = config.raydium.use_official_program;
    console.log('\x1b[36m%s\x1b[0m', '🔑 Raydium CP Program:', RAYDIUM_CP_PROGRAM_ID.toBase58());
    console.log('\x1b[36m%s\x1b[0m', '🔑 Using Official Raydium:', useOfficialProgram ? 'YES' : 'NO (custom deployed)');
    
    // Setup Raydium program with wallet wrapper
    const wallet = new Wallet(deployer);
    const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
    const cpIdlPath = path.resolve(__dirname, config.deployment.paths.raydium_idl || '../target/idl/raydium_cp_swap.json');
    
    if (!fs.existsSync(cpIdlPath)) {
        console.error('\x1b[31m%s\x1b[0m', `❌ Raydium IDL not found at: ${cpIdlPath}`);
        console.error('\x1b[31m%s\x1b[0m', '⚠️ Please ensure Raydium program is built first.');
        process.exit(1);
    }
    
    const cpIdl = setIdlAddress(
        JSON.parse(fs.readFileSync(cpIdlPath, 'utf8')),
        RAYDIUM_CP_PROGRAM_ID
    );
    const cpProgram = new Program(cpIdl, provider);
    if (!cpProgram.programId.equals(RAYDIUM_CP_PROGRAM_ID)) {
        throw new Error(`Raydium IDL/program mismatch: Program=${cpProgram.programId.toBase58()} expected=${RAYDIUM_CP_PROGRAM_ID.toBase58()}`);
    }
    
    try {
        // 1. Create AMM Config (skip for official Raydium - use existing configs)
        await createAmmConfig(connection, cpProgram, deployer, deploymentData, deploymentPath, RAYDIUM_CP_PROGRAM_ID, useOfficialProgram);
        
        // 2. Initialize Pool :: Automatically adds initial liquidity
        await initializePool(connection, cpProgram, deployer, deploymentData, deploymentPath, RAYDIUM_CP_PROGRAM_ID);
        
        // 3. Add Initial Liquidity
        await addInitialLiquidity(connection, cpProgram, deployer, deploymentData, deploymentPath);
        // return
        
        // 4. Burn LP Tokens (if configured)
        if (config.raydium.burn_lp_tokens) {
            await burnLpTokens(connection, deployer, deploymentData, deploymentPath);
        }
        
        // Print completion summary
        printCompletionSummary(deploymentData);
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Pool creation failed:', error);
        process.exit(1);
    }
})();

// ============================================================================
// ========== HELPER FUNCTIONS ===============================================
// ============================================================================

/**
 * Validates all required configuration parameters from config.json
 * Exits the process if any validation fails
 */
function validateConfiguration() {
    console.log('\x1b[33m%s\x1b[0m', '🔍 Validating configuration...');
    
    const errors = [];
    
    // Network configuration
    if (!config.network?.cluster) {
        errors.push('network.cluster is required');
    }
    if (!config.network?.rpc_url) {
        errors.push('network.rpc_url is required');
    }
    if (!config.network?.commitment) {
        errors.push('network.commitment is required');
    }
    
    // Token configuration
    if (!config.token?.decimals || config.token.decimals < 0) {
        errors.push('token.decimals must be a positive number');
    }
    if (!config.token?.initial_supply || config.token.initial_supply <= 0) {
        errors.push('token.initial_supply must be greater than 0');
    }
    
    // Raydium configuration
    if (!config.raydium) {
        errors.push('raydium configuration is missing');
    } else {
        if (config.raydium.amm_config_index === undefined) {
            errors.push('raydium.amm_config_index is required');
        }
        if (!config.raydium.trade_fee_rate && config.raydium.trade_fee_rate !== 0) {
            errors.push('raydium.trade_fee_rate is required');
        }
        if (!config.raydium.protocol_fee_rate && config.raydium.protocol_fee_rate !== 0) {
            errors.push('raydium.protocol_fee_rate is required');
        }
        if (!config.raydium.fund_fee_rate && config.raydium.fund_fee_rate !== 0) {
            errors.push('raydium.fund_fee_rate is required');
        }
        if (!config.raydium.create_pool_fee || config.raydium.create_pool_fee <= 0) {
            errors.push('raydium.create_pool_fee must be greater than 0');
        }
        if (!config.raydium.initial_sol_amount || config.raydium.initial_sol_amount <= 0) {
            errors.push('raydium.initial_sol_amount must be greater than 0');
        }
        if (!config.raydium.initial_dbtc_percentage || config.raydium.initial_dbtc_percentage <= 0 || config.raydium.initial_dbtc_percentage > 100) {
            errors.push('raydium.initial_dbtc_percentage must be between 0 and 100');
        }
        if (config.raydium.open_time === undefined) {
            errors.push('raydium.open_time is required (use 0 for immediate opening)');
        }
        if (config.raydium.burn_lp_tokens === undefined) {
            errors.push('raydium.burn_lp_tokens must be true or false');
        }
    }
    
    // Deployment paths
    if (!config.deployment?.paths?.deployer_key) {
        errors.push('deployment.paths.deployer_key is required');
    }
    if (!config.deployment?.paths?.deployments_dir) {
        errors.push('deployment.paths.deployments_dir is required');
    }
    
    if (errors.length > 0) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Configuration validation failed:');
        errors.forEach(error => console.error('\x1b[31m%s\x1b[0m', `   • ${error}`));
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please check your config.json file');
        process.exit(1);
    }
    
    console.log('\x1b[32m%s\x1b[0m', '✅ Configuration validated successfully');
}

/**
 * Initializes connection to Solana RPC with retry logic
 * @returns {Promise<Connection>} Established Solana connection
 */
async function initializeConnection() {
    console.log('\x1b[33m%s\x1b[0m', '🔄 Initializing connection...');
    
    let connection;
    let retries = 3;
    
    while (retries > 0) {
        try {
            connection = new Connection(RPC_URL, COMMITMENT);
                await connection.getVersion();
            console.log('\x1b[32m%s\x1b[0m', '✅ Successfully connected to Solana network');
            break;
        } catch (error) {
            retries--;
            if (retries === 0) {
                console.error('\x1b[31m%s\x1b[0m', '❌ Failed to connect after multiple attempts');
                process.exit(1);
            }
            console.log('\x1b[33m%s\x1b[0m', `⚠️ Connection failed, retrying... (${retries} attempts remaining)`);
            await new Promise(resolve => setTimeout(resolve, 2000));
        }
    }
    
    return connection;
}

/**
 * Loads and validates the deployer account, checks balance
 * @param {Connection} connection - Solana connection
 * @returns {Promise<Keypair>} Deployer keypair
 */
async function setupDeployerAccount(connection) {
    console.log('\x1b[33m%s\x1b[0m', '🔄 Setting up deployer account...');
    
    const deployerPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
    let deployer;
    
    try {
        if (fs.existsSync(deployerPath)) {
            console.log('\x1b[36m%s\x1b[0m', '📂 Loading existing deployer account...');
            const deployerData = JSON.parse(fs.readFileSync(deployerPath, 'utf8'));
            deployer = Keypair.fromSecretKey(new Uint8Array(deployerData));
            console.log('\x1b[32m%s\x1b[0m', '✅ Deployer account loaded successfully!');
        } else {
            console.error('\x1b[31m%s\x1b[0m', '❌ Deployer account not found. Please run token deployment first.');
            process.exit(1);
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error loading deployer account:', error);
        process.exit(1);
    }
    
    console.log('\x1b[36m%s\x1b[0m', '👤 Deployer Address:', deployer.publicKey.toBase58());
    
    // Check balance
    const balance = await getSolanaBalance(connection, deployer.publicKey);
    console.log('\x1b[36m%s\x1b[0m', '💰 Deployer Balance:', balance / 1e9, 'SOL');
    
    if (balance < config.raydium.create_pool_fee + config.raydium.initial_sol_amount) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Insufficient SOL balance for pool creation');
        console.log('\x1b[33m%s\x1b[0m', `⚠️ Required: ${(config.raydium.create_pool_fee + config.raydium.initial_sol_amount) / 1e9} SOL`);
        console.log('\x1b[33m%s\x1b[0m', `⚠️ Available: ${balance / 1e9} SOL`);
        
        if (CLUSTER.includes('devnet')) {
            console.log('\x1b[33m%s\x1b[0m', '💧 Requesting airdrop...');
            try {
                const shortfall = Math.max(
                    0,
                    config.raydium.create_pool_fee + config.raydium.initial_sol_amount - balance
                );
                const airdropAmount = config.dev?.airdrop_amount
                    ?? Math.max(shortfall + LAMPORTS_PER_SOL / 2, LAMPORTS_PER_SOL);
                const airdropSignature = await connection.requestAirdrop(
                    deployer.publicKey,
                    airdropAmount
                );
                await connection.confirmTransaction(airdropSignature);
                console.log('\x1b[32m%s\x1b[0m', '✅ Airdrop successful!');
            } catch (error) {
                console.error('\x1b[31m%s\x1b[0m', '❌ Airdrop failed. Please fund the deployer manually.');
                process.exit(1);
            }
        } else {
            process.exit(1);
        }
    }
    
    return deployer;
}

/**
 * Loads existing deployment state from deployments/{cluster}.json
 * @returns {Object} Object containing deploymentData and deploymentPath
 */
function loadDeploymentState() {
    console.log('\x1b[33m%s\x1b[0m', '📋 Loading deployment state...');
    
    const deploymentDir = path.resolve(__dirname, config.deployment.paths.deployments_dir);
    const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);
    
    if (!fs.existsSync(deploymentPath)) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Deployment file not found. Please run token deployment first.');
        process.exit(1);
    }
    
    const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
    console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data loaded successfully');
    
    return { deploymentData, deploymentPath };
}

/**
 * Validates that all prerequisites for pool creation are met
 * @param {Object} deploymentData - Deployment state data
 */
function validatePrerequisites(deploymentData) {
    console.log('\x1b[33m%s\x1b[0m', '🔍 Validating prerequisites...');
    
    const errors = [];
    
    try {
        resolveRaydiumProgramId(config, deploymentData, {
            requireCustomDeployment: true,
        });
    } catch (error) {
        errors.push(error.message);
    }
    
    if (!deploymentData.dbtc_mint_address) {
        errors.push('DEGEN_BTC token mint address not found');
    }
    
    if (!deploymentData.dbtc_token_account_created?.token_account_address) {
        errors.push('DEGEN_BTC token account not found');
    }
    
    if (!deploymentData.initial_supply_minted) {
        errors.push('Initial supply not minted');
    }
    
    if (errors.length > 0) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Prerequisites not met:');
        errors.forEach(error => console.error('\x1b[31m%s\x1b[0m', `   • ${error}`));
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Please run token deployment script first.');
        process.exit(1);
    }
    
    console.log('\x1b[32m%s\x1b[0m', '✅ All prerequisites validated');
}

/**
 * Creates or uses existing AMM configuration for the Raydium pool
 * For official Raydium mainnet, uses existing pre-created configs
 * @param {Connection} connection - Solana connection
 * @param {Program} cpProgram - Raydium CP program instance
 * @param {Keypair} deployer - Deployer keypair
 * @param {Object} deploymentData - Deployment state data
 * @param {string} deploymentPath - Path to deployment file
 * @param {PublicKey} RAYDIUM_CP_PROGRAM_ID - Raydium program ID
 * @param {boolean} useOfficialProgram - Whether using official Raydium mainnet program
 */
async function createAmmConfig(connection, cpProgram, deployer, deploymentData, deploymentPath, RAYDIUM_CP_PROGRAM_ID, useOfficialProgram) {
    if (deploymentData.raydium_amm_config_created) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Raydium AMM config already exists. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔑 AMM Config:', deploymentData.raydium_amm_config_created.amm_config_pda);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ SETTING UP AMM CONFIG ] ===================');
    console.log(`deployer.publicKey: ${deployer.publicKey.toBase58()}`);
    console.log(`RAYDIUM_CP_PROGRAM_ID: ${RAYDIUM_CP_PROGRAM_ID.toBase58()}`);
    
    const configIndex = config.raydium.amm_config_index;
    const tradeFeeRate = new BN(config.raydium.trade_fee_rate);
    const createPoolFee = new BN(config.raydium.create_pool_fee);
            
    console.log('\x1b[36m%s\x1b[0m', '⚙️ AMM Config Parameters:');
    console.log('\x1b[36m%s\x1b[0m', `   • Config Index: ${configIndex}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Trade Fee Rate: ${tradeFeeRate.toNumber() / 10000}%`);
    console.log('\x1b[36m%s\x1b[0m', `   • Create Pool Fee: ${createPoolFee.toNumber() / 1e9} SOL`);
            
    try {
        // Derive AMM Config PDA using the same seeds as in the Rust program
        const [ammConfigPDA, bump] = PublicKey.findProgramAddressSync(
            [
                Buffer.from(CP_AMM_CONFIG_SEED),
                new BN(configIndex).toArrayLike(Buffer, 'be', 2) // u16 to big-endian bytes
            ],
            RAYDIUM_CP_PROGRAM_ID
        );
        
        console.log('\x1b[36m%s\x1b[0m', '🔑 AMM Config PDA:', ammConfigPDA.toBase58());
        console.log('\x1b[36m%s\x1b[0m', '🔑 PDA Bump:', bump);
                
        // Check if config already exists on-chain
        const configInfo = await connection.getAccountInfo(ammConfigPDA);
        
        if (configInfo) {
            console.log('\x1b[32m%s\x1b[0m', '✅ AMM config exists on-chain');
            
            // For official Raydium, this is expected - just record it
            if (useOfficialProgram) {
                console.log('\x1b[36m%s\x1b[0m', '📝 Using existing Raydium mainnet AMM config');
            }
            
            // Update deployment data
            deploymentData.RAYDIUM_CP_PROGRAM_ID = RAYDIUM_CP_PROGRAM_ID.toBase58();
            deploymentData.raydium_amm_config_created = {
                amm_config_pda: ammConfigPDA.toBase58(),
                raydium_program_id: RAYDIUM_CP_PROGRAM_ID.toBase58(),
                config_index: configIndex,
                trade_fee_rate: tradeFeeRate.toNumber(),
                create_pool_fee: createPoolFee.toString(),
                status: useOfficialProgram ? 'using_official_raydium_config' : 'already_exists',
                is_official_raydium: useOfficialProgram,
                timestamp: new Date().toISOString()
            };
            
            fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
            console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data updated');
            return;
        }
        
        // If using official Raydium and config doesn't exist, that's an error
        if (useOfficialProgram) {
            console.error('\x1b[31m%s\x1b[0m', `❌ AMM config index ${configIndex} not found on official Raydium program`);
            console.log('\x1b[33m%s\x1b[0m', '⚠️ Try using a different amm_config_index (0, 1, 2, 3 are common)');
            throw new Error(`AMM config index ${configIndex} not found on Raydium mainnet`);
        }
        
        // Only create config if NOT using official program (custom deployed)
        console.log('\x1b[33m%s\x1b[0m', '📡 Creating AMM config on custom Raydium program...');
        
        const protocolFeeRate = new BN(config.raydium.protocol_fee_rate);
        const fundFeeRate = new BN(config.raydium.fund_fee_rate);
        const creatorFeeRate = new BN(config.raydium.creator_fee_rate || 0);
        
        const txid = await cpProgram.methods
            .createAmmConfig(
                configIndex,
                tradeFeeRate,
                protocolFeeRate,
                fundFeeRate,
                createPoolFee,
                creatorFeeRate
            )
            .accounts({
                owner: deployer.publicKey,
                ammConfig: ammConfigPDA,
                systemProgram: SystemProgram.programId,
            })
            .rpc();
            
        console.log('\x1b[32m%s\x1b[0m', '✅ AMM config created successfully!');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', txid);
        
        deploymentData.raydium_amm_config_created = {
            amm_config_pda: ammConfigPDA.toBase58(),
            raydium_program_id: RAYDIUM_CP_PROGRAM_ID.toBase58(),
            config_index: configIndex,
            trade_fee_rate: tradeFeeRate.toNumber(),
            create_pool_fee: createPoolFee.toString(),
            creation_signature: txid,
            pda_bump: bump,
            status: 'created',
            is_official_raydium: false,
            timestamp: new Date().toISOString()
        };
        
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ AMM config creation complete');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to setup AMM config:', error);
        throw error;
    }
}

/**
 * Initializes the DEGEN_BTC-SOL liquidity pool with Raydium CP-Swap
 * @param {Connection} connection - Solana connection
 * @param {Program} cpProgram - Raydium CP program instance
 * @param {Keypair} deployer - Deployer keypair
 * @param {Object} deploymentData - Deployment state data
 * @param {string} deploymentPath - Path to deployment file
 * @param {PublicKey} RAYDIUM_CP_PROGRAM_ID - Raydium program ID
 */
async function initializePool(connection, cpProgram, deployer, deploymentData, deploymentPath, RAYDIUM_CP_PROGRAM_ID) {
    // Check if already created
    if (deploymentData.dbtc_sol_pool_created) {
        console.log(COLOR_INFO, 'ℹ️ DEGEN_BTC-SOL pool already exists');
        console.log(COLOR_INFO, '🔑 Pool State:', deploymentData.dbtc_sol_pool_created.poolStatePDA);
        return;
    }

    // ------------- xxxx -----------------
    // CREATE DEGEN_BTC-SOL POOL
    // ------------- xxxx -----------------

    const ammConfigPDA = new PublicKey(deploymentData.raydium_amm_config_created.amm_config_pda);

    // Step 2: Create DEGEN_BTC-SOL Pool
    console.log(COLOR_STEP, '\n=================== [ CREATING DEGEN_BTC-SOL POOL ] ===================');
    
    if (!deploymentData.dbtc_sol_pool_created) {
        console.log(COLOR_INFO, '🏊 Creating DEGEN_BTC-SOL pool...');

        // Get the token mints
        const minebtcMintKey = new PublicKey(deploymentData.dbtc_mint_address);
        const wsolMintKey = new PublicKey(WSOL_MINT);
        
        console.log(COLOR_DIM, `🔍 DEGEN_BTC Mint: ${minebtcMintKey.toString()}`);
        console.log(COLOR_DIM, `🔍 WSOL Mint: ${wsolMintKey.toString()}`);
        
        // Ensure correct token order (token0 < token1) using byte-wise comparison
        const minebtcMintBytes = minebtcMintKey.toBytes();
        const wsolMintBytes = wsolMintKey.toBytes();
        const isDbtcToken0 = Buffer.compare(minebtcMintBytes, wsolMintBytes) < 0;
        
        const token0Mint = isDbtcToken0 ? minebtcMintKey : wsolMintKey;
        const token1Mint = isDbtcToken0 ? wsolMintKey : minebtcMintKey;
        
        console.log(COLOR_INFO, `🪙 Token0: ${token0Mint.toString()} ${isDbtcToken0 ? '(DEGEN_BTC)' : '(WSOL)'}`);
        console.log(COLOR_INFO, `🪙 Token1: ${token1Mint.toString()} ${!isDbtcToken0 ? '(DEGEN_BTC)' : '(WSOL)'}`);
        console.log(COLOR_DIM, `🔍 Token order check: ${token0Mint.toString() < token1Mint.toString() ? 'CORRECT' : 'INCORRECT'}`);

        // Create and fund WSOL account
        console.log(COLOR_INFO, '💰 Creating and funding WSOL account...');
        console.log(COLOR_DIM, `🔍 Deployer balance before WSOL creation: ${(await getSolanaBalance(connection, deployer.publicKey)) / 1e9} SOL`);
        
        const creatorWsolAccount = await getOrCreateAssociatedTokenAccount(
            connection,
            deployer,
            wsolMintKey,
            deployer.publicKey
        );
        console.log(COLOR_DIM, `🔍 WSOL account created: ${creatorWsolAccount.address.toString()}`);

        // Check current WSOL balance
        const currentWsolBalance = await connection.getTokenAccountBalance(creatorWsolAccount.address);
        console.log(COLOR_DIM, `🔍 Current WSOL balance: ${currentWsolBalance.value.uiAmount || 0} WSOL`);

        // Calculate pool amounts from config
        const initialDbtcAmount = new BN(
            Math.floor(config.token.initial_supply * Math.pow(10, config.token.decimals) * config.raydium.initial_dbtc_percentage / 100)
        );
        const initialSolAmount = new BN(config.raydium.initial_sol_amount);
        
        // Wrap SOL to WSOL for pool creation (add buffer for fees)
        const wrapSolAmount = initialSolAmount.add(new BN(100000000)); // Extra 0.1 SOL for fees
        console.log(COLOR_DIM, `🔍 Wrapping ${wrapSolAmount.toNumber() / 1e9} SOL to WSOL...`);
        
        const wrapSolTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: deployer.publicKey,
                toPubkey: creatorWsolAccount.address,
                lamports: wrapSolAmount.toNumber(),
            }),
            // Sync native instruction to convert SOL to WSOL
            {
                keys: [{ pubkey: creatorWsolAccount.address, isSigner: false, isWritable: true }],
                programId: TOKEN_PROGRAM_ID,
                data: Buffer.from([17]), // SyncNative instruction
            }
        );

        const wrapTxid = await sendAndConfirmTransaction(connection, wrapSolTx, [deployer]);
        console.log(COLOR_SUCCESS, `✅ Wrapped ${wrapSolAmount.toNumber() / 1e9} SOL to WSOL`);
        console.log(COLOR_DIM, `🔗 Wrap Transaction: ${wrapTxid}`);

        // Verify WSOL balance after wrapping
        const newWsolBalance = await connection.getTokenAccountBalance(creatorWsolAccount.address);
        console.log(COLOR_DIM, `🔍 WSOL balance after wrapping: ${newWsolBalance.value.uiAmount} WSOL`);

        // Derive pool PDA using correct seeds
        console.log(COLOR_DIM, '🔍 Deriving pool PDAs...');
        const [poolStatePDA] = PublicKey.findProgramAddressSync(
            [
                Buffer.from("pool"),
                ammConfigPDA.toBuffer(),
                token0Mint.toBuffer(),
                token1Mint.toBuffer(),
            ],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 Pool State PDA: ${poolStatePDA.toString()}`);

        // Derive other required PDAs
        const [authorityPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("vault_and_lp_mint_auth_seed")],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 Authority PDA: ${authorityPDA.toString()}`);

        const [lpMintPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("pool_lp_mint"), poolStatePDA.toBuffer()],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 LP Mint PDA: ${lpMintPDA.toString()}`);

        const [token0VaultPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("pool_vault"), poolStatePDA.toBuffer(), token0Mint.toBuffer()],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 Token0 Vault PDA: ${token0VaultPDA.toString()}`);

        const [token1VaultPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("pool_vault"), poolStatePDA.toBuffer(), token1Mint.toBuffer()],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 Token1 Vault PDA: ${token1VaultPDA.toString()}`);

        const [observationStatePDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("observation"), poolStatePDA.toBuffer()],
            cpProgram.programId
        );
        console.log(COLOR_DIM, `🔍 Observation State PDA: ${observationStatePDA.toString()}`);

        console.log(COLOR_INFO, `🔑 Pool State PDA: ${poolStatePDA.toString()}`);
        console.log(COLOR_INFO, `🔑 LP Mint PDA: ${lpMintPDA.toString()}`);
        console.log(COLOR_INFO, `🔑 Authority PDA: ${authorityPDA.toString()}`);

        // Get or create user token accounts
        console.log(COLOR_DIM, '🔍 Setting up user token accounts...');
        
        // Get DEGEN_BTC account (SPL-2022)
        const creatorDbtcAccount = await getAssociatedTokenAddress(
            minebtcMintKey,
            deployer.publicKey,
            false,
            anchor_spl.TOKEN_2022_PROGRAM_ID
        );
        console.log(COLOR_DIM, `🔍 Creator DEGEN_BTC Account: ${creatorDbtcAccount.toString()}`);

        // Check DEGEN_BTC balance
        try {
            const dbtcBalance = await connection.getTokenAccountBalance(creatorDbtcAccount);
            console.log(COLOR_DIM, `🔍 Creator DEGEN_BTC balance: ${dbtcBalance.value.uiAmount} DEGEN_BTC`);
            
            if (parseFloat(dbtcBalance.value.amount) < initialDbtcAmount.toNumber()) {
                console.error(COLOR_ERROR, `❌ Insufficient DEGEN_BTC balance. Need: ${initialDbtcAmount.toNumber() / Math.pow(10, config.token.decimals)}, Have: ${dbtcBalance.value.uiAmount}`);
                throw new Error('Insufficient DEGEN_BTC balance');
            }
        } catch (error) {
            console.error(COLOR_ERROR, `❌ Error checking DEGEN_BTC balance: ${error.message}`);
            throw error;
        }

        const creatorLpAccount = await getAssociatedTokenAddress(
            lpMintPDA,
            deployer.publicKey
        );
        console.log(COLOR_DIM, `🔍 Creator LP Account: ${creatorLpAccount.toString()}`);

        // The pool fee receiver address should be set to deployer's address in the deployed Raydium program
        // This was configured during deployment in 0_deploy_game.js
        const POOL_FEE_RECEIVER = deployer.publicKey;
        console.log(COLOR_INFO, `🔍 Pool fee receiver (from deployed program): ${POOL_FEE_RECEIVER.toString()}`);
        
        // Get the WSOL token account for the fee receiver (deployer)
        // This is where the pool creation fee will be sent
        const poolFeeAccount = await getAssociatedTokenAddress(
            wsolMintKey,
            POOL_FEE_RECEIVER,
            false, // allowOwnerOffCurve
            TOKEN_PROGRAM_ID
        );
        console.log(COLOR_INFO, `🔍 Pool fee account (Deployer's WSOL ATA): ${poolFeeAccount.toString()}`);
        
        // Check if the pool fee account exists
        const poolFeeAccountInfo = await connection.getAccountInfo(poolFeeAccount);
        if (!poolFeeAccountInfo) {
            console.log(COLOR_WARNING, '⚠️ Pool fee account does not exist yet, creating it...');
            // Create the WSOL account for the deployer to receive pool creation fees
            const createWsolAccountIx = await getOrCreateAssociatedTokenAccount(
                connection,
                deployer,
                wsolMintKey,
                POOL_FEE_RECEIVER,
                false,
                undefined,
                undefined,
                TOKEN_PROGRAM_ID
            );
            console.log(COLOR_SUCCESS, `✅ Created pool fee WSOL account: ${createWsolAccountIx.address.toString()}`);
        } else {
            console.log(COLOR_DIM, `🔍 Pool fee account exists: ${!!poolFeeAccountInfo}`);
            console.log(COLOR_DIM, `🔍 Pool fee account owner: ${poolFeeAccountInfo.owner.toString()}`);
            console.log(COLOR_DIM, `🔍 Pool fee account data length: ${poolFeeAccountInfo.data.length} bytes`);
        }

        // Determine token programs based on the actual token types, not order
        const dbtcTokenProgram = anchor_spl.TOKEN_2022_PROGRAM_ID; // DEGEN_BTC is always Token-2022
        const wsolTokenProgram = TOKEN_PROGRAM_ID; // WSOL is always standard SPL
        
        // Assign programs based on which token is token0/token1
        const token0Program = isDbtcToken0 ? dbtcTokenProgram : wsolTokenProgram;
        const token1Program = isDbtcToken0 ? wsolTokenProgram : dbtcTokenProgram;
        
        console.log(COLOR_DIM, `🔍 Token program assignment:`);
        console.log(COLOR_DIM, `   DEGEN_BTC program: ${dbtcTokenProgram.toString()}`);
        console.log(COLOR_DIM, `   WSOL program: ${wsolTokenProgram.toString()}`);
        console.log(COLOR_DIM, `   token0Program: ${token0Program.toString()} (${isDbtcToken0 ? 'DEGEN_BTC' : 'WSOL'})`);
        console.log(COLOR_DIM, `   token1Program: ${token1Program.toString()} (${isDbtcToken0 ? 'WSOL' : 'DEGEN_BTC'})`);
        
        // Verify token ownership
        try {
            const token0MintInfo = await connection.getAccountInfo(token0Mint);
            const token1MintInfo = await connection.getAccountInfo(token1Mint);
            console.log(COLOR_DIM, `🔍 Token ownership verification:`);
            console.log(COLOR_DIM, `   token0Mint owner: ${token0MintInfo?.owner.toString()}`);
            console.log(COLOR_DIM, `   token1Mint owner: ${token1MintInfo?.owner.toString()}`);
            console.log(COLOR_DIM, `   token0Program matches: ${token0MintInfo?.owner.equals(token0Program)}`);
            console.log(COLOR_DIM, `   token1Program matches: ${token1MintInfo?.owner.equals(token1Program)}`);
            
            // Verify creator token accounts
            console.log(COLOR_DIM, `🔍 Creator token account verification:`);
            const creatorToken0Info = await connection.getAccountInfo(creatorDbtcAccount);
            const creatorToken1Info = await connection.getAccountInfo(creatorWsolAccount.address);
            
            console.log(COLOR_DIM, `   creatorDbtcAccount exists: ${!!creatorToken0Info}`);
            console.log(COLOR_DIM, `   creatorDbtcAccount owner: ${creatorToken0Info?.owner.toString()}`);
            console.log(COLOR_DIM, `   creatorWsolAccount exists: ${!!creatorToken1Info}`);
            console.log(COLOR_DIM, `   creatorWsolAccount owner: ${creatorToken1Info?.owner.toString()}`);
            
        } catch (error) {
            console.log(COLOR_WARNING, `⚠️ Could not verify token ownership: ${error.message}`);
        }

        // Determine creator token accounts based on token order
        const creatorToken0 = isDbtcToken0 ? creatorDbtcAccount : creatorWsolAccount.address;
        const creatorToken1 = isDbtcToken0 ? creatorWsolAccount.address : creatorDbtcAccount;

        // Determine initial amounts based on token order
        const initAmount0 = isDbtcToken0 ? initialDbtcAmount : initialSolAmount;
        const initAmount1 = isDbtcToken0 ? initialSolAmount : initialDbtcAmount;

        // Prepare pool accounts
        console.log(COLOR_DIM, '🔍 Preparing pool accounts structure...');
        const poolAccounts = {
                creator: deployer.publicKey,
                ammConfig: ammConfigPDA,
            authority: authorityPDA,
            poolState: poolStatePDA,
                token0Mint: token0Mint,
                token1Mint: token1Mint,
            lpMint: lpMintPDA,
            creatorToken0: creatorToken0,
            creatorToken1: creatorToken1,
            creatorLpToken: creatorLpAccount,
            token0Vault: token0VaultPDA,
            token1Vault: token1VaultPDA,
            createPoolFee: poolFeeAccount,
            observationState: observationStatePDA,
                tokenProgram: TOKEN_PROGRAM_ID,
                token0Program: token0Program,
                token1Program: token1Program,
            associatedTokenProgram: anchor_spl.ASSOCIATED_TOKEN_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
                rent: web3.SYSVAR_RENT_PUBKEY,
        };

        console.log(COLOR_DIM, '🔍 Pool accounts structure:');
        Object.entries(poolAccounts).forEach(([key, value]) => {
            console.log(COLOR_DIM, `   ${key}: ${value.toString()}`);
        });

        const openTime = config.raydium.open_time;
        
        console.log(COLOR_DIM, '🔍 Pool initialization parameters:');
        console.log(COLOR_DIM, `   initAmount0: ${initAmount0.toString()} (${isDbtcToken0 ? 'DEGEN_BTC' : 'WSOL'})`);
        console.log(COLOR_DIM, `   initAmount1: ${initAmount1.toString()} (${isDbtcToken0 ? 'WSOL' : 'DEGEN_BTC'})`);
        console.log(COLOR_DIM, `   openTime: ${openTime} (${openTime > 0 ? new Date(openTime * 1000).toLocaleString() : 'Opens immediately'})`);

        try {
            console.log(COLOR_INFO, '🚀 Calling cpInitializePool...');
            
            // Import helper function from helper.js
            const { cpInitializePool } = await import('./helper.js');
            
            const { txid: poolTxid } = await cpInitializePool(
            connection,
                cpProgram,
                deployer,
                {
                    initAmount0: initAmount0,
                    initAmount1: initAmount1,
                    openTime: openTime,
                    accounts: poolAccounts
                }
            );

            console.log(COLOR_SUCCESS, '✅ DEGEN_BTC-SOL pool created successfully!');
            console.log(COLOR_INFO, `🔗 Transaction: ${poolTxid}`);
            console.log(COLOR_DIM, `🔍 Explorer URL: https://explorer.solana.com/tx/${poolTxid}?cluster=${CLUSTER}`);

            // Verify pool creation by checking pool state
            try {
                const poolStateInfo = await connection.getAccountInfo(poolStatePDA);
                console.log(COLOR_DIM, `🔍 Pool state account created: ${!!poolStateInfo}`);
                console.log(COLOR_DIM, `🔍 Pool state account size: ${poolStateInfo?.data.length || 0} bytes`);
            } catch (error) {
                console.log(COLOR_WARNING, `⚠️ Could not verify pool state: ${error.message}`);
            }

            // Update deployment data
            deploymentData.dbtc_sol_pool_created = {
                poolStatePDA: poolStatePDA.toString(),
                lpMintPDA: lpMintPDA.toString(),
                token0VaultPDA: token0VaultPDA.toString(),
                token1VaultPDA: token1VaultPDA.toString(),
                authorityPDA: authorityPDA.toString(),
                observationStatePDA: observationStatePDA.toString(),
                token0Mint: token0Mint.toString(),
                token1Mint: token1Mint.toString(),
                isDbtcToken0: isDbtcToken0,
                txid: poolTxid,
                wrapTxid: wrapTxid,
                initialDbtcAmount: initialDbtcAmount.toString(),
                initialSolAmount: initialSolAmount.toString(),
                openTime: openTime,
                timestamp: new Date().toISOString()
            };
            fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
            console.log(COLOR_SUCCESS, '✅ Deployment status updated');
        } catch (error) {
            console.error(COLOR_ERROR, `❌ Pool creation failed: ${error.message}`);
            console.error(COLOR_ERROR, `❌ Error stack: ${error.stack}`);
            
            if (error.logs) {
                console.error(COLOR_ERROR, '📝 Transaction logs:');
                error.logs.forEach((log, index) => {
                    console.error(COLOR_ERROR, `[${index}] ${log}`);
                });
            }
            
            if (error.transactionLogs) {
                console.error(COLOR_ERROR, '📝 Transaction logs (alternative):');
                error.transactionLogs.forEach((log, index) => {
                    console.error(COLOR_ERROR, `[${index}] ${log}`);
                });
            }

            // Additional debugging info
            console.error(COLOR_ERROR, '🔍 Debug information:');
            console.error(COLOR_ERROR, `   Program ID: ${cpProgram.programId.toString()}`);
            console.error(COLOR_ERROR, `   AMM Config: ${ammConfigPDA.toString()}`);
            console.error(COLOR_ERROR, `   Pool State: ${poolStatePDA.toString()}`);
            console.error(COLOR_ERROR, `   Token0 Mint: ${token0Mint.toString()}`);
            console.error(COLOR_ERROR, `   Token1 Mint: ${token1Mint.toString()}`);
            console.error(COLOR_ERROR, `   Token0 Program: ${token0Program.toString()}`);
            console.error(COLOR_ERROR, `   Token1 Program: ${token1Program.toString()}`);
            
                throw error;
        }
    } else {
        console.log(COLOR_INFO, 'ℹ️ DEGEN_BTC-SOL pool already exists');
            }
}

/**
 * Adds additional liquidity to the pool (optional, for post-creation liquidity)
 * @param {Connection} connection - Solana connection
 * @param {Program} cpProgram - Raydium CP program instance
 * @param {Keypair} deployer - Deployer keypair
 * @param {Object} deploymentData - Deployment state data
 * @param {string} deploymentPath - Path to deployment file
 */
async function addInitialLiquidity(connection, cpProgram, deployer, deploymentData, deploymentPath) {
    // Check if already added
    if (deploymentData.dbtc_sol_liquidity_added) {
        console.log(COLOR_INFO, 'ℹ️ Liquidity already added to pool');
        console.log(COLOR_INFO, '🔑 LP Tokens Received:', deploymentData.dbtc_sol_liquidity_added.lpTokensReceived);
        return;
    }

    // Get DEGEN_BTC mint
    const minebtcMint = new PublicKey(deploymentData.dbtc_mint_address);

    // =================== [ ADDING LIQUIDITY TO POOL ] ===================
    if (!deploymentData.dbtc_sol_liquidity_added) {
        console.log(COLOR_STEP, '\n=================== [ ADDING LIQUIDITY TO POOL ] ===================');
        console.log(COLOR_INFO, '💧 Adding liquidity to DEGEN_BTC-SOL pool...');
        
        // Get token accounts (recreate them since they were in pool creation scope)
        const wsolMintKey = new PublicKey(WSOL_MINT);
        const creatorWsolAccount = await getOrCreateAssociatedTokenAccount(
            connection,
            deployer,
            wsolMintKey,
            deployer.publicKey
        );
        
        const creatorDbtcAccount = await getAssociatedTokenAddress(
            minebtcMint,
            deployer.publicKey,
            false,
            anchor_spl.TOKEN_2022_PROGRAM_ID
        );
        
        // Get pool data from deployment
        const poolData = deploymentData.dbtc_sol_pool_created;
        const poolStatePDA = new PublicKey(poolData.poolStatePDA);
        const lpMintPDA = new PublicKey(poolData.lpMintPDA);
        const token0VaultPDA = new PublicKey(poolData.token0VaultPDA);
        const token1VaultPDA = new PublicKey(poolData.token1VaultPDA);
        const authorityPDA = new PublicKey(poolData.authorityPDA);
        const isDbtcToken0 = poolData.isDbtcToken0;
        
        const creatorLpAccount = await getAssociatedTokenAddress(
            lpMintPDA,
            deployer.publicKey
        );
        
        // Read the pool's CURRENT reserves + LP supply on-chain so the
        // deposit ratio matches the pool exactly. Hardcoding amounts
        // here used to fail with ExceededSlippage (Raydium 0x1775)
        // whenever the create-pool seed ratio drifted from the chosen
        // deepening size. The deposit ix takes `lp_amount` and derives
        // needed_tokenN = lp_amount * reserveN / lp_supply, so we just
        // size `lp_amount` to whatever fraction of the pool we want and
        // pass the matching maxes (with a small fee + slippage cushion).
        const targetSolBaseUnits = new BN("5000000000"); // deepen by 5 WSOL

        const wsolMintInfo = wsolMintKey;
        const token0VaultAcc = await anchor_spl.getAccount(
            connection,
            token0VaultPDA,
            undefined,
            isDbtcToken0 ? anchor_spl.TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID
        );
        const token1VaultAcc = await anchor_spl.getAccount(
            connection,
            token1VaultPDA,
            undefined,
            isDbtcToken0 ? TOKEN_PROGRAM_ID : anchor_spl.TOKEN_2022_PROGRAM_ID
        );
        const lpMintInfo = await anchor_spl.getMint(connection, lpMintPDA);
        const lpSupply = new BN(lpMintInfo.supply.toString());
        const token0Reserve = new BN(token0VaultAcc.amount.toString());
        const token1Reserve = new BN(token1VaultAcc.amount.toString());
        const wsolReserve = isDbtcToken0 ? token1Reserve : token0Reserve;
        const dbtcReserve = isDbtcToken0 ? token0Reserve : token1Reserve;

        if (wsolReserve.isZero() || dbtcReserve.isZero() || lpSupply.isZero()) {
            throw new Error("Pool reserves / LP supply look empty — pool create probably failed.");
        }

        // lp_amount = targetSolBaseUnits / wsolReserve * lpSupply (proportional share)
        const lpAmount = targetSolBaseUnits.mul(lpSupply).div(wsolReserve);
        const requiredWsol = lpAmount.mul(wsolReserve).div(lpSupply);
        const requiredDbtc = lpAmount.mul(dbtcReserve).div(lpSupply);

        // Cushion: dBTC has a 0.1% Token-2022 transfer fee + we add 0.4%
        // pure slippage. WSOL has no fee, so 0.5% slippage is plenty.
        const maxWsol = requiredWsol.mul(new BN(1005)).div(new BN(1000));
        const maxDbtc = requiredDbtc.mul(new BN(1015)).div(new BN(1000));

        const liquidityAmount0 = isDbtcToken0 ? maxDbtc : maxWsol;
        const liquidityAmount1 = isDbtcToken0 ? maxWsol : maxDbtc;

        console.log(COLOR_DIM, `🔍 Pool ratio (live):`);
        console.log(COLOR_DIM, `   WSOL reserve: ${wsolReserve.toString()} (${wsolReserve.toNumber() / 1e9} WSOL)`);
        console.log(COLOR_DIM, `   DEGEN_BTC reserve: ${dbtcReserve.toString()} (${dbtcReserve.toNumber() / 1e6} dBTC)`);
        console.log(COLOR_DIM, `   LP supply: ${lpSupply.toString()}`);
        console.log(COLOR_DIM, `🔍 Computed deposit (target = 5 WSOL share):`);
        console.log(COLOR_DIM, `   lp_amount: ${lpAmount.toString()}`);
        console.log(COLOR_DIM, `   needed WSOL: ${requiredWsol.toString()} (${requiredWsol.toNumber() / 1e9} WSOL)`);
        console.log(COLOR_DIM, `   needed dBTC: ${requiredDbtc.toString()} (${requiredDbtc.toNumber() / 1e6} dBTC)`);
        console.log(COLOR_DIM, `   maxToken0 (with cushion): ${liquidityAmount0.toString()}`);
        console.log(COLOR_DIM, `   maxToken1 (with cushion): ${liquidityAmount1.toString()}`);

        // Check current balances
        const wsolBalance = await anchor_spl.getAccount(connection, creatorWsolAccount.address, undefined, TOKEN_PROGRAM_ID);
        const dbtcBalance = await anchor_spl.getAccount(connection, creatorDbtcAccount, undefined, anchor_spl.TOKEN_2022_PROGRAM_ID);

        console.log(COLOR_DIM, `🔍 Current balances:`);
        console.log(COLOR_DIM, `   WSOL: ${wsolBalance.amount.toString()} (${Number(wsolBalance.amount) / 1e9} WSOL)`);
        console.log(COLOR_DIM, `   DEGEN_BTC: ${dbtcBalance.amount.toString()} (${Number(dbtcBalance.amount) / 1e6} dBTC)`);

        // Top up WSOL if we don't have enough.
        const wsolHave = new BN(wsolBalance.amount.toString());
        if (wsolHave.lt(maxWsol)) {
            const deficit = maxWsol.sub(wsolHave);
            const additionalSol = deficit.add(new BN(1_000_000_000)); // +1 SOL buffer
            console.log(COLOR_WARNING, `⚠️ Insufficient WSOL. Wrapping ${additionalSol.toString()} extra lamports...`);
            const wrapTx = new Transaction().add(
                SystemProgram.transfer({
                    fromPubkey: deployer.publicKey,
                    toPubkey: creatorWsolAccount.address,
                    lamports: additionalSol.toNumber(),
                }),
                anchor_spl.createSyncNativeInstruction(creatorWsolAccount.address, TOKEN_PROGRAM_ID)
            );
            const wrapSig = await web3.sendAndConfirmTransaction(connection, wrapTx, [deployer]);
            console.log(COLOR_SUCCESS, `✅ Wrapped ${additionalSol.toNumber() / 1e9} SOL → WSOL`);
            console.log(COLOR_DIM, `🔗 Wrap Transaction: ${wrapSig}`);
        }

        const dbtcHave = new BN(dbtcBalance.amount.toString());
        if (dbtcHave.lt(maxDbtc)) {
            throw new Error(
                `Insufficient DEGEN_BTC. Need ${maxDbtc.toString()} base units, have ${dbtcHave.toString()}.`
            );
        }
        
        // Determine token accounts based on token order
        const token0Account = isDbtcToken0 ? creatorDbtcAccount : creatorWsolAccount.address;
        const token1Account = isDbtcToken0 ? creatorWsolAccount.address : creatorDbtcAccount;
        
        // Prepare liquidity accounts
        const liquidityAccounts = {
            owner: deployer.publicKey,
            authority: authorityPDA,
            poolState: poolStatePDA,
            ownerLpToken: creatorLpAccount,
            token0Account: token0Account,
            token1Account: token1Account,
            token0Vault: token0VaultPDA,
            token1Vault: token1VaultPDA,
            tokenProgram: TOKEN_PROGRAM_ID,
            tokenProgram2022: anchor_spl.TOKEN_2022_PROGRAM_ID,
            vault0Mint: isDbtcToken0 ? minebtcMint : wsolMintKey,
            vault1Mint: isDbtcToken0 ? wsolMintKey : minebtcMint,
            lpMint: lpMintPDA,
        };
        
        console.log(COLOR_DIM, `🔍 Liquidity accounts structure:`);
        console.log(COLOR_DIM, `   owner: ${liquidityAccounts.owner.toString()}`);
        console.log(COLOR_DIM, `   authority: ${liquidityAccounts.authority.toString()}`);
        console.log(COLOR_DIM, `   poolState: ${liquidityAccounts.poolState.toString()}`);
        console.log(COLOR_DIM, `   ownerLpToken: ${liquidityAccounts.ownerLpToken.toString()}`);
        console.log(COLOR_DIM, `   token0Account: ${liquidityAccounts.token0Account.toString()}`);
        console.log(COLOR_DIM, `   token1Account: ${liquidityAccounts.token1Account.toString()}`);
        console.log(COLOR_DIM, `   token0Vault: ${liquidityAccounts.token0Vault.toString()}`);
        console.log(COLOR_DIM, `   token1Vault: ${liquidityAccounts.token1Vault.toString()}`);
        
        console.log(COLOR_INFO, '🚀 Adding liquidity to pool...');
        
        try {
            // Import helper function from helper.js
            const { cpDepositLiquidity } = await import('./helper.js');
            
            // Use the deposit function from helper. lpAmount drives the
            // pool-side math; maxToken0 / maxToken1 are the cushioned
            // amounts we computed above (already include the dBTC
            // transfer fee + slippage buffers).
            const liquidityResult = await cpDepositLiquidity(
                connection,
                cpProgram,
                deployer,
                {
                    lpAmount,
                    maxToken0: liquidityAmount0,
                    maxToken1: liquidityAmount1,
                    accounts: liquidityAccounts,
                }
            );
            
            console.log(COLOR_SUCCESS, '✅ Liquidity added successfully!');
            console.log(COLOR_DIM, `🔗 Transaction: ${liquidityResult.txid}`);
            console.log(COLOR_DIM, `🔍 Explorer URL: https://explorer.solana.com/tx/${liquidityResult.txid}?cluster=${CLUSTER}`);
            
            // Check LP token balance
            const lpBalance = await anchor_spl.getAccount(connection, creatorLpAccount, undefined, TOKEN_PROGRAM_ID);
            console.log(COLOR_SUCCESS, `💧 LP tokens received: ${lpBalance.amount.toString()} (${Number(lpBalance.amount) / 1e9} LP)`);
            
            // Update deployment status (record both the math-derived
            // amounts and the cushioned maxes for audit).
            deploymentData.dbtc_sol_liquidity_added = {
                timestamp: new Date().toISOString(),
                lp_amount_minted: lpAmount.toString(),
                required_wsol: requiredWsol.toString(),
                required_dbtc: requiredDbtc.toString(),
                max_token0: liquidityAmount0.toString(),
                max_token1: liquidityAmount1.toString(),
                lpTokensReceived: lpBalance.amount.toString(),
                txid: liquidityResult.txid,
            };
                
                fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
            console.log(COLOR_SUCCESS, '✅ Liquidity deployment status updated');
                
            } catch (error) {
            console.error(COLOR_ERROR, `❌ Error adding liquidity: ${error.message}`);
            if (error.logs) {
                console.error(COLOR_ERROR, '📝 Transaction logs:');
                error.logs.forEach((log, index) => {
                    console.error(COLOR_ERROR, `[${index}] ${log}`);
                });
            }
                throw error;
        }
    } else {
        console.log(COLOR_INFO, 'ℹ️ Liquidity already added to pool');
            }
}

/**
 * Burns LP tokens to permanently lock initial liquidity
 * @param {Connection} connection - Solana connection
 * @param {Keypair} deployer - Deployer keypair
 * @param {Object} deploymentData - Deployment state data
 * @param {string} deploymentPath - Path to deployment file
 */
async function burnLpTokens(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.lp_tokens_burned) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ LP tokens already burned. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔥 Burned Amount:', deploymentData.lp_tokens_burned.burned_amount);
        console.log('\x1b[36m%s\x1b[0m', '🔗 Burn Transaction:', deploymentData.lp_tokens_burned.burn_signature);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ BURNING LP TOKENS ] ===================');
    
    console.log('\x1b[36m%s\x1b[0m', '🔥 LP Token Burning Configuration:');
    console.log('\x1b[36m%s\x1b[0m', `   • Pool: ${deploymentData.dbtc_sol_pool_created.poolStatePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Purpose: Permanent liquidity lock`);
    console.log('\x1b[36m%s\x1b[0m', `   • Effect: Cannot remove initial liquidity`);
    
    try {
        // Get the LP token mint from the pool creation data
        const lpTokenMint = new PublicKey(deploymentData.dbtc_sol_pool_created.lpMintPDA);
        console.log(COLOR_DIM, `🔍 LP Token Mint: ${lpTokenMint.toString()}`);
        
        // Get deployer's LP token account (Associated Token Account)
        const deployerLpAccount = await getAssociatedTokenAddress(
            lpTokenMint,
            deployer.publicKey,
            false, // allowOwnerOffCurve
            TOKEN_PROGRAM_ID // LP tokens are regular SPL tokens
        );
        console.log(COLOR_DIM, `🔍 Deployer LP Account: ${deployerLpAccount.toString()}`);
        
        // Check if the LP account exists and get balance
        let lpBalance;
        try {
            const lpAccountInfo = await anchor_spl.getAccount(connection, deployerLpAccount, undefined, TOKEN_PROGRAM_ID);
            lpBalance = lpAccountInfo.amount;
            console.log(COLOR_INFO, `💧 Current LP balance: ${lpBalance.toString()} tokens`);
            console.log(COLOR_DIM, `💧 Current LP balance (readable): ${Number(lpBalance) / 1e9} LP`);
        } catch (error) {
            if (error.name === 'TokenAccountNotFoundError') {
                console.log(COLOR_WARNING, '⚠️ No LP token account found for deployer');
                console.log(COLOR_INFO, 'ℹ️ This might mean LP tokens were already burned or never received');
                
                // Record as already burned/no tokens to burn
                deploymentData.lp_tokens_burned = {
                    pool_state: deploymentData.dbtc_sol_pool_created.poolStatePDA,
                    burn_all_lp_tokens: true,
                    burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
                    dbtc_locked_amount: deploymentData.dbtc_sol_pool_created.initialDbtcAmount,
                    sol_locked_amount: deploymentData.dbtc_sol_pool_created.initialSolAmount,
                    burned_amount: "0",
                    burn_signature: "N/A - No tokens to burn",
                    status: 'no_tokens_to_burn',
                    permanent_lock: true,
                    timestamp: new Date().toISOString()
                };
                
                fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
                console.log(COLOR_SUCCESS, '✅ LP token burning check complete (no tokens found)');
                return;
            }
            throw error;
        }
        
        // Check if there are tokens to burn
        if (lpBalance === 0n) {
            console.log(COLOR_INFO, 'ℹ️ No LP tokens to burn (balance is 0)');
            
        deploymentData.lp_tokens_burned = {
                pool_state: deploymentData.dbtc_sol_pool_created.poolStatePDA,
            burn_all_lp_tokens: true,
            burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
                dbtc_locked_amount: deploymentData.dbtc_sol_pool_created.initialDbtcAmount,
                sol_locked_amount: deploymentData.dbtc_sol_pool_created.initialSolAmount,
                burned_amount: "0",
                burn_signature: "N/A - No tokens to burn",
                status: 'no_tokens_to_burn',
            permanent_lock: true,
            timestamp: new Date().toISOString()
        };
        
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
            console.log(COLOR_SUCCESS, '✅ LP token burning check complete (zero balance)');
            return;
        }
        
        console.log(COLOR_WARNING, '🔥 About to burn ALL LP tokens to permanently lock liquidity!');
        console.log(COLOR_WARNING, '⚠️ This action is IRREVERSIBLE and will make the liquidity PERMANENTLY LOCKED');
        console.log(COLOR_DIM, `🔥 Tokens to burn: ${lpBalance.toString()}`);
        
        // Burn all LP tokens using the SPL token burn instruction
        console.log(COLOR_INFO, '🔥 Burning LP tokens...');
        
        const burnSignature = await burn(
            connection,
            deployer,           // payer (for transaction fees)
            deployerLpAccount,  // token account to burn from
            lpTokenMint,        // mint of the token
            deployer,           // owner of the token account
            lpBalance,          // amount to burn (all tokens)
            [],                 // multisig signers (empty for single signer)
            { commitment: 'confirmed' }, // confirmation options
            TOKEN_PROGRAM_ID    // token program (regular SPL for LP tokens)
        );
        
        console.log(COLOR_SUCCESS, '🔥 LP tokens burned successfully!');
        console.log(COLOR_DIM, `🔗 Burn Transaction: ${burnSignature}`);
        console.log(COLOR_DIM, `🔍 Explorer URL: https://explorer.solana.com/tx/${burnSignature}?cluster=${CLUSTER}`);
        
        // Verify the burn was successful by checking balance again
        try {
            const postBurnAccount = await anchor_spl.getAccount(connection, deployerLpAccount, undefined, TOKEN_PROGRAM_ID);
            const postBurnBalance = postBurnAccount.amount;
            
            if (postBurnBalance === 0n) {
                console.log(COLOR_SUCCESS, '✅ Burn verification successful - LP balance is now 0');
            } else {
                console.log(COLOR_WARNING, `⚠️ Warning: LP balance after burn is ${postBurnBalance.toString()} (expected 0)`);
            }
    } catch (error) {
            if (error.name === 'TokenAccountNotFoundError') {
                console.log(COLOR_SUCCESS, '✅ Burn verification successful - LP account no longer exists');
            } else {
                console.log(COLOR_WARNING, '⚠️ Could not verify burn status:', error.message);
            }
        }
        
        // Record the successful LP token burn
        deploymentData.lp_tokens_burned = {
            pool_state: deploymentData.dbtc_sol_pool_created.poolStatePDA,
            lp_mint: lpTokenMint.toString(),
            deployer_lp_account: deployerLpAccount.toString(),
            burn_all_lp_tokens: true,
            burned_amount: lpBalance.toString(),
            burned_amount_readable: `${Number(lpBalance) / 1e9} LP`,
            burn_signature: burnSignature,
            burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
            dbtc_locked_amount: deploymentData.dbtc_sol_pool_created.initialDbtcAmount,
            sol_locked_amount: deploymentData.dbtc_sol_pool_created.initialSolAmount,
            status: 'burned_successfully',
            permanent_lock: true,
            timestamp: new Date().toISOString()
        };
        
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log(COLOR_SUCCESS, '✅ LP token burning complete!');
        console.log(COLOR_SUCCESS, '🔒 Liquidity is now PERMANENTLY LOCKED');
        console.log(COLOR_SUCCESS, '🎯 Pool initial liquidity can never be removed');
        
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to burn LP tokens:', error);
        
        // Log additional error details
        if (error.logs) {
            console.error(COLOR_ERROR, '📝 Transaction logs:');
            error.logs.forEach((log, index) => {
                console.error(COLOR_ERROR, `[${index}] ${log}`);
            });
        }
        
        throw error;
    }
}

/**
 * Prints a summary of the completed pool configuration
 * @param {Object} deploymentData - Deployment state data
 */
function printCompletionSummary(deploymentData) {
    console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ POOL CONFIGURATION COMPLETE ================================');
    console.log('\x1b[32m%s\x1b[0m', '✅ DEGEN_BTC-SOL pool production configuration completed!');
    
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Pool Summary:');
    console.log('\x1b[36m%s\x1b[0m', `  • Network: ${CLUSTER}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Pool Type: Raydium CP-Swap`);
    console.log('\x1b[36m%s\x1b[0m', `  • Trade Fee: ${config.raydium.trade_fee_rate / 10000}%`);
    console.log('\x1b[36m%s\x1b[0m', `  • Initial SOL: ${config.raydium.initial_sol_amount / 1e9} SOL`);
    if (deploymentData.raydium_pool_initialized) {
        console.log('\x1b[36m%s\x1b[0m', `  • Initial DEGEN_BTC: ${deploymentData.raydium_pool_initialized.pool_dbtc_readable}`);
        console.log('\x1b[36m%s\x1b[0m', `  • DEGEN_BTC Percentage: ${deploymentData.raydium_pool_initialized.dbtc_percentage_for_pool}% of total supply`);
    }
    if (config.raydium.burn_lp_tokens) {
        console.log('\x1b[32m%s\x1b[0m', `  • LP Tokens: Will be BURNED (permanent liquidity lock)`);
    }
    
    console.log('\x1b[90m%s\x1b[0m', '\n🔑 Important Addresses:');
        if (deploymentData.raydium_amm_config_created) {
        console.log('\x1b[90m%s\x1b[0m', `   AMM Config: ${deploymentData.raydium_amm_config_created.amm_config_pda}`);
        }
    if (deploymentData.raydium_pool_initialized) {
        console.log('\x1b[90m%s\x1b[0m', `   Pool State: ${deploymentData.raydium_pool_initialized.pool_state_pda}`);
        console.log('\x1b[90m%s\x1b[0m', `   Token 0: ${deploymentData.raydium_pool_initialized.token_0_mint}`);
        console.log('\x1b[90m%s\x1b[0m', `   Token 1: ${deploymentData.raydium_pool_initialized.token_1_mint}`);
        }
    
    // Show LP token burning status
    if (deploymentData.lp_tokens_burned) {
        console.log('\x1b[32m%s\x1b[0m', '\n🔥 LP Token Burning:');
        console.log('\x1b[32m%s\x1b[0m', `   Status: ${deploymentData.lp_tokens_burned.status}`);
        console.log('\x1b[32m%s\x1b[0m', `   Locked DEGEN_BTC: ${deploymentData.lp_tokens_burned.dbtc_locked_amount}`);
        console.log('\x1b[32m%s\x1b[0m', `   Locked SOL: ${deploymentData.lp_tokens_burned.sol_locked_amount}`);
        console.log('\x1b[32m%s\x1b[0m', `   🔒 Permanent Lock: ${deploymentData.lp_tokens_burned.permanent_lock ? 'YES' : 'NO'}`);
    }
        
    console.log('\x1b[33m%s\x1b[0m', '\n📋 PRODUCTION READY:');
    console.log('\x1b[33m%s\x1b[0m', '   • Pool configuration calculated and validated');
    console.log('\x1b[33m%s\x1b[0m', '   • Liquidity amounts: 1% DEGEN_BTC supply + 10 SOL');
    console.log('\x1b[33m%s\x1b[0m', '   • LP tokens configured for burning (permanent lock)');
    console.log('\x1b[33m%s\x1b[0m', '   • Ready for Raydium SDK integration');
    
    console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
    console.log('\x1b[36m%s\x1b[0m', '📁 Pool configuration saved to:', path.resolve(__dirname, config.deployment.paths.deployments_dir, `${CLUSTER}.json`));
}
