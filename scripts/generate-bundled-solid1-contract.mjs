// Generates pkg/contracts/bundled/solid-js-v1.json, the bundled Solid 1.x
// (solid-js@1.9.14) package contract in the normalized schemaVersion-1
// document shape decoded by rust/crates/solid-facts-backend/src/contract_document.rs
// (shared summaries + per-entrypoint export groups, mirroring the layout of
// pkg/contracts/bundled/solid-js.json for Solid 2).
//
// Inputs (both checked in, read-only):
//
// 1. pkg/contracts/bundled/solid-js-v1-census.json — the per-subpath export
//    census: a JSON array of wire-v2 contract units, one per
//    (moduleSubpath, exportName) of solid-js@1.9.14, materialized from the
//    1.x branch with:
//        git show 1.x:fixtures/bundled-contracts/bundled/solid-js.json \
//          > pkg/contracts/bundled/solid-js-v1-census.json
//    It decides WHICH exports exist under WHICH entrypoint (".", "./store",
//    "./web"), plus whether a name is a value (reactive-value-flow role
//    "ordinary") or a function (role "callable"). Its other facets use a
//    different vocabulary and are intentionally ignored here.
//
// 2. rust/crates/solid-dialect/contracts/solid-js-1x.json — the reviewed flat
//    semantics map (export name -> {kind, callbacks, returns}). It supplies
//    the callback/return summaries. Exports present in the census but absent
//    from this map fall back to the plain "function" / "value" summary.
//
// Run from anywhere: node scripts/generate-bundled-solid1-contract.mjs

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
const censusPath = join(root, "pkg/contracts/bundled/solid-js-v1-census.json");
const semanticsPath = join(
  root,
  "rust/crates/solid-dialect/contracts/solid-js-1x.json",
);
const outputPath = join(root, "pkg/contracts/bundled/solid-js-v1.json");

const census = JSON.parse(readFileSync(censusPath, "utf8"));
const semantics = JSON.parse(readFileSync(semanticsPath, "utf8")).exports;

function fail(message) {
  console.error(`generate-bundled-solid1-contract: ${message}`);
  process.exit(1);
}

// Rebuild each summary with the field order serde uses for ContractExport
// (kind, reactiveReads, returns, callbacks, asyncBehavior) so canonical JSON
// strings are stable for deduplication.
function canonicalSummary(entry) {
  const summary = { kind: entry.kind };
  if (entry.returns !== undefined) {
    summary.returns = { kind: entry.returns.kind, label: entry.returns.label };
  }
  if (entry.callbacks !== undefined && entry.callbacks.length > 0) {
    summary.callbacks = entry.callbacks.map(({ parameter, execution }) => ({
      parameter,
      execution,
    }));
  }
  return summary;
}

function censusKind(unit) {
  const roles = unit.facets?.reactiveValueFlow?.values?.[0]?.roles ?? [];
  if (roles.includes("ordinary")) return "value";
  if (roles.includes("callable")) return "function";
  fail(
    `census unit ${unit.scope.moduleSubpath}:${unit.scope.exportName} has no recognizable role`,
  );
}

// entrypoint subpath -> export name -> canonical summary object
const entrypointExports = new Map();
const conditionSets = new Set();
for (const unit of census) {
  const { moduleSubpath, exportName, exportConditions } = unit.scope;
  conditionSets.add(JSON.stringify(exportConditions));
  const kind = censusKind(unit);
  const reviewed = semantics[exportName];
  let summary;
  if (reviewed !== undefined) {
    if (reviewed.kind !== kind) {
      fail(
        `kind conflict for ${moduleSubpath}:${exportName}: census says ${kind}, semantics say ${reviewed.kind}`,
      );
    }
    summary = canonicalSummary(reviewed);
  } else {
    summary = { kind };
  }
  if (!entrypointExports.has(moduleSubpath)) {
    entrypointExports.set(moduleSubpath, new Map());
  }
  const exports = entrypointExports.get(moduleSubpath);
  if (exports.has(exportName)) {
    fail(`duplicate census unit ${moduleSubpath}:${exportName}`);
  }
  exports.set(exportName, summary);
}

if (conditionSets.size !== 1) {
  fail(`census units disagree on export conditions: ${[...conditionSets]}`);
}
const conditions = JSON.parse([...conditionSets][0]);

// Deduplicate summaries into shared identifiers, following the naming scheme
// of contract_document.rs normalize(): summaries with no effects keep their
// bare kind ("function" / "value"); the rest are numbered "<kind>-<n>" in
// canonical-JSON order.
const unique = new Map(); // canonical JSON -> summary object
for (const exports of entrypointExports.values()) {
  for (const summary of exports.values()) {
    unique.set(JSON.stringify(summary), summary);
  }
}
const counters = new Map();
const summaryIds = new Map(); // canonical JSON -> summary id
const summaries = new Map(); // summary id -> summary object
for (const canonical of [...unique.keys()].sort()) {
  const summary = unique.get(canonical);
  const plain = summary.returns === undefined && summary.callbacks === undefined;
  let id;
  if (plain) {
    id = summary.kind;
  } else {
    const counter = (counters.get(summary.kind) ?? 0) + 1;
    counters.set(summary.kind, counter);
    id = `${summary.kind}-${counter}`;
  }
  summaryIds.set(canonical, id);
  summaries.set(id, summary);
}

// Emit numbered summaries first, then the plain ones, matching the visual
// layout of pkg/contracts/bundled/solid-js.json.
const orderedSummaries = {};
for (const [id, summary] of [...summaries].sort(([a], [b]) => {
  const plainA = !a.includes("-");
  const plainB = !b.includes("-");
  if (plainA !== plainB) return plainA ? 1 : -1;
  return a < b ? -1 : a > b ? 1 : 0;
})) {
  orderedSummaries[id] = summary;
}

const entrypoints = {};
for (const subpath of [...entrypointExports.keys()].sort()) {
  const groups = new Map(); // summary id -> export names
  const exports = entrypointExports.get(subpath);
  for (const name of [...exports.keys()].sort()) {
    const id = summaryIds.get(JSON.stringify(exports.get(name)));
    if (!groups.has(id)) groups.set(id, []);
    groups.get(id).push(name);
  }
  const grouped = {};
  for (const id of [...groups.keys()].sort()) {
    grouped[id] = groups.get(id);
  }
  entrypoints[subpath] = { exports: grouped, conditions };
}

const document = {
  schemaVersion: 1,
  package: { name: "solid-js", version: "1.9.14" },
  compilerFactsProtocol: 1,
  summaries: orderedSummaries,
  entrypoints,
  evidence: {
    kind: "verified",
    generator: "solid-checker bundled Solid 1.x model",
  },
};

// Sanity checks from the Solid 1.x review before writing anything.
function summaryOf(subpath, name) {
  const summary = entrypointExports.get(subpath)?.get(name);
  if (summary === undefined) fail(`expected export ${subpath}:${name}`);
  return summary;
}
for (const name of [
  "batch",
  "onMount",
  "createResource",
  "createComputed",
  "onError",
  "catchError",
  "on",
  "untrack",
  "createSignal",
  "createMemo",
  "createEffect",
]) {
  summaryOf(".", name);
}
for (const name of ["render", "hydrate", "Dynamic", "Portal"]) {
  summaryOf("./web", name);
}
const createEffect = summaryOf(".", "createEffect");
if (
  JSON.stringify(createEffect.callbacks) !==
  JSON.stringify([{ parameter: 0, execution: "tracked" }])
) {
  fail("createEffect must run callback parameter 0 tracked");
}
const createSelector = summaryOf(".", "createSelector");
if (
  JSON.stringify(createSelector.callbacks) !==
  JSON.stringify([
    { parameter: 0, execution: "tracked" },
    { parameter: 1, execution: "inline" },
  ])
) {
  fail("createSelector must distinguish its tracked source from its inline comparator");
}
if (summaryOf(".", "children").returns?.kind !== "accessor") {
  fail("children must return an accessor");
}
if (summaryOf(".", "createDeferred").returns?.kind !== "accessor") {
  fail("createDeferred must return an accessor");
}
if (summaryOf("./store", "createMutable").returns?.kind !== "store-path") {
  fail("createMutable must return a store path");
}
summaryOf("./store", "createStore");
if (entrypointExports.get(".").has("createStore")) {
  fail("createStore must live under ./store only");
}

writeFileSync(outputPath, `${JSON.stringify(document, null, 2)}\n`);
console.log(`wrote ${outputPath}`);
for (const [subpath, exports] of entrypointExports) {
  console.log(`  ${subpath}: ${exports.size} exports`);
}
console.log(`  summaries: ${summaries.size}`);
