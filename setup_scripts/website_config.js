#!/usr/bin/env node
//
// website_config.js
// -----------------
// Reads the latest cluster deployment file under ./deployments/<cluster>.json
// and emits ./deployments/website.json containing ONLY the addresses + config
// the frontend / backend actually need to:
//   - build instructions for the mineBTC + marketplace programs,
//   - query on-chain state (PDAs by name),
//   - render the UI (token metadata, fee splits, breeding pricing, etc.).
//
// What's intentionally stripped from the output (vs. the raw deployment file):
//   - all `tx_signature` fields                  (deploy artifact, not state)
//   - all `timestamp` fields                     (deploy artifact)
//   - all "_status" markers ("removed", "frozen", "set_to_pda")
//   - duplicated /aliased entries (e.g. `dbtc_mint_address` vs nested .mint_address)
//   - the `metadata_included` / `creation_signature` boilerplate
//
// What's intentionally normalized:
//   - snake_case everywhere (matches Rust on-chain field names so devs reading
//     the program source don't have to translate),
//   - `sol_fee_config.newFooPct` → `sol_fee_config.foo_pct` (the "new" prefix
//     comes from the admin ix argument shape; meaningless once persisted).
//
// FE/BE behaviour contract:
//   - Always read the top-level `<cluster>` key (e.g. `website.json.devnet`).
//   - The exact same script runs against localnet / devnet / mainnet — the
//     output schema is identical across clusters; only the addresses change.
//   - This is a snapshot. If anything is re-initialized on chain, re-run this
//     script and ship the new website.json.

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const CONFIG_PATH = path.join(__dirname, "config.json");
const DEPLOYMENTS_DIR = path.join(__dirname, "deployments");
const OUTPUT_PATH = path.join(DEPLOYMENTS_DIR, "website.json");

// ───────── Solana ecosystem program IDs the FE/BE need to reference ─────────
const MPL_CORE_PROGRAM_ID = "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const TOKEN_PROGRAM_ID = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";
const ASSOCIATED_TOKEN_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const NATIVE_SOL_MINT = "So11111111111111111111111111111111111111112";

// PDA seeds. Expose so the FE can derive per-user PDAs (player_data,
// user_game_bet, user_war_bets, lootbox_claim, etc.) without re-typing the
// strings. Keep this list in sync with `programs/mineBTC/src/state.rs` —
// search for `pub const *_SEED: &[u8] =`.
const PDA_SEEDS = {
  // Global / singleton PDAs (already given as resolved addresses below, but
  // included so the FE can verify with find_program_address if needed).
  global_config: "global-config",
  global_game_state: "global-game-state",
  hashpower_config: "hashpower-config",
  hashbeast_config: "hashbeast-config",
  hashbeast_mint_config: "hashbeast-mint-config",
  tax_config: "tax-config",
  hodl_pool: "hodl-pool",
  buybacks: "buybacks",
  buybacks_sol_vault: "buybacks-sol-vault",
  sol_treasury: "sol-treasury",
  autominer_custody: "autominer-custody",
  faction_war_config: "faction-war-config",
  faction_war_sol_vault: "faction-war-sol-vault",
  staker_sol_reward_vault: "staker-sol-reward-vault",
  jackpot_pot_vault: "jackpot-pot-vault",
  inventory_pool: "inventory-pool",
  inventory_sweep_vault: "inventory-sweep-vault",
  floor_queue: "floor-queue",
  sale_history: "sale-history",
  floor_history: "floor-history",
  collection_authority: "collection-authority",
  hashbeast_custody: "hashbeast-custody",

  // Per-user / per-asset PDAs — FE derives at runtime.
  player_data: "player-data",                       // [seed, user]
  user_game_bet: "user-game-bet",                   // [seed, user, round_id_le]
  user_faction_war_bets: "user-faction-war",        // [seed, user, war_id_le]
  faction_war_state: "faction-war-state",           // [seed, war_id_le]
  faction_war_settlement: "faction-war-settlement", // [seed, war_id_le]
  game_session: "game-session",                     // [seed, round_id_le]
  autominer_vault: "autominer-vault",               // [seed, owner]
  referral_rewards: "referral-rewards",             // [seed, referral_code_pubkey]
  hashbeast_metadata: "hashbeast-metadata",         // [seed, asset_mint]
  reborn_entry: "reborn-entry",                     // [seed, asset_mint]
  lootbox_queue: "lootbox-queue",                   // [seed, [faction_id_u8]]
  lootbox_claim: "lootbox-claim",                   // [seed, user]
  faction_state: "faction",                         // [seed, faction_name_bytes]
};

// ───────── file IO ─────────

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

// Strip the `newFooBar` → `foo_bar` rename done by the admin ix args.
// (The fee config was persisted using the JS ix-arg shape, but FE wants the
// stored-state shape.)
function normalizeSolFeeConfig(raw) {
  if (!raw) return null;
  const stripNew = (k) =>
    k.startsWith("new") ? k[3].toLowerCase() + k.slice(4) : k;
  const camelToSnake = (s) =>
    s.replace(/[A-Z]/g, (m) => "_" + m.toLowerCase());
  const out = {};
  for (const [k, v] of Object.entries(raw)) {
    out[camelToSnake(stripNew(k))] = v;
  }
  return out;
}

// ───────── shape builder ─────────

function generateWebsiteConfig(config, deployment) {
  const cluster = config.network?.cluster || "localnet";

  // Defensive accessors — every nested entry in the deployment file is
  // populated step-by-step during init; partial / aborted runs leave gaps.
  // The FE/BE should still get a usable (but explicitly-null) object.
  const d = deployment;
  const get = (path, fallback = null) =>
    path
      .split(".")
      .reduce((acc, k) => (acc == null ? acc : acc[k]), d) ?? fallback;

  const dbtcMint =
    get("dbtc_mint_address") || get("dbtc_mint_created.mint_address");

  const factions =
    get("factions_added.factions")?.map((f) => ({
      faction_id: f.faction_id,
      name: f.name,
      state_pda: f.faction_state_pda,
    })) || [];

  const lootboxQueues =
    get("lootbox_queues_initialized.queues")?.map((q) => ({
      faction_id: q.faction_id,
      faction_name: q.faction_name,
      queue_pda: q.queue_pda,
    })) || [];

  const ticketTiers =
    get("ticket_tier_configs_initialized.ticket_tiers")?.map((t) => ({
      tier_index: t.tier_index,
      ticket_value_lamports: String(t.ticket_value),
    })) ||
    config.hashbeasts_config?.ticket_tiers ||
    [];

  const breedParentPrices =
    get("breeding_config_seeded.breed_parent_prices_lamports")?.map((p) =>
      String(p)
    ) || [];

  return {
    [cluster]: {
      // ───────── network ─────────
      network: {
        cluster,
        rpc_url: config.network?.rpc_url || "http://127.0.0.1:8899",
        commitment: config.network?.commitment || "confirmed",
      },

      // ───────── program IDs ─────────
      // Top-level program IDs the FE/BE pass into Anchor `Program` ctors.
      programs: {
        minebtc: get("MINE_BTC_PROGRAM_ID"),
        market: get("DEGENBTC_MARKET_PROGRAM_ID"),
        raydium_cp_swap: get("RAYDIUM_CP_PROGRAM_ID"),
        mpl_core: MPL_CORE_PROGRAM_ID,
        token_2022: TOKEN_2022_PROGRAM_ID,
        token: TOKEN_PROGRAM_ID,
        system: SYSTEM_PROGRAM_ID,
        associated_token: ASSOCIATED_TOKEN_PROGRAM_ID,
      },

      // ───────── dBTC token (Token-2022) ─────────
      dbtc: {
        mint: dbtcMint,
        decimals:
          get("dbtc_mint_created.decimals") ?? config.token?.decimals ?? 6,
        // Transfer fee in basis points; FE displays as % and grosses up amounts.
        transfer_fee_bps:
          get("dbtc_mint_created.transfer_tax_bps") ??
          config.token?.transfer_tax_bps,
        max_transfer_fee_lamports:
          get("dbtc_mint_created.max_transfer_fee_amount")?.toString() ||
          config.token?.max_transfer_fee_amount?.toString(),
        metadata: {
          name: get("dbtc_mint_created.metadata_name"),
          symbol: get("dbtc_mint_created.metadata_symbol"),
          uri: get("dbtc_mint_created.metadata_uri"),
          image: get("dbtc_mint_created.metadata_image"),
          animation_url: get("dbtc_mint_created.metadata_animation_url"),
          external_url: get("dbtc_mint_created.metadata_external_url"),
        },
      },

      // ───────── native SOL mint (handy for swap UIs) ─────────
      sol: { mint: NATIVE_SOL_MINT, decimals: 9 },

      // ───────── Raydium CP-Swap dBTC/SOL pool ─────────
      // FE uses these for price reads + when building swap routes.
      raydium_pool: {
        amm_config: get("raydium_amm_config_created.amm_config_pda"),
        pool_state: get("dbtc_sol_pool_created.poolStatePDA"),
        lp_mint: get("dbtc_sol_pool_created.lpMintPDA"),
        token_0_vault: get("dbtc_sol_pool_created.token0VaultPDA"),
        token_1_vault: get("dbtc_sol_pool_created.token1VaultPDA"),
        pool_authority: get("dbtc_sol_pool_created.authorityPDA"),
        observation_state: get("dbtc_sol_pool_created.observationStatePDA"),
        token_0_mint: get("dbtc_sol_pool_created.token0Mint"),
        token_1_mint: get("dbtc_sol_pool_created.token1Mint"),
        // true ⇢ token_0 is dBTC, token_1 is WSOL (and vice versa).
        is_dbtc_token_0: get("dbtc_sol_pool_created.isDbtcToken0", false),
      },

      // ───────── mineBTC core PDAs (singletons) ─────────
      // Everything FE/BE needs to read program state or pass as an account.
      minebtc: {
        global_config: get("minebtc_program_initialized.globalConfig_address"),
        dbtc_mining: get("minebtc_program_initialized.mineBtcMining_address"),
        sol_treasury: get("minebtc_program_initialized.solTreasury_address"),
        hodl_pool: get("minebtc_program_initialized.hodlPool_address"),
        autominer_custody:
          get("minebtc_program_initialized.autominerCustody_address"),
        hashpower_config: get("hashpower_config_initialized.hashpowerConfig_pda"),
      },

      // ───────── dBTC mining vault (emissions source) ─────────
      mining_vault: {
        vault: get("mining_vault_initialized.vault_address"),
        authority: get("mining_vault_initialized.vault_authority"),
        per_round_emission_lamports:
          get("mining_vault_initialized.degen_btc_per_round")?.toString(),
        funded_amount_lamports:
          get("mining_tokens_deposited.amount")?.toString(),
      },

      // ───────── round / game state ─────────
      game: {
        global_state: get("game_state_initialized.global_game_state_pda"),
        round_duration_seconds:
          get("game_state_initialized.round_duration_seconds"),
        gameplay_tuning: get("gameplay_tuning_updated.gameplay_tuning"),
      },

      // ───────── faction war (cycle-level) ─────────
      faction_war: {
        config: get("war_config_initialized.war_config_pda"),
        starting_war_id: get("war_config_initialized.starting_war_id", 1),
        sol_vault: get("war_sol_vault_pda"),
        sol_rewards_vault: get("raydium_pool_state_set.sol_rewards_vault"),
        sol_prize_pot_vault: get("raydium_pool_state_set.sol_prize_pot_vault"),
      },

      // ───────── factions (countries) ─────────
      factions,

      // ───────── HashBeast NFT collection + mint config ─────────
      hashbeasts: {
        collection: get("hashbeast_collection_created.collection_address"),
        collection_authority:
          get("hashbeast_collection_created.collection_authority"),
        config: get("hashbeast_config_initialized.hashbeasts_config_pda"),
        mint_config:
          get("hashbeast_mint_config_initialized.hashbeast_mint_config_pda"),
        base_price_lamports:
          get("hashbeast_mint_config_initialized.base_price")?.toString(),
        curve_a:
          get("hashbeast_mint_config_initialized.curve_a")?.toString(),
        genesis_mint_limit:
          get("hashbeast_mint_config_initialized.genesis_mint_limit")?.toString(),
        max_genesis_mints_per_faction: get(
          "hashbeast_mint_config_initialized.max_genesis_mints_per_faction"
        )?.toString(),
        royalties: {
          basis_points:
            get("hashbeast_royalties_initialized.basis_points"),
          creators: get("hashbeast_royalties_initialized.creators", []),
        },
        breeding: {
          allowed: get("breeding_config_seeded.breeding_allowed", false),
          parent_prices_lamports: breedParentPrices,
          floor_multiplier_bps:
            get("breeding_config_seeded.floor_multiplier_bps", 15000),
        },
        mining_enabled: !!get("hashbeast_mining_enabled"),
      },

      // ───────── degenbtc marketplace + permissionless market maker ─────────
      // `inventory_pool` is both the typed `InventoryPool` account and the
      // raw custody PDA holding swept NFTs.
      marketplace: {
        config: get("degenbtc_marketplace_initialized.marketplace_config_pda"),
        admin: get("degenbtc_marketplace_initialized.admin"),
        fee_recipient:
          get("degenbtc_marketplace_initialized.fee_recipient"),
        fee_bps: get("degenbtc_marketplace_initialized.fee_bps"),
        min_price_lamports:
          get("degenbtc_marketplace_initialized.min_price_lamports")?.toString(),
        inventory_pool: get("inventory_pool_initialized.inventory_pool_pda"),
        floor_queue: get("inventory_pool_initialized.floor_queue_pda"),
        sale_history: get("inventory_pool_initialized.sale_history_pda"),
        floor_history: get("inventory_pool_initialized.floor_history_pda"),
        inventory_sweep_vault:
          get("inventory_pool_initialized.inventory_sweep_vault_pda"),
      },

      // ───────── lootbox queues (one per faction) ─────────
      // FE uses these PDAs to render queue depth + show pending lootbox NFTs.
      lootbox_queues: lootboxQueues,

      // ───────── Token-2022 transfer-fee routing ─────────
      tax: {
        config: get("tax_config_initialized.tax_config_pda"),
        withdraw_withheld_authority:
          get("tax_config_initialized.withdraw_withheld_authority"),
        faction_treasury_vault:
          get("tax_config_initialized.faction_treasury_vault"),
        treasury_pct: get("tax_config_initialized.treasury_pct"),
        burn_pct: get("tax_config_initialized.burn_pct"),
        // remainder (100 - treasury_pct - burn_pct) is recycled to the
        // mining vault; FE shouldn't display "back to vault" separately.
      },

      // ───────── SOL fee routing ─────────
      // Normalised: stripped the `new` prefix the admin ix-args carry.
      // `protocol_fee_pct` is the total cut from each bet that gets split
      // into the stakers / treasury / referral lanes.
      sol_fee_config: normalizeSolFeeConfig(get("fees_updated.fee_config")),

      // ───────── ticket tiers ─────────
      ticket_tiers: ticketTiers,

      // ───────── referral + buybacks accounts ─────────
      system: {
        // The default-no-referrer sentinel PDA the program created at init.
        // Players without a referrer have their `player_data.referral_code`
        // point at the system program ID; this account is the corresponding
        // ReferralRewards PDA for sanity-checking that flow.
        referral_rewards_sentinel:
          get("system_accounts_initialized.system_referral_rewards_pda"),
        buybacks_account:
          get("system_accounts_initialized.buybacks_account_pda"),
        buybacks_sol_vault:
          get("system_accounts_initialized.buybacks_sol_vault_pda"),
      },

      // ───────── token custodians (singleton PDAs) ─────────
      custodians: {
        dbtc: get("custodian_accounts_initialized.dbtc_custodian"),
        dbtc_authority:
          get("custodian_accounts_initialized.dbtc_custodian_authority"),
        lp: get("custodian_accounts_initialized.liquidity_custodian"),
        lp_authority:
          get("custodian_accounts_initialized.liquidity_custodian_authority"),
      },

      // ───────── LP token (locked) ─────────
      lp_token: {
        mint: get("lp_token_accounts_initialized.lp_mint"),
        token_account: get("lp_token_accounts_initialized.lp_token_account"),
        token_owner: get("lp_token_accounts_initialized.lp_token_owner"),
      },

      // ───────── authorities (admin / multisig pubkeys) ─────────
      authorities: {
        deployer: get("deployer_address"),
        fee_recipient_multisig:
          get("minebtc_program_initialized.FEE_RECIPIENT_MULTISIG") ||
          config.deployment?.FEE_RECIPIENT_MULTISIG,
      },

      // ───────── PDA seed strings ─────────
      // FE derives per-user / per-asset PDAs at runtime. Comments in the
      // const definition above show the [seed, …extra_seeds] shape per PDA.
      pda_seeds: PDA_SEEDS,
    },
  };
}

function saveWebsiteConfig(websiteConfig, cluster) {
  let existing = {};
  if (fs.existsSync(OUTPUT_PATH)) {
    try {
      existing = JSON.parse(fs.readFileSync(OUTPUT_PATH, "utf8"));
    } catch {
      console.log("⚠️  Could not parse existing website.json, overwriting.");
    }
  }
  // Merge so re-running for a single cluster doesn't wipe the others.
  const merged = { ...existing, ...websiteConfig };
  fs.writeFileSync(OUTPUT_PATH, JSON.stringify(merged, null, 2));
  console.log(`✅ ${OUTPUT_PATH} written (cluster=${cluster})`);
}

function main() {
  console.log("🌐 Generating website config…");
  const config = readConfig();
  const cluster = config.network?.cluster || "localnet";
  const deployment = readDeploymentFile(cluster);
  console.log(
    `📖 cluster=${cluster}  deployment entries=${Object.keys(deployment).length}`
  );

  const websiteConfig = generateWebsiteConfig(config, deployment);
  saveWebsiteConfig(websiteConfig, cluster);

  // Quick sanity print — surfaces missing-PDA bugs early.
  const c = websiteConfig[cluster];
  console.log(
    `\n  programs.minebtc           = ${c.programs.minebtc || "—"}`
  );
  console.log(
    `  programs.market            = ${c.programs.market || "—"}`
  );
  console.log(
    `  programs.raydium_cp_swap   = ${c.programs.raydium_cp_swap || "—"}`
  );
  console.log(`  dbtc.mint                  = ${c.dbtc.mint || "—"}`);
  console.log(
    `  raydium_pool.pool_state    = ${c.raydium_pool.pool_state || "—"}`
  );
  console.log(`  game.global_state          = ${c.game.global_state || "—"}`);
  console.log(`  faction_war.config         = ${c.faction_war.config || "—"}`);
  console.log(
    `  hashbeasts.collection      = ${c.hashbeasts.collection || "—"}`
  );
  console.log(
    `  marketplace.inventory_pool = ${c.marketplace.inventory_pool || "—"}`
  );
  console.log(
    `  factions count             = ${c.factions.length}`
  );
  console.log(
    `  lootbox_queues count       = ${c.lootbox_queues.length}`
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  main();
}

export default main;
