#!/usr/bin/env node
/**
 * audit_authorities.js — read every authority/admin/recipient field on chain
 * and flag anything that's still the deployer wallet (or anything that's not
 * the expected Squads multisig).
 */

import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import anchorPkg from "@coral-xyz/anchor";
const { AnchorProvider, Program, Wallet } = anchorPkg;
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { setIdlAddress } from "./raydium_id_sync.js";
import { loadConfig } from "./load_config.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const config = loadConfig(path.join(__dirname, "config.json"));
const cluster = config.network.cluster;
const deployment = JSON.parse(fs.readFileSync(path.join(__dirname, "deployments", `${cluster}.json`), "utf8"));
const minebtcIdl = setIdlAddress(
  JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.minebtc_idl), "utf8")),
  deployment.MINE_BTC_PROGRAM_ID,
);
const marketIdl = setIdlAddress(
  JSON.parse(fs.readFileSync(path.resolve(__dirname, config.deployment.paths.degenbtc_market_idl), "utf8")),
  deployment.DEGENBTC_MARKET_PROGRAM_ID,
);

const connection = new Connection(config.network.rpc_url, config.network.commitment);
const provider = new AnchorProvider(connection, new Wallet(Keypair.generate()), { commitment: config.network.commitment });
const minebtc = new Program(minebtcIdl, provider);
const market = new Program(marketIdl, provider);

const DEPLOYER = new PublicKey("7zXuQt1gYax8fu1ahbsdXe3g1FtVQWkwm55KiF9EQSfX");
const SQUADS   = new PublicKey("ixQ7TC6ENzVV8Qzdi6xUu5Pm6Pefg42772cqq8EfjBE");

function classify(pk) {
  if (!pk) return "❓ null";
  const s = pk.toBase58();
  if (s === DEPLOYER.toBase58()) return "🟥 DEPLOYER";
  if (s === SQUADS.toBase58()) return "🟩 SQUADS";
  if (s === "11111111111111111111111111111111") return "⬜ none/sentinel";
  return `🟦 OTHER (${s})`;
}

function row(label, pk) {
  const s = pk?.toBase58?.() ?? String(pk);
  console.log(`  ${label.padEnd(38)} ${classify(pk).padEnd(15)} ${s}`);
}

(async () => {
  console.log(`\n=== Audit @ ${cluster} ===\n`);
  console.log(`Deployer wallet : ${DEPLOYER.toBase58()}`);
  console.log(`Squads multisig : ${SQUADS.toBase58()}\n`);

  // --- BPF upgrade authorities ---
  console.log("─── BPF upgrade authorities ───");
  for (const [name, pid] of [
    ["MineBTC program", new PublicKey(deployment.MINE_BTC_PROGRAM_ID)],
    ["Marketplace program", new PublicKey(deployment.DEGENBTC_MARKET_PROGRAM_ID)],
  ]) {
    const programAccount = await connection.getAccountInfo(pid);
    if (!programAccount) { console.log(`  ${name}: not found`); continue; }
    const programDataAddr = new PublicKey(programAccount.data.slice(4, 36));
    const programDataAccount = await connection.getAccountInfo(programDataAddr);
    if (!programDataAccount) { console.log(`  ${name}: programdata missing`); continue; }
    // ProgramData layout: 4 (state) + 8 (slot) + 1 (has_authority) + 32 (authority)
    const hasAuthority = programDataAccount.data[12] === 1;
    if (!hasAuthority) { row(name, null); continue; }
    const authPk = new PublicKey(programDataAccount.data.slice(13, 45));
    row(name, authPk);
  }

  // --- MineBTC GlobalConfig ---
  console.log("\n─── MineBTC GlobalConfig ───");
  const [globalConfigPda] = PublicKey.findProgramAddressSync([Buffer.from("global-config")], minebtc.programId);
  const gc = await minebtc.account.globalConfig.fetch(globalConfigPda);
  row("ext_authority", gc.extAuthority);
  row("pending_authority", gc.pendingAuthority);
  row("fee_recipient", gc.feeRecipient);

  // --- MineBTC TaxConfig (PDAs are program-owned, but withdraw_withheld_authority might be relevant) ---
  console.log("\n─── MineBTC TaxConfig ───");
  const [taxConfigPda] = PublicKey.findProgramAddressSync([Buffer.from("tax-config")], minebtc.programId);
  try {
    const tc = await minebtc.account.taxConfig.fetch(taxConfigPda);
    row("withdraw_withheld_authority", tc.withdrawWithheldAuthority);
    row("faction_treasury_vault", tc.factionTreasuryVault);
  } catch (e) { console.log(`  TaxConfig: ${e.message.split("\n")[0]}`); }

  // --- Marketplace MarketplaceConfig ---
  console.log("\n─── Marketplace MarketplaceConfig ───");
  const marketplaceConfigAddr = deployment.inventory_pool_initialized?.marketplace_config
    ?? deployment.degenbtc_marketplace_initialized?.marketplace_config_pda;
  if (!marketplaceConfigAddr) {
    console.log("  ⚠️  marketplace_config PDA not in deployment.json");
  } else {
    const mcPda = new PublicKey(marketplaceConfigAddr);
    try {
      const mc = await market.account.marketplaceConfig.fetch(mcPda);
      row("admin", mc.admin);
      row("fee_recipient", mc.feeRecipient);
      row("collection_mint", mc.collectionMint);
      console.log(`  enabled=${mc.enabled}  fee_bps=${mc.feeBps}  min_price_lamports=${mc.minPriceLamports.toString()}`);
    } catch (e) { console.log(`  MarketplaceConfig (${mcPda.toBase58()}): ${e.message.split("\n")[0]}`); }
  }

  // --- dBTC mint Token-2022 authorities ---
  console.log("\n─── dBTC Token-2022 mint ───");
  const dbtcMint = new PublicKey(deployment.dbtc_mint_address);
  const mintAcc = await connection.getAccountInfo(dbtcMint);
  if (!mintAcc) { console.log("  dBTC mint not found"); }
  else {
    // Token-2022 Mint layout: 0..36 = mint_authority Option<Pubkey>
    // Offset 0: COption discriminator (4 bytes), then 32 bytes pubkey if Some
    // 4..36: mint_authority pk (if 0..4 == 1)
    // 36..44: supply u64
    // 44: decimals
    // 45: is_initialized
    // 46..50: COption disc for freeze_authority
    // 50..82: freeze_authority pk
    const hasMintAuth = mintAcc.data.readUInt32LE(0) === 1;
    if (hasMintAuth) row("mint_authority", new PublicKey(mintAcc.data.slice(4, 36)));
    else console.log("  mint_authority                      🟩 removed (None)");
    const hasFreeze = mintAcc.data.readUInt32LE(46) === 1;
    if (hasFreeze) row("freeze_authority", new PublicKey(mintAcc.data.slice(50, 82)));
    else console.log("  freeze_authority                    🟩 none");
  }

  console.log("\n=== Done ===\n");
})().catch((e) => {
  console.error("❌ FATAL:", e.message);
  process.exit(1);
});
