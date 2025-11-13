// Import Anchor as CommonJS package
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3, Wallet } = pkg;
import { SystemProgram } from '@solana/web3.js';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import path from 'path';

// Get the current file's directory
const __dirname = new URL('.', import.meta.url).pathname;

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

// Load deployment data
const deploymentDir = path.resolve(__dirname, './deployments');
const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);

let deploymentFile = {};
if (fs.existsSync(deploymentPath)) {
    deploymentFile = JSON.parse(fs.readFileSync(deploymentPath, 'utf-8'));
} else {
    if (!fs.existsSync(deploymentDir)) {
        fs.mkdirSync(deploymentDir, { recursive: true });
    }
    console.log(COLOR_WARNING, '⚠️ No deployment file found. Starting fresh deployment.');
}

// Get deployed addresses
const MOONDOGE_TOKEN_MINT = deploymentFile.dbtc_mint_address ? 
    new PublicKey(deploymentFile.dbtc_mint_address) : null;

const ID_MOONBASE_PROGRAM = deploymentFile.MOON_BASE_PROGRAM_ID ? 
    new PublicKey(deploymentFile.MOON_BASE_PROGRAM_ID) : null;

const RAYDIUM_PROGRAM_ID = deploymentFile.RAYDIUM_CP_PROGRAM_ID;

// Mining configuration
const MINING_START_TIMESTAMP = config.mining.start_timestamp || Math.floor(Date.now() / 1000);
const MINING_DOGE_BTC_PER_SLOT = new BN(config.mining.doge_btc_per_slot);
const DBTC_DEPOSIT_AMOUNT = new BN(config.mining.initial_deposit);

// Load MoonBase Program IDL
const IDL_MOONBASE = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, config.deployment.paths.moonbase_idl), 'utf-8')
);

// Solana Connection
const connection = new Connection(RPC_URL, COMMITMENT);

// Load wallet keypair
const walletKeypair = (() => {
    try {
        const walletPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
        return Keypair.fromSecretKey(
            new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf-8')))
        );
    } catch (e) {
        console.error(COLOR_ERROR, "❌ Failed to load wallet keypair:", e);
        console.error(COLOR_ERROR, `   Expected path: ${path.resolve(__dirname, config.deployment.paths.deployer_key || 'undefined')}`);
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
const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });
setProvider(provider);

// Helper function to save deployment data
function saveDeploymentData() {
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentFile, null, 2));
    console.log(COLOR_SUCCESS, '✅ Deployment file updated');
}

async function getSolanaBalance(pubkey) {
    try {
        return await connection.getBalance(pubkey);
    } catch (error) {
        console.error(COLOR_ERROR, `❌ Error getting SOL balance: ${error.message}`);
        throw error;
    }
}

// ==================== [ MAIN SCRIPT ] ====================

async function main() {
    console.log(COLOR_STEP, '🚀 ================================ DogeTech Faction Surge Initialization ================================');
    console.log(COLOR_INFO, '👤 Admin Wallet:', walletKeypair.publicKey.toString());
    console.log(COLOR_INFO, '🌐 Network:', CLUSTER);
    console.log(COLOR_INFO, '🔗 RPC URL:', RPC_URL);
    
    const balance = await getSolanaBalance(walletKeypair.publicKey);
    console.log(COLOR_INFO, '💰 Balance:', balance / 1e9, 'SOL');

    // Verify prerequisites
    if (!MOONDOGE_TOKEN_MINT) {
        console.error(COLOR_ERROR, '❌ DOGE_BTC token mint address not found in deployment file.');
        console.log(COLOR_WARNING, '⚠️ Please run 1_init_mdoge_token.js first.');
        return;
    }

    if (!ID_MOONBASE_PROGRAM) {
        console.error(COLOR_ERROR, '❌ MoonBase program ID not found in deployment file.');
        console.log(COLOR_WARNING, '⚠️ Please run 0_deploy_game.js first.');
        return;
    }

    console.log(COLOR_STEP, '============================== [ PROGRAMS ] ===============================');
    console.log(COLOR_INFO, '🚀 MoonBase Program ID:', ID_MOONBASE_PROGRAM.toString());
    console.log(COLOR_INFO, '🪙 DOGE_BTC Token Mint:', MOONDOGE_TOKEN_MINT.toString());

    const moonbaseProgram = new Program(IDL_MOONBASE, provider);
    console.log(COLOR_SUCCESS, '✅ Connected to program:', moonbaseProgram.programId.toString());

    try {
        // 1. Initialize MoonBase Program (GlobalConfig + DogeBtcMining + SOL Treasury)
        await initializeMoonbaseProgram(moonbaseProgram);

        // 2. Initialize System Accounts (Referral + Buybacks)
        await initializeSystemAccounts(moonbaseProgram);

        // 3. Add Factions (12 factions for the raffle)
        await addFactions(moonbaseProgram);

        // 4. Initialize Mining System (Token Vault + Mining Parameters)
        await initializeMiningSystem(moonbaseProgram);

        // 5. Deposit Mining Tokens
        await depositMiningTokens(moonbaseProgram);

        // 6. Set Raydium Pool State (for price discovery and swaps)
        await setRaydiumPoolState(moonbaseProgram);

        // 7. Initialize EggConfig
        await initializeEggConfig(moonbaseProgram);

        // 8. Create Dragon Egg Collection
        await createDragonEggCollection(moonbaseProgram);

        // 9. Set Dragon Egg URIs (one per faction)
        await setDragonEggUris(moonbaseProgram);

        // 10. Initialize Dragon Egg Royalties
        await initializeDragonEggRoyalties(moonbaseProgram);

        // 11. Configure Ticket Tiers (for Dragon Egg minting)
        await configureTicketTiers(moonbaseProgram);

        // 12. Initialize Tax Config (for tax distribution)
        await initializeTaxConfig(moonbaseProgram);

        // 13. Initialize Game State (for Faction Surge rounds)
        await initializeGameState(moonbaseProgram);

        // 14. Initialize LP Token Accounts (for Raydium integration)
        await initializeLpTokenAccounts(moonbaseProgram);

        // Print completion summary
        printCompletionSummary();

    } catch (error) {
        console.error(COLOR_ERROR, '❌ Initialization failed:', error);
        if (error.logs) {
            console.error(COLOR_ERROR, '📝 Transaction logs:');
            error.logs.forEach(log => console.error(COLOR_DIM, log));
        }
        process.exit(1);
    }
}

// ==================== [ INITIALIZATION FUNCTIONS ] ====================

async function initializeMoonbaseProgram(moonbaseProgram) {
    if (deploymentFile.moonbase_program_initialized) {
        console.log(COLOR_INFO, 'ℹ️ MoonBase program already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n====================== [ INITIALIZING MOONBASE PROGRAM ] ====================');

    // Derive PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('global-config')],
        moonbaseProgram.programId
    );

    const [dogeBtcMiningPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('moon-doge-mining')],
        moonbaseProgram.programId
    );

    const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('sol-treasury')],
        moonbaseProgram.programId
    );

    const feeRecipient = new PublicKey(config.deployment.fee_recipient);

    console.log(COLOR_INFO, `🔑 Global Config PDA: ${globalConfigPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 DogeBtc Mining PDA: ${dogeBtcMiningPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 SOL Treasury PDA: ${solTreasuryPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 Fee Recipient: ${feeRecipient.toString()}`);

    try {
        const tx = await moonbaseProgram.methods
            .initialize(feeRecipient)
            .accounts({
                globalConfig: globalConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                solTreasury: solTreasuryPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Program initialized successfully!');
        console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);

        deploymentFile.moonbase_program_initialized = {
            globalConfig_address: globalConfigPDA.toString(),
            dogeBtcMining_address: dogeBtcMiningPDA.toString(),
            solTreasury_address: solTreasuryPDA.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
            console.log(COLOR_INFO, 'ℹ️ Program already initialized. Skipping...');
            deploymentFile.moonbase_program_initialized = {
                globalConfig_address: globalConfigPDA.toString(),
                dogeBtcMining_address: dogeBtcMiningPDA.toString(),
                solTreasury_address: solTreasuryPDA.toString(),
            };
            saveDeploymentData();
        } else {
            throw error;
        }
    }
}

async function initializeSystemAccounts(moonbaseProgram) {
    if (deploymentFile.system_accounts_initialized) {
        console.log(COLOR_INFO, 'ℹ️ System accounts already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ INITIALIZING SYSTEM ACCOUNTS ] ================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);

    // Derive PDAs
    const [systemReferralRewardsPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('referral-rewards'), SystemProgram.programId.toBuffer()],
        moonbaseProgram.programId
    );

    const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('buybacks')],
        moonbaseProgram.programId
    );

    const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('buybacks-sol-vault')],
        moonbaseProgram.programId
    );

    console.log(COLOR_INFO, `🔑 System Referral Rewards PDA: ${systemReferralRewardsPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 Buybacks Account PDA: ${buybacksAccountPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 Buybacks SOL Vault PDA: ${buybacksSolVaultPDA.toString()}`);

    try {
        const tx = await moonbaseProgram.methods
            .initializeSystemAccounts()
            .accounts({
                globalConfig: globalConfigPDA,
                systemReferralRewards: systemReferralRewardsPDA,
                buybacksAccount: buybacksAccountPDA,
                buybacksSolVault: buybacksSolVaultPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ System accounts initialized!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.system_accounts_initialized = {
            system_referral_rewards_pda: systemReferralRewardsPDA.toString(),
            buybacks_account_pda: buybacksAccountPDA.toString(),
            buybacks_sol_vault_pda: buybacksSolVaultPDA.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
            console.log(COLOR_INFO, 'ℹ️ System accounts already initialized. Skipping...');
            deploymentFile.system_accounts_initialized = {
                system_referral_rewards_pda: systemReferralRewardsPDA.toString(),
                buybacks_account_pda: buybacksAccountPDA.toString(),
                buybacks_sol_vault_pda: buybacksSolVaultPDA.toString(),
            };
            saveDeploymentData();
        } else {
            throw error;
        }
    }
}

async function addFactions(moonbaseProgram) {
    if (deploymentFile.factions_added) {
        console.log(COLOR_INFO, 'ℹ️ Factions already added. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ ADDING FACTIONS ] ================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const addedFactions = [];

    console.log(COLOR_INFO, `📝 Adding ${config.factions.length} factions...`);

    for (let i = 0; i < config.factions.length; i++) {
        const faction = config.factions[i];
        const factionId = i;

        // Derive FactionState PDA
        const [factionStatePDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('faction'), Buffer.from([factionId])],
            moonbaseProgram.programId
        );

        console.log(`   ${i + 1}. ${faction.name} (ID: ${factionId})`);

        try {
            const tx = await moonbaseProgram.methods
                .addFaction(faction.name)
                .accounts({
                    globalConfig: globalConfigPDA,
                    factionState: factionStatePDA,
                    authority: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc();

            console.log(COLOR_SUCCESS, `      ✅ Added: ${faction.name}`);
            addedFactions.push({
                faction_id: factionId,
                name: faction.name,
                faction_state_pda: factionStatePDA.toString(),
                tx_signature: tx
            });
        } catch (error) {
            if (error.toString().includes("already in use") || error.toString().includes("MaxFactionsReached")) {
                console.log(COLOR_WARNING, `      ⚠️ ${faction.name} may already exist`);
                addedFactions.push({
                    faction_id: factionId,
                    name: faction.name,
                    faction_state_pda: factionStatePDA.toString(),
                    status: 'already_exists'
                });
            } else {
                throw error;
            }
        }
    }

    console.log(COLOR_SUCCESS, `✅ ${addedFactions.length} factions configured!`);

    deploymentFile.factions_added = {
        factions: addedFactions,
        timestamp: new Date().toISOString()
    };
    saveDeploymentData();
}

async function initializeMiningSystem(moonbaseProgram) {
    if (deploymentFile.mining_vault_initialized) {
        console.log(COLOR_INFO, 'ℹ️ Mining system already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ INITIALIZING MINING SYSTEM ] ===================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created?.poolStatePDA;

    if (!raydiumPoolState) {
        console.error(COLOR_ERROR, '❌ Raydium pool state not found in deployment file.');
        console.log(COLOR_WARNING, '⚠️ Please run 2_init_mdoge_SOL_pool.js first.');
        return;
    }

    const [vaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('dbtc_vault'), dogeBtcMiningPDA.toBuffer()],
        moonbaseProgram.programId
    );

    const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('mdoge-vault-authority')],
        moonbaseProgram.programId
    );

    console.log(COLOR_INFO, `🔑 Mining Token Vault PDA: ${vaultPDA.toString()}`);
    console.log(COLOR_INFO, `🔑 Vault Authority PDA: ${vaultAuthorityPDA.toString()}`);
    console.log(COLOR_INFO, `⏰ Start Timestamp: ${MINING_START_TIMESTAMP}`);
    console.log(COLOR_INFO, `💰 DogeBtc Per Slot: ${MINING_DOGE_BTC_PER_SLOT.toString()}`);
    console.log(COLOR_INFO, `🔄 Raydium Pool State: ${raydiumPoolState}`);

    try {
        const tx = await moonbaseProgram.methods
            .initializeMining(
                new BN(MINING_START_TIMESTAMP),
                MINING_DOGE_BTC_PER_SLOT,
                new PublicKey(raydiumPoolState)
            )
            .accounts({
                globalConfig: globalConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                vaultAuthority: vaultAuthorityPDA,
                tokenVault: vaultPDA,
                tokenMint: MOONDOGE_TOKEN_MINT,
                tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
                rent: web3.SYSVAR_RENT_PUBKEY,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Mining system initialized!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.mining_vault_initialized = {
            vault_address: vaultPDA.toString(),
            vault_authority: vaultAuthorityPDA.toString(),
            start_timestamp: MINING_START_TIMESTAMP,
            doge_btc_per_slot: MINING_DOGE_BTC_PER_SLOT.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("MiningAlreadyInitialized")) {
            console.log(COLOR_INFO, 'ℹ️ Mining already initialized. Skipping...');
            deploymentFile.mining_vault_initialized = {
                vault_address: vaultPDA.toString(),
                vault_authority: vaultAuthorityPDA.toString(),
                start_timestamp: MINING_START_TIMESTAMP,
                doge_btc_per_slot: MINING_DOGE_BTC_PER_SLOT.toString(),
            };
            saveDeploymentData();
        } else {
            throw error;
        }
    }
}

async function depositMiningTokens(moonbaseProgram) {
    if (deploymentFile.mining_tokens_deposited) {
        console.log(COLOR_INFO, 'ℹ️ Mining tokens already deposited. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ DEPOSITING MINING TOKENS ] ================');

    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const vaultPDA = new PublicKey(deploymentFile.mining_vault_initialized.vault_address);

    // Get user's token account
    const userTokenAccount = await anchor_spl.getAssociatedTokenAddress(
        MOONDOGE_TOKEN_MINT,
        wallet.publicKey,
        false,
        anchor_spl.TOKEN_2022_PROGRAM_ID
    );

    console.log(COLOR_INFO, `💰 Depositing ${DBTC_DEPOSIT_AMOUNT.toString()} tokens...`);
    console.log(COLOR_INFO, `   From: ${userTokenAccount.toString()}`);
    console.log(COLOR_INFO, `   To: ${vaultPDA.toString()}`);

    try {
        const tx = await moonbaseProgram.methods
            .depositDogeBtcTokens(DBTC_DEPOSIT_AMOUNT)
            .accounts({
                depositor: wallet.publicKey,
                depositorTokenAccount: userTokenAccount,
                dbtcTokenVault: vaultPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                tokenMint: MOONDOGE_TOKEN_MINT,
                tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Mining tokens deposited successfully!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.mining_tokens_deposited = {
            amount: DBTC_DEPOSIT_AMOUNT.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to deposit mining tokens:', error);
        throw error;
    }
}

async function setRaydiumPoolState(moonbaseProgram) {
    if (deploymentFile.raydium_pool_state_set) {
        console.log(COLOR_INFO, 'ℹ️ Raydium pool state already set. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ SETTING RAYDIUM POOL STATE ] ===================');

    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created?.poolStatePDA;

    if (!raydiumPoolState) {
        console.error(COLOR_ERROR, '❌ Raydium pool state not found in deployment file.');
        console.log(COLOR_WARNING, '⚠️ Please run 2_init_mdoge_SOL_pool.js first.');
        return;
    }

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);
    const poolStatePubkey = new PublicKey(raydiumPoolState);

    console.log(COLOR_INFO, `🔑 Pool State Address: ${poolStatePubkey.toString()}`);

    try {
        const tx = await moonbaseProgram.methods
            .setRaydiumPoolState(poolStatePubkey)
            .accounts({
                globalConfig: globalConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Raydium pool state set successfully!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.raydium_pool_state_set = {
            pool_state_address: poolStatePubkey.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to set Raydium pool state:', error);
        throw error;
    }
}

async function initializeEggConfig(moonbaseProgram) {
    if (deploymentFile.egg_config_initialized) {
        console.log(COLOR_INFO, 'ℹ️ EggConfig already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ INITIALIZING EGG CONFIG ] ===================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);

    const [eggsConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('egg-config')],
        moonbaseProgram.programId
    );

    // Get egg config values
    const basePrice = config.dragon_eggs.base_price || new BN(100_000_000); // 0.1 SOL
    const curveA = config.dragon_eggs.curve_a || new BN(10_000); // Curve steepness
    const maxSupply = config.dragon_eggs.max_supply || new BN(10000); // Max 10k eggs

    console.log(COLOR_INFO, `🔑 EggConfig PDA: ${eggsConfigPDA.toString()}`);
    console.log(COLOR_INFO, `💰 Base Price: ${basePrice / 1e9} SOL`);
    console.log(COLOR_INFO, `📈 Curve A: ${curveA}`);
    console.log(COLOR_INFO, `🥚 Max Supply: ${maxSupply}`);

    try {
        const tx = await moonbaseProgram.methods
            .initializeEggConfig(
                new BN(basePrice),
                new BN(curveA),
                new BN(maxSupply)
            )
            .accounts({
                eggsConfig: eggsConfigPDA,
                globalConfig: globalConfigPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ EggConfig initialized successfully!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.egg_config_initialized = {
            eggs_config_pda: eggsConfigPDA.toString(),
            base_price: basePrice.toString(),
            curve_a: curveA.toString(),
            max_supply: maxSupply.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
            console.log(COLOR_INFO, 'ℹ️ EggConfig already initialized. Skipping...');
            deploymentFile.egg_config_initialized = {
                eggs_config_pda: eggsConfigPDA.toString(),
            };
            saveDeploymentData();
        } else {
            console.error(COLOR_ERROR, '❌ Failed to initialize EggConfig:', error);
            throw error;
        }
    }
}

async function createDragonEggCollection(moonbaseProgram) {
    if (deploymentFile.dragon_egg_collection_created) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg collection already created');
        console.log(COLOR_INFO, '🔑 Collection Address:', deploymentFile.dragon_egg_collection_created.collection_address);
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ CREATING DRAGON EGG COLLECTION ] ===================');

    // Derive PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('global-config')],
        moonbaseProgram.programId
    );

    const [eggsConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('egg-config')],
        moonbaseProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('collection_authority')],
        moonbaseProgram.programId
    );

    console.log(COLOR_INFO, '🎨 Creating Metaplex Core collection...');
    console.log(COLOR_DIM, `   Name: ${config.dragon_eggs.collection_name}`);
    console.log(COLOR_DIM, `   URI: ${config.dragon_eggs.collection_uri}`);
    console.log(COLOR_INFO, '🔐 Collection Authority PDA:', collectionAuthorityPDA.toString());

    // Generate a new keypair for the collection
    const collectionKeypair = Keypair.generate();

    try {
        const tx = await moonbaseProgram.methods
            .createDragonEggCollection(
                config.dragon_eggs.collection_name,
                config.dragon_eggs.collection_uri
            )
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                eggsConfig: eggsConfigPDA,
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
        console.log(COLOR_DIM, `   Transaction: ${tx}`);
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/address/${collectionPubkey.toString()}?cluster=${CLUSTER}`);

        deploymentFile.dragon_egg_collection_created = {
            collection_address: collectionPubkey.toString(),
            collection_name: config.dragon_eggs.collection_name,
            collection_uri: config.dragon_eggs.collection_uri,
            update_authority: collectionAuthorityPDA.toString(),
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to create collection:', error);
        throw error;
    }
}

async function setDragonEggUris(moonbaseProgram) {
    if (!deploymentFile.dragon_egg_collection_created) {
        console.error(COLOR_ERROR, '❌ Dragon Egg collection must be created first');
        throw new Error('Collection not created');
    }

    if (deploymentFile.dragon_egg_uris_set) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg URIs already set');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ SETTING DRAGON EGG URIS ] ===================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);

    const [eggsConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('egg-config')],
        moonbaseProgram.programId
    );

    console.log(COLOR_INFO, '📝 Setting URIs:', config.dragon_eggs.uris.length);
    config.dragon_eggs.uris.forEach((uri, index) => {
        console.log(COLOR_DIM, `   ${index + 1}. ${uri}`);
    });

    try {
        const tx = await moonbaseProgram.methods
            .setDragonEggUris(config.dragon_eggs.uris)
            .accounts({
                globalConfig: globalConfigPDA,
                eggsConfig: eggsConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                authority: walletKeypair.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Dragon Egg URIs set successfully!');
        console.log(COLOR_DIM, '🔗 Transaction:', tx);

        deploymentFile.dragon_egg_uris_set = {
            uris: config.dragon_eggs.uris,
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to set Dragon Egg URIs:', error);
        throw error;
    }
}

async function initializeDragonEggRoyalties(moonbaseProgram) {
    if (deploymentFile.dragon_egg_royalties_initialized) {
        console.log(COLOR_INFO, 'ℹ️ Dragon Egg royalties already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n=================== [ INITIALIZING DRAGON EGG ROYALTIES ] ===================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const collectionPubkey = new PublicKey(deploymentFile.dragon_egg_collection_created.collection_address);

    const [eggsConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('egg-config')],
        moonbaseProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('collection_authority')],
        moonbaseProgram.programId
    );

    // Configure royalties
    const basisPoints = config.dragon_eggs.royalties?.basis_points || 500; // 5%
    const creators = config.dragon_eggs.royalties?.creators || [
        { address: walletKeypair.publicKey, percentage: 100 }
    ];

    console.log(COLOR_INFO, `💎 Royalty: ${basisPoints / 100}%`);
    console.log(COLOR_INFO, `👥 Creators: ${creators.length}`);
    creators.forEach((creator, idx) => {
        console.log(COLOR_DIM, `   ${idx + 1}. ${creator.address} (${creator.percentage}%)`);
    });

    try {
        const tx = await moonbaseProgram.methods
            .initDragonEggRoyalties(
                basisPoints,
                creators
            )
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                eggsConfig: eggsConfigPDA,
                collection: collectionPubkey,
                collectionAuthority: collectionAuthorityPDA,
                mplCoreProgram: new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"),
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Dragon Egg royalties initialized!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.dragon_egg_royalties_initialized = {
            basis_points: basisPoints,
            creators: creators,
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to initialize royalties:', error);
        throw error;
    }
}

async function configureTicketTiers(moonbaseProgram) {
    if (deploymentFile.ticket_tier_configs_initialized) {
        console.log(COLOR_INFO, 'ℹ️ Ticket tier configs already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ CONFIGURING TICKET TIERS ] ================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);
    const dogeBtcMiningPDA = new PublicKey(deploymentFile.moonbase_program_initialized.dogeBtcMining_address);

    const [eggsConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('egg-config')],
        moonbaseProgram.programId
    );

    const ticketTiers = config.dragon_eggs.ticket_tiers || [];

    console.log(COLOR_INFO, `📝 Adding ${ticketTiers.length} ticket tier configs...`);

    const addedTiers = [];

    for (const tier of ticketTiers) {
        console.log(`   Tier ${tier.tier_index}: ${tier.ticket_value / 1e9} SOL × ${tier.ticket_count} tickets`);

        try {
            const tx = await moonbaseProgram.methods
                .addTicketTierConfig(
                    tier.tier_index,
                    new BN(tier.ticket_value),
                    tier.ticket_count
                )
                .accounts({
                    globalConfig: globalConfigPDA,
                    eggsConfig: eggsConfigPDA,
                    dogeBtcMining: dogeBtcMiningPDA,
                    authority: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc();

            console.log(COLOR_SUCCESS, `      ✅ Tier ${tier.tier_index} configured`);
            addedTiers.push({ ...tier, tx_signature: tx });
        } catch (error) {
            console.error(COLOR_ERROR, `❌ Failed to add tier ${tier.tier_index}:`, error);
            throw error;
        }
    }

    console.log(COLOR_SUCCESS, '✅ All ticket tier configs initialized!');

    deploymentFile.ticket_tier_configs_initialized = {
        ticket_tiers: addedTiers,
        timestamp: new Date().toISOString()
    };
    saveDeploymentData();
}

async function initializeTaxConfig(moonbaseProgram) {
    if (deploymentFile.tax_config_initialized) {
        console.log(COLOR_INFO, 'ℹ️ Tax config already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ INITIALIZING TAX CONFIG ] ================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);

    // Derive PDAs
    const [taxConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('tax-config')],
        moonbaseProgram.programId
    );

    const [withdrawWithheldAuthorityPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('withdraw-withheld-authority')],
        moonbaseProgram.programId
    );

    const [factionTreasuryVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('faction-treasury-vault')],
        moonbaseProgram.programId
    );

    const [nftFloorSweepVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('nft-floor-sweep-vault')],
        moonbaseProgram.programId
    );

    const [nftSaleSolVaultPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('nft-sale-sol-vault')],
        moonbaseProgram.programId
    );

    // Get config values
    const whitelistedAddress = config.tax.nft_floor_sweep_whitelisted_address;
    const nftFloorSweepPct = config.tax.nft_floor_sweep_pct;
    const factionTreasuryPct = config.tax.faction_treasury_pct;
    const burnPct = 100 - nftFloorSweepPct - factionTreasuryPct;

    console.log(COLOR_INFO, `💰 Tax Distribution:`);
    console.log(COLOR_INFO, `   NFT Floor Sweep: ${nftFloorSweepPct}%`);
    console.log(COLOR_INFO, `   Faction Treasury: ${factionTreasuryPct}%`);
    console.log(COLOR_INFO, `   Burn: ${burnPct}%`);
    console.log(COLOR_INFO, `🔑 Whitelisted Address: ${whitelistedAddress}`);

    try {
        const tx = await moonbaseProgram.methods
            .initializeTaxConfig(
                nftFloorSweepPct,
                factionTreasuryPct,
                new PublicKey(whitelistedAddress)
            )
            .accounts({
                globalConfig: globalConfigPDA,
                taxConfig: taxConfigPDA,
                dbtcMint: MOONDOGE_TOKEN_MINT,
                withdrawWithheldAuthority: withdrawWithheldAuthorityPDA,
                factionTreasuryVault: factionTreasuryVaultPDA,
                nftFloorSweepVault: nftFloorSweepVaultPDA,
                nftSaleSolVault: nftSaleSolVaultPDA,
                authority: wallet.publicKey,
                tokenProgram2022: anchor_spl.TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Tax config initialized successfully!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.tax_config_initialized = {
            tax_config_pda: taxConfigPDA.toString(),
            withdraw_withheld_authority: withdrawWithheldAuthorityPDA.toString(),
            faction_treasury_vault: factionTreasuryVaultPDA.toString(),
            nft_floor_sweep_vault: nftFloorSweepVaultPDA.toString(),
            nft_sale_sol_vault: nftSaleSolVaultPDA.toString(),
            nft_floor_sweep_pct: nftFloorSweepPct,
            faction_treasury_pct: factionTreasuryPct,
            whitelisted_address: whitelistedAddress,
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to initialize tax config:', error);
        throw error;
    }
}

async function initializeGameState(moonbaseProgram) {
    if (deploymentFile.game_state_initialized) {
        console.log(COLOR_INFO, 'ℹ️ Game state already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ INITIALIZING GAME STATE ] ================');

    const globalConfigPDA = new PublicKey(deploymentFile.moonbase_program_initialized.globalConfig_address);

    // Derive GlobalGameState PDA
    const [globalGameStatePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from('global-game-state')],
        moonbaseProgram.programId
    );

    const roundDurationSeconds = config.game.round_duration_seconds;

    console.log(COLOR_INFO, `🔑 Global Game State PDA: ${globalGameStatePDA.toString()}`);
    console.log(COLOR_INFO, `⏱️ Round Duration: ${roundDurationSeconds} seconds (${roundDurationSeconds / 3600} hours)`);

    try {
        const tx = await moonbaseProgram.methods
            .initializeGameState(new BN(roundDurationSeconds))
            .accounts({
                globalGameState: globalGameStatePDA,
                globalConfig: globalConfigPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ Game state initialized successfully!');
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.game_state_initialized = {
            global_game_state_pda: globalGameStatePDA.toString(),
            round_duration_seconds: roundDurationSeconds,
            tx_signature: tx,
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
            console.log(COLOR_INFO, 'ℹ️ Game state already initialized. Skipping...');
            deploymentFile.game_state_initialized = {
                global_game_state_pda: globalGameStatePDA.toString(),
                round_duration_seconds: roundDurationSeconds,
            };
            saveDeploymentData();
        } else {
            console.error(COLOR_ERROR, '❌ Failed to initialize game state:', error);
            throw error;
        }
    }
}

async function initializeLpTokenAccounts(moonbaseProgram) {
    if (deploymentFile.lp_token_accounts_initialized) {
        console.log(COLOR_INFO, 'ℹ️ LP token accounts already initialized. Skipping...');
        return;
    }

    console.log(COLOR_STEP, '\n================ [ INITIALIZING LP TOKEN ACCOUNTS ] ================');

    try {
        if (!deploymentFile.dbtc_sol_pool_created?.lpMintPDA) {
            console.log(COLOR_WARNING, '⚠️ LP mint not found in deployment file. Cannot initialize LP token accounts.');
            return;
        }

        if (!deploymentFile.mining_vault_initialized?.vault_authority) {
            console.log(COLOR_WARNING, '⚠️ Vault authority not found. Cannot initialize LP token accounts.');
            return;
        }

        const lpMint = new PublicKey(deploymentFile.dbtc_sol_pool_created.lpMintPDA);
        const vaultAuthority = new PublicKey(deploymentFile.mining_vault_initialized.vault_authority);

        // For Raydium deposit, LP token account must be owned by vault authority
        const lpTokenAccount = await anchor_spl.getAssociatedTokenAddress(
            lpMint,
            vaultAuthority,
            true,
            anchor_spl.TOKEN_PROGRAM_ID
        );

        // Check if LP token account already exists
        const lpAccountInfo = await connection.getAccountInfo(lpTokenAccount);
        if (lpAccountInfo) {
            console.log(COLOR_INFO, 'ℹ️ LP token accounts already exist. Skipping...');
            deploymentFile.lp_token_accounts_initialized = {
                lp_token_account: lpTokenAccount.toString(),
                lp_token_owner: vaultAuthority.toString(),
                lp_mint: lpMint.toString(),
            };
            saveDeploymentData();
            return;
        }

        console.log(COLOR_INFO, '🔄 Initializing LP token accounts...');
        console.log(COLOR_DIM, `   LP Token Account (ATA): ${lpTokenAccount.toString()}`);
        console.log(COLOR_DIM, `   LP Token Owner (Vault Authority): ${vaultAuthority.toString()}`);
        console.log(COLOR_DIM, `   LP Mint: ${lpMint.toString()}`);

        // Create associated token account
        const createdAccount = await anchor_spl.getOrCreateAssociatedTokenAccount(
            connection,
            walletKeypair,
            lpMint,
            vaultAuthority,
            true,
            'confirmed',
            {},
            anchor_spl.TOKEN_PROGRAM_ID
        );

        console.log(COLOR_SUCCESS, '✅ LP token accounts initialized successfully!');
        console.log(COLOR_DIM, `   LP Token Account: ${createdAccount.address.toString()}`);

        deploymentFile.lp_token_accounts_initialized = {
            lp_token_account: createdAccount.address.toString(),
            lp_token_owner: vaultAuthority.toString(),
            lp_mint: lpMint.toString(),
            timestamp: new Date().toISOString()
        };
        saveDeploymentData();
    } catch (error) {
        console.error(COLOR_ERROR, '❌ Failed to initialize LP token accounts:', error);
        console.log(COLOR_WARNING, '   This may not be critical - LP accounts can be created on-demand');
    }
}

function printCompletionSummary() {
    console.log(COLOR_STEP, '\n🎉 ================================ INITIALIZATION COMPLETE ================================');
    console.log(COLOR_SUCCESS, '✅ All systems initialized successfully!');
    console.log(COLOR_INFO, '\n📋 Summary:');
    console.log(COLOR_INFO, `  • MoonBase Program: ${deploymentFile.moonbase_program_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • System Accounts: ${deploymentFile.system_accounts_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Factions: ${deploymentFile.factions_added ? config.factions.length + ' added ✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Mining System: ${deploymentFile.mining_vault_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Mining Tokens: ${deploymentFile.mining_tokens_deposited ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Raydium Pool State: ${deploymentFile.raydium_pool_state_set ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Dragon Egg Collection: ${deploymentFile.dragon_egg_collection_created ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Dragon Egg URIs: ${deploymentFile.dragon_egg_uris_set ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Dragon Egg Royalties: ${deploymentFile.dragon_egg_royalties_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Ticket Tiers: ${deploymentFile.ticket_tier_configs_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Tax Config: ${deploymentFile.tax_config_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • Game State: ${deploymentFile.game_state_initialized ? '✅' : '❌'}`);
    console.log(COLOR_INFO, `  • LP Token Accounts: ${deploymentFile.lp_token_accounts_initialized ? '✅' : '❌'}`);
    console.log(COLOR_STEP, '========================================================================================');

    if (deploymentFile.moonbase_program_initialized) {
        console.log(COLOR_DIM, '\n🔑 Important Addresses:');
        console.log(COLOR_DIM, `   Global Config: ${deploymentFile.moonbase_program_initialized.globalConfig_address}`);
        console.log(COLOR_DIM, `   Mining State: ${deploymentFile.moonbase_program_initialized.dogeBtcMining_address}`);
        console.log(COLOR_DIM, `   SOL Treasury: ${deploymentFile.moonbase_program_initialized.solTreasury_address}`);
        if (deploymentFile.mining_vault_initialized) {
            console.log(COLOR_DIM, `   Mining Vault: ${deploymentFile.mining_vault_initialized.vault_address}`);
        }
        if (deploymentFile.dragon_egg_collection_created) {
            console.log(COLOR_DIM, `   Dragon Egg Collection: ${deploymentFile.dragon_egg_collection_created.collection_address}`);
        }
        if (deploymentFile.game_state_initialized) {
            console.log(COLOR_DIM, `   Game State: ${deploymentFile.game_state_initialized.global_game_state_pda}`);
        }
    }

    console.log(COLOR_INFO, '\n📝 Next Steps:');
    console.log(COLOR_INFO, '   1. Users can now initialize their PlayerData accounts');
    console.log(COLOR_INFO, '   2. Users can mint Dragon Eggs for their factions');
    console.log(COLOR_INFO, '   3. Users can stake DogeBtc and LP tokens');
    console.log(COLOR_INFO, '   4. Admins can start game rounds with start_round');
    console.log(COLOR_INFO, '   5. Keeper bots can harvest and distribute tax via crank functions');
}

// Run the main script
main().catch(console.error);
