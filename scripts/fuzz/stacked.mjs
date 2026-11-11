// Stacked claims (coordinator, Part 16): a violation on a line where Solid's first throw halted the run before the rule's own code could
// fire (classify.mjs offSiteVerdicts `stacked`). It scores `stacked-claim` only for a (first code, rule) pair stacked-pairs.json proves with
// a Chrome twin; any other is a false positive. patterns.mjs checks every pair's twins on each run (`stackedProblems`).
// A `repeat` claim is also settled per program (ADR 0953, option c; repeat-twin.mjs): where its own twin holds the claim is the agreement it is.
import { readFileSync } from "node:fs";
import { codesForRule } from "./codes.mjs";

const TABLE = JSON.parse(readFileSync(new URL("./stacked-pairs.json", import.meta.url), "utf8"));
export const STACKED_PAIRS = TABLE.pairs;
/** Patterns scored `stacked-claim` by exact id (`patternClaim`); no rule reaches them, each is listed with its pair. */
export const PATTERN_CLAIMS = TABLE.patternClaims;
export const pairOf = (row) => `${row.first}|${row.finding.split("@")[0]}`;

/**
 * A repeat claim a twin can settle (ADR 0953, option c): the rule's own code reported once elsewhere (`why` `repeat`, not a disturbed run's), of a pair
 * the table marks `repeat`. A claim of any other kind, or of an unproved pair, has no twin and is split as before.
 */
export const isRepeat = (row, pairs = STACKED_PAIRS) => row.verdict === "stacked" && row.why === "repeat" && pairs[pairOf(row)]?.kind === "repeat";

/**
 * Off-site rows (`{ finding, verdict, first? }`) split: false positives (an unproved stack among them), proved stacked claims, and `twinned`: the
 * repeat claims whose twin held (`twins`: finding label -> `{ holds }`, repeat-twin.mjs). A twinned claim is the agreement it is: it is neither
 * stacked nor wrong. Without a twin, or with one that failed, a repeat is split by its pair as it always was.
 */
export function stackedSplit(judged, pairs = STACKED_PAIRS, { twins = {} } = {}) {
  const twinned = judged.filter((row) => isRepeat(row, pairs) && twins[row.finding]?.holds === true);
  const open = judged.filter((row) => !twinned.includes(row));
  const stacked = open.filter((row) => row.verdict === "stacked" && pairs[pairOf(row)]);
  const wrong = open.filter((row) => row.verdict === "false-positive" || (row.verdict === "stacked" && !pairs[pairOf(row)]));
  return { wrong, stacked, twinned };
}

/** Why a pair's proof fails on a patterns run (`rows`: judged patterns by id; `records`: Chrome's, by id), as sentences. */
export function stackedProblems(rows, records, pairs = STACKED_PAIRS, { only } = {}) {
  const problems = [];
  for (const [pair, { patterns }] of Object.entries(pairs)) {
    const rule = pair.split("|")[1];
    for (const id of patterns) {
      if (only && !only.split(",").some((part) => id.includes(part))) continue;
      const row = rows.find((one) => one.id === id);
      if (!row) { problems.push(`${pair}: no pattern ${id}`); continue; }
      if (row.outcome !== "fine") problems.push(`${pair}: ${id} is ${row.outcome}, not fine`);
      if (!(records.get(id)?.codes ?? []).some((code) => codesForRule(rule).includes(code))) problems.push(`${pair}: Chrome reported no code of ${rule} in ${id}`);
    }
  }
  return problems;
}

/**
 * The pattern lane's `stacked-claim`, ADR 0887's rule unchanged: a false positive with the pair's rule alone, of an id `claims` lists, whose first-throw
 * code fired in Chrome on the pattern, and whose pair's twins hold in this same run. Anything else keeps its outcome: an unlisted pattern, or a failed
 * twin, is a false positive. `rows` are the run's judged rows, `records` Chrome's by id.
 */
export function patternClaim(row, rows, records, claims = PATTERN_CLAIMS, pairs = STACKED_PAIRS) {
  const pair = claims[row.id]?.pair;
  if (!pair || row.outcome !== "false-positive" || !pairs[pair]) return row;
  const [first, rule] = pair.split("|");
  if (row.rules?.length !== 1 || row.rules[0] !== rule) return row;
  if (!(records.get(row.id)?.codes ?? []).includes(first)) return row;
  if (stackedProblems(rows, records, { [pair]: pairs[pair] }).length) return row;
  return { ...row, outcome: "stacked-claim", pairs: [pair] };
}
