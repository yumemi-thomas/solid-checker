// The twin consistency check: a cheap oracle with the checker alone (no runtime, no Chrome).
//
// The grids generate twins of one program: the same code, context, variant, binding, callback form, mount and source, with the site
// under different layers (none, a closure called once, an IIFE, a helper that calls its callback, a module helper in its import
// forms, a control-flow statement, a hand-off form, N closures called in place). Every layer the grids draw runs its body once, in
// place, synchronously (TWIN_LAYERS says why for each), so the twins of a group do the same thing at run time: the same reads and
// writes, in the same owner, context and timing. Their answers may differ in certainty (fail-closed: a checker that cannot follow a
// layer declines), never in what they prove. Per group and per defect code (codes.mjs: a `defect-*` disposition with claiming rules;
// an advisory or out-of-scope code's silence is no miss), a twin's answer is `violation` (a violation of a rule that claims the code,
// anywhere in the file), `uncertifiable` (only a declining finding of such a rule, or of a withholding rule) or `none`, and:
//   - a violation in one twin and `none` in another is flagged: one of the two is wrong (a false positive or a false clean);
//   - in a legal grid, a violation of the cell's own code where a twin is `uncertifiable` is flagged too: the program is legal by
//     construction (legal.mjs), so the proof is the suspect, and the declining twin shows the checker could not make it;
//   - a violation beside a declining twin is otherwise the fail-closed direction (a misuse twin's recall gap; in a legal program, a
//     real defect of another code that a twin declines), and is not flagged.
// A flagged group is a counter-example for a person: Chrome and the grids say which twin is right. Differences the execution model
// justifies are an explicit allow-list (ALLOWED, each with its reason), never a heuristic.
//
//   node scripts/fuzz/twins.mjs --out <fresh dir> [--codes A,B] [--contexts a,b] [--grids legal,misuse] [--recall [--recall-full]]
//     [--sample <fraction of groups>] [--sample-seed N] [--checker-bin …] [--typefacts …] [--tsc …] [--concurrency N] [--limit N]
//     [--cache-dir <dir>] [--no-cache] [--keep-work]
//   node scripts/fuzz/twins.mjs --from <earlier --out> --out <fresh dir>   (judge a recorded run again; no checker)
//   make fuzz-twins CODES=A,B [FUZZ_TWINS_ARGS="--recall --contexts reaction"]
// Exit status: 0 nothing flagged; 1 a group flagged that the allow-list does not hold; 2 a usage or setup error.
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { EntryCache, cacheDirectory, cacheableTypecheck, typecheckDigest, typecheckEnvironmentKey } from "./cache.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { assertFindingsMapped, materializeForChecker, runChecker } from "./checker.mjs";
import { codeRules, declinesCode, ignoredRules, policyOf, withholdingRules } from "./codes.mjs";
import { generateLegal, legalCells, legalShapes, recallLegalShapes } from "./legal.mjs";
import { axesKey, cellList, generateShape, gridShapes, recallShapes } from "./matrix.mjs";
import { CONTROL_LAYERS, FILE_LAYERS, LAYERS, PAIR_LAYERS, RECALL_LAYERS } from "./misuse.mjs";
import { CHAIN_FORMS, DEPTH_FORMS, HANDOFF_FORMS } from "./model.mjs";
import { typecheckKeyOf } from "./pipeline.mjs";
import { fnv } from "./rng.mjs";
import { requireTypeChecker, resolveTsc, typecheck, typecheckBatch } from "./typecheck.mjs";

/**
 * Every layer a grid draws, and why its body runs once, in place, synchronously (print.mjs prints each): the twins of a group are
 * runtime-identical only over these. A layer the grids draw that is not here stops the check (`twinLayer`): a new layer is classified
 * before its twins are compared. A layer that defers its body (a timer, a microtask, an `await` before it) is not a twin; none is
 * drawn as a layer today (the deferring forms are contexts, `loop-await`, `for-await` and `finally-await` among them).
 */
export const TWIN_LAYERS = {
  none: "the site itself",
  closure: "a named closure called once right after its declaration",
  iife: "an arrow called where it is written",
  helper: "a same-file helper that calls its callback once, synchronously",
  "closure+helper": "a closure called once, holding the helper call",
  "module-helper": "the helper from helpers.ts: same body, another module",
  ...Object.fromEntries(FILE_LAYERS.map((layer) => [layer, "the module helper imported as a default, through a barrel or as a namespace: same body"])),
  for: "`for (let i = 0; i < 1; i += 1)`: one pass", while: "`while (true) { …; break; }`: one pass", do: "`do … while (false)`: one pass",
  switch: "`switch (1) { case 1: …; break; }`", "try-catch": "`try { … } catch {}`: the body runs (a code the runtime only throws takes no try-catch layer)",
  "try-finally": "`try {} finally { … }`", label: "`outer: { …; break outer; }`", generator: "`(function* () { …; yield 1; })().next()`: the body runs up to the yield at `.next()`",
  ...Object.fromEntries(HANDOFF_FORMS.map((layer) => [layer, "a function handed on by name and called once at once (forEach, Array.from, sort of two, call, apply, Reflect.apply, a tag, a wrapper, a member, an assigned member, a declaration)"])),
  ...Object.fromEntries(DEPTH_FORMS.map((layer) => [layer, "N arrows, each called where it is written"])),
  ...Object.fromEntries(CHAIN_FORMS.map((layer) => [layer, "N named helpers, or a callback N helpers hand on"])),
  // The pair grids' composites (misuse.mjs PAIR_LAYERS): both parts run their body once, in place.
  ...Object.fromEntries(PAIR_LAYERS.map((layer) => [layer, "a hand-off form inside the module helper's callback: each calls its body once, at once"])),
};

/** A grid layer's twin class, or a thrown error for a layer nobody classified. */
export function twinLayer(layer) {
  if (TWIN_LAYERS[layer]) return layer;
  throw new Error(`twins.mjs: the layer ${layer} is not classified (TWIN_LAYERS): say whether its body runs once, in place, before its twins are compared`);
}
// Every layer the grids draw is classified (the module fails to load otherwise).
for (const layer of [...LAYERS, ...RECALL_LAYERS, ...CONTROL_LAYERS, ...PAIR_LAYERS]) twinLayer(layer);

/**
 * Differences the execution model justifies, each with its reason: `{ kind?, cell?, context?, code?, layers, reason }` (`cell` the
 * grid cell's code, `code` the judged code), where `layers`
 * names the twins whose answer may differ from the others' (a flagged group whose violating or contradicting twins are all in
 * `layers` is held, and listed as `allowed`). Empty: on the head this was written for, no flagged group needed one (see the README).
 */
export const ALLOWED = [];

/** The codes a twin is judged on: every code a rule claims whose disposition is a defect (not advisory, not out of scope). */
export const TWIN_CODES = Object.keys(codeRules).filter((code) => policyOf(code)?.disposition?.startsWith("defect")).sort();

/**
 * A program's answer per code: a violation of a rule that claims it anywhere in the file, else a declining finding of such a rule, an
 * uncertifiable finding of a rule that declines the code (codes.mjs `decliningRules`), or a finding of a withholding rule (codes.mjs `withholdingRules`: the checker withheld an answer, which covers any code, as the grids count it),
 * else none.
 */
export function answersOf(findings) {
  const out = {};
  const relevant = findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const withheld = relevant.some((finding) => withholdingRules.includes(finding.rule));
  for (const code of TWIN_CODES) {
    const mine = relevant.filter((finding) => codeRules[code].includes(finding.rule) || declinesCode(code, finding));
    out[code] = mine.some((finding) => finding.kind === "violation") ? "violation" : mine.length || withheld ? "uncertifiable" : "none";
  }
  return out;
}

/** The twin group of a grid program: everything but the layer. */
export const groupKey = (kind, code, context, shape) => [kind, code, context, shape.variant ?? "-", shape.binding, shape.callback ?? "-", shape.mount ?? "root", shape.source ?? "local"].join("|");

/**
 * The judgement of one group (`members`: `{ id, layer, answers }`, the programs that type-checked and were checked): per code, the
 * violating twins and the twins that contradict them (header). Returns the flags, each `{ code, codes, violating, contradicting }`
 * with `[layer, id, answer]` triples; codes whose twins split the same way (one rule claims several: `missing-owner`) are one flag,
 * `code` the first of its `codes`.
 */
export function judgeGroup({ kind, code: cellCode }, members) {
  const flags = [];
  for (const code of TWIN_CODES) {
    const violating = members.filter((m) => m.answers[code] === "violation");
    if (!violating.length) continue;
    const contradicts = (answer) => answer === "none" || (answer === "uncertifiable" && kind === "legal" && code === cellCode);
    const contradicting = members.filter((m) => contradicts(m.answers[code]));
    if (!contradicting.length) continue;
    const flag = { code, codes: [code], violating: violating.map((m) => [m.layer, m.id, "violation"]), contradicting: contradicting.map((m) => [m.layer, m.id, m.answers[code]]) };
    const same = flags.find((other) => JSON.stringify([other.violating, other.contradicting]) === JSON.stringify([flag.violating, flag.contradicting]));
    if (same) same.codes.push(code); else flags.push(flag);
  }
  return flags;
}

/** The allow-list entry that holds a flag, or null: every twin on the smaller side of the flag is a layer the entry names. */
export function allowedBy(group, flag, allowed = ALLOWED) {
  const side = flag.violating.length <= flag.contradicting.length ? flag.violating : flag.contradicting;
  return allowed.find((entry) => (!entry.kind || entry.kind === group.kind) && (!entry.cell || entry.cell === group.code) && (!entry.context || entry.context === group.context)
    && (!entry.code || flag.codes.includes(entry.code)) && side.every(([layer]) => entry.layers.includes(layer))) ?? null;
}

/**
 * A cell's shapes: its base grid, and with `--recall` the recall grid's shapes too. A base context's recall shapes put the recall
 * layers under the base bindings, whose layer-`none` twin is in the base grid, so the two are taken together (each shape once).
 */
function cellShapes(base, recall) {
  const seen = new Set();
  return [...base, ...(recall ?? [])].filter((shape) => {
    const key = `${shape.variant ?? "-"}/${shape.binding}/${shape.layer}${axesKey(shape)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** The programs of the asked grids, with their group: `{ kind, cell, shape, group, make }`, generated as the grids generate them. */
export function twinPrograms(options) {
  const codes = options.codes?.split(",");
  const contexts = options.contexts?.split(",");
  const kinds = (options.grids ?? "legal,misuse").split(",");
  for (const kind of kinds) if (!["legal", "misuse"].includes(kind)) throw new Error(`--grids: legal and/or misuse, not ${kind}`);
  const out = [];
  if (kinds.includes("legal")) {
    for (const cell of legalCells({ codes, recall: Boolean(options.recall) }).filter((c) => !contexts || contexts.includes(c.context))) {
      const shapes = cellShapes(legalShapes(cell.code, ["root"], ["local"], cell.context), options.recall ? recallLegalShapes(cell.code, cell.context, { lean: !options.recallFull }) : null);
      for (const shape of shapes) out.push({ kind: "legal", cell, shape, group: groupKey("legal", cell.code, cell.context, shape), make: () => generateLegal(options, cell, { shape }).built });
    }
  }
  if (kinds.includes("misuse")) {
    for (const cell of cellList({ codes, contexts, recall: Boolean(options.recall) })) {
      // A shape's program as its grid generates it: the base grid's index for a base shape, the recall grid's for the others.
      const base = gridShapes(cell.code, cell.context);
      const recall = options.recall ? recallShapes(cell.code, cell.context, { lean: !options.recallFull }) : [];
      const indexOf = new Map([...recall.map((shape, index) => [shape, index]), ...base.map((shape, index) => [shape, index])]);
      cellShapes(base, recall).forEach((shape) => out.push({ kind: "misuse", cell, shape, group: groupKey("misuse", cell.code, cell.context, shape), make: () => generateShape(options, cell, shape, indexOf.get(shape)).built }));
    }
  }
  // Only groups with two twins or more say anything.
  const sizes = new Map();
  for (const program of out) sizes.set(program.group, (sizes.get(program.group) ?? 0) + 1);
  let kept = out.filter((program) => sizes.get(program.group) > 1);
  // `--sample`: a fraction of the groups, by a seeded hash of the group key, each group whole.
  if (options.sample) kept = kept.filter((program) => (fnv(`${options.sampleSeed ?? 1}:${program.group}`) % 100000) < Number(options.sample) * 100000);
  for (const program of kept) twinLayer(program.shape.layer);
  return kept;
}

/** The rules whose violations a flag's violating twins hold (for a person: which rule proved what its twins do not). */
function rulesOf(members, flag) {
  const ids = new Set(flag.violating.map(([, id]) => id));
  return [...new Set(members.filter((m) => ids.has(m.id)).flatMap((m) => m.findings).filter((label) => label.includes("/violation@")).map((label) => label.split("/")[0]).filter((rule) => flag.codes.some((code) => codeRules[code].includes(rule))))].sort();
}

async function pool(items, work, concurrency) {
  let next = 0;
  await Promise.all(Array.from({ length: Math.max(1, Math.min(concurrency, items.length)) }, async () => {
    while (next < items.length) { const index = next++; await work(items[index], index); }
  }));
}

export async function runTwins(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  const tsc = resolveTsc(options.tsc);
  requireTypeChecker(tsc);
  mkdirSync(out, { recursive: true });
  const install = options.install ?? DEFAULT_ORACLE_INSTALL;
  const concurrency = Number(options.concurrency ?? Math.max(1, cpus().length));
  const cache = options.noCache ? null : new EntryCache(cacheDirectory(options), "typecheck", typecheckEnvironmentKey({ tsc, install }));
  let programs = twinPrograms(options);
  if (options.limit) programs = programs.slice(0, Number(options.limit));
  const started = Date.now();
  console.error(`twins: ${programs.length} programs in ${new Set(programs.map((p) => p.group)).size} groups`);
  const rows = [];
  const chunk = 512;
  for (let start = 0; start < programs.length; start += chunk) {
    const part = programs.slice(start, start + chunk).map((program) => {
      const built = program.make();
      return { ...program, built, key: typecheckKeyOf(built, "typecheck") };
    });
    // Type checks: from the type-check cache (cache.mjs `typecheckEnvironmentKey`, shared with the grids), the rest in `tsrs -b` batches.
    const types = new Map();
    const fresh = [];
    for (const program of part) {
      const stored = cache?.get(program.key);
      if (stored !== undefined) types.set(program.built.id, stored);
      if (stored === undefined || cache.takeSpot("typecheck")) fresh.push(program);
    }
    for (const program of part) program.dir = materializeForChecker(program.built, install, `tw-${program.built.id}`);
    const groups = [];
    for (let index = 0; index < fresh.length; index += 32) groups.push(fresh.slice(index, index + 32));
    await pool(groups, async (group) => {
      const got = await typecheckBatch(group.map((p) => p.dir), { tsc });
      for (const program of group) {
        const answer = got?.get(program.dir) ?? await typecheck(program.dir, { tsc });
        const stored = types.get(program.built.id);
        if (stored !== undefined) {
          if (cacheableTypecheck(answer) && typecheckDigest(answer) !== typecheckDigest(stored)) throw new Error(`the type-check cache's answer for ${program.built.id} differs from a fresh one: remove ${cache.root} or pass --no-cache`);
          continue;
        }
        types.set(program.built.id, answer);
        if (cache && cacheableTypecheck(answer)) cache.set(program.key, answer);
      }
    }, concurrency);
    await pool(part, async (program) => {
      const row = { id: program.built.id, kind: program.kind, code: program.cell.code, context: program.cell.context, layer: program.shape.layer, group: program.group };
      if (!types.get(program.built.id)?.ok) rows.push({ ...row, outcome: "typed-away" });
      else {
        const checked = await runChecker(program.dir, { bin: options.checkerBin, typefacts: options.typefacts });
        if (!checked.ok) rows.push({ ...row, outcome: "harness", why: String(checked.error).slice(0, 200) });
        else rows.push({ ...row, outcome: "checked", answers: answersOf(checked.findings), findings: checked.findings.filter((f) => !ignoredRules.includes(f.rule)).map((f) => `${f.rule}/${f.kind}@${f.line}`) });
      }
      if (!options.keepWork) rmSync(program.dir, { recursive: true, force: true });
    }, concurrency);
    console.error(`twins: ${Math.min(start + chunk, programs.length)}/${programs.length}, ${Math.round((Date.now() - started) / 1000)}s`);
  }
  assertFindingsMapped();
  return report(rows, { ...options, out, started, cache });
}

/** A recorded row's findings as the checker gave them (`rule/kind@line` labels, twins.jsonl), for `answersOf`. */
const parsedFindings = (labels) => labels.map((label) => { const [rule, rest] = label.split("/"); const [kind, line] = rest.split("@"); return { rule, kind, line: Number(line) }; });

/** Judges the rows of a run (fresh, or `--from` an earlier run's twins.jsonl) and writes twins.json and twins.jsonl. */
function report(rows, { out, started, cache = null, ...options }) {
  for (const row of rows) if (row.outcome === "checked") row.answers = answersOf(parsedFindings(row.findings));
  const byGroup = new Map();
  for (const row of rows) { if (!byGroup.has(row.group)) byGroup.set(row.group, []); byGroup.get(row.group).push(row); }
  const flagged = [];
  const allowed = [];
  for (const [group, members] of byGroup) {
    const checked = members.filter((m) => m.outcome === "checked");
    if (checked.length < 2) continue;
    const [kind, code, context] = group.split("|");
    for (const flag of judgeGroup({ kind, code }, checked)) {
      const entry = allowedBy({ kind, code, context }, flag);
      (entry ? allowed : flagged).push({ group, kind, cell: code, context, ...flag, rules: rulesOf(checked, flag), ...(entry ? { reason: entry.reason } : {}) });
    }
  }
  const tally = (key) => Object.entries(flagged.reduce((acc, flag) => { acc[key(flag)] = (acc[key(flag)] ?? 0) + 1; return acc; }, {})).sort((a, b) => b[1] - a[1]);
  const summary = {
    programs: rows.length, groups: byGroup.size, checked: rows.filter((r) => r.outcome === "checked").length,
    typedAway: rows.filter((r) => r.outcome === "typed-away").length, harness: rows.filter((r) => r.outcome === "harness").length,
    flaggedGroups: new Set(flagged.map((f) => f.group)).size, flags: flagged.length, allowed: allowed.length,
    byCell: Object.fromEntries(tally((f) => `${f.kind}/${f.cell}/${f.context}/${f.code}`)),
    byViolatingLayer: Object.fromEntries(tally((f) => f.violating.map(([layer]) => layer).sort().join(","))),
    seconds: Math.round((Date.now() - started) / 1000), ...(cache ? { typecheckCache: cache.stats() } : {}),
    ...(options.sample ? { sample: { fraction: Number(options.sample), seed: options.sampleSeed ?? 1 } } : {}),
    ...(options.from ? { from: resolve(options.from) } : {}),
  };
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "twins.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n") + (rows.length ? "\n" : ""));
  writeFileSync(join(out, "twins.json"), `${JSON.stringify({ seed: options.seed, recall: Boolean(options.recall), recallLean: Boolean(options.recall) && !options.recallFull, grids: options.grids ?? "legal,misuse", codes: options.codes ?? null, contexts: options.contexts ?? null, summary, flagged, allowed }, null, 2)}\n`);
  console.log(JSON.stringify({ ...summary, byCell: undefined, byViolatingLayer: undefined }));
  for (const flag of flagged.slice(0, 20)) console.log(`twins: flagged ${flag.group} ${flag.code} [${flag.rules.join(",")}]: violation in ${flag.violating.map(([l]) => l).join(",")}; ${flag.contradicting.map(([l, , a]) => `${l}=${a}`).join(",")}`);
  if (flagged.length > 20) console.log(`twins: ... ${flagged.length - 20} more (${join(out, "twins.json")})`);
  return flagged.length ? 1 : 0;
}

/** `--from <dir>`: an earlier run's rows judged again (the judgement or the allow-list changed; the checker does not run). */
export function rejudge(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  const rows = readFileSync(join(resolve(options.from), "twins.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
  return report(rows, { ...options, out, started: Date.now() });
}

export function parse(argv) {
  const options = { seed: "1" };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (["recall", "recallLean", "recallFull", "keepWork", "noCache"].includes(key)) { options[key] = true; continue; }
    if (!["out", "seed", "codes", "contexts", "grids", "sample", "sampleSeed", "checkerBin", "typefacts", "tsc", "concurrency", "limit", "install", "cacheDir", "from"].includes(key) || argv[index + 1] === undefined) throw new Error(`twins.mjs: unknown or empty option ${argv[index]}`);
    options[key] = argv[++index];
  }
  if (!options.out) throw new Error("twins.mjs: --out <fresh dir>");
  if (options.sample !== undefined && !(Number(options.sample) > 0 && Number(options.sample) <= 1)) throw new Error(`--sample: a fraction in (0, 1], not ${options.sample}`);
  return options;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = parse(process.argv.slice(2));
  Promise.resolve().then(() => (options.from ? rejudge(options) : runTwins(options))).then((code) => { process.exitCode = code; }, (error) => { console.error(error.message); process.exitCode = 2; });
}
