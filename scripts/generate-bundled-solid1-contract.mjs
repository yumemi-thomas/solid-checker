// Generates pkg/contracts/bundled/solid-v1/solid-js.json, the bundled Solid 1.x
// (solid-js@1.9.14) package contract in the normalized schemaVersion-1
// document shape decoded by rust/crates/solid-facts-backend/src/contract_document.rs
// (shared summaries + per-entrypoint export groups, mirroring the layout of
// pkg/contracts/bundled/solid-v2/solid-js.json for Solid 2).
//
// Inputs (both checked in, read-only):
//
// 1. pkg/contracts/bundled/solid-v1/solid-js-runtime-surface.json — which
//    exports solid-js@1.9.14 actually has, under which entrypoints, and
//    whether each is a function or a value. Generated from the installed
//    package by scripts/generate-solid1-runtime-surface.mjs, which is where
//    the note on why a declaration census could not answer this lives.
//
// 2. rust/crates/solid-dialect/contracts/solid-v1/solid-js.json — the reviewed flat
//    semantics map (export name -> {kind, callbacks, returns}). It supplies
//    the callback/return summaries. Exports present in the surface but absent
//    from this map fall back to the plain "function" / "value" summary.
//
// Run from anywhere: node scripts/generate-bundled-solid1-contract.mjs

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { loadDialectManifests, root } from "./dialect-manifests.mjs";

const dialect = loadDialectManifests({ requireArtifacts: true }).find(
  manifest => manifest.id === "solid-v1",
);
const contract = dialect?.contracts.find(
  item => item.composeScript === "scripts/generate-bundled-solid1-contract.mjs",
);
if (!contract || contract.composeInputs?.length !== 1) {
  throw new Error("solid-v1 manifest must declare this composer and one surface input");
}
const surfacePath = join(root, contract.composeInputs[0]);
const semanticsPath = join(root, contract.reviewContract);
const outputPath = join(root, contract.bundledContract);

const surface = JSON.parse(readFileSync(surfacePath, "utf8"));
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
  if (entry.callbacks !== undefined) {
    if (Array.isArray(entry.callbacks)) {
      if (entry.callbacks.length > 0) {
        summary.callbacks = entry.callbacks.map(({ parameter, execution }) => ({
          parameter,
          execution,
        }));
      }
    } else if (entry.callbacks?.status === "unknown") {
      summary.callbacks = { status: "unknown" };
    } else {
      fail(`unsupported callbacks value ${JSON.stringify(entry.callbacks)}`);
    }
  }
  return summary;
}

if (surface.package?.name !== "solid-js") {
  fail(`${contract.composeInputs[0]} describes ${surface.package?.name}`);
}

// entrypoint subpath -> export name -> canonical summary object
const entrypointExports = new Map();
for (const [moduleSubpath, entry] of Object.entries(surface.entrypoints)) {
  const exports = new Map();
  for (const [exportName, kind] of Object.entries(entry.exports)) {
    const reviewed = semantics[exportName];
    if (reviewed !== undefined && reviewed.kind !== kind) {
      fail(
        `kind conflict for ${moduleSubpath}:${exportName}: the runtime says ${kind}, semantics say ${reviewed.kind}`,
      );
    }
    exports.set(exportName, reviewed === undefined ? { kind } : canonicalSummary(reviewed));
  }
  entrypointExports.set(moduleSubpath, exports);
}

// The contract covers the union of every build 1.x resolves, so its entrypoints
// are environment-agnostic: no environment condition selects a different
// contracted surface. The per-entrypoint `conditions` in the surface document
// are the resolution keys walked to reach each build, which is a different
// question and deliberately not what is recorded here. An export that a build
// omits would need per-condition variants instead; every export carrying a
// claim exists in all three builds, which the probe suite re-checks.
const conditions = ["default", "import"];

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
// layout of pkg/contracts/bundled/solid-v2/solid-js.json.
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
  // The exact tarball this model was read from. `scripts/check-contract-pins.mjs`
  // holds it to the registry, so a republished 1.9.14 stops matching instead of
  // silently becoming what this contract claims to describe.
  package: {
    name: "solid-js",
    version: "1.9.14",
    integrity:
      "sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==",
  },
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

const encoded = `${JSON.stringify(document, null, 2)}\n`;
if (process.argv.includes("--check")) {
  // Drift gate: the checked-in artifact is compiled into the backend, so an
  // edit to either input (the census or the reviewed semantics map) without
  // a regeneration must fail loudly instead of shipping a stale contract.
  const current = readFileSync(outputPath, "utf8");
  if (current !== encoded) {
    fail(
      `${outputPath} is stale relative to its inputs; re-run node scripts/generate-bundled-solid1-contract.mjs`,
    );
  }
  console.log(`ok   ${outputPath} matches its inputs`);
} else {
  writeFileSync(outputPath, encoded);
  console.log(`wrote ${outputPath}`);
}
for (const [subpath, exports] of entrypointExports) {
  console.log(`  ${subpath}: ${exports.size} exports`);
}
console.log(`  summaries: ${summaries.size}`);
