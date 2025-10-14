// Import Anchor as CommonJS package
import pkg from '@coral-xyz/anchor';
const { AnchorProvider, BN, Program, setProvider, web3 } = pkg;
import { SystemProgram } from '@solana/web3.js';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as anchor_spl from '@solana/spl-token';
import fs from 'fs';
import path from 'path';
import {  BorshAccountsCoder } from '@coral-xyz/anchor';
import { getSolanaBalance, createUserMoonbase, stakeMDOGE, unStakeMDOGE,  stakeLP, 
    addNewModuleToConfigStore, addNewGearToConfigStore, unStakeLP,sendSPLToken,
    DOGE_BTC_VAULT_SEED,  DOGE_BTC_VAULT_AUTHORITY_SEED,  
    MODULE_CONFIG_STORE_SEED,  sendSPL22Token,
    USER_MOONBASE_SEED, REFERRAL_REWARDS_SEED, MODULE_INSTANCE_SEED,
 } from './helper.js';

// Get the current file's directory
const __dirname = new URL('.', import.meta.url).pathname;

const CLUSTER = "localnet"; // "devnet";
const RPC_URL = "http://127.0.0.1:8899"; // "https://api.devnet.solana.com";

// 6. Load deployment data
const deploymentDir = path.resolve(__dirname, './deployments');
const deploymentFile = JSON.parse(fs.readFileSync( path.resolve(deploymentDir, `${CLUSTER}.json`), 'utf-8'));
const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);
// console.log('\x1b[90m%s\x1b[0m', "📋 Current deployment state:", JSON.stringify(deploymentFile, null, 2));


// 7. Load config data
const configDir = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configDir, 'utf-8'));


const MOONDOGE_TOKEN_MINT = new PublicKey(deploymentFile.dbtc_dbtc_mint_account_created.dbtc_mintAddress);
const dbtc_NFT_ADDRESS = "8ieqNFzgctWbKjkbTjnTrTmkwjFNVCFkqowbBDDpHE1b";

const ID_MOONBASE_PROGRAM = new PublicKey(deploymentFile.MOON_BASE_PROGRAM_ID);
const ID_MOONECONOMY_PROGRAM = new PublicKey(deploymentFile.MOON_ECONOMY_PROGRAM_ID);

  
// -------------------------------------------------------------------
// ==================== [ READ ::: IDL | WALLET | DEPLOYMENT ] ====================
// -------------------------------------------------------------------

// 1. MoonBase Program ID and IDL
const IDL_MOONBASE = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, '../prod_moonbase/target/idl/moon_base.json'), 'utf-8')
);
const IDL_MOON_ECONOMY = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, '../prod_moonbase/target/idl/moon_economy.json'), 'utf-8')
);

const moonEconomyCoder = new BorshAccountsCoder(IDL_MOON_ECONOMY);

 
// 2. Solana Connection
const connection = new Connection(RPC_URL, "confirmed");

// 3. Load wallet keypair
const walletKeypair = (() => {
  try {
    return Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(path.resolve(__dirname, '../wallet-keypair.json'), 'utf-8')))
    );
  } catch (e) {
    console.error('\x1b[31m%s\x1b[0m', "❌ Failed to load wallet keypair:", e);
    throw e;
  }
})();

// 4. Create a wallet interface with sign methods
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

// 5. Create provider
const provider = new AnchorProvider( connection,   wallet, { commitment: 'confirmed' });
setProvider(provider);


// ----------------------------------------------------------- 
// ==================== [ ::: SCRIPT ::: ] ====================
// ----------------------------------------------------------- 

async function main() {
 
    console.log('\x1b[35m%s\x1b[0m', '================================ [ WALLET ] ================================');
    console.log('\x1b[36m%s\x1b[0m', '👤 Admin Wallet:', walletKeypair.publicKey.toString());
    const balance = await getSolanaBalance(connection, walletKeypair.publicKey);
    console.log('\x1b[36m%s\x1b[0m', '💰 Balance:', balance / 1e9, 'SOL');
  
    console.log('\x1b[35m%s\x1b[0m', '============================== [ PROGRAMS ] ===============================');
    console.log('\x1b[36m%s\x1b[0m', '🚀 MoonBase Program ID:', ID_MOONBASE_PROGRAM.toString());        
    console.log('\x1b[36m%s\x1b[0m', '🚀 MoonEconomy Program ID:', ID_MOONECONOMY_PROGRAM.toString());        
    const moonBaseProgram = new Program(IDL_MOONBASE, provider);
    const moonEconomyProgram = new Program(IDL_MOON_ECONOMY, provider);

    console.log('\x1b[32m%s\x1b[0m', '✅ Connected to program:', moonBaseProgram.programId.toString() + " \n");
    console.log('\x1b[32m%s\x1b[0m', '✅ Connected to program:', moonEconomyProgram.programId.toString() + " \n");
    
    let mDogeMint = deploymentFile.dbtc_dbtc_mint_account_created.mintAddress;
    let lpMint = deploymentFile.testLP_mint_account_created.mintAddress;
    
    let globalConfigPDA = deploymentFile.moonEconomy_program_initialized.globalConfig_data_ac;
    let moondogeVaultPDA = deploymentFile.moonEconomy_mDogeVault_initialized.moondogeVault;
    let mdogeCustodianPDA = deploymentFile.moonEconomy_mDogeVault_initialized.mdogeCustodian;
    let mdogeCustodianAuthorityPDA = deploymentFile.moonEconomy_mDogeVault_initialized.mdogeCustodianAuthority;
    let moonbaseGlobalConfigPDA = deploymentFile.moonbase_program_initialized.globalConfig_data_ac;

    let liquidityVaultPDA = deploymentFile.moonEconomy_liquidityVault_initialized.liquidityVault;
    let liquidityCustodianPDA = deploymentFile.moonEconomy_liquidityVault_initialized.liquidityCustodian;
    let liquidityCustodianAuthorityPDA = deploymentFile.moonEconomy_liquidityVault_initialized.liquidityCustodianAuthority;

    let facilityMiningStatePDA = deploymentFile.moonbase_program_initialized.mdogeMining_data_ac;
    let feeCollectorPDA = deploymentFile.moonEconomy_program_initialized.feeCollector_data_ac;
    let moonFacilityProgramPDA = deploymentFile.MOON_BASE_PROGRAM_ID;

    let token22ProgramPDA = anchor_spl.TOKEN_2022_PROGRAM_ID;
    let tokenProgramPDA = anchor_spl.TOKEN_PROGRAM_ID;


    // ------------ 1. Create User Moonbase ------------
 
    //     const result = await createUserMoonbase(
    //         connection,
    //         moonBaseProgram,
    //         wallet,
    //         walletKeypair,
    //         deploymentFile.moonbase_program_initialized.globalConfig_data_ac,
    //         deploymentFile.moonbase_program_initialized.SOL_treasury_vault_ac,
    //         deploymentFile.create_system_referral_account.referral_rewards_ac
    //     );
    //     return;

    const USER_MOONBASE_PDA = "AjaptRSv3yYRji1TD1vEC6Ex1tff91UFSmXjfAXHJdCF";
 


  // // TRANSFER DOGE_BTC TO USER
  // await sendSPL22Token({
  //   connection,
  //   payer: walletKeypair,
  //   senderTokenAccount: deploymentFile.dbtc_token_account_created.tokenAccountAddress,
  //   recipientWallet: "ZMgahU99BRMx9mjUngYyZSSZDREGizKaN4qT3KMJyeY",
  //   mintAddress: deploymentFile.dbtc_dbtc_mint_account_created.dbtc_mintAddress,
  //   amount: 1000 * 1e9,
  // });

  
  // TRANSFER DOGE_BTC TO USER
  await sendSPLToken({
    connection,
    payer: walletKeypair,
    senderTokenAccount: deploymentFile.testLP_token_account_created.tokenAccountAddress,
    recipientWallet: "ZMgahU99BRMx9mjUngYyZSSZDREGizKaN4qT3KMJyeY",
    mintAddress: deploymentFile.testLP_mint_account_created.testLP_mintAddress,
    amount: 1000 * 1e9,
  });

  return;



    // ------------ 2. Stake DOGE_BTC tokens ------------

    let deposit_amount = new BN("10000000");
    let lockup_duration = 10;
    let index = 0;
    
    const stake_dbtc_result = await stakeMDOGE(  connection,  moonEconomyProgram,  wallet,  walletKeypair,  deposit_amount,  lockup_duration,  index,
        mDogeMint,  globalConfigPDA,  moondogeVaultPDA,  mdogeCustodianPDA,  moonbaseGlobalConfigPDA,  USER_MOONBASE_PDA,
        facilityMiningStatePDA,  feeCollectorPDA,  moonFacilityProgramPDA,  token22ProgramPDA)
    return;

    // // ------------ 3. Get user's electricity account info ------------
    // let userElectricityAccountPDA = "H8vLoQSoSK2fgnNRXhNEQD6qzjFkMfP2u1u1TNhW9ZMg";
    // const userElectricityAccountInfo = await connection.getAccountInfo( new PublicKey(userElectricityAccountPDA));    
    // const userMoonElectricity = moonEconomyCoder.decode('UserMoonElectricity', userElectricityAccountInfo.data);
    //     userMoonElectricity.owner = userMoonElectricity.owner.toBase58();
    //     userMoonElectricity.total_moondoge_staked = userMoonElectricity.total_moondoge_staked.toNumber();
    //     userMoonElectricity.total_weighted_moondoge = userMoonElectricity.total_weighted_moondoge.toNumber();

    //     userMoonElectricity.total_lp_tokens_staked = userMoonElectricity.total_lp_tokens_staked.toNumber();
    //     userMoonElectricity.total_weighted_lp = userMoonElectricity.total_weighted_lp.toNumber();

    //     userMoonElectricity.electricity_earned = userMoonElectricity.electricity_earned.toNumber();
    //     userMoonElectricity.moondoge_reward_debt = userMoonElectricity.moondoge_reward_debt.toNumber();
    //     userMoonElectricity.lp_reward_debt = userMoonElectricity.lp_reward_debt.toNumber();
    //     userMoonElectricity.pending_moondoge_rewards = userMoonElectricity.pending_moondoge_rewards.toNumber();
    //     userMoonElectricity.pending_lp_rewards = userMoonElectricity.pending_lp_rewards.toNumber();
    //     userMoonElectricity.total_sol_claimed = userMoonElectricity.total_sol_claimed.toNumber();
    //     // Decode Vec<u8> fields
    //     userMoonElectricity.moondoge_position_indices = Array.from(userMoonElectricity.moondoge_position_indices);
    //     userMoonElectricity.lp_position_indices = Array.from(userMoonElectricity.lp_position_indices);

    //     console.log(userMoonElectricity);
        
    // // ------------ 3. Get user's Individual staked DOGE_BTC position info ------------
    // let userMoonDogePositionPDA = "6nqEcbDjjYDssCHFqR5eSCLqeXiRJ8RgvLEiSZr1hw3G";
    // const userMoonDogePositionInfo = await connection.getAccountInfo( new PublicKey(userMoonDogePositionPDA));
    // const userMoonDogePosition = moonEconomyCoder.decode('MoonDogePosition', userMoonDogePositionInfo.data);
    //     userMoonDogePosition.staked_amount = userMoonDogePosition.staked_amount.toNumber();
    //     userMoonDogePosition.weighted_amount = userMoonDogePosition.weighted_amount.toNumber();
    //     userMoonDogePosition.start_timestamp = userMoonDogePosition.start_timestamp.toNumber();
    //     userMoonDogePosition.lockup_end_timestamp = userMoonDogePosition.lockup_end_timestamp.toNumber();
    //     userMoonDogePosition.lockup_duration = userMoonDogePosition.lockup_duration.toNumber();
    //     userMoonDogePosition.electricity_per_day = userMoonDogePosition.electricity_per_day.toNumber();
    //     console.log(userMoonDogePosition);
    // // return;

    // // // ------------ 4. Unstake DOGE_BTC tokens ------------
        
    // // const _tokenAccount = await anchor_spl.getAccount( connection,   new PublicKey(mdogeCustodianPDA),   undefined,  token22ProgramPDA);
    // // console.log(`available DOGE_BTC: ${_tokenAccount.amount}`);
    // // return

    // let unstake_dbtc_index = 0;
    // let unStakedbtc_result = await unStakeMDOGE(  connection,  moonEconomyProgram,  wallet,  walletKeypair,
    //     unstake_dbtc_index,  mDogeMint,  globalConfigPDA,  moondogeVaultPDA,  mdogeCustodianPDA,  mdogeCustodianAuthorityPDA,
    //                                                 moonbaseGlobalConfigPDA,  USER_MOONBASE_PDA,  facilityMiningStatePDA,
    //                                                 feeCollectorPDA,  moonFacilityProgramPDA,  token22ProgramPDA ) 

     // ------------ 1. Stake LP tokens ------------

    // let deposit_amount = new BN("10000000");
    // let lockup_duration = 10;
    // let index = 0;
    
    // const stake_lp_result = await stakeLP(  connection,  moonEconomyProgram,  wallet,  walletKeypair,  deposit_amount,  lockup_duration,  index,
    //     lpMint,  globalConfigPDA,  liquidityVaultPDA,  liquidityCustodianPDA,  moonbaseGlobalConfigPDA,  USER_MOONBASE_PDA,
    //     facilityMiningStatePDA,  feeCollectorPDA,  moonFacilityProgramPDA,  tokenProgramPDA)
    // return;

    // ------------ 2. Unstake LP tokens ------------

    let unstake_lp_index = 0;
    let unStakeLP_result = await unStakeLP(  connection,  moonEconomyProgram,  wallet,  walletKeypair,
        unstake_lp_index,  lpMint,  globalConfigPDA,  liquidityVaultPDA,  liquidityCustodianPDA, liquidityCustodianAuthorityPDA, 
        moonbaseGlobalConfigPDA,
        USER_MOONBASE_PDA,  facilityMiningStatePDA,  feeCollectorPDA,  moonFacilityProgramPDA,  tokenProgramPDA ) 


 



}

main();