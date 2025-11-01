#!/usr/bin/env node

import { Connection, Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3, Wallet } = pkg;
import { MPL_CORE_PROGRAM_ID } from '@metaplex-foundation/mpl-core';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// =============================================================================
// ======================== CONFIGURATION =====================================
// =============================================================================

// Load configuration
const CONFIG_PATH = path.join(__dirname, 'config.json');
const config = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment || 'confirmed';

// Console colors
const COLOR_SUCCESS = '\x1b[32m%s\x1b[0m';
const COLOR_ERROR = '\x1b[31m%s\x1b[0m';
const COLOR_WARNING = '\x1b[33m%s\x1b[0m';
const COLOR_INFO = '\x1b[36m%s\x1b[0m';
const COLOR_DIM = '\x1b[90m%s\x1b[0m';
const COLOR_STEP = '\x1b[35m%s\x1b[0m';

// Dragon egg asset seeds
const DRAGON_EGG_ASSET_SEED = "dragon_egg_asset";
const DRAGON_EGG_METADATA_SEED = "dragon-egg-metadata";

// =============================================================================
// ======================== MAIN EXECUTION ====================================
// =============================================================================

async function main() {
    console.log(COLOR_STEP, '\n🌟 =============== CREATE USER MOONBASE =============== 🌟');
    console.log(COLOR_INFO, `📍 Network: ${CLUSTER}`);
    console.log(COLOR_INFO, `🔗 RPC URL: ${RPC_URL}\n`);

    // Get command line arguments
    const args = process.argv.slice(2);
    if (args.length < 1) {
        console.error(COLOR_ERROR, '❌ Usage: node 5_create_user_moonbase.js <pricing_tier> [referrer_address]');
        console.error(COLOR_WARNING, 'Pricing tiers:');
        console.error(COLOR_WARNING, '  1 = 0.5 SOL (no Dragon Egg)');
        console.error(COLOR_WARNING, '  2 = 1.42 SOL (with Dragon Egg + 10k electricity)');
        console.error(COLOR_WARNING, '  3 = 2.42 SOL (with Dragon Egg + 30k electricity)');
        console.error(COLOR_WARNING, '  4 = 4.20 SOL (with Dragon Egg + 75k electricity)');
        process.exit(1);
    }

    const pricingTier = parseInt(args[0]);
    const referrerAddress = args[1] || null;

    if (pricingTier < 1 || pricingTier > 4) {
        console.error(COLOR_ERROR, '❌ Invalid pricing tier. Must be 1, 2, 3, or 4');
        process.exit(1);
    }

    // Tier prices in lamports
    const TIER_PRICES = {
        1: 0.5 * LAMPORTS_PER_SOL,
        2: 1.42 * LAMPORTS_PER_SOL,
        3: 2.42 * LAMPORTS_PER_SOL,
        4: 4.20 * LAMPORTS_PER_SOL
    };

    const tierPrice = TIER_PRICES[pricingTier];
    const includesDragonEgg = pricingTier > 1;

    console.log(COLOR_INFO, `💰 Selected Tier ${pricingTier}: ${tierPrice / LAMPORTS_PER_SOL} SOL`);
    console.log(COLOR_INFO, `🥚 Includes Dragon Egg: ${includesDragonEgg ? 'Yes' : 'No'}`);
    if (referrerAddress) {
        console.log(COLOR_INFO, `👥 Referrer: ${referrerAddress}`);
    }

    try {
        // Load deployment data
        const deploymentPath = path.join(__dirname, 'deployments', `${CLUSTER}.json`);
        const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));

        if (!deploymentData.MOON_BASE_PROGRAM_ID) {
            throw new Error('MoonBase program not deployed. Run 0_deploy_game.js first');
        }

        // Connect to Solana
        const connection = new Connection(RPC_URL, COMMITMENT);

        // Load user wallet
        const walletPath = path.resolve(__dirname, config.deployment.paths.deployer_key);
        const userKeypair = Keypair.fromSecretKey(
            new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf8')))
        );
        const wallet = new Wallet(userKeypair);
        const provider = new AnchorProvider(connection, wallet, { commitment: COMMITMENT });

        console.log(COLOR_INFO, `🔑 User wallet: ${userKeypair.publicKey.toString()}`);

        // Check wallet balance
        const balance = await connection.getBalance(userKeypair.publicKey);
        console.log(COLOR_INFO, `💳 Wallet balance: ${balance / LAMPORTS_PER_SOL} SOL`);

        if (balance < tierPrice + 0.01 * LAMPORTS_PER_SOL) {
            throw new Error(`Insufficient balance. Need at least ${(tierPrice + 0.01 * LAMPORTS_PER_SOL) / LAMPORTS_PER_SOL} SOL`);
        }

        // Load MoonBase program
        const moonbaseIdlPath = path.join(__dirname, config.deployment.paths.moonbase_idl);
        const moonbaseIdl = JSON.parse(fs.readFileSync(moonbaseIdlPath, 'utf8'));
        const moonbaseProgram = new Program(moonbaseIdl, provider);

        // Get PDAs from deployment file
        const globalConfigPDA = new PublicKey(deploymentData.moonbase_program_initialized.globalConfig_address);
        const dogeBtcMiningPDA = new PublicKey(deploymentData.moonbase_program_initialized.dogeBtcMining_address);
        const solTreasuryPDA = new PublicKey(deploymentData.moonbase_program_initialized.solTreasury_address);

        // Derive user-specific PDAs
        const [userMoonbasePDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("user-moonbase"), userKeypair.publicKey.toBuffer()],
            moonbaseProgram.programId
        );

        const [newUserRewardsPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from("referral-rewards"), userKeypair.publicKey.toBuffer()],
            moonbaseProgram.programId
        );

        // Get global config to find creation fee recipient
        const globalConfig = await moonbaseProgram.account.globalConfig.fetch(globalConfigPDA);

        // Prepare optional accounts for Dragon Egg minting
        let dragonEggAsset = null;
        let dragonEggCollection = null;
        let dragonEggMetadata = null;
        let mplCoreProgram = null;
        let collectionAuthorityPDA = null;

        if (includesDragonEgg) {
            // Get collection address from deployment data
            if (!deploymentData.dragon_egg_collection_created) {
                throw new Error('Dragon Egg collection not created. Run 3_create_dragon_egg_collection.js first');
            }

            dragonEggCollection = new PublicKey(deploymentData.dragon_egg_collection_created.collection_address);
            
            // For MPL Core assets, we need to provide a keypair that will become the asset address
            // The program will create the NFT at this address
            const dragonEggAssetKeypair = Keypair.generate();
            dragonEggAsset = dragonEggAssetKeypair.publicKey;

            // Derive Dragon Egg Metadata PDA
            const [dragonEggMetadataPDA] = PublicKey.findProgramAddressSync(
                [
                    Buffer.from(DRAGON_EGG_METADATA_SEED),
                    userKeypair.publicKey.toBuffer()
                ],
                moonbaseProgram.programId
            );
            dragonEggMetadata = dragonEggMetadataPDA;

            mplCoreProgram = MPL_CORE_PROGRAM_ID;
            
            // Also derive the collection authority PDA
            [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
                [Buffer.from("collection_authority")],
                moonbaseProgram.programId
            );

            console.log(COLOR_INFO, `🥚 Dragon Egg Asset: ${dragonEggAsset.toString()}`);
            console.log(COLOR_INFO, `📋 Dragon Egg Metadata PDA: ${dragonEggMetadata.toString()}`);
            console.log(COLOR_INFO, `📚 Dragon Egg Collection: ${dragonEggCollection.toString()}`);
            console.log(COLOR_INFO, `🔐 Collection Authority PDA: ${collectionAuthorityPDA.toString()}`);
        }

        // Prepare referrer rewards account if referrer provided
        let referrerRewardsPDA = null;
        if (referrerAddress) {
            const referrerPubkey = new PublicKey(referrerAddress);
            [referrerRewardsPDA] = PublicKey.findProgramAddressSync(
                [Buffer.from("referral_rewards"), referrerPubkey.toBuffer()],
                moonbaseProgram.programId
            );
        }

        // Select faction (using first available faction)
        const factionId = 0; // You can make this configurable

        console.log(COLOR_STEP, '\n🚀 Creating User MoonBase...');

        // Build the transaction
        const tx = await moonbaseProgram.methods
            .createUserMoonbase(
                referrerAddress ? new PublicKey(referrerAddress) : null,
                factionId,
                new BN(tierPrice)
            )
            .accounts({
                userMoonbase: userMoonbasePDA,
                newUserRewards: newUserRewardsPDA,
                referrerRewards: referrerRewardsPDA,
                globalConfig: globalConfigPDA,
                dogeBtcMining: dogeBtcMiningPDA,
                solTreasury: solTreasuryPDA,
                creationFeeRecipient: globalConfig.creationFeeRecipient,
                dragonEggAsset: includesDragonEgg ? dragonEggAsset : null,
                dragonEggCollection: includesDragonEgg ? dragonEggCollection : null,
                dragonEggMetadata: includesDragonEgg ? dragonEggMetadata : null,
                mplCoreProgram: includesDragonEgg ? mplCoreProgram : null,
                collectionAuthority: includesDragonEgg ? collectionAuthorityPDA : null,
                user: userKeypair.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        console.log(COLOR_SUCCESS, '✅ User MoonBase created successfully!');
        console.log(COLOR_INFO, `📍 Transaction: ${tx}`);
        console.log(COLOR_DIM, `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`);

        // Fetch and display the created moonbase
        const userMoonbase = await moonbaseProgram.account.userMoonBaseInstance.fetch(userMoonbasePDA);
        console.log(COLOR_STEP, '\n📊 User MoonBase Details:');
        console.log(COLOR_INFO, `👤 Owner: ${userMoonbase.owner.toString()}`);
        console.log(COLOR_INFO, `🏛️ Faction ID: ${userMoonbase.factionId}`);
        console.log(COLOR_INFO, `🎯 Init Type: ${userMoonbase.initType}`);
        console.log(COLOR_INFO, `⚡ Available Electricity: ${userMoonbase.availableElectricity}`);
        console.log(COLOR_INFO, `🎮 Level: ${userMoonbase.level}`);
        console.log(COLOR_INFO, `✨ XP: ${userMoonbase.xp}`);

        if (includesDragonEgg && dragonEggMetadata) {
            try {
                const eggMetadata = await moonbaseProgram.account.dragonEggMetadata.fetch(dragonEggMetadata);
                console.log(COLOR_STEP, '\n🥚 Dragon Egg Details:');
                console.log(COLOR_INFO, `🪙 Mint: ${eggMetadata.mint.toString()}`);
                console.log(COLOR_INFO, `💪 Power: ${eggMetadata.power}`);
                console.log(COLOR_INFO, `🧬 DNA: ${Buffer.from(eggMetadata.dna).toString('hex')}`);
                console.log(COLOR_INFO, `📅 Created: ${new Date(eggMetadata.createdAt * 1000).toLocaleString()}`);
            } catch (error) {
                console.log(COLOR_WARNING, '⚠️ Could not fetch Dragon Egg metadata:', error.message);
            }
        }

        // Save user moonbase info
        const userDataPath = path.join(__dirname, 'deployments', `${CLUSTER}_users.json`);
        let userData = {};
        if (fs.existsSync(userDataPath)) {
            userData = JSON.parse(fs.readFileSync(userDataPath, 'utf8'));
        }

        userData[userKeypair.publicKey.toString()] = {
            moonbase_pda: userMoonbasePDA.toString(),
            rewards_pda: newUserRewardsPDA.toString(),
            faction_id: factionId,
            init_type: userMoonbase.initType,
            pricing_tier: pricingTier,
            referrer: referrerAddress,
            dragon_egg_asset: includesDragonEgg ? dragonEggAsset.toString() : null,
            dragon_egg_metadata: includesDragonEgg ? dragonEggMetadata.toString() : null,
            created_at: new Date().toISOString(),
            tx_signature: tx
        };

        fs.writeFileSync(userDataPath, JSON.stringify(userData, null, 2));
        console.log(COLOR_SUCCESS, '\n✅ User data saved to deployment file');

    } catch (error) {
        console.error(COLOR_ERROR, '\n❌ Failed to create user moonbase:', error);
        if (error.logs) {
            console.error(COLOR_ERROR, '📋 Program logs:');
            error.logs.forEach(log => console.error(COLOR_DIM, `   ${log}`));
        }
        process.exit(1);
    }
}

main();
