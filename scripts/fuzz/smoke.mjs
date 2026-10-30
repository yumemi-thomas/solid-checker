// The fuzzer's smoke set (`make fuzz-smoke`): a few programs of each full grid that cover what the whole grid covers, chosen from a full
// run and kept as a manifest (scripts/fuzz/smoke/<grid>.json), run after any checker change in seconds where the full grids take most of
// an hour. Chosen to cover, per grid:
//   - every behaviour the run saw: a code, context and outcome with the checker's findings at the site (rule/kind, lines dropped);
//   - every value of every axis (context, variant, binding, layer, mount, source, a combination's extra codes and decoys) for each code,
//     and every pair of the first six for each code: a regression confined to one wrapper, mount or source has a program here;
//   - every program a recorded run judged a problem (false clean, wrong rule, false positive, not legal, harness), and every
//     program rediscovery.json lists for a defect found earlier.
// The cover is greedy (the program covering most of what is still uncovered first; ties by a hash of the id, so the same run gives
// the same set). A set is only as good as what it catches: `check` replays a full run of another checker against the reference run and
// reports every program whose verdict moved that the set does not hold (`node scripts/fuzz/smoke.mjs check`).
//
// Each manifest also records the behaviour every program had in the run it was chosen from; a smoke run (focus.mjs --smoke) reports
// every program whose behaviour moved, and fails on a move to a worse outcome (`movedVerdicts`). After an intended change, reselect
// from the full run before the pull request.
//
//   node scripts/fuzz/smoke.mjs select --from <full run: one directory per grid> [--out scripts/fuzz/smoke] [--per-behaviour 5]
//   node scripts/fuzz/smoke.mjs check --reference <full run> --against <full run of another checker> [--manifests scripts/fuzz/smoke]
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GENERATOR_VERSION } from "./generate.mjs";
import { fnv } from "./rng.mjs";

const here = dirname(fileURLToPath(import.meta.url));
/** Where the manifests live. */
export const SMOKE_DIR = join(here, "smoke");
const AXES = ["context", "variant", "binding", "layer", "mount", "source"];
const PROBLEMS = ["false-clean", "wrong-rule", "harness", "false-positive", "not-legal"];

/** A grid's rows (programs.jsonl of its output directory). */
export const readRows = (dir) => readFileSync(join(dir, "programs.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

/** The behaviour of a row: code, context, outcome, and the findings at its site without their lines. */
export const behaviourOf = (row) => [row.code, row.context, row.outcome, [...new Set((row.detail?.findings ?? []).map((f) => String(f).replace(/@\d+$/, "")))].sort().join("+"), row.detail?.placed ?? ""].join("|");

/** The defects found so far and the programs that rediscover them (rediscovery.json; AGENTS.md, the rediscovery rule). */
export const REDISCOVERY = join(here, "rediscovery.json");
export const readRediscovery = (file = REDISCOVERY) => JSON.parse(readFileSync(file, "utf8"));

/** What a row covers (see the file comment); a program that rediscovers a recorded defect is its own feature, so the set holds it. */
export function featuresOf(row, required) {
  required = required instanceof Set ? required : new Set();
  const values = AXES.map((axis) => [axis, row[axis] ?? "-"]);
  const features = [`b:${behaviourOf(row)}`, ...values.map(([axis, value]) => `a:${row.code}|${axis}=${value}`)];
  for (let i = 0; i < values.length; i += 1) for (let j = i + 1; j < values.length; j += 1) features.push(`p:${row.code}|${values[i].join("=")}|${values[j].join("=")}`);
  // A combination's extra codes and decoys each once per code (paired with every axis value they would be most of the grid).
  for (const extra of row.combo?.extras ?? []) features.push(`a:${row.code}|extra=${extra}`);
  if (row.combo) features.push(`a:${row.code}|decoys=${Boolean(row.combo.decoys)}`);
  if (PROBLEMS.includes(row.outcome) || required.has(row.id)) features.push(`x:${row.id}`);
  return features;
}

/**
 * The greedy cover of `rows`' features: the chosen ids, in the order chosen. A behaviour is held by `perBehaviour` programs (as many as
 * the run has), every other feature by one.
 */
export function cover(rows, { perBehaviour = 1, required = new Set() } = {}) {
  const features = rows.map((row) => [...new Set(featuresOf(row, required))]);
  const need = new Map();
  for (const list of features) for (const feature of list) need.set(feature, feature.startsWith("b:") ? Math.min(perBehaviour, (need.get(feature) ?? 0) + 1) : 1);
  const uncovered = { get size() { return remaining; } };
  let remaining = [...need.values()].reduce((a, b) => a + b, 0);
  const rank = rows.map((row) => fnv(String(row.id)));
  // A max-heap of (gain, then hash, then id). Lazy greedy: a row's gain only falls as others are chosen, so the top's gain is recomputed
  // and the row put back when it fell; a top whose fresh gain still leads is the best choice.
  const before = (a, b) => a.gain > b.gain || (a.gain === b.gain && (rank[a.index] < rank[b.index] || (rank[a.index] === rank[b.index] && rows[a.index].id < rows[b.index].id)));
  const heap = [];
  const push = (item) => {
    heap.push(item);
    for (let at = heap.length - 1; at > 0;) {
      const parent = (at - 1) >> 1;
      if (!before(heap[at], heap[parent])) break;
      [heap[at], heap[parent]] = [heap[parent], heap[at]];
      at = parent;
    }
  };
  const pop = () => {
    const top = heap[0];
    const last = heap.pop();
    if (heap.length) {
      heap[0] = last;
      for (let at = 0; ;) {
        const left = 2 * at + 1, right = left + 1;
        let best = at;
        if (left < heap.length && before(heap[left], heap[best])) best = left;
        if (right < heap.length && before(heap[right], heap[best])) best = right;
        if (best === at) break;
        [heap[at], heap[best]] = [heap[best], heap[at]];
        at = best;
      }
    }
    return top;
  };
  rows.forEach((row, index) => push({ index, gain: features[index].length }));
  const chosen = [];
  while (uncovered.size && heap.length) {
    const top = pop();
    const gain = features[top.index].filter((feature) => need.get(feature) > 0).length;
    if (gain === 0) continue;
    if (heap.length && gain < heap[0].gain) { push({ index: top.index, gain }); continue; }
    chosen.push(rows[top.index].id);
    for (const feature of features[top.index]) if (need.get(feature) > 0) { need.set(feature, need.get(feature) - 1); remaining -= 1; }
  }
  return chosen;
}

/**
 * `--ids <manifest>`: the entries the manifest names, in grid order. A name the grid no longer generates stops the run (reselect).
 * `sample` (a fraction) adds that share of the grid's other entries, picked by a hash of the id and `sampleSeed`: a different few each
 * run, so over many runs the smoke set reaches what its cover did not foresee. They have no recorded behaviour; only problems count.
 */
export function keepIds(entries, file, grid, seed, { sample = 0, sampleSeed = 0 } = {}) {
  const manifest = JSON.parse(readFileSync(file, "utf8"));
  if (manifest.generator !== GENERATOR_VERSION) throw new Error(`${file} was chosen under generator ${manifest.generator}, this is ${GENERATOR_VERSION}: reselect (scripts/fuzz/smoke.mjs select)`);
  // A grid's ids name its programs without the seed: another seed would compare other programs with the recorded behaviours.
  if (manifest.seed !== undefined && String(manifest.seed) !== String(seed)) throw new Error(`${file} was chosen from the grid of seed ${manifest.seed}, not ${seed}`);
  const wanted = new Set(manifest.ids);
  const named = entries.filter((entry) => wanted.has(entry.built.id)).length;
  if (named !== wanted.size) throw new Error(`${file} names ${wanted.size - named} programs this ${grid} grid does not generate: reselect (scripts/fuzz/smoke.mjs select)`);
  const sampled = (id) => sample > 0 && (fnv(`${sampleSeed}:${id}`) % 100000) < sample * 100000;
  return entries.filter((entry) => wanted.has(entry.built.id) || sampled(entry.built.id));
}

/** The grids of a full run directory (one directory per grid, each with programs.jsonl). */
const gridsIn = (dir) => readdirSync(dir).filter((name) => existsSync(join(dir, name, "programs.jsonl"))).sort();

/** Five programs per behaviour: 91% of the kinds of move an earlier checker made were held (scripts/fuzz/README.md, the smoke set). */
export const PER_BEHAVIOUR = 5;

function select(from, out, perBehaviour) {
  mkdirSync(out, { recursive: true });
  for (const grid of gridsIn(from)) {
    const rows = readRows(join(from, grid));
    const required = new Set(readRediscovery().filter((entry) => entry.grid === grid).flatMap((entry) => entry.ids));
    const ids = cover(rows, { perBehaviour, required });
    const byId = new Map(rows.map((row) => [row.id, behaviourOf(row)]));
    const table = [...new Set(ids.map((id) => byId.get(id)))].sort();
    const expected = ids.map((id) => table.indexOf(byId.get(id)));
    const report = ["matrix.json", "legal.json"].map((name) => join(from, grid, name)).find((path) => existsSync(path));
    const seed = report ? JSON.parse(readFileSync(report, "utf8")).seed : undefined;
    if (seed === undefined) throw new Error(`${join(from, grid)}: no matrix.json or legal.json to read the grid's seed from`);
    writeFileSync(join(out, `${grid}.json`), `${JSON.stringify({ grid, generator: GENERATOR_VERSION, seed, perBehaviour, programs: rows.length, behaviours: table, ids, expected }, null, 0)}\n`);
    console.log(`${grid.padEnd(8)} ${String(ids.length).padStart(5)} of ${rows.length} programs (${new Set(rows.map(behaviourOf)).size} behaviours)`);
  }
}

/** Outcomes from best to worst, a misuse grid's and a legal grid's: a move down this list is a regression. */
const RANK = [
  ["agree", "recall-gap", "not-fired", "typed-away", "harness", "wrong-rule", "false-clean"],
  ["clean", "uncertifiable", "unexecuted", "disturbed", "typed-away", "not-legal", "harness", "false-positive"],
];
const worse = (from, to) => RANK.some((rank) => rank.includes(from) && rank.includes(to) && rank.indexOf(to) > rank.indexOf(from));

/** A smoke run's programs whose behaviour moved from the manifest's: `{ worse, other, missing }` (moves `[{ id, from, to }]`, ids with no row). */
export function movedVerdicts(manifest, rows) {
  const expected = new Map(manifest.ids.map((id, index) => [id, manifest.behaviours[manifest.expected[index]]]));
  const seen = new Set(rows.map((row) => row.id));
  const moved = { worse: [], other: [], missing: manifest.ids.filter((id) => !seen.has(id)) };
  for (const row of rows) {
    const from = expected.get(row.id);
    const to = behaviourOf(row);
    if (from === undefined || from === to) continue;
    (worse(from.split("|")[2], row.outcome) ? moved.worse : moved.other).push({ id: row.id, from, to });
  }
  return moved;
}

/**
 * The verdicts that moved between a reference run and another checker's (same grids, same programs), and which of them the smoke set
 * holds: a moved program the set lacks is a miss when no program of the set moved the same way (same code, context, old and new
 * behaviour), and a miss of the move itself otherwise.
 */
export function compare(referenceRows, againstRows, ids) {
  const before = new Map(referenceRows.map((row) => [row.id, row]));
  const smoke = new Set(ids);
  const moved = [];
  for (const row of againstRows) {
    const old = before.get(row.id);
    if (old && behaviourOf(old) !== behaviourOf(row)) moved.push({ id: row.id, move: `${behaviourOf(old)} -> ${row.outcome}|${behaviourOf(row).split("|").slice(3).join("|")}`, problem: PROBLEMS.includes(row.outcome) });
  }
  const caughtMoves = new Set(moved.filter((m) => smoke.has(m.id)).map((m) => m.move));
  const moves = new Set(moved.map((m) => m.move));
  return {
    moved: moved.length, inSmoke: moved.filter((m) => smoke.has(m.id)).length,
    moves: moves.size, movesCaught: caughtMoves.size, missedMoves: [...moves].filter((move) => !caughtMoves.has(move)),
    problems: moved.filter((m) => m.problem).length, problemsInSmoke: moved.filter((m) => m.problem && smoke.has(m.id)).length,
    problemMovesMissed: [...new Set(moved.filter((m) => m.problem && !caughtMoves.has(m.move)).map((m) => m.move))],
  };
}

function check(reference, against, manifests) {
  for (const grid of gridsIn(reference)) {
    if (!existsSync(join(against, grid, "programs.jsonl")) || !existsSync(join(manifests, `${grid}.json`))) continue;
    const { ids } = JSON.parse(readFileSync(join(manifests, `${grid}.json`), "utf8"));
    const result = compare(readRows(join(reference, grid)), readRows(join(against, grid)), ids);
    console.log(`${grid.padEnd(8)} moved ${result.moved} (${result.inSmoke} in the set); kinds of move ${result.moves}, caught ${result.movesCaught}; problems ${result.problems} (${result.problemsInSmoke} in the set)`);
    for (const move of result.missedMoves.slice(0, 20)) console.log(`  missed: ${move}`);
  }
}

function main(argv) {
  const [command, ...rest] = argv;
  const option = (name, fallback) => { const at = rest.indexOf(`--${name}`); return at < 0 ? fallback : rest[at + 1]; };
  if (command === "select") return select(resolve(option("from")), resolve(option("out", SMOKE_DIR)), Number(option("per-behaviour", PER_BEHAVIOUR)));
  if (command === "check") return check(resolve(option("reference")), resolve(option("against")), resolve(option("manifests", SMOKE_DIR)));
  throw new Error("smoke.mjs select --from <full run> | check --reference <full run> --against <full run>");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
