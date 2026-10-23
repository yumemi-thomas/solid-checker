// The runtime resolver worker (ADR 0220) against a fake `vite` package: what it
// asks Vite for, and which answers it attests. The real Vite is never loaded;
// the fake records every call and answers from a table the test writes.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, test } from "vitest";

const worker = new URL("../scripts/runtime-resolver.mjs", import.meta.url).pathname;
const PROTOCOL = "solid-checker-runtime-resolution-v1";
const scratch = mkdtempSync(join(tmpdir(), "solid-runtime-worker-"));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

const FAKE_VITE = `
import { appendFileSync, readFileSync } from "node:fs";
const log = (entry) => appendFileSync(process.env.FAKE_VITE_LOG, JSON.stringify(entry) + "\\n");
export const version = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version;
export async function resolveConfig(inline, command, mode) {
  log({ call: "resolveConfig", inline, command, mode, nodeEnv: process.env.NODE_ENV ?? null });
  if (process.env.FAKE_VITE_CONFIG_FAILS) throw new Error("config failed: " + process.env.FAKE_VITE_CONFIG_FAILS);
  return { fake: true };
}
export class DevEnvironment {
  constructor(name, config, options) {
    log({ call: "DevEnvironment", name, config, options });
    this.pluginContainer = new Proxy({
      resolveId: async (specifier, importer, options) => {
        log({ call: "resolveId", specifier, importer, options });
        const answer = JSON.parse(readFileSync(process.env.FAKE_VITE_ANSWERS, "utf8"))[specifier];
        if (answer === "throw") throw new Error("plugin failed");
        return answer ?? null;
      }
    }, { get: (target, name) => (name in target ? target[name] : (log({ call: "unexpected", name: String(name) }), undefined)) });
  }
  async init() { log({ call: "init" }); }
  async close() { log({ call: "close" }); }
}
`;

let counter = 0;
/** A project whose installed Vite is the fake, of version `version`. */
function project(version = "8.0.0") {
  const root = join(scratch, `project-${counter++}`);
  const vite = join(root, "node_modules", "vite");
  mkdirSync(vite, { recursive: true });
  writeFileSync(join(vite, "package.json"), JSON.stringify({ name: "vite", version, type: "module", exports: { ".": { import: "./index.mjs" }, "./package.json": "./package.json" } }));
  writeFileSync(join(vite, "index.mjs"), FAKE_VITE);
  writeFileSync(join(root, "vite.config.ts"), "export default {};\n");
  return { root, log: join(root, "vite.log"), answers: join(root, "answers.json") };
}

function run(fixture, request, answers = {}, env = {}) {
  writeFileSync(fixture.answers, JSON.stringify(answers));
  writeFileSync(fixture.log, "");
  const result = spawnSync(process.env.SOLID_CHECKER_PROBE_NODE ?? "node", [worker], {
    cwd: fixture.root,
    input: JSON.stringify(request),
    encoding: "utf8",
    env: { ...process.env, FAKE_VITE_LOG: fixture.log, FAKE_VITE_ANSWERS: fixture.answers, ...env }
  });
  const calls = readFileSync(fixture.log, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
  return { status: result.status, report: JSON.parse(result.stdout.trim().split("\n").at(-1)), calls };
}

/** The request the Rust side sends (`runtime_resolution::request`), minus loads. */
function request(fixture, loads, extra = {}) {
  return {
    protocol: PROTOCOL,
    root: fixture.root,
    configFile: join(fixture.root, "vite.config.ts"),
    mode: "development",
    loads: loads.map((load, index) => ({ id: String(index), importer: join(fixture.root, "main.ts"), kind: "static-import", ...load })),
    ...extra
  };
}

test("it asks a client dev environment to resolveId, never to load or transform", () => {
  const fixture = project("8.0.0");
  const { status, report, calls } = run(
    fixture,
    request(fixture, [
      { specifier: "a", kind: "static-import" },
      { specifier: "b", kind: "export-from" },
      { specifier: "c", kind: "dynamic-import" }
    ])
  );
  assert.equal(status, 0);
  assert.equal(report.status, "answered");
  assert.equal(report.vite, "8.0.0");
  const [config] = calls.filter((call) => call.call === "resolveConfig");
  assert.equal(config.command, "serve", "a dev-server config, never a build");
  assert.equal(config.mode, "development");
  assert.equal(config.inline.mode, "development");
  assert.equal(config.inline.root, fixture.root);
  assert.equal(config.inline.configFile, join(fixture.root, "vite.config.ts"));
  assert.deepEqual(config.inline.optimizeDeps, { noDiscovery: true, include: [] });
  const [environment] = calls.filter((call) => call.call === "DevEnvironment");
  assert.equal(environment.name, "client", "no ssr or other environment");
  assert.deepEqual(environment.options, { hot: false });
  assert.deepEqual(
    calls.map((call) => call.call),
    ["resolveConfig", "DevEnvironment", "init", "resolveId", "resolveId", "resolveId", "close"],
    "resolveId only: no load, transform or unexpected plugin-container access"
  );
  const resolves = calls.filter((call) => call.call === "resolveId");
  assert.deepEqual(resolves.map((call) => call.specifier), ["a", "b", "c"], "sequential, in load order");
  assert.deepEqual(
    resolves.map((call) => call.options),
    [
      { kind: "import-statement", attributes: {}, isEntry: false, scan: false },
      { kind: "import-statement", attributes: {}, isEntry: false, scan: false },
      { kind: "dynamic-import", attributes: {}, isEntry: false, scan: false }
    ]
  );
});

test("the mode is the request's, defaulting to development; Rust sends development", () => {
  const fixture = project();
  const production = run(fixture, request(fixture, [], { mode: "production" }));
  assert.equal(production.calls.find((call) => call.call === "resolveConfig").mode, "production");
  const absent = request(fixture, []);
  delete absent.mode;
  const defaulted = run(fixture, absent);
  assert.equal(defaulted.calls.find((call) => call.call === "resolveConfig").mode, "development");
  // The Rust request is pinned by `runtime_resolution::tests`; the worker has
  // no other source of a mode (no `--runtime-build`, no `ssr`).
});

test("the worker inherits NODE_ENV: Vite's development|production condition follows it, not the mode", () => {
  // Vite reads `isProduction` from NODE_ENV once the config resolves, so a
  // checker run under NODE_ENV=production resolves `exports` conditions as a
  // production build although the request says development. Pinned as the
  // current behaviour (docs/decisions.md, ADR 0284 amendment), not endorsed.
  const fixture = project();
  const { calls } = run(fixture, request(fixture, []), {}, { NODE_ENV: "production" });
  const config = calls.find((call) => call.call === "resolveConfig");
  assert.equal(config.mode, "development");
  assert.equal(config.nodeEnv, "production");
});

test("Vite before 8 is not told the import kind", () => {
  for (const version of ["6.4.1", "7.1.0"]) {
    const fixture = project(version);
    const { report, calls } = run(fixture, request(fixture, [{ specifier: "a", kind: "dynamic-import" }]));
    assert.equal(report.status, "answered", version);
    assert.deepEqual(calls.find((call) => call.call === "resolveId").options, { attributes: {}, isEntry: false, scan: false }, version);
  }
});

test("every answer the worker cannot attest is unknown, with its reason", () => {
  const fixture = project();
  const real = join(fixture.root, "real.ts");
  writeFileSync(real, "export {};\n");
  const link = join(fixture.root, "link.ts");
  symlinkSync(real, link);
  mkdirSync(join(fixture.root, "directory"));
  const declaration = join(fixture.root, "types.d.ts");
  writeFileSync(declaration, "export {};\n");
  const answers = {
    file: { id: link },
    "file-url": { id: new URL(`file://${real}`).href },
    builtin: { id: "node:fs", external: true },
    package: { id: "left-pad", external: true },
    qualified: { id: `${real}?raw` },
    fragment: { id: `${real}#x` },
    virtual: { id: "virtual:module" },
    nul: { id: "\0plugin" },
    internal: { id: "__vite-browser-external" },
    relative: { id: "relative/x.js" },
    declaration: { id: declaration },
    directory: { id: join(fixture.root, "directory") },
    absent: { id: join(fixture.root, "absent.ts") },
    unresolved: null,
    explodes: "throw"
  };
  const specifiers = [...Object.keys(answers), "required", "equals"];
  const { status, report } = run(
    fixture,
    request(fixture, specifiers.map((specifier) => ({
      specifier,
      kind: specifier === "required" ? "require" : specifier === "equals" ? "import-equals" : "static-import"
    }))),
    answers
  );
  assert.equal(status, 0);
  const outcomes = Object.fromEntries(report.rows.map((row, index) => [specifiers[index], row.outcome]));
  assert.deepEqual(outcomes, {
    file: { kind: "file", path: link, physicalPath: realpathSync(real) },
    "file-url": { kind: "file", path: real, physicalPath: realpathSync(real) },
    builtin: { kind: "builtin" },
    package: { kind: "external" },
    qualified: { kind: "unknown", reason: "virtual-or-qualified" },
    fragment: { kind: "unknown", reason: "virtual-or-qualified" },
    virtual: { kind: "unknown", reason: "virtual-or-qualified" },
    nul: { kind: "unknown", reason: "virtual-or-qualified" },
    internal: { kind: "unknown", reason: "virtual-or-qualified" },
    relative: { kind: "unknown", reason: "non-file-id" },
    declaration: { kind: "unknown", reason: "declaration-target" },
    directory: { kind: "unknown", reason: "not-a-file" },
    absent: { kind: "unknown", reason: "unreadable" },
    unresolved: { kind: "unknown", reason: "unresolved" },
    explodes: { kind: "unknown", reason: "resolve-error" },
    required: { kind: "unknown", reason: "commonjs-load" },
    equals: { kind: "unknown", reason: "commonjs-load" }
  });
});

test("a require or import-equals load never reaches the plugin container", () => {
  const fixture = project();
  const { calls } = run(fixture, request(fixture, [{ specifier: "a", kind: "require" }, { specifier: "b", kind: "import-equals" }]));
  assert.equal(calls.filter((call) => call.call === "resolveId").length, 0);
});

test("any failure answers nothing: every row stays not-answered and the run exits 1", () => {
  const unsupported = project("5.4.0");
  const old = run(unsupported, request(unsupported, [{ specifier: "a" }]));
  assert.equal(old.status, 1);
  assert.equal(old.report.status, "error");
  assert.match(old.report.failure, /unsupported-vite-api 5\.4\.0/);
  assert.deepEqual(old.report.rows, [{ id: "0", outcome: { kind: "unknown", reason: "not-answered" } }]);
  assert.deepEqual(old.calls, [], "an unsupported major is never configured");

  const broken = project();
  const failed = run(broken, request(broken, [{ specifier: "a" }, { specifier: "b" }]), { a: { id: "/x" } }, { FAKE_VITE_CONFIG_FAILS: "plugin threw" });
  assert.equal(failed.status, 1);
  assert.match(failed.report.failure, /config failed: plugin threw/);
  assert.deepEqual(
    failed.report.rows.map((row) => row.outcome),
    [{ kind: "unknown", reason: "not-answered" }, { kind: "unknown", reason: "not-answered" }]
  );
});

test("a malformed request is refused before any project code runs", () => {
  const fixture = project();
  for (const [broken, failure] of [
    [{ protocol: "other" }, /protocol-mismatch/],
    [{ root: "relative" }, /absolute-root-and-config-required/],
    [{ loads: "none" }, /invalid-loads/],
    [{ loads: [{ id: "0", importer: "relative.ts", specifier: "a" }] }, /invalid-load/]
  ]) {
    const { status, report, calls } = run(fixture, { ...request(fixture, []), ...broken });
    assert.equal(status, 1);
    assert.match(report.failure, failure);
    assert.deepEqual(report.rows, []);
    assert.deepEqual(calls, []);
  }
});
