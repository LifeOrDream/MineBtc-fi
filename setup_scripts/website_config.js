#!/usr/bin/env node

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const CONFIG_PATH = path.join(__dirname, "config.json");
const DEPLOYMENTS_DIR = path.join(__dirname, "deployments");

// Solana well-known program IDs the FE/BE need to reference directly
const MPL_CORE_PROGRAM_ID = "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";
const ASSOCIATED_TOKEN_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

function readConfig() {
  try {
    return JSON.parse(fs.readFileSync(CONFIG_PATH, "utf8"));
  } catch (error) {
    console.error(`❌ Failed to read config.json: ${error.message}`);
    process.exit(1);
  }
}

function readDeploymentFile(cluster) {
  const deploymentPath = path.join(DEPLOYMENTS_DIR, `${cluster}.json`);
  if (!fs.existsSync(deploymentPath)) {
    console.error(`❌ Deployment file not found: ${deploymentPath}`);
    process.exit(1);
  }
  try {
    return JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
  } catch (error) {
    console.error(`❌ Failed to read deployment file: ${error.message}`);
    process.exit(1);
  }
}

function generateWebsiteConfig(config, deployment) {
  const cluster = config.network?.cluster || "localnet";

  const dbtcMint =
    deployment.dbtc_mint_address ||
    deployment.dbtc_mint_created?.mint_address ||
    null;

  const collectionMint =
    deployment.hashbeast_collection_created?.collection_address || null;

  // Lootbox queues: one PDA per faction. FE consumes by faction_id.
  const lootboxQueues =
    deployment.lootbox_queues_initialized?.queues?.map((q) => ({
      faction_id: q.faction_id,
      faction_name: q.faction_name,
      queue_pda: q.queue_pda,
    })) || [];

  // Faction state PDAs (one per supported country).
  const factions =
    deployment.factions_added?.factions?.map((f) => ({
      faction_id: f.faction_id,
      name: f.name,
      faction_state_pda: f.faction_state_pda,
    })) || [];

  const websiteConfig = {
    [cluster]: {
      // ───────── NETWORK ─────────
      network: {
        cluster,
        rpc_url: config.network?.rpc_url || "http://127.0.0.1:8899",
        commitment: config.network?.commitment || "confirmed",
      },

      // ───────── PROGRAMS ─────────
      programs: {
        mineBTC: deployment.MINE_BTC_PROGRAM_ID,
        degenbtc_market: deployment.DEGENBTC_MARKET_PROGRAM_ID,
        raydium_cp: deployment.RAYDIUM_CP_PROGRAM_ID,
        mpl_core: MPL_CORE_PROGRAM_ID,
        token_2022: TOKEN_2022_PROGRAM_ID,
        system: SYSTEM_PROGRAM_ID,
        associated_token: ASSOCIATED_TOKEN_PROGRAM_ID,
      },

      // ───────── DEGEN_BTC TOKEN (Token-2022) ─────────
      // The "burn tax" is a Token-2022 transfer fee. Routing of withheld fees
      // is handled by the program: 25% faction treasury, 50% burned, 25%
      // recycled into the mining vault (see tax_config).
      token: {
        mint: dbtcMint,
        decimals:
          deployment.dbtc_mint_created?.decimals ??
          config.token?.decimals ??
          6,
        transfer_fee_bps:
          deployment.dbtc_mint_created?.transfer_tax_bps ??
          config.token?.transfer_tax_bps,
        max_transfer_fee:
          deployment.dbtc_mint_created?.max_transfer_fee_amount?.toString() ||
          config.token?.max_transfer_fee_amount?.toString(),
        withdraw_withheld_authority:
          deployment.dbtc_mint_created?.withdraw_withheld_authority || null,
        transfer_fee_config_authority:
          deployment.dbtc_mint_created?.transfer_fee_config_authority ?? null,
        metadata: {
          name: deployment.dbtc_mint_created?.metadata_name || null,
          symbol: deployment.dbtc_mint_created?.metadata_symbol || null,
          uri: deployment.dbtc_mint_created?.metadata_uri || null,
          image: deployment.dbtc_mint_created?.metadata_image || null,
        },
      },

      // ───────── RAYDIUM POOL (dBTC / SOL) ─────────
      raydium: {
        amm_config: deployment.raydium_amm_config_created?.amm_config_pda,
        pool_state: deployment.dbtc_sol_pool_created?.poolStatePDA,
        lp_mint: deployment.dbtc_sol_pool_created?.lpMintPDA,
        token0_vault: deployment.dbtc_sol_pool_created?.token0VaultPDA,
        token1_vault: deployment.dbtc_sol_pool_created?.token1VaultPDA,
        authority: deployment.dbtc_sol_pool_created?.authorityPDA,
        observation_state:
          deployment.dbtc_sol_pool_created?.observationStatePDA,
        token0_mint: deployment.dbtc_sol_pool_created?.token0Mint,
        token1_mint: deployment.dbtc_sol_pool_created?.token1Mint,
        is_dbtc_token0: deployment.dbtc_sol_pool_created?.isDbtcToken0 || false,
      },

      // ───────── CORE MINEBTC PROGRAM PDAS ─────────
      minebtc: {
        global_config: deployment.minebtc_program_initialized?.globalConfig_address,
        mine_btc_mining: deployment.minebtc_program_initialized?.mineBtcMining_address,
        sol_treasury: deployment.minebtc_program_initialized?.solTreasury_address,
        hodl_pool: deployment.minebtc_program_initialized?.hodlPool_address,
        autominer_custody:
          deployment.minebtc_program_initialized?.autominerCustody_address,
        hashpower_config:
          deployment.hashpower_config_initialized?.hashpowerConfig_pda,
      },

      // ───────── DEGEN_BTC MINING VAULT ─────────
      mining_vault: {
        vault: deployment.mining_vault_initialized?.vault_address,
        authority: deployment.mining_vault_initialized?.vault_authority,
        start_timestamp: deployment.mining_vault_initialized?.start_timestamp,
        per_round_emission:
          deployment.mining_vault_initialized?.degen_btc_per_round,
        funded_amount: deployment.mining_tokens_deposited?.amount || null,
      },

      // ───────── GAME STATE ─────────
      game: {
        global_state: deployment.game_state_initialized?.global_game_state_pda,
        round_duration_seconds:
          deployment.game_state_initialized?.round_duration_seconds,
        gameplay_tuning:
          deployment.gameplay_tuning_updated?.gameplay_tuning || null,
      },

      // ───────── FACTION WAR ─────────
      faction_war: {
        config: deployment.faction_war_config_initialized?.faction_war_config_pda,
        starting_id:
          deployment.faction_war_config_initialized?.starting_faction_war_id || 1,
        // Lazy-created on first SOL bet; PDA derived from [b"faction-war-sol-vault"].
        sol_vault: deployment.faction_war_sol_vault_pda || null,
        // Per-faction SOL reward pots used by the faction-war SOL bet/settlement
        // path; the FE hits these vaults (one per faction) plus the global vault
        // above when displaying pot sizes.
        sol_rewards_vault: deployment.raydium_pool_state_set?.sol_rewards_vault,
        sol_prize_pot_vault:
          deployment.raydium_pool_state_set?.sol_prize_pot_vault,
      },

      // ───────── FACTIONS (countries) ─────────
      factions,

      // ───────── HASHBEAST COLLECTION + MINTING ─────────
      hashbeasts: {
        collection: collectionMint,
        collection_authority:
          deployment.hashbeast_collection_created?.collection_authority,
        config: deployment.hashbeast_config_initialized?.hashbeasts_config_pda,
        mint_config:
          deployment.hashbeast_mint_config_initialized?.hashbeast_mint_config_pda,
        base_price:
          deployment.hashbeast_mint_config_initialized?.base_price?.toString(),
        curve_a:
          deployment.hashbeast_mint_config_initialized?.curve_a?.toString(),
        genesis_mint_limit:
          deployment.hashbeast_mint_config_initialized?.genesis_mint_limit?.toString(),
        max_genesis_mints_per_faction:
          deployment.hashbeast_mint_config_initialized?.max_genesis_mints_per_faction?.toString(),
        // Royalty config baked into the mpl-core collection plugin.
        royalties: {
          basis_points:
            deployment.hashbeast_royalties_initialized?.basis_points ?? null,
          creators:
            deployment.hashbeast_royalties_initialized?.creators || [],
        },
        // Breeding parameters seeded into HashBeastConfig. `breeding_allowed`
        // is initially false; admin flips it on after genesis sells out.
        // Runtime cost = max(curve_price, 1.5 × current_floor_anchor),
        // paid 50% SOL + 50% dbtc by SOL value.
        breeding: {
          allowed: deployment.breeding_config_seeded?.breeding_allowed ?? false,
          base_price:
            deployment.breeding_config_seeded?.breed_base_price?.toString() ||
            null,
          curve_a:
            deployment.breeding_config_seeded?.breed_curve_a?.toString() || null,
        },
      },

      // ───────── DEGENBTC NFT MARKETPLACE (permissionless on-chain) ─────────
      // Inventory pool is the FloorQueue + SaleHistory + FloorHistory state
      // owned by mineBTC; sweep_vault is the SOL pot fed by 3% of
      // distribute_sol_fees that the program uses to sweep the floor.
      marketplace: {
        config: deployment.degenbtc_marketplace_initialized?.marketplace_config_pda,
        admin: deployment.degenbtc_marketplace_initialized?.admin,
        fee_recipient: deployment.degenbtc_marketplace_initialized?.fee_recipient,
        fee_bps: deployment.degenbtc_marketplace_initialized?.fee_bps,
        min_price_lamports:
          deployment.degenbtc_marketplace_initialized?.min_price_lamports,
        inventory_pool: deployment.inventory_pool_initialized?.inventory_pool_pda,
        floor_queue: deployment.inventory_pool_initialized?.floor_queue_pda,
        sale_history: deployment.inventory_pool_initialized?.sale_history_pda,
        floor_history: deployment.inventory_pool_initialized?.floor_history_pda,
        inventory_sweep_vault:
          deployment.inventory_pool_initialized?.inventory_sweep_vault_pda,
      },

      // ───────── LOOTBOX QUEUES (one per faction) ─────────
      lootbox_queues: lootboxQueues,

      // ───────── TAX / FEE ROUTING ─────────
      // tax_config governs Token-2022 withheld-fee distribution.
      // sol_fee_config governs SOL fee distribution (distribute_sol_fees).
      tax: {
        config: deployment.tax_config_initialized?.tax_config_pda,
        withdraw_withheld_authority:
          deployment.tax_config_initialized?.withdraw_withheld_authority,
        faction_treasury_vault:
          deployment.tax_config_initialized?.faction_treasury_vault,
        faction_treasury_pct:
          deployment.tax_config_initialized?.faction_treasury_pct,
        burn_pct: deployment.tax_config_initialized?.burn_pct,
      },
      sol_fee_config: deployment.fees_updated?.fee_config || null,

      // ───────── TICKET TIERS ─────────
      ticket_tiers:
        deployment.ticket_tier_configs_initialized?.ticket_tiers?.map((t) => ({
          tier_index: t.tier_index,
          ticket_value: t.ticket_value,
        })) ||
        config.hashbeasts_config?.ticket_tiers ||
        [],

      // ───────── SYSTEM / REFERRAL / BUYBACKS ─────────
      system: {
        referral_rewards:
          deployment.system_accounts_initialized?.system_referral_rewards_pda,
        buybacks_account:
          deployment.system_accounts_initialized?.buybacks_account_pda,
        buybacks_sol_vault:
          deployment.system_accounts_initialized?.buybacks_sol_vault_pda,
      },

      // ───────── CUSTODIANS ─────────
      custodians: {
        minebtc: deployment.custodian_accounts_initialized?.dbtc_custodian,
        minebtc_authority:
          deployment.custodian_accounts_initialized?.dbtc_custodian_authority,
        liquidity:
          deployment.custodian_accounts_initialized?.liquidity_custodian,
        liquidity_authority:
          deployment.custodian_accounts_initialized
            ?.liquidity_custodian_authority,
      },

      // ───────── LP TOKEN (locked) ─────────
      lp: {
        token_account:
          deployment.lp_token_accounts_initialized?.lp_token_account,
        token_owner:
          deployment.lp_token_accounts_initialized?.lp_token_owner,
        mint: deployment.lp_token_accounts_initialized?.lp_mint,
      },

      // ───────── AUTHORITIES ─────────
      authorities: {
        fee_recipient_multisig:
          deployment.minebtc_program_initialized?.FEE_RECIPIENT_MULTISIG ||
          config.deployment?.FEE_RECIPIENT_MULTISIG,
      },
    },
  };

  return websiteConfig;
}

function saveWebsiteConfig(websiteConfig, cluster) {
  const websitePath = path.join(DEPLOYMENTS_DIR, "website.json");

  let existingConfig = {};
  if (fs.existsSync(websitePath)) {
    try {
      existingConfig = JSON.parse(fs.readFileSync(websitePath, "utf8"));
    } catch (_error) {
      console.log("⚠️  Could not parse existing website.json, creating new one");
    }
  }

  // Merge so other clusters' entries are preserved when only one is regenerated.
  const mergedConfig = { ...existingConfig, ...websiteConfig };
  fs.writeFileSync(websitePath, JSON.stringify(mergedConfig, null, 2));
  console.log(`✅ Website configuration saved to: ${websitePath}`);
  console.log(`📍 Cluster: ${cluster}`);
}

function main() {
  console.log("🌐 Generating website configuration...");
  console.log("=======================================");

  try {
    const config = readConfig();
    const cluster = config.network?.cluster || "localnet";
    console.log(`📖 Reading configuration for cluster: ${cluster}`);

    const deployment = readDeploymentFile(cluster);
    console.log(
      `🔍 Found deployment data with ${Object.keys(deployment).length} entries`,
    );

    const websiteConfig = generateWebsiteConfig(config, deployment);
    const c = websiteConfig[cluster];

    saveWebsiteConfig(websiteConfig, cluster);

    console.log("\n🎉 WEBSITE CONFIGURATION GENERATED SUCCESSFULLY! 🎉");
    console.log("================================================");
    console.log(`  🌐 Cluster              : ${cluster}`);
    console.log(`  🔗 mineBTC program      : ${c.programs.mineBTC}`);
    console.log(`  🛒 Marketplace program  : ${c.programs.degenbtc_market}`);
    console.log(`  🪙 dBTC mint            : ${c.token.mint}`);
    console.log(`  🏊 Raydium pool         : ${c.raydium.pool_state}`);
    console.log(`  🎮 Global game state    : ${c.game.global_state || "—"}`);
    console.log(`  ⚔️  Faction war config   : ${c.faction_war.config || "—"}`);
    console.log(`  🥚 HashBeast collection : ${c.hashbeasts.collection || "—"}`);
    console.log(`  🛒 Marketplace config   : ${c.marketplace.config || "—"}`);
    console.log(`  📦 Inventory pool       : ${c.marketplace.inventory_pool || "—"}`);
    console.log(`  💰 Buybacks account     : ${c.system.buybacks_account || "—"}`);

    console.log("\n🔗 Next Steps:");
    console.log("  1. Frontend/backend should consume deployments/website.json");
    console.log("  2. Verify all addresses are accessible on " + cluster);
  } catch (error) {
    console.error("\n💥 CONFIGURATION GENERATION FAILED! 💥");
    console.error("======================================");
    console.error(`Error: ${error.message}`);
    console.error(error.stack);
    process.exit(1);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  main();
}

export default main;
