import {
    Connection,
    PublicKey,
} from "@solana/web3.js";
import {
    TOKEN_2022_PROGRAM_ID,
    getMint,
    getTransferFeeConfig,
    unpackMint,
} from "@solana/spl-token";
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

// Load deployment data
const deploymentPath = path.resolve(__dirname, `./deployments/${CLUSTER}.json`);
const deploymentData = JSON.parse(fs.readFileSync(deploymentPath, 'utf-8'));

// ============================================================================
// ========== TOKEN STATISTICS TRACKER ======================================
// ============================================================================

async function getTokenStats() {
    console.log('\x1b[35m%s\x1b[0m', '📊 ================================ DOGE_BTC Token Statistics ================================');
    
    // Setup connection
    const connection = new Connection(RPC_URL, COMMITMENT);
    const mintAddress = new PublicKey(deploymentData.dbtc_mint_address);
    
    console.log('\x1b[36m%s\x1b[0m', '🔗 Network:', CLUSTER);
    console.log('\x1b[36m%s\x1b[0m', '🪙 Mint Address:', mintAddress.toBase58());
    console.log('\x1b[36m%s\x1b[0m', '⏰ Timestamp:', new Date().toISOString());
    
    try {
        // Get mint account info
        const mintInfo = await getMint(connection, mintAddress, COMMITMENT, TOKEN_2022_PROGRAM_ID);
        
        console.log(`mintInfo`, mintInfo);

        console.log('\x1b[35m%s\x1b[0m', '\n📈 Current Token Statistics:');
        
        // Basic token info
        const totalSupply = Number(mintInfo.supply) / Math.pow(10, mintInfo.decimals);
        const initialSupply = config.token.initial_supply;
        const totalBurned = initialSupply - totalSupply;
        const burnPercentage = ((totalBurned / initialSupply) * 100);
        
        console.log('\x1b[36m%s\x1b[0m', `   • Total Supply: ${totalSupply.toLocaleString()} ${config.token.symbol}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Initial Supply: ${initialSupply.toLocaleString()} ${config.token.symbol}`);
        console.log('\x1b[31m%s\x1b[0m', `   • Total Burned: ${totalBurned.toLocaleString()} ${config.token.symbol}`);
        console.log('\x1b[31m%s\x1b[0m', `   • Burn Percentage: ${burnPercentage.toFixed(6)}%`);
        console.log('\x1b[36m%s\x1b[0m', `   • Decimals: ${mintInfo.decimals}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Mint Authority: ${mintInfo.mintAuthority ? mintInfo.mintAuthority.toBase58() : '🔒 REMOVED (Non-mintable)'}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Freeze Authority: ${mintInfo.freezeAuthority ? mintInfo.freezeAuthority.toBase58() : 'None'}`);
        
        // Get transfer fee info
        let transferFeeConfig = null;
        try {
            transferFeeConfig = getTransferFeeConfig(mintInfo);
            if (transferFeeConfig) {
                const burnRate = transferFeeConfig.newerTransferFee.transferFeeBasisPoints;
                const maxBurnAmount = Number(transferFeeConfig.newerTransferFee.maximumFee) / Math.pow(10, mintInfo.decimals);
                const withheldAmount = Number(transferFeeConfig.withheldAmount) / Math.pow(10, mintInfo.decimals);
                
                console.log('\x1b[35m%s\x1b[0m', '\n🔥 Burn Mechanism Status:');
                console.log('\x1b[36m%s\x1b[0m', `   • Burn Rate: ${burnRate / 100}% per transfer`);
                console.log('\x1b[36m%s\x1b[0m', `   • Max Burn Per Transfer: ${maxBurnAmount.toLocaleString()} ${config.token.symbol}`);
                console.log('\x1b[36m%s\x1b[0m', `   • Currently Withheld: ${withheldAmount.toLocaleString()} ${config.token.symbol}`);
                console.log('\x1b[36m%s\x1b[0m', `   • Transfer Fee Authority: ${transferFeeConfig.transferFeeConfigAuthority ? transferFeeConfig.transferFeeConfigAuthority.toBase58() : 'None'}`);
                console.log('\x1b[36m%s\x1b[0m', `   • Withdraw Authority: ${transferFeeConfig.withdrawWithheldAuthority ? transferFeeConfig.withdrawWithheldAuthority.toBase58() : 'None'}`);
            }
        } catch (error) {
            console.log('\x1b[33m%s\x1b[0m', '⚠️ Could not fetch transfer fee config:', error.message);
        }
        
        // Calculate deflationary metrics
        console.log('\x1b[35m%s\x1b[0m', '\n📉 Deflationary Metrics:');
        if (totalBurned > 0) {
            console.log('\x1b[32m%s\x1b[0m', `   • Deflation Active: YES`);
            console.log('\x1b[32m%s\x1b[0m', `   • Tokens Removed: ${totalBurned.toLocaleString()} ${config.token.symbol}`);
            console.log('\x1b[32m%s\x1b[0m', `   • Supply Reduction: ${burnPercentage.toFixed(6)}%`);
        } else {
            console.log('\x1b[36m%s\x1b[0m', `   • Deflation Active: Not yet (no transfers with burns)`);
        }
        
        // Market cap calculation (if you have price data)
        console.log('\x1b[35m%s\x1b[0m', '\n💰 Supply Economics:');
        console.log('\x1b[36m%s\x1b[0m', `   • Circulating Supply: ${totalSupply.toLocaleString()} ${config.token.symbol}`);
        console.log('\x1b[36m%s\x1b[0m', `   • Max Supply: ${initialSupply.toLocaleString()} ${config.token.symbol} (Fixed)`);
        console.log('\x1b[36m%s\x1b[0m', `   • Supply Type: Deflationary (burns reduce supply)`);
        console.log('\x1b[36m%s\x1b[0m', `   • Mintable: ${mintInfo.mintAuthority ? 'YES' : '🔒 NO (Permanently disabled)'}`);
        
        // // Save stats to file
        // const statsData = {
        //     timestamp: new Date().toISOString(),
        //     network: CLUSTER,
        //     mint_address: mintAddress.toBase58(),
        //     total_supply: totalSupply,
        //     initial_supply: initialSupply,
        //     total_burned: totalBurned,
        //     burn_percentage: burnPercentage,
        //     deflation_active: totalBurned > 0,
        //     mint_authority_removed: !mintInfo.mintAuthority,
        //     circulating_supply: totalSupply,
        //     transfer_fee_config: transferFeeConfig ? {
        //         burn_rate_bps: transferFeeConfig.newerTransferFee.transferFeeBasisPoints,
        //         burn_rate_percent: transferFeeConfig.newerTransferFee.transferFeeBasisPoints / 100,
        //         max_burn_per_transfer: Number(transferFeeConfig.newerTransferFee.maximumFee) / Math.pow(10, mintInfo.decimals),
        //         currently_withheld: Number(transferFeeConfig.withheldAmount) / Math.pow(10, mintInfo.decimals)
        //     } : null
        // };
        
        // const statsPath = path.resolve(__dirname, `./stats/token_stats_${CLUSTER}_${Date.now()}.json`);
        // const statsDir = path.dirname(statsPath);
        // if (!fs.existsSync(statsDir)) {
        //     fs.mkdirSync(statsDir, { recursive: true });
        // }
        // fs.writeFileSync(statsPath, JSON.stringify(statsData, null, 2));
        
        // console.log('\x1b[35m%s\x1b[0m', '\n💾 Stats saved to:', statsPath);
        // console.log('\x1b[35m%s\x1b[0m', '========================================================================================');
        
        // return statsData;
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Error fetching token statistics:', error);
        throw error;
    }
}

// Run if called directly
if (import.meta.url === `file://${process.argv[1]}`) {
    getTokenStats().catch(console.error);
}

export { getTokenStats }; 