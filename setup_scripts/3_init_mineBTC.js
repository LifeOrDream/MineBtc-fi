// Import Anchor as CommonJS package
import pkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, setProvider, web3, Wallet } = pkg;
import { SystemProgram } from "@solana/web3.js";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as anchor_spl from "@solana/spl-token";
import crypto from "crypto";
import fs from "fs";
import path from "path";

// Get the current file's directory
const __dirname = decodeURIComponent(new URL(".", import.meta.url).pathname);

// Load configuration
const configPath = path.resolve(__dirname, "./config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// Color constants for consistent logging
const COLOR_STEP = "\x1b[35m%s\x1b[0m";
const COLOR_INFO = "\x1b[36m%s\x1b[0m";
const COLOR_SUCCESS = "\x1b[32m%s\x1b[0m";
const COLOR_WARNING = "\x1b[33m%s\x1b[0m";
const COLOR_ERROR = "\x1b[31m%s\x1b[0m";
const COLOR_DIM = "\x1b[90m%s\x1b[0m";

// Load deployment data
const deploymentDir = path.resolve(__dirname, "./deployments");
const deploymentPath = path.resolve(deploymentDir, `${CLUSTER}.json`);

let deploymentFile = {};
if (fs.existsSync(deploymentPath)) {
  deploymentFile = JSON.parse(fs.readFileSync(deploymentPath, "utf-8"));
} else {
    if (!fs.existsSync(deploymentDir)) {
        fs.mkdirSync(deploymentDir, { recursive: true });
    }
  console.log(
    COLOR_WARNING,
    "⚠️ No deployment file found. Starting fresh deployment."
  );
}

// Get deployed addresses
const DOGEBTC_TOKEN_MINT = deploymentFile.dbtc_mint_address
  ? new PublicKey(deploymentFile.dbtc_mint_address)
  : null;

const ID_MineBTC_PROGRAM = deploymentFile.MINE_BTC_PROGRAM_ID
  ? new PublicKey(deploymentFile.MINE_BTC_PROGRAM_ID)
  : null;

// Mining configuration
const MINING_START_TIMESTAMP =
  config.mining.start_timestamp || Math.floor(Date.now() / 1000);
const MINING_DOGE_BTC_PER_SLOT = new BN(config.mining.doge_btc_per_round);
const DBTC_DEPOSIT_AMOUNT = new BN(config.mining.initial_deposit);

// Load MineBTC Program IDL
const IDL_MineBTC = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, config.deployment.paths.minebtc_idl),
    "utf-8"
  )
);

// Solana Connection
const connection = new Connection(RPC_URL, COMMITMENT);

// Load wallet keypair
const walletKeypair = (() => {
    try {
    const walletPath = path.resolve(
      __dirname,
      config.deployment.paths.deployer_key
    );
        return Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
        );
    } catch (e) {
        console.error(COLOR_ERROR, "❌ Failed to load wallet keypair:", e);
    console.error(
      COLOR_ERROR,
      `   Expected path: ${path.resolve(
        __dirname,
        config.deployment.paths.deployer_key || "undefined"
      )}`
    );
    throw e;
  }
})();

const gameKeypair = (() => {
  try {
    const gameKeypairPath = path.resolve(
      __dirname,
      config.deployment.paths.game_keypair || "../game_keypair.json"
    );
    return Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(gameKeypairPath, "utf-8")))
    );
  } catch (e) {
    console.error(COLOR_ERROR, "❌ Failed to load game wallet keypair:", e);
    console.error(
      COLOR_ERROR,
      `   Expected path: ${path.resolve(
        __dirname,
        config.deployment.paths.game_keypair || "undefined"
      )}`
    );
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
    return txs.map((tx) => {
      tx.partialSign(walletKeypair);
      return tx;
    });
  },
};

// Create provider
const provider = new AnchorProvider(connection, wallet, {
  commitment: COMMITMENT,
});
setProvider(provider);

// Helper function to save deployment data
function saveDeploymentData() {
  fs.writeFileSync(deploymentPath, JSON.stringify(deploymentFile, null, 2));
  console.log(COLOR_SUCCESS, "✅ Deployment file updated");
}

async function getSolanaBalance(pubkey) {
  try {
    return await connection.getBalance(pubkey);
  } catch (error) {
    console.error(
      COLOR_ERROR,
      `❌ Error getting SOL balance: ${error.message}`
    );
    throw error;
  }
}

function toByteArray32FromHex(hexString) {
  const sanitized = hexString.startsWith("0x")
    ? hexString.slice(2)
    : hexString;
  if (!/^[0-9a-fA-F]{64}$/.test(sanitized)) {
    throw new Error(
      `initial_question_hash_hex must be 32 bytes / 64 hex chars, got: ${hexString}`
    );
  }
  return Array.from(Buffer.from(sanitized, "hex"));
}

function deriveQuestionHashFromConfig() {
  if (config.epoch?.initial_question_hash_hex) {
    return toByteArray32FromHex(config.epoch.initial_question_hash_hex);
  }

  const questionSource =
    config.epoch?.initial_question ||
    config.epoch?.initial_index_name ||
    "Will country rankings move up or down this epoch?";
  return Array.from(
    crypto.createHash("sha256").update(questionSource).digest()
  );
}

function getInitialIndexConfig() {
  const initialIndexId = config.epoch?.initial_index_id ?? 0;
  const initialIndexName =
    config.epoch?.initial_index_name ?? "Geopolitical Risk Index";
  const initialScores = Array.isArray(config.epoch?.initial_scores)
    ? config.epoch.initial_scores
    : Array(config.factions.length).fill(0);

  const NUM_FACTIONS = 15; // Must match program's NUM_FACTIONS constant
  if (config.factions.length > NUM_FACTIONS) {
    throw new Error(
      `Expected at most ${NUM_FACTIONS} factions for epoch index initialization, found ${config.factions.length}`
    );
  }
  // Pad initialScores to NUM_FACTIONS (program expects fixed-size array)
  while (initialScores.length < NUM_FACTIONS) {
    initialScores.push(0);
  }
  if (initialScores.length !== NUM_FACTIONS) {
    throw new Error(
      `epoch.initial_scores must contain exactly ${NUM_FACTIONS} values, found ${initialScores.length}`
    );
  }

  return {
    indexId: initialIndexId,
    name: initialIndexName,
    initialScores: initialScores.map((score) => Number(score)),
    questionHash: deriveQuestionHashFromConfig(),
  };
}

// ==================== [ MAIN SCRIPT ] ====================

async function main() {
  console.log(
    COLOR_STEP,
    "🚀 ================================ DogeTech Faction Surge Initialization ================================"
  );
  console.log(
    COLOR_INFO,
    "👤 Admin Wallet:",
    walletKeypair.publicKey.toString()
  );
  console.log(COLOR_INFO, "🌐 Network:", CLUSTER);
  console.log(COLOR_INFO, "🔗 RPC URL:", RPC_URL);
    
    const balance = await getSolanaBalance(walletKeypair.publicKey);
  console.log(COLOR_INFO, "💰 Balance:", balance / 1e9, "SOL");

    // Verify prerequisites
  if (!DOGEBTC_TOKEN_MINT) {
    console.error(
      COLOR_ERROR,
      "❌ DOGE_BTC token mint address not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 1_init_mdoge_token.js first.");
        return;
    }

  if (!ID_MineBTC_PROGRAM) {
    console.error(
      COLOR_ERROR,
      "❌ MineBTC program ID not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 0_deploy_game.js first.");
        return;
    }

  console.log(
    COLOR_STEP,
    "============================== [ PROGRAMS ] ==============================="
  );
  console.log(
    COLOR_INFO,
    "🚀 MineBTC Program ID:",
    ID_MineBTC_PROGRAM.toString()
  );
  console.log(
    COLOR_INFO,
    "🪙 DOGE_BTC Token Mint:",
    DOGEBTC_TOKEN_MINT.toString()
  );

  const minebtcProgram = new Program(IDL_MineBTC, provider);
  console.log(
    COLOR_SUCCESS,
    "✅ Connected to program:",
    minebtcProgram.programId.toString()
  );

  // Verify program ID matches deployment
  const programIdFromIdl =
    IDL_MineBTC.metadata?.address || IDL_MineBTC.programId;
  if (programIdFromIdl && programIdFromIdl !== ID_MineBTC_PROGRAM.toString()) {
    console.log(
      COLOR_WARNING,
      `⚠️ IDL program ID (${programIdFromIdl}) differs from deployment (${ID_MineBTC_PROGRAM.toString()})`
    );
    console.log(
      COLOR_INFO,
      `   Using deployment program ID: ${ID_MineBTC_PROGRAM.toString()}`
    );
  }

  // Double-check the program ID is correct
  if (!minebtcProgram.programId.equals(ID_MineBTC_PROGRAM)) {
    console.error(COLOR_ERROR, `❌ Program ID mismatch!`);
    console.error(
      COLOR_ERROR,
      `   Program instance ID: ${minebtcProgram.programId.toString()}`
    );
    console.error(
      COLOR_ERROR,
      `   Deployment Program ID: ${ID_MineBTC_PROGRAM.toString()}`
    );
    throw new Error(
      "Program ID mismatch between program instance and deployment file"
    );
  }

  try {
    // 1. Initialize MineBTC Program
    // Instruction: initialize(fee_recipient: Pubkey)
    // Creates 6 PDAs in one tx:
    //   - GlobalConfig     [seeds: "global-config"]           — stores authority, fee config, factions
    //   - MineBtcMining    [seeds: "mine-btc-mining"]         — mining emission state
    //   - UnrefinedRewards [seeds: "unrefined-rewards"]       — unrefined MineBTC reward pool
    //   - SOL Treasury     [seeds: "sol-treasury"]            — 0-byte system PDA for protocol SOL
    //   - Doges Treasury   [seeds: "doges-treasury"]          — 0-byte system PDA for doge mint fees
    //   - Autominer Custody[seeds: "autominer-custody"]       — 0-byte system PDA for autominer SOL
    // Params: fee_recipient (Pubkey) — initial fee recipient address
    await initializeMinebtcProgram(minebtcProgram);

    // 2. Set Raydium Pool State
    // Instruction: set_raydium_pool_state(raydium_pool_state: Pubkey)
    // Stores the authorized Raydium CPMM pool address in GlobalConfig for price discovery.
    // Also init_if_needed two SOL vault PDAs:
    //   - SOL Rewards Vault  [seeds: "staker-sol-reward-vault"] — holds SOL for staker distribution
    //   - SOL Prize Pot Vault[seeds: "sol-prize-pot"]           — holds SOL for round prize pots
    // Accounts: globalConfig, solRewardsVault, solPrizePotVault, authority, systemProgram
    await setRaydiumPoolState(minebtcProgram);

    // 3. Add Factions (12 factions)
    // Instruction: add_faction(faction_name: String, faction_id: u8)
    // Creates a FactionState PDA per faction [seeds: "faction", faction_name.as_bytes()]
    // Each stores: bump, faction_id, staking indexes, bet/win totals, motherlode pot
    // Accounts: globalConfig, factionState, authority, systemProgram
    await addFactions(minebtcProgram);

    // 4. Initialize System Accounts (Referral + Buybacks)
    // Instruction: initialize_system_accounts() — no args
    // Creates 3 PDAs:
    //   - SystemReferralRewards [seeds: "referral-rewards", system_program_id] — reserved sentinel for no-referral players
    //   - BuybacksAccount       [seeds: "buybacks"]                            — buyback SOL tracker
    //   - BuybacksSolVault      [seeds: "buybacks-sol-vault"]                  — 0-byte PDA for buyback SOL
    // Accounts: globalConfig, systemReferralRewards, buybacksAccount, buybacksSolVault, authority, systemProgram
    await initializeSystemAccounts(minebtcProgram);

    // 1.6. Update Fees
    // Instruction: update_fees(
    //   new_protocol_fee_pct: Option<u8>,            — % of SOL bets taken as protocol fee
    //   new_buyback_pct: Option<u8>,                 — % of treasury SOL used for buybacks + POL
    //   new_stakers_pct: Option<u8>,                 — % of protocol fee redirected to staker rewards vault
    //   new_minebtc_stakers_pct: Option<u8>,         — % of mined MineBTC going to stakers
    //   new_minebtc_winners_pct: Option<u8>,         — % of mined MineBTC going to round winners
    //   new_minebtc_same_faction_pct: Option<u8>,    — % of mined MineBTC going to winning-country non-exact bettors
    //   new_minebtc_motherlode_pct: Option<u8>,      — % of mined MineBTC going to motherlode pot
    //   new_refining_fee: Option<u8>,                — % fee when withdrawing unrefined MineBTC rewards
    //   change_faction_fee: Option<u64>,             — SOL cost to change faction (lamports)
    //   snapshot_interval: Option<u64>,              — min seconds between price snapshots
    // )
    // Accounts: globalConfig, mineBtcMining, authority, systemProgram
    await updateFees(minebtcProgram, {
      // deducted in internal_bet, stakers_pct deducted from protocol fee and custodied with SOL rewards vault, remaining with SOL treasury
        newProtocolFeePct: 15, // 15,
        newBuybackPct: 80, // 80% (remaining 20% goes to devs)
        newStakersPct: 10, // 10 of 15% = 1.5%,

        // dogeBTC distribution config:
        newMinebtcStakersPct: 3, // 3% of dogeBTC rewards go to stakers
        newMinebtcWinnersPct: 50, // 50% of dogeBTC rewards go to winners
        newMinebtcSameFactionPct: 42, // 42% of dogeBTC rewards go to winning-country, non-winning-direction bettors
        newMinebtcMotherlodePct: 5, // 5% of dogeBTC rewards go to motherlode

        newRefiningFee: 10, // 10% of dogeBTC rewards go to refining

        // split 50:50 between sol_treasury and fee_recipient (as WSOL)
        changeFactionFee: 100000000, // 0.1 SOL

        snapshotInterval: 5 * 60, // 5 minutes between price snapshots
    });

    // 5. Initialize Mining System (Token Vault + Mining Parameters)
    // Instruction: initialize_mining(start_timestamp: u64, mine_btc_per_round: u64, pool_state: Pubkey)
    // Sets up the mining emission vault:
    //   - VaultAuthority [seeds: "minebtc-vault-authority"] — signer-only PDA
    //   - TokenVault     [seeds: "minebtc_vault", mine_btc_mining.key()] — Token-2022 vault for MineBTC
    // Stores start_timestamp, emission rate, and Raydium pool state in MineBtcMining
    // Accounts: globalConfig, mineBtcMining, vaultAuthority, tokenVault, tokenMint, tokenProgram(T22), authority, systemProgram, rent
    await initializeMiningSystem(minebtcProgram);

    // 6. Deposit Mining Tokens
    // Instruction: deposit_mine_btc_tokens(amount: u64)
    // Transfers MineBTC from depositor's Token-2022 ATA to the mining vault
    // Accounts: depositor, depositorTokenAccount, minebtcTokenVault, mineBtcMining, tokenMint, tokenProgram(T22)
    await depositMiningTokens(minebtcProgram);

    // 7. Initialize Hashpower Config
    // Instruction: initialize_hashpower_config(min_lockup_days: u64, max_lockup_days: u64, base_multiplier: u16, max_multiplier: u16)
    // Creates HashpowerConfig PDA [seeds: "hashpower-config"] with lockup duration and multiplier ranges
    // Accounts: globalConfig, hashpowerConfig, authority, systemProgram
    await initializeHashpowerConfig(minebtcProgram);

    // 8. Initialize Custodian Accounts (DBTC and Liquidity custodians)
    // Instruction: initialize_custodian_accounts() — no args
    // Creates 4 PDAs:
    //   - minebtcCustodian           [seeds: "minebtc-custodian"]           — Token-2022 account for staked MineBTC
    //   - minebtcCustodianAuthority  [seeds: "minebtc-custodian-authority"] — signer PDA for MineBTC custodian
    //   - liquidityCustodian         [seeds: "lp-custodian"]                — SPL Token account for staked LP tokens
    //   - liquidityCustodianAuthority[seeds: "lp-custodian-authority"]      — signer PDA for LP custodian
    // Accounts: globalConfig, minebtcMint, minebtcCustodian, minebtcCustodianAuthority,
    //           lpMint, liquidityCustodian, liquidityCustodianAuthority, authority,
    //           systemProgram, token2022Program, tokenProgram, rent
    await initializeCustodianAccounts(minebtcProgram);

    // 9. Initialize DogeConfig
    // Instruction: initialize_doge_config(base_price: u64, curve_a: u64, max_supply: u64)
    // Creates DogeConfig PDA [seeds: "doge-config"] with bonding curve pricing params
    // Accounts: dogesConfig, globalConfig, authority, systemProgram
    await initializeDogeConfig(minebtcProgram);

    // 10. Create Doge Collection (Metaplex Core)
    // Instruction: create_doge_collection(name: String, uri: String)
    // Creates a Metaplex Core NFT collection with PDA as update authority
    // CollectionAuthority PDA [seeds: "collection_authority"] becomes the update authority
    // Accounts: authority, globalConfig, dogesConfig, collection (signer keypair),
    //           collectionAuthority, mplCoreProgram, systemProgram
    await createDogeCollection(minebtcProgram);

    // 11. Initialize Doge Royalties
    // Instruction: init_doge_royalties(basis_points: u16, creators: Vec<CreatorInput>)
    // Sets royalty config on the Metaplex Core collection (e.g. 5% split between multisig + treasury)
    // Accounts: authority, globalConfig, dogesConfig, collection, collectionAuthority, mplCoreProgram, systemProgram
    await initializeDogeRoyalties(minebtcProgram);

    // 12. Configure Ticket Tiers (for Doge minting)
    // Instruction: add_ticket_tier_config(ticket_tier_index: u8, ticket_value: u64)
    // Adds/updates a ticket tier in DogeConfig (max 4 tiers)
    // Accounts: globalConfig, dogesConfig, authority, systemProgram
    await configureTicketTiers(minebtcProgram);

    // 13. Initialize Tax Config (for tax distribution)
    // Instruction: initialize_tax_config(nft_floor_sweep_pct: u8, faction_treasury_pct: u8, burn_pct: u8, nft_floor_sweep_whitelisted_address: Pubkey)
    // Creates TaxConfig PDA [seeds: "tax-config"] and associated vaults:
    //   - WithdrawWithheldAuthority [seeds: "withdraw-withheld-authority"] — 0-byte signer PDA
    //   - FactionTreasuryVault      [seeds: "faction-treasury-vault"]      — Token-2022 vault
    //   - NftFloorSweepVault        [seeds: "nft-floor-sweep-vault"]       — Token-2022 vault
    //   - NftSaleSolVault           [seeds: "nft-sale-sol-vault"]          — 0-byte system PDA for SOL
    // Accounts: globalConfig, taxConfig, minebtcMint, withdrawWithheldAuthority,
    //           factionTreasuryVault, nftFloorSweepVault, nftSaleSolVault, authority,
    //           tokenProgram2022, systemProgram
    await initializeTaxConfig(minebtcProgram);

    // 14. Initialize Game State (for Faction Surge rounds)
    // Instruction: initialize_game_state(round_duration_seconds: i64)
    // Creates GlobalGameState PDA [seeds: "global-game-state"] with round timing
    // Accounts: globalGameState, globalConfig, authority, systemProgram
    await initializeGameState(minebtcProgram);

    // 15. Initialize LP Token Accounts (for Raydium LP integration)
    // Off-chain helper: creates an ATA for LP tokens owned by vaultAuthority PDA
    // Uses @solana/spl-token getOrCreateAssociatedTokenAccount (no program instruction)
    await initializeLpTokenAccounts(minebtcProgram);



    // // 1.5. Update Fee Recipient (if needed - can be called anytime after initialization)
    // const feeRecipientFromConfig = "BH54VNvpq4b3V2PDzDhNAVmNTH4xbSx8dqo1uKz3qmVz";
    // // if (feeRecipientFromConfig) {
    //     await updateFeeRecipient(minebtcProgram, feeRecipientFromConfig);
    // // }

    // 1.5.1. Update Authority (if needed - can be called anytime after initialization)
    // const newAuthorityFromConfig = "2Xze8BhdWV3GoJUyzpQPF7d1N2KUCS1TCkdVECfkDTcd"; // Set to authority address string, or null to skip
    // // Example: const newAuthorityFromConfig = "YourMultisigAddressHere";
    // if (newAuthorityFromConfig) {
    //     await updateAuthority(minebtcProgram, newAuthorityFromConfig);
    // }
    
    // return;


    // // 1.7. Update Doge Config (if needed - can be called anytime after initialization)
    // // Example usage:
    // await updateDogeConfig(minebtcProgram, {
    //     basePrice: 100000000, // 1 SOL in lamports
    //     curveA: 1111111, // Curve parameter
    // });
    // return;
 






    // return;

    console.log(
      COLOR_STEP,
      "\n================ [ ADDING GAME CRANKER BOT ] ================"
    );
    console.log(
      COLOR_INFO,
      `🔑 Game Cranker Bot: ${gameKeypair.publicKey.toString()}`
    );

    // 16. Add Game Cranker Bot
    // Instruction: add_cranker_bot(bot_pubkey: Pubkey)
    // Whitelists a keeper bot wallet to call start_round / end_round
    // Accounts: globalGameState, globalConfig, authority, systemProgram
    await addGameCrankerBot(
      minebtcProgram,
      gameKeypair.publicKey.toBase58()
    );

    // 17. Initialize Epoch Config (for active-index, oracle-settled epoch markets)
    // Instruction: initialize_epoch_config(oracle_authority: Pubkey, epoch_duration: u64,
    //              risk_factor: u16, model5_pct: u8, top1_pct: u8, top2_pct: u8, top3_pct: u8)
    // Creates EpochConfig PDA [seeds: "epoch-config"] with oracle authority, active-index rotation,
    // and rank-weighted epoch reward splits
    // Accounts: epochConfig, globalConfig, authority, systemProgram
    await initializeEpochConfig(minebtcProgram);

    // 18. Initialize bootstrap index state for epoch markets
    // Without an on-chain IndexState for active_index_id=0, join_round / autominer flows
    // will fail because they always pass the active index account.
    await initializeBootstrapIndexState(minebtcProgram);

    // 19. Schedule the initial epoch market using the oracle wallet so the first epoch
    // has a non-empty active market/question hash before gameplay starts.
    await scheduleInitialEpochMarket(minebtcProgram);

    // Print completion summary
    // printCompletionSummary();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Initialization failed:", error);
        if (error.logs) {
      console.error(COLOR_ERROR, "📝 Transaction logs:");
      error.logs.forEach((log) => console.error(COLOR_DIM, log));
        }
        process.exit(1);
    }
}

// ==================== [ INITIALIZATION FUNCTIONS ] ====================

async function initializeMinebtcProgram(minebtcProgram) {
  if (deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ MineBTC program already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n====================== [ INITIALIZING MineBTC PROGRAM ] ===================="
  );

    // Derive PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-config")],
    minebtcProgram.programId
  );

  const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("mine-btc-mining")],
    minebtcProgram.programId
    );

    const [solTreasuryPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("sol-treasury")],
    minebtcProgram.programId
  );

  const [dogesTreasuryPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("doges-treasury")],
    minebtcProgram.programId
  );

  const [unrefinedRewardsPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("unrefined-rewards")],
    minebtcProgram.programId
  );

  const [autominerCustodyPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("autominer-custody")],
    minebtcProgram.programId
  );

  const FEE_RECIPIENT_MULTISIG = new PublicKey(
    config.deployment.FEE_RECIPIENT_MULTISIG
  );

  console.log(
    COLOR_INFO,
    `🔑 Global Config PDA: ${globalConfigPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 DogeBtc Mining PDA: ${mineBtcMiningPDA.toString()}`
  );
    console.log(COLOR_INFO, `🔑 SOL Treasury PDA: ${solTreasuryPDA.toString()}`);
  console.log(
    COLOR_INFO,
    `🔑 Fee Recipient: ${FEE_RECIPIENT_MULTISIG.toString()}`
  );

    try {
    const tx = await minebtcProgram.methods
            .initialize(FEE_RECIPIENT_MULTISIG)
            .accounts({
                globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
        unrefinedRewards: unrefinedRewardsPDA,
                solTreasury: solTreasuryPDA,
        dogesTreasury: dogesTreasuryPDA,
        autominerCustody: autominerCustodyPDA,        
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Program initialized successfully!");
        console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    deploymentFile.minebtc_program_initialized = {
            globalConfig_address: globalConfigPDA.toString(),
      mineBtcMining_address: mineBtcMiningPDA.toString(),
            solTreasury_address: solTreasuryPDA.toString(),
      dogesTreasury_address: dogesTreasuryPDA.toString(),
      autominerCustody_address: autominerCustodyPDA.toString(),
      unrefinedRewards_address: unrefinedRewardsPDA.toString(),
            FEE_RECIPIENT_MULTISIG: FEE_RECIPIENT_MULTISIG.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
      console.log(COLOR_INFO, "ℹ️ Program already initialized. Skipping...");
      deploymentFile.minebtc_program_initialized = {
                globalConfig_address: globalConfigPDA.toString(),
        mineBtcMining_address: mineBtcMiningPDA.toString(),
        unrefinedRewards: unrefinedRewardsPDA.toString(),
                solTreasury_address: solTreasuryPDA.toString(),
        dogesTreasury_address: dogesTreasuryPDA.toString(),
            };
            saveDeploymentData();
        } else {
            throw error;
        }
    }
}

async function initializeSystemAccounts(minebtcProgram) {
    if (deploymentFile.system_accounts_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ System accounts already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING SYSTEM ACCOUNTS ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    // Derive PDAs
    const [systemReferralRewardsPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("referral-rewards"), SystemProgram.programId.toBuffer()],
    minebtcProgram.programId
    );

    const [buybacksAccountPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("buybacks")],
    minebtcProgram.programId
    );

    const [buybacksSolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("buybacks-sol-vault")],
    minebtcProgram.programId
  );

  console.log(
    COLOR_INFO,
    `🔑 System Referral Rewards PDA: ${systemReferralRewardsPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 Buybacks Account PDA: ${buybacksAccountPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 Buybacks SOL Vault PDA: ${buybacksSolVaultPDA.toString()}`
  );

  try {
    const tx = await minebtcProgram.methods
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

    console.log(COLOR_SUCCESS, "✅ System accounts initialized!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.system_accounts_initialized = {
            system_referral_rewards_pda: systemReferralRewardsPDA.toString(),
            buybacks_account_pda: buybacksAccountPDA.toString(),
            buybacks_sol_vault_pda: buybacksSolVaultPDA.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
      console.log(
        COLOR_INFO,
        "ℹ️ System accounts already initialized. Skipping..."
      );
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

async function addFactions(minebtcProgram) {
    if (deploymentFile.factions_added) {
    console.log(COLOR_INFO, "ℹ️ Factions already added. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ ADDING FACTIONS ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
    const addedFactions = [];

  // First, fetch the current global config to get the current faction count
  let currentFactionCount = 0;
  try {
    const globalConfig = await minebtcProgram.account.globalConfig.fetch(
      globalConfigPDA
    );
    currentFactionCount = globalConfig.supportedFactions?.length || 0;
    console.log(
      COLOR_INFO,
      `📊 Current factions in GlobalConfig: ${currentFactionCount}`
    );
  } catch (error) {
    console.log(
      COLOR_WARNING,
      `⚠️ Could not fetch GlobalConfig, assuming 0 factions`
    );
    currentFactionCount = 0;
  }

    console.log(COLOR_INFO, `📝 Adding ${config.factions.length} factions...`);

    for (let i = 0; i < config.factions.length; i++) {
        const faction = config.factions[i];
        const factionId = i;
    console.log(COLOR_INFO, `Faction ID: ${factionId}`);
    console.log(COLOR_INFO, `Current faction count: ${currentFactionCount}`);

    // CRITICAL: The faction_id must match the current faction count in GlobalConfig
    // This matches the Rust validation: require!(faction_id == current_faction_count as u8, ErrorCode::InvalidFactionId)
    if (factionId !== currentFactionCount) {
      console.log(
        COLOR_WARNING,
        `   ⚠️ Skipping faction ${factionId} - expected faction ID ${currentFactionCount} (current count: ${currentFactionCount})`
      );
      console.log(
        COLOR_WARNING,
        `      Factions must be added sequentially starting from ${currentFactionCount}`
      );
      continue;
    }

    // Derive FactionState PDA
    // Rust seeds: [b"faction", faction_name.as_bytes()]
    const [factionStatePDA, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("faction"), Buffer.from(faction.name)],
      minebtcProgram.programId
    );
        console.log(`   ${i + 1}. ${faction.name} (ID: ${factionId})`);
    console.log(`      FactionState PDA: ${factionStatePDA.toString()}`);
    console.log(`      Bump: ${bump}`);

    // Check if faction state already exists and verify it matches
    let factionStateExists = false;
    let existingFactionId = null;
    let shouldSkip = false;

    try {
      const existingFactionState =
        await minebtcProgram.account.factionState.fetch(factionStatePDA);
      if (existingFactionState) {
        factionStateExists = true;
        // Handle BN conversion if needed
        existingFactionId =
          typeof existingFactionState.factionId === "object" &&
          existingFactionState.factionId?.toNumber
            ? existingFactionState.factionId.toNumber()
            : existingFactionState.factionId;

        console.log(
          COLOR_WARNING,
          `      ⚠️ FactionState already exists for faction ${factionId}`
        );
        console.log(
          COLOR_DIM,
          `         Existing faction ID in account: ${existingFactionId}`
        );

        // If the faction ID matches, we can skip adding it
        if (existingFactionId === factionId) {
          console.log(
            COLOR_INFO,
            `      ℹ️ Skipping - faction ${factionId} already initialized correctly`
          );
          addedFactions.push({
            faction_id: factionId,
            name: faction.name,
            faction_state_pda: factionStatePDA.toString(),
            status: "already_exists",
            existing_faction_id: existingFactionId,
          });
          shouldSkip = true;
        } else {
          console.log(
            COLOR_WARNING,
            `      ⚠️ Faction ID mismatch! Account has ${existingFactionId}, trying to set ${factionId}`
          );
          console.log(
            COLOR_WARNING,
            `         This may cause a ConstraintSeeds error. Account may need to be closed first.`
          );
        }
      }
    } catch (error) {
      // Account doesn't exist, which is fine - we'll create it
      factionStateExists = false;
    }

    if (shouldSkip) {
      continue;
    }

    try {
      // Verify the PDA derivation matches what Anchor expects
      // Anchor will derive: [b"faction", faction_id] with the program ID
      console.log(COLOR_DIM, `      Verifying PDA derivation...`);
      console.log(
        COLOR_DIM,
        `         Program ID: ${minebtcProgram.programId.toString()}`
      );
      console.log(
        COLOR_DIM,
        `         Seeds: ["faction" (7 bytes), [${factionId}] (1 byte)]`
      );
      console.log(
        COLOR_DIM,
        `         Expected PDA: ${factionStatePDA.toString()}`
      );
      console.log(COLOR_DIM, `         Bump: ${bump}`);

      const tx = await minebtcProgram.methods
                .addFaction(faction.name, factionId)
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
        tx_signature: tx,
            });

      // Increment the faction count for next iteration
      currentFactionCount++;
        } catch (error) {
      // Check for specific error types
      const errorStr = error.toString();
      console.log(errorStr);

      // Check if it's a ConstraintSeeds error - this means PDA mismatch
      if (errorStr.includes("ConstraintSeeds")) {
        console.error(
          COLOR_ERROR,
          `      ❌ ConstraintSeeds error for ${faction.name}`
        );
        console.error(
          COLOR_ERROR,
          `         This means the PDA derivation doesn't match what Anchor expects.`
        );

        // Try to extract the "Right" PDA from error logs if available
        if (error.logs) {
          const logs = error.logs.join("\n");
          const rightMatch = logs.match(/Right:\s*([A-Za-z0-9]{32,44})/);
          if (rightMatch) {
            const rightPDA = rightMatch[1];
            console.error(
              COLOR_ERROR,
              `         Anchor derived PDA: ${rightPDA}`
            );
            console.error(
              COLOR_ERROR,
              `         Mismatch detected! Check program ID and seeds.`
            );
          }
        }

        throw new Error(
          `ConstraintSeeds error: PDA derivation mismatch for faction ${factionId}. Check program ID and seeds.`
        );
      }

      if (
        errorStr.includes("already in use") ||
        errorStr.includes("MaxFactionsReached") ||
        errorStr.includes("already exists")
      ) {
        console.log(
          COLOR_WARNING,
          `      ⚠️ ${faction.name} may already exist`
        );
        console.log(COLOR_DIM, `         Error: ${errorStr.substring(0, 200)}`);

                addedFactions.push({
                    faction_id: factionId,
                    name: faction.name,
                    faction_state_pda: factionStatePDA.toString(),
          status: "error_or_exists",
          error: errorStr.substring(0, 200),
        });

        // Still increment count if it already exists
        currentFactionCount++;
      } else if (errorStr.includes("InvalidFactionId")) {
        console.error(
          COLOR_ERROR,
          `      ❌ InvalidFactionId error for ${faction.name}`
        );
        console.error(
          COLOR_ERROR,
          `         Expected faction ID: ${currentFactionCount}, but got: ${factionId}`
        );
        console.error(
          COLOR_ERROR,
          `         Factions must be added sequentially.`
        );
        throw error;
            } else {
        console.error(COLOR_ERROR, `      ❌ Failed to add ${faction.name}:`);
        console.error(COLOR_ERROR, `         ${errorStr}`);
                throw error;
            }
        }
    }

    console.log(COLOR_SUCCESS, `✅ ${addedFactions.length} factions configured!`);

    deploymentFile.factions_added = {
        factions: addedFactions,
    timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
}

async function initializeMiningSystem(minebtcProgram) {
    if (deploymentFile.mining_vault_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Mining system already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING MINING SYSTEM ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const mineBtcMiningPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.mineBtcMining_address
  );
    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created?.poolStatePDA;

    if (!raydiumPoolState) {
    console.error(
      COLOR_ERROR,
      "❌ Raydium pool state not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_mdoge_SOL_pool.js first.");
        return;
    }

    const [vaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("minebtc_vault"), mineBtcMiningPDA.toBuffer()],
    minebtcProgram.programId
    );

    const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("minebtc-vault-authority")],
    minebtcProgram.programId
    );

    console.log(COLOR_INFO, `🔑 Mining Token Vault PDA: ${vaultPDA.toString()}`);
  console.log(
    COLOR_INFO,
    `🔑 Vault Authority PDA: ${vaultAuthorityPDA.toString()}`
  );
    console.log(COLOR_INFO, `⏰ Start Timestamp: ${MINING_START_TIMESTAMP}`);
  console.log(
    COLOR_INFO,
    `💰 DogeBtc Per Slot: ${MINING_DOGE_BTC_PER_SLOT.toString()}`
  );
    console.log(COLOR_INFO, `🔄 Raydium Pool State: ${raydiumPoolState}`);

    try {
    const tx = await minebtcProgram.methods
            .initializeMining(
                new BN(MINING_START_TIMESTAMP),
                MINING_DOGE_BTC_PER_SLOT,
                new PublicKey(raydiumPoolState)
            )
            .accounts({
                globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
                vaultAuthority: vaultAuthorityPDA,
                tokenVault: vaultPDA,
        tokenMint: DOGEBTC_TOKEN_MINT,
                tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
                rent: web3.SYSVAR_RENT_PUBKEY,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Mining system initialized!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.mining_vault_initialized = {
            vault_address: vaultPDA.toString(),
            vault_authority: vaultAuthorityPDA.toString(),
            start_timestamp: MINING_START_TIMESTAMP,
            doge_btc_per_round: MINING_DOGE_BTC_PER_SLOT.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("MiningAlreadyInitialized")) {
      console.log(COLOR_INFO, "ℹ️ Mining already initialized. Skipping...");
            deploymentFile.mining_vault_initialized = {
                vault_address: vaultPDA.toString(),
                vault_authority: vaultAuthorityPDA.toString(),
                start_timestamp: MINING_START_TIMESTAMP,
                doge_btc_per_round: MINING_DOGE_BTC_PER_SLOT.toString(),
            };
            saveDeploymentData();
        } else {
            throw error;
        }
    }
}

async function depositMiningTokens(minebtcProgram) {
    if (deploymentFile.mining_tokens_deposited) {
    console.log(COLOR_INFO, "ℹ️ Mining tokens already deposited. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ DEPOSITING MINING TOKENS ] ================"
  );

  const mineBtcMiningPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.mineBtcMining_address
  );
  const vaultPDA = new PublicKey(
    deploymentFile.mining_vault_initialized.vault_address
  );

    // Get user's token account
    const userTokenAccount = await anchor_spl.getAssociatedTokenAddress(
    DOGEBTC_TOKEN_MINT,
        wallet.publicKey,
        false,
        anchor_spl.TOKEN_2022_PROGRAM_ID
    );

  console.log(
    COLOR_INFO,
    `💰 Depositing ${DBTC_DEPOSIT_AMOUNT.toString()} tokens...`
  );
    console.log(COLOR_INFO, `   From: ${userTokenAccount.toString()}`);
    console.log(COLOR_INFO, `   To: ${vaultPDA.toString()}`);

    try {
    const tx = await minebtcProgram.methods
      .depositMineBtcTokens(DBTC_DEPOSIT_AMOUNT)
            .accounts({
                depositor: wallet.publicKey,
                depositorTokenAccount: userTokenAccount,
                minebtcTokenVault: vaultPDA,
        mineBtcMining: mineBtcMiningPDA,
        tokenMint: DOGEBTC_TOKEN_MINT,
                tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Mining tokens deposited successfully!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.mining_tokens_deposited = {
            amount: DBTC_DEPOSIT_AMOUNT.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to deposit mining tokens:", error);
        throw error;
    }
}

async function initializeHashpowerConfig(minebtcProgram) {
  if (deploymentFile.hashpower_config_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Hashpower config already initialized. Skipping..."
    );
    return;
  }

  const minLockupDays = config.hashpower.min_lockup_days;
  const maxLockupDays = config.hashpower.max_lockup_days;
  const baseMultiplier = config.hashpower.base_multiplier;
  const maxMultiplier = config.hashpower.max_multiplier;

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING HASHPOWER CONFIG ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

  const [hashpowerConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashpower-config")],
    minebtcProgram.programId
  );

  console.log(
    COLOR_INFO,
    `🔑 Hashpower Config PDA: ${hashpowerConfigPDA.toString()}`
  );

  try {
    const tx = await minebtcProgram.methods
      .initializeHashpowerConfig(
        new BN(minLockupDays),
        new BN(maxLockupDays),
        new BN(baseMultiplier),
        new BN(maxMultiplier)
      )
      .accounts({
        globalConfig: globalConfigPDA,
        hashpowerConfig: hashpowerConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, "✅ Hashpower config initialized successfully!");
    console.log(COLOR_DIM, `   Transaction: ${tx}`);

    deploymentFile.hashpower_config_initialized = {
      hashpowerConfig_pda: hashpowerConfigPDA.toString(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    console.error(
      COLOR_ERROR,
      "❌ Failed to initialize hashpower config:",
      error
    );
    throw error;
  }
}

async function initializeCustodianAccounts(minebtcProgram) {
  if (deploymentFile.custodian_accounts_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Custodian accounts already initialized. Skipping..."
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING CUSTODIAN ACCOUNTS ] ==================="
  );

  // Verify prerequisites
  if (!DOGEBTC_TOKEN_MINT) {
    console.error(
      COLOR_ERROR,
      "❌ DOGE_BTC token mint address not found in deployment file."
    );
    throw new Error(
      "DOGE_BTC mint address required for custodian initialization"
    );
  }

  if (!deploymentFile.dbtc_sol_pool_created?.lpMintPDA) {
    console.error(
      COLOR_ERROR,
      "❌ LP mint address not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_mdoge_SOL_pool.js first.");
    throw new Error("LP mint address required for custodian initialization");
  }

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const minebtcMint = DOGEBTC_TOKEN_MINT;
  const lpMint = new PublicKey(deploymentFile.dbtc_sol_pool_created.lpMintPDA);

  // Derive DBTC custodian PDAs
  const [minebtcCustodianPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("minebtc-custodian")],
    minebtcProgram.programId
  );

  const [minebtcCustodianAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("minebtc-custodian-authority")],
    minebtcProgram.programId
  );

  // Derive Liquidity custodian PDAs
  const [liquidityCustodianPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("lp-custodian")],
    minebtcProgram.programId
  );

  const [liquidityCustodianAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("lp-custodian-authority")],
    minebtcProgram.programId
  );

  console.log(COLOR_INFO, `🔑 DBTC Mint: ${minebtcMint.toString()}`);
  console.log(
    COLOR_INFO,
    `🔑 DBTC Custodian PDA: ${minebtcCustodianPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 DBTC Custodian Authority PDA: ${minebtcCustodianAuthorityPDA.toString()}`
  );
  console.log(COLOR_INFO, `🔑 LP Mint: ${lpMint.toString()}`);
  console.log(
    COLOR_INFO,
    `🔑 Liquidity Custodian PDA: ${liquidityCustodianPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 Liquidity Custodian Authority PDA: ${liquidityCustodianAuthorityPDA.toString()}`
  );

  try {
    const tx = await minebtcProgram.methods
      .initializeCustodianAccounts()
      .accounts({
        globalConfig: globalConfigPDA,
        minebtcMint: minebtcMint,
        minebtcCustodian: minebtcCustodianPDA,
        minebtcCustodianAuthority: minebtcCustodianAuthorityPDA,
        lpMint: lpMint,
        liquidityCustodian: liquidityCustodianPDA,
        liquidityCustodianAuthority: liquidityCustodianAuthorityPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
        token2022Program: anchor_spl.TOKEN_2022_PROGRAM_ID,
        tokenProgram: anchor_spl.TOKEN_PROGRAM_ID,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    console.log(
      COLOR_SUCCESS,
      "✅ Custodian accounts initialized successfully!"
    );
    console.log(COLOR_SUCCESS, "   ✅ DBTC custodian (Token-2022) initialized");
    console.log(
      COLOR_SUCCESS,
      "   ✅ Liquidity custodian (SPL Token) initialized"
    );
    console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    deploymentFile.custodian_accounts_initialized = {
      dbtc_custodian: minebtcCustodianPDA.toString(),
      dbtc_custodian_authority: minebtcCustodianAuthorityPDA.toString(),
      liquidity_custodian: liquidityCustodianPDA.toString(),
      liquidity_custodian_authority: liquidityCustodianAuthorityPDA.toString(),
      dbtc_mint: minebtcMint.toString(),
      lp_mint: lpMint.toString(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    const errorStr = error.toString();
    if (
      errorStr.includes("already in use") ||
      errorStr.includes("already exists")
    ) {
      console.log(
        COLOR_WARNING,
        "⚠️ Custodian accounts may already be initialized. Checking..."
      );

      // Check if accounts exist
      try {
        const minebtcCustodianInfo = await connection.getAccountInfo(
          minebtcCustodianPDA
        );
        const liquidityCustodianInfo = await connection.getAccountInfo(
          liquidityCustodianPDA
        );

        if (minebtcCustodianInfo && liquidityCustodianInfo) {
          console.log(
            COLOR_INFO,
            "ℹ️ Custodian accounts already exist. Skipping..."
          );
          deploymentFile.custodian_accounts_initialized = {
            dbtc_custodian: minebtcCustodianPDA.toString(),
            dbtc_custodian_authority: minebtcCustodianAuthorityPDA.toString(),
            liquidity_custodian: liquidityCustodianPDA.toString(),
            liquidity_custodian_authority:
              liquidityCustodianAuthorityPDA.toString(),
            dbtc_mint: minebtcMint.toString(),
            lp_mint: lpMint.toString(),
            status: "already_exists",
          };
          saveDeploymentData();
          return;
        }
      } catch (checkError) {
        // Continue to throw original error
      }
    }
    console.error(
      COLOR_ERROR,
      "❌ Failed to initialize custodian accounts:",
      error
    );
    if (error.logs) {
      console.error(COLOR_ERROR, "📝 Transaction logs:");
      error.logs.forEach((log) => console.error(COLOR_DIM, log));
    }
    throw error;
  }
}

async function setRaydiumPoolState(minebtcProgram) {
    if (deploymentFile.raydium_pool_state_set) {
    console.log(COLOR_INFO, "ℹ️ Raydium pool state already set. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ SETTING RAYDIUM POOL STATE ] ==================="
  );

    const raydiumPoolState = deploymentFile.dbtc_sol_pool_created?.poolStatePDA;

    if (!raydiumPoolState) {
    console.error(
      COLOR_ERROR,
      "❌ Raydium pool state not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_mdoge_SOL_pool.js first.");
        return;
    }

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
    const poolStatePubkey = new PublicKey(raydiumPoolState);

  // Derive vault PDAs
  const [solRewardsVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("staker-sol-reward-vault")],
    minebtcProgram.programId
  );

  const [solPrizePotVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("sol-prize-pot")],
    minebtcProgram.programId
  );

  console.log(
    COLOR_INFO,
    `🔑 Pool State Address: ${poolStatePubkey.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 SOL Rewards Vault: ${solRewardsVaultPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 SOL Prize Pot Vault: ${solPrizePotVaultPDA.toString()}`
  );

  try {
    const tx = await minebtcProgram.methods
            .setRaydiumPoolState(poolStatePubkey)
            .accounts({
                globalConfig: globalConfigPDA,
        solRewardsVault: solRewardsVaultPDA,
        solPrizePotVault: solPrizePotVaultPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Raydium pool state set successfully!");
    console.log(COLOR_SUCCESS, "✅ SOL rewards vault initialized!");
    console.log(COLOR_SUCCESS, "✅ SOL prize pot vault initialized!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.raydium_pool_state_set = {
            pool_state_address: poolStatePubkey.toString(),
      sol_rewards_vault: solRewardsVaultPDA.toString(),
      sol_prize_pot_vault: solPrizePotVaultPDA.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to set Raydium pool state:", error);
        throw error;
    }
}

async function initializeDogeConfig(minebtcProgram) {
    if (deploymentFile.doge_config_initialized) {
    console.log(COLOR_INFO, "ℹ️ DogeConfig already initialized. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING DOGE CONFIG ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    const [dogesConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("doge-config")],
    minebtcProgram.programId
    );

    // Get doge config values
    const basePrice = config.doges_config.base_price; 
    const curveA = config.doges_config.curve_a; // Curve steepness
    const maxSupply = config.doges_config.max_supply; // Genesis supply (15K), total 100K via breeding

    if (!basePrice || !curveA || !maxSupply) {
    console.error(COLOR_ERROR, "❌ Doge config values not found in config.json");
    throw new Error("Doge config values not found");
    }

    console.log(COLOR_INFO, `🔑 DogeConfig PDA: ${dogesConfigPDA.toString()}`);
    console.log(COLOR_INFO, `💰 Base Price: ${basePrice / 1e9} SOL`);
    console.log(COLOR_INFO, `📈 Curve A: ${curveA}`);
    console.log(COLOR_INFO, `🥚 Max Supply: ${maxSupply}`);

    try {
    const tx = await minebtcProgram.methods
      .initializeDogeConfig(new BN(basePrice), new BN(curveA), new BN(maxSupply))
            .accounts({
                dogesConfig: dogesConfigPDA,
                globalConfig: globalConfigPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ DogeConfig initialized successfully!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.doge_config_initialized = {
            doges_config_pda: dogesConfigPDA.toString(),
            base_price: basePrice.toString(),
            curve_a: curveA.toString(),
            max_supply: maxSupply.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
      console.log(COLOR_INFO, "ℹ️ DogeConfig already initialized. Skipping...");
            deploymentFile.doge_config_initialized = {
                doges_config_pda: dogesConfigPDA.toString(),
            };
            saveDeploymentData();
        } else {
      console.error(COLOR_ERROR, "❌ Failed to initialize DogeConfig:", error);
            throw error;
        }
    }
}

async function createDogeCollection(minebtcProgram) {
    if (deploymentFile.doge_collection_created) {
    console.log(COLOR_INFO, "ℹ️ Doge collection already created");
    console.log(
      COLOR_INFO,
      "🔑 Collection Address:",
      deploymentFile.doge_collection_created.collection_address
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ CREATING  DOGE COLLECTION ] ==================="
  );

    // Derive PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-config")],
    minebtcProgram.programId
    );

    const [dogesConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("doge-config")],
    minebtcProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection_authority")],
    minebtcProgram.programId
    );

  console.log(COLOR_INFO, "🎨 Creating Metaplex Core collection...");
    console.log(COLOR_DIM, `   Name: ${config.doges.collection_name}`);
    console.log(COLOR_DIM, `   URI: ${config.doges.collection_uri}`);
  console.log(
    COLOR_INFO,
    "🔐 Collection Authority PDA:",
    collectionAuthorityPDA.toString()
  );

    // Generate a new keypair for the collection
    const collectionKeypair = Keypair.generate();

    try {
    const tx = await minebtcProgram.methods
            .createDogeCollection(
                config.doges.collection_name,
                config.doges.collection_uri
            )
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                dogesConfig: dogesConfigPDA,
                collection: collectionKeypair.publicKey,
                collectionAuthority: collectionAuthorityPDA,
        mplCoreProgram: new PublicKey(
          "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        ),
                systemProgram: SystemProgram.programId,
            })
            .signers([collectionKeypair])
            .rpc();

        const collectionPubkey = collectionKeypair.publicKey;

    console.log(
      COLOR_SUCCESS,
      "✅ Doge collection created successfully!"
    );
    console.log(
      COLOR_INFO,
      "🔑 Collection Address:",
      collectionPubkey.toString()
    );
        console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `🔍 Explorer: https://explorer.solana.com/address/${collectionPubkey.toString()}?cluster=${CLUSTER}`
    );

        deploymentFile.doge_collection_created = {
            collection_address: collectionPubkey.toString(),
            collection_name: config.doges.collection_name,
            collection_uri: config.doges.collection_uri,
      collection_authority: collectionAuthorityPDA.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to create collection:", error);
        throw error;
    }
}


async function initializeDogeRoyalties(minebtcProgram) {
    if (deploymentFile.doge_royalties_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Doge royalties already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING  DOGE ROYALTIES ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const collectionPubkey = new PublicKey(
    deploymentFile.doge_collection_created.collection_address
  );

    const [dogesConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("doge-config")],
    minebtcProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection_authority")],
    minebtcProgram.programId
    );

    // Configure royalties
  const basisPoints = config.doges_config.royalties;
    let creators = [];

  // Convert addresses to PublicKey objects
  const multisigAddress = new PublicKey(
    config.deployment.FEE_RECIPIENT_MULTISIG
  );
  const treasuryAddress = new PublicKey(
    deploymentFile.minebtc_program_initialized.solTreasury_address
  );

  creators.push({
    address: multisigAddress,
    percentage:
      config.doges_config.creators.find(
        (creator) => creator.identifier === "multisig_fee_recipient"
      )?.percentage || 50,
  });
  creators.push({
    address: treasuryAddress,
    percentage:
      config.doges_config.creators.find(
        (creator) => creator.identifier === "treasury"
      )?.percentage || 50,
  });

  console.log(COLOR_INFO, `💎 Royalty: ${basisPoints / 100}%`);
    console.log(COLOR_INFO, `👥 Creators: ${creators.length}`);
    creators.forEach((creator, idx) => {
    console.log(
      COLOR_DIM,
      `   ${idx + 1}. ${creator.address.toBase58()} (${creator.percentage}%)`
    );
    });

    try {
    const tx = await minebtcProgram.methods
      .initDogeRoyalties(basisPoints, creators)
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                dogesConfig: dogesConfigPDA,
                collection: collectionPubkey,
                collectionAuthority: collectionAuthorityPDA,
        mplCoreProgram: new PublicKey(
          "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        ),
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Doge royalties initialized!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.doge_royalties_initialized = {
            basis_points: basisPoints,
            creators: creators,
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to initialize royalties:", error);
        throw error;
    }
}

async function configureTicketTiers(minebtcProgram) {
    if (deploymentFile.ticket_tier_configs_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Ticket tier configs already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ CONFIGURING TICKET TIERS ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    const [dogesConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("doge-config")],
    minebtcProgram.programId
    );

  const ticketTiers = config.doges_config.ticket_tiers || [];

  console.log(
    COLOR_INFO,
    `📝 Adding ${ticketTiers.length} ticket tier configs...`
  );

    const addedTiers = [];

    for (const tier of ticketTiers) {
    console.log(
      `   Tier ${tier.tier_index}: ${tier.ticket_value / 1e9} SOL`
    );

    try {
      const tx = await minebtcProgram.methods
                .addTicketTierConfig(
                    tier.tier_index,
                    new BN(tier.ticket_value)
                )
                .accounts({
                    globalConfig: globalConfigPDA,
                    dogesConfig: dogesConfigPDA,
                    authority: wallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc();

            console.log(COLOR_SUCCESS, `      ✅ Tier ${tier.tier_index} configured`);
            addedTiers.push({ ...tier, tx_signature: tx });
        } catch (error) {
      console.error(
        COLOR_ERROR,
        `❌ Failed to add tier ${tier.tier_index}:`,
        error
      );
            throw error;
        }
    }

  console.log(COLOR_SUCCESS, "✅ All ticket tier configs initialized!");

    deploymentFile.ticket_tier_configs_initialized = {
        ticket_tiers: addedTiers,
    timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
}

async function initializeTaxConfig(minebtcProgram) {
    if (deploymentFile.tax_config_initialized) {
    console.log(COLOR_INFO, "ℹ️ Tax config already initialized. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING TAX CONFIG ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    // Derive PDAs
    const [taxConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("tax-config")],
    minebtcProgram.programId
    );

    const [withdrawWithheldAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("withdraw-withheld-authority")],
    minebtcProgram.programId
    );

    const [factionTreasuryVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("faction-treasury-vault")],
    minebtcProgram.programId
    );

    const [nftFloorSweepVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("nft-floor-sweep-vault")],
    minebtcProgram.programId
    );

    const [nftSaleSolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("nft-sale-sol-vault")],
    minebtcProgram.programId
    );

    // Get config values
    const whitelistedAddress = config.tax.nft_floor_sweep_whitelisted_address;
    const nftFloorSweepPct = config.tax.nft_floor_sweep_pct;
    const factionTreasuryPct = config.tax.faction_treasury_pct;
    const burnPct = config.tax.burnt_pct;

    // Splits must sum to exactly 100% — validated here so a misconfigured
    // config.json fails before the transaction is built.
    if (nftFloorSweepPct + factionTreasuryPct + burnPct !== 100) {
        throw new Error(
            `Tax splits must sum to 100 (got ${nftFloorSweepPct}+${factionTreasuryPct}+${burnPct}=${nftFloorSweepPct + factionTreasuryPct + burnPct})`
        );
    }

    console.log(COLOR_INFO, `💰 Tax Distribution:`);
    console.log(COLOR_INFO, `   NFT Floor Sweep: ${nftFloorSweepPct}%`);
    console.log(COLOR_INFO, `   Faction Treasury: ${factionTreasuryPct}%`);
    console.log(COLOR_INFO, `   Burn: ${burnPct}%`);
    console.log(COLOR_INFO, `🔑 Whitelisted Address: ${whitelistedAddress}`);

    try {
    const tx = await minebtcProgram.methods
            .initializeTaxConfig(
                nftFloorSweepPct,
                factionTreasuryPct,
                burnPct,
                new PublicKey(whitelistedAddress)
            )
            .accounts({
                globalConfig: globalConfigPDA,
                taxConfig: taxConfigPDA,
                minebtcMint: DOGEBTC_TOKEN_MINT,
                withdrawWithheldAuthority: withdrawWithheldAuthorityPDA,
                factionTreasuryVault: factionTreasuryVaultPDA,
                nftFloorSweepVault: nftFloorSweepVaultPDA,
                nftSaleSolVault: nftSaleSolVaultPDA,
                authority: wallet.publicKey,
                tokenProgram2022: anchor_spl.TOKEN_2022_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Tax config initialized successfully!");
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
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to initialize tax config:", error);
        throw error;
    }
}

async function initializeGameState(minebtcProgram) {
    if (deploymentFile.game_state_initialized) {
    console.log(COLOR_INFO, "ℹ️ Game state already initialized. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING GAME STATE ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    // Derive GlobalGameState PDA
    const [globalGameStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-game-state")],
    minebtcProgram.programId
    );

    const roundDurationSeconds = config.game.round_duration_seconds;

  console.log(
    COLOR_INFO,
    `🔑 Global Game State PDA: ${globalGameStatePDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `⏱️ Round Duration: ${roundDurationSeconds} seconds (${
      roundDurationSeconds / 3600
    } hours)`
  );

  try {
    const tx = await minebtcProgram.methods
            .initializeGameState(new BN(roundDurationSeconds))
            .accounts({
                globalGameState: globalGameStatePDA,
                globalConfig: globalConfigPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Game state initialized successfully!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.game_state_initialized = {
            global_game_state_pda: globalGameStatePDA.toString(),
            round_duration_seconds: roundDurationSeconds,
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
      console.log(COLOR_INFO, "ℹ️ Game state already initialized. Skipping...");
            deploymentFile.game_state_initialized = {
                global_game_state_pda: globalGameStatePDA.toString(),
                round_duration_seconds: roundDurationSeconds,
            };
            saveDeploymentData();
        } else {
      console.error(COLOR_ERROR, "❌ Failed to initialize game state:", error);
            throw error;
        }
    }
}

async function initializeLpTokenAccounts(minebtcProgram) {
    if (deploymentFile.lp_token_accounts_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ LP token accounts already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING LP TOKEN ACCOUNTS ] ================"
  );

    try {
        if (!deploymentFile.dbtc_sol_pool_created?.lpMintPDA) {
      console.log(
        COLOR_WARNING,
        "⚠️ LP mint not found in deployment file. Cannot initialize LP token accounts."
      );
            return;
        }

        if (!deploymentFile.mining_vault_initialized?.vault_authority) {
      console.log(
        COLOR_WARNING,
        "⚠️ Vault authority not found. Cannot initialize LP token accounts."
      );
            return;
        }

    const lpMint = new PublicKey(
      deploymentFile.dbtc_sol_pool_created.lpMintPDA
    );
    const vaultAuthority = new PublicKey(
      deploymentFile.mining_vault_initialized.vault_authority
    );

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
      console.log(
        COLOR_INFO,
        "ℹ️ LP token accounts already exist. Skipping..."
      );
            deploymentFile.lp_token_accounts_initialized = {
                lp_token_account: lpTokenAccount.toString(),
                lp_token_owner: vaultAuthority.toString(),
                lp_mint: lpMint.toString(),
            };
            saveDeploymentData();
            return;
        }

    console.log(COLOR_INFO, "🔄 Initializing LP token accounts...");
    console.log(
      COLOR_DIM,
      `   LP Token Account (ATA): ${lpTokenAccount.toString()}`
    );
    console.log(
      COLOR_DIM,
      `   LP Token Owner (Vault Authority): ${vaultAuthority.toString()}`
    );
        console.log(COLOR_DIM, `   LP Mint: ${lpMint.toString()}`);

        // Create associated token account
        const createdAccount = await anchor_spl.getOrCreateAssociatedTokenAccount(
            connection,
            walletKeypair,
            lpMint,
            vaultAuthority,
            true,
      "confirmed",
            {},
            anchor_spl.TOKEN_PROGRAM_ID
        );

    console.log(
      COLOR_SUCCESS,
      "✅ LP token accounts initialized successfully!"
    );
    console.log(
      COLOR_DIM,
      `   LP Token Account: ${createdAccount.address.toString()}`
    );

        deploymentFile.lp_token_accounts_initialized = {
            lp_token_account: createdAccount.address.toString(),
            lp_token_owner: vaultAuthority.toString(),
            lp_mint: lpMint.toString(),
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
    console.error(
      COLOR_ERROR,
      "❌ Failed to initialize LP token accounts:",
      error
    );
    console.log(
      COLOR_WARNING,
      "   This may not be critical - LP accounts can be created on-demand"
    );
  }
}

/**
 * Update program configuration (authority and/or fee recipient)
 * @param {Program} minebtcProgram - MineBTC program instance
 * @param {Object} options - Update options
 * @param {string|null} options.newAuthorityAddress - New authority address (null to skip)
 * @param {string|null} options.newFeeRecipientAddress - New fee recipient address (null to skip)
 */
async function updateConfig(minebtcProgram, options = {}) {
  const { newAuthorityAddress, newFeeRecipientAddress } = options;
  
  // Determine what we're updating
  const updatingAuthority = newAuthorityAddress !== null && newAuthorityAddress !== undefined;
  const updatingFeeRecipient = newFeeRecipientAddress !== null && newFeeRecipientAddress !== undefined;
  
  if (!updatingAuthority && !updatingFeeRecipient) {
    console.log(
      COLOR_WARNING,
      "⚠️ No updates specified. Skipping config update..."
    );
    return;
  }

  const updateType = updatingAuthority && updatingFeeRecipient 
    ? "AUTHORITY & FEE RECIPIENT"
    : updatingAuthority 
    ? "AUTHORITY"
    : "FEE RECIPIENT";

  console.log(
    COLOR_STEP,
    `\n================ [ UPDATING ${updateType} ] ================`
  );

  // Check if program is initialized
  if (!deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ MineBTC program not initialized. Skipping config update..."
    );
    return;
  }

  try {
    // Load PDAs
    const globalConfigPDA = new PublicKey(
      deploymentFile.minebtc_program_initialized.globalConfig_address
    );

    // Get current config
    const globalConfig = await minebtcProgram.account.globalConfig.fetch(
      globalConfigPDA
    );
    const currentAuthority = globalConfig.extAuthority;
    const currentFeeRecipient = globalConfig.feeRecipient;

    // Prepare new values
    let newAuthority = null;
    let newFeeRecipient = null;

    if (updatingAuthority) {
      newAuthority = new PublicKey(newAuthorityAddress);
      console.log(
        COLOR_INFO,
        `🔑 Current authority: ${currentAuthority.toString()}`
      );
      console.log(
        COLOR_INFO,
        `🔑 New authority: ${newAuthority.toString()}`
      );

      // Check if already set
      if (currentAuthority.equals(newAuthority)) {
        console.log(
          COLOR_WARNING,
          `   ⚠️ Authority is already set to ${newAuthority.toString()}`
        );
        newAuthority = null; // Skip update
      }
    }

    if (updatingFeeRecipient) {
      newFeeRecipient = new PublicKey(newFeeRecipientAddress);
      console.log(
        COLOR_INFO,
        `💰 Current fee recipient: ${currentFeeRecipient.toString()}`
      );
      console.log(
        COLOR_INFO,
        `💰 New fee recipient: ${newFeeRecipient.toString()}`
      );

      // Check if already set
      if (currentFeeRecipient.equals(newFeeRecipient)) {
        console.log(
          COLOR_WARNING,
          `   ⚠️ Fee recipient is already set to ${newFeeRecipient.toString()}`
        );
        newFeeRecipient = null; // Skip update
      }
    }

    // If both are already set correctly, skip
    if (!newAuthority && !newFeeRecipient) {
      console.log(
        COLOR_WARNING,
        "⚠️ All values are already set correctly. Skipping update..."
      );
      return;
    }

    // Derive DogeBtcMining PDA (optional account)
    const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("mine-btc-mining")],
      minebtcProgram.programId
    );

    console.log(
      COLOR_INFO,
      `   Global Config PDA: ${globalConfigPDA.toString()}`
    );
    console.log(COLOR_INFO, `   Current Authority: ${wallet.publicKey.toString()}`);

    // Build and send transaction
    // Pass null for values we don't want to change
    const tx = await minebtcProgram.methods
      .updateConfig(newAuthority, newFeeRecipient)
      .accounts({
        globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Config updated successfully!`);
    console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `   Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    // Update deployment file
    const updateData = {
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };

    if (newAuthority) {
      updateData.old_authority = currentAuthority.toString();
      updateData.new_authority = newAuthority.toString();
    }

    if (newFeeRecipient) {
      updateData.old_fee_recipient = currentFeeRecipient.toString();
      updateData.new_fee_recipient = newFeeRecipient.toString();
    }

    if (newAuthority) {
      if (!deploymentFile.authority_updated) {
        deploymentFile.authority_updated = {};
      }
      deploymentFile.authority_updated = {
        old_authority: currentAuthority.toString(),
        new_authority: newAuthority.toString(),
        tx_signature: tx,
        timestamp: new Date().toISOString(),
      };
    }

    if (newFeeRecipient) {
      if (!deploymentFile.fee_recipient_updated) {
        deploymentFile.fee_recipient_updated = {};
      }
      deploymentFile.fee_recipient_updated = {
        old_fee_recipient: currentFeeRecipient.toString(),
        new_fee_recipient: newFeeRecipient.toString(),
        tx_signature: tx,
        timestamp: new Date().toISOString(),
      };

      // Also update the initial deployment data
      if (deploymentFile.minebtc_program_initialized) {
        deploymentFile.minebtc_program_initialized.FEE_RECIPIENT_MULTISIG =
          newFeeRecipient.toString();
      }
    }

    if (newAuthority && deploymentFile.minebtc_program_initialized) {
      deploymentFile.minebtc_program_initialized.EXT_AUTHORITY =
        newAuthority.toString();
    }

    // Save deployment file
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentFile, null, 2));

  } catch (error) {
    console.error(COLOR_ERROR, "❌ Error updating config:", error);
    throw error;
  }
}

/**
 * Update fee recipient (backward compatibility wrapper)
 * @param {Program} minebtcProgram - MineBTC program instance
 * @param {string} newFeeRecipientAddress - New fee recipient address
 */
async function updateFeeRecipient(minebtcProgram, newFeeRecipientAddress) {
  await updateConfig(minebtcProgram, {
    newFeeRecipientAddress: newFeeRecipientAddress,
    newAuthorityAddress: null,
  });
}

/**
 * Update authority (convenience wrapper)
 * @param {Program} minebtcProgram - MineBTC program instance
 * @param {string} newAuthorityAddress - New authority address
 */
async function updateAuthority(minebtcProgram, newAuthorityAddress) {
  await updateConfig(minebtcProgram, {
    newAuthorityAddress: newAuthorityAddress,
    newFeeRecipientAddress: null,
  });
}

async function updateFees(minebtcProgram, feeConfig) {
  console.log(
    COLOR_STEP,
    "\n================ [ UPDATING FEES ] ================"
  );

  // Check if program is initialized
  if (!deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ MineBTC program not initialized. Skipping fee update..."
    );
    return;
  }

  try {
    // Load PDAs
    const globalConfigPDA = new PublicKey(
      deploymentFile.minebtc_program_initialized.globalConfig_address
    );
    const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("mine-btc-mining")],
      minebtcProgram.programId
    );

    // Get current config
    const globalConfig = await minebtcProgram.account.globalConfig.fetch(
      globalConfigPDA
    );

    console.log(COLOR_INFO, "   Current SOL fee config:");
    console.log(
      COLOR_INFO,
      `     Protocol fee: ${globalConfig.solFeeConfig.protocolFeePct}%`
    );
    console.log(
      COLOR_INFO,
      `     Buyback: ${globalConfig.solFeeConfig.buybackPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Stakers: ${globalConfig.solFeeConfig.stakersPct}%`
    );

    console.log(COLOR_INFO, "   Current DogeBtc dist config:");
    console.log(
      COLOR_INFO,
      `     Stakers: ${globalConfig.minebtcDistConfig.minebtcStakersPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Winners: ${globalConfig.minebtcDistConfig.minebtcWinnersPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Same-faction: ${globalConfig.minebtcDistConfig.minebtcSameFactionPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Motherlode: ${globalConfig.minebtcDistConfig.minebtcMotherlodePct}%`
    );
    console.log(
      COLOR_INFO,
      `     Refining fee: ${globalConfig.minebtcDistConfig.refiningFee}%`
    );
    console.log(
      COLOR_INFO,
      `     Change faction fee: ${globalConfig.changeFactionFee.toString()} lamports`
    );
    console.log(
      COLOR_INFO,
      `     Snapshot interval: ${globalConfig.snapshotInterval.toString()} seconds`
    );

    // Prepare fee config with defaults (null = don't update)
    const feeParams = {
      newProtocolFeePct: feeConfig?.newProtocolFeePct ?? null,
      newBuybackPct: feeConfig?.newBuybackPct ?? null,
      newStakersPct: feeConfig?.newStakersPct ?? null,
      newMinebtcStakersPct: feeConfig?.newMinebtcStakersPct ?? null,
      newMinebtcWinnersPct: feeConfig?.newMinebtcWinnersPct ?? null,
      newMinebtcSameFactionPct: feeConfig?.newMinebtcSameFactionPct ?? null,
      newMinebtcMotherlodePct: feeConfig?.newMinebtcMotherlodePct ?? null,
      newRefiningFee: feeConfig?.newRefiningFee ?? null,
      changeFactionFee: feeConfig?.changeFactionFee
        ? new BN(feeConfig.changeFactionFee)
        : null,
      snapshotInterval:
        (feeConfig?.snapshotInterval ?? feeConfig?.snapshot_interval) != null
          ? new BN(
              feeConfig?.snapshotInterval ?? feeConfig?.snapshot_interval
            )
        : null,
    };

    // Log what will be updated
    console.log(COLOR_INFO, "\n   Updating fees:");
    if (feeParams.newProtocolFeePct !== null)
      console.log(
        COLOR_INFO,
        `     Protocol fee: ${feeParams.newProtocolFeePct}%`
      );
    if (feeParams.newBuybackPct !== null)
      console.log(COLOR_INFO, `     Buyback: ${feeParams.newBuybackPct}%`);
    if (feeParams.newStakersPct !== null)
      console.log(COLOR_INFO, `     Stakers: ${feeParams.newStakersPct}%`);
    if (feeParams.newMinebtcStakersPct !== null)
      console.log(
        COLOR_INFO,
        `     DBTC Stakers: ${feeParams.newMinebtcStakersPct}%`
      );
    if (feeParams.newMinebtcWinnersPct !== null)
      console.log(
        COLOR_INFO,
        `     DBTC Winners: ${feeParams.newMinebtcWinnersPct}%`
      );
    if (feeParams.newMinebtcSameFactionPct !== null)
      console.log(
        COLOR_INFO,
        `     DBTC Same-faction: ${feeParams.newMinebtcSameFactionPct}%`
      );
    if (feeParams.newMinebtcMotherlodePct !== null)
      console.log(
        COLOR_INFO,
        `     DBTC Motherlode: ${feeParams.newMinebtcMotherlodePct}%`
      );
    if (feeParams.newRefiningFee !== null)
      console.log(
        COLOR_INFO,
        `     Refining fee: ${feeParams.newRefiningFee}%`
      );
    if (feeParams.changeFactionFee !== null)
      console.log(
        COLOR_INFO,
        `     Change faction fee: ${feeParams.changeFactionFee.toString()} lamports`
      );
    if (feeParams.snapshotInterval !== null)
      console.log(
        COLOR_INFO,
        `     Snapshot interval: ${feeParams.snapshotInterval.toString()} seconds`
      );

    console.log(
      COLOR_INFO,
      `   Global Config PDA: ${globalConfigPDA.toString()}`
    );
    console.log(
      COLOR_INFO,
      `   MineBTC Mining PDA: ${mineBtcMiningPDA.toString()}`
    );
    console.log(COLOR_INFO, `   Authority: ${wallet.publicKey.toString()}`);

    // Build and send transaction
    const tx = await minebtcProgram.methods
      .updateFees(
        feeParams.newProtocolFeePct,
        feeParams.newBuybackPct,
        feeParams.newStakersPct,
        feeParams.newMinebtcStakersPct,
        feeParams.newMinebtcWinnersPct,
        feeParams.newMinebtcSameFactionPct,
        feeParams.newMinebtcMotherlodePct,
        feeParams.newRefiningFee,
        feeParams.changeFactionFee,
        feeParams.snapshotInterval
      )
      .accounts({
        globalConfig: globalConfigPDA,
        mineBtcMining: mineBtcMiningPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Fees updated successfully!`);
    console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `   Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    // Update deployment file
    if (!deploymentFile.fees_updated) {
      deploymentFile.fees_updated = {};
    }
    deploymentFile.fees_updated = {
      fee_config: feeConfig,
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };

    saveDeploymentData();
  } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to update fees:", error);
    throw error;
  }
}

async function updateDogeConfig(minebtcProgram, dogeConfig) {
  console.log(
    COLOR_STEP,
    "\n================ [ UPDATING DOGE CONFIG ] ================"
  );

  // Check if program is initialized
  if (!deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ MineBTC program not initialized. Skipping doge config update..."
    );
    return;
  }

  // Check if doge config is initialized
  if (!deploymentFile.doge_config_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ Doge config not initialized. Please initialize it first."
    );
    return;
  }

  try {
    // Load PDAs
    const globalConfigPDA = new PublicKey(
      deploymentFile.minebtc_program_initialized.globalConfig_address
    );

    const [dogesConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("doge-config")],
      minebtcProgram.programId
    );

    // Get current config
    const dogesConfig = await minebtcProgram.account.dogeConfig.fetch(
      dogesConfigPDA
    );

    console.log(COLOR_INFO, "   Current Doge Config:");
    console.log(
      COLOR_INFO,
      `     Base Price: ${dogesConfig.basePrice.toString()} lamports (${dogesConfig.basePrice.toNumber() / 1e9} SOL)`
    );
    console.log(
      COLOR_INFO,
      `     Curve A: ${dogesConfig.curveA.toString()}`
    );
    console.log(
      COLOR_INFO,
      `     Max Supply: ${dogesConfig.maxSupply.toString()}`
    );

    // Get values from config or use provided values
    const basePrice = dogeConfig?.basePrice 
      ? new BN(dogeConfig.basePrice)
      : new BN(config.doges_config.base_price);
    const curveA = dogeConfig?.curveA 
      ? new BN(dogeConfig.curveA)
      : new BN(config.doges_config.curve_a);

    console.log(COLOR_INFO, "\n   Updating Doge Config:");
    console.log(
      COLOR_INFO,
      `     Base Price: ${basePrice.toString()} lamports (${basePrice.toNumber() / 1e9} SOL)`
    );
    console.log(COLOR_INFO, `     Curve A: ${curveA.toString()}`);

    console.log(
      COLOR_INFO,
      `   Global Config PDA: ${globalConfigPDA.toString()}`
    );
    console.log(COLOR_INFO, `   Doge Config PDA: ${dogesConfigPDA.toString()}`);
    console.log(COLOR_INFO, `   Authority: ${wallet.publicKey.toString()}`);

    // Build and send transaction
    const tx = await minebtcProgram.methods
      .updateDogeConfig(basePrice, curveA)
      .accounts({
        globalConfig: globalConfigPDA,
        dogesConfig: dogesConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Doge config updated successfully!`);
    console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `   Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    // Update deployment file
    if (!deploymentFile.doge_config_updated) {
      deploymentFile.doge_config_updated = {};
    }
    deploymentFile.doge_config_updated = {
      base_price: basePrice.toString(),
      curve_a: curveA.toString(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };

    saveDeploymentData();
  } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to update doge config:", error);
    throw error;
  }
}

async function addGameCrankerBot(minebtcProgram, botWalletAddress) {
  // Check if cranker bots are already added
  if (deploymentFile.cranker_bots_added) {
    console.log(COLOR_INFO, "ℹ️ Cranker bots already added. Skipping...");
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ ADDING CRANKER BOTS ] ================"
  );

  // Check if game state is initialized
  if (!deploymentFile.game_state_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ Game state not initialized. Skipping cranker bot addition..."
    );
    return;
  }

  const botPubkey = new PublicKey(botWalletAddress);
  console.log(COLOR_INFO, `🤖 Adding cranker bot: ${botPubkey.toString()}`);
  // return;

  try {
    // Load PDAs
    const globalConfigPDA = new PublicKey(
      deploymentFile.minebtc_program_initialized.globalConfig_address
    );
    const globalGameStatePDA = new PublicKey(
      deploymentFile.game_state_initialized.global_game_state_pda
    );

    console.log(
      COLOR_INFO,
      `   Global Config PDA: ${globalConfigPDA.toString()}`
    );
    console.log(
      COLOR_INFO,
      `   Global Game State PDA: ${globalGameStatePDA.toString()}`
    );
    console.log(COLOR_INFO, `   Authority: ${wallet.publicKey.toString()}`);

    // Check if bot is already added
    try {
      const gameState = await minebtcProgram.account.globalGameSate.fetch(
        globalGameStatePDA
      );
      if (
        gameState.crankerBots &&
        gameState.crankerBots.some((bot) => bot.equals(botPubkey))
      ) {
        console.log(
          COLOR_WARNING,
          `   ⚠️ Bot ${botPubkey.toString()} is already whitelisted`
        );
        deploymentFile.cranker_bots_added = {
          bots: [botPubkey.toString()],
          timestamp: new Date().toISOString(),
          status: "already_exists",
        };
        saveDeploymentData();
        return;
      }
    } catch (error) {
      // Game state might not exist yet, continue
    }

    // Build and send transaction
    const tx = await minebtcProgram.methods
      .addCrankerBot(botPubkey)
      .accounts({
        globalGameState: globalGameStatePDA,
        globalConfig: globalConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Cranker bot added successfully!`);
    console.log(COLOR_DIM, `   Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `   Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    deploymentFile.cranker_bots_added = {
      bots: [botPubkey.toString()],
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    const errorStr = error.toString();
    if (
      errorStr.includes("already") ||
      errorStr.includes("InvalidParameters")
    ) {
      console.log(
        COLOR_WARNING,
        `   ⚠️ Bot may already be whitelisted or max bots reached`
      );
      console.log(COLOR_DIM, `   Error: ${errorStr.substring(0, 200)}`);

      deploymentFile.cranker_bots_added = {
        bots: [botPubkey.toString()],
        status: "error_or_exists",
        error: errorStr.substring(0, 200),
        timestamp: new Date().toISOString(),
      };
      saveDeploymentData();
    } else {
      console.error(COLOR_ERROR, "❌ Failed to add cranker bot:", error);
      throw error;
    }
    }
}

async function initializeEpochConfig(minebtcProgram) {
  if (deploymentFile.epoch_config_initialized) {
    console.log(COLOR_INFO, 'ℹ️ Epoch config already initialized. Skipping...');
    return;
  }

  console.log(
    COLOR_STEP,
    '\n================ [ INITIALIZING EPOCH CONFIG ] ================'
  );

  try {
    const [epochConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('epoch-config')],
      minebtcProgram.programId
    );

    const [globalConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('global-config')],
      minebtcProgram.programId
    );

    // Oracle authority = cranker bot wallet (same as game cranker)
    const oracleAuthority = gameKeypair.publicKey;
    // Read epoch config values from config.json
    const epochDuration = config.epoch.epoch_duration_seconds;
    const initialRiskFactor = config.epoch.initial_risk_factor;
    const model5Pct = config.epoch.model5_pct;
    const top1Pct = config.epoch.top1_pct;
    const top2Pct = config.epoch.top2_pct;
    const top3Pct = config.epoch.top3_pct;

    console.log(COLOR_INFO, `🔑 Epoch Config PDA: ${epochConfigPda.toBase58()}`);
    console.log(COLOR_INFO, `🔑 Global Config PDA: ${globalConfigPda.toBase58()}`);
    console.log(COLOR_INFO, `🔑 Oracle Authority: ${oracleAuthority.toBase58()}`);
    console.log(
      COLOR_INFO,
      `⏱️  Epoch Duration: ${epochDuration}s (${(epochDuration / 3600).toFixed(2)}h from config)`
    );
    console.log(COLOR_INFO, `📊 Initial Risk Factor: ${initialRiskFactor} (1.00x)`);
    console.log(
      COLOR_INFO,
      `📊 Epoch pool splits -> base rank pool: ${model5Pct}%, top1 bonus: ${top1Pct}%, top2 bonus: ${top2Pct}%, top3 bonus: ${top3Pct}%`
    );

    const tx = await minebtcProgram.methods
      .initializeEpochConfig(oracleAuthority, new BN(epochDuration), initialRiskFactor, model5Pct, top1Pct, top2Pct, top3Pct)
      .accounts({
        epochConfig: epochConfigPda,
        globalConfig: globalConfigPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Epoch config initialized successfully!`);
    console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    deploymentFile.epoch_config_initialized = {
      epoch_config_pda: epochConfigPda.toBase58(),
      oracle_authority: oracleAuthority.toBase58(),
      epoch_duration: epochDuration,
      initial_risk_factor: initialRiskFactor,
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    if (error.toString().includes('already in use')) {
      console.log(COLOR_INFO, 'ℹ️ Epoch config already exists on-chain. Skipping...');
      deploymentFile.epoch_config_initialized = {
        status: 'already_exists',
        timestamp: new Date().toISOString(),
      };
      saveDeploymentData();
    } else {
      console.error(COLOR_ERROR, '❌ Failed to initialize epoch config:', error);
      throw error;
    }
  }
}

async function initializeBootstrapIndexState(minebtcProgram) {
  if (deploymentFile.epoch_index_state_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Bootstrap epoch index state already initialized. Skipping..."
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING BOOTSTRAP INDEX STATE ] ================"
  );

  const { indexId, name, initialScores } = getInitialIndexConfig();
  const [globalConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-config")],
    minebtcProgram.programId
  );
  const [indexStatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("index-state"), Buffer.from([indexId])],
    minebtcProgram.programId
  );

  try {
    const existing = await minebtcProgram.account.indexState.fetch(indexStatePda);
    console.log(
      COLOR_INFO,
      `ℹ️ Index state ${existing.indexId} already exists at ${indexStatePda.toBase58()}. Skipping...`
    );
    deploymentFile.epoch_index_state_initialized = {
      index_id: indexId,
      name,
      index_state_pda: indexStatePda.toBase58(),
      status: "already_exists",
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
    return;
  } catch (_) {
    // continue and create
  }

  console.log(COLOR_INFO, `🔑 Index State PDA: ${indexStatePda.toBase58()}`);
  console.log(COLOR_INFO, `📊 Index ID: ${indexId}`);
  console.log(COLOR_INFO, `📝 Name: ${name}`);
  console.log(COLOR_INFO, `📈 Initial Scores: ${initialScores.join(", ")}`);

  const tx = await minebtcProgram.methods
    .initializeIndexState(indexId, name, initialScores.map((s) => new BN(s)))
    .accounts({
      indexState: indexStatePda,
      globalConfig: globalConfigPda,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log(COLOR_SUCCESS, "✅ Bootstrap index state initialized!");
  console.log(COLOR_DIM, `   Transaction: ${tx}`);

  deploymentFile.epoch_index_state_initialized = {
    index_id: indexId,
    name,
    index_state_pda: indexStatePda.toBase58(),
    initial_scores: initialScores,
    tx_signature: tx,
    timestamp: new Date().toISOString(),
  };
  saveDeploymentData();
}

async function scheduleInitialEpochMarket(minebtcProgram) {
  if (deploymentFile.epoch_market_scheduled) {
    console.log(
      COLOR_INFO,
      "ℹ️ Initial epoch market already scheduled. Skipping..."
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ SCHEDULING INITIAL EPOCH MARKET ] ================"
  );

  const { indexId, questionHash } = getInitialIndexConfig();
  const [epochConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("epoch-config")],
    minebtcProgram.programId
  );
  const [indexStatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("index-state"), Buffer.from([indexId])],
    minebtcProgram.programId
  );

  const epochConfig = await minebtcProgram.account.epochConfig.fetch(epochConfigPda);
  const activeQuestionHash = Array.from(epochConfig.activeQuestionHash || []);
  const nextQuestionHash = Array.from(epochConfig.nextQuestionHash || []);
  const hasAnyScheduledHash =
    activeQuestionHash.some((byte) => byte !== 0) ||
    nextQuestionHash.some((byte) => byte !== 0);

  if (
    hasAnyScheduledHash &&
    Number(epochConfig.activeIndexId) === indexId &&
    activeQuestionHash.length === 32
  ) {
    console.log(
      COLOR_INFO,
      "ℹ️ Epoch market already scheduled on-chain. Skipping..."
    );
    deploymentFile.epoch_market_scheduled = {
      index_id: indexId,
      epoch_config_pda: epochConfigPda.toBase58(),
      index_state_pda: indexStatePda.toBase58(),
      active_question_hash: activeQuestionHash,
      status: "already_exists",
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
    return;
  }

  console.log(COLOR_INFO, `🔑 Epoch Config PDA: ${epochConfigPda.toBase58()}`);
  console.log(COLOR_INFO, `🔑 Index State PDA: ${indexStatePda.toBase58()}`);
  console.log(COLOR_INFO, `🔑 Oracle Authority: ${gameKeypair.publicKey.toBase58()}`);
  console.log(COLOR_INFO, `🧠 Question Hash: ${Buffer.from(questionHash).toString("hex")}`);

  const tx = await minebtcProgram.methods
    .scheduleNextEpochMarket(indexId, questionHash)
    .accounts({
      epochConfig: epochConfigPda,
      indexState: indexStatePda,
      authority: gameKeypair.publicKey,
    })
    .signers([gameKeypair])
    .rpc();

  console.log(COLOR_SUCCESS, "✅ Initial epoch market scheduled!");
  console.log(COLOR_DIM, `   Transaction: ${tx}`);

  deploymentFile.epoch_market_scheduled = {
    index_id: indexId,
    epoch_config_pda: epochConfigPda.toBase58(),
    index_state_pda: indexStatePda.toBase58(),
    oracle_authority: gameKeypair.publicKey.toBase58(),
    question_hash: questionHash,
    tx_signature: tx,
    timestamp: new Date().toISOString(),
  };
  saveDeploymentData();
}

function printCompletionSummary() {
  console.log(
    COLOR_STEP,
    "\n🎉 ================================ INITIALIZATION COMPLETE ================================"
  );
  console.log(COLOR_SUCCESS, "✅ All systems initialized successfully!");
  console.log(COLOR_INFO, "\n📋 Summary:");
  console.log(
    COLOR_INFO,
    `  • MineBTC Program: ${
      deploymentFile.minebtc_program_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • System Accounts: ${
      deploymentFile.system_accounts_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Factions: ${
      deploymentFile.factions_added
        ? config.factions.length + " added ✅"
        : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Mining System: ${
      deploymentFile.mining_vault_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Mining Tokens: ${deploymentFile.mining_tokens_deposited ? "✅" : "❌"}`
  );
  console.log(
    COLOR_INFO,
    `  • Raydium Pool State: ${
      deploymentFile.raydium_pool_state_set ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Doge Collection: ${
      deploymentFile.doge_collection_created ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Doge Royalties: ${
      deploymentFile.doge_royalties_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Ticket Tiers: ${
      deploymentFile.ticket_tier_configs_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Tax Config: ${deploymentFile.tax_config_initialized ? "✅" : "❌"}`
  );
  console.log(
    COLOR_INFO,
    `  • Game State: ${deploymentFile.game_state_initialized ? "✅" : "❌"}`
  );
  console.log(
    COLOR_INFO,
    `  • LP Token Accounts: ${
      deploymentFile.lp_token_accounts_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_STEP,
    "========================================================================================"
  );

  if (deploymentFile.minebtc_program_initialized) {
    console.log(COLOR_DIM, "\n🔑 Important Addresses:");
    console.log(
      COLOR_DIM,
      `   Global Config: ${deploymentFile.minebtc_program_initialized.globalConfig_address}`
    );
    console.log(
      COLOR_DIM,
      `   Mining State: ${deploymentFile.minebtc_program_initialized.mineBtcMining_address}`
    );
    console.log(
      COLOR_DIM,
      `   SOL Treasury: ${deploymentFile.minebtc_program_initialized.solTreasury_address}`
    );
        if (deploymentFile.mining_vault_initialized) {
      console.log(
        COLOR_DIM,
        `   Mining Vault: ${deploymentFile.mining_vault_initialized.vault_address}`
      );
        }
        if (deploymentFile.doge_collection_created) {
      console.log(
        COLOR_DIM,
        `   Doge Collection: ${deploymentFile.doge_collection_created.collection_address}`
      );
        }
        if (deploymentFile.game_state_initialized) {
      console.log(
        COLOR_DIM,
        `   Game State: ${deploymentFile.game_state_initialized.global_game_state_pda}`
      );
    }
  }

  console.log(COLOR_INFO, "\n📝 Next Steps:");
  console.log(
    COLOR_INFO,
    "   1. Users can now initialize their PlayerData accounts"
  );
  console.log(
    COLOR_INFO,
    "   2. Users can mint Doge for their factions"
  );
  console.log(COLOR_INFO, "   3. Users can stake DogeBtc and LP tokens");
  console.log(
    COLOR_INFO,
    "   4. Admins can start game rounds with start_round"
  );
  console.log(
    COLOR_INFO,
    "   5. Keeper bots can harvest and distribute tax via crank functions"
  );
}

// Run the main script
main().catch(console.error);
