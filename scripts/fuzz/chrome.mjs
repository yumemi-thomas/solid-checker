// Chrome confirmation: the authority. A candidate from the fast path is copied into a
// cases directory and run by scripts/runtime-oracle.mjs unchanged (the oracle runs the
// case in Chrome under the dev build, with the checker, and attributes each runtime
// diagnostic to an authored site). This module only builds that invocation and turns the
// oracle's row for the case into the same `runtime` and `checker` records the comparator
// reads for the fast path, so one classifier judges both.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const ORACLE = join(repo, "scripts/runtime-oracle.mjs");
export const DEFAULT_ORACLE_INSTALL = join(repo, "scripts/runtime-oracle-install");

/** `row.diagnostics` of the oracle -> the comparator's diagnostics (the site, then each application frame, as positions). */
export function chromeDiagnostics(row) {
  return (row.diagnostics ?? []).map((diagnostic) => ({
    code: diagnostic.code, message: diagnostic.message ?? null,
    positions: [{ file: diagnostic.file, line: diagnostic.line }, ...(diagnostic.stack ?? []).map((frame) => {
      const [file, line] = frame.split(":");
      return { file, line: Number(line) };
    })],
  }));
}

/**
 * The oracle's row (and its report.json checkpoints) as a runtime record.
 * `checkpoints` are `{ selector, text }` rows from report.execution.checkpoints.
 */
export function chromeRuntime(row, checkpoints, sites) {
  const failure = ["harness-error", "type-error"].includes(row.verdict) ? { phase: row.verdict, message: row.error ?? row.failure ?? row.verdict } : null;
  const text = (selector) => {
    const found = (checkpoints ?? []).filter((checkpoint) => checkpoint.selector === selector).at(-1);
    return found === undefined ? undefined : (found.text ?? "").trim().replace(/\s+/g, " ");
  };
  return {
    failure,
    diagnostics: chromeDiagnostics(row),
    errors: (row.pageErrors ?? []).map((message) => ({ kind: "page-error", message, positions: [] })),
    witnesses: sites.map((site) => {
      if (!site.witness) return { siteId: site.id, witness: null, present: false, actual: null, reached: null };
      const actual = text(site.witness.selector);
      return { siteId: site.id, witness: site.witness, present: actual !== undefined, actual: actual ?? null, reached: actual === undefined ? null : actual === site.witness.text };
    }),
  };
}

/** The oracle's findings for the case as the comparator's checker record. */
export const chromeChecker = (row) => ({ ok: !["harness-error", "type-error"].includes(row.verdict), error: row.error, findings: row.findings ?? [] });

function spawnOracle(args, env) {
  return new Promise((done) => {
    const child = spawn(process.execPath, [ORACLE, ...args], { cwd: repo, env, stdio: ["ignore", "pipe", "pipe"] });
    let log = "";
    child.stdout.on("data", (chunk) => { log += chunk; });
    child.stderr.on("data", (chunk) => { log += chunk; });
    child.on("close", (status) => done({ status, log }));
  });
}

/**
 * Runs the cases through scripts/runtime-oracle.mjs and returns `Map<id, { row, runtime, checker }>`.
 * Exit status 1 from the oracle (a finding it reports as unproven-clean, say) is not a failure here.
 * @param {{ id: string, dir: string, sites: object[] }[]} cases
 */
export async function runChrome(cases, { out, install = DEFAULT_ORACLE_INSTALL, bin, typefacts, browser, concurrency, env = process.env }) {
  if (!bin || !typefacts) throw new Error("Chrome confirmation needs the checker binary and typefacts (--checker-bin, --typefacts)");
  const root = join(resolve(out), "chrome-cases");
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });
  for (const { id, dir } of cases) cpSync(dir, join(root, id), { recursive: true });
  const result = join(resolve(out), `chrome-${Date.now()}.json`);
  const args = ["--cases", root, "--out", result, "--install", resolve(install), "--cache", "off",
    ...(browser ? ["--browser", browser] : []), ...(concurrency ? ["--concurrency", String(concurrency)] : [])];
  const run = await spawnOracle(args, { ...env, SOLID_CHECKER_NATIVE_BIN: bin, SOLID_TYPEFACTS_BIN: typefacts });
  if (!existsSync(result)) throw new Error(`the oracle wrote no result (exit ${run.status}): ${run.log.slice(-800)}`);
  const report = JSON.parse(readFileSync(result, "utf8"));
  const rows = new Map(report.results.map((row) => [row.id, row]));
  const records = new Map();
  for (const { id, sites } of cases) {
    const row = rows.get(id);
    if (!row) { records.set(id, { row: null, runtime: { failure: { phase: "chrome", message: "no row" }, diagnostics: [], errors: [], witnesses: [] }, checker: { ok: false, error: "no oracle row", findings: [] } }); continue; }
    const reportPath = join(resolve(install), ".solid-checker-runtime-oracle", id, "report.json");
    const checkpoints = existsSync(reportPath) ? (JSON.parse(readFileSync(reportPath, "utf8")).execution?.checkpoints ?? []) : [];
    records.set(id, { row, runtime: chromeRuntime(row, checkpoints, sites), checker: chromeChecker(row) });
  }
  return records;
}
