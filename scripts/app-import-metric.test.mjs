import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  ACCEPTANCE_GATE,
  CORPUS_PATH,
  DIALECT_OWNED,
  classifyProject,
  entrypointOf,
  environmentMismatch,
  gateOf,
  installCommand,
  measure,
  mergeProjects,
  moduleOfCallee,
  noContractCause,
  openCauses,
  packageMetricIndex,
  packageOfModule,
  rankWalls,
  renderMarkdown,
  roleOfPath,
  sitesOf,
  tierByPackage,
  valueBindings
} from "./app-import-metric.mjs";

const source = [
  'import { a, b as c, type T } from "m";',
  'import * as ns from "m";',
  'import d, { e } from "m";',
  'import { Link } from "@solidjs/router";',
  'import { createSignal } from "solid-js";',
  "ns.f();",
  "a(() => 1);"
].join("\n");
const data = Buffer.from(source);
const PATH = "/app/src/App.tsx";
const readSource = path => (path === PATH ? data : null);
const span = (text, occurrence = 0) => {
  let start = -1;
  for (let index = 0; index <= occurrence; index += 1) start = data.indexOf(text, start + 1);
  assert.ok(start >= 0, text);
  return { path: PATH, startByte: start, endByte: start + Buffer.byteLength(text) };
};
const first = span('import { a, b as c, type T } from "m";');
const third = span('import d, { e } from "m";');
const call = span("ns.f");
const member = { path: PATH, startByte: call.startByte + 3, endByte: call.startByte + 4 };
const router = span('import { Link } from "@solidjs/router";');
const solid = span('import { createSignal } from "solid-js";');

const acceptance = (module, primary, related = []) => ({
  id: "SC9005",
  analysisContext: ACCEPTANCE_GATE,
  message: `this project has no accepted reactivity contract for ${module}; solid-checker cannot tell …`,
  primaryLocation: primary,
  relatedLocations: related
});
const openClaim = (module, exportName, domains, primary, related = []) => ({
  id: "SC9005",
  analysisContext: `unknown-contract-claims:${domains}`,
  message: `the reactivity contract for ${module} leaves ${domains} unknown for imported export ${exportName}; code whose proof depends on those claims cannot be certified`,
  primaryLocation: primary,
  relatedLocations: related
});

test("value bindings follow the import declaration's own order and skip types and namespaces", () => {
  assert.deepEqual(valueBindings('import d, { a, b as c, type T } from "m";'), ["default", "a", "b"]);
  assert.deepEqual(valueBindings('import * as ns from "m";'), []);
  assert.equal(valueBindings('import { "a-b" as c } from "m";'), null);
  assert.equal(valueBindings('import type { T } from "m";'), null);
});

test("a collapsed acceptance-gate finding is attributed per site from the bytes", () => {
  const { gate, sites } = sitesOf(acceptance("m", first, [first, third, third, member]), readSource);
  assert.equal(gate, "acceptance");
  assert.deepEqual(
    sites.map(site => site.export).sort(),
    ["a", "b", "default", "e", "f"]
  );
  // One repeat for two value bindings: which one raised it is not in the bytes.
  const partial = sitesOf(acceptance("m", third), readSource);
  assert.deepEqual(partial.sites.map(site => site.export), ["?"]);
});

test("gates are read from the analysis context, not the sentence", () => {
  const missing = {
    id: "SC9005",
    analysisContext: "",
    message: "the reactivity contract for m has no entrypoint/export summary for imported export a; solid-checker …",
    primaryLocation: first
  };
  assert.equal(gateOf(missing), "export missing");
  assert.equal(gateOf({ ...missing, analysisContext: ACCEPTANCE_GATE }), "acceptance");
  assert.equal(gateOf(openClaim("m", "a", "returns", first)), "open claims");
  assert.equal(gateOf({ ...missing, analysisContext: "unbound-contract-claims:callback arguments" }), "unbound claims");
  assert.equal(gateOf({ id: "SC9011", message: "" }), null);
  assert.equal(
    gateOf({ id: "SC9005", message: "callback parameter 1 (T) of @x/y.:run reaches a call whose execution timing is unknown; this callback cannot be certified" }),
    "callback execution"
  );
});

test("a callback-execution callee names the package module, or the project", () => {
  assert.equal(moduleOfCallee("current project."), null);
  assert.equal(moduleOfCallee("@solidjs/router."), "@solidjs/router");
  assert.equal(moduleOfCallee("@solidjs/router./fs"), "@solidjs/router/fs");
});

test("packages and entrypoints of specifiers", () => {
  assert.equal(packageOfModule("@scope/name/sub/path"), "@scope/name");
  assert.equal(packageOfModule("name/sub"), "name");
  assert.equal(entrypointOf("@solidjs/router"), ".");
  assert.equal(entrypointOf("@solidjs/router/fs"), "./fs");
  assert.deepEqual(DIALECT_OWNED, ["solid-js", "@solidjs/signals", "@solidjs/web"]);
});

const resolvePackage = (_path, module) => {
  const name = packageOfModule(module);
  return { package: name, version: name === "m" ? "1.0.0" : "2.0.0", origin: name === "ws" ? "workspace" : "third-party" };
};

test("each tier-off site takes the shipped run's state for the same file, module and export", () => {
  const off = {
    findings: [
      acceptance("m", first, [first, third, third, member]),
      acceptance("@solidjs/router", router),
      acceptance("solid-js", solid)
    ]
  };
  const on = {
    packageSummaries: [{ name: "m", version: "1.0.0", evidence: "accepted" }],
    findings: [
      openClaim("m", "a", "reactiveReads,returns", first),
      { ...openClaim("m", "b", "x", first), analysisContext: "unbound-contract-claims:callback arguments", message: "the reactivity contract for m states callbacks for imported export b, but this call site gives the claim nothing to bind to" },
      acceptance("@solidjs/router", router),
      {
        id: "SC9005",
        message: "callback parameter 1 (T) of m.:e reaches a call whose execution timing is unknown; this callback cannot be certified",
        primaryLocation: first
      }
    ]
  };
  const { rows, onOnly } = classifyProject({ on, off, readSource, resolvePackage });
  const state = Object.fromEntries(rows.map(row => [`${row.module}:${row.export}`, row.state]));
  assert.deepEqual(state, {
    "m:a": "open",
    "m:b": "refused",
    "m:default": "certified",
    "m:e": "open",
    "m:f": "certified",
    "@solidjs/router:Link": "no contract"
  });
  assert.deepEqual(rows.find(row => row.export === "a").domains, ["reactiveReads", "returns"]);
  assert.deepEqual(rows.find(row => row.export === "e").domains, ["callbacks (execution)"]);
  assert.ok(!rows.some(row => row.module === "solid-js"), "the dialect owns solid-js");
  assert.deepEqual(onOnly, []);
});

test("no finding without an accepted contract is unexplained, never certified", () => {
  const off = { findings: [acceptance("@solidjs/router", router)] };
  const { rows } = classifyProject({ on: { findings: [], packageSummaries: [] }, off, readSource, resolvePackage });
  assert.equal(rows[0].state, "unexplained");
});

test("a file two projects analyse counts once, from the first project", () => {
  const row = { path: "a.tsx", module: "m", export: "x", sites: 1, state: "open" };
  const merged = mergeProjects([
    { project: "one", rows: [row] },
    { project: "two", rows: [{ ...row, state: "certified" }, { ...row, export: "y" }] }
  ]);
  assert.deepEqual(merged.map(entry => [entry.export, entry.state, entry.project]), [["x", "open", "one"], ["y", "open", "two"]]);
});

const tier = tierByPackage({
  bundles: [
    { packageName: "@solidjs/meta", packageVersion: "1.0.0-next.2", specifier: "@solidjs/meta" },
    { packageName: "@tanstack/solid-router", packageVersion: "2.0.0-rc.8", specifier: "@tanstack/solid-router/ssr/client" }
  ]
});

test("a no-contract site is explained against the tier", () => {
  const row = (module, version, admission = []) => ({ module, package: packageOfModule(module), version, admission });
  assert.equal(noContractCause(row("@solidjs/router", "2.0.0-next.26"), tier).class, "no contract: package not in the tier");
  assert.equal(noContractCause(row("@solidjs/meta", "1.0.0-next.1"), tier).class, "no contract: installed version not in the tier");
  assert.equal(noContractCause(row("@tanstack/solid-router", "2.0.0-rc.8"), tier).class, "no contract: specifier not in the tier");
  const environment = noContractCause(row("@solidjs/meta", "1.0.0-next.2", ["admission refused: signals rc.3"]), tier);
  assert.equal(environment.class, "no contract: environment not admitted");
  assert.equal(environment.detail, "admission refused: signals rc.3");
});

test("the checker's admission sentence outranks the tier comparison", () => {
  const row = { module: "@tanstack/solid-router", package: "@tanstack/solid-router", version: "2.0.0-rc.8" };
  const integrity = noContractCause({ ...row, admission: ["the installed package has no exact lockfile integrity, so its acceptance root cannot be reproduced"] }, tier);
  assert.equal(integrity.class, "no contract: lockfile integrity not read");
  const version = noContractCause({ ...row, version: "2.0.0-rc.4", admission: ["the installed package is 2.0.0-rc.4, not the certified 2.0.0-rc.8"] }, tier);
  assert.deepEqual([version.class, version.key], ["no contract: installed version not in the tier", "@tanstack/solid-router@2.0.0-rc.4 (tier certified 2.0.0-rc.8)"]);
});

test("an environment mismatch names the runtime entry first", () => {
  const entries = [
    { name: "@tanstack/history", version: "1.0.0" },
    { name: "@solidjs/signals", version: "2.0.0-rc.9" }
  ];
  assert.deepEqual(environmentMismatch(entries, { "@solidjs/signals": { version: "2.0.0-rc.4" } }), { name: "@solidjs/signals", bundle: "2.0.0-rc.9", installed: "2.0.0-rc.4" });
  assert.deepEqual(environmentMismatch(entries, { "@solidjs/signals": { version: "2.0.0-rc.9" } }), { name: "@tanstack/history", bundle: "1.0.0", installed: null });
  assert.equal(environmentMismatch(entries, { "@solidjs/signals": { version: "2.0.0-rc.9" }, "@tanstack/history": { version: "1.0.0" } }), null);
});

test("test files and build configs are not application sites", () => {
  assert.equal(roleOfPath("src/App.test.tsx"), "test");
  assert.equal(roleOfPath("src/__tests__/a.tsx"), "test");
  assert.equal(roleOfPath("tests/setup.ts"), "test");
  assert.equal(roleOfPath("src/testing.tsx"), "test");
  assert.equal(roleOfPath("vite.config.ts"), "tooling");
  assert.equal(roleOfPath("apps/web/app.config.ts"), "tooling");
  assert.equal(roleOfPath("src/routes/index.tsx"), "app");
  assert.equal(roleOfPath("src/latest/view.tsx"), "app");
});

test("an open site's domains join the package side's own causes at the same version", () => {
  const index = packageMetricIndex({
    packages: [
      {
        package: "@solidjs/meta",
        version: "1.0.0-next.2",
        exports: [{ entrypoint: ".", export: "Title", bucket: "degenerate", causes: [{ domain: "reads", class: "declined", key: "unresolved-callee" }] }]
      }
    ]
  });
  const site = { package: "@solidjs/meta", version: "1.0.0-next.2", module: "@solidjs/meta", export: "Title", domains: ["reactiveReads", "ownerRequirements"] };
  const causes = openCauses(site, index);
  assert.deepEqual(causes[0], { class: "open: reads", key: "declined: unresolved-callee", package: "@solidjs/meta@1.0.0-next.2" });
  assert.match(causes[1].key, /closed on the package side/);
  assert.match(openCauses({ ...site, version: "9" }, index)[0].key, /not in the certification-metric corpus/);
});

test("walls rank by sites and count a site as solely blocked only when it has one cause", () => {
  const walls = rankWalls([
    { app: "x", package: "p", version: "1", module: "p", export: "a", sites: 3, causes: [{ class: "c", key: "k1" }] },
    { app: "y", package: "p", version: "1", module: "p", export: "b", sites: 2, causes: [{ class: "c", key: "k1" }, { class: "c", key: "k2" }] }
  ]);
  assert.deepEqual(walls.map(wall => [wall.key, wall.sites, wall.sole, wall.apps]), [["k1", 5, 3, 2], ["k2", 2, 0, 1]]);
});

test("the headline pools third-party sites and leaves workspace packages out", () => {
  const rows = [
    { path: "a", module: "m", export: "a", sites: 3, state: "certified", origin: "third-party", package: "m", version: "1", domains: [], gates: [] },
    { path: "a", module: "m", export: "b", sites: 1, state: "no contract", origin: "third-party", package: "m", version: "1", domains: [], gates: [] },
    { path: "a", module: "ws", export: "c", sites: 9, state: "no contract", origin: "workspace", package: "ws", version: "0", domains: [], gates: [] }
  ];
  const result = measure({
    apps: [
      { id: "one", status: "analysed", projects: [{ project: "p", rows }] },
      { id: "two", status: "analysed", projects: [{ project: "p", rows: [{ ...rows[1], path: "b" }] }] },
      { id: "three", status: "refused", refusal: "install failed" }
    ],
    tierIndex: { bundles: [] }
  });
  assert.equal(result.headline.pooled.total, 5);
  assert.equal(result.headline.pooled.certified, 3);
  assert.equal(result.headline.pooled.certifiedShare, 0.6);
  assert.equal(result.headline.meanPerApp, (0.75 + 0) / 2);
  assert.equal(result.headline.workspace.total, 9);
  assert.equal(result.walls[0].class, "no contract: package not in the tier");
  assert.equal(result.walls[0].sites, 2);
  assert.match(renderMarkdown(result), /\*\*3 \(60\.0 %\)\*\*/);
});

test("the install command is frozen to the lockfile and runs no scripts", () => {
  assert.deepEqual(installCommand("pnpm-lock.yaml", "pnpm@11.20.0+sha512.abc").args, ["-y", "pnpm@11.20.0", "install", "--frozen-lockfile", "--ignore-scripts"]);
  assert.equal(installCommand("a/pnpm-lock.yaml", null, "lockfileVersion: '9.0'").manager, "pnpm@10.33.2");
  assert.deepEqual(installCommand("package-lock.json").args, ["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
  assert.ok(installCommand("bun.lock").args.includes("--frozen-lockfile"));
  assert.ok(installCommand("yarn.lock", "yarn@4.5.0").args.includes("--immutable"));
});

test("every pinned app names a commit, a lockfile digest, its projects and why it qualifies", () => {
  const corpus = JSON.parse(readFileSync(CORPUS_PATH, "utf8"));
  const ids = new Set();
  for (const app of corpus.apps) {
    assert.ok(!ids.has(app.id), `duplicate id ${app.id}`);
    ids.add(app.id);
    assert.match(app.commit, /^[0-9a-f]{40}$/, app.id);
    assert.match(app.lockfileDigest, /^sha256:[0-9a-f]{64}$/, app.id);
    assert.ok(app.projects.length > 0, app.id);
    assert.ok(app.why && app.why.length > 20, app.id);
  }
  for (const entry of corpus.excluded ?? []) assert.ok(entry.reason, entry.repository);
});
