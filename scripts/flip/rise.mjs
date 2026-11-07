// The uncertifiable rise between two sweeps, by row and by distinct span, split test/source (Fable's U review, item 1). A report, no gate:
//   node scripts/flip/rise.mjs <old sweep.json> <new sweep.json> [--top 15]
// A span is (path, startByte, endByte, code, blocker): a row a kui kernel file gets once per tsconfig project is one span, and so is the
// same project counted twice. A test file is `*.test.*` / `*.spec.*`, or a path under `__tests__/`, `test/` or `tests/`.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const isTestPath = (path) => /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(path) || /(?:^|\/)(?:__tests__|tests?)\//.test(path);

/** The uncertifiable rows of a sweep: `{ key, span, app, path, code, blocker, test }`. */
export function rowsOf(sweep) {
  const rows = [];
  for (const result of sweep.results ?? []) {
    for (const finding of result.findings ?? []) {
      if (finding.kind !== "uncertifiable") continue;
      const at = finding.primaryLocation ?? {};
      const path = String(at.path ?? "");
      const blocker = finding.analysisContext ?? "";
      const span = `${path}|${at.startByte}|${at.endByte}|${finding.id}|${blocker}`;
      rows.push({ span, app: result.app, path, code: finding.id, blocker, test: isTestPath(path) });
    }
  }
  return rows;
}

/** Old and new rows compared as multisets of spans (rows) and as sets (distinct spans). */
export function rise(oldRows, newRows) {
  const count = (rows) => { const map = new Map(); for (const row of rows) map.set(row.span, (map.get(row.span) ?? 0) + 1); return map; };
  const was = count(oldRows);
  const now = count(newRows);
  const byspan = new Map([...oldRows, ...newRows].map((row) => [row.span, row]));
  const out = { rows: { old: oldRows.length, new: newRows.length }, spans: { old: was.size, new: now.size }, added: [], removed: [] };
  for (const [span, n] of now) if (n > (was.get(span) ?? 0)) out.added.push({ ...byspan.get(span), rows: n - (was.get(span) ?? 0), newSpan: !was.has(span) });
  for (const [span, n] of was) if (n > (now.get(span) ?? 0)) out.removed.push({ ...byspan.get(span), rows: n - (now.get(span) ?? 0), goneSpan: !now.has(span) });
  return out;
}

function summary(list, field) {
  const sum = (items) => items.reduce((total, item) => total + item.rows, 0);
  const spans = list.filter((item) => item[field]);
  return { rows: sum(list), spans: spans.length, testRows: sum(list.filter((item) => item.test)), testSpans: spans.filter((item) => item.test).length };
}

function top(list, keyOf, n) {
  const map = new Map();
  for (const item of list) { const key = keyOf(item); const was = map.get(key) ?? { rows: 0, spans: 0, test: 0 }; was.rows += item.rows; was.spans += item.newSpan || item.goneSpan ? 1 : 0; was.test += item.test ? item.rows : 0; map.set(key, was); }
  return [...map].sort((a, b) => b[1].rows - a[1].rows).slice(0, n);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [oldPath, newPath] = process.argv.slice(2).filter((arg) => !arg.startsWith("--"));
  const at = process.argv.indexOf("--top");
  const n = at < 0 ? 15 : Number(process.argv[at + 1]);
  const result = rise(rowsOf(JSON.parse(readFileSync(oldPath, "utf8"))), rowsOf(JSON.parse(readFileSync(newPath, "utf8"))));
  const added = summary(result.added, "newSpan");
  const removed = summary(result.removed, "goneSpan");
  console.log(`rows ${result.rows.old} -> ${result.rows.new}; distinct spans ${result.spans.old} -> ${result.spans.new}`);
  console.log(`added: ${added.rows} rows (${added.testRows} in test files), ${added.spans} new spans (${added.testSpans} in test files, ${added.spans - added.testSpans} source)`);
  // Fable's "withheld spans": a location (path, startByte, endByte) with no uncertifiable row of any code before, so a second blocker
  // stacked on a span that already had one is not a new place.
  const place = (item) => item.span.split("|").slice(0, 3).join("|");
  const oldPlaces = new Set(rowsOf(JSON.parse(readFileSync(oldPath, "utf8"))).map(place));
  const newPlaces = [...new Map(result.added.filter((item) => !oldPlaces.has(place(item))).map((item) => [place(item), item])).values()];
  console.log(`added at locations with no U before: ${newPlaces.length} (${newPlaces.filter((item) => item.test).length} in test files, ${newPlaces.filter((item) => !item.test).length} source)`);
  console.log(`removed: ${removed.rows} rows (${removed.testRows} in test files), ${removed.spans} spans gone (${removed.testSpans} in test files, ${removed.spans - removed.testSpans} source)`);
  for (const [label, list] of [["added", result.added], ["removed", result.removed]]) {
    console.log(`\n${label} by code and blocker (rows, spans, rows in test files):`);
    for (const [key, v] of top(list, (item) => `${item.code} ${item.blocker}`, n)) console.log(`  ${v.rows}\t${v.spans}\t${v.test}\t${key}`);
    console.log(`${label} by app:`);
    for (const [key, v] of top(list, (item) => item.app, n)) console.log(`  ${v.rows}\t${v.spans}\t${v.test}\t${key}`);
  }
}
