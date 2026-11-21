// probe.mjs <vite package dir> <port>: serve page/ with that vite (no config), open it in the oracle's Chromium, print what the dev client changed.
import { join } from "node:path";
const [viteDir, port, defineJson] = process.argv.slice(2);
const vite = await import(join(viteDir, "dist/node/index.js"));
const server = await vite.createServer({ root: join(import.meta.dirname, "page"), configFile: false, define: defineJson ? JSON.parse(defineJson) : undefined, logLevel: "error", server: { port: Number(port), strictPort: true } });
await server.listen();
const { chromium } = await import(join(import.meta.dirname, "../../runtime-oracle-install/node_modules/playwright/index.mjs"));
const browser = await chromium.launch();
const page = await browser.newPage();
const errors = []; page.on("pageerror", (e) => errors.push(String(e)));
await page.goto(`http://localhost:${port}/`);
await page.waitForFunction(() => window.__result, null, { timeout: 15000 });
const result = await page.evaluate(() => window.__result);
const version = (await import(join(viteDir, "package.json"), { with: { type: "json" } })).default.version;
console.log(JSON.stringify({ vite: version, browser: browser.version(), ...result, pageErrors: errors }, null, 1));
await browser.close(); await server.close();
