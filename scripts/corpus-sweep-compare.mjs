#!/usr/bin/env node
// Compare two corpus-sweep reports by finding key, not by count.
//
//   node scripts/corpus-sweep-compare.mjs <before.json> <after.json> [--list N]
//
// A finding key is (app, project, id, kind, path, startByte, endByte). The
// output lists removed and added keys grouped by id and kind, removals per
// app, and the first N added keys (default 20).
//
// "0 added" is not a soundness gate: an unproven clean claim is a finding
// that is missing, and this comparison only sees which keys moved.
import { readFileSync } from "node:fs";

const [beforePath, afterPath, ...rest] = process.argv.slice(2);
if (!beforePath || !afterPath) throw new Error("usage: corpus-sweep-compare.mjs <before> <after> [--list N]");
const listIndex = rest.indexOf("--list");
const listLimit = listIndex >= 0 ? Number(rest[listIndex + 1]) : 20;

function load(path) {
  const report = JSON.parse(readFileSync(path, "utf8"));
  const keys = new Map();
  for (const result of report.results) {
    for (const finding of result.findings) {
      const location = finding.primaryLocation ?? {};
      const key = [
        result.app, result.project, finding.id, finding.kind,
        location.path, location.startByte, location.endByte,
      ].join("|");
      keys.set(key, (keys.get(key) ?? 0) + 1);
    }
  }
  return keys;
}

const before = load(beforePath);
const after = load(afterPath);
const removed = [...before.keys()].filter((key) => !after.has(key));
const added = [...after.keys()].filter((key) => !before.has(key));

function byIdAndKind(keys) {
  const totals = {};
  for (const key of keys) {
    const [, , id, kind] = key.split("|");
    totals[`${id} ${kind}`] = (totals[`${id} ${kind}`] ?? 0) + 1;
  }
  return totals;
}
function byApp(keys) {
  const totals = {};
  for (const key of keys) {
    const app = key.split("|")[0];
    totals[app] = (totals[app] ?? 0) + 1;
  }
  return totals;
}

console.log("removed", removed.length, byIdAndKind(removed));
console.log("added", added.length, byIdAndKind(added));
console.log("removed by app", byApp(removed));
console.log("added by app", byApp(added));
for (const key of added.slice(0, listLimit)) console.log("ADD", key);
