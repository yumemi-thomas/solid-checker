#!/usr/bin/env node
// The neutrality regression: do the counters move any verdict? (scripts/fuzz/README.md, "Neutrality")
//
// Runs a seed range per profile twice through the same pipeline, once on the clean text with no hit ledger
// (the baseline: what the fuzzer did before the ledger) and once instrumented, and compares verdicts program
// by program. Only an `inconclusive` verdict may move, and only to a resolved one (the ledger turns "nothing
// shows it ran" into a verdict); any other change means the counters perturbed something, and exit status 1
// says so. It also reports: strip(runText) == text over the range, and how many findings that used to
// "span no site" (a finding on a plumbing line) now map to a site.
//
//   node scripts/fuzz/neutrality.mjs --seed 1 --count 300 --profiles closure-escape,body-call --out /tmp/neutral-1 \
//     --checker-bin <solid-checker> --typefacts <solid-typefacts> [--control]
//   node scripts/fuzz/neutrality.mjs --baseline a/results.jsonl --candidate b/results.jsonl   (compare two recorded runs)
//
// Speed (none of it changes a verdict; --no-daemon, --no-cache and --order natural give the plain run):
//   the fast path runs in one long-lived Vitest host (fast-daemon.mjs); --concurrency/--workers default to half
//   the cores; the baseline pass is cached (cache.mjs) under --cache-dir keyed by generator version, program,
//   checker and type-facts hashes, the fast install's lockfile and the judging sources, so a second round
//   re-runs only the instrumented pass; --order failing-first runs what mismatched last time first;
//   --stop-on-mismatch ends the round at the first chunk (--chunk programs, default 50) that mismatches.
// `--control` also runs the clean twin and the checker on the twin's text.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { compareVerdictRuns } from "./hits.mjs";
import { evaluate } from "./pipeline.mjs";
import { BaselineCache, MismatchHistory, environmentKey, programKey } from "./cache.mjs";
import { attachDaemon, configFrom, generateEntries, parseArgs, resultRows } from "./run.mjs";

export const BLIND = "the finding spans no site of interest";

/** results.jsonl text -> rows. */
export const readRows = (path) => readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const blindKeys = (rows) => rows.flatMap((row) => row.items.filter((item) => item.reason === BLIND && item.finding).map((item) => `${row.id}|${item.finding.rule}|${item.finding.line}`));

/**
 * How many findings the baseline could not place on a site (`BLIND`) are placed by the instrumented run.
 * A finding is identified by program, rule and line; "mapped" means the instrumented run has an item for it
 * that is not `BLIND`.
 */
export function blindFindingStats(baseRows, instrRows) {
  const before = blindKeys(baseRows);
  const stillBlind = new Set(blindKeys(instrRows));
  const outcome = new Map();
  for (const row of instrRows) for (const item of row.items) if (item.finding && item.reason !== BLIND) outcome.set(`${row.id}|${item.finding.rule}|${item.finding.line}`, item.kind);
  const mapped = before.filter((key) => !stillBlind.has(key) && outcome.has(key));
  const by = mapped.reduce((counts, key) => ({ ...counts, [outcome.get(key)]: (counts[outcome.get(key)] ?? 0) + 1 }), {});
  return { before: before.length, stillBlind: before.filter((key) => stillBlind.has(key)).length, mapped: mapped.length, mappedTo: by };
}

/** The full comparison of two runs' rows: verdict equality, resolved moves and the blind-finding statistics. */
export function compareRuns(baseRows, instrRows) {
  const verdicts = (rows) => new Map(rows.map((row) => [row.id, row.verdict]));
  const verdict = compareVerdictRuns(verdicts(baseRows), verdicts(instrRows));
  const control = instrRows.filter((row) => row.control);
  return {
    programs: verdict.total, sameVerdict: verdict.same, resolved: verdict.resolved, failures: verdict.failures, missing: verdict.missing, equal: verdict.equal,
    blind: blindFindingStats(baseRows, instrRows),
    control: control.length ? { programs: control.length, matched: control.filter((row) => row.control.matched).length, mismatches: control.filter((row) => !row.control.matched).map((row) => ({ id: row.id, diffs: row.control.diffs })) } : null,
  };
}

/**
 * One profile of the regression, in chunks: each chunk's baseline comes from the cache when its key matches
 * (otherwise it is run, uninstrumented, and stored) and its instrumented pass is always run; the chunk is compared
 * at once so `stopOnMismatch` can end the round early.
 */
export async function runProfile({ options, profile, config, cache, history, out, evaluateFn = evaluate, log = () => {} }) {
  const programs = generateEntries({ ...options, profile });
  const ordered = history ? history.order(programs, (entry) => programKey({ seed: entry.program.seed, profile, components: options.components }), "programs") : programs;
  const baseRows = []; const instrRows = [];
  const time = { baseline: 0, instrumented: 0 };
  const chunkSize = Math.max(1, Number(options.chunk ?? 50));
  let stopped = false;
  for (let start = 0; start < ordered.length && !stopped; start += chunkSize) {
    const chunk = ordered.slice(start, start + chunkSize);
    const name = `${profile ?? "default"}-${start / chunkSize}`;
    const keyOf = (entry) => ({ seed: entry.program.seed, profile, components: options.components });
    const cached = new Map();
    const missing = [];
    for (const entry of chunk) {
      const row = cache?.get(keyOf(entry));
      if (row) cached.set(entry.built.id, row); else missing.push(entry);
    }
    let started = Date.now();
    if (missing.length) {
      const fresh = resultRows(await evaluateFn(missing, { ...config, casesDir: join(out, "cases", `${name}-base`), instrument: false, control: false }));
      for (const row of fresh) {
        cached.set(row.id, row);
        const entry = missing.find((candidate) => candidate.built.id === row.id);
        cache?.set(keyOf(entry), row);
      }
    }
    time.baseline += Date.now() - started;
    started = Date.now();
    const fresh = resultRows(await evaluateFn(chunk, { ...config, casesDir: join(out, "cases", `${name}-instr`), instrument: true }));
    time.instrumented += Date.now() - started;
    const base = chunk.map((entry) => cached.get(entry.built.id)).filter(Boolean);
    baseRows.push(...base); instrRows.push(...fresh);
    const compared = compareRuns(base, fresh);
    log(`${profile ?? "default"} chunk ${start / chunkSize}: ${compared.sameVerdict}/${compared.programs} same, ${compared.resolved.length} resolved, ${compared.failures.length} failed${missing.length < chunk.length ? ` (baseline: ${chunk.length - missing.length} cached)` : ""}`);
    if (options.stopOnMismatch && (!compared.equal || (compared.control?.mismatches.length ?? 0))) stopped = true;
    cache?.save();
  }
  const report = compareRuns(baseRows, instrRows);
  const mismatched = new Set([...report.failures.map((f) => f.id), ...(report.control?.mismatches ?? []).map((m) => m.id)]);
  for (const entry of programs) history?.record("programs", programKey({ seed: entry.program.seed, profile, components: options.components }), mismatched.has(entry.built.id));
  history?.record("profiles", profile ?? "default", mismatched.size > 0);
  return { profile: profile ?? "default", stripFailures: programs.filter(({ built }) => !built.neutral).map(({ built }) => built.id), stopped, ranPrograms: instrRows.length, ms: time, report, baseRows, instrRows };
}

async function main(argv) {
  const options = parseArgs(argv, { extraValues: ["baseline", "candidate", "profiles", "chunk", "cache-dir", "order"] });
  const baselinePath = options.baseline;
  const candidatePath = options.candidate;
  if (baselinePath && candidatePath) {
    const report = compareRuns(readRows(baselinePath), readRows(candidatePath));
    console.log(JSON.stringify({ ...report, resolved: report.resolved.length, failures: report.failures.slice(0, 10), missing: report.missing.length }, null, 2));
    return report.equal ? 0 : 1;
  }
  if (!options.out) throw new Error("--out <fresh directory> is required");
  if (options.order !== undefined && !["natural", "failing-first"].includes(options.order)) throw new Error("--order is natural or failing-first");
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  mkdirSync(out, { recursive: true });
  const config = attachDaemon(configFrom(options, out), options);
  const cacheDir = resolve(options.cacheDir ?? join(tmpdir(), "solid-checker-fuzz-cache"));
  const cache = options.noCache ? null : new BaselineCache(cacheDir, environmentKey(config));
  const history = options.order === "failing-first" || options.stopOnMismatch ? new MismatchHistory(cacheDir) : null;
  const wanted = (options.profiles ? String(options.profiles).split(",") : [options.profile]).map((name) => (name === "default" ? undefined : name));
  const profiles = options.order === "failing-first" && history ? history.order(wanted, (name) => name ?? "default", "profiles") : wanted;
  const started = Date.now();
  const results = [];
  let status = 0;
  try {
    for (const profile of profiles) {
      const result = await runProfile({ options, profile, config, cache, history, out, log: (line) => console.error(line) });
      results.push(result);
      console.error(`${result.profile}: strip ${result.ranPrograms - result.stripFailures.length}/${result.ranPrograms}, verdicts ${result.report.equal ? "equal" : "DIFFER"}, ${result.ms.baseline} ms baseline + ${result.ms.instrumented} ms instrumented`);
      if (!result.report.equal || result.stripFailures.length || (result.report.control?.mismatches.length ?? 0)) status = 1;
      if (result.stopped) break;
    }
  } finally {
    history?.save();
    await config.fast?.close();
  }
  const summary = {
    wallMs: Date.now() - started, cache: cache ? { hits: cache.hits, misses: cache.misses, file: cache.path } : null, order: options.order ?? "natural", daemon: !options.noDaemon,
    concurrency: config.concurrency, workers: config.workers,
    profiles: results.map(({ baseRows, instrRows, report, ...rest }) => ({ ...rest, ...report, resolved: report.resolved.length, failures: report.failures.slice(0, 10), missing: report.missing.length })),
  };
  writeFileSync(join(out, "neutrality.json"), `${JSON.stringify(summary, null, 2)}\n`);
  for (const result of results) {
    writeFileSync(join(out, `${result.profile}.baseline.jsonl`), result.baseRows.map((row) => JSON.stringify(row)).join("\n") + "\n");
    writeFileSync(join(out, `${result.profile}.instrumented.jsonl`), result.instrRows.map((row) => JSON.stringify(row)).join("\n") + "\n");
  }
  console.log(JSON.stringify(summary, null, 2));
  return status;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    console.error(`neutrality: ${error.message}`);
    process.exitCode = 2;
  }
}
