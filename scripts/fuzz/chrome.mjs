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
import { caseFiles, caseKey, recordDigest } from "./cache.mjs";
import { HALT_TEXT, HITS_SELECTOR, findingsDiff, haltable, parseHitsText } from "./hits.mjs";

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
  // A row the oracle marks failed, or carrying an execution failure of its own, is a disturbed run whatever its verdict says.
  // A step that resolved to the halt marker (emit.mjs `haltable`) found no element after REACTIVITY_HALTED or an uncaught error: the run was cut short there,
  // as a step that timed out cuts it short, and is a partial run (the judges never read a missing diagnostic past it as silence).
  const halted = (checkpoints ?? []).filter((checkpoint) => (checkpoint.text ?? "").trim() === HALT_TEXT);
  const failure = ["harness-error", "type-error"].includes(row.verdict) ? { phase: row.verdict, message: row.error ?? row.failure ?? row.verdict }
    : row.failure ? { phase: "execution", message: typeof row.failure === "string" ? row.failure : JSON.stringify(row.failure) }
      : halted.length ? { phase: "halted", message: `reactivity halted: ${halted.length} step(s) found no element (${halted[0].selector.split(",")[0]})` } : null;
  const text = (selector) => {
    const found = (checkpoints ?? []).filter((checkpoint) => checkpoint.selector === selector || checkpoint.selector === haltable(selector)).at(-1);
    if (found === undefined || (found.text ?? "").trim() === HALT_TEXT) return undefined;
    return (found.text ?? "").trim().replace(/\s+/g, " ");
  };
  // The hit readout: the last two reads of #sc-hits (equal texts: nothing moved between them).
  const reads = (checkpoints ?? []).filter((checkpoint) => checkpoint.selector === HITS_SELECTOR);
  const counted = parseHitsText(reads.at(-1)?.text);
  return {
    failure,
    hits: counted?.hits ?? null,
    order: counted?.order ?? [],
    quiescent: counted && reads.length >= 2 ? reads.at(-1).text === reads.at(-2).text : null,
    diagnostics: chromeDiagnostics(row),
    errors: (row.pageErrors ?? []).map((message) => ({ kind: "page-error", message, positions: [] })),
    // Codes of diagnostics Chrome saw but could not place in the case file (the oracle's `unattributed`). The comparator
    // treats them as a disturbance: a candidate false positive is never confirmed past one.
    unattributed: (row.unattributed ?? []).map(String),
    witnesses: sites.map((site) => {
      if (!site.witness) return { siteId: site.id, witness: null, present: false, actual: null, reached: null };
      const actual = text(site.witness.selector);
      return { siteId: site.id, witness: site.witness, present: actual !== undefined, actual: actual ?? null, reached: actual === undefined ? null : actual === site.witness.text };
    }),
  };
}

/**
 * What Chrome's judgement of a fast-path false-positive candidate means for it. Only a Chrome run that itself
 * classifies `candidate-false-positive` confirms; a run that shows the diagnostic (`agree`) refutes; a run that
 * cannot decide (an unattributed diagnostic, a page error, a failure) is neither: the candidate stays open.
 */
export function chromeStatus(judged) {
  if (!judged) return "unconfirmed";
  if (judged.verdict === "candidate-false-positive") return "confirmed";
  return judged.verdict === "inconclusive" ? "chrome-inconclusive" : "refuted-by-chrome";
}

/** The oracle's findings for the case as the comparator's checker record. */
export const chromeChecker = (row, cleanFindings) => {
  const findings = row.findings ?? [];
  // The oracle checks whatever App.tsx it is given, which here is the instrumented twin. Its findings must be the ones
  // the clean text got (rule, kind, line); a difference means the counters perturbed the checker, and nothing it says counts.
  const perturbed = cleanFindings === undefined ? [] : cleanFindings === null ? ["no clean reference run in Chrome to compare with"] : findingsDiff(cleanFindings, findings);
  return { ok: !["harness-error", "type-error"].includes(row.verdict), error: row.error, findings, perturbed };
};

function spawnOracle(args, env, oracle = ORACLE) {
  return new Promise((done) => {
    const child = spawn(process.execPath, [oracle, ...args], { cwd: repo, env, stdio: ["ignore", "pipe", "pipe"] });
    let log = "";
    child.stdout.on("data", (chunk) => { log += chunk; });
    child.stderr.on("data", (chunk) => { log += chunk; });
    child.on("close", (status) => done({ status, log }));
  });
}

/**
 * The CLI closes the browser, the page context and the dev server with a 2 s limit each (packages/cli/scripts/feedback-browser.mjs
 * `closeFeedbackResources`); on a loaded machine a close overruns it and the run fails after it has finished its work. That is a
 * failure of the harness, not a finding, and the same case run again succeeds.
 */
export const isCloseTimeout = (row) => row?.verdict === "harness-error" && /close timed out after \d+ ms/.test(String(row.error ?? ""));

/** The id under which a case's clean control runs in the same oracle call as its twin (`withClean`). */
export const CLEAN_ID = (id) => `${id}~clean`;

/** One oracle call over a staged cases directory; returns its rows by id. */
async function oracleRows(root, { out, install, browser, concurrency, env, bin, typefacts, oracle, executionCache }, only) {
  const result = join(resolve(out), `chrome-${Date.now()}-${Math.random().toString(36).slice(2, 6)}.json`);
  const args = ["--cases", root, "--out", result, "--install", resolve(install), "--cache", executionCache ? resolve(executionCache) : "off",
    ...(only ? ["--only", only.join(",")] : []),
    ...(browser ? ["--browser", browser] : []), ...(concurrency ? ["--concurrency", String(concurrency)] : [])];
  const run = await spawnOracle(args, { ...env, SOLID_CHECKER_NATIVE_BIN: bin, SOLID_TYPEFACTS_BIN: typefacts }, oracle);
  if (!existsSync(result)) throw new Error(`the oracle wrote no result (exit ${run.status}): ${run.log.slice(-800)}`);
  return new Map(JSON.parse(readFileSync(result, "utf8")).results.map((row) => [row.id, row]));
}

/**
 * Runs the cases through scripts/runtime-oracle.mjs and returns `Map<id, { row, runtime, checker }>`.
 * Exit status 1 from the oracle (a finding it reports as unproven-clean, say) is not a failure here.
 * Each case runs its instrumented twin (App.run.txt as App.tsx, oracle.run.json as oracle.json) so the hit ledger is read
 * back; `control: true` runs the clean case unchanged instead. `findings` (the oracle's own on the clean text, from a
 * `control: true` run) are compared with the oracle's on the twin; without them nothing is compared.
 * `withClean: true` runs each case's clean control in the same oracle call as its twin (one pool, one process, no tail between
 * two calls) and compares the twin's findings with the control's: the records are the ones a `control: true` call followed by
 * a call with `findings` gives.
 * A case whose run ended in a close timeout (`isCloseTimeout`) is run once more, alone, up to `closeRetries` times (default 1); a case
 * that still fails keeps its row. A retried case's record says `retries`.
 * A case may carry `checker` (a record as `chromeChecker` returns): the oracle's own run is `feedback run`, which does not read dependency
 * packages from source, so a case that ships a package is judged by the checker run as a user's is (checker.mjs ORDINARY_ARGS) and Chrome
 * supplies the runtime only.
 * `cache` (an `EntryCache` of cache.mjs under `chromeEnvironmentKey`) answers a case whose files match a stored run from the store, with
 * `cached: true` on its record, and runs the others. It is for confirmation only: the record's `checker` half is the one of the run
 * that stored it, so a caller that judges the checker by Chrome's findings passes no cache. A run the machine spoiled (no row, a
 * browser that would not close, a page that never loaded) is not stored.
 * @param {{ id: string, dir: string, sites: object[], findings?: object[], checker?: object }[]} cases
 */
export async function runChrome(cases, options) {
  const { cache, control = false, withClean = false } = options;
  if (!cache) return runChromeCases(cases, options);
  const mode = control ? "control" : withClean ? "twin+clean" : "twin";
  const records = new Map();
  const missing = [];
  const hits = [];
  for (const entry of cases) {
    const key = caseKey(caseFiles(entry.dir), `chrome-${mode}|${entry.id}`);
    const stored = cache.get(key);
    if (stored !== undefined) {
      records.set(entry.id, { ...stored, checker: entry.checker ?? stored.checker, cached: true });
      hits.push({ entry, key, stored, spot: cache.takeSpot("chrome") });
    } else missing.push({ entry, key });
  }
  // The first hit of a process runs fresh in the same oracle call as the misses. Chrome can flake under load, so a difference does not stop
  // the run: it drops the cache for this call, and every hit runs fresh (named `cached: false`, the mismatch in the cache's stats).
  const spots = hits.filter((hit) => hit.spot);
  const ran = missing.length || spots.length ? await runChromeCases([...missing, ...spots].map(({ entry }) => entry), options) : new Map();
  const mismatch = spots.find((hit) => cacheableChrome(ran.get(hit.entry.id)) && recordDigest(ran.get(hit.entry.id).runtime) !== recordDigest(hit.stored.runtime));
  if (mismatch) {
    cache.spotMismatches.push(mismatch.entry.id);
    console.error(`chrome cache: a fresh run of ${mismatch.entry.id} differs from its stored record; this call runs every case fresh`);
    const rest = hits.filter((hit) => !hit.spot);
    const again = rest.length ? await runChromeCases(rest.map(({ entry }) => entry), options) : new Map();
    for (const hit of hits) {
      const record = hit.spot ? ran.get(hit.entry.id) : again.get(hit.entry.id);
      records.set(hit.entry.id, { ...record, cached: false });
      if (cacheableChrome(record)) cache.set(hit.key, record);
    }
  }
  for (const { entry, key } of missing) {
    const record = ran.get(entry.id);
    records.set(entry.id, { ...record, cached: false });
    if (cacheableChrome(record)) cache.set(key, record);
  }
  return records;
}

/** A Chrome record that says something about the program, not about the machine: an oracle row that is not a harness error, a page that loaded. */
export const cacheableChrome = (record) => Boolean(record?.row) && record.row.verdict !== "harness-error"
  && !/page\.goto|bootstrap did not bind|close timed out|Target (page, context or browser )?closed|browserType\.launch/i.test(String(record.runtime?.failure?.message ?? ""));

async function runChromeCases(cases, { out, install = DEFAULT_ORACLE_INSTALL, bin, typefacts, browser, concurrency, control = false, withClean = false, closeRetries = 1, oracle, env = process.env, executionCache }) {
  if (!bin || !typefacts) throw new Error("Chrome confirmation needs the checker binary and typefacts (--checker-bin, --typefacts)");
  const root = join(resolve(out), "chrome-cases");
  rmSync(root, { recursive: true, force: true });
  mkdirSync(root, { recursive: true });
  const cleanToo = withClean && !control;
  for (const { id, dir } of cases) {
    cpSync(dir, join(root, id), { recursive: true });
    if (cleanToo) cpSync(dir, join(root, CLEAN_ID(id)), { recursive: true });
    if (control) continue;
    const twin = join(root, id, "App.run.txt");
    if (existsSync(twin)) {
      cpSync(twin, join(root, id, "App.tsx"));
      cpSync(join(root, id, "oracle.run.json"), join(root, id, "oracle.json"));
    }
  }
  const settings = { out, install, browser, concurrency, env, bin, typefacts, oracle, executionCache };
  const rows = await oracleRows(root, settings);
  const retries = new Map();
  for (let attempt = 1; attempt <= closeRetries; attempt += 1) {
    const failed = [...rows.values()].filter(isCloseTimeout).map((row) => row.id);
    if (!failed.length) break;
    const again = await oracleRows(root, settings, failed);
    for (const id of failed) {
      retries.set(id, attempt);
      if (again.has(id)) rows.set(id, again.get(id));
    }
  }
  const reportOf = (id) => {
    const reportPath = join(resolve(install), ".solid-checker-runtime-oracle", id, "report.json");
    return existsSync(reportPath) ? (JSON.parse(readFileSync(reportPath, "utf8")).execution?.checkpoints ?? []) : [];
  };
  const records = new Map();
  for (const { id, sites, findings, checker } of cases) {
    const row = rows.get(id);
    if (!row) { records.set(id, { row: null, runtime: { failure: { phase: "chrome", message: "no row" }, diagnostics: [], errors: [], witnesses: [] }, checker: { ok: false, error: "no oracle row", findings: [] } }); continue; }
    // The reference for the twin's findings: the oracle's own on the clean text (from this call's control, or passed in).
    let reference = findings;
    if (cleanToo) {
      const cleanRow = rows.get(CLEAN_ID(id));
      const cleanChecker = cleanRow ? (checker ?? chromeChecker(cleanRow, undefined)) : null;
      reference = cleanChecker?.ok ? cleanChecker.findings : null;
    }
    records.set(id, { row, runtime: chromeRuntime(row, reportOf(id), sites), checker: checker ?? chromeChecker(row, control ? undefined : reference), ...(retries.has(id) ? { retries: retries.get(id) } : {}) });
  }
  return records;
}
