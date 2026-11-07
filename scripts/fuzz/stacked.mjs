// Stacked claims (coordinator, Part 16): a violation on a line where Solid's first throw halted the run before the rule's own code could
// fire (classify.mjs offSiteVerdicts `stacked`). It scores `stacked-claim` only for a (first code, rule) pair stacked-pairs.json proves with
// a Chrome twin; any other is a false positive. patterns.mjs checks every pair's twins on each run (`stackedProblems`).
import { readFileSync } from "node:fs";
import { codesForRule } from "./codes.mjs";

export const STACKED_PAIRS = JSON.parse(readFileSync(new URL("./stacked-pairs.json", import.meta.url), "utf8")).pairs;
export const pairOf = (row) => `${row.first}|${row.finding.split("@")[0]}`;

/** Off-site rows (`{ finding, verdict, first? }`) split: false positives (an unproved stack among them) and proved stacked claims. */
export function stackedSplit(judged, pairs = STACKED_PAIRS) {
  const stacked = judged.filter((row) => row.verdict === "stacked" && pairs[pairOf(row)]);
  const wrong = judged.filter((row) => row.verdict === "false-positive" || (row.verdict === "stacked" && !pairs[pairOf(row)]));
  return { wrong, stacked };
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
