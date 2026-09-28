import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  CORPUS_PATH,
  RUNTIME_FOUNDATION,
  causeOf,
  chooseProbe,
  corpusProblems,
  headline,
  measureRow,
  packageOfPath,
  rankWalls,
  renderHostSummary,
  selectCorpus,
  surfaceOf,
  unlockCurve,
  wideningFor
} from "./certification-metric.mjs";

const row = (name, probes, extra = {}) => ({
  package: name,
  solidTarget: "solid2",
  version: "1.0.0",
  integrity: `sha512-${name}`,
  family: "solid-primitives",
  probes,
  ...extra
});

test("a head probe answers the metric, an only probe when there is no head, never a floor", () => {
  const head = { id: "p|solid2|head", kind: "head" };
  const floor = { id: "p|solid2|floor", kind: "floor" };
  const only = { id: "p|solid2|only", kind: "only" };
  assert.equal(chooseProbe({ probes: [floor, head] }), head);
  assert.equal(chooseProbe({ probes: [only] }), only);
  assert.equal(chooseProbe({ probes: [floor] }), null);
});

test("the corpus is the top by downloads, and every skip above the cut-off is recorded", () => {
  const candidates = [
    { package: "solid-js", weeklyDownloads: 100, row: row("solid-js", [{ id: "s|head", kind: "head" }]) },
    { package: "a", weeklyDownloads: 50, row: row("a", [{ id: "a|floor", kind: "floor" }, { id: "a|head", kind: "head" }]) },
    { package: "outside", weeklyDownloads: 40, row: null, detail: "no family" },
    { package: "b", weeklyDownloads: 30, row: row("b", [{ id: "b|only", kind: "only" }]) },
    { package: "c", weeklyDownloads: 20, row: row("c", [{ id: "c|head", kind: "head" }]) }
  ];
  const exclusions = { "solid-js": { reason: "runtime-foundation", detail: "ADR 0027" } };
  const { packages, skipped } = selectCorpus({ candidates, exclusions, size: 2 });
  assert.deepEqual(packages.map(entry => [entry.rank, entry.package, entry.probe]), [
    [1, "a", "a|head"],
    [2, "b", "b|only"]
  ]);
  // `c` ranks below the cut-off, so it is neither selected nor listed as skipped.
  assert.deepEqual(skipped.map(entry => [entry.package, entry.reason]), [
    ["solid-js", "runtime-foundation"],
    ["outside", "not-in-manifest"]
  ]);
});

test("a manifest that moved a pinned version is refused", () => {
  const corpus = { packages: [{ package: "a", version: "1.0.0", integrity: "sha512-a", probe: "a|head" }] };
  assert.deepEqual(corpusProblems(corpus, { rows: [row("a", [{ id: "a|head", kind: "head" }])] }), []);
  const moved = { rows: [row("a", [{ id: "a|head", kind: "head" }], { version: "1.0.1" })] };
  assert.equal(corpusProblems(corpus, moved).length, 1);
  assert.equal(corpusProblems(corpus, { rows: [] }).length, 1);
});

test("the checked-in corpus pins 30 packages and none of the runtime foundation", () => {
  const corpus = JSON.parse(readFileSync(CORPUS_PATH, "utf8"));
  assert.equal(corpus.packages.length, 30);
  for (const entry of corpus.packages) {
    assert.ok(!RUNTIME_FOUNDATION.includes(entry.package), entry.package);
    assert.ok(["head", "only"].includes(entry.probeKind), entry.probe);
    assert.equal(typeof entry.weeklyDownloads, "number");
  }
  const downloads = corpus.packages.map(entry => entry.weeklyDownloads);
  assert.deepEqual(downloads, [...downloads].sort((left, right) => right - left));
});

test("a wildcard-reached entrypoint is not part of the surface", () => {
  const surface = surfaceOf({
    entrypoints: {
      ".": { cases: [{ exports: { a: "s" } }] },
      "./src/dom.ts": { cases: [{ exports: { b: "s" } }] }
    }
  });
  assert.deepEqual([...surface], [".\u0000a"]);
});

test("causes are read from the reason, and the catch-all keeps the obligation's location", () => {
  const catchAll = causeOf(
    {
      status: "withheld",
      reason:
        'census refused: creates census refuses a resolved callee that is neither a default-library member, a dialect primitive under the negative authority, nor a declaration in this artifact\'s own runtime source: "omit" declared at /tmp/p/node_modules/@solidjs/signals/dist/types/store/utils.d.ts:1..2, called at /tmp/p/node_modules/@solidjs/meta/dist/index.js:3..4'
    },
    "callbacks"
  );
  assert.equal(catchAll.class, "attribution catch-all");
  assert.equal(catchAll.key, "callee in @solidjs/signals");
  assert.equal(catchAll.callee, "@solidjs/signals:omit");
  assert.equal(catchAll.location, "@solidjs/meta/dist/index.js");
  const scoped = causeOf(
    {
      status: "withheld",
      reason:
        'census refused: creates census refuses a resolved callee that is neither a default-library member, a dialect primitive under the negative authority, nor a declaration in this artifact\'s own runtime source: "createSignal" declared at /t/node_modules/solid-js/types/a.d.ts:1..2, called at /t/node_modules/x/dist/index.js:3..4; the dialect row solid-js@2.0.0-rc.9:createSignal:creates is scoped to the `browser` host target, and this certification requested [import]'
    },
    "creates"
  );
  assert.equal(scoped.key, "dialect row scoped to the browser host (solid-js)");
  assert.equal(causeOf({ status: "withheld", reason: "no recipe in corpus" }, "reads").class, "recipe");
  assert.deepEqual(causeOf({ status: "never proposed" }, "returns"), {
    class: "missing claim form",
    key: "returns never proposed"
  });
  assert.deepEqual(
    causeOf({ status: "declined", declined: { kind: "unaccepted-external-dependency", location: "./dist/a.js:@kobalte/utils" } }, "reads"),
    { class: "unaccepted dependency", key: "@kobalte/utils" }
  );
  assert.equal(causeOf({ status: "graph: never proposed" }, "reads").class, "graph lane: unrecorded");
  assert.equal(packageOfPath("/x/node_modules/@a/b/c.js"), "@a/b");
  assert.equal(packageOfPath("/x/node_modules/a/c.js"), "a");
});

// ADR 0158 § 3: an unresolved claim a `fallback-all` widening covers is the
// widening, not a missing claim form; a namespace-member callee with no
// package prints a placeholder, never `undefined`.
test("an attribution widening explains a never-proposed domain", () => {
  const widenings = [
    {
      entrypoint: ".",
      obligation: "PackageContractExportMissing",
      location: "<package-root>/dist/esm/routerStores.js:9:38",
      domains: ["callbacks", "ownerRequirements", "reactiveReads", "returns"],
      exports: ["createFileRoute", "useNavigate"]
    }
  ];
  const widening = wideningFor(widenings, "createFileRoute", "creates", ".");
  assert.equal(widening.location, "<package-root>/dist/esm/routerStores.js:9:38");
  assert.equal(wideningFor(widenings, "createFileRoute", "creates", "./ssr"), null);
  assert.equal(wideningFor(widenings, "Link", "returns", "."), null);
  assert.equal(wideningFor([{ ...widenings[0], domains: ["callbacks"] }], "useNavigate", "reads", "."), null);
  assert.deepEqual(causeOf({ status: "never proposed", widening }, "returns"), {
    class: "attribution widening",
    key: "fallback-all: PackageContractExportMissing",
    location: "<package-root>/dist/esm/routerStores.js:9:38"
  });
  assert.deepEqual(causeOf({ status: "declined", declined: { kind: "dialect-silent", callee: "createSignal" } }, "creates"), {
    class: "dialect-silent",
    key: "<no package>:createSignal"
  });
});

function fixtureRow() {
  const document = {
    package: { name: "@example/p" },
    entrypoints: {
      ".": {
        cases: [
          { exports: { value: "s-value", clean: "s-clean", open: "s-open", nothing: "s-nothing", uncertified: "s-missing" } }
        ]
      }
    },
    summaries: {
      "s-value": { shape: "plain", call: {} },
      "s-clean": { shape: "callable", call: { closed: ["callbacks", "reads", "returns", "creates"] } },
      "s-open": { shape: "callable", call: { closed: ["callbacks", "reads", "creates"], operations: [{ kind: "read", inputs: [{ kind: "parameter" }] }] } },
      "s-nothing": { shape: "callable", call: {} }
    }
  };
  const accepted = JSON.parse(JSON.stringify(document));
  delete accepted.entrypoints["."].cases[0].exports.uncertified;
  const claim = (exportName, domain) => ({
    artifact: { entrypoint: "." },
    subject: { artifactCase: "case-1", export: exportName, path: { domain } }
  });
  return {
    generated: document,
    documents: [{ document: accepted, graph: null }],
    proposal: {
      unresolvedClaims: [claim("open", "returns"), ...["callbacks", "reads", "returns"].map(domain => claim("nothing", domain))],
      closureCandidates: [claim("nothing", "creates")]
    },
    refusals: { declinedClosures: [] },
    audit: { withheldClosures: [{ artifactCase: "case-1", export: "nothing", domain: "creates", reason: "no recipe in corpus" }] }
  };
}

test("one row: buckets, misuse classes and causes per open domain", () => {
  const measured = measureRow(
    { package: "@example/p", version: "1.0.0", probeId: "p|head", class: "success", certificationAttempt: { status: "certified" } },
    fixtureRow()
  );
  assert.deepEqual(measured.counts, { clean: 2, partial: 1, degenerate: 1, uncertified: 1 });
  assert.equal(measured.misuseCapable, 1);
  assert.deepEqual(measured.misuse, { argumentRead: 1 });
  const open = measured.exports.find(entry => entry.export === "open");
  assert.deepEqual(open.causes.map(cause => [cause.domain, cause.key]), [["returns", "returns never proposed"]]);
  const nothing = measured.exports.find(entry => entry.export === "nothing");
  assert.deepEqual(
    nothing.causes.map(cause => [cause.domain, cause.class]),
    [
      ["callbacks", "missing claim form"],
      ["reads", "missing claim form"],
      ["returns", "missing claim form"],
      ["creates", "recipe"]
    ]
  );
  assert.equal(measured.exports.find(entry => entry.export === "uncertified").bucket, "uncertified");
});

test("a graph-lane answer is classified from its node's own records", () => {
  const summaries = {
    "s-declined": { shape: "callable", call: { closed: ["callbacks", "returns", "creates"] } },
    "s-unresolved": { shape: "callable", call: { closed: ["callbacks", "reads", "creates"] } },
    "s-bare": { shape: "callable", call: { closed: ["callbacks", "reads", "returns"] } },
    "s-other": { shape: "callable", call: { closed: ["callbacks", "reads", "returns"] } }
  };
  const document = {
    package: { name: "@example/g" },
    entrypoints: {
      ".": { cases: [{ exports: { declined: "s-declined", unresolved: "s-unresolved", bare: "s-bare" } }] },
      "./other": { cases: [{ exports: { other: "s-other" } }] }
    },
    summaries
  };
  const claim = (exportName, domain) => ({ export: exportName, path: { kind: "call", domain }, claimId: `claim:${exportName}` });
  const row = {
    lane: "published-graph",
    generated: document,
    // The root is named by its digest only (a `graphs/<g>/root/objects/`
    // document), the second entrypoint's node by nothing the records know.
    documents: [
      { document, graph: { node: "sha256:root", artifactCase: null } }
    ],
    audit: { withheldClosures: [], withheldOperations: [] },
    proposal: null,
    refusals: null,
    retainedProposalCases: 0,
    graphNodes: {
      cases: [{ digest: "sha256:root", package: "@example/g", version: "1.0.0", artifactCase: "case-root" }],
      records: [
        {
          package: "@example/g",
          version: "1.0.0",
          entrypoint: ".",
          conditions: ["import"],
          root: true,
          artifactCase: "case-root",
          declinedClosures: [{ export: "declined", domain: "reads", kind: "unresolved-callee", package: "", callee: "", location: "<package-root>/dist/index.js:1:2" }],
          refusals: [],
          withheldClaims: [],
          unresolvedClaims: [claim("unresolved", "returns")],
          closureCandidates: []
        }
      ]
    }
  };
  const measured = measureRow(
    { package: "@example/g", version: "1.0.0", probeId: "g|head", class: "success", certificationAttempt: { status: "certified", lane: "published-graph" } },
    row
  );
  const causes = name => measured.exports.find(entry => entry.export === name).causes.map(cause => [cause.domain, cause.class, cause.key]);
  assert.deepEqual(causes("declined"), [["reads", "declined", "unresolved-callee"]]);
  assert.deepEqual(causes("unresolved"), [["returns", "missing claim form", "returns never proposed"]]);
  // Recorded by the node, and in none of its lists: a real "no record".
  assert.deepEqual(causes("bare"), [["creates", "no record", "no record"]]);
  assert.deepEqual(measured.graphNodeRecords, { records: 1, truncatedRecords: 0, truncatedLists: 0, nodeCases: 1 });

  // A capped list cannot prove an absence, so it answers as a record gap.
  row.graphNodes.records[0].truncated = { declinedClosures: 3 };
  const capped = measureRow(
    { package: "@example/g", version: "1.0.0", probeId: "g|head", class: "success", certificationAttempt: { status: "certified", lane: "published-graph" } },
    row
  );
  assert.deepEqual(
    capped.exports.find(entry => entry.export === "bare").causes.map(cause => [cause.class, cause.key]),
    [["graph lane: unrecorded", "creates (node record truncated: declinedClosures)"]]
  );

  // An audit from before the lane kept node records stays an absence.
  const older = measureRow(
    { package: "@example/g", version: "1.0.0", probeId: "g|head", class: "success", certificationAttempt: { status: "certified", lane: "published-graph" } },
    { ...row, graphNodes: null }
  );
  assert.deepEqual(
    older.exports.find(entry => entry.export === "unresolved").causes.map(cause => [cause.class, cause.key]),
    [["graph lane: unrecorded", "returns"]]
  );
});

test("a graph answer no node record names is an attribution gap, not a cause", () => {
  const document = {
    package: { name: "@example/g" },
    entrypoints: { ".": { cases: [{ exports: { a: "s-a" } }] } },
    summaries: { "s-a": { shape: "callable", call: { closed: ["callbacks", "reads", "returns"] } } }
  };
  const measured = measureRow(
    { package: "@example/g", version: "1.0.0", probeId: "g|head", class: "success", certificationAttempt: { status: "certified", lane: "published-graph" } },
    {
      lane: "published-graph",
      generated: document,
      documents: [{ document, graph: { node: "sha256:unmapped", artifactCase: null } }],
      audit: {},
      graphNodes: { cases: [], records: [] },
      retainedProposalCases: 0
    }
  );
  assert.deepEqual(measured.exports[0].causes.map(cause => [cause.class, cause.key]), [
    ["graph lane: unrecorded", "creates (no node record for the answering case)"]
  ]);
});

test("headline weights per package and by downloads, an uncertifiable package scoring zero", () => {
  const result = headline([
    { counts: { clean: 1 }, total: 2, weeklyDownloads: 300 },
    { counts: { clean: 0 }, total: 0, weeklyDownloads: 100 }
  ]);
  assert.equal(result.perPackage, 0.25);
  assert.equal(result.byDownloads, 0.375);
  assert.equal(result.pooled, 0.5);
});

test("walls count an export once per cause, solely when it is the only cause, with demanded sites", () => {
  const packages = [
    {
      package: "p",
      exports: [
        { entrypoint: ".", export: "a", bucket: "partial", causes: [{ class: "recipe", key: "r" }, { class: "recipe", key: "r" }] },
        { entrypoint: ".", export: "b", bucket: "degenerate", causes: [{ class: "recipe", key: "r" }, { class: "census refusal", key: "c" }] },
        { entrypoint: ".", export: "c", bucket: "clean", causes: [] }
      ]
    }
  ];
  const { walls, classes } = rankWalls(packages, [{ package: "p", export: "a", sites: 7 }]);
  const recipe = walls.find(wall => wall.class === "recipe");
  assert.deepEqual([recipe.exports, recipe.sole, recipe.sites], [2, 1, 7]);
  assert.deepEqual(classes.map(group => [group.class, group.exports, group.sole]), [
    ["recipe", 2, 1],
    ["census refusal", 1, 0]
  ]);
});

test("the unlock curve adds the class that leaves the most exports with no remaining cause", () => {
  const item = (...classes) => ({ bucket: "partial", causes: classes.map(group => ({ class: group, key: group })) });
  const curve = unlockCurve([
    { exports: [item("a", "b"), item("a", "b"), item("c"), { bucket: "clean", causes: [] }] }
  ]);
  // `c` alone clears one export and `a` or `b` alone clears none, so `c` goes
  // first; then `a` and `b` tie and the name breaks it.
  assert.deepEqual(curve.map(step => [step.add, step.exportsCleared, step.of]), [
    ["c", 1, 3],
    ["a", 1, 3],
    ["b", 3, 3]
  ]);
});

// ADR 0140: each host is its own measurement; the summary sets them side by
// side and names the host-free run "none".
test("the per-host summary lists every host measurement with its dialect-silent class", () => {
  const measured = (host, clean, silent) => ({
    host,
    headline: { perPackage: 0.1, byDownloads: 0.2, pooled: clean / 10, cleanExports: clean, exports: 10 },
    totals: { clean, partial: 10 - clean, degenerate: 0, uncertified: 0, misuseCapable: 1 },
    classes: [{ class: "dialect-silent", exports: silent }],
    walls: [{ class: "dialect-silent", key: "solid-js:createSignal", exports: silent, packages: ["a", "b"] }]
  });
  const markdown = renderHostSummary([measured(null, 1, 4), measured("browser", 3, 1)]);
  assert.match(markdown, /\| none \| 10\.0% \| 20\.0% \| 1 \(10\.0%\) \| 10 \| 1 \/ 9 \/ 0 \/ 0 \| 4 \| 1 \|/);
  assert.match(markdown, /\| browser \| 10\.0% \| 20\.0% \| 3 \(30\.0%\) \| 10 \| 3 \/ 7 \/ 0 \/ 0 \| 1 \| 1 \|/);
  assert.match(markdown, /\| browser \| solid-js:createSignal \| 1 \| 2 \|/);
});
