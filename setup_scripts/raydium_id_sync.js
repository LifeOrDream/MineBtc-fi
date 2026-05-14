import fs from "fs";
import path from "path";
import { PublicKey } from "@solana/web3.js";

export function readJsonIfExists(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

export function resolveRaydiumProgramId(config, deploymentData, options = {}) {
  const requireCustomDeployment = options.requireCustomDeployment ?? true;

  if (config.raydium?.use_official_program) {
    if (!config.raydium.program_id) {
      throw new Error("config.raydium.program_id is required when use_official_program=true");
    }
    return new PublicKey(config.raydium.program_id).toBase58();
  }

  const deployedProgramId =
    deploymentData?.RAYDIUM_CP_PROGRAM_ID ||
    deploymentData?.raydium_amm_config_created?.raydium_program_id;
  if (deployedProgramId) {
    return new PublicKey(deployedProgramId).toBase58();
  }

  if (!requireCustomDeployment && config.raydium?.program_id) {
    return new PublicKey(config.raydium.program_id).toBase58();
  }

  throw new Error(
    "Custom Raydium is enabled, but deployments/{cluster}.json has no RAYDIUM_CP_PROGRAM_ID. Run setup_scripts/0_deploy_raydium.js first."
  );
}

export function setIdlAddress(idl, programId) {
  const normalized = new PublicKey(programId).toBase58();
  idl.address = normalized;
  if (idl.metadata) {
    idl.metadata.address = normalized;
  }
  return idl;
}

function replaceAnchorProgramAddress(anchorToml, cluster, programName, programId) {
  const sectionRegex = new RegExp(
    `(\\[programs\\.${cluster}\\][\\s\\S]*?)(?=\\n\\[|$)`,
    "i"
  );
  const programRegex = new RegExp(`^(\\s*)${programName}\\s*=\\s*"[^"]*"`, "m");
  const sectionMatch = anchorToml.match(sectionRegex);

  if (!sectionMatch) {
    return `${anchorToml.trimEnd()}\n\n[programs.${cluster}]\n${programName} = "${programId}"\n`;
  }

  const section = sectionMatch[1];
  const updatedSection = programRegex.test(section)
    ? section.replace(programRegex, `$1${programName} = "${programId}"`)
    : `${section.trimEnd()}\n${programName} = "${programId}"`;

  return anchorToml.replace(section, updatedSection);
}

function updateFileIfChanged(filePath, nextContent, label) {
  const current = fs.existsSync(filePath) ? fs.readFileSync(filePath, "utf8") : "";
  if (current === nextContent) {
    return false;
  }
  fs.writeFileSync(filePath, nextContent);
  console.log(`\x1b[32m  ✅ Synced ${label}\x1b[0m`);
  return true;
}

function syncIdlFile(idlPath, programId) {
  if (!fs.existsSync(idlPath)) {
    return;
  }

  try {
    const idl = readJsonIfExists(idlPath);
    setIdlAddress(idl, programId);
    updateFileIfChanged(idlPath, JSON.stringify(idl, null, 2), `IDL ${path.relative(process.cwd(), idlPath)}`);
  } catch (error) {
    console.log(`\x1b[33m  ⚠️ Could not sync Raydium IDL ${idlPath}: ${error.message}\x1b[0m`);
  }
}

export function syncRaydiumProgramId({
  rootDir,
  setupDir,
  config,
  programId,
  deployerPubkey,
  updateConfig = false,
}) {
  const normalizedProgramId = new PublicKey(programId).toBase58();
  const cluster = config.network?.cluster || "localnet";
  const raydiumLibPath = path.join(rootDir, "raydium", "programs", "cp-swap", "src", "lib.rs");
  const raydiumAnchorTomlPath = path.join(rootDir, "raydium", "Anchor.toml");
  const rootAnchorTomlPath = path.join(rootDir, "Anchor.toml");

  console.log(`\x1b[36m🔗 Syncing Raydium program id for ${cluster}: ${normalizedProgramId}\x1b[0m`);

  if (fs.existsSync(raydiumLibPath)) {
    let libContent = fs.readFileSync(raydiumLibPath, "utf8");
    libContent = libContent.replace(
      /declare_id!\("([^"]+)"\);/g,
      `declare_id!("${normalizedProgramId}");`
    );

    if (deployerPubkey) {
      const adminRegex =
        /(pub mod admin \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\(")([^"]+)("\);)/;
      libContent = libContent.replace(adminRegex, `$1${deployerPubkey}$3`);

      const feeReceiverRegex =
        /(pub mod create_pool_fee_reveiver \{[\s\S]*?#\[cfg\(feature = "devnet"\)\]\s*pub const ID: Pubkey = pubkey!\(")([^"]+)("\);)/;
      libContent = libContent.replace(feeReceiverRegex, `$1${deployerPubkey}$3`);
    }

    updateFileIfChanged(raydiumLibPath, libContent, "Raydium declare_id/admin ids");
  }

  if (fs.existsSync(raydiumAnchorTomlPath)) {
    const current = fs.readFileSync(raydiumAnchorTomlPath, "utf8");
    const next = current.replace(
      /raydium_cp_swap\s*=\s*"([^"]+)"/g,
      `raydium_cp_swap = "${normalizedProgramId}"`
    );
    updateFileIfChanged(raydiumAnchorTomlPath, next, "raydium/Anchor.toml");
  }

  if (fs.existsSync(rootAnchorTomlPath)) {
    const current = fs.readFileSync(rootAnchorTomlPath, "utf8");
    const next = replaceAnchorProgramAddress(
      current,
      cluster,
      "raydium_cp_swap",
      normalizedProgramId
    );
    updateFileIfChanged(rootAnchorTomlPath, next, "root Anchor.toml Raydium id");
  }

  for (const idlPath of [
    path.join(rootDir, "target", "idl", "raydium_cp_swap.json"),
    path.join(rootDir, "raydium", "target", "idl", "raydium_cp_swap.json"),
  ]) {
    syncIdlFile(idlPath, normalizedProgramId);
  }

  if (updateConfig) {
    const configPath = path.join(setupDir, "config.json");
    const nextConfig = { ...config, raydium: { ...config.raydium, program_id: normalizedProgramId } };
    updateFileIfChanged(configPath, `${JSON.stringify(nextConfig, null, 4)}\n`, "setup_scripts/config.json raydium.program_id");
  }

  return normalizedProgramId;
}
