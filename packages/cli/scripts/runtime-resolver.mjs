#!/usr/bin/env node
// Runtime module resolution through the project's own installed Vite
// (ADR 0220). One JSON request on stdin, one JSON report on stdout.
//
// This loads the project's vite.config and its plugins, which is project code:
// the checker runs it only when asked (`--runtime-resolution required`). It
// uses resolveConfig and a client DevEnvironment's plugin container. No server
// is created or listened on, and dependency optimization is disabled.
//
// Every load gets a row. A row the resolver cannot attest is `unknown`, never
// a guess: a CommonJS `require`, a virtual or query-qualified module, a
// declaration target, or any resolver error.
import { createRequire, isBuiltin } from "node:module";
import { readFile, realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const PROTOCOL = "solid-checker-runtime-resolution-v1";
// Captured before any project module can replace them.
const writeOutput = process.stdout.write.bind(process.stdout);
const stringify = JSON.stringify.bind(JSON);
const processExit = process.exit.bind(process);

async function input() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

function validate(request) {
  if (request.protocol !== PROTOCOL) throw new Error("protocol-mismatch");
  if (!isAbsolute(request.root) || !isAbsolute(request.configFile)) {
    throw new Error("absolute-root-and-config-required");
  }
  if (!Array.isArray(request.loads)) throw new Error("invalid-loads");
  for (const load of request.loads) {
    if (typeof load.id !== "string" || !isAbsolute(load.importer) || typeof load.specifier !== "string") {
      throw new Error("invalid-load");
    }
  }
}

// The project's installed Vite, never the checker's.
async function installedVite(root) {
  const require = createRequire(join(root, "__solid_checker_resolution__.cjs"));
  const manifestPath = require.resolve("vite/package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const entry = manifest.exports?.["."];
  const importEntry = typeof entry === "string" ? entry : entry?.import;
  if (typeof importEntry !== "string" || !importEntry.startsWith("./")) {
    throw new Error("unsupported-vite-export-map");
  }
  const vite = await import(pathToFileURL(resolve(dirname(manifestPath), importEntry)).href);
  const major = Number(String(vite.version).split(".")[0]);
  if (![6, 7, 8].includes(major) || typeof vite.resolveConfig !== "function" || typeof vite.DevEnvironment !== "function") {
    throw new Error(`unsupported-vite-api ${vite.version}`);
  }
  return { vite, major, version: vite.version };
}

async function classify(result) {
  if (!result || typeof result.id !== "string") return { kind: "unknown", reason: "unresolved" };
  const id = result.id;
  if (result.external) {
    return isBuiltin(id) ? { kind: "builtin" } : { kind: "external" };
  }
  if (id.includes("\0") || id.startsWith("virtual:") || id.startsWith("__vite-") || id.includes("?") || id.includes("#")) {
    return { kind: "unknown", reason: "virtual-or-qualified" };
  }
  const path = id.startsWith("file:") ? fileURLToPath(id) : id;
  if (!isAbsolute(path)) return { kind: "unknown", reason: "non-file-id" };
  if (/\.d\.[cm]?ts$/.test(path)) return { kind: "unknown", reason: "declaration-target" };
  try {
    if (!(await stat(path)).isFile()) return { kind: "unknown", reason: "not-a-file" };
    return { kind: "file", path, physicalPath: await realpath(path) };
  } catch {
    return { kind: "unknown", reason: "unreadable" };
  }
}

async function run(request) {
  validate(request);
  const rows = request.loads.map((load) => ({ id: load.id, outcome: { kind: "unknown", reason: "not-answered" } }));
  const report = { protocol: PROTOCOL, status: "error", rows };
  let environment;
  try {
    const { vite, major, version } = await installedVite(request.root);
    report.vite = version;
    const config = await vite.resolveConfig(
      {
        root: request.root,
        configFile: request.configFile,
        mode: request.mode ?? "development",
        logLevel: "silent",
        optimizeDeps: { noDiscovery: true, include: [] },
        server: { watch: null, ws: false, hmr: false, middlewareMode: true, warmup: {} },
      },
      "serve",
      request.mode ?? "development",
    );
    environment = new vite.DevEnvironment("client", config, { hot: false });
    await environment.init();
    // Sequential: resolver plugins may keep importer-dependent state.
    for (const [index, load] of request.loads.entries()) {
      if (load.kind === "require" || load.kind === "import-equals") {
        // A browser bundle has no `require`; whether a plugin rewrites this
        // one is not attested here.
        rows[index].outcome = { kind: "unknown", reason: "commonjs-load" };
        continue;
      }
      try {
        const result = await environment.pluginContainer.resolveId(load.specifier, load.importer, {
          ...(major >= 8 ? { kind: load.kind === "dynamic-import" ? "dynamic-import" : "import-statement" } : {}),
          attributes: {},
          isEntry: false,
          scan: false,
        });
        rows[index].outcome = await classify(result);
      } catch {
        rows[index].outcome = { kind: "unknown", reason: "resolve-error" };
      }
    }
    report.status = "answered";
  } catch (cause) {
    report.failure = String(cause?.message ?? cause).slice(0, 2048);
  } finally {
    try {
      if (environment) await environment.close();
    } catch {
      // The rows already answered stand; closing does not change them.
    }
  }
  return report;
}

try {
  const report = await run(await input());
  writeOutput(stringify(report) + "\n", () => processExit(report.status === "answered" ? 0 : 1));
} catch (cause) {
  writeOutput(stringify({ protocol: PROTOCOL, status: "error", failure: String(cause?.message ?? cause), rows: [] }) + "\n", () => processExit(1));
}
