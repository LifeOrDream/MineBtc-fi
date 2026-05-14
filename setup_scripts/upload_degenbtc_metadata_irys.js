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
const uploadDir = path.resolve(__dirname, "tmp/irys_upload");
const receiptPath = path.resolve(__dirname, "tmp/irys_upload_receipt.json");

const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
const wallet = JSON.parse(fs.readFileSync(walletPath, "utf8"));

const gatewayUrl = (id) => `https://gateway.irys.xyz/${id}`;

const irys = await Uploader(Solana)
  .withWallet(wallet)
  .mainnet()
  .withRpc("https://api.mainnet-beta.solana.com");

console.log("Irys uploader:", irys.address);
console.log("Irys token:", irys.token);

async function uploadFile(filePath, contentType) {
  const size = fs.statSync(filePath).size;
  const price = await irys.getPrice(size);
  console.log(
    `Uploading ${path.basename(filePath)} (${size} bytes), price ${irys.utils.fromAtomic(price)} ${irys.token}`
  );
  await irys.fund(price);
  const receipt = await irys.uploadFile(filePath, {
    tags: [{ name: "Content-Type", value: contentType }],
  });
  const id = receipt.id;
  console.log(`Uploaded ${path.basename(filePath)}: ${gatewayUrl(id)}`);
  return { id, url: gatewayUrl(id), size, contentType };
}

async function uploadJson(filePath) {
  const data = fs.readFileSync(filePath);
  const price = await irys.getPrice(data.length);
  console.log(
    `Uploading ${path.basename(filePath)} (${data.length} bytes), price ${irys.utils.fromAtomic(price)} ${irys.token}`
  );
  await irys.fund(price);
  const receipt = await irys.upload(data, {
    tags: [{ name: "Content-Type", value: "application/json" }],
  });
  const id = receipt.id;
  console.log(`Uploaded ${path.basename(filePath)}: ${gatewayUrl(id)}`);
  return { id, url: gatewayUrl(id), size: data.length, contentType: "application/json" };
}

const image = await uploadFile(path.resolve(uploadDir, "btc-logo.png"), "image/png");
const animation = await uploadFile(
  path.resolve(uploadDir, "btc-logo-blink.gif"),
  "image/gif"
);

const metadata = {
  name: config.token.name,
  symbol: config.token.symbol,
  description: config.token.description,
  image: image.url,
  animation_url: animation.url,
  external_url: config.token.external_url,
  properties: {
    category: "image",
    files: [
      { uri: image.url, type: "image/png" },
      { uri: animation.url, type: "image/gif" },
    ],
  },
};

const metadataPath = path.resolve(uploadDir, "btc.json");
fs.writeFileSync(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`);
const metadataUpload = await uploadJson(metadataPath);

const receipt = {
  uploaded_at: new Date().toISOString(),
  uploader: irys.address,
  token: irys.token,
  image,
  animation,
  metadata: metadataUpload,
};

fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log("Receipt saved:", receiptPath);
console.log("Metadata URI:", metadataUpload.url);
