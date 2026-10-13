// Run original client assertions in an isolated application copy, preserving
// its Vite configuration. Only dev-server/browser/output settings are adapted.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { performance } from "node:perf_hooks";

const [outputArg, browserArg] = process.argv.slice(2), output = resolve(outputArg);
assert(outputArg && browserArg && !existsSync(output), "Supply a fresh output directory and Chromium executable");
const source = resolve("rust/target/app-import-metric/apps/helge-dev"), clone = join(output, "application");
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const excluded = new Set(["node_modules", ".git", "build", "dist", "test-results", "playwright-report"]);
function pins() {
  const result = [];
  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (excluded.has(entry.name) || entry.name.startsWith(".env")) continue;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else { assert(entry.isFile()); result.push({ path: relative(source, path), sha256: hash(readFileSync(path)) }); }
    }
  }
  visit(source); return result.sort((a, b) => a.path.localeCompare(b.path));
}
const inputs = pins(); mkdirSync(clone, { recursive: true });
for (const pin of inputs) { const target = join(clone, pin.path); mkdirSync(dirname(target), { recursive: true }); copyFileSync(join(source, pin.path), target); }
symlinkSync(realpathSync(join(source, "node_modules")), join(clone, "node_modules"), "dir");
const quote = text => "'" + text.replaceAll("'", "'\\''") + "'";
const vite = realpathSync(join(source, "node_modules/vite/bin/vite.js"));
const config = join(clone, "playwright-evaluation.config.mjs");
writeFileSync(config, `import original from './playwright.config.ts';
export default {...original, workers: 1, retries: 0, reporter: [['json', {outputFile: ${JSON.stringify(join(output, "playwright.json"))}}]],
  outputDir: ${JSON.stringify(join(output, "test-results"))},
  projects: [{name: 'chromium', use: {...original.projects[0].use, launchOptions: {executablePath: ${JSON.stringify(realpathSync(browserArg))}}}}],
  webServer: {command: ${JSON.stringify([process.execPath, vite].map(quote).join(" ") + " --host 127.0.0.1 --port 4173 --strictPort")},
    url: 'http://127.0.0.1:4173', reuseExistingServer: false, timeout: 20000}, use: {...original.use, baseURL: 'http://127.0.0.1:4173'}};\n`);
const cli = realpathSync(join(source, "node_modules/@playwright/test/cli.js"));
const args = [cli, "test", "--config", config, "--grep", "has correct title and headings|email icon opens modal|hamburger opens nav links|clicking a nav link in the hamburger menu closes it"];
const began = performance.now();
const child = spawnSync(process.execPath, args, { cwd: clone, encoding: "utf8", timeout: 90000, maxBuffer: 16 * 1024 * 1024 });
writeFileSync(join(output, "stdout.log"), child.stdout ?? ""); writeFileSync(join(output, "stderr.log"), child.stderr ?? "");
assert.deepEqual(pins(), inputs, "Original application changed");
for (const pin of inputs) assert.equal(hash(readFileSync(join(clone, pin.path))), pin.sha256, "Copied source changed");
const raw = existsSync(join(output, "playwright.json")) ? JSON.parse(readFileSync(join(output, "playwright.json"))) : null;
const report = { authority: false, certification: false, source, clone, inputs, command: [process.execPath, ...args],
  status: child.status, signal: child.signal, error: child.error && { code: child.error.code, message: child.error.message },
  elapsedMs: performance.now() - began, stats: raw?.stats ?? null, preservedSource: true,
  scope: "Original home/contact/mobile tests and original Vite configuration; no feedback instrumentation, source repair, seeded defect or browser precision score." };
writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ status: report.status, stats: report.stats, elapsedMs: report.elapsedMs }));
