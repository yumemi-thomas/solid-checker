// Exercise unchanged application source at two precise integration boundaries.
// This is transform/SSR helper evidence, not a full application browser run.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";

const [outputArg] = process.argv.slice(2), output = resolve(outputArg);
assert(outputArg && !existsSync(output), "Supply a fresh result path");
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const base = resolve("rust/target/app-import-metric/apps"), helge = join(base, "helge-dev"), oscar = join(base, "oscartbeaumont-website");
const require = createRequire(join(helge, "package.json"));
const vitePath = require.resolve("vite"), pluginPath = require.resolve("@solidjs/vite-plugin");
const { createServer } = await import(pathToFileURL(vitePath));
// The package's ESM entry avoids its older require('vite') fallback.
const pluginRoot = realpathSync(join(helge, "node_modules/@solidjs/vite-plugin"));
const { default: solid } = await import(pathToFileURL(join(pluginRoot, "dist/esm/index.mjs")));
const icons = realpathSync(join(helge, "node_modules/solid-icons/lib/index.js"));
const helper = join(oscar, "src/routes/invoicer/util.ts");
const sources = [icons, helper, join(helge, "vite.config.ts"), join(oscar, "src/routes/invoicer/index.tsx")]
  .map(path => ({ path, sha256: hash(readFileSync(path)) }));
const report = { authority: false, certification: false, sources, tooling: { vitePath, pluginPath }, results: [] };
for (const alias of [false, true]) {
  const start = performance.now();
  const server = await createServer({ configFile: false, root: helge, publicDir: false,
    plugins: [solid({ hot: false })], resolve: { alias: alias ? { "solid-js/web": "@solidjs/web" } : {} },
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { middlewareMode: true, fs: { allow: [helge] } }, logLevel: "silent" });
  const row = { boundary: "precompiled-icon-browser-artifact", module: icons, originalAliasPresent: alias,
    scope: "Explicit transform of the installed browser artifact; does not prove that the failed CLI selected this artifact." };
  try { row.transformed = !!(await server.transformRequest("/@fs" + icons)); }
  catch (error) { row.error = error.message; }
  finally { await server.close(); }
  row.elapsedMs = performance.now() - start; report.results.push(row);
}
// Load the application's actual mutable wrapper under its SSR branch. Mutate
// through a props object exactly as its components do; no synthetic readonly
// replacement, copied wrapper implementation, or stub supplies the behavior.
const server = await createServer({ configFile: false, root: oscar, publicDir: false,
  server: { middlewareMode: true }, optimizeDeps: { noDiscovery: true, include: [] }, logLevel: "silent" });
try {
  const { createMutableLocalStorage } = await server.ssrLoadModule("/src/routes/invoicer/util.ts");
  const props = { state: createMutableLocalStorage("evaluation-only", { invoiceId: 0, client: { name: "before" } }) };
  props.state.client.name = "after";
  props.state.invoiceId++;
  assert.equal(props.state.client.name, "after"); assert.equal(props.state.invoiceId, 1);
  report.results.push({ boundary: "nested-props-mutable-wrapper", helper, mode: "SSR helper execution; no browser lifecycle or DOM update proof",
    writesRetained: true, name: props.state.client.name, invoiceId: props.state.invoiceId });
} finally { await server.close(); }
for (const pin of sources) assert.equal(hash(readFileSync(pin.path)), pin.sha256, "Original source changed");
report.aliasContrast = !!report.results[0].error && !!report.results[1].transformed;
report.preservedSource = true;
writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report.results));
