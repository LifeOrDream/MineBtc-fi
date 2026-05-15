import { Uploader } from "@irys/upload";
import { Solana } from "@irys/upload-solana";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const repoRoot = path.resolve(__dirname, "..");
const configPath = path.resolve(__dirname, "config.json");
const walletPath = path.resolve(repoRoot, "mainnet-irys-upload-wallet-keypair.json");
const outputDir = path.resolve(__dirname, "tmp/hashbeasts_collection_metadata");
const receiptPath = path.resolve(outputDir, "irys_receipt.json");

const args = new Set(process.argv.slice(2));
const shouldUpload = args.has("--upload");

const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
const hashbeasts = config.hashbeasts;
const deploymentPath = path.resolve(
  __dirname,
  "deployments",
  `${config.network.cluster}.json`
);
const deployment = fs.existsSync(deploymentPath)
  ? JSON.parse(fs.readFileSync(deploymentPath, "utf8"))
  : {};

const gatewayUrl = (id) => `https://gateway.irys.xyz/${id}`;

fs.mkdirSync(outputDir, { recursive: true });

async function downloadFile(url, filePath) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  fs.writeFileSync(filePath, bytes);
  return bytes.length;
}

async function buildMetadata(imageUrl, animationUrl = null) {
  const programId = deployment.MINE_BTC_PROGRAM_ID;
  if (!programId) {
    throw new Error(
      `Missing MINE_BTC_PROGRAM_ID in ${deploymentPath}. Deploy the MineBTC program before final collection metadata upload.`
    );
  }
  const { PublicKey } = await import("@solana/web3.js");
  const [inventorySweepVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("inventory-sweep-vault")],
    new PublicKey(programId)
  );
  const creatorShare = (identifier, fallback) =>
    config.hashbeasts_config.creators?.find(
      (creator) => creator.identifier === identifier
    )?.percentage ?? fallback;
  const inventorySweepShare = creatorShare("inventory_sweep_vault", 50);
  const multisigShare = creatorShare("multisig_fee_recipient", 50);

  const files = [
    {
      uri: imageUrl,
      type: "image/png",
    },
  ];
  if (animationUrl) {
    files.push({
      uri: animationUrl,
      type: "image/gif",
    });
  }

  return {
    name: hashbeasts.collection_name,
    symbol: hashbeasts.collection_symbol,
    description: hashbeasts.collection_description,
    seller_fee_basis_points: config.hashbeasts_config.royalties,
    image: imageUrl,
    ...(animationUrl ? { animation_url: animationUrl } : {}),
    external_url: config.token.external_url,
    collection: {
      name: hashbeasts.collection_name,
      family: "MineBTC",
    },
    attributes: [],
    properties: {
      category: "image",
      files,
      creators: [
        {
          address: inventorySweepVaultPDA.toString(),
          share: inventorySweepShare,
        },
        {
          address: config.deployment.FEE_RECIPIENT_MULTISIG,
          share: multisigShare,
        },
      ],
    },
  };
}

function validateCreatorShares(metadata) {
  const creators = metadata.properties?.creators || [];
  const total = creators.reduce((sum, creator) => sum + Number(creator.share || 0), 0);
  if (creators.length === 0 || total !== 100) {
    throw new Error(`Invalid creator shares: expected total 100, got ${total}`);
  }
  for (const creator of creators) {
    if (!creator.address || creator.address.length < 32) {
      throw new Error(`Invalid creator address in metadata: ${creator.address}`);
    }
  }
}

async function uploadFile(irys, filePath, contentType) {
  const size = fs.statSync(filePath).size;
  const price = await irys.getPrice(size);
  console.log(
    `Uploading ${path.basename(filePath)} (${size} bytes), price ${irys.utils.fromAtomic(price)} ${irys.token}`
  );
  await irys.fund(price);
  const receipt = await irys.uploadFile(filePath, {
    tags: [{ name: "Content-Type", value: contentType }],
  });
  return { id: receipt.id, url: gatewayUrl(receipt.id), size, contentType };
}

async function uploadJson(irys, filePath) {
  const data = fs.readFileSync(filePath);
  const price = await irys.getPrice(data.length);
  console.log(
    `Uploading ${path.basename(filePath)} (${data.length} bytes), price ${irys.utils.fromAtomic(price)} ${irys.token}`
  );
  await irys.fund(price);
  const receipt = await irys.upload(data, {
    tags: [{ name: "Content-Type", value: "application/json" }],
  });
  return {
    id: receipt.id,
    url: gatewayUrl(receipt.id),
    size: data.length,
    contentType: "application/json",
  };
}

const localImagePath = path.resolve(outputDir, "hashbeasts-collection.png");
const localAnimationPath = path.resolve(outputDir, "hashbeasts-collection-animation.gif");
const localJsonPath = path.resolve(outputDir, "hashbeasts-collection.json");

const downloadedSize = await downloadFile(hashbeasts.collection_image, localImagePath);
console.log(`Prepared image: ${localImagePath} (${downloadedSize} bytes)`);
let downloadedAnimationSize = null;
if (hashbeasts.collection_animation) {
  downloadedAnimationSize = await downloadFile(hashbeasts.collection_animation, localAnimationPath);
  console.log(`Prepared animation: ${localAnimationPath} (${downloadedAnimationSize} bytes)`);
}

if (!shouldUpload) {
  const metadata = await buildMetadata(
    hashbeasts.collection_image,
    hashbeasts.collection_animation || null
  );
  validateCreatorShares(metadata);
  fs.writeFileSync(localJsonPath, `${JSON.stringify(metadata, null, 2)}\n`);
  console.log(`Dry run JSON: ${localJsonPath}`);
  console.log("Run with --upload to upload image + animation + JSON to Irys and update config.hashbeasts collection URLs.");
  console.log(JSON.stringify(metadata, null, 2));
  process.exit(0);
}

const wallet = JSON.parse(fs.readFileSync(walletPath, "utf8"));
const irys = await Uploader(Solana)
  .withWallet(wallet)
  .mainnet()
  .withRpc("https://api.mainnet-beta.solana.com");

console.log("Irys uploader:", irys.address);
console.log("Irys token:", irys.token);

const image = await uploadFile(irys, localImagePath, "image/png");
const animation = hashbeasts.collection_animation
  ? await uploadFile(irys, localAnimationPath, "image/gif")
  : null;
const metadata = await buildMetadata(image.url, animation?.url || null);
validateCreatorShares(metadata);
fs.writeFileSync(localJsonPath, `${JSON.stringify(metadata, null, 2)}\n`);
const metadataUpload = await uploadJson(irys, localJsonPath);

config.hashbeasts.collection_image = image.url;
if (animation) {
  config.hashbeasts.collection_animation = animation.url;
}
config.hashbeasts.collection_uri = metadataUpload.url;
fs.writeFileSync(configPath, `${JSON.stringify(config, null, 4)}\n`);

const receipt = {
  uploaded_at: new Date().toISOString(),
  uploader: irys.address,
  token: irys.token,
  image,
  ...(animation ? { animation } : {}),
  metadata: metadataUpload,
  config_updated: {
    collection_image: image.url,
    ...(animation ? { collection_animation: animation.url } : {}),
    collection_uri: metadataUpload.url,
  },
};

fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log("Receipt saved:", receiptPath);
console.log("Collection image:", image.url);
if (animation) {
  console.log("Collection animation:", animation.url);
}
console.log("Collection URI:", metadataUpload.url);
