import {
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    SystemProgram,
    Transaction,
} from "@solana/web3.js";
import {
    TOKEN_2022_PROGRAM_ID,
    ExtensionType,
    createInitializeMintInstruction,
    createInitializeMetadataPointerInstruction,
    getMintLen,
    createInitializeTransferFeeConfigInstruction,
    mintTo,
    getOrCreateAssociatedTokenAccount,
    LENGTH_SIZE,
    TYPE_SIZE,
    setAuthority,
    AuthorityType,
    getMint,
} from "@solana/spl-token";
import {
    createInitializeInstruction,
    pack,
} from "@solana/spl-token-metadata";
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { 
    getSolanaBalance, 
    updateDeploymentStatus, 
    createMintAccount,
    createMintAccountWithMetadata 
} from './helper.js';

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Token metadata configuration
const TOKEN_METADATA = {
    name: config.token.name,
    symbol: config.token.symbol,
    description: config.token.description,
    image: config.token.image,
    external_url: config.token.external_url,
    attributes: [
        {
            trait_type: "Generation",
            value: "1"
        },
        {
            trait_type: "Type",
            value: "Utility Token"
        },
        {
            trait_type: "Network",
            value: CLUSTER
        }
    ],
    properties: {
        files: [
            {
                uri: config.token.image,
                type: "image/png"
            }
        ],
        category: "cryptocurrency"
    }
};

// ============================================================================
// ========== MAIN DEPLOYMENT SCRIPT =========================================
// ============================================================================

(async () => {
    console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ DogeTech mDOGE Token Deployment ================================');
    console.log('\x1b[36m%s\x1b[0m', '🌐 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🔗 RPC URL:', RPC_URL);
    console.log('\x1b[36m%s\x1b[0m', '🪙 Token Symbol:', config.token.symbol);
    console.log('\x1b[36m%s\x1b[0m', '📊 Initial Supply:', config.token.initial_supply.toLocaleString());


    // Setup connection
    const connection = await initializeConnection();
    
    // Setup deployer account
    const deployer = await setupDeployerAccount();
    console.log('\x1b[36m%s\x1b[0m', '👤 Deployer Address:', deployer.publicKey.toBase58());

    let deployer_balance = await getSolanaBalance(connection, deployer.publicKey);
    console.log('\x1b[36m%s\x1b[0m', '💰 Deployer Balance:', deployer_balance / 1e9, 'SOL');
   
    
    
    // Load or create deployment state
    const { deploymentData, deploymentPath } = loadDeploymentState();
    // return;
    
    try {
        // 1. Create mint account with metadata
        await createMintAccountTx(connection, deployer, deploymentData, deploymentPath);
        
        // 2. Create token account
        await createTokenAccount(connection, deployer, deploymentData, deploymentPath);

        // 3. Mint initial supply
        await mintInitialSupply(connection, deployer, deploymentData, deploymentPath);

        // 4. Remove mint authority (make token non-mintable)
        await removeMintAuthority(connection, deployer, deploymentData, deploymentPath);

        // 5. Remove withdraw withheld authority
        await removeWithdrawWithheldAuthority(connection, deployer, deploymentData, deploymentPath);

        // 6. Transfer transfer fee config authority
        await transferTransferFeeConfigAuthority(connection, deployer, deploymentData, deploymentPath);

        // Print completion summary
        printCompletionSummary(deploymentData);
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Deployment failed:', error);
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

async function setupDeployerAccount() {
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
            console.log('\x1b[36m%s\x1b[0m', '🆕 Creating new deployer account...');
            deployer = Keypair.generate();
            
            // Create directory if it doesn't exist
            const deployerDir = path.dirname(deployerPath);
            if (!fs.existsSync(deployerDir)) {
                fs.mkdirSync(deployerDir, { recursive: true });
            }
            
            fs.writeFileSync(deployerPath, JSON.stringify(Array.from(deployer.secretKey)));
            console.log('\x1b[32m%s\x1b[0m', '✅ New deployer account created and saved');
        }
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error handling deployer account:', error);
        process.exit(1);
    }
    
    return deployer;
}
 

function loadDeploymentState() {
    console.log('\x1b[33m%s\x1b[0m', '📋 Loading deployment state...');
    
    const deploymentDir = path.resolve(__dirname, config.deployment.paths.deployments_dir);
    const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);
    
    // Create deployments directory if it doesn't exist
    if (!fs.existsSync(deploymentDir)) {
        fs.mkdirSync(deploymentDir, { recursive: true });
    }
    
    let deploymentData = {};
    if (fs.existsSync(deploymentPath)) {
        deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
        console.log('\x1b[32m%s\x1b[0m', '✅ Found existing deployment data');
    } else {
        console.log('\x1b[36m%s\x1b[0m', '🆕 Creating new deployment state');
    }
    
    return { deploymentData, deploymentPath };
}

async function createMintAccountTx(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.mdoge_mint_created) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ mDOGE mint account already exists. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔑 Mint Address:', deploymentData.mdoge_mint_created.mint_address);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ CREATING MDOGE MINT ACCOUNT WITH METADATA ] ===================');
    
    // Generate mint keypair
    const moonDogeMint = Keypair.generate();
    const mintPubkey = moonDogeMint.publicKey;
    
    console.log('\x1b[36m%s\x1b[0m', '🔑 Generated Mint Address:', mintPubkey.toBase58());
    
    // Setup mint parameters from config
    const decimals = config.token.decimals;
    const burnTaxBps = config.token.burn_tax_bps;
    const maxBurnAmount = config.token.max_burn_amount;
    
    // Authority configuration
    const mintAuthority = deployer.publicKey;
    const freezeAuthority = null; // No freeze authority
    const transferFeeConfigAuthority = deployer.publicKey;
    const withdrawWithheldAuthority = deployer.publicKey;
    
    // Prepare metadata
    const metadata = {
        mint: mintPubkey,
        name: TOKEN_METADATA.name,
        symbol: TOKEN_METADATA.symbol,
        uri: TOKEN_METADATA.image,
        additionalMetadata: [
            ['description', TOKEN_METADATA.description],
            ['generation', '1'],
            ['type', 'Utility Token'],
            ['network', CLUSTER]
        ],
    };
    
    console.log('\x1b[36m%s\x1b[0m', '⚙️ Mint Configuration:');
    console.log('\x1b[36m%s\x1b[0m', `   • Decimals: ${decimals}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Burn Tax: ${burnTaxBps / 100}%`);
    console.log('\x1b[36m%s\x1b[0m', `   • Max Burn: ${maxBurnAmount.toLocaleString()} tokens`);
    console.log('\x1b[36m%s\x1b[0m', `   • Mint Authority: ${mintAuthority.toBase58()}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Freeze Authority: ${freezeAuthority || 'None'}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Metadata: ${metadata.name} (${metadata.symbol})`);
    
    try {
        const signature = await createMintAccountWithMetadata(
            connection, deployer, moonDogeMint, burnTaxBps, maxBurnAmount, decimals,
            mintAuthority, freezeAuthority, transferFeeConfigAuthority, withdrawWithheldAuthority,
            metadata
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Mint account with metadata created successfully!');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', signature);
        
        // Update deployment data
        deploymentData.mdoge_mint_created = {
            mint_address: mintPubkey.toBase58(),
            mint_authority: mintAuthority.toBase58(),
            freeze_authority: freezeAuthority,
            transfer_fee_config_authority: transferFeeConfigAuthority.toBase58(),
            withdraw_withheld_authority: withdrawWithheldAuthority.toBase58(),
            decimals: decimals,
            burn_tax_bps: burnTaxBps,
            max_burn_amount: maxBurnAmount,
            metadata_included: true,
            metadata_name: metadata.name,
            metadata_symbol: metadata.symbol,
            metadata_uri: metadata.uri,
            creation_signature: signature,
            timestamp: new Date().toISOString()
        };
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to create mint account with metadata:', error);
        throw error;
    }
}

async function createTokenAccount(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.mdoge_token_account_created) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ mDOGE token account already exists. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔑 Token Account:', deploymentData.mdoge_token_account_created.token_account_address);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ CREATING TOKEN ACCOUNT ] ===================');
    
    const mintPubkey = new PublicKey(deploymentData.mdoge_mint_created.mint_address);
    
    try {
        const tokenAccount = await getOrCreateAssociatedTokenAccount(
            connection,
            deployer, // payer
            mintPubkey, // mint
            deployer.publicKey, // owner
            undefined, // allowOwnerOffCurve
            undefined, // commitment
            undefined, // confirmOptions
            TOKEN_2022_PROGRAM_ID // programId
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Token account created successfully!');
        console.log('\x1b[36m%s\x1b[0m', '🔑 Token Account Address:', tokenAccount.address.toBase58());
        console.log('\x1b[36m%s\x1b[0m', '👤 Owner:', deployer.publicKey.toBase58());
        
        // Update deployment data
        deploymentData.mdoge_token_account_created = {
            token_account_address: tokenAccount.address.toBase58(),
            owner_address: deployer.publicKey.toBase58(),
            mint_address: mintPubkey.toBase58(),
            timestamp: new Date().toISOString()
        };
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to create token account:', error);
        throw error;
    }
}

async function mintInitialSupply(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.initial_supply_minted) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Initial supply already minted. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '💰 Amount:', deploymentData.initial_supply_minted.amount);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ MINTING INITIAL SUPPLY ] ===================');
    
    const mintPubkey = new PublicKey(deploymentData.mdoge_mint_created.mint_address);
    const tokenAccountAddress = new PublicKey(deploymentData.mdoge_token_account_created.token_account_address);
    
    // Use string-based BigInt calculation to avoid any number conversion issues
    const initialSupplyString = config.token.initial_supply.toString();
    const decimalsString = config.token.decimals.toString();
    
    console.log('\x1b[36m%s\x1b[0m', '🔢 BigInt Calculation Debug:');
    console.log('\x1b[36m%s\x1b[0m', `   • Initial Supply (string): "${initialSupplyString}"`);
    console.log('\x1b[36m%s\x1b[0m', `   • Decimals (string): "${decimalsString}"`);
    
    // Create BigInt from string to ensure no precision loss
    const supplyBigInt = BigInt(initialSupplyString);
    const decimalsBigInt = BigInt(decimalsString);
    const multiplierBigInt = BigInt(10) ** decimalsBigInt;
    const finalAmountBigInt = supplyBigInt * multiplierBigInt;
    
    console.log('\x1b[36m%s\x1b[0m', `   • Supply BigInt: ${supplyBigInt.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Multiplier BigInt: ${multiplierBigInt.toString()}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Final Amount BigInt: ${finalAmountBigInt.toString()}`);
    
 
    console.log('\x1b[36m%s\x1b[0m', '💰 Minting Details:');
    console.log('\x1b[36m%s\x1b[0m', `   • Target Amount: ${config.token.initial_supply.toLocaleString()} ${config.token.symbol}`);
 
    
    // Double-check the BigInt is truly a BigInt type
    console.log('\x1b[36m%s\x1b[0m', '🔍 Type Verification:');
    console.log('\x1b[36m%s\x1b[0m', `   • Type of finalAmountBigInt: ${typeof finalAmountBigInt}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Is BigInt: ${typeof finalAmountBigInt === 'bigint'}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Constructor: ${finalAmountBigInt.constructor.name}`);
    
    try {
        console.log('\x1b[33m%s\x1b[0m', '📡 Sending mintTo transaction with BigInt...');
        
        const signature = await mintTo(
            connection,
            deployer, // payer
            mintPubkey, // mint
            tokenAccountAddress, // destination
            deployer, // authority
            finalAmountBigInt, // amount as pure BigInt
            [], // multiSigners
            undefined, // confirmOptions
            TOKEN_2022_PROGRAM_ID // programId
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Initial supply minted successfully!');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', signature);
        
        // Immediately verify the mint supply after minting
        console.log('\x1b[33m%s\x1b[0m', '🔍 Verifying mint supply...');
        const mintInfo = await getMint(connection, mintPubkey, 'confirmed', TOKEN_2022_PROGRAM_ID);
        const actualSupply = mintInfo.supply.toString();
        
        console.log('\x1b[36m%s\x1b[0m', '✅ Post-Mint Verification:');
        console.log('\x1b[36m%s\x1b[0m', `   • Expected Supply: ${finalAmountBigInt.toString()}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Actual Supply: ${actualSupply}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Supply Match: ${actualSupply === finalAmountBigInt.toString() ? '✅' : '❌'}`);
        
        if (actualSupply !== finalAmountBigInt.toString()) {
            console.log('\x1b[31m%s\x1b[0m', '⚠️ WARNING: Minted supply does not match expected amount!');
            console.log('\x1b[31m%s\x1b[0m', `   • Difference: ${(BigInt(actualSupply) - finalAmountBigInt).toString()}`);
        }
        
        // Update deployment data
        deploymentData.initial_supply_minted = {
            amount: finalAmountBigInt.toString(),
            actual_minted_amount: actualSupply,
            amount_readable: `${config.token.initial_supply.toLocaleString()} ${config.token.symbol}`,
            token_account_address: tokenAccountAddress.toBase58(),
            mint_signature: signature,
            bigint_verification: {
                expected: finalAmountBigInt.toString(),
                actual: actualSupply,
                match: actualSupply === finalAmountBigInt.toString()
            },
            timestamp: new Date().toISOString()
        };
        
        // Store mint address at top level for easy access
        deploymentData.mdoge_mint_address = mintPubkey.toBase58();
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to mint initial supply:', error);
        throw error;
    }
}

async function removeMintAuthority(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.mint_authority_removed) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Mint authority already removed. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔒 Token is non-mintable');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ REMOVING MINT AUTHORITY ] ===================');
    
    const mintPubkey = new PublicKey(deploymentData.mdoge_mint_created.mint_address);
    
    console.log('\x1b[36m%s\x1b[0m', '🔒 Making token non-mintable by removing mint authority...');
    console.log('\x1b[36m%s\x1b[0m', `   • Current Mint Authority: ${deploymentData.mdoge_mint_created.mint_authority}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Action: Set mint authority to null`);
    
    try {
        const signature = await setAuthority(
            connection,
            deployer, // payer
            mintPubkey, // mint
            deployer, // current authority
            AuthorityType.MintTokens, // authority type
            null, // new authority (null removes it)
            [], // multiSigners
            undefined, // confirmOptions
            TOKEN_2022_PROGRAM_ID // programId
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Mint authority removed successfully!');
        console.log('\x1b[32m%s\x1b[0m', '🔒 Token is now non-mintable - no additional tokens can ever be created');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', signature);
        
        // Update deployment data
        deploymentData.mint_authority_removed = {
            previous_mint_authority: deploymentData.mdoge_mint_created.mint_authority,
            new_mint_authority: null,
            removal_signature: signature,
            timestamp: new Date().toISOString(),
            total_supply_locked: deploymentData.initial_supply_minted.amount_readable
        };
        
        // Update the mint creation data to reflect removed authority
        deploymentData.mdoge_mint_created.mint_authority = null;
        deploymentData.mdoge_mint_created.mint_authority_status = "removed";
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to remove mint authority:', error);
        throw error;
    }
}

async function removeWithdrawWithheldAuthority(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.withdraw_withheld_authority_removed) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Withdraw withheld authority already removed. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔒 Withdraw withheld authority is null');
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ REMOVING WITHDRAW WITHHELD AUTHORITY ] ===================');
    
    const mintPubkey = new PublicKey(deploymentData.mdoge_mint_created.mint_address);
    
    console.log('\x1b[36m%s\x1b[0m', '🔒 Removing withdraw withheld authority...');
    console.log('\x1b[36m%s\x1b[0m', `   • Current Withdraw Withheld Authority: ${deploymentData.mdoge_mint_created.withdraw_withheld_authority}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Action: Set withdraw withheld authority to null`);
    
    try {
        const signature = await setAuthority(
            connection,
            deployer, // payer
            mintPubkey, // mint
            deployer, // current authority
            AuthorityType.WithheldWithdraw, // authority type
            null, // new authority (null removes it)
            [], // multiSigners
            undefined, // confirmOptions
            TOKEN_2022_PROGRAM_ID // programId
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Withdraw withheld authority removed successfully!');
        console.log('\x1b[32m%s\x1b[0m', '🔒 Withheld tokens can no longer be withdrawn by anyone');
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', signature);
        
        // Update deployment data
        deploymentData.withdraw_withheld_authority_removed = {
            previous_withdraw_withheld_authority: deploymentData.mdoge_mint_created.withdraw_withheld_authority,
            new_withdraw_withheld_authority: null,
            removal_signature: signature,
            timestamp: new Date().toISOString()
        };
        
        // Update the mint creation data to reflect removed authority
        deploymentData.mdoge_mint_created.withdraw_withheld_authority = null;
        deploymentData.mdoge_mint_created.withdraw_withheld_authority_status = "removed";
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to remove withdraw withheld authority:', error);
        throw error;
    }
}

async function transferTransferFeeConfigAuthority(connection, deployer, deploymentData, deploymentPath) {
    if (deploymentData.transfer_fee_config_authority_transferred) {
        console.log('\x1b[34m%s\x1b[0m', 'ℹ️ Transfer fee config authority already transferred. Skipping...');
        console.log('\x1b[36m%s\x1b[0m', '🔑 Current Authority:', deploymentData.transfer_fee_config_authority_transferred.new_transfer_fee_config_authority);
        return;
    }

    console.log('\x1b[35m%s\x1b[0m', '\n=================== [ TRANSFERRING TRANSFER FEE CONFIG AUTHORITY ] ===================');
    
    const mintPubkey = new PublicKey(deploymentData.mdoge_mint_created.mint_address);
    const newAuthority = new PublicKey(config.deployment.transfer_fee_config_authority);
    
    console.log('\x1b[36m%s\x1b[0m', '🔄 Transferring transfer fee config authority...');
    console.log('\x1b[36m%s\x1b[0m', `   • Current Transfer Fee Config Authority: ${deploymentData.mdoge_mint_created.transfer_fee_config_authority}`);
    console.log('\x1b[36m%s\x1b[0m', `   • New Transfer Fee Config Authority: ${newAuthority.toBase58()}`);
    console.log('\x1b[36m%s\x1b[0m', `   • Action: Transfer authority to configured address`);
    
    try {
        const signature = await setAuthority(
            connection,
            deployer, // payer
            mintPubkey, // mint
            deployer, // current authority
            AuthorityType.TransferFeeConfig, // authority type
            newAuthority, // new authority
            [], // multiSigners
            undefined, // confirmOptions
            TOKEN_2022_PROGRAM_ID // programId
        );
        
        console.log('\x1b[32m%s\x1b[0m', '✅ Transfer fee config authority transferred successfully!');
        console.log('\x1b[32m%s\x1b[0m', `🔑 New authority can now update transfer fee configuration`);
        console.log('\x1b[90m%s\x1b[0m', '🔗 Transaction:', signature);
        
        // Update deployment data
        deploymentData.transfer_fee_config_authority_transferred = {
            previous_transfer_fee_config_authority: deploymentData.mdoge_mint_created.transfer_fee_config_authority,
            new_transfer_fee_config_authority: newAuthority.toBase58(),
            transfer_signature: signature,
            timestamp: new Date().toISOString()
        };
        
        // Update the mint creation data to reflect new authority
        deploymentData.mdoge_mint_created.transfer_fee_config_authority = newAuthority.toBase58();
        deploymentData.mdoge_mint_created.transfer_fee_config_authority_status = "transferred";
        
        // Save deployment data
        fs.writeFileSync(deploymentPath, JSON.stringify(deploymentData, null, 2));
        console.log('\x1b[32m%s\x1b[0m', '✅ Deployment data saved');
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Failed to transfer transfer fee config authority:', error);
        throw error;
    }
}

function printCompletionSummary(deploymentData) {
    console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ DEPLOYMENT COMPLETE ================================');
    console.log('\x1b[32m%s\x1b[0m', '✅ mDOGE token deployment completed successfully!');
    
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Deployment Summary:');
    console.log('\x1b[36m%s\x1b[0m', `  • Network: ${CLUSTER}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Token Name: ${config.token.name}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Token Symbol: ${config.token.symbol}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Initial Supply: ${config.token.initial_supply.toLocaleString()}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Decimals: ${config.token.decimals}`);
    console.log('\x1b[36m%s\x1b[0m', `  • Burn Tax: ${config.token.burn_tax_bps / 100}%`);
    
    console.log('\x1b[90m%s\x1b[0m', '\n🔑 Important Addresses:');
    if (deploymentData.mdoge_mint_created) {
        console.log('\x1b[90m%s\x1b[0m', `   Mint Address: ${deploymentData.mdoge_mint_created.mint_address}`);
        if (deploymentData.mdoge_mint_created.metadata_included) {
            console.log('\x1b[90m%s\x1b[0m', `   Metadata: ${deploymentData.mdoge_mint_created.metadata_name} (${deploymentData.mdoge_mint_created.metadata_symbol})`);
            console.log('\x1b[90m%s\x1b[0m', `   Metadata Location: Built into mint account (Token-2022 native)`);
        }
        
        // Mint Authority Status
        if (deploymentData.mint_authority_removed) {
            console.log('\x1b[32m%s\x1b[0m', `   🔒 Mint Authority: REMOVED - Token is non-mintable`);
            console.log('\x1b[32m%s\x1b[0m', `   🔒 Total Supply: ${deploymentData.mint_authority_removed.total_supply_locked} (LOCKED FOREVER)`);
        } else {
            console.log('\x1b[90m%s\x1b[0m', `   Mint Authority: ${deploymentData.mdoge_mint_created.mint_authority || 'None'}`);
        }
        
        // Withdraw Withheld Authority Status
        if (deploymentData.withdraw_withheld_authority_removed) {
            console.log('\x1b[32m%s\x1b[0m', `   🔒 Withdraw Withheld Authority: REMOVED - Withheld tokens locked`);
        } else {
            console.log('\x1b[90m%s\x1b[0m', `   Withdraw Withheld Authority: ${deploymentData.mdoge_mint_created.withdraw_withheld_authority || 'None'}`);
        }
        
        // Transfer Fee Config Authority Status
        if (deploymentData.transfer_fee_config_authority_transferred) {
            console.log('\x1b[33m%s\x1b[0m', `   🔄 Transfer Fee Config Authority: TRANSFERRED`);
            console.log('\x1b[33m%s\x1b[0m', `   🔑 New Authority: ${deploymentData.transfer_fee_config_authority_transferred.new_transfer_fee_config_authority}`);
        } else {
            console.log('\x1b[90m%s\x1b[0m', `   Transfer Fee Config Authority: ${deploymentData.mdoge_mint_created.transfer_fee_config_authority || 'None'}`);
        }
    }
    if (deploymentData.mdoge_token_account_created) {
        console.log('\x1b[90m%s\x1b[0m', `   Token Account: ${deploymentData.mdoge_token_account_created.token_account_address}`);
    }
    if (deploymentData.initial_supply_minted) {
        console.log('\x1b[90m%s\x1b[0m', `   Initial Supply Minted: ${deploymentData.initial_supply_minted.amount_readable}`);
    }
    
    console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
    console.log('\x1b[36m%s\x1b[0m', '📁 Deployment data saved to:', path.resolve(__dirname, config.deployment.paths.deployments_dir, `${CLUSTER}.json`));
    console.log('\x1b[36m%s\x1b[0m', '🔄 Ready for next steps: Pool creation and MoonBase initialization');
}
  