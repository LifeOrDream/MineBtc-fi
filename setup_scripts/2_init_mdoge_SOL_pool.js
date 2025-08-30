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
const { AnchorProvider, BN, Program, setProvider, web3 } = pkg;
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import path from 'path';
import { getSolanaBalance, updateDeploymentStatus } from './helper.js';
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


// Raydium CP-Swap constants
const CP_AMM_CONFIG_SEED = "amm_config";

// Load deployment state
const { deploymentData, deploymentPath } = loadDeploymentState();

// Raydium CP-Swap Program ID (official)
const RAYDIUM_CP_PROGRAM_ID = new PublicKey(deploymentData.RAYDIUM_CP_PROGRAM_ID);

// Pool configuration constants
const POOL_CONFIG = {
    initialMdogeAmount: new BN(config.token.initial_supply * Math.pow(10, config.token.decimals) * config.raydium.initial_mdoge_percentage / 100),
    initialSolAmount: new BN(config.raydium.initial_sol_amount),
    openTime: config.raydium.open_time
};

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
    console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ DogeTech mDOGE-SOL Pool Creation ================================');
    console.log('\x1b[36m%s\x1b[0m', '🌐 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🔗 RPC URL:', RPC_URL);
    console.log('\x1b[36m%s\x1b[0m', '💱 Pool Type: Raydium CP-Swap (Constant Product)');

    // Setup connection and deployer
    const connection = await initializeConnection();
    const deployer = await setupDeployerAccount(connection);
    
    // Load deployment state
    const { deploymentData, deploymentPath } = loadDeploymentState();
    
    // Validate prerequisites
    validatePrerequisites(deploymentData);
    
    // Setup Raydium program
    const provider = new AnchorProvider( connection,   deployer, { commitment: 'confirmed' });
    const cpIdlPath = path.resolve(__dirname, '../raydium_cp/target/idl/raydium_cp_swap.json');
    const cpIdl = JSON.parse(fs.readFileSync(cpIdlPath, 'utf8'));

    const cpProgram =  new Program(cpIdl, provider);
    // return
    
    try {
        // 1. Create AMM Config
        await createAmmConfig(connection, cpProgram, deployer, deploymentData, deploymentPath);
        // return
        
        // 2. Initialize Pool :: Automatically adds initial liquidity
        await initializePool(connection, cpProgram, deployer, deploymentData, deploymentPath);
        return
        
        // 3. Add Initial Liquidity
        // await addInitialLiquidity(connection, cpProgram, deployer, deploymentData, deploymentPath);
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
                const airdropSignature = await connection.requestAirdrop(
                    deployer.publicKey,
                    config.dev.airdrop_amount
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

function validatePrerequisites(deploymentData) {
    console.log('\x1b[33m%s\x1b[0m', '🔍 Validating prerequisites...');
    
    const errors = [];
    
    if (!deploymentData.mdoge_mint_address) {
        errors.push('mDOGE token mint address not found');
    }
    
    if (!deploymentData.mdoge_token_account_created?.token_account_address) {
        errors.push('mDOGE token account not found');
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

 

async function createAmmConfig(connection, cpProgram, deployer, deploymentData, deploymentPath) {
    if (deploymentData.raydium_amm_config_created) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Raydium AMM config already exists. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔑 AMM Config:', deploymentData.raydium_amm_config_created.amm_config_pda);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ CREATING AMM CONFIG ] ===================');
    
    const configIndex = config.raydium.amm_config_index;
    const tradeFeeRate = new BN(config.raydium.trade_fee_rate);
    const protocolFeeRate = new BN(config.raydium.protocol_fee_rate);
    const fundFeeRate = new BN(config.raydium.fund_fee_rate);
    const createPoolFee = new BN(config.raydium.create_pool_fee);
            
    console.log('\x1b[36m%s\x1b[0m', '⚙️ AMM Config Parameters:');
    console.log('\x1b[36m%s\x1b[0m', `   • Config Index: ${configIndex}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Trade Fee Rate: ${tradeFeeRate.toNumber() / 10000}%`);
    console.log('\x1b[36m%s\x1b[0m', `   • Protocol Fee Rate: ${protocolFeeRate.toNumber() / 10000}%`);
    console.log('\x1b[36m%s\x1b[0m', `   • Fund Fee Rate: ${fundFeeRate.toNumber() / 10000}%`);
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
                
        // Check if config already exists
        try {
            const configInfo = await connection.getAccountInfo(ammConfigPDA);
            if (configInfo) {
                console.log('\x1b[33m%s\x1b[0m', '⚠️ AMM config already exists on-chain');
                
                // Update deployment data
                deploymentData.RAYDIUM_CP_PROGRAM_ID = RAYDIUM_CP_PROGRAM_ID.toBase58();
                deploymentData.raydium_amm_config_created = {
                    amm_config_pda: ammConfigPDA.toBase58(),
                    raydium_program_id: RAYDIUM_CP_PROGRAM_ID.toBase58(),
                    config_index: configIndex,
                    trade_fee_rate: tradeFeeRate.toNumber(),
                    protocol_fee_rate: protocolFeeRate.toNumber(),
                    fund_fee_rate: fundFeeRate.toNumber(),
                    create_pool_fee: createPoolFee.toString(),
                    status: 'already_exists',
                    timestamp: new Date().toISOString()
                };
                
                fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
                console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data updated');
                return;
            }
        } catch (error) {
            // Config doesn't exist, continue with creation
        }
        
        console.log('\x1b[33m%s\x1b[0m', '📡 Creating AMM config on-chain...');
        
        // Call the createAmmConfig instruction
        const tx = await cpProgram.methods
            .createAmmConfig(
                configIndex,           // index: u16
                tradeFeeRate,         // trade_fee_rate: u64  
                protocolFeeRate,      // protocol_fee_rate: u64
                fundFeeRate,          // fund_fee_rate: u64
                createPoolFee         // create_pool_fee: u64
            )
            .accounts({
                owner: deployer.publicKey,
                ammConfig: ammConfigPDA,
                systemProgram: SystemProgram.programId,
            })
            .transaction();

            const txid = await web3.sendAndConfirmTransaction(
                connection,
                tx,
                [deployer]
            );
            
        console.log('\x1b[32m%s\x1b[0m', '✅ AMM config created successfully!');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', txid);
        console.log('\x1b[90m%s\x1b[0m', `🔍 Explorer URL: https://explorer.solana.com/tx/${txid}?cluster=${CLUSTER}`);
    
        
        // Record the successful configuration
        deploymentData.raydium_amm_config_created = {
            amm_config_pda: ammConfigPDA.toBase58(),
            raydium_program_id: RAYDIUM_CP_PROGRAM_ID.toBase58(),
            config_index: configIndex,
            trade_fee_rate: tradeFeeRate.toNumber(),
            protocol_fee_rate: protocolFeeRate.toNumber(),
            fund_fee_rate: fundFeeRate.toNumber(),
            create_pool_fee: createPoolFee.toString(),
            trade_fee_readable: `${tradeFeeRate.toNumber() / 10000}%`,
            protocol_fee_readable: `${protocolFeeRate.toNumber() / 10000}%`,
            fund_fee_readable: `${fundFeeRate.toNumber() / 10000}%`,
            create_pool_fee_readable: `${createPoolFee.toNumber() / 1e9} SOL`,
            creation_signature: txid,
            pda_bump: bump,
            status: 'created',
            timestamp: new Date().toISOString()
        };
        
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ AMM config creation complete');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to create AMM config:', error);
        throw error;
    }
}

async function initializePool(connection, cpProgram, deployer, deploymentData, deploymentPath) {
    // Check if already created
    if (deploymentData.mdoge_sol_pool_created) {
        console.log(COLOR_INFO, 'ℹ️ mDOGE-SOL pool already exists');
        console.log(COLOR_INFO, '🔑 Pool State:', deploymentData.mdoge_sol_pool_created.poolStatePDA);
        return;
    }

    // ------------- xxxx -----------------
    // CREATE mDOGE-SOL POOL
    // ------------- xxxx -----------------

    const ammConfigPDA = new PublicKey(deploymentData.raydium_amm_config_created.amm_config_pda);

    // Step 2: Create mDOGE-SOL Pool
    console.log(COLOR_STEP, '\n=================== [ CREATING mDOGE-SOL POOL ] ===================');
    
    if (!deploymentData.mdoge_sol_pool_created) {
        console.log(COLOR_INFO, '🏊 Creating mDOGE-SOL pool...');

        // Get the token mints
        const mdogeMintKey = new PublicKey(deploymentData.mdoge_mint_address);
        const wsolMintKey = new PublicKey("So11111111111111111111111111111111111111112"); // Wrapped SOL mint
        
        console.log(COLOR_DIM, `🔍 mDOGE Mint: ${mdogeMintKey.toString()}`);
        console.log(COLOR_DIM, `🔍 WSOL Mint: ${wsolMintKey.toString()}`);
        
        // Ensure correct token order (token0 < token1) using byte-wise comparison
        const mdogeMintBytes = mdogeMintKey.toBytes();
        const wsolMintBytes = wsolMintKey.toBytes();
        const isMdogeToken0 = Buffer.compare(mdogeMintBytes, wsolMintBytes) < 0;
        
        const token0Mint = isMdogeToken0 ? mdogeMintKey : wsolMintKey;
        const token1Mint = isMdogeToken0 ? wsolMintKey : mdogeMintKey;
        
        console.log(COLOR_INFO, `🪙 Token0: ${token0Mint.toString()} ${isMdogeToken0 ? '(mDOGE)' : '(WSOL)'}`);
        console.log(COLOR_INFO, `🪙 Token1: ${token1Mint.toString()} ${!isMdogeToken0 ? '(mDOGE)' : '(WSOL)'}`);
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

        // Wrap SOL to WSOL for pool creation
        const wrapSolAmount = POOL_CONFIG.initialSolAmount.add(new BN("100000000")); // Extra 0.1 SOL for fees
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
        
        // Get mDOGE account (SPL-2022)
        const creatorMdogeAccount = await getAssociatedTokenAddress(
            mdogeMintKey,
            deployer.publicKey,
            false,
            anchor_spl.TOKEN_2022_PROGRAM_ID
        );
        console.log(COLOR_DIM, `🔍 Creator mDOGE Account: ${creatorMdogeAccount.toString()}`);

        // Check mDOGE balance
        try {
            const mdogeBalance = await connection.getTokenAccountBalance(creatorMdogeAccount);
            console.log(COLOR_DIM, `🔍 Creator mDOGE balance: ${mdogeBalance.value.uiAmount} mDOGE`);
            
            if (parseFloat(mdogeBalance.value.amount) < POOL_CONFIG.initialMdogeAmount.toNumber()) {
                console.error(COLOR_ERROR, `❌ Insufficient mDOGE balance. Need: ${POOL_CONFIG.initialMdogeAmount.toNumber() / 1e6}, Have: ${mdogeBalance.value.uiAmount}`);
                throw new Error('Insufficient mDOGE balance');
            }
        } catch (error) {
            console.error(COLOR_ERROR, `❌ Error checking mDOGE balance: ${error.message}`);
            throw error;
        }

        const creatorLpAccount = await getAssociatedTokenAddress(
            lpMintPDA,
            deployer.publicKey
        );
        console.log(COLOR_DIM, `🔍 Creator LP Account: ${creatorLpAccount.toString()}`);

        // Get the correct pool fee account - use WSOL token account for fee receiver
        const feeReceiverAddress = new PublicKey("DdktaCvL7i5c8yqnpiwXgVJpJmVWmhDCBGd9WB8xBc61");
        
        console.log(COLOR_INFO, `🔍 Fee Receiver Address: ${feeReceiverAddress.toString()}`);
        console.log(COLOR_INFO, `🔍 Our deployer address: ${deployer.publicKey.toString()}`);
        console.log(COLOR_INFO, `🔍 Addresses match: ${feeReceiverAddress.equals(deployer.publicKey)}`);
        
        // Since fee receiver is our deployer, use our WSOL account
        const poolFeeAccount = creatorWsolAccount.address;
        console.log(COLOR_INFO, `🔍 Using our WSOL account as pool fee account: ${poolFeeAccount.toString()}`);
        
        // Verify the account details
        try {
            const feeAccountInfo = await connection.getAccountInfo(poolFeeAccount);
            console.log(COLOR_DIM, `🔍 Pool fee account owner: ${feeAccountInfo?.owner.toString()}`);
            console.log(COLOR_DIM, `🔍 Pool fee account exists: ${!!feeAccountInfo}`);
            console.log(COLOR_DIM, `🔍 Pool fee account data length: ${feeAccountInfo?.data.length}`);
        } catch (error) {
            console.log(COLOR_ERROR, `❌ Error checking pool fee account: ${error.message}`);
        }

        // Determine token programs based on the actual token types, not order
        const mdogeTokenProgram = anchor_spl.TOKEN_2022_PROGRAM_ID; // mDOGE is always Token-2022
        const wsolTokenProgram = TOKEN_PROGRAM_ID; // WSOL is always standard SPL
        
        // Assign programs based on which token is token0/token1
        const token0Program = isMdogeToken0 ? mdogeTokenProgram : wsolTokenProgram;
        const token1Program = isMdogeToken0 ? wsolTokenProgram : mdogeTokenProgram;
        
        console.log(COLOR_DIM, `🔍 Token program assignment:`);
        console.log(COLOR_DIM, `   mDOGE program: ${mdogeTokenProgram.toString()}`);
        console.log(COLOR_DIM, `   WSOL program: ${wsolTokenProgram.toString()}`);
        console.log(COLOR_DIM, `   token0Program: ${token0Program.toString()} (${isMdogeToken0 ? 'mDOGE' : 'WSOL'})`);
        console.log(COLOR_DIM, `   token1Program: ${token1Program.toString()} (${isMdogeToken0 ? 'WSOL' : 'mDOGE'})`);
        
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
            const creatorToken0Info = await connection.getAccountInfo(creatorMdogeAccount);
            const creatorToken1Info = await connection.getAccountInfo(creatorWsolAccount.address);
            
            console.log(COLOR_DIM, `   creatorMdogeAccount exists: ${!!creatorToken0Info}`);
            console.log(COLOR_DIM, `   creatorMdogeAccount owner: ${creatorToken0Info?.owner.toString()}`);
            console.log(COLOR_DIM, `   creatorWsolAccount exists: ${!!creatorToken1Info}`);
            console.log(COLOR_DIM, `   creatorWsolAccount owner: ${creatorToken1Info?.owner.toString()}`);
            
        } catch (error) {
            console.log(COLOR_WARNING, `⚠️ Could not verify token ownership: ${error.message}`);
        }

        // Determine creator token accounts based on token order
        const creatorToken0 = isMdogeToken0 ? creatorMdogeAccount : creatorWsolAccount.address;
        const creatorToken1 = isMdogeToken0 ? creatorWsolAccount.address : creatorMdogeAccount;

        // Determine initial amounts based on token order
        const initAmount0 = isMdogeToken0 ? POOL_CONFIG.initialMdogeAmount : POOL_CONFIG.initialSolAmount;
        const initAmount1 = isMdogeToken0 ? POOL_CONFIG.initialSolAmount : POOL_CONFIG.initialMdogeAmount;

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

        console.log(COLOR_DIM, '🔍 Pool initialization parameters:');
        console.log(COLOR_DIM, `   initAmount0: ${initAmount0.toString()} (${isMdogeToken0 ? 'mDOGE' : 'WSOL'})`);
        console.log(COLOR_DIM, `   initAmount1: ${initAmount1.toString()} (${isMdogeToken0 ? 'WSOL' : 'mDOGE'})`);
        console.log(COLOR_DIM, `   openTime: ${POOL_CONFIG.openTime} (${new Date(POOL_CONFIG.openTime * 1000).toLocaleString()})`);

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
                    openTime: POOL_CONFIG.openTime,
                    accounts: poolAccounts
                }
            );

            console.log(COLOR_SUCCESS, '✅ mDOGE-SOL pool created successfully!');
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
            deploymentData.mdoge_sol_pool_created = {
                poolStatePDA: poolStatePDA.toString(),
                lpMintPDA: lpMintPDA.toString(),
                token0VaultPDA: token0VaultPDA.toString(),
                token1VaultPDA: token1VaultPDA.toString(),
                authorityPDA: authorityPDA.toString(),
                observationStatePDA: observationStatePDA.toString(),
                token0Mint: token0Mint.toString(),
                token1Mint: token1Mint.toString(),
                isMdogeToken0: isMdogeToken0,
                txid: poolTxid,
                wrapTxid: wrapTxid,
                initialMdogeAmount: POOL_CONFIG.initialMdogeAmount.toString(),
                initialSolAmount: POOL_CONFIG.initialSolAmount.toString(),
                openTime: POOL_CONFIG.openTime
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
        console.log(COLOR_INFO, 'ℹ️ mDOGE-SOL pool already exists');
            }
}

async function addInitialLiquidity(connection, cpProgram, deployer, deploymentData, deploymentPath) {
    // Check if already added
    if (deploymentData.mdoge_sol_liquidity_added) {
        console.log(COLOR_INFO, 'ℹ️ Liquidity already added to pool');
        console.log(COLOR_INFO, '🔑 LP Tokens Received:', deploymentData.mdoge_sol_liquidity_added.lpTokensReceived);
        return;
    }

    // Get mDOGE mint
    const mdogeMint = new PublicKey(deploymentData.mdoge_mint_address);

    // =================== [ ADDING LIQUIDITY TO POOL ] ===================
    if (!deploymentData.mdoge_sol_liquidity_added) {
        console.log(COLOR_STEP, '\n=================== [ ADDING LIQUIDITY TO POOL ] ===================');
        console.log(COLOR_INFO, '💧 Adding liquidity to mDOGE-SOL pool...');
        
        // Get token accounts (recreate them since they were in pool creation scope)
        const wsolMintKey = new PublicKey("So11111111111111111111111111111111111111112");
        const creatorWsolAccount = await getOrCreateAssociatedTokenAccount(
            connection,
            deployer,
            wsolMintKey,
            deployer.publicKey
        );
        
        const creatorMdogeAccount = await getAssociatedTokenAddress(
            mdogeMint,
            deployer.publicKey,
            false,
            anchor_spl.TOKEN_2022_PROGRAM_ID
        );
        
        // Get pool data from deployment
        const poolData = deploymentData.mdoge_sol_pool_created;
        const poolStatePDA = new PublicKey(poolData.poolStatePDA);
        const lpMintPDA = new PublicKey(poolData.lpMintPDA);
        const token0VaultPDA = new PublicKey(poolData.token0VaultPDA);
        const token1VaultPDA = new PublicKey(poolData.token1VaultPDA);
        const authorityPDA = new PublicKey(poolData.authorityPDA);
        const isMdogeToken0 = poolData.isMdogeToken0;
        
        const creatorLpAccount = await getAssociatedTokenAddress(
            lpMintPDA,
            deployer.publicKey
        );
        
        // Define liquidity amounts
        const liquidityAmount0 = new BN("5000000000"); // 5 WSOL
        const liquidityAmount1 = new BN("5000000000000"); // 5000 mDOGE
        const maxSlippage = new BN("100000000"); // 0.1 SOL max slippage
        const maxSlippageMdoge = new BN("100000000000"); // 100 mDOGE max slippage
        
        console.log(COLOR_DIM, `🔍 Liquidity parameters:`);
        console.log(COLOR_DIM, `   Amount0 (WSOL): ${liquidityAmount0.toString()} (${liquidityAmount0.toNumber() / 1e9} WSOL)`);
        console.log(COLOR_DIM, `   Amount1 (mDOGE): ${liquidityAmount1.toString()} (${liquidityAmount1.toNumber() / 1e9} mDOGE)`);
        console.log(COLOR_DIM, `   Max slippage0: ${maxSlippage.toString()}`);
        console.log(COLOR_DIM, `   Max slippage1: ${maxSlippageMdoge.toString()}`);
        
        // Check current balances
        const wsolBalance = await anchor_spl.getAccount(connection, creatorWsolAccount.address, undefined, TOKEN_PROGRAM_ID);
        const mdogeBalance = await anchor_spl.getAccount(connection, creatorMdogeAccount, undefined, anchor_spl.TOKEN_2022_PROGRAM_ID);
        
        console.log(COLOR_DIM, `🔍 Current balances:`);
        console.log(COLOR_DIM, `   WSOL: ${wsolBalance.amount.toString()} (${Number(wsolBalance.amount) / 1e9} WSOL)`);
        console.log(COLOR_DIM, `   mDOGE: ${mdogeBalance.amount.toString()} (${Number(mdogeBalance.amount) / 1e9} mDOGE)`);
        
        // Verify we have enough tokens
        if (wsolBalance.amount < liquidityAmount0.toNumber()) {
            console.log(COLOR_WARNING, `⚠️ Insufficient WSOL balance. Need ${liquidityAmount0.toNumber() / 1e9} WSOL, have ${Number(wsolBalance.amount) / 1e9} WSOL`);
            console.log(COLOR_INFO, '💰 Wrapping more SOL to WSOL...');
            
            const additionalSol = liquidityAmount0.toNumber() - Number(wsolBalance.amount) + 1e9; // Add 1 extra SOL buffer
            const wrapTx = new Transaction().add(
                SystemProgram.transfer({
                    fromPubkey: deployer.publicKey,
                    toPubkey: creatorWsolAccount.address,
                    lamports: additionalSol,
                }),
                anchor_spl.createSyncNativeInstruction(creatorWsolAccount.address, TOKEN_PROGRAM_ID)
            );
            
            const wrapSig = await web3.sendAndConfirmTransaction(connection, wrapTx, [deployer]);
            console.log(COLOR_SUCCESS, `✅ Wrapped additional ${additionalSol / 1e9} SOL to WSOL`);
            console.log(COLOR_DIM, `🔗 Wrap Transaction: ${wrapSig}`);
        }
        
        if (mdogeBalance.amount < liquidityAmount1.toNumber()) {
            throw new Error(`Insufficient mDOGE balance. Need ${liquidityAmount1.toNumber() / 1e9} mDOGE, have ${Number(mdogeBalance.amount) / 1e9} mDOGE`);
        }
        
        // Determine token accounts based on token order
        const token0Account = isMdogeToken0 ? creatorMdogeAccount : creatorWsolAccount.address;
        const token1Account = isMdogeToken0 ? creatorWsolAccount.address : creatorMdogeAccount;
        
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
            vault0Mint: isMdogeToken0 ? mdogeMint : wsolMintKey,
            vault1Mint: isMdogeToken0 ? wsolMintKey : mdogeMint,
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
            
            // Use the deposit function from helper
            const liquidityResult = await cpDepositLiquidity(
                connection,
                cpProgram,
                deployer,
               {
                   lpAmount: liquidityAmount0, // Use amount0 as LP amount reference
                   maxToken0: liquidityAmount0.add(maxSlippage),
                   maxToken1: liquidityAmount1.add(maxSlippageMdoge),
                   accounts: liquidityAccounts
               }
            );
            
            console.log(COLOR_SUCCESS, '✅ Liquidity added successfully!');
            console.log(COLOR_DIM, `🔗 Transaction: ${liquidityResult.txid}`);
            console.log(COLOR_DIM, `🔍 Explorer URL: https://explorer.solana.com/tx/${liquidityResult.txid}?cluster=${CLUSTER}`);
            
            // Check LP token balance
            const lpBalance = await anchor_spl.getAccount(connection, creatorLpAccount, undefined, TOKEN_PROGRAM_ID);
            console.log(COLOR_SUCCESS, `💧 LP tokens received: ${lpBalance.amount.toString()} (${Number(lpBalance.amount) / 1e9} LP)`);
            
            // Update deployment status
            deploymentData.mdoge_sol_liquidity_added = {
                timestamp: new Date().toISOString(),
                amount0: liquidityAmount0.toString(),
                amount1: liquidityAmount1.toString(),
                lpTokensReceived: lpBalance.amount.toString(),
                txid: liquidityResult.txid
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

async function burnLpTokens(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.lp_tokens_burned) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ LP tokens already burned. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔥 Burned Amount:', deploymentData.lp_tokens_burned.burned_amount);
        console.log('\x1b[36m%s\x1b[0m', '🔗 Burn Transaction:', deploymentData.lp_tokens_burned.burn_signature);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ BURNING LP TOKENS ] ===================');
    
    console.log('\x1b[36m%s\x1b[0m', '🔥 LP Token Burning Configuration:');
    console.log('\x1b[36m%s\x1b[0m', `   • Pool: ${deploymentData.mdoge_sol_pool_created.poolStatePDA}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Purpose: Permanent liquidity lock`);
    console.log('\x1b[36m%s\x1b[0m', `   • Effect: Cannot remove initial liquidity`);
    
    try {
        // Get the LP token mint from the pool creation data
        const lpTokenMint = new PublicKey(deploymentData.mdoge_sol_pool_created.lpMintPDA);
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
                    pool_state: deploymentData.mdoge_sol_pool_created.poolStatePDA,
                    burn_all_lp_tokens: true,
                    burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
                    mdoge_locked_amount: deploymentData.mdoge_sol_pool_created.initialMdogeAmount,
                    sol_locked_amount: deploymentData.mdoge_sol_pool_created.initialSolAmount,
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
                pool_state: deploymentData.mdoge_sol_pool_created.poolStatePDA,
            burn_all_lp_tokens: true,
            burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
                mdoge_locked_amount: deploymentData.mdoge_sol_pool_created.initialMdogeAmount,
                sol_locked_amount: deploymentData.mdoge_sol_pool_created.initialSolAmount,
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
            pool_state: deploymentData.mdoge_sol_pool_created.poolStatePDA,
            lp_mint: lpTokenMint.toString(),
            deployer_lp_account: deployerLpAccount.toString(),
            burn_all_lp_tokens: true,
            burned_amount: lpBalance.toString(),
            burned_amount_readable: `${Number(lpBalance) / 1e9} LP`,
            burn_signature: burnSignature,
            burn_purpose: "Permanent liquidity lock - cannot remove initial liquidity",
            mdoge_locked_amount: deploymentData.mdoge_sol_pool_created.initialMdogeAmount,
            sol_locked_amount: deploymentData.mdoge_sol_pool_created.initialSolAmount,
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

function printCompletionSummary(deploymentData) {
    console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ POOL CONFIGURATION COMPLETE ================================');
    console.log('\x1b[32m%s\x1b[0m', '✅ mDOGE-SOL pool production configuration completed!');
    
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Pool Summary:');
    console.log('\x1b[36m%s\x1b[0m', `  • Network: ${CLUSTER}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Pool Type: Raydium CP-Swap`);
    console.log('\x1b[36m%s\x1b[0m', `  • Trade Fee: ${config.raydium.trade_fee_rate / 10000}%`);
    console.log('\x1b[36m%s\x1b[0m', `  • Initial SOL: ${config.raydium.initial_sol_amount / 1e9} SOL`);
    if (deploymentData.raydium_pool_initialized) {
        console.log('\x1b[36m%s\x1b[0m', `  • Initial mDOGE: ${deploymentData.raydium_pool_initialized.pool_mdoge_readable}`);
        console.log('\x1b[36m%s\x1b[0m', `  • mDOGE Percentage: ${deploymentData.raydium_pool_initialized.mdoge_percentage_for_pool}% of total supply`);
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
        console.log('\x1b[32m%s\x1b[0m', `   Locked mDOGE: ${deploymentData.lp_tokens_burned.mdoge_locked_amount}`);
        console.log('\x1b[32m%s\x1b[0m', `   Locked SOL: ${deploymentData.lp_tokens_burned.sol_locked_amount}`);
        console.log('\x1b[32m%s\x1b[0m', `   🔒 Permanent Lock: ${deploymentData.lp_tokens_burned.permanent_lock ? 'YES' : 'NO'}`);
    }
        
    console.log('\x1b[33m%s\x1b[0m', '\n📋 PRODUCTION READY:');
    console.log('\x1b[33m%s\x1b[0m', '   • Pool configuration calculated and validated');
    console.log('\x1b[33m%s\x1b[0m', '   • Liquidity amounts: 1% mDOGE supply + 10 SOL');
    console.log('\x1b[33m%s\x1b[0m', '   • LP tokens configured for burning (permanent lock)');
    console.log('\x1b[33m%s\x1b[0m', '   • Ready for Raydium SDK integration');
    
    console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
    console.log('\x1b[36m%s\x1b[0m', '📁 Pool configuration saved to:', path.resolve(__dirname, config.deployment.paths.deployments_dir, `${CLUSTER}.json`));
    console.log('\x1b[36m%s\x1b[0m', '🔄 Ready for next steps: MoonBase program initialization');
}
  