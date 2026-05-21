// Shared config loader.
//
// Side effect on import: loads .env from the first existing path among
// (repo root, setup_scripts/). Lets every script read secrets via process.env
// without each having to import dotenv itself.
//
// Exports loadConfig() which reads setup_scripts/config.json and substitutes
// ${VAR} placeholders inside rpc_url with matching env vars. Fails fast with
// a clear message if a referenced env var is missing — we never want a
// script to silently use a broken URL.

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import dotenv from "dotenv";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const envCandidates = [
  path.resolve(__dirname, "..", ".env"),
  path.resolve(__dirname, ".env"),
];
for (const p of envCandidates) {
  if (fs.existsSync(p)) dotenv.config({ path: p });
}

function resolveEnvVars(str) {
  if (typeof str !== "string") return str;
  return str.replace(/\$\{([A-Z0-9_]+)\}/g, (full, name) => {
    const v = process.env[name];
    if (v === undefined || v === "") {
      throw new Error(
        `Missing required env var ${name} (referenced in config.json rpc_url). ` +
          `Set it in your shell or in a .env file at repo root or setup_scripts/ ` +
          `before running this script. See .env.example.`,
      );
    }
    return v;
  });
}

export function loadConfig(configPath = path.join(__dirname, "config.json")) {
  const cfg = JSON.parse(fs.readFileSync(configPath, "utf8"));
  if (cfg.network?.rpc_url) {
    cfg.network.rpc_url = resolveEnvVars(cfg.network.rpc_url);
  }
  return cfg;
}
