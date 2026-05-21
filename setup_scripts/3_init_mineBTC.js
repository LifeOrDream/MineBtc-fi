// Import Anchor as CommonJS package
import pkg from "@coral-xyz/anchor";
const { AnchorProvider, BN, Program, setProvider, web3 } = pkg;
import { SystemProgram } from "@solana/web3.js";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as anchor_spl from "@solana/spl-token";
import fs from "fs";
import path from "path";
import { setIdlAddress } from "./raydium_id_sync.js";
import { Uploader } from "@irys/upload";
import { Solana } from "@irys/upload-solana";

// Get the current file's directory
const __dirname = decodeURIComponent(new URL(".", import.meta.url).pathname);

// Load configuration
const configPath = path.resolve(__dirname, "./config.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
const repoRoot = path.resolve(__dirname, "..");
const irysWalletPath = path.resolve(repoRoot, "mainnet-irys-upload-wallet-keypair.json");

const CLUSTER = config.network.cluster;
const RPC_URL = config.network.rpc_url;
const COMMITMENT = config.network.commitment;

// fee_recipient is now keyed by cluster (devnet/mainnet); resolve once at
// startup. Fall back to a bare string for backward compat.
const FEE_RECIPIENT_MULTISIG_STR = (() => {
  const raw = config.deployment.FEE_RECIPIENT_MULTISIG;
  if (typeof raw === "string") return raw;
  const resolved = raw?.[CLUSTER];
  if (!resolved) {
    throw new Error(
      `config.deployment.FEE_RECIPIENT_MULTISIG missing entry for cluster '${CLUSTER}'. Expected object like { devnet: '...', mainnet: '...' }.`,
    );
  }
  return resolved;
})();

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
const DEGENBTC_TOKEN_MINT = deploymentFile.dbtc_mint_address
  ? new PublicKey(deploymentFile.dbtc_mint_address)
  : null;

const ID_MineBTC_PROGRAM = deploymentFile.MINE_BTC_PROGRAM_ID
  ? new PublicKey(deploymentFile.MINE_BTC_PROGRAM_ID)
  : null;

// Mining configuration
const MINING_DEGEN_BTC_PER_SLOT = new BN(config.mining.degen_btc_per_round);

// Keep these explicit in setup so fresh deployments don't silently depend on
// whatever the contract default happened to be at compile time.
const EMISSION_CONFIG = {
  priceChangeThresholdPct:
    config.emissions?.price_change_threshold_pct ?? 3,
  emissionIncreasePct: config.emissions?.emission_increase_pct ?? 1,
  emissionDecreasePct: config.emissions?.emission_decrease_pct ?? 3,
};

const FACTION_WAR_CONFIG = {
  isActive: config.faction_war?.is_active ?? true,
};

const GAME_CONFIG = {
  roundDurationSeconds: config.game?.round_duration_seconds ?? 60,
  roundsEnabledAtLaunch: config.game?.rounds_enabled_at_launch ?? true,
};

const GAMEPLAY_TUNING_CONFIG = {
  enableRpgProgression:
    config.gameplay_tuning?.enable_rpg_progression ?? true,
  maxEvolutionStageUnlocked:
    config.gameplay_tuning?.max_evolution_stage_unlocked ?? 0,
  factionWarBaseRewardBps:
    config.gameplay_tuning?.war_base_reward_bps ?? 7500,
  factionWarMvpRewardBps:
    config.gameplay_tuning?.war_mvp_reward_bps ?? 500,
  factionWarHashBeastRewardBps:
    config.gameplay_tuning?.war_hashbeast_reward_bps ?? 2000,
  baseMutationChanceBps:
    config.gameplay_tuning?.base_mutation_chance_bps ?? 2000,
  mutationChanceFloorBps:
    config.gameplay_tuning?.mutation_chance_floor_bps ?? 25,
  mutationChanceCapBps:
    config.gameplay_tuning?.mutation_chance_cap_bps ?? 2500,
  factionVolumeThresholdLamports:
    config.gameplay_tuning?.faction_volume_threshold_lamports ?? 85000000,
  extraVolumeThresholdPerMutationLamports:
    config.gameplay_tuning?.extra_volume_threshold_per_mutation_lamports ??
    85000000,
  targetMutationsPerCycle:
    config.gameplay_tuning?.target_mutations_per_cycle ?? 12,
  targetRoundsPerCycle:
    config.gameplay_tuning?.target_rounds_per_cycle ?? 240,
  pacingMaxAdjustmentBps:
    config.gameplay_tuning?.pacing_max_adjustment_bps ?? 4000,
};

const LIVE_FEE_CONFIG = {
  newProtocolFeePct: 15,
  newBuybackPct: 70,
  newStakersPct: 10,
  newMinebtcStakersPct: 3,
  newMinebtcWinnersPct: 50,
  newMinebtcSameFactionPct: 21,
  newMinebtcJackpotPct: 5,
  newHodlTaxPct: 10,
  snapshotInterval: 30 * 60,
  // Cycle SOL split: % of user bet reserved for faction-war jackpot (taken from gross bet, in addition to protocol fee)
  newCycleSolSplitPct: 5,
  // NFT market making: % of distribute_sol_fees SOL routed to inventory_sweep_vault
  // for the on-chain marketplace. Constraint: newBuybackPct + newNftMarketMakingPct <= 100.
  newNftMarketMakingPct: 3,
};

const DEGENBTC_MARKET_PROGRAM_ID = deploymentFile.DEGENBTC_MARKET_PROGRAM_ID
  ? new PublicKey(deploymentFile.DEGENBTC_MARKET_PROGRAM_ID)
  : null;

// Load MineBTC Program IDL
const rawMinebtcIdl = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, config.deployment.paths.minebtc_idl),
    "utf-8"
  )
);
const IDL_MineBTC = ID_MineBTC_PROGRAM
  ? setIdlAddress(rawMinebtcIdl, ID_MineBTC_PROGRAM)
  : rawMinebtcIdl;

// Load DegenBTC Marketplace IDL (optional — only required once the marketplace
// is being initialized; init steps below skip themselves if the IDL is absent
// so the script still works on early-stage deployments that haven't built it).
let IDL_DegenBtcMarket = null;
try {
  const marketIdlPath = config.deployment.paths.degenbtc_market_idl;
  if (marketIdlPath) {
    const rawMarketIdl = JSON.parse(
      fs.readFileSync(path.resolve(__dirname, marketIdlPath), "utf-8"),
    );
    IDL_DegenBtcMarket = DEGENBTC_MARKET_PROGRAM_ID
      ? setIdlAddress(rawMarketIdl, DEGENBTC_MARKET_PROGRAM_ID)
      : rawMarketIdl;
  }
} catch (e) {
  // Defer the warning to when the marketplace init step actually runs.
  IDL_DegenBtcMarket = null;
}

const MARKETPLACE_CONFIG = {
  feeBps: config.marketplace?.fee_bps ?? 300,
  minPriceLamports: new BN(config.marketplace?.min_price_lamports ?? 10_000_000),
};

const MPL_CORE_PROGRAM_ID = new PublicKey(
  "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d",
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

function valueEquals(left, right) {
  if (left === right) return true;
  if (left == null || right == null) return left == null && right == null;
  return left.toString() === right.toString();
}

async function fetchJsonMetadata(uri, label, requiredFields = []) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  try {
    const response = await fetch(uri, {
      headers: { accept: "application/json" },
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`${response.status} ${response.statusText}`);
    }
    const contentType = response.headers.get("content-type") || "";
    if (!contentType.toLowerCase().includes("application/json")) {
      throw new Error(`expected application/json, got ${contentType || "missing content-type"}`);
    }
    const json = await response.json();
    for (const field of requiredFields) {
      if (!json[field]) {
        throw new Error(`missing required field "${field}"`);
      }
    }
    console.log(COLOR_SUCCESS, `✅ ${label} metadata OK: ${uri}`);
    return json;
  } finally {
    clearTimeout(timeout);
  }
}

async function validateFetchableUrl(uri, label) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  try {
    const response = await fetch(uri, {
      method: "GET",
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`${response.status} ${response.statusText}`);
    }
    console.log(COLOR_SUCCESS, `✅ ${label} OK: ${uri}`);
  } finally {
    clearTimeout(timeout);
  }
}

async function uploadJsonToIrys(json, label) {
  if (!fs.existsSync(irysWalletPath)) {
    throw new Error(`Missing Irys upload wallet: ${irysWalletPath}`);
  }

  const wallet = JSON.parse(fs.readFileSync(irysWalletPath, "utf8"));
  const irys = await Uploader(Solana)
    .withWallet(wallet)
    .mainnet()
    .withRpc("https://api.mainnet-beta.solana.com");
  const data = Buffer.from(`${JSON.stringify(json, null, 2)}\n`);
  const price = await irys.getPrice(data.length);
  console.log(
    COLOR_INFO,
    `🧾 Uploading ${label} JSON to Irys (${data.length} bytes), price ${irys.utils.fromAtomic(price)} ${irys.token}`
  );
  await irys.fund(price);
  const receipt = await irys.upload(data, {
    tags: [{ name: "Content-Type", value: "application/json" }],
  });
  const uri = `https://gateway.irys.xyz/${receipt.id}`;
  console.log(COLOR_SUCCESS, `✅ Uploaded ${label} JSON: ${uri}`);
  return { uri, id: receipt.id, size: data.length };
}

function hashBeastCreatorPct(identifier, fallback) {
  const configured = config.hashbeasts_config.creators?.find(
    (creator) => creator.identifier === identifier
  );
  return configured?.percentage ?? fallback;
}

function buildHashBeastCollectionMetadata() {
  const collectionImage = config.hashbeasts.collection_image;
  if (!collectionImage) {
    throw new Error("config.hashbeasts.collection_image is required");
  }
  new URL(collectionImage);

  const inventorySweepShare = hashBeastCreatorPct("inventory_sweep_vault", 50);
  const multisigShare = hashBeastCreatorPct("multisig_fee_recipient", 50);
  if (inventorySweepShare + multisigShare !== 100) {
    throw new Error(
      `HashBeast creator shares must total 100, got ${
        inventorySweepShare + multisigShare
      }`
    );
  }

  return {
    name: config.hashbeasts.collection_name,
    symbol: config.hashbeasts.collection_symbol,
    description: config.hashbeasts.collection_description,
    seller_fee_basis_points: config.hashbeasts_config.royalties,
    image: collectionImage,
    external_url: config.token.external_url,
    collection: {
      name: config.hashbeasts.collection_name,
      family: "MineBTC",
    },
    attributes: [],
    properties: {
      category: "image",
      files: [
        {
          uri: collectionImage,
          type: "image/png",
        },
      ],
      creators: [
        {
          address: inventorySweepVaultAddress().toString(),
          share: inventorySweepShare,
        },
        {
          address: FEE_RECIPIENT_MULTISIG_STR,
          share: multisigShare,
        },
      ],
    },
  };
}

function inventorySweepVaultAddress() {
  if (!ID_MineBTC_PROGRAM) {
    throw new Error(
      `MINE_BTC_PROGRAM_ID missing in ${deploymentPath}; deploy MineBTC before deriving inventory_sweep_vault`
    );
  }
  const [inventorySweepVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("inventory-sweep-vault")],
    ID_MineBTC_PROGRAM
  );
  return inventorySweepVaultPDA;
}

function assertMainnetSafety() {
  if (CLUSTER !== "mainnet") {
    return;
  }

  if (/devnet/i.test(RPC_URL)) {
    throw new Error(`Mainnet config is using a devnet RPC URL: ${RPC_URL}`);
  }

  const deployerPath = config.deployment?.paths?.deployer_key || "";
  if (/devnet/i.test(deployerPath)) {
    throw new Error(
      `Mainnet config is using a devnet deployer key path: ${deployerPath}. Use ../mainnet-wallet-keypair.json or the intended mainnet deployer.`
    );
  }
}

function assertDeploymentFileCompatibility() {
  const createdCollection = deploymentFile.hashbeast_collection_created;
  if (
    createdCollection?.collection_name &&
    createdCollection.collection_name !== config.hashbeasts.collection_name
  ) {
    throw new Error(
      `Deployment file has stale HashBeast collection name "${createdCollection.collection_name}" but config expects "${config.hashbeasts.collection_name}". Move/reset ${deploymentPath} before continuing.`
    );
  }

  const seededBreeding = deploymentFile.breeding_config_seeded;
  const configuredBreedPrices =
    config.hashbeasts_config.breed_parent_prices_lamports || [];
  if (seededBreeding?.breed_parent_prices_lamports) {
    const existing = JSON.stringify(
      seededBreeding.breed_parent_prices_lamports.map((value) =>
        value.toString()
      )
    );
    const expected = JSON.stringify(
      configuredBreedPrices.map((value) => value.toString())
    );
    if (existing !== expected) {
      throw new Error(
        `Deployment file has stale breeding parent prices ${existing}; config expects ${expected}. Move/reset ${deploymentPath} before continuing.`
      );
    }
  }
}

async function validateInitializationConfig() {
  console.log(COLOR_STEP, "\n================ [ VALIDATING INIT CONFIG ] ================");
  assertMainnetSafety();
  assertDeploymentFileCompatibility();

  const tokenMetadataUri =
    config.token.metadata_uri || config.token.uri || config.token.image;
  new URL(tokenMetadataUri);
  new URL(config.hashbeasts.collection_image);
  await fetchJsonMetadata(tokenMetadataUri, "Token", ["name", "symbol", "image"]);
  await validateFetchableUrl(config.hashbeasts.collection_image, "HashBeast collection image source");

  if (deploymentFile.hashbeast_collection_created?.collection_uri) {
    const hashbeastCollectionMetadata = await fetchJsonMetadata(
      deploymentFile.hashbeast_collection_created.collection_uri,
      "HashBeast collection",
      ["name", "image"]
    );
    if (hashbeastCollectionMetadata.name !== config.hashbeasts.collection_name) {
      throw new Error(
        `HashBeast collection metadata name mismatch: config has "${config.hashbeasts.collection_name}", uri returns "${hashbeastCollectionMetadata.name}"`
      );
    }
    if (
      config.hashbeasts.collection_symbol &&
      hashbeastCollectionMetadata.symbol !== config.hashbeasts.collection_symbol
    ) {
      throw new Error(
        `HashBeast collection metadata symbol mismatch: config has "${config.hashbeasts.collection_symbol}", uri returns "${hashbeastCollectionMetadata.symbol}"`
      );
    }
  }

  const hashbeastsCfg = config.hashbeasts_config;
  const expectedPerFactionCap = Math.floor(
    hashbeastsCfg.genesis_mint_limit / config.factions.length
  );
  if (hashbeastsCfg.max_genesis_mints_per_faction !== expectedPerFactionCap) {
    throw new Error(
      `max_genesis_mints_per_faction should be ${expectedPerFactionCap} for ${config.factions.length} factions and ${hashbeastsCfg.genesis_mint_limit} genesis mints`
    );
  }

  const minebtcDistTotal =
    LIVE_FEE_CONFIG.newMinebtcStakersPct +
    LIVE_FEE_CONFIG.newMinebtcWinnersPct +
    LIVE_FEE_CONFIG.newMinebtcSameFactionPct * 2 +
    LIVE_FEE_CONFIG.newMinebtcJackpotPct;
  if (minebtcDistTotal !== 100) {
    throw new Error(`MineBTC round dist must equal 100%, got ${minebtcDistTotal}%`);
  }

  const gameplayRewardTotal =
    GAMEPLAY_TUNING_CONFIG.factionWarBaseRewardBps +
    GAMEPLAY_TUNING_CONFIG.factionWarMvpRewardBps +
    GAMEPLAY_TUNING_CONFIG.factionWarHashBeastRewardBps;
  if (gameplayRewardTotal !== 10_000) {
    throw new Error(`Faction-war reward bps (base+mvp+hashbeast) must equal 10000, got ${gameplayRewardTotal}`);
  }

  if (config.hashpower.base_multiplier !== 100 || config.hashpower.max_multiplier !== 300) {
    throw new Error("Hashpower config should be base=100 and max=300 for 1x..3x lockup multiplier");
  }

  if (!GAME_CONFIG.roundsEnabledAtLaunch && GAMEPLAY_TUNING_CONFIG.enableRpgProgression) {
    throw new Error(
      "Mainnet launch config disables 60s rounds but leaves HashBeast RPG locking enabled. Set gameplay_tuning.enable_rpg_progression=false for the intended launch posture."
    );
  }

  console.log(COLOR_SUCCESS, "✅ Init config validated");
}

function printFeeEconomicsSummary() {
  console.log(COLOR_STEP, "\n================ [ USER / BOT FEE ECONOMICS ] ================");
  const protocolFeePct = LIVE_FEE_CONFIG.newProtocolFeePct;
  const stakerFeePctOfBet =
    (LIVE_FEE_CONFIG.newProtocolFeePct * LIVE_FEE_CONFIG.newStakersPct) / 100;
  const treasuryFeePctOfBet = protocolFeePct - stakerFeePctOfBet;
  const cycleSolSplitPct = LIVE_FEE_CONFIG.newCycleSolSplitPct;
  const prizePotPctOfBet = 100 - protocolFeePct - cycleSolSplitPct;
  console.log(
    COLOR_INFO,
    `Manual SOL bets: ${prizePotPctOfBet}% to 60s prize pot, ${cycleSolSplitPct}% to 4h faction-war SOL pool, ${stakerFeePctOfBet}% to SOL stakers, ${treasuryFeePctOfBet}% to protocol treasury`
  );
  console.log(
    COLOR_INFO,
    "Autominer keeper compensation: 0.1% of sol_per_round, capped at 0.00005 SOL, deducted before splitting bets"
  );
  console.log(
    COLOR_INFO,
    "Ticket autominers: frontend sends sol_per_round=0; contract reserves 0.00005 SOL per ticket round for keeper gas"
  );
  for (const solPerRound of [0.1, 0.5, 1, 5, 10]) {
    const lamports = Math.round(solPerRound * 1e9);
    const keeper = Math.min(Math.floor(lamports / 1000), 50_000);
    const betBudget = lamports - keeper;
    console.log(
      COLOR_DIM,
      `   ${solPerRound} SOL/round -> keeper ${(keeper / 1e9).toFixed(6)} SOL, bet budget ${(betBudget / 1e9).toFixed(6)} SOL`
    );
  }
  console.log(
    COLOR_INFO,
    "Reward claims: no protocol SOL fee; claimant pays tx fee, and closed bet-account rent is returned to the caller."
  );
  console.log(
    COLOR_INFO,
    `MineBTC staking withdrawals: ${LIVE_FEE_CONFIG.newHodlTaxPct}% HODL tax only when there are remaining HODL pool participants; otherwise 0%.`
  );
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

function expectedMiningDepositAmount() {
  const mintedAmount =
    deploymentFile.initial_supply_minted?.actual_minted_amount ??
    deploymentFile.initial_supply_minted?.amount;
  const poolSeedAmount =
    deploymentFile.dbtc_sol_pool_created?.initialDbtcAmount ??
    (config.raydium?.initial_dbtc_amount !== undefined
      ? (BigInt(config.raydium.initial_dbtc_amount) *
          10n ** BigInt(config.token.decimals)).toString()
      : null);

  if (!mintedAmount || !poolSeedAmount) {
    return null;
  }

  return BigInt(mintedAmount) - BigInt(poolSeedAmount);
}

function calculateTokenTransferFee(amountBaseUnits) {
  const bps = BigInt(config.token.transfer_tax_bps ?? 0);
  const maxFee = BigInt(config.token.max_transfer_fee_amount ?? 0);
  if (bps === 0n || maxFee === 0n) {
    return 0n;
  }

  const fee = (amountBaseUnits * bps) / 10_000n;
  return fee > maxFee ? maxFee : fee;
}

function formatBaseUnits(amountBaseUnits, decimals) {
  const amount = BigInt(amountBaseUnits);
  const scale = 10n ** BigInt(decimals);
  const whole = amount / scale;
  const fraction = amount % scale;

  if (fraction === 0n) {
    return whole.toLocaleString("en-US");
  }

  return `${whole.toLocaleString("en-US")}.${fraction
    .toString()
    .padStart(decimals, "0")
    .replace(/0+$/, "")}`;
}

// Epoch / index / oracle scaffolding was removed when the contract moved to
// gameplay-score-driven faction-war cycles. The faction-war system has no initial
// scores, no question hash, and no oracle authority — settlement is driven entirely
// by on-chain gameplay scores and the LP-burn cycle count. No helper needed.

// ==================== [ MAIN SCRIPT ] ====================

async function main() {
  console.log(
    COLOR_STEP,
    "🚀 ================================ MineBTC Faction Surge Initialization ================================"
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
  await validateInitializationConfig();
  printFeeEconomicsSummary();

    // Verify prerequisites
  if (!DEGENBTC_TOKEN_MINT) {
    console.error(
      COLOR_ERROR,
      "❌ DEGEN_BTC token mint address not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 1_init_degenbtc_token.js first.");
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
    "🪙 DEGEN_BTC Token Mint:",
    DEGENBTC_TOKEN_MINT.toString()
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
    // Creates 5 PDAs in one tx:
    //   - GlobalConfig     [seeds: "global-config"]           — stores authority, fee config, factions
    //   - DegenBtcMining    [seeds: "mine-btc-mining"]         — mining emission state
    //   - HodlPool [seeds: "hodl-pool"]       — HODL pool — pending degenBTC claims pool with HODL tax redistribution
    //   - SOL Treasury     [seeds: "sol-treasury"]            — 0-byte system PDA for protocol SOL
    //   - Autominer Custody[seeds: "autominer-custody"]       — 0-byte system PDA for autominer SOL
    // Params: fee_recipient (Pubkey) — initial fee recipient address
    await initializeMinebtcProgram(minebtcProgram);

    // 2. Set Raydium Pool State
    // Instruction: set_raydium_pool_state(raydium_pool_state: Pubkey)
    // Stores the authorized Raydium CPMM pool address in GlobalConfig for price discovery.
    // Also init_if_needed two SOL vault PDAs:
    //   - SOL Rewards Vault  [seeds: "staker-sol-reward-vault"] — holds SOL for staker distribution
    //   - SOL Prize Pot Vault[seeds: "jackpot-pot"]           — holds SOL for round prize pots
    // Accounts: globalConfig, solRewardsVault, solPrizePotVault, authority, systemProgram
    await setRaydiumPoolState(minebtcProgram);

    // 3. Add Factions (12 factions)
    // Instruction: add_faction(faction_name: String, faction_id: u8)
    // Creates a FactionState PDA per faction [seeds: "faction", faction_name.as_bytes()]
    // Each stores: bump, faction_id, staking indexes, bet/win totals, jackpot pot (global)
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
    //   new_dbtc_stakers_pct: Option<u8>,         — % of mined MineBTC going to stakers
    //   new_dbtc_winners_pct: Option<u8>,         — % of mined MineBTC going to round winners
    //   new_dbtc_same_faction_pct: Option<u8>,    — per-losing-direction % of mined MineBTC going to winning-country non-exact bettors
    //   new_dbtc_jackpot_pct: Option<u8>,      — % of mined MineBTC going to global jackpot pot
    //   new_hodl_tax_pct: Option<u8>,                — % HODL tax charged on degenBTC withdrawal (paper hands → diamond hands)
    //   snapshot_interval: Option<u64>,              — min seconds between price snapshots
    // )
    // Accounts: globalConfig, authority
    // await updateFees(minebtcProgram, LIVE_FEE_CONFIG);

    // console.log("\n✅ First 5 init functions completed. Continuing with remaining init functions...");

    // 5. Initialize Mining System (Token Vault + Mining Parameters)
    // Instruction: initialize_mining(dbtc_per_round: u64, pool_state: Pubkey)
    // Sets up the mining emission vault:
    //   - VaultAuthority [seeds: "degenBTC-vault-authority"] — signer-only PDA
    //   - TokenVault     [seeds: "dbtc_vault", dbtc_mining.key()] — Token-2022 vault for MineBTC
    // Stores emission rate and Raydium pool state in DegenBtcMining
    // Accounts: globalConfig, dbtcMining, vaultAuthority, tokenVault, tokenMint, tokenProgram(T22), authority, systemProgram, rent
    await initializeMiningSystem(minebtcProgram);

    // 5.1. Update emission controller params
    // Instruction: update_emission_params(price_change_threshold, emission_increase_pct, emission_decrease_pct)
    // Stores explicit live-cycle rate adjustment settings on DegenBtcMining so
    // fresh deployments don't silently rely on compile-time defaults.
    // Accounts: dbtcMining, globalConfig, authority, systemProgram
    await updateEmissionParams(minebtcProgram, EMISSION_CONFIG);

    // 6. Deposit Mining Tokens
    // Instruction: deposit_dbtc_tokens(amount: u64)
    // Transfers MineBTC from depositor's Token-2022 ATA to the mining vault
    // Accounts: depositor, depositorTokenAccount, dbtcTokenVault, dbtcMining, tokenMint, tokenProgram(T22)
    await depositMiningTokens(minebtcProgram);

    // 7. Initialize Hashpower Config
    // Instruction: initialize_hashpower_config(min_lockup_days: u64, max_lockup_days: u64, base_multiplier: u16, max_multiplier: u16)
    // Creates HashpowerConfig PDA [seeds: "hashpower-config"] with lockup duration.
    // Lockup can add up to 3x; passive HashBeast staking can add up to 3x, so max staking boost is 9x.
    // Accounts: globalConfig, hashpowerConfig, authority, systemProgram
    await initializeHashpowerConfig(minebtcProgram);

    // 8. Initialize Custodian Accounts (DBTC and Liquidity custodians)
    // Instruction: initialize_custodian_accounts() — no args
    // Creates 4 PDAs:
    //   - minebtcCustodian           [seeds: "degenBTC-custodian"]           — Token-2022 account for staked dBTC
    //   - minebtcCustodianAuthority  [seeds: "degenBTC-custodian-authority"] — signer PDA for dBTC custodian
    //   - liquidityCustodian         [seeds: "lp-custodian"]                — SPL Token account for staked LP tokens
    //   - liquidityCustodianAuthority[seeds: "lp-custodian-authority"]      — signer PDA for LP custodian
    // Accounts: globalConfig, degenbtcMint, minebtcCustodian, minebtcCustodianAuthority,
    //           lpMint, liquidityCustodian, liquidityCustodianAuthority, authority,
    //           systemProgram, token2022Program, tokenProgram, rent
    await initializeCustodianAccounts(minebtcProgram);

    // 9. Initialize HashBeastConfig
    // Instruction: initialize_hashbeast_config()  (no params — no lifetime supply cap)
    // Creates HashBeastConfig PDA [seeds: "hashbeast-config"] with collection + breeding state.
    // Accounts: hashbeastsConfig, globalConfig, authority, systemProgram
    await initializeHashBeastConfig(minebtcProgram);

    // 9a. Seed breeding config. Breeding stays disabled at launch; parent
    //     breed-count prices are stored on HashBeastConfig for later tuning.
    // Instruction: update_breeding_config(breeding_allowed, breed_parent_prices_lamports)
    // Accounts: globalConfig, hashbeastsConfig, authority, systemProgram
    await seedBreedingConfig(minebtcProgram);

    // 9b. Initialize HashBeastMintConfig
    // Instruction: initialize_hashbeast_mint_config(base_price, curve_a, genesis_mint_limit, max_genesis_mints_per_faction)
    // Creates mint-only PDA [seeds: "hashbeast-mint-config"] for genesis sale curve, ticket tiers, and per-country caps.
    // Accounts: hashbeastMintConfig, globalConfig, authority, systemProgram
    await initializeHashBeastMintConfig(minebtcProgram);

    // 10. Create HashBeast Collection (Metaplex Core)
    // Instruction: create_hashbeast_collection(name: String, uri: String)
    // Creates a Metaplex Core NFT collection with PDA as update authority
    // CollectionAuthority PDA [seeds: "collection_authority"] becomes the update authority
    // Accounts: authority, globalConfig, hashbeastsConfig, collection (signer keypair),
    //           collectionAuthority, mplCoreProgram, systemProgram
    await createHashBeastCollection(minebtcProgram);

    // 11. Initialize HashBeast Royalties
    // Instruction: init_hashbeast_royalties(basis_points: u16, creators: Vec<CreatorInput>)
    // Sets royalty config on the Metaplex Core collection (e.g. 5% split between multisig + treasury)
    // Accounts: authority, globalConfig, hashbeastsConfig, collection, collectionAuthority, mplCoreProgram, systemProgram
    await initializeHashBeastRoyalties(minebtcProgram);

    // 12. Configure Ticket Tiers (for HashBeast minting)
    // Instruction: add_ticket_tier_config(ticket_tier_index: u8, ticket_value: u64)
    // Adds/updates a ticket tier in HashBeastMintConfig (max 3 tiers)
    // Accounts: globalConfig, hashbeastMintConfig, authority, systemProgram
    await configureTicketTiers(minebtcProgram);

    // 13. Initialize Tax Config (for tax distribution)
    // Instruction: initialize_tax_config(treasury_pct: u8, burn_pct: u8)
    // Creates TaxConfig PDA [seeds: "tax-config"] and the two surviving vaults:
    //   - WithdrawWithheldAuthority [seeds: "withdraw-withheld-authority"] — 0-byte signer PDA
    //   - FactionTreasuryVault      [seeds: "faction-treasury-vault"]      — Token-2022 vault
    // (NFT floor sweep vault + sale SOL vault were removed; NFT market making is
    // now SOL-funded via SolFeeConfig::nft_market_making_pct.)
    // Accounts: globalConfig, taxConfig, degenbtcMint, withdrawWithheldAuthority,
    //           factionTreasuryVault, authority, tokenProgram2022, systemProgram
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
    return;

    // 9c. Enable HashBeast minting (default is inactive after init)
    // Instruction: switch_hashbeast_mining() — toggles is_active to true
    // Accounts: hashbeastMintConfig, globalConfig, authority
    // await enableHashBeastMining(minebtcProgram);


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








    // return;

    // NOTE: Cranker bot whitelist was removed when keepers became fully
    // permissionless. start_round / end_round / settle_war / claim crank
    // instructions are callable by any wallet — the protocol only pays the
    // (capped) keeper compensation to the caller that lands the tx first.

    // 17. Initialize Faction War Config (mutation-driven competitive cycles)
    // Instruction: initialize_war_config() — no args
    // Creates FactionWarConfig PDA [seeds: "faction-war-config"] with
    // current_war_id=1, settle_at_lp_op_count=0, and identity
    // start ranks [0..NUM_FACTIONS). Each cycle's FactionWarState is then
    // initialized by a keeper before that cycle's round settlements are folded in.
    // Accounts: warConfig, globalConfig, authority, systemProgram
    await initializeFactionWarConfig(minebtcProgram);

    // 18. Legacy faction-war active toggle
    // The old update_war_config(is_active) instruction was removed. Cycles are
    // live once war_config exists; this no-op keeps older runbooks readable.
    await updateFactionWarConfig(minebtcProgram, FACTION_WAR_CONFIG);

    // 19. Update unified gameplay tuning
    // Instruction: update_gameplay_tuning(args)
    // Sets the live mutation engine + cycle reward split in one payload:
    //   - enable RPG progression
    //   - evolution unlock stage
    //   - cycle reward split (base / MVP / hashbeast)
    //   - mutation chance bounds
    //   - volume gates
    //   - pacing controls
    // Accounts: globalConfig, authority
    await updateGameplayTuning(minebtcProgram, GAMEPLAY_TUNING_CONFIG);

    // 20. Initialize DegenBTC Marketplace (standalone program — reborn NFT
    //     listings + P2P trades with 3% fee). Idempotent: skips if config already
    //     exists on-chain.
    // Instruction: degenbtc_market::initialize_marketplace(fee_bps, fee_recipient,
    //     min_price_lamports, mpl_core_program)
    // PDAs: marketplace_config [seeds: "marketplace-config", collection_mint]
    await initializeDegenBtcMarketplace(minebtcProgram);

    // 21. Initialize Inventory Pool + Floor Queue + Sale History + Floor
    //     History + Sweep Vault inside the mineBTC program. Caches the
    //     marketplace program + config pubkeys so CPI helpers can validate
    //     them. The system is fully permissionless — no crank authority.
    // Instruction: mineBTC::init_inventory_pool(marketplace_program, marketplace_config)
    // PDAs: inventory_pool ["inventory-pool"], floor_queue ["floor-queue"],
    //       sale_history ["sale-history"], floor_history ["floor-history"],
    //       inventory_sweep_vault ["inventory-sweep-vault"]
    await initializeInventoryPool(minebtcProgram);

    // 22. Initialize per-country LootboxQueue PDAs. One PDA per active
    //     faction. rebirth_hashbeast / sweep_floor_lowest push assets into
    //     the matching country's queue; losing players roll for slots from
    //     it during round-claim.
    // Instruction: mineBTC::init_lootbox_queue(faction_id)
    // PDAs: lootbox_queue [seeds: "lootbox-queue", faction_id]
    await initializeLootboxQueues(minebtcProgram);

    // Print completion summary
    printCompletionSummary();
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

  const [hodlPoolPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hodl-pool")],
    minebtcProgram.programId
  );

  const [autominerCustodyPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("autominer-custody")],
    minebtcProgram.programId
  );

  const FEE_RECIPIENT_MULTISIG = new PublicKey(
    FEE_RECIPIENT_MULTISIG_STR
  );

  console.log(
    COLOR_INFO,
    `🔑 Global Config PDA: ${globalConfigPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `🔑 DegenBtc Mining PDA: ${mineBtcMiningPDA.toString()}`
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
        dbtcMining: mineBtcMiningPDA,
        hodlPool: hodlPoolPDA,
                solTreasury: solTreasuryPDA,
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
      autominerCustody_address: autominerCustodyPDA.toString(),
      hodlPool_address: hodlPoolPDA.toString(),
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
        hodlPool: hodlPoolPDA.toString(),
                solTreasury_address: solTreasuryPDA.toString(),
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
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_degenbtc_SOL_pool.js first.");
        return;
    }

    const [vaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("dbtc_vault"), mineBtcMiningPDA.toBuffer()],
    minebtcProgram.programId
    );

    const [vaultAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("degenBTC-vault-authority")],
    minebtcProgram.programId
    );

    console.log(COLOR_INFO, `🔑 Mining Token Vault PDA: ${vaultPDA.toString()}`);
  console.log(
    COLOR_INFO,
    `🔑 Vault Authority PDA: ${vaultAuthorityPDA.toString()}`
  );
  console.log(
    COLOR_INFO,
    `💰 DegenBtc Per Slot: ${MINING_DEGEN_BTC_PER_SLOT.toString()}`
  );
    console.log(COLOR_INFO, `🔄 Raydium Pool State: ${raydiumPoolState}`);

    try {
    const tx = await minebtcProgram.methods
            .initializeMining(
                MINING_DEGEN_BTC_PER_SLOT,
                new PublicKey(raydiumPoolState)
            )
            .accounts({
                globalConfig: globalConfigPDA,
        dbtcMining: mineBtcMiningPDA,
                vaultAuthority: vaultAuthorityPDA,
                tokenVault: vaultPDA,
        tokenMint: DEGENBTC_TOKEN_MINT,
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
            degen_btc_per_round: MINING_DEGEN_BTC_PER_SLOT.toString(),
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
                degen_btc_per_round: MINING_DEGEN_BTC_PER_SLOT.toString(),
            };
            saveDeploymentData()
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
    DEGENBTC_TOKEN_MINT,
        wallet.publicKey,
        false,
        anchor_spl.TOKEN_2022_PROGRAM_ID
    );

  // Deposit whatever degenBTC is left in the deployer wallet after the
  // configured launch-pool seed. This should be the full supply minus the
  // pool's explicit initial dBTC amount.
  const depositorAcc = await anchor_spl.getAccount(
    connection,
    userTokenAccount,
    undefined,
    anchor_spl.TOKEN_2022_PROGRAM_ID
  );
  const depositAmount = new BN(depositorAcc.amount.toString());
  const expectedDepositAmount = expectedMiningDepositAmount();

  if (depositAmount.isZero()) {
    throw new Error(
      `Depositor token account ${userTokenAccount.toString()} has 0 degenBTC — pool/LP setup likely consumed the full mint.`
    );
  }
  if (expectedDepositAmount && depositAmount.toString() !== expectedDepositAmount.toString()) {
    throw new Error(
      `Unexpected deployer dBTC balance. Expected ${formatBaseUnits(expectedDepositAmount, config.token.decimals)} dBTC (${expectedDepositAmount.toString()} base units) after pool seed, found ${formatBaseUnits(BigInt(depositAmount.toString()), config.token.decimals)} dBTC (${depositAmount.toString()} base units).`
    );
  }

  const depositAmountBigInt = BigInt(depositAmount.toString());
  const expectedTransferFee = calculateTokenTransferFee(depositAmountBigInt);
  const expectedVaultCredit = depositAmountBigInt - expectedTransferFee;

  console.log(
    COLOR_INFO,
    `💰 Depositing ${formatBaseUnits(depositAmountBigInt, config.token.decimals)} dBTC (${depositAmount.toString()} base units, full deployer balance)...`
  );
  if (expectedTransferFee > 0n) {
    console.log(
      COLOR_DIM,
      `   Token-2022 transfer fee: ${formatBaseUnits(expectedTransferFee, config.token.decimals)} dBTC; expected vault credit before tax harvest: ${formatBaseUnits(expectedVaultCredit, config.token.decimals)} dBTC`
    );
  }
    console.log(COLOR_INFO, `   From: ${userTokenAccount.toString()}`);
    console.log(COLOR_INFO, `   To: ${vaultPDA.toString()}`);

    try {
    const tx = await minebtcProgram.methods
      .depositDbtcTokens(depositAmount)
            .accounts({
                depositor: wallet.publicKey,
                depositorTokenAccount: userTokenAccount,
                dbtcTokenVault: vaultPDA,
        dbtcMining: mineBtcMiningPDA,
        tokenMint: DEGENBTC_TOKEN_MINT,
                tokenProgram: anchor_spl.TOKEN_2022_PROGRAM_ID,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Mining tokens deposited successfully!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.mining_tokens_deposited = {
            amount: depositAmount.toString(),
            amount_readable: `${formatBaseUnits(depositAmountBigInt, config.token.decimals)} dBTC`,
            expected_transfer_fee: expectedTransferFee.toString(),
            expected_vault_credit_after_transfer_fee: expectedVaultCredit.toString(),
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
  if (!DEGENBTC_TOKEN_MINT) {
    console.error(
      COLOR_ERROR,
      "❌ DEGEN_BTC token mint address not found in deployment file."
    );
    throw new Error(
      "DEGEN_BTC mint address required for custodian initialization"
    );
  }

  if (!deploymentFile.dbtc_sol_pool_created?.lpMintPDA) {
    console.error(
      COLOR_ERROR,
      "❌ LP mint address not found in deployment file."
    );
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_degenbtc_SOL_pool.js first.");
    throw new Error("LP mint address required for custodian initialization");
  }

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const minebtcMint = DEGENBTC_TOKEN_MINT;
  const lpMint = new PublicKey(deploymentFile.dbtc_sol_pool_created.lpMintPDA);

  // Derive DBTC custodian PDAs
  const [minebtcCustodianPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("degenBTC-custodian")],
    minebtcProgram.programId
  );

  const [minebtcCustodianAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("degenBTC-custodian-authority")],
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
        degenbtcMint: minebtcMint,
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
    console.log(COLOR_WARNING, "⚠️ Please run 2_init_degenbtc_SOL_pool.js first.");
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
    [Buffer.from("jackpot-pot")],
    minebtcProgram.programId
  );
  const [factionWarSolVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("faction-war-sol-vault")],
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
        warSolVault: factionWarSolVaultPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ Raydium pool state set successfully!");
    console.log(COLOR_SUCCESS, "✅ SOL rewards vault initialized!");
    console.log(COLOR_SUCCESS, "✅ SOL prize pot vault initialized!");
    console.log(COLOR_SUCCESS, "✅ Faction-war SOL vault derived!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.raydium_pool_state_set = {
            pool_state_address: poolStatePubkey.toString(),
      sol_rewards_vault: solRewardsVaultPDA.toString(),
      sol_prize_pot_vault: solPrizePotVaultPDA.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
    deploymentFile.war_sol_vault_pda = factionWarSolVaultPDA.toString();
        saveDeploymentData();
    } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to set Raydium pool state:", error);
        throw error;
    }
}

async function initializeHashBeastConfig(minebtcProgram) {
    if (deploymentFile.hashbeast_config_initialized) {
    console.log(COLOR_INFO, "ℹ️ HashBeastConfig already initialized. Skipping...");
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING HASHBEAST CONFIG ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

    const [hashbeastsConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-config")],
    minebtcProgram.programId
    );

    console.log(COLOR_INFO, `🔑 HashBeastConfig PDA: ${hashbeastsConfigPDA.toString()}`);
    console.log(
      COLOR_INFO,
      `🥚 No lifetime supply cap — only the genesis sale is bounded (HashBeastMintConfig)`
    );

    try {
    const tx = await minebtcProgram.methods
      .initializeHashbeastConfig()
            .accounts({
                hashbeastsConfig: hashbeastsConfigPDA,
                globalConfig: globalConfigPDA,
                authority: wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ HashBeastConfig initialized successfully!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.hashbeast_config_initialized = {
            hashbeasts_config_pda: hashbeastsConfigPDA.toString(),
            tx_signature: tx,
      timestamp: new Date().toISOString(),
        };
        saveDeploymentData();
    } catch (error) {
        if (error.toString().includes("already in use")) {
      console.log(COLOR_INFO, "ℹ️ HashBeastConfig already initialized. Skipping...");
            deploymentFile.hashbeast_config_initialized = {
                hashbeasts_config_pda: hashbeastsConfigPDA.toString(),
            };
            saveDeploymentData();
        } else {
      console.error(COLOR_ERROR, "❌ Failed to initialize HashBeastConfig:", error);
            throw error;
        }
    }
}

async function seedBreedingConfig(minebtcProgram) {
  if (deploymentFile.breeding_config_seeded) {
    console.log(COLOR_INFO, "ℹ️ Breeding config already seeded. Skipping...");
    return;
  }

  console.log(
    COLOR_STEP,
    "\n=================== [ SEEDING BREEDING CONFIG ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const [hashbeastsConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-config")],
    minebtcProgram.programId
  );

  const breedingAllowed = !!config.hashbeasts_config.breeding_allowed;
  const breedParentPrices = config.hashbeasts_config.breed_parent_prices_lamports || [];
  if (breedParentPrices.length !== 5) {
    throw new Error(
      `config.hashbeasts_config.breed_parent_prices_lamports must have exactly 5 entries, got ${breedParentPrices.length}`
    );
  }

  console.log(COLOR_INFO, `🐣 Breeding allowed: ${breedingAllowed}`);
  console.log(
    COLOR_INFO,
    `   parent price table: ${breedParentPrices
      .map((price, idx) => `${idx}:${price / 1e9} SOL`)
      .join(", ")}`
  );
  console.log(COLOR_INFO, "   floor guard: 1.5x current floor anchor");

  try {
    const tx = await minebtcProgram.methods
      .updateBreedingConfig(
        breedingAllowed,
        breedParentPrices.map((price) => new BN(price))
      )
      .accounts({
        globalConfig: globalConfigPDA,
        hashbeastsConfig: hashbeastsConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, "✅ Breeding config seeded successfully!");
    console.log(COLOR_DIM, `   Transaction: ${tx}`);

    deploymentFile.breeding_config_seeded = {
      breeding_allowed: breedingAllowed,
      breed_parent_prices_lamports: breedParentPrices.map((price) =>
        price.toString()
      ),
      floor_multiplier_bps: 15000,
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    console.error(COLOR_ERROR, "❌ Failed to seed breeding config:", error);
    throw error;
  }
}

async function initializeHashBeastMintConfig(minebtcProgram) {
  if (deploymentFile.hashbeast_mint_config_initialized) {
    console.log(COLOR_INFO, "ℹ️ HashBeastMintConfig already initialized. Skipping...");
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING HASHBEAST MINT CONFIG ] ================"
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );

  const [hashbeastMintConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-mint-config")],
    minebtcProgram.programId
  );

  const basePrice = config.hashbeasts_config.base_price;
  const curveA = config.hashbeasts_config.curve_a;
  const genesisMintLimit = config.hashbeasts_config.genesis_mint_limit;
  const maxGenesisMintsPerFaction =
    config.hashbeasts_config.max_genesis_mints_per_faction ?? 1000;

  if (!basePrice || !curveA || !genesisMintLimit || !maxGenesisMintsPerFaction) {
    console.error(COLOR_ERROR, "❌ HashBeast mint config values not found in config.json");
    throw new Error("HashBeast mint config values not found");
  }

  console.log(COLOR_INFO, `🔑 HashBeastMintConfig PDA: ${hashbeastMintConfigPDA.toString()}`);
  console.log(COLOR_INFO, `💰 Genesis Base Price: ${basePrice / 1e9} SOL`);
  console.log(COLOR_INFO, `📈 Genesis Curve A: ${curveA}`);
  console.log(COLOR_INFO, `🥚 Genesis Mint Limit: ${genesisMintLimit}`);
  console.log(COLOR_INFO, `🏁 Per-country Genesis Cap: ${maxGenesisMintsPerFaction}`);

  try {
    const tx = await minebtcProgram.methods
      .initializeHashbeastMintConfig(
        new BN(basePrice),
        new BN(curveA),
        new BN(genesisMintLimit),
        maxGenesisMintsPerFaction
      )
      .accounts({
        hashbeastMintConfig: hashbeastMintConfigPDA,
        globalConfig: globalConfigPDA,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, "✅ HashBeastMintConfig initialized successfully!");
    console.log(COLOR_DIM, `   Transaction: ${tx}`);

    deploymentFile.hashbeast_mint_config_initialized = {
      hashbeast_mint_config_pda: hashbeastMintConfigPDA.toString(),
      base_price: basePrice.toString(),
      curve_a: curveA.toString(),
      genesis_mint_limit: genesisMintLimit.toString(),
      max_genesis_mints_per_faction: maxGenesisMintsPerFaction.toString(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log(COLOR_INFO, "ℹ️ HashBeastMintConfig already initialized. Skipping...");
      deploymentFile.hashbeast_mint_config_initialized = {
        hashbeast_mint_config_pda: hashbeastMintConfigPDA.toString(),
      };
      saveDeploymentData();
    } else {
      console.error(COLOR_ERROR, "❌ Failed to initialize HashBeastMintConfig:", error);
      throw error;
    }
  }
}

async function enableHashBeastMining(minebtcProgram) {
  if (deploymentFile.hashbeast_mining_enabled) {
    console.log(COLOR_INFO, "ℹ️ HashBeast mining already enabled");
    return;
  }

  console.log(COLOR_INFO, "🔄 Enabling HashBeast mining...");
  try {
    const provider = minebtcProgram.provider;
    const authority = provider.wallet.publicKey;

    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("global-config")],
      minebtcProgram.programId
    );
    const [hashbeastMintConfigPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("hashbeast-mint-config")],
      minebtcProgram.programId
    );

    const tx = await minebtcProgram.methods
      .switchHashbeastMining()
      .accounts({
        hashbeastMintConfig: hashbeastMintConfigPDA,
        globalConfig: globalConfigPDA,
        authority: authority,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ HashBeast mining enabled! Tx: ${tx}`);
    deploymentFile.hashbeast_mining_enabled = true;
    saveDeploymentData();
    return tx;
  } catch (error) {
    if (error.toString().includes("already active")) {
      console.log(COLOR_INFO, "ℹ️ HashBeast mining already active");
      deploymentFile.hashbeast_mining_enabled = true;
      saveDeploymentData();
    } else {
      console.error(COLOR_ERROR, "❌ Failed to enable HashBeast mining:", error);
      throw error;
    }
  }
}

async function createHashBeastCollection(minebtcProgram) {
    if (deploymentFile.hashbeast_collection_created) {
    console.log(COLOR_INFO, "ℹ️ HashBeast collection already created");
    console.log(
      COLOR_INFO,
      "🔑 Collection Address:",
      deploymentFile.hashbeast_collection_created.collection_address
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ CREATING  HASHBEAST COLLECTION ] ==================="
  );

    // Derive PDAs
    const [globalConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-config")],
    minebtcProgram.programId
    );

    const [hashbeastsConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-config")],
    minebtcProgram.programId
    );

    const [hashbeastMintConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-mint-config")],
    minebtcProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection_authority")],
    minebtcProgram.programId
    );

  console.log(COLOR_INFO, "🎨 Creating Metaplex Core collection...");
    console.log(COLOR_DIM, `   Name: ${config.hashbeasts.collection_name}`);
    console.log(COLOR_DIM, `   Image: ${config.hashbeasts.collection_image}`);
  console.log(
    COLOR_INFO,
    "🔐 Collection Authority PDA:",
    collectionAuthorityPDA.toString()
  );

  const collectionMetadata = buildHashBeastCollectionMetadata();
  const metadataFingerprint = JSON.stringify(collectionMetadata);
  let collectionMetadataUpload = deploymentFile.hashbeast_collection_metadata_uploaded;
  if (
    !collectionMetadataUpload?.uri ||
    collectionMetadataUpload.metadata_fingerprint !== metadataFingerprint
  ) {
    collectionMetadataUpload = await uploadJsonToIrys(
      collectionMetadata,
      "HashBeast collection"
    );
    collectionMetadataUpload.metadata_fingerprint = metadataFingerprint;
    collectionMetadataUpload.metadata = collectionMetadata;
    collectionMetadataUpload.timestamp = new Date().toISOString();
    deploymentFile.hashbeast_collection_metadata_uploaded = collectionMetadataUpload;
    saveDeploymentData();
  } else {
    console.log(
      COLOR_INFO,
      `ℹ️ Reusing uploaded HashBeast collection metadata: ${collectionMetadataUpload.uri}`
    );
  }
  const collectionUri = collectionMetadataUpload.uri;
  config.hashbeasts.collection_uri = collectionUri;
  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 4)}\n`);

  console.log(COLOR_DIM, `   Final URI: ${collectionUri}`);
  console.log(
    COLOR_DIM,
    `   Creators: ${collectionMetadata.properties.creators
      .map((creator) => `${creator.address} (${creator.share}%)`)
      .join(", ")}`
  );

    // Generate a new keypair for the collection
    const collectionKeypair = Keypair.generate();

    try {
    const tx = await minebtcProgram.methods
            .createHashbeastCollection(
                config.hashbeasts.collection_name,
                collectionUri
            )
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                hashbeastsConfig: hashbeastsConfigPDA,
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
      "✅ HashBeast collection created successfully!"
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

        deploymentFile.hashbeast_collection_created = {
            collection_address: collectionPubkey.toString(),
            collection_name: config.hashbeasts.collection_name,
            collection_uri: collectionUri,
      collection_metadata_upload: collectionMetadataUpload,
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


async function initializeHashBeastRoyalties(minebtcProgram) {
    if (deploymentFile.hashbeast_royalties_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ HashBeast royalties already initialized. Skipping..."
    );
        return;
    }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING  HASHBEAST ROYALTIES ] ==================="
  );

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const collectionPubkey = new PublicKey(
    deploymentFile.hashbeast_collection_created.collection_address
  );

    const [hashbeastsConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-config")],
    minebtcProgram.programId
    );

    const [collectionAuthorityPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection_authority")],
    minebtcProgram.programId
    );

    // Configure royalties
  const basisPoints = config.hashbeasts_config.royalties;
    let creators = [];

  // Convert addresses to PublicKey objects
  const multisigAddress = new PublicKey(
    FEE_RECIPIENT_MULTISIG_STR
  );
  const inventorySweepShare = hashBeastCreatorPct("inventory_sweep_vault", 50);
  const multisigShare = hashBeastCreatorPct("multisig_fee_recipient", 50);
  if (inventorySweepShare + multisigShare !== 100) {
    throw new Error(
      `HashBeast royalty creator shares must total 100, got ${
        inventorySweepShare + multisigShare
      }`
    );
  }

  creators.push({
    address: inventorySweepVaultAddress(),
    percentage: inventorySweepShare,
  });
  creators.push({
    address: multisigAddress,
    percentage: multisigShare,
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
      .initHashbeastRoyalties(basisPoints, creators)
            .accounts({
                authority: walletKeypair.publicKey,
                globalConfig: globalConfigPDA,
                hashbeastsConfig: hashbeastsConfigPDA,
                collection: collectionPubkey,
                collectionAuthority: collectionAuthorityPDA,
        mplCoreProgram: new PublicKey(
          "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        ),
                systemProgram: SystemProgram.programId,
            })
            .rpc();

    console.log(COLOR_SUCCESS, "✅ HashBeast royalties initialized!");
        console.log(COLOR_DIM, `   Transaction: ${tx}`);

        deploymentFile.hashbeast_royalties_initialized = {
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

    const [hashbeastMintConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("hashbeast-mint-config")],
    minebtcProgram.programId
    );

  const ticketTiers = config.hashbeasts_config.ticket_tiers || [];

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
                    hashbeastMintConfig: hashbeastMintConfigPDA,
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

    // Get config values. Tax now only splits faction_treasury + burn (the
    // residual flows back to the mining vault). NFT market making is funded
    // from SOL via `distribute_sol_fees` → `inventory_sweep_vault`.
    const factionTreasuryPct = config.tax.treasury_pct;
    const burnPct = config.tax.burnt_pct;

    if (factionTreasuryPct + burnPct > 100) {
        throw new Error(
            `Tax splits must sum to ≤100 (got ${factionTreasuryPct}+${burnPct}=${factionTreasuryPct + burnPct})`
        );
    }

    console.log(COLOR_INFO, `💰 Tax Distribution:`);
    console.log(COLOR_INFO, `   Faction Treasury: ${factionTreasuryPct}%`);
    console.log(COLOR_INFO, `   Burn: ${burnPct}%`);
    console.log(COLOR_INFO, `   Back to Vault: ${100 - factionTreasuryPct - burnPct}%`);

    try {
    const tx = await minebtcProgram.methods
            .initializeTaxConfig(factionTreasuryPct, burnPct)
            .accounts({
                globalConfig: globalConfigPDA,
                taxConfig: taxConfigPDA,
                degenbtcMint: DEGENBTC_TOKEN_MINT,
                withdrawWithheldAuthority: withdrawWithheldAuthorityPDA,
                factionTreasuryVault: factionTreasuryVaultPDA,
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
            treasury_pct: factionTreasuryPct,
            burn_pct: burnPct,
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

    const roundDurationSeconds = GAME_CONFIG.roundDurationSeconds;
    const roundsEnabledAtLaunch = GAME_CONFIG.roundsEnabledAtLaunch;

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
  console.log(
    COLOR_INFO,
    `🎮 60s rounds enabled at launch: ${roundsEnabledAtLaunch}`
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
            rounds_enabled_at_launch: roundsEnabledAtLaunch,
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
                rounds_enabled_at_launch: roundsEnabledAtLaunch,
            };
            saveDeploymentData();
        } else {
      console.error(COLOR_ERROR, "❌ Failed to initialize game state:", error);
            throw error;
        }
    }

  await applyInitialRoundLaunchState(
    minebtcProgram,
    globalGameStatePDA,
    globalConfigPDA,
    roundsEnabledAtLaunch
  );
}

async function applyInitialRoundLaunchState(
  minebtcProgram,
  globalGameStatePDA,
  globalConfigPDA,
  roundsEnabledAtLaunch
) {
  const current = await minebtcProgram.account.globalGameState.fetch(
    globalGameStatePDA
  );

  if (current.isActive === roundsEnabledAtLaunch) {
    console.log(
      COLOR_INFO,
      `ℹ️ Round launch state already matches config: is_active=${current.isActive}`
    );
    deploymentFile.game_state_launch_configured = {
      global_game_state_pda: globalGameStatePDA.toString(),
      rounds_enabled_at_launch: roundsEnabledAtLaunch,
      tx_signature: null,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ APPLYING ROUND LAUNCH STATE ] ================"
  );
  console.log(
    COLOR_INFO,
    `🎮 Updating GlobalGameState.is_active ${current.isActive} → ${roundsEnabledAtLaunch}`
  );

  const tx = await minebtcProgram.methods
    .updateGameState(roundsEnabledAtLaunch, null)
    .accounts({
      globalGameState: globalGameStatePDA,
      globalConfig: globalConfigPDA,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log(COLOR_SUCCESS, "✅ Round launch state applied successfully!");
  console.log(COLOR_DIM, `   Transaction: ${tx}`);

  deploymentFile.game_state_launch_configured = {
    global_game_state_pda: globalGameStatePDA.toString(),
    rounds_enabled_at_launch: roundsEnabledAtLaunch,
    tx_signature: tx,
    timestamp: new Date().toISOString(),
  };
  saveDeploymentData();
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
        authority: wallet.publicKey,
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

    console.log(COLOR_INFO, "   Current DegenBtc dist config:");
    console.log(
      COLOR_INFO,
      `     Stakers: ${globalConfig.dbtcDistConfig.dbtcStakersPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Winners: ${globalConfig.dbtcDistConfig.dbtcWinnersPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Same-faction: ${globalConfig.dbtcDistConfig.dbtcSameFactionPct}%`
    );
    console.log(
      COLOR_INFO,
      `     Jackpot: ${globalConfig.dbtcDistConfig.dbtcJackpotPct}%`
    );
    console.log(
      COLOR_INFO,
      `     HODL tax: ${globalConfig.dbtcDistConfig.hodlTaxPct}%`
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
      newMinebtcJackpotPct: feeConfig?.newMinebtcJackpotPct ?? null,
      newHodlTaxPct: feeConfig?.newHodlTaxPct ?? null,
      snapshotInterval:
        (feeConfig?.snapshotInterval ?? feeConfig?.snapshot_interval) != null
          ? new BN(
              feeConfig?.snapshotInterval ?? feeConfig?.snapshot_interval
            )
        : null,
      newCycleSolSplitPct: feeConfig?.newCycleSolSplitPct ?? null,
      newNftMarketMakingPct: feeConfig?.newNftMarketMakingPct ?? null,
    };

    // Validate the MineBTC distribution invariant before sending the transaction.
    // The on-chain program treats same-faction % as PER LOSING DIRECTION.
    if (
      feeParams.newMinebtcStakersPct !== null ||
      feeParams.newMinebtcWinnersPct !== null ||
      feeParams.newMinebtcSameFactionPct !== null ||
      feeParams.newMinebtcJackpotPct !== null
    ) {
      const minebtcStakersPct =
        feeParams.newMinebtcStakersPct ??
        globalConfig.dbtcDistConfig.dbtcStakersPct;
      const minebtcWinnersPct =
        feeParams.newMinebtcWinnersPct ??
        globalConfig.dbtcDistConfig.dbtcWinnersPct;
      const minebtcSameFactionPct =
        feeParams.newMinebtcSameFactionPct ??
        globalConfig.dbtcDistConfig.dbtcSameFactionPct;
      const minebtcJackpotPct =
        feeParams.newMinebtcJackpotPct ??
        globalConfig.dbtcDistConfig.dbtcJackpotPct;

      const losingDirectionCount = 2; // Up / Neutral / Down => 2 losing directions
      const minebtcTotal =
        minebtcStakersPct +
        minebtcWinnersPct +
        minebtcSameFactionPct * losingDirectionCount +
        minebtcJackpotPct;

      if (minebtcTotal !== 100) {
        throw new Error(
          `Invalid MineBTC distribution config: stakers (${minebtcStakersPct}) + winners (${minebtcWinnersPct}) + ${losingDirectionCount}*sameFaction (${minebtcSameFactionPct}) + jackpot (${minebtcJackpotPct}) must equal 100, got ${minebtcTotal}.`
        );
      }
    }

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
        `     DBTC Same-faction: ${feeParams.newMinebtcSameFactionPct}% per losing direction (${feeParams.newMinebtcSameFactionPct * 2}% total)`
      );
    if (feeParams.newMinebtcJackpotPct !== null)
      console.log(
        COLOR_INFO,
        `     DBTC Jackpot: ${feeParams.newMinebtcJackpotPct}%`
      );
    if (feeParams.newHodlTaxPct !== null)
      console.log(
        COLOR_INFO,
        `     HODL tax: ${feeParams.newHodlTaxPct}%`
      );
    if (feeParams.snapshotInterval !== null)
      console.log(
        COLOR_INFO,
        `     Snapshot interval: ${feeParams.snapshotInterval.toString()} seconds`
      );
    if (feeParams.newCycleSolSplitPct !== null)
      console.log(
        COLOR_INFO,
        `     Cycle SOL split: ${feeParams.newCycleSolSplitPct}%`
      );
    if (feeParams.newNftMarketMakingPct !== null)
      console.log(
        COLOR_INFO,
        `     NFT market making: ${feeParams.newNftMarketMakingPct}% (of distribute_sol_fees → inventory_sweep_vault)`
      );

    console.log(
      COLOR_INFO,
      `   Global Config PDA: ${globalConfigPDA.toString()}`
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
        feeParams.newMinebtcJackpotPct,
        feeParams.newHodlTaxPct,
        feeParams.snapshotInterval,
        feeParams.newCycleSolSplitPct,
        feeParams.newNftMarketMakingPct
      )
      .accounts({
        globalConfig: globalConfigPDA,
        authority: wallet.publicKey,
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

async function updateEmissionParams(minebtcProgram, emissionConfig) {
  console.log(
    COLOR_STEP,
    "\n================ [ UPDATING EMISSION PARAMS ] ================"
  );

  if (!deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ MineBTC program not initialized. Skipping emission params update..."
    );
    return;
  }

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const [mineBtcMiningPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("mine-btc-mining")],
    minebtcProgram.programId
  );

  const mineBtcMining = await minebtcProgram.account.degenBtcMining.fetch(
    mineBtcMiningPDA
  );

  const target = {
    priceChangeThreshold: new BN(emissionConfig.priceChangeThresholdPct),
    emissionIncreasePct: new BN(emissionConfig.emissionIncreasePct),
    emissionDecreasePct: new BN(emissionConfig.emissionDecreasePct),
  };

  console.log(COLOR_INFO, "   Current emission params:");
  console.log(
    COLOR_INFO,
    `     Price threshold: ${mineBtcMining.priceChangeThreshold.toString()}%`
  );
  console.log(
    COLOR_INFO,
    `     Increase: ${mineBtcMining.emissionIncreasePct.toString()}%`
  );
  console.log(
    COLOR_INFO,
    `     Decrease: ${mineBtcMining.emissionDecreasePct.toString()}%`
  );
  console.log(COLOR_INFO, "   Target emission params:");
  console.log(
    COLOR_INFO,
    `     Price threshold: ${target.priceChangeThreshold.toString()}%`
  );
  console.log(COLOR_INFO, `     Increase: ${target.emissionIncreasePct}%`);
  console.log(COLOR_INFO, `     Decrease: ${target.emissionDecreasePct}%`);

  const alreadyMatches =
    valueEquals(mineBtcMining.priceChangeThreshold, target.priceChangeThreshold) &&
    valueEquals(mineBtcMining.emissionIncreasePct, target.emissionIncreasePct) &&
    valueEquals(mineBtcMining.emissionDecreasePct, target.emissionDecreasePct);

  if (alreadyMatches) {
    console.log(COLOR_INFO, "ℹ️ Emission params already match config. Skipping...");
    return;
  }

  const tx = await minebtcProgram.methods
    .updateEmissionParams(
      target.priceChangeThreshold,
      target.emissionIncreasePct,
      target.emissionDecreasePct
    )
    .accounts({
      dbtcMining: mineBtcMiningPDA,
      globalConfig: globalConfigPDA,
      authority: wallet.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log(COLOR_SUCCESS, "✅ Emission params updated successfully!");
  console.log(COLOR_DIM, `   Transaction: ${tx}`);

  deploymentFile.emission_params_updated = {
    price_change_threshold_pct: target.priceChangeThreshold.toString(),
    emission_increase_pct: target.emissionIncreasePct.toString(),
    emission_decrease_pct: target.emissionDecreasePct.toString(),
    tx_signature: tx,
    timestamp: new Date().toISOString(),
  };
  saveDeploymentData();
}

async function updateFactionWarConfig(minebtcProgram, factionWarConfig) {
  console.log(
    COLOR_STEP,
    "\n================ [ FACTION WAR CONFIG TOGGLE ] ================"
  );

  if (factionWarConfig?.isActive === false) {
    console.log(
      COLOR_WARNING,
      "⚠️ config.faction_war.is_active=false is ignored: the on-chain active toggle was removed. Use set_pause for global launch pause."
    );
  } else {
    console.log(
      COLOR_INFO,
      "ℹ️ No-op: faction-war cycles are enabled once war_config is initialized."
    );
  }
}

async function updateGameplayTuning(minebtcProgram, gameplayTuningConfig) {
  console.log(
    COLOR_STEP,
    "\n================ [ UPDATING GAMEPLAY TUNING ] ================"
  );

  if (!deploymentFile.minebtc_program_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ MineBTC program not initialized. Skipping gameplay tuning update..."
    );
    return;
  }

  const globalConfigPDA = new PublicKey(
    deploymentFile.minebtc_program_initialized.globalConfig_address
  );
  const globalConfig = await minebtcProgram.account.globalConfig.fetch(
    globalConfigPDA
  );
  const current = globalConfig.gameplayTuning;

  // Field name follows the Rust `GameplayTuningUpdateArgs` struct, not the
  // stored `GameplayTuningConfig` struct — the args type renames it from
  // `rpg_progression` to `enable_rpg_progression`. Anchor exposes that to
  // JS as `enableRpgProgression`; sending `rpgProgression` would drop to
  // undefined and crash the Option<bool> borsh encoder.
  const target = {
    enableRpgProgression: gameplayTuningConfig.enableRpgProgression,
    maxEvolutionStageUnlocked: gameplayTuningConfig.maxEvolutionStageUnlocked,
    warBaseRewardBps: gameplayTuningConfig.factionWarBaseRewardBps,
    warMvpRewardBps: gameplayTuningConfig.factionWarMvpRewardBps,
    warHashbeastRewardBps: gameplayTuningConfig.factionWarHashBeastRewardBps,
    baseMutationChanceBps: gameplayTuningConfig.baseMutationChanceBps,
    mutationChanceFloorBps: gameplayTuningConfig.mutationChanceFloorBps,
    mutationChanceCapBps: gameplayTuningConfig.mutationChanceCapBps,
    factionVolumeThresholdLamports: new BN(
      gameplayTuningConfig.factionVolumeThresholdLamports
    ),
    extraVolumeThresholdPerMutationLamports: new BN(
      gameplayTuningConfig.extraVolumeThresholdPerMutationLamports
    ),
    targetMutationsPerCycle: gameplayTuningConfig.targetMutationsPerCycle,
    targetRoundsPerCycle: gameplayTuningConfig.targetRoundsPerCycle,
    pacingMaxAdjustmentBps: gameplayTuningConfig.pacingMaxAdjustmentBps,
  };

  const rewardSplit =
    target.warBaseRewardBps +
    target.warMvpRewardBps +
    target.warHashbeastRewardBps;
  if (rewardSplit !== 10000) {
    throw new Error(
      `Invalid gameplay reward split: base + mvp + hashbeast must equal 10000 bps, got ${rewardSplit}.`
    );
  }

  console.log(COLOR_INFO, "   Current gameplay tuning:");
  console.log(COLOR_INFO, `     RPG progression: ${current.rpgProgression}`);
  console.log(
    COLOR_INFO,
    `     Evolution stage unlocked: ${current.maxEvolutionStageUnlocked}`
  );
  console.log(
    COLOR_INFO,
    `     Rewards bps base/mvp/hashbeast: ${current.warBaseRewardBps}/${current.warMvpRewardBps}/${current.warHashbeastRewardBps}`
  );
  console.log(
    COLOR_INFO,
    `     Mutation chance bps base/floor/cap: ${current.baseMutationChanceBps}/${current.mutationChanceFloorBps}/${current.mutationChanceCapBps}`
  );
  console.log(
    COLOR_INFO,
    `     Target mutations/rounds: ${current.targetMutationsPerCycle}/${current.targetRoundsPerCycle}`
  );

  console.log(COLOR_INFO, "   Target gameplay tuning:");
  console.log(COLOR_INFO, `     RPG progression: ${target.enableRpgProgression}`);
  console.log(
    COLOR_INFO,
    `     Evolution stage unlocked: ${target.maxEvolutionStageUnlocked}`
  );
  console.log(
    COLOR_INFO,
    `     Rewards bps base/mvp/hashbeast: ${target.warBaseRewardBps}/${target.warMvpRewardBps}/${target.warHashbeastRewardBps}`
  );
  console.log(
    COLOR_INFO,
    `     Mutation chance bps base/floor/cap: ${target.baseMutationChanceBps}/${target.mutationChanceFloorBps}/${target.mutationChanceCapBps}`
  );
  console.log(
    COLOR_INFO,
    `     Volume thresholds: first=${target.factionVolumeThresholdLamports.toString()} lamports, extra=${target.extraVolumeThresholdPerMutationLamports.toString()} lamports`
  );
  console.log(
    COLOR_INFO,
    `     Target mutations/rounds: ${target.targetMutationsPerCycle}/${target.targetRoundsPerCycle}`
  );

  const alreadyMatches =
    current.rpgProgression === target.rpgProgression &&
    current.maxEvolutionStageUnlocked === target.maxEvolutionStageUnlocked &&
    current.warBaseRewardBps === target.warBaseRewardBps &&
    current.warMvpRewardBps === target.warMvpRewardBps &&
    current.warHashbeastRewardBps === target.warHashbeastRewardBps &&
    current.baseMutationChanceBps === target.baseMutationChanceBps &&
    current.mutationChanceFloorBps === target.mutationChanceFloorBps &&
    current.mutationChanceCapBps === target.mutationChanceCapBps &&
    valueEquals(
      current.factionVolumeThresholdLamports,
      target.factionVolumeThresholdLamports
    ) &&
    valueEquals(
      current.extraVolumeThresholdPerMutationLamports,
      target.extraVolumeThresholdPerMutationLamports
    ) &&
    current.targetMutationsPerCycle === target.targetMutationsPerCycle &&
    current.targetRoundsPerCycle === target.targetRoundsPerCycle &&
    current.pacingMaxAdjustmentBps === target.pacingMaxAdjustmentBps;

  if (alreadyMatches) {
    console.log(COLOR_INFO, "ℹ️ Gameplay tuning already matches config. Skipping...");
    return;
  }

  const tx = await minebtcProgram.methods
    .updateGameplayTuning(target)
    .accounts({
      globalConfig: globalConfigPDA,
      authority: wallet.publicKey,
    })
    .rpc();

  console.log(COLOR_SUCCESS, "✅ Gameplay tuning updated successfully!");
  console.log(COLOR_DIM, `   Transaction: ${tx}`);

  deploymentFile.gameplay_tuning_updated = {
    gameplay_tuning: config.gameplay_tuning,
    tx_signature: tx,
    timestamp: new Date().toISOString(),
  };
  saveDeploymentData();
}

// Note: `addGameCrankerBot` / `add_cranker_bot` was removed with the switch to
// permissionless keepers. No on-chain whitelist step is required anymore.

async function initializeFactionWarConfig(minebtcProgram) {
  if (deploymentFile.war_config_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Faction war config already initialized. Skipping..."
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n================ [ INITIALIZING FACTION WAR CONFIG ] ================"
  );

  try {
    const [factionWarConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("faction-war-config")],
      minebtcProgram.programId
    );

    const [globalConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global-config")],
      minebtcProgram.programId
    );

    console.log(
      COLOR_INFO,
      `🔑 Faction War Config PDA: ${factionWarConfigPda.toBase58()}`
    );
    console.log(COLOR_INFO, `🔑 Global Config PDA: ${globalConfigPda.toBase58()}`);
    console.log(
      COLOR_INFO,
      `🔄 Faction wars are keeper-initialized per cycle and settle after LP burn completion`
    );

    const tx = await minebtcProgram.methods
      .initializeWarConfig()
      .accounts({
        warConfig: factionWarConfigPda,
        globalConfig: globalConfigPda,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, `✅ Faction war config initialized successfully!`);
    console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);
    console.log(
      COLOR_DIM,
      `🔍 Explorer: https://explorer.solana.com/tx/${tx}?cluster=${CLUSTER}`
    );

    deploymentFile.war_config_initialized = {
      war_config_pda: factionWarConfigPda.toBase58(),
      starting_war_id: 1,
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log(
        COLOR_INFO,
        "ℹ️ Faction war config already exists on-chain. Skipping..."
      );
      deploymentFile.war_config_initialized = {
        status: "already_exists",
        timestamp: new Date().toISOString(),
      };
      saveDeploymentData();
    } else {
      console.error(
        COLOR_ERROR,
        "❌ Failed to initialize faction war config:",
        error
      );
      throw error;
    }
  }
}

// ==================== [ MARKETPLACE + INVENTORY INITIALIZATION ] ====================

async function initializeDegenBtcMarketplace(minebtcProgram) {
  if (deploymentFile.degenbtc_marketplace_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ DegenBTC marketplace already initialized. Skipping...",
    );
    return;
  }

  if (!deploymentFile.hashbeast_collection_created?.collection_address) {
    console.log(
      COLOR_WARNING,
      "⚠️ HashBeast collection not yet created — skipping marketplace init. Re-run after collection step.",
    );
    return;
  }

  if (!IDL_DegenBtcMarket) {
    console.log(
      COLOR_WARNING,
      "⚠️ DegenBTC marketplace IDL not found at " +
        `${config.deployment.paths.degenbtc_market_idl}. ` +
        "Run `anchor build` then re-run this script.",
    );
    return;
  }

  if (!DEGENBTC_MARKET_PROGRAM_ID) {
    console.log(
      COLOR_WARNING,
      "⚠️ DegenBTC market program ID not found in deployment file. Run 0_deploy_game.js first.",
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING DEGENBTC MARKETPLACE ] ===================",
  );

  const collectionMint = new PublicKey(
    deploymentFile.hashbeast_collection_created.collection_address,
  );
  const feeRecipient = walletKeypair.publicKey; // protocol-fee router target.
  // Fee proceeds eventually flow into the protocol fee pipeline; we route the
  // marketplace fee straight to the deploy/admin wallet here, matching the
  // pattern used by the rest of the program (admin can rotate via
  // update_marketplace_config later).

  const marketProgram = new Program(IDL_DegenBtcMarket, provider);

  const [marketplaceConfigPda, marketplaceConfigBump] =
    PublicKey.findProgramAddressSync(
      [Buffer.from("marketplace-config"), collectionMint.toBuffer()],
      DEGENBTC_MARKET_PROGRAM_ID,
    );

  console.log(COLOR_INFO, "🏪 Marketplace config PDA:", marketplaceConfigPda.toBase58());
  console.log(COLOR_DIM, `   collection_mint: ${collectionMint.toBase58()}`);
  console.log(COLOR_DIM, `   fee_bps: ${MARKETPLACE_CONFIG.feeBps}`);
  console.log(
    COLOR_DIM,
    `   min_price_lamports: ${MARKETPLACE_CONFIG.minPriceLamports.toString()}`,
  );
  console.log(COLOR_DIM, `   fee_recipient: ${feeRecipient.toBase58()}`);
  console.log(COLOR_DIM, `   admin: ${walletKeypair.publicKey.toBase58()}`);

  try {
    const tx = await marketProgram.methods
      .initializeMarketplace(
        MARKETPLACE_CONFIG.feeBps,
        feeRecipient,
        MARKETPLACE_CONFIG.minPriceLamports,
        MPL_CORE_PROGRAM_ID,
      )
      .accounts({
        payer: walletKeypair.publicKey,
        admin: walletKeypair.publicKey,
        marketplaceConfig: marketplaceConfigPda,
        collectionMint: collectionMint,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, "✅ DegenBTC marketplace initialized");
    console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);

    deploymentFile.degenbtc_marketplace_initialized = {
      marketplace_config_pda: marketplaceConfigPda.toBase58(),
      marketplace_config_bump: marketplaceConfigBump,
      collection_mint: collectionMint.toBase58(),
      fee_bps: MARKETPLACE_CONFIG.feeBps,
      fee_recipient: feeRecipient.toBase58(),
      min_price_lamports: MARKETPLACE_CONFIG.minPriceLamports.toString(),
      admin: walletKeypair.publicKey.toBase58(),
      mpl_core_program: MPL_CORE_PROGRAM_ID.toBase58(),
      program_id: DEGENBTC_MARKET_PROGRAM_ID.toBase58(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log(
        COLOR_INFO,
        "ℹ️ Marketplace config already exists on-chain. Recording PDA and continuing...",
      );
      deploymentFile.degenbtc_marketplace_initialized = {
        marketplace_config_pda: marketplaceConfigPda.toBase58(),
        marketplace_config_bump: marketplaceConfigBump,
        collection_mint: collectionMint.toBase58(),
        program_id: DEGENBTC_MARKET_PROGRAM_ID.toBase58(),
        status: "already_exists",
        timestamp: new Date().toISOString(),
      };
      saveDeploymentData();
    } else {
      console.error(
        COLOR_ERROR,
        "❌ Failed to initialize marketplace:",
        error,
      );
      if (error.logs) {
        error.logs.forEach((log) => console.error(COLOR_DIM, log));
      }
      throw error;
    }
  }
}

async function initializeInventoryPool(minebtcProgram) {
  if (deploymentFile.inventory_pool_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Inventory pool already initialized. Skipping...",
    );
    return;
  }

  if (!deploymentFile.degenbtc_marketplace_initialized) {
    console.log(
      COLOR_WARNING,
      "⚠️ Marketplace must be initialized before inventory pool — skipping.",
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING INVENTORY POOL ] ===================",
  );

  const [inventoryPoolPda, inventoryPoolBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("inventory-pool")],
    minebtcProgram.programId,
  );
  const [floorQueuePda, floorQueueBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("floor-queue")],
    minebtcProgram.programId,
  );
  const [saleHistoryPda, saleHistoryBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("sale-history")],
    minebtcProgram.programId,
  );
  const [floorHistoryPda, floorHistoryBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("floor-history")],
    minebtcProgram.programId,
  );
  const [inventorySweepVaultPda, inventorySweepVaultBump] =
    PublicKey.findProgramAddressSync(
      [Buffer.from("inventory-sweep-vault")],
      minebtcProgram.programId,
    );

  const marketplaceProgramId = new PublicKey(
    deploymentFile.degenbtc_marketplace_initialized.program_id,
  );
  const marketplaceConfigPda = new PublicKey(
    deploymentFile.degenbtc_marketplace_initialized.marketplace_config_pda,
  );

  console.log(COLOR_INFO, "📦 Inventory PDAs:");
  console.log(COLOR_DIM, `   inventory_pool: ${inventoryPoolPda.toBase58()}`);
  console.log(COLOR_DIM, `   floor_queue: ${floorQueuePda.toBase58()}`);
  console.log(COLOR_DIM, `   sale_history: ${saleHistoryPda.toBase58()}`);
  console.log(COLOR_DIM, `   floor_history: ${floorHistoryPda.toBase58()}`);
  console.log(
    COLOR_DIM,
    `   inventory_sweep_vault: ${inventorySweepVaultPda.toBase58()}`,
  );
  console.log(
    COLOR_DIM,
    `   marketplace_program: ${marketplaceProgramId.toBase58()}`,
  );
  console.log(
    COLOR_DIM,
    `   marketplace_config: ${marketplaceConfigPda.toBase58()}`,
  );

  const [globalConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("global-config")],
    minebtcProgram.programId,
  );

  try {
    const tx = await minebtcProgram.methods
      .initInventoryPool(marketplaceProgramId, marketplaceConfigPda)
      .accounts({
        authority: walletKeypair.publicKey,
        globalConfig: globalConfigPda,
        inventoryPool: inventoryPoolPda,
        floorQueue: floorQueuePda,
        saleHistory: saleHistoryPda,
        floorHistory: floorHistoryPda,
        inventorySweepVault: inventorySweepVaultPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(COLOR_SUCCESS, "✅ Inventory pool initialized");
    console.log(COLOR_DIM, `🔗 Transaction: ${tx}`);

    deploymentFile.inventory_pool_initialized = {
      inventory_pool_pda: inventoryPoolPda.toBase58(),
      inventory_pool_bump: inventoryPoolBump,
      floor_queue_pda: floorQueuePda.toBase58(),
      floor_queue_bump: floorQueueBump,
      sale_history_pda: saleHistoryPda.toBase58(),
      sale_history_bump: saleHistoryBump,
      floor_history_pda: floorHistoryPda.toBase58(),
      floor_history_bump: floorHistoryBump,
      inventory_sweep_vault_pda: inventorySweepVaultPda.toBase58(),
      inventory_sweep_vault_bump: inventorySweepVaultBump,
      marketplace_program: marketplaceProgramId.toBase58(),
      marketplace_config: marketplaceConfigPda.toBase58(),
      tx_signature: tx,
      timestamp: new Date().toISOString(),
    };
    saveDeploymentData();
  } catch (error) {
    if (error.toString().includes("already in use")) {
      console.log(
        COLOR_INFO,
        "ℹ️ Inventory pool already exists on-chain. Recording PDAs and continuing...",
      );
      deploymentFile.inventory_pool_initialized = {
        inventory_pool_pda: inventoryPoolPda.toBase58(),
        inventory_pool_bump: inventoryPoolBump,
        floor_queue_pda: floorQueuePda.toBase58(),
        floor_queue_bump: floorQueueBump,
        sale_history_pda: saleHistoryPda.toBase58(),
        sale_history_bump: saleHistoryBump,
        floor_history_pda: floorHistoryPda.toBase58(),
        floor_history_bump: floorHistoryBump,
        inventory_sweep_vault_pda: inventorySweepVaultPda.toBase58(),
        inventory_sweep_vault_bump: inventorySweepVaultBump,
        marketplace_program: marketplaceProgramId.toBase58(),
        marketplace_config: marketplaceConfigPda.toBase58(),
        status: "already_exists",
        timestamp: new Date().toISOString(),
      };
      saveDeploymentData();
    } else {
      console.error(
        COLOR_ERROR,
        "❌ Failed to initialize inventory pool:",
        error,
      );
      if (error.logs) {
        error.logs.forEach((log) => console.error(COLOR_DIM, log));
      }
      throw error;
    }
  }
}

async function initializeLootboxQueues(minebtcProgram) {
  if (deploymentFile.lootbox_queues_initialized) {
    console.log(
      COLOR_INFO,
      "ℹ️ Lootbox queues already initialized. Skipping...",
    );
    return;
  }

  console.log(
    COLOR_STEP,
    "\n=================== [ INITIALIZING LOOTBOX QUEUES ] ===================",
  );

  const factions = config.factions || [];
  if (factions.length === 0) {
    console.log(
      COLOR_WARNING,
      "⚠️ No factions configured — skipping lootbox queue init.",
    );
    return;
  }

  const queues = [];
  for (let i = 0; i < factions.length; i++) {
    const faction = factions[i];
    // faction_id matches the index used in addFactions (which is what the
    // contract uses as the canonical faction id).
    const factionId = i;
    const [queuePda, queueBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("lootbox-queue"), Buffer.from([factionId])],
      minebtcProgram.programId,
    );
    const [globalConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global-config")],
      minebtcProgram.programId,
    );

    console.log(
      COLOR_DIM,
      `   faction ${factionId} (${faction.name}) → ${queuePda.toBase58()}`,
    );

    try {
      const tx = await minebtcProgram.methods
        .initLootboxQueue(factionId)
        .accounts({
          authority: walletKeypair.publicKey,
          globalConfig: globalConfigPda,
          lootboxQueue: queuePda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      queues.push({
        faction_id: factionId,
        faction_name: faction.name,
        queue_pda: queuePda.toBase58(),
        bump: queueBump,
        tx_signature: tx,
      });
    } catch (error) {
      if (error.toString().includes("already in use")) {
        console.log(
          COLOR_INFO,
          `   ℹ️ faction ${factionId} queue already exists.`,
        );
        queues.push({
          faction_id: factionId,
          faction_name: faction.name,
          queue_pda: queuePda.toBase58(),
          bump: queueBump,
          status: "already_exists",
        });
      } else {
        console.error(
          COLOR_ERROR,
          `❌ Failed to init lootbox queue for faction ${factionId}:`,
          error,
        );
        throw error;
      }
    }
  }

  console.log(
    COLOR_SUCCESS,
    `✅ Initialized ${queues.length} lootbox queues`,
  );

  deploymentFile.lootbox_queues_initialized = {
    queues,
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
    `  • HashBeast Collection: ${
      deploymentFile.hashbeast_collection_created ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • HashBeast Mining: ${
      deploymentFile.hashbeast_mining_enabled ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • HashBeast Royalties: ${
      deploymentFile.hashbeast_royalties_initialized ? "✅" : "❌"
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
    COLOR_INFO,
    `  • Faction War Config: ${
      deploymentFile.war_config_initialized ? "✅" : "❌"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Emission Params: ${
      deploymentFile.emission_params_updated ? "✅" : "config/default"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • Gameplay Tuning: ${
      deploymentFile.gameplay_tuning_updated ? "✅" : "config/default"
    }`
  );
  console.log(
    COLOR_INFO,
    `  • DegenBTC Marketplace: ${
      deploymentFile.degenbtc_marketplace_initialized ? "✅" : "❌"
    }`,
  );
  console.log(
    COLOR_INFO,
    `  • Inventory Pool: ${
      deploymentFile.inventory_pool_initialized ? "✅" : "❌"
    }`,
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
        if (deploymentFile.hashbeast_collection_created) {
      console.log(
        COLOR_DIM,
        `   HashBeast Collection: ${deploymentFile.hashbeast_collection_created.collection_address}`
      );
        }
        if (deploymentFile.game_state_initialized) {
      console.log(
        COLOR_DIM,
        `   Game State: ${deploymentFile.game_state_initialized.global_game_state_pda}`
      );
    }
    if (deploymentFile.degenbtc_marketplace_initialized) {
      console.log(
        COLOR_DIM,
        `   Marketplace Program: ${deploymentFile.degenbtc_marketplace_initialized.program_id}`,
      );
      console.log(
        COLOR_DIM,
        `   Marketplace Config:  ${deploymentFile.degenbtc_marketplace_initialized.marketplace_config_pda}`,
      );
    }
    if (deploymentFile.inventory_pool_initialized) {
      console.log(
        COLOR_DIM,
        `   Inventory Pool:       ${deploymentFile.inventory_pool_initialized.inventory_pool_pda}`,
      );
      console.log(
        COLOR_DIM,
        `   Floor Queue:          ${deploymentFile.inventory_pool_initialized.floor_queue_pda}`,
      );
      console.log(
        COLOR_DIM,
        `   Sale History:         ${deploymentFile.inventory_pool_initialized.sale_history_pda}`,
      );
      console.log(
        COLOR_DIM,
        `   Floor History:        ${deploymentFile.inventory_pool_initialized.floor_history_pda}`,
      );
      console.log(
        COLOR_DIM,
        `   Inventory Sweep Vault: ${deploymentFile.inventory_pool_initialized.inventory_sweep_vault_pda}`,
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
    "   2. Users can mint HashBeast for their factions"
  );
  console.log(COLOR_INFO, "   3. Users can stake DegenBtc and LP tokens");
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
