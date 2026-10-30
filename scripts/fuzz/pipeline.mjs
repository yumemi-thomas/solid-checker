// One program's trip through the differential: type-check (tsrs), checker verdicts,
// fast runtime records, classification. Used for a whole batch (a run) and for a single
// program (the shrinker's predicate).
//
// The checker reads the clean text. The type check of the clean text is the authority for "tsc accepts
// this" (invariant 1); the instrumented twin (hits.mjs) is type-checked too, because it is what runs,
// and it runs in Happy DOM. The neutrality gate (hits.mjs, README): strip(runText) == text is checked
// when the case is built; `config.control` runs the clean twin in the same engine and compares the
// outcomes (and the checker on the twin's text); both are required before an unexecuted candidate is confirmed (unexecuted.mjs).
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { mapPoolSettled } from "../lib/pool.mjs";
import { StaleCacheError, cacheableRecord, cacheableTypecheck, caseKey, recordDigest, typecheckDigest } from "./cache.mjs";
import { checkerProjectFiles, checkerResult, checkerWorkDir, materializeForChecker, runChecker, siteVerdicts } from "./checker.mjs";
import { classify, runtimeEvents } from "./classify.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { runFast } from "./fast.mjs";
import { findingsDiff, outcomeDiff } from "./hits.mjs";
import { closedGrammar } from "./model.mjs";
import { signature as findingSignature } from "./dedupe.mjs";
import { callAtLine } from "./falseclean.mjs";
import { registrationLines } from "./registration.mjs";
import { createTypecheckBatcher, typecheck, typecheckBatch } from "./typecheck.mjs";
import { judgeUnexecuted, lexicallyDeadHits } from "./unexecuted.mjs";

/** A case that ships a generated package is checked as a user's run is (checker.mjs ORDINARY_ARGS): the package is read from source. */
export const hasPackages = (built) => Object.keys(built.packages ?? {}).length > 0;

const inconclusive = (reason, why) => ({ verdict: "inconclusive", reasons: [reason], items: [{ kind: "inconclusive", reason, why }] });

/**
 * Evaluates built cases (from `buildCase`) through the fast path.
 * `config`: { install (oracle install, hosts checker + type-check work dirs), fastInstall, bin, typefacts, tsc,
 *   casesDir (where case dirs are written), concurrency, workers, keepWork,
 *   instrument (default true; false runs the clean text and classifies as before the hit ledger: the baseline of the neutrality regression),
 *   control (also run the clean twin and the checker on the twin's text, and compare),
 *   (`deadSite` is the strict confirmation of unexecuted candidates, read by run.mjs; it changes no verdict here),
 *   runtimeCache (an `EntryCache` of cache.mjs under `runtimeEnvironmentKey`: the type checks and the fast-path records are taken from it
 *   when the case's files match, and stored in it otherwise; only the checker runs again. Absent, every step runs, as before it existed) }
 * @returns {Promise<Map<string, object>>} id -> { program, built, typecheck, checker, runtime, classification, verdicts, control }
 */
export async function evaluate(entries, config, deps = {}) {
  return finishEntries(await prepareEntries(entries, config, deps), config, deps);
}

/**
 * Where a run's time goes, summed over the work items of each phase (busy time: items overlap in a pool, so a sum can exceed the
 * wall time): materializing projects, type checks run (not cache hits), checker runs, fast-path batches. `phaseTimings()` reads and
 * `resetPhaseTimings()` clears them; the grids print them at the end.
 */
const timings = { materialize: { ms: 0, n: 0 }, typecheck: { ms: 0, n: 0 }, checker: { ms: 0, n: 0 }, fast: { ms: 0, n: 0 } };
export const phaseTimings = () => JSON.parse(JSON.stringify(timings));
/** The timings as one log line: each phase's busy seconds and item count. */
export const formatPhaseTimings = (phases = phaseTimings()) => Object.entries(phases).map(([phase, { ms, n }]) => `${phase} ${(ms / 1000).toFixed(1)}s/${n}`).join(", ");
export function resetPhaseTimings() { for (const phase of Object.values(timings)) { phase.ms = 0; phase.n = 0; } }
const timed = (phase, work) => {
  const start = performance.now();
  const done = () => { timings[phase].ms += performance.now() - start; timings[phase].n += 1; };
  const value = work();
  if (value && typeof value.then === "function") return value.finally(done);
  done();
  return value;
};

/**
 * The first half of `evaluate`: writes the cases, type-checks them and runs the checker (step 1). `finishEntries` runs the fast path and
 * classifies (steps 2 and 3). Apart, a caller can overlap one batch's checker work with the previous batch's fast path (matrix.mjs); the
 * halves are `evaluate`'s, so the records and classifications are the same.
 */
export async function prepareEntries(entries, config, deps = {}) {
  // Type checks of this call share `tsrs -b` processes (as many at a time as the pool runs), unless a test injects one or the run
  // asks for one process each.
  const { runChecker: check = runChecker, typecheck: batched = config.typecheckBatch ? createTypecheckBatcher({ size: config.concurrency }) : typecheck } = deps;
  const { install, casesDir } = config;
  const instrument = config.instrument !== false;
  const control = instrument && Boolean(config.control);
  const cache = config.runtimeCache ?? null;
  // A type check (or a fast record) from the cache when the case's files match, else the real one, stored when it says something about the program.
  const remembered = async (built, part, compute, keep) => {
    if (!cache) return compute();
    const key = caseKey(built.files, part);
    const stored = cache.get(key);
    if (stored !== undefined) {
      // The first hits of a process are also run fresh: a cause the key does not cover shows here, and stops the run.
      if (cache.takeSpot("typecheck")) {
        const fresh = await compute();
        if (cacheableTypecheck(fresh) && typecheckDigest(fresh) !== typecheckDigest(stored)) {
          cache.spotMismatches.push(built.id);
          throw new StaleCacheError(`the runtime cache's ${part} of ${built.id} differs from a fresh one (${typecheckDigest(stored)} vs ${typecheckDigest(fresh)}): something its key does not cover changed. Remove ${cache.root} or pass --no-cache`);
        }
      }
      return stored;
    }
    const value = await compute();
    if (keep(value)) cache.set(key, value);
    return value;
  };
  mkdirSync(casesDir, { recursive: true });

  // With the Rust executor (`config.exec`, exec.mjs) step 1 is its work: every program of the call goes to it at once, and it writes,
  // type-checks and checks them on its own threads. The checker control on the twin stays on this path.
  if (config.exec && !control && !deps.typecheck && !deps.runChecker) {
    const exec = await config.exec;
    const prepared = await mapPoolSettled(entries, (entry) => prepareViaExecutor(entry, { config, cache, exec, instrument }), { concurrency: Math.max(1, entries.length) });
    return settlePrepared(entries, prepared, config, instrument);
  }

  // 0. With batched type checks, every project of the call is written first and the type checks the cache does not answer start as one
  // `tsrs -b` per pool slot over a share of them: a tsrs process spends most of its time reading the libraries, so a few large batches
  // cost a fraction of the small ones step 1's pool would gather (typecheck.mjs createTypecheckBatcher). The answers are typecheckBatch's,
  // the projects alone where a batch gives none, as the batcher's are. A test that injects `typecheck` skips this step.
  const staged = new Map();
  const answers = new Map();
  if (config.typecheckBatch && !deps.typecheck) {
    const pending = [];
    for (const { built } of entries) {
      const stage = {
        caseDir: timed("materialize", () => writeCase(join(casesDir, built.id), built)),
        dir: timed("materialize", () => materializeForChecker(built, install, built.id)),
        twinDir: null,
      };
      if (!cache?.has(caseKey(built.files, "typecheck"))) pending.push(stage.dir);
      if (instrument && !cache?.has(caseKey(built.files, "typecheck-twin"))) {
        stage.twinDir = timed("materialize", () => materializeForChecker(built, install, `${built.id}-run`, { twin: true }));
        pending.push(stage.twinDir);
      }
      staged.set(built.id, stage);
      // The Chrome host serves its pages from this event loop (chrome-host.mjs): writing a whole call's projects in one go would stall them.
      await new Promise((resume) => setImmediate(resume));
    }
    const slots = Math.max(1, Math.min(config.concurrency ?? 1, pending.length));
    const groups = Array.from({ length: slots }, (_, slot) => pending.filter((_, index) => index % slots === slot)).filter((group) => group.length);
    // Not awaited here: step 1's pool starts at once, and a program waits for its own batch only.
    for (const group of groups) {
      const batch = timed("typecheck", () => typecheckBatch(group, { tsc: config.tsc }));
      for (const dir of group) answers.set(dir, batch.then((got) => got?.get(dir) ?? typecheck(dir, { tsc: config.tsc })));
    }
  }
  const types = (dir, options) => (answers.has(dir) ? answers.get(dir) : batched(dir, options));

  // 1. Write the case, type-check it as the oracle compiles it, and ask the checker (on the clean text).
  const prepared = await mapPoolSettled(entries, async ({ program, built }) => {
    const stage = staged.get(built.id);
    const caseDir = stage?.caseDir ?? timed("materialize", () => writeCase(join(casesDir, built.id), built));
    const dir = stage?.dir ?? timed("materialize", () => materializeForChecker(built, install, built.id));
    const typed = await remembered(built, "typecheck", () => timed("typecheck", () => types(dir, { tsc: config.tsc })), cacheableTypecheck);
    if (!typed.ok) {
      if (stage?.twinDir && !config.keepWork) rmSync(stage.twinDir, { recursive: true, force: true });
      return { program, built, caseDir, dir, typecheck: typed };
    }
    const ordinary = hasPackages(built);
    const checker = await timed("checker", () => check(dir, { bin: config.bin, typefacts: config.typefacts, ordinary }));
    const value = { program, built, caseDir, dir, typecheck: typed, checker };
    if (instrument) {
      // The twin's project is written only when something reads it: a type check that is not a cache hit (or is spot-checked),
      // or the twin's checker control.
      let twinDir = stage?.twinDir ?? null;
      const twin = () => (twinDir ??= timed("materialize", () => materializeForChecker(built, install, `${built.id}-run`, { twin: true })));
      value.twinTypecheck = await remembered(built, "typecheck-twin", () => timed("typecheck", () => types(twin(), { tsc: config.tsc })), cacheableTypecheck);
      if (control && checker.ok && value.twinTypecheck.ok) value.twinChecker = await timed("checker", () => check(twin(), { bin: config.bin, typefacts: config.typefacts, ordinary }));
      if (twinDir && !config.keepWork) rmSync(twinDir, { recursive: true, force: true });
    }
    return value;
  }, { concurrency: config.concurrency });
  return settlePrepared(entries, prepared, config, instrument);
}

/** Step 1's answers as `prepareEntries` returns them: a stale cache entry stops the run; the live ones go on. */
function settlePrepared(entries, prepared, config, instrument) {
  const results = new Map();
  // A stale cache entry stops the run: it is not one program's error.
  const stale = prepared.find((settled) => settled.error instanceof StaleCacheError);
  if (stale) throw stale.error;
  const live = [];
  for (const [index, settled] of prepared.entries()) {
    const { program, built } = entries[index];
    if (settled.error) { results.set(built.id, { program, built, error: String(settled.error.message ?? settled.error) }); continue; }
    results.set(built.id, { program, built, ...settled.value });
    const value = settled.value;
    if (value.typecheck.ok && value.checker?.ok && (!instrument || (built.neutral && value.twinTypecheck?.ok))) live.push(value);
    if (!config.keepWork && value.dir) rmSync(value.dir, { recursive: true, force: true });
  }
  return { results, live };
}

/**
 * One program's step 1 through the Rust executor: the same files, type checks (cached ones answered from the cache, the cache's spot
 * checks run fresh), checker run and value as the JS path's.
 */
async function prepareViaExecutor({ program, built }, { config, cache, exec, instrument }) {
  const { install, casesDir } = config;
  const caseDir = join(casesDir, built.id);
  const dir = checkerWorkDir(install, built.id);
  const twinDir = checkerWorkDir(install, `${built.id}-run`);
  const caseWrites = Object.entries(built.files).map(([name, text]) => [join(caseDir, name), text]);
  // A type check from the cache, or run fresh (no entry, or a spot check of one); `settle` stores or compares as `remembered` does.
  const cached = (part) => {
    if (!cache) return { stored: undefined, fresh: true };
    const stored = cache.get(caseKey(built.files, part));
    return { part, stored, fresh: stored === undefined || cache.takeSpot("typecheck") };
  };
  const settle = (slot, fresh) => {
    if (!cache) return fresh;
    if (slot.stored !== undefined) {
      if (cacheableTypecheck(fresh) && typecheckDigest(fresh) !== typecheckDigest(slot.stored)) {
        cache.spotMismatches.push(built.id);
        throw new StaleCacheError(`the runtime cache's ${slot.part} of ${built.id} differs from a fresh one (${typecheckDigest(slot.stored)} vs ${typecheckDigest(fresh)}): something its key does not cover changed. Remove ${cache.root} or pass --no-cache`);
      }
      return slot.stored;
    }
    if (cacheableTypecheck(fresh)) cache.set(caseKey(built.files, slot.part), fresh);
    return fresh;
  };
  const clean = cached("typecheck");
  const knownBad = !clean.fresh && !clean.stored.ok;
  const twin = instrument && !knownBad ? cached("typecheck-twin") : null;
  // The projects go to a slot pair the executor rewrites in place, unless the program ships a package (its node_modules) or the run
  // keeps its work: those get directories of their own.
  const ordinary = hasPackages(built);
  const own = ordinary || config.keepWork;
  const answer = await exec.prepare({
    caseWrites,
    project: knownBad ? null : { files: checkerProjectFiles(built), ...(own ? { dir } : {}) },
    twinProject: twin?.fresh ? { files: checkerProjectFiles(built, { twin: true }), ...(own ? { dir: twinDir } : {}) } : null,
    clean: clean.fresh, twin: Boolean(twin?.fresh),
    checker: knownBad ? null : { ordinary },
  });
  const typed = clean.fresh ? settle(clean, answer.typecheck) : clean.stored;
  if (!typed.ok) return { program, built, caseDir, dir: own ? dir : null, typecheck: typed };
  const checker = checkerResult(answer.checker, answer.checker.dir);
  timings.checker.ms += checker.wallMs ?? 0;
  timings.checker.n += 1;
  // A slot's directory is the executor's, not this program's: nothing removes it.
  const value = { program, built, caseDir, dir: own ? dir : null, typecheck: typed, checker };
  if (instrument) value.twinTypecheck = twin.fresh ? settle(twin, answer.twinTypecheck) : twin.stored;
  return value;
}

/** The second half of `evaluate` (steps 2 and 3) on what `prepareEntries` returned. */
export async function finishEntries({ results, live }, config, deps = {}) {
  // The long-lived host (`config.fast`) serves the batches when there is one; tests inject `deps.runFast`.
  const runBatch = deps.runFast ?? (config.fast ? (cases) => config.fast.run(cases) : runFast);
  const fast = (cases, options) => timed("fast", () => runBatch(cases, options));
  const instrument = config.instrument !== false;
  const control = instrument && Boolean(config.control);
  const cache = config.runtimeCache ?? null;

  // 2. Run everything the checker answered in one Vitest batch (the twin), and, as a control, the clean twin in a second.
  const variant = instrument ? "run" : "clean";
  const fastOptions = { install: config.fastInstall, workers: config.workers, virtualClock: config.virtualClock };
  const records = live.length ? await fastCached(live.map(({ built, caseDir }) => ({ id: built.id, dir: caseDir, variant, built })), fast, fastOptions, cache) : new Map();
  const controls = control && live.length
    ? await fastCached(live.map(({ built, caseDir }) => ({ id: `${built.id}-clean`, dir: caseDir, variant: "clean", built })), fast, fastOptions, cache)
    : new Map();

  // 3. Classify.
  for (const value of results.values()) {
    if (value.error || !value.typecheck?.ok) {
      value.classification = { verdict: "type-error", reasons: [value.error ?? value.typecheck?.errors?.[0] ?? "type error"], items: [] };
      continue;
    }
    if (!value.checker?.ok) { value.classification = classify({ sites: value.built.sites, checker: value.checker ?? { ok: false, error: "no checker run" }, runtime: { diagnostics: [], errors: [], witnesses: [] }, plan: value.built.plan }); continue; }
    if (instrument && !value.built.neutral) { value.classification = inconclusive("instrumentation is not neutral: stripping the counters does not give the clean text", "not-neutral"); continue; }
    if (instrument && !value.twinTypecheck?.ok) { value.classification = inconclusive(`the instrumented twin does not type-check: ${value.twinTypecheck?.errors?.[0] ?? "type error"}`, "twin-type-error"); continue; }
    value.runtime = records.get(value.built.id);
    value.verdicts = siteVerdicts([...value.built.sites, ...(instrument ? value.built.glue : [])], value.checker.findings);
    let neutral = instrument;
    if (control) {
      const clean = controls.get(`${value.built.id}-clean`);
      const diffs = [...(clean ? outcomeDiff(value.runtime, clean) : ["no clean-twin record"]),
        ...(value.twinChecker?.ok ? findingsDiff(value.checker.findings, value.twinChecker.findings).map((d) => `checker ${d}`) : ["the checker failed on the instrumented text"])];
      value.control = { diffs, matched: diffs.length === 0, clean };
      neutral = diffs.length === 0;
      if (diffs.length) { value.classification = inconclusive(`instrumentation-perturbed-run: ${diffs[0]}`, "perturbed-run"); continue; }
    }
    value.classification = classify({
      sites: value.built.sites, glue: instrument ? value.built.glue : [], checker: value.checker, runtime: value.runtime, plan: value.built.plan,
      evidence: { ledger: value.built.ledger, closed: closedGrammar(value.program), neutral, lexicallyDead: lexicallyDeadHits(value.program, value.built.ledger), registrationLines: registrationLines(value.program, value.built.lineTable) },
    });
  }
  return results;
}

/**
 * Whether the runtime cache holds every type check and fast record `evaluate` would ask of these entries: then no Vitest batch runs,
 * and the workers' share of the machine is free for the checker (matrix.mjs). Counts no hit or miss.
 */
export function runtimeCached(entries, config) {
  const cache = config.runtimeCache;
  if (!cache) return false;
  const instrument = config.instrument !== false;
  const control = instrument && Boolean(config.control);
  const variant = instrument ? "run" : "clean";
  return entries.every(({ built }) => cache.has(caseKey(built.files, "typecheck"))
    && (!instrument || cache.has(caseKey(built.files, "typecheck-twin")))
    && cache.has(caseKey(built.files, `fast-${variant}|${built.id}`))
    && (!control || cache.has(caseKey(built.files, `fast-clean|${built.id}-clean`))));
}

/** The fast path for `cases` (`{ id, dir, variant, built }`): the cached records, and one batch for the rest (stored when the runner wrote them). */
async function fastCached(cases, fast, options, cache) {
  const strip = (list) => list.map(({ id, dir, variant }) => ({ id, dir, variant }));
  if (!cache) return fast(strip(cases), options);
  const records = new Map();
  const missing = [];
  const checks = [];
  for (const entry of cases) {
    const key = caseKey(entry.built.files, `fast-${entry.variant}|${entry.id}`);
    const stored = cache.get(key);
    if (stored !== undefined) {
      records.set(entry.id, stored);
      if (cache.takeSpot("fast")) checks.push({ ...entry, stored });
    } else missing.push({ ...entry, key });
  }
  // The first hits of a process run fresh beside the misses (the same batch), and must give the same record.
  if (checks.length) {
    const fresh = await fast(strip(checks), options);
    for (const entry of checks) {
      const record = fresh.get(entry.id);
      if (cacheableRecord(record) && recordDigest(record) !== recordDigest(entry.stored)) {
        cache.spotMismatches.push(entry.id);
        throw new StaleCacheError(`the runtime cache's record of ${entry.id} differs from a fresh run: something its key does not cover changed. Remove ${cache.root} or pass --no-cache\n  stored ${recordDigest(entry.stored).slice(0, 300)}\n  fresh  ${recordDigest(record).slice(0, 300)}`);
      }
    }
  }
  if (missing.length) {
    const ran = await fast(strip(missing), options);
    for (const entry of missing) {
      const record = ran.get(entry.id);
      records.set(entry.id, record);
      if (cacheableRecord(record)) cache.set(entry.key, record);
    }
  }
  return records;
}

/**
 * Whether a shrink asks for the neutrality control (`config.control`) of an accepted candidate only, not of every candidate it tries.
 * The control can only turn a verdict into `inconclusive` (`perturbed-run`); it adds nothing to a verdict it lets stand. So a candidate
 * holds under the control exactly when it holds without it and then passes it: `judgeEntries` runs the plain pass for the batch and the
 * controlled pass for the candidates that hold, in order, until one passes, which is the one the search takes.
 * `config.shrinkControl === "each"` restores the old rule.
 */
export const lazyControl = (config) => Boolean(config.control) && config.shrinkControl !== "each";

/**
 * Judges built entries as `evaluate` does and answers `holds(entry, result)` for each. With `lazyControl(config)` the batch runs
 * without the control and the control runs on the entries that hold, one at a time in order, stopping at the first that still
 * holds under it (the search takes the first that holds, so the entries after it are answered false, unreached).
 */
export async function judgeEntries(entries, config, holds, deps = {}) {
  if (!lazyControl(config)) {
    const results = await evaluate(entries, config, deps);
    return entries.map((entry) => holds(entry, results.get(entry.built.id)));
  }
  const plain = await evaluate(entries, { ...config, control: false }, deps);
  const answers = entries.map((entry) => holds(entry, plain.get(entry.built.id)));
  let taken = false;
  for (let index = 0; index < entries.length; index += 1) {
    if (taken) { answers[index] = false; continue; }
    if (!answers[index]) continue;
    const controlled = await evaluate([entries[index]], config, deps);
    answers[index] = holds(entries[index], controlled.get(entries[index].built.id));
    taken = answers[index];
  }
  return answers;
}

/**
 * Chrome's judgement of a fast-path result: the same comparator, on Chrome's runtime and checker records
 * (the oracle's findings on the twin, compared with the fast pipeline's clean ones: a difference is
 * `instrumentation-perturbed-checker`).
 */
export function classifyChrome(value, record, config = {}) {
  return classify({
    sites: value.built.sites, glue: value.built.glue ?? [], checker: record.checker, runtime: record.runtime, plan: value.built.plan,
    evidence: { ledger: value.built.ledger, closed: closedGrammar(value.program), neutral: Boolean(value.built.neutral) && !record.checker.perturbed?.length && value.control?.matched === true,
      lexicallyDead: lexicallyDeadHits(value.program, value.built.ledger), registrationLines: registrationLines(value.program, value.built.lineTable) },
  });
}

/**
 * Chrome's judgement of one fast-path `candidate-unexecuted` item (unexecuted.mjs): `confirmed-unexecuted` only when Chrome shows the same
 * and every condition of deadsite.mjs holds on Chrome's records. `controlMatched` is the clean-twin control of the program (a fast-path
 * run of the clean text, compared with the twin's), which Chrome's records cannot supply.
 */
export function judgeUnexecutedInChrome(value, item, record, { controlMatched, strict = false } = {}) {
  const judged = classifyChrome({ ...value, control: { matched: controlMatched === true } }, record, {});
  return judgeUnexecuted({ item, program: value.program, built: value.built, record, judged, events: runtimeEvents(record.runtime), controlMatched, strict });
}

/** Signatures of a classified result's candidate items: { signature, item } per candidate. */
export function signaturesOf(program, built, classification) {
  const out = [];
  const known = [...built.sites, ...(built.glue ?? [])];
  for (const item of classification.items) {
    if (item.kind === "candidate-false-positive") {
      const site = known.find((s) => s.id === item.siteIds?.[0]);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: item.subkind ?? "silent" }) });
    } else if (item.kind === "candidate-unexecuted") {
      const site = known.find((s) => s.id === item.siteIds?.[0]);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: [...new Set(item.bases ?? [])].sort().join("+") || "unexecuted" }) });
    } else if (item.kind === "candidate-wrong-rule") {
      const site = known.find((candidate) => candidate.line === item.finding.line) ?? known.find((candidate) => Math.abs(candidate.line - item.finding.line) <= 1);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: item.diagnostic.code }) });
    } else if (item.kind === "candidate-false-clean") {
      // The node at the diagnostic's innermost App line, from the printer's line table (not the nearest site by line).
      const node = item.diagnostic.line === null ? null : callAtLine(built.lineTable, item.diagnostic.line);
      if (node) out.push({ item, site: built.sites.find((s) => s.line === item.diagnostic.line) ?? null, signature: findingSignature({ rule: item.diagnostic.code, kind: item.kind, program, path: node.path, outcome: "diagnosed" }) });
    }
  }
  return out;
}

export { buildCase };
