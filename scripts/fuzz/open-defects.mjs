// The open defects: real false positives and false cleans the fuzzer finds and the checker does not fix yet. Keeping them out of the
// generators would hide them, and leaving them in would fail every run, so they are listed (open-defects.json) and held here:
//   - a listed program that still fails is printed by every gate, with its count, and does not fail the run;
//   - a program that fails and is not listed fails the run, as any problem does;
//   - a listed program that no longer fails also fails the run, until its entry leaves the list for rediscovery.json: a fix turns the
//     program green, and the list cannot go stale or pass quietly.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const OPEN_DEFECTS = join(here, "open-defects.json");
export const readOpenDefects = (path = OPEN_DEFECTS) => JSON.parse(readFileSync(path, "utf8"));

/** The entry holding a grid row (`{ idPrefix, code, variantPrefix?, contexts?, partial?, outcome, item }`), or null. */
export function heldGridEntry(list, row) {
  return (list.grids ?? []).find((entry) => String(row.id).startsWith(entry.idPrefix) && entry.code === row.code
    && (!entry.contexts || entry.contexts.includes(row.context)) && (!entry.variantPrefix || String(row.variant ?? "").startsWith(entry.variantPrefix))) ?? null;
}

/** A grid row against the list: `held` (listed, still failing), `fixed` (listed, judged, and no longer failing), or null. */
export function gridStanding(list, row, problem) {
  const entry = heldGridEntry(list, row);
  if (!entry) return null;
  if (problem === entry.outcome) return { kind: "held", entry };
  // `partial`: the entry names a grid some of whose rows fail, so a passing row says nothing about the rest.
  return !entry.partial && !problem && ["agree", "clean", "uncertifiable", "recall-gap"].includes(row.outcome) ? { kind: "fixed", entry } : null;
}

/** The line every gate prints: what is open, so a green run still says it. */
export function heldLine(grid, entries) {
  const byItem = {};
  for (const entry of entries) byItem[entry.item] = (byItem[entry.item] ?? 0) + 1;
  return `known open ${grid}: ${entries.length} programs still fail (${Object.entries(byItem).map(([item, count]) => `${item} ${count}`).join(", ")}; scripts/fuzz/open-defects.json)`;
}
