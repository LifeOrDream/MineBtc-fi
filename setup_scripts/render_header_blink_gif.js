import { chromium } from "playwright";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const frontendRoot = path.resolve(repoRoot, "../mdogeWifBtcFE");
const svgPath = path.resolve(
  frontendRoot,
  "public/game-assets/token/animated/degenbtc-logo-blink.svg"
);
const outDir = path.resolve(__dirname, "tmp/header_blink_frames");

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

const svg = fs.readFileSync(svgPath, "utf8");
const dataUrl = `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
const existingChromiumCandidates = [
  "/Users/sunshinerider/Library/Caches/ms-playwright/chromium_headless_shell-1217/chrome-headless-shell-mac-arm64/chrome-headless-shell",
  "/Users/sunshinerider/Library/Caches/ms-playwright/chromium-1223/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
];
const existingChromium = existingChromiumCandidates.find((candidate) =>
  fs.existsSync(candidate)
);
const launchOptions = existingChromium
  ? { headless: true, executablePath: existingChromium }
  : { headless: true };
const browser = await chromium.launch(launchOptions);
const page = await browser.newPage({
  viewport: { width: 512, height: 512 },
  deviceScaleFactor: 1,
});

await page.setContent(`<!doctype html>
<html>
  <body style="margin:0;background:transparent;">
    <img id="logo" src="${dataUrl}" width="512" height="512" style="width:512px;height:512px;display:block;" />
  </body>
</html>`);
await page.waitForLoadState("networkidle");
await page.locator("#logo").waitFor();

// Capture the same CSS animation used by the website header, with extra samples
// around the blink window so the final GIF preserves the actual motion.
const timesMs = [0, 350, 700, 1050, 1400, 1750, 2100, 2450, 2850, 3040, 3120, 3190, 3260, 3340, 3450, 3600];
let previous = 0;
for (let i = 0; i < timesMs.length; i += 1) {
  const waitMs = timesMs[i] - previous;
  if (waitMs > 0) await page.waitForTimeout(waitMs);
  await page.screenshot({
    path: path.resolve(outDir, `frame-${String(i).padStart(2, "0")}.png`),
    omitBackground: true,
  });
  previous = timesMs[i];
}

await browser.close();
console.log(outDir);
