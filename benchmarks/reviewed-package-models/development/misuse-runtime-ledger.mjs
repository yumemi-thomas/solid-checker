// Run every misuse-ledger case (fixtures/primitives-misuse/cases.json) through
// `solid-checker feedback run`: the misuse and the correct twin, each as a tiny
// client app on the published package and Solid rc.9 dev build. Records:
// - the Solid dev diagnostics the runtime emitted, with the authored site the
//   collector attributed them to (the runtime's own report, not a guess);
// - the native findings the static checker produced for the same file.
// A case's runtime verdict is "detected" when the misuse twin gets an expected
// diagnostic attributed to the case file and the correct twin gets none.
//
// usage: node misuse-runtime-ledger.mjs <fresh-output.json> <chromium> [--only id,...] [--concurrency N]
// env: SOLID_CHECKER_NATIVE_BIN, SOLID_TYPEFACTS_BIN, MISUSE_CASES_ROOT (retained ledger installs),
//      MISUSE_TOOLING (an app with vite, @solidjs/vite-plugin, playwright, @jridgewell/trace-mapping)
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url)), repo = resolve(here, "../../..");
const args = process.argv.slice(2);
const [outputArg, browserArg] = args.filter(arg => !arg.startsWith("--") && !args[args.indexOf(arg) - 1]?.startsWith("--"));
assert(outputArg && browserArg, "usage: misuse-runtime-ledger.mjs <fresh-output.json> <chromium> [--only ids] [--concurrency N]");
const option = name => { const index = args.indexOf(name); return index >= 0 ? args[index + 1] : null; };
const only = option("--only")?.split(",") ?? [];
const concurrency = Number(option("--concurrency") ?? 3);
const output = resolve(outputArg);
assert(!existsSync(output), "use a fresh output path");
const casesRoot = process.env.MISUSE_CASES_ROOT ?? "/Users/thomas/Documents/Github/solid-checker/rust/target/primitives-checkpoint/misuse";
const tooling = process.env.MISUSE_TOOLING ?? "/Users/thomas/Documents/Github/solid-checker/rust/target/app-import-metric/apps/helge-dev";
assert(process.env.SOLID_CHECKER_NATIVE_BIN && process.env.SOLID_TYPEFACTS_BIN, "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN");
const ledger = JSON.parse(readFileSync(resolve(option("--cases") ?? join(repo, "fixtures/primitives-misuse/cases.json")), "utf8"));
// A case without a retained ledger install is written into its own directory
// under the cases root, whose node_modules (an ancestor) supplies the packages.
const synthesizedOptions = { target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
  jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], skipLibCheck: true, strict: true, noEmit: true };

// The runtime codes that report each ledger rule's misuse class.
export const expectedCodes = {
  "missing-owner": ["NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_BOUNDARY", "SETTLED_CLEANUP_UNOWNED"],
  "strict-read-untracked": ["STRICT_READ_UNTRACKED"],
  "reactive-write-in-owned-scope": ["REACTIVE_WRITE_IN_OWNED_SCOPE"],
  "leaf-owner-forbidden-call": ["CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE"]
};

const caseDir = entry => join(casesRoot, entry.id.replace(/[^A-Za-z0-9._-]/g, "_"));
const main = `import { render } from "@solidjs/web";
import * as mod from "./case";

// Rendered as a component (<App />), so Solid creates it the way an
// application does, with the component's strict-read window open.
const App = (mod as { default?: unknown }).default as ((props: {}) => any) | undefined;
const root = document.getElementById("root")!;
if (typeof App === "function") render(() => <App />, root);
// Give effects, timers and settled callbacks a moment, then mark readiness.
setTimeout(() => { document.getElementById("ready")!.textContent = "settled"; }, 400);
`;
const html = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div><p id="ready">running</p><script type="module" src="/main.tsx"></script></body></html>\n`;
const scenario = { schemaVersion: 1, steps: [{ action: "wait-for-text", selector: "#ready", text: "settled" }] };

function run(command, argv, options) {
  return new Promise(done => {
    const child = spawn(command, argv, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "", stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), 180000);
    child.stdout.on("data", chunk => { stdout += chunk; });
    child.stderr.on("data", chunk => { stderr += chunk; });
    child.on("close", status => { clearTimeout(timer); done({ status, stdout, stderr }); });
  });
}

async function runTwin(entry, twin) {
  const dir = caseDir(entry);
  mkdirSync(dir, { recursive: true });
  const retained = existsSync(join(dir, `tsconfig.${twin}.json`));
  const tsconfig = retained ? JSON.parse(readFileSync(join(dir, `tsconfig.${twin}.json`), "utf8")) : { compilerOptions: synthesizedOptions };
  writeFileSync(join(dir, "case.tsx"), retained ? readFileSync(join(dir, `${twin}.tsx`)) : entry[twin]);
  writeFileSync(join(dir, "main.tsx"), main);
  writeFileSync(join(dir, "index.html"), html);
  // A case may drive the page (ADR 0207: an event claim clicks its target).
  writeFileSync(join(dir, "scenario.json"), JSON.stringify(entry.scenario ? { schemaVersion: 1, steps: [...scenario.steps, ...entry.scenario] } : scenario));
  writeFileSync(join(dir, "tsconfig.runtime.json"), JSON.stringify({ ...tsconfig, files: ["main.tsx", "case.tsx"] }, null, 2));
  const started = Date.now();
  const result = await run(process.execPath, [join(repo, "packages/cli/bin/solid-checker.mjs"), "feedback", "run",
    "--project", join(dir, "tsconfig.runtime.json"), "--scenario", join(dir, "scenario.json"),
    "--browser", resolve(browserArg), "--tooling", tooling, "--native-bin", process.env.SOLID_CHECKER_NATIVE_BIN], { cwd: dir, env: process.env });
  let report = null;
  try { report = JSON.parse(result.stdout); } catch { /* recorded below */ }
  if (!report) return { error: (result.stderr || result.stdout).slice(-1500), status: result.status, wallMs: Date.now() - started };
  const casePath = join(dir, "case.tsx");
  const diagnostics = (report.execution?.diagnostics ?? []).map(row => ({ code: row.code, severity: row.severity,
    site: row.site ? { file: row.site.location.path === casePath ? "case" : row.site.location.path, line: row.site.line, column: row.site.column } : null,
    operation: row.operation ?? null, siteKind: row.siteKind ?? null,
    attribution: row.attribution ?? null, firstPackage: row.firstPackageFrame?.package?.name ?? null, message: row.message?.slice(0, 160) }));
  // Uncaught exceptions the collector mapped to the case file.
  const exceptions = (report.observations ?? []).filter(row => row.kind === "runtime-exception" && row.location?.path === casePath)
    .map(row => ({ message: String(row.message ?? "").slice(0, 160), startByte: row.location.startByte }));
  const native = (report.analysis?.findings ?? []).filter(finding => finding.primaryLocation?.path === casePath)
    .map(finding => ({ rule: finding.rule, kind: finding.kind, line: finding.primaryLocation.line }));
  return { status: result.status, wallMs: Date.now() - started, typingErrors: report.typingErrorCount ?? null,
    failure: report.execution?.failure?.message ?? null,
    console: (report.execution?.consoleDiagnostics ?? []).map(row => row.message.slice(0, 200)), pageErrors: (report.execution?.pageErrors ?? []).map(error => error.message.slice(0, 200)),
    blocked: report.execution?.blockedRequests ?? [], diagnostics, exceptions, native,
    nativeOther: (report.analysis?.findings ?? []).filter(finding => finding.primaryLocation?.path !== casePath).map(finding => finding.rule) };
}

function verdict(entry, misuse, correct) {
  // A twin whose page never settled (a failed transform, a hung module) proves
  // nothing either way.
  if (misuse.error || correct.error) return "harness-error";
  const expected = expectedCodes[entry.rule] ?? [];
  // A claim against the call site: a diagnostic at the case file whose
  // operation ran in application code. A strict read performed inside an
  // installed package is the package's behaviour; an owner diagnostic depends
  // on the caller's context wherever the operation runs.
  // A strict read in package code still belongs to the call site when the
  // site reads a value the package returned (`position.y`, `now()`), not when
  // it is the export's own call (`createPolled(...)`).
  const authored = twin => twin.diagnostics.filter(row => row.site?.file === "case"
    && (row.code !== "STRICT_READ_UNTRACKED" || row.operation?.in === "application"
      || (row.operation?.in === "package" && row.siteKind === "value-access")));
  // An uncaught exception at the case file is the runtime refusing the misuse
  // outright; it counts when the correct twin raises none.
  const misuseHit = authored(misuse).some(row => expected.includes(row.code)) || misuse.exceptions.length > 0;
  const correctAny = [...authored(correct), ...correct.exceptions];
  // A page that never settled without any such report proves nothing.
  if ((misuse.failure && !misuseHit) || correct.failure) return "harness-error";
  if (misuseHit && correctAny.length === 0) return "detected";
  if (misuseHit) return "correct-twin-also-warns";
  if (misuse.diagnostics.some(row => expected.includes(row.code))) return "warned-unattributed";
  return "runtime-silent";
}

function staticVerdict(entry, misuse, correct) {
  if (misuse.error || correct.error) return "harness-error";
  const ruleRows = twin => twin.native.filter(row => row.rule === entry.rule);
  const correctViolations = correct.native.filter(row => row.kind === "violation" && row.rule !== "prefer-show" && row.rule !== "prefer-for");
  if (correctViolations.length) return "false-positive-on-correct";
  const hit = ruleRows(misuse);
  if (hit.some(row => row.kind === "violation")) return "violation";
  if (hit.length) return "uncertifiable";
  if (misuse.native.some(row => row.rule === "package-contract-incomplete")) return "contract-missing";
  return "silent";
}

const selected = ledger.cases.filter(entry => !only.length || only.includes(entry.id));
const results = [];
let next = 0;
async function worker() {
  while (next < selected.length) {
    const entry = selected[next++];
    if (!option("--cases") && !existsSync(join(caseDir(entry), "node_modules"))) { results.push({ id: entry.id, error: "no retained install" }); continue; }
    const misuse = await runTwin(entry, "misuse");
    const correct = await runTwin(entry, "correct");
    const packageBehaviour = [...(misuse.diagnostics ?? []), ...(correct.diagnostics ?? [])]
      .filter(row => row.code === "STRICT_READ_UNTRACKED" && row.operation?.in === "package" && row.siteKind !== "value-access")
      .map(row => row.operation.package.name);
    const row = { id: entry.id, package: entry.package, export: entry.export, rule: entry.rule, ledgerKind: entry.kind ?? "violation",
      runtime: verdict(entry, misuse, correct), static: staticVerdict(entry, misuse, correct),
      packageBehaviour: [...new Set(packageBehaviour)], misuse, correct };
    results.push(row);
    process.stderr.write(`${results.length}/${selected.length} ${entry.id}: runtime=${row.runtime} static=${row.static}\n`);
  }
}
await Promise.all(Array.from({ length: concurrency }, worker));
const tally = key => results.reduce((counts, row) => ({ ...counts, [row[key] ?? "error"]: (counts[row[key] ?? "error"] ?? 0) + 1 }), {});
writeFileSync(output, JSON.stringify({ cases: results.length, packages: new Set(results.map(row => row.package)).size,
  runtime: tally("runtime"), static: tally("static"), results }, null, 2) + "\n");
console.log(JSON.stringify({ runtime: tally("runtime"), static: tally("static") }));
