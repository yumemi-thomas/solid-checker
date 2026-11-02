// A deterministic, stratified sample of a grid's programs, for iterating on the two codes that are about 40% of a full run
// (STORE_WRITE_OUTSIDE_SETTER, ~250k programs, and REACTIVE_WRITE_IN_OWNED_SCOPE, ~110k). Never the PR gate: a full run
// (`focus.mjs --codes all` without `--sample-big`) keeps every program, and a sampled matrix is refused by the matrix gate.
//
// A stratum is a (code, context, layer, variant) of the grid. Each stratum keeps max(min(k, n), ceil(fraction * n)) of its n programs:
// the ones whose `fnv(<seed>:<id>)` is smallest, so the pick depends on the seed and the program ids only (not on the order the grid
// lists them, nor on the other strata), the same seed repeats it, and a program the grid keeps generating stays picked as the grid
// grows unless its stratum's quota moves. Programs of a code not in `codes` are all kept.
import { fnv } from "./rng.mjs";

/** The codes `focus.mjs --sample-big` samples: the two largest of a full run. */
export const BIG_CODES = ["STORE_WRITE_OUTSIDE_SETTER", "REACTIVE_WRITE_IN_OWNED_SCOPE"];

/** A grid entry's stratum: its code, context, layer and variant (the misuse plan the generator wrote). */
export function stratumOf(entry) {
  const plan = entry.program?.components?.[0]?.misuse ?? {};
  return `${entry.cell.code}/${entry.cell.context}/${plan.layer ?? entry.shape?.layer ?? "-"}/${plan.variant ?? entry.shape?.variant ?? "-"}`;
}

/** The grid sample's settings from the grids' options (`--grid-sample`, `--grid-sample-seed`, `--grid-sample-min`, `--grid-sample-codes`), or null. */
export function gridSampleOptions(options) {
  if (options.gridSample === undefined) return null;
  const fraction = Number(options.gridSample);
  if (!(fraction > 0 && fraction <= 1)) throw new Error(`--grid-sample: a fraction in (0, 1], not ${options.gridSample}`);
  const min = Number(options.gridSampleMin ?? 1);
  if (!Number.isInteger(min) || min < 0) throw new Error(`--grid-sample-min: a count, not ${options.gridSampleMin}`);
  if (options.ids) throw new Error("--grid-sample does not apply to --ids (a smoke manifest names its programs)");
  return { fraction, seed: String(options.gridSampleSeed ?? 1), min, codes: options.gridSampleCodes ? options.gridSampleCodes.split(",") : null };
}

/**
 * The entries a sample keeps, in the grid's order, and what it did: `{ entries, summary }`. `summary` is what a grid records
 * (`gridSample` in matrix.json and legal.json): the settings, `kept` of `of` programs of the sampled codes, and the strata.
 */
export function sampleEntries(entries, { fraction, seed, min = 1, codes = null }) {
  const sampled = (entry) => !codes || codes.includes(entry.cell.code);
  const strata = new Map();
  for (const entry of entries) {
    if (!sampled(entry)) continue;
    const key = stratumOf(entry);
    if (!strata.has(key)) strata.set(key, []);
    strata.get(key).push(entry);
  }
  const keep = new Set();
  for (const members of strata.values()) {
    const quota = Math.max(Math.min(min, members.length), Math.ceil(fraction * members.length));
    const ranked = members.map((entry) => ({ entry, rank: fnv(`${seed}:${entry.built.id}`) })).sort((a, b) => a.rank - b.rank || (a.entry.built.id < b.entry.built.id ? -1 : 1));
    for (const { entry } of ranked.slice(0, quota)) keep.add(entry);
  }
  const kept = entries.filter((entry) => !sampled(entry) || keep.has(entry));
  const of = [...strata.values()].reduce((sum, members) => sum + members.length, 0);
  return { entries: kept, summary: { fraction, seed, min, codes, strata: strata.size, kept: keep.size, of } };
}

/** One line for a grid's log and the focus summary. */
export const sampleLine = (summary) => `sampled ${summary.kept} of ${summary.of} programs${summary.codes ? ` of ${summary.codes.join(",")}` : ""} (fraction ${summary.fraction}, at least ${summary.min} per stratum of ${summary.strata}, seed ${summary.seed}); not a full run`;
