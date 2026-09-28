// Two certifications of one published artifact routinely differ, and the
// difference decides whether the artifact ships at all. Measured over a full
// census run: 89 artifact/entrypoint pairs, 87 agreeing exactly, 2 differing,
// 0 contradicting. Both differences were a root row closing a claim domain that
// the same artifact left open when it was reached as another package's
// dependency node. Dropping those cost the largest primitives contract.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "vitest";

import {
  bundleKey,
  claimsOf,
  collectBundles,
  indexDocument,
  parseArguments,
  relate,
  retainedOutputDirectories,
  withdrawUncarriedCitations
} from "./bundle-accepted-contracts.mjs";

const document = (summary) =>
  JSON.stringify({
    entrypoints: { ".": { cases: [{ exports: { access: "s1" } }] } },
    summaries: { s1: summary }
  });

const invoke = {
  callbacks: [{ from: { arg: 0, path: [] }, operation: "callback-0" }],
  operations: [{ id: "callback-0", kind: "invoke", tracking: "untracked" }]
};

const open = document({ shape: "callable", call: { ...invoke, closed: ["creates"], creates: [] } });
const closing = document({
  shape: "callable",
  call: { ...invoke, closed: ["reads", "creates"], creates: [], reads: [] }
});

describe("resolving two certifications of one artifact", () => {
  test("a document that closes a domain the other left open refines it", () => {
    assert.equal(relate(claimsOf(closing), claimsOf(open)), "refines");
    assert.equal(relate(claimsOf(open), claimsOf(closing)), "coarsens");
  });

  test("identical claims are the same however the bytes were ordered", () => {
    assert.equal(relate(claimsOf(open), claimsOf(open)), "same");
  });

  // The trap this guards: `reads: []` with `reads` closed and no `reads` key at
  // all are the same bytes once the empty array is dropped, and only `closed`
  // tells "proved there is nothing here" from "states nothing here". If the
  // body comparison swallowed that, an open document would read as `same` and
  // could win the deterministic digest tie-break over a closing one.
  test("an empty closed domain is not the same claim as an absent one", () => {
    assert.notEqual(relate(claimsOf(open), claimsOf(closing)), "same");
  });

  test("closing different domains is a conflict, not a merge", () => {
    const closesReads = document({
      shape: "callable",
      call: { ...invoke, closed: ["reads"], reads: [] }
    });
    const closesCreates = document({
      shape: "callable",
      call: { ...invoke, closed: ["creates"], creates: [] }
    });
    assert.equal(relate(claimsOf(closesReads), claimsOf(closesCreates)), "conflict");
  });

  test("a different operation is a conflict however the domains close", () => {
    const tracked = document({
      shape: "callable",
      call: {
        callbacks: invoke.callbacks,
        operations: [{ id: "callback-0", kind: "invoke", tracking: "tracked" }],
        closed: ["reads", "creates"],
        creates: [],
        reads: []
      }
    });
    assert.equal(relate(claimsOf(tracked), claimsOf(open)), "conflict");
  });

  test("an export one document does not carry is a conflict", () => {
    const extra = JSON.stringify({
      entrypoints: { ".": { cases: [{ exports: { access: "s1", chain: "s1" } }] } },
      summaries: { s1: { shape: "callable", call: { ...invoke, closed: ["creates"], creates: [] } } }
    });
    assert.equal(relate(claimsOf(extra), claimsOf(open)), "conflict");
  });
});

// The defect this key exists for, measured on the ecosystem corpus: the same
// `@solid-primitives/utils@7.0.0-next.4` bytes certified on a floor row
// (`@solidjs/signals@2.0.0-rc.0`, no audited rows) and a head row (rc.6,
// audited), and only the head rows close `createMicrotask` `creates`. Keyed by
// artifact alone, "keep the closing certification" shipped the head closure to
// rc.0 projects.
describe("certifications of one artifact in different environments", () => {
  const signals = version => [
    { name: "@solidjs/signals", version, integrity: `sha512-signals-${version}` }
  ];
  const entry = (environment, document, digest) => ({
    packageName: "@solid-primitives/utils",
    packageVersion: "7.0.0-next.4",
    packageIntegrity: "sha512-utils",
    specifier: "@solid-primitives/utils",
    requestedEntrypoint: ".",
    exportConditions: ["import"],
    document: `objects/${digest}.main.json`,
    documentDigest: `sha256:${digest}`,
    receipt: `objects/${digest}.receipt.json`,
    receiptDigest: `sha256:${digest}r`,
    dependencyEnvironment: environment,
    _document: document
  });
  const result = (...entries) => ({
    bundles: entries.map(({ _document, ...published }) => published),
    objects: Object.fromEntries(
      entries.flatMap(({ _document, document, receipt }) => [
        [document, _document],
        [receipt, "{}"]
      ])
    )
  });

  test("the environment is part of the key", () => {
    const floor = entry(signals("2.0.0-rc.0"), open, "a");
    const head = entry(signals("2.0.0-rc.6"), closing, "b");
    assert.notEqual(bundleKey(floor), bundleKey(head));
    assert.equal(bundleKey(floor), bundleKey(entry(signals("2.0.0-rc.0"), closing, "c")));
  });

  // An environment that records who resolved what is keyed by those edges
  // too: they are part of what its root signs and of what admission replays,
  // and it is read by a different rule than the same packages stated without
  // edges, so neither may stand in for the other.
  test("resolution edges are part of the key", () => {
    const edged = (importer, specifier = "@solidjs/signals") => [
      {
        ...signals("2.0.0-rc.6")[0],
        resolvedFrom: { importer, specifier }
      }
    ];
    const fromRoot = entry(edged("certified"), open, "a");
    const strict = entry(signals("2.0.0-rc.6"), open, "b");
    const fromOther = entry(
      edged({ package: { name: "other", version: "1.0.0", integrity: "sha512-other" } }),
      open,
      "c"
    );
    const bySpecifier = entry(edged("certified", "signals-alias"), open, "d");
    const keys = [fromRoot, strict, fromOther, bySpecifier].map(bundleKey);
    assert.equal(new Set(keys).size, 4, keys.join("\n"));
    assert.equal(bundleKey(fromRoot), bundleKey(entry(edged("certified"), closing, "e")));
  });

  test("a floor and a head certification are two bundles, and neither refines the other", () => {
    const floor = entry(signals("2.0.0-rc.0"), open, "a");
    const head = entry(signals("2.0.0-rc.6"), closing, "b");
    for (const order of [[floor, head], [head, floor]]) {
      const { ordered, conflicted, refinements } = collectBundles([result(...order)]);
      assert.equal(ordered.length, 2, "one bundle per environment");
      assert.equal(conflicted.size, 0);
      assert.equal(refinements.size, 0, "a closure proven in one environment refines nothing in another");
      const byEnvironment = new Map(
        ordered.map(bundle => [bundle.dependencyEnvironment[0].version, bundle.documentDigest])
      );
      assert.equal(byEnvironment.get("2.0.0-rc.0"), "sha256:a", "the floor keeps its own, open, claim");
      assert.equal(byEnvironment.get("2.0.0-rc.6"), "sha256:b");
    }
  });

  test("within one environment the closing certification still refines", () => {
    const narrow = entry(signals("2.0.0-rc.6"), open, "a");
    const full = entry(signals("2.0.0-rc.6"), closing, "b");
    const { ordered, refinements } = collectBundles([result(narrow), result(full)]);
    assert.equal(ordered.length, 1);
    assert.equal(ordered[0].documentDigest, "sha256:b");
    assert.equal(refinements.size, 1);
  });

  test("an entry that states no environment is refused rather than keyed", () => {
    const unstated = entry(undefined, open, "a");
    assert.throws(() => collectBundles([result(unstated)]), /states no dependency environment/);
  });
});

// The census run and every delivery run for a reviewed consumer environment
// feed one tier (Makefile `accepted-bundles`), so `--run` is repeatable. Their
// bundles cannot collide: an environment run proves each artifact in another
// dependency environment, and the environment is part of `bundleKey`.
describe("bundling from several runs", () => {
  const withRuns = (reports, body) => {
    const directory = mkdtempSync(join(tmpdir(), "bundle-runs-"));
    try {
      const paths = reports.map((report, index) => {
        const path = join(directory, `run-${index}.json`);
        writeFileSync(path, JSON.stringify(report));
        return path;
      });
      return body(paths);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  };
  const run = (scope, ...outputDirs) => ({
    scope,
    results: outputDirs.map(outputDir => ({ retainedArtifacts: { outputDir } }))
  });

  test("--run is repeatable, a repeated run is refused, and one input is still required", () => {
    assert.deepEqual(parseArguments(["--run", "a.json", "--run", "b.json", "--dry-run"]).runs, ["a.json", "b.json"]);
    assert.throws(() => parseArguments(["--run", "a.json", "--run", "./a.json"]), /given twice/);
    assert.throws(() => parseArguments(["--run"]), /--run needs a path/);
    assert.throws(() => parseArguments(["--dry-run"]), /one of --run/);
    assert.equal(parseArguments(["--catalogs", "dir"]).catalogs, "dir");
  });

  test("every run's retained directories are bundled, in the order given", () => {
    withRuns(
      [
        run({ kind: "filtered", solidTargets: ["2"] }, "/census/a", "/census/b"),
        run({ kind: "filtered", solidTargets: ["2"], consumerEnvironment: "kobalte-solid2-e9d426d4" }, "/delivery/a")
      ],
      paths => {
        assert.deepEqual(
          retainedOutputDirectories(paths, { reviewedEnvironments: ["kobalte-solid2-e9d426d4"] }),
          ["/census/a", "/census/b", "/delivery/a"]
        );
      }
    );
  });

  test("a run that kept nothing, or a delivery run for an unreviewed environment, is refused", () => {
    withRuns([run({ kind: "filtered" }, "/census/a"), run({ kind: "filtered" })], paths => {
      assert.throws(() => retainedOutputDirectories(paths), /run-1\.json retained no output directories/);
    });
    withRuns([run({ kind: "filtered", consumerEnvironment: "someone-else" }, "/delivery/a")], paths => {
      assert.throws(
        () => retainedOutputDirectories(paths, { reviewedEnvironments: ["kobalte-solid2-e9d426d4"] }),
        /consumer environment someone-else, which .* does not list/
      );
    });
    // The committed reviewed list is what the Makefile relies on.
    withRuns([run({ kind: "filtered", consumerEnvironment: "kobalte-solid2-e9d426d4" }, "/delivery/a")], paths => {
      assert.deepEqual(retainedOutputDirectories(paths), ["/delivery/a"]);
    });
  });

  test("a delivery certification of a tier artifact is its own bundle beside the census's", () => {
    const entry = (signals, digest) => ({
      packageName: "@solid-primitives/keyed",
      packageVersion: "3.0.0-next.2",
      packageIntegrity: "sha512-keyed",
      specifier: "@solid-primitives/keyed",
      requestedEntrypoint: ".",
      exportConditions: ["import"],
      document: `objects/${digest}.main.json`,
      documentDigest: `sha256:${digest}`,
      receipt: `objects/${digest}.receipt.json`,
      receiptDigest: `sha256:${digest}r`,
      dependencyEnvironment: [{ name: "@solidjs/signals", version: signals, integrity: `sha512-${signals}` }]
    });
    const result = published => ({
      bundles: [published],
      objects: { [published.document]: open, [published.receipt]: "{}" }
    });
    const { ordered } = collectBundles([
      result(entry("2.0.0-rc.0", "floor")),
      result(entry("2.0.0-rc.6", "head")),
      result(entry("2.0.0-rc.3", "kobalte"))
    ]);
    assert.deepEqual(
      ordered.map(bundle => bundle.dependencyEnvironment[0].version).sort(),
      ["2.0.0-rc.0", "2.0.0-rc.3", "2.0.0-rc.6"]
    );
  });
});

// A per-host tier proves one artifact in one environment for every host, so
// the index stores each environment once, keyed by the root its receipts sign,
// and never restates a receipt's bindings (index version 3). Measured on the
// 729-bundle per-host tier: 120 distinct environments, 7.7 MB inline.
describe("writing the index", () => {
  const environment = version => [{ name: "@solidjs/signals", version, integrity: `sha512-${version}` }];
  const root = version => `sha256:${version.replace(/\D/g, "").padStart(64, "0")}`;
  const bundle = (name, version, digest) => ({
    packageName: name,
    packageVersion: "1.0.0",
    receipt: `objects/${digest}.receipt.json`,
    bindings: { dependencyEnvironmentRoot: root(version), specifier: name },
    dependencyEnvironment: environment(version)
  });
  const objectsOf = (...bundles) =>
    new Map(
      bundles.map(entry => [
        entry.receipt,
        JSON.stringify({ payload: { dependencyEnvironmentRoot: entry.bindings.dependencyEnvironmentRoot } })
      ])
    );

  test("each environment is stored once and every bundle names it by root", () => {
    const bundles = [
      bundle("a", "2.0.0-rc.6", "1"),
      bundle("b", "2.0.0-rc.6", "2"),
      bundle("a", "2.0.0-rc.0", "3")
    ];
    const index = indexDocument(bundles, objectsOf(...bundles));
    assert.equal(index.bundleIndexVersion, 3);
    assert.deepEqual(Object.keys(index.environments), [root("2.0.0-rc.0"), root("2.0.0-rc.6")]);
    assert.deepEqual(index.environments[root("2.0.0-rc.6")], environment("2.0.0-rc.6"));
    assert.deepEqual(
      index.bundles.map(entry => entry.dependencyEnvironmentRoot),
      [root("2.0.0-rc.6"), root("2.0.0-rc.6"), root("2.0.0-rc.0")]
    );
    for (const entry of index.bundles) {
      assert.equal(entry.bindings, undefined, "the receipt states the bindings");
      assert.equal(entry.dependencyEnvironment, undefined);
    }
  });

  test("two environments under one root, or a root the receipt does not sign, are refused", () => {
    const one = bundle("a", "2.0.0-rc.6", "1");
    const other = { ...bundle("b", "2.0.0-rc.6", "2"), dependencyEnvironment: environment("2.0.0-rc.3") };
    assert.throws(() => indexDocument([one, other], objectsOf(one, other)), /different environments under the root/);
    const unsigned = bundle("a", "2.0.0-rc.6", "1");
    const objects = objectsOf(unsigned);
    objects.set(unsigned.receipt, JSON.stringify({ payload: { dependencyEnvironmentRoot: root("2.0.0-rc.0") } }));
    assert.throws(() => indexDocument([unsigned], objects), /does not sign the environment root/);
    const unbound = { ...bundle("a", "2.0.0-rc.6", "1"), bindings: {} };
    assert.throws(() => indexDocument([unbound], objectsOf(one)), /binds no dependency environment/);
  });
});

describe("ADR 0151: a tier carries what its bundles cite", () => {
  // A claim is named by content: acceptance root, environment root and
  // semantic digest. `contract` stands for all three here.
  const bundle = (name, receiptDigest, citedAcceptances = [], contract = `${name}-contract`) => ({
    entry: {
      packageName: name,
      packageVersion: "1.0.0",
      requestedEntrypoint: ".",
      receipt: `objects/${name}-${receiptDigest}.receipt.json`,
      receiptDigest
    },
    receipt: JSON.stringify({
      payload: {
        artifactAcceptanceRoot: `${name}-artifact`,
        dependencyEnvironmentRoot: `${name}-environment`,
        semanticDigest: contract,
        ...(citedAcceptances.length ? { citedAcceptances } : {})
      }
    })
  });
  const cite = (name, receiptDigest, contract = `${name}-contract`) => ({
    packageName: name,
    packageVersion: "1.0.0",
    artifactAcceptanceRoot: `${name}-artifact`,
    dependencyEnvironmentRoot: `${name}-environment`,
    semanticDigest: contract,
    receiptDigest
  });

  test("a bundle whose citation the tier carries is kept, and one whose citation it dropped is withdrawn, transitively", () => {
    const utils = bundle("utils", "sha256:u");
    const media = bundle("media", "sha256:m", [cite("utils", "sha256:u")]);
    const orphan = bundle("orphan", "sha256:o", [cite("gone", "sha256:g")]);
    // Cites the orphan, so it goes when the orphan goes.
    const chained = bundle("chained", "sha256:c", [cite("orphan", "sha256:o")]);
    const all = [utils, media, orphan, chained];
    const objects = new Map(all.map(({ entry, receipt }) => [entry.receipt, receipt]));
    const { kept, withdrawn } = withdrawUncarriedCitations(all.map(({ entry }) => entry), objects);
    assert.deepEqual(kept.map(entry => entry.packageName), ["utils", "media"]);
    assert.deepEqual(
      withdrawn.map(({ entry, citation }) => [entry.packageName, citation.packageName]),
      [["orphan", "gone"], ["chained", "orphan"]]
    );
  });

  test("a citation names the claim: a re-issued receipt of the same claim carries it, another contract does not", () => {
    const reissued = bundle("utils", "sha256:new");
    const media = bundle("media", "sha256:m", [cite("utils", "sha256:old")]);
    const objects = new Map([reissued, media].map(({ entry, receipt }) => [entry.receipt, receipt]));
    assert.deepEqual(
      withdrawUncarriedCitations([reissued.entry, media.entry], objects).kept.map(entry => entry.packageName),
      ["utils", "media"]
    );
    const weaker = bundle("utils", "sha256:weaker", [], "utils-weaker-contract");
    const others = new Map([weaker, media].map(({ entry, receipt }) => [entry.receipt, receipt]));
    assert.deepEqual(
      withdrawUncarriedCitations([weaker.entry, media.entry], others).kept.map(entry => entry.packageName),
      ["utils"]
    );
  });
});
