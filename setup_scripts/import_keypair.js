#!/usr/bin/env node
/**
 * import_keypair.js — convert a base58-encoded private key (from Phantom /
 * Solflare / Backpack exports) into the JSON keypair array format Solana CLI
 * uses, and write it to disk.
 *
 * Usage:
 *   node import_keypair.js <BASE58_PRIVATE_KEY>
 *   node import_keypair.js <BASE58_PRIVATE_KEY> <OUTPUT_PATH>
 *
 * Default output: ../mainnet-wallet-keypair.json (relative to this script).
 *
 * The written file is chmod 600 so only the current user can read it.
 *
 * ⚠️ Anything you paste on the command line lands in your shell history.
 * After a successful import, scrub history: `history -c && rm ~/.zsh_history`
 * (or your shell's equivalent), or set HISTCONTROL=ignorespace and prefix
 * the command with a space so it's never recorded.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import bs58Pkg from "bs58";
import { Keypair } from "@solana/web3.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const bs58 = bs58Pkg.default ?? bs58Pkg;

function die(msg, code = 1) {
  console.error(`❌ ${msg}`);
  process.exit(code);
}

const args = process.argv.slice(2);
if (args.length < 1 || args[0] === "--help" || args[0] === "-h") {
  console.error(
    `Usage: node import_keypair.js <BASE58_PRIVATE_KEY> [OUTPUT_PATH]\n` +
      `\n` +
      `  BASE58_PRIVATE_KEY  the 88-char base58 string Phantom etc. export\n` +
      `  OUTPUT_PATH         optional, default: ../mainnet-wallet-keypair.json`,
  );
  process.exit(args[0] ? 0 : 1);
}

const b58 = args[0].trim();
const outputPath = args[1]
  ? path.resolve(args[1])
  : path.resolve(__dirname, "..", "mainnet-wallet-keypair.json");

let secret;
try {
  secret = bs58.decode(b58);
} catch (err) {
  die(`base58 decode failed: ${err.message || err}`);
}

if (secret.length !== 64) {
  die(
    `expected 64 bytes (Solana secret key = 32 secret + 32 pubkey), got ${secret.length}. ` +
      `Wrong format? Phantom/Solflare 'private key' export should be the right one.`,
  );
}

let kp;
try {
  kp = Keypair.fromSecretKey(secret);
} catch (err) {
  die(`Keypair.fromSecretKey rejected the bytes: ${err.message || err}`);
}

// Refuse to silently overwrite a file unless the existing file represents the
// SAME keypair (idempotent re-runs are fine; overwrites of a different key
// would be a silent footgun on a deployer wallet).
if (fs.existsSync(outputPath)) {
  try {
    const existingArr = JSON.parse(fs.readFileSync(outputPath, "utf8"));
    if (Array.isArray(existingArr) && existingArr.length === 64) {
      const existingKp = Keypair.fromSecretKey(Uint8Array.from(existingArr));
      if (existingKp.publicKey.toString() !== kp.publicKey.toString()) {
        console.error(
          `❌ ${outputPath} already holds a DIFFERENT keypair (${existingKp.publicKey.toString()}).`,
        );
        console.error(
          `   Refusing to overwrite. If you really want to replace it, delete the file first:`,
        );
        console.error(`     rm "${outputPath}"`);
        process.exit(2);
      }
    }
  } catch {
    // Existing file isn't a valid keypair JSON — fall through and overwrite.
  }
}

fs.writeFileSync(outputPath, JSON.stringify(Array.from(secret)));
try {
  fs.chmodSync(outputPath, 0o600);
} catch {
  /* chmod may not be supported on all FS — non-fatal */
}

console.log(`✅ wrote ${outputPath}`);
console.log(`   pubkey: ${kp.publicKey.toString()}`);
console.log(`   perms:  0600 (owner-only read)`);
console.log("");
console.log(
  `Verify:  cd "${path.resolve(__dirname, "..")}" && solana-keygen pubkey ${path.relative(path.resolve(__dirname, ".."), outputPath)}`,
);
