import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "vitest";

import { flattenRanges, solidRanges } from "./ecosystem-benchmark/lib/registry.mjs";
import { satisfies } from "./ecosystem-benchmark/lib/semver.mjs";
import {
  CORPUS_PATH,
  HOSTS,
  MISUSE_LEDGER_PATH,
  checkpoint,
  checkpointCorpusProblems,
  combineWalls,
  contractMisuse,
  contractMisusePaths,
  graphRefusalOf,
  hostOfConditions,
  ledgerProblems,
  misusePathsOf,
  misuseVerdict,
  publishedDefectOf,
  refusalOf,
  solid2Releases,
  tierEntrypoints,
  typeMisuse
} from "./primitives-checkpoint.mjs";

const root = resolve(import.meta.dirname, "..");

test("a Solid 2 release is one whose solid-js range admits a published 2.x, or that depends on @solidjs/signals", () => {
  const packument = {
    versions: {
      "1.0.0": { peerDependencies: { "solid-js": "^1.6.0" }, dist: { integrity: "sha512-a" } },
      "2.0.0-next.0": { peerDependencies: { "solid-js": "^2.0.0-beta.15" }, dist: { integrity: "sha512-b" } },
      "2.0.0-next.1": { dependencies: { "@solidjs/signals": "^0.4.0" }, dist: { integrity: "sha512-c" } },
      // A runtime dependency is the stronger statement: a 1.x runtime range
      // wins over a 2.x peer range.
      "2.0.0-next.2": { dependencies: { "solid-js": "^1.9.0" }, peerDependencies: { "solid-js": "^2.0.0-rc.0" } }
    }
  };
  const releases = solid2Releases(packument, ["2.0.0-beta.15", "2.0.0-rc.9"], { satisfies, flattenRanges, solidRanges });
  assert.deepEqual(
    releases.map(release => release.version),
    ["2.0.0-next.0", "2.0.0-next.1"]
  );
  assert.equal(releases[0].integrity, "sha512-b");
});

test("the checked-in corpus agrees with the manifest, and pins every solid2 row of the scope by its head probe", () => {
  const corpus = JSON.parse(readFileSync(CORPUS_PATH, "utf8"));
  const manifest = JSON.parse(readFileSync(join(root, "scripts/ecosystem-benchmark/manifest.json"), "utf8"));
  assert.deepEqual(checkpointCorpusProblems(corpus, manifest), []);
  assert.ok(corpus.packages.length > 0);
  for (const entry of corpus.packages) {
    assert.ok(entry.package.startsWith("@solid-primitives/"), entry.package);
    assert.equal(entry.probeKind, "head", entry.probe);
  }
  const names = corpus.packages.map(entry => entry.package);
  assert.deepEqual(names, [...names].sort());
  for (const entry of corpus.withoutSolid2) assert.ok(!names.includes(entry.package), entry.package);
});

test("a manifest row of the scope the corpus leaves out is refused", () => {
  const corpus = { packages: [{ package: "@solid-primitives/a", version: "1.0.0", integrity: "i", probe: "a|head" }] };
  const manifest = {
    rows: [
      { package: "@solid-primitives/a", solidTarget: "solid2", version: "1.0.0", integrity: "i", probes: [{ id: "a|head" }] },
      { package: "@solid-primitives/b", solidTarget: "solid2", version: "1.0.0", integrity: "j", probes: [{ id: "b|head" }] },
      { package: "@solid-primitives/c", solidTarget: "solid1", version: "1.0.0", integrity: "k", probes: [] }
    ]
  };
  assert.deepEqual(checkpointCorpusProblems(corpus, manifest), ["@solid-primitives/b: a solid2 manifest row the corpus does not pin"]);
});

const operation = (fields) => ({ at: { event: "call", schedule: "same-stack" }, tracking: "untracked", ...fields });

test("contract misuse classes come from the operations a summary states", () => {
  const summary = {
    call: {
      operations: [
        operation({ kind: "cleanup", owner: { requires: "required", requiresCleanup: "required" } }),
        operation({ kind: "return", output: { kind: "described-callable", reads: ["owned-signal"], returns: ["plain"] } }),
        operation({ kind: "invoke", tracking: "tracked" }),
        operation({ kind: "read", inputs: [{ kind: "parameter" }] })
      ]
    }
  };
  assert.deepEqual([...contractMisuse(summary)].sort(), ["argumentRead", "owner", "returnedAccessor", "trackedCallback"]);
  // A deferred callback, a tracked read, and a callable that reads nothing
  // are not misuse paths.
  const quiet = {
    call: {
      operations: [
        operation({ kind: "invoke", at: { event: "call", schedule: "later" } }),
        operation({ kind: "read", tracking: "tracked", inputs: [{ kind: "parameter" }] }),
        operation({ kind: "return", output: { kind: "described-callable", reads: [], returns: ["plain"] } })
      ]
    }
  };
  assert.equal(contractMisuse(quiet).size, 0);
  const nested = { call: { operations: [operation({ kind: "return", output: { kind: "object", properties: [{ value: { kind: "reactive", role: "accessor" } }] } })] } };
  assert.deepEqual([...contractMisuse(nested)], ["returnedAccessor"]);
});

test("an accepted claim outranks a proposed one, and types stand in only where no summary states a path", () => {
  const document = claims => ({
    entrypoints: { ".": { cases: [{ exports: { useIt: "s" } }] } },
    summaries: { s: { call: { operations: claims } } }
  });
  const proposed = document([operation({ kind: "cleanup", owner: { requires: "required" } }), operation({ kind: "invoke", tracking: "tracked" })]);
  const acceptedDoc = document([operation({ kind: "cleanup", owner: { requires: "required" } })]);
  const paths = contractMisusePaths([
    { document: proposed, source: "proposed" },
    { document: acceptedDoc, source: "accepted" }
  ]);
  const key = ".\u0000useIt";
  assert.deepEqual(Object.fromEntries(paths.get(key)), { owner: "accepted", trackedCallback: "proposed" });
  const types = new Map([[key, new Set(["typeCallback"])], [".\u0000other", new Set(["typeAccessorReturn"])]]);
  assert.deepEqual(misusePathsOf(key, paths, types).map(path => [path.class, path.source, path.code]), [
    ["owner", "accepted", "SC4001"],
    ["trackedCallback", "proposed", "SC2001"]
  ]);
  assert.deepEqual(misusePathsOf(".\u0000other", paths, types).map(path => [path.class, path.source]), [["typeAccessorReturn", "types"]]);
  assert.deepEqual(misusePathsOf(".\u0000none", paths, types), []);
});

test("types-only paths read the published declaration, and never look inside library types", () => {
  const ts = createRequire(join(root, "packages/cli/package.json"))("typescript");
  const dir = mkdtempSync(join(tmpdir(), "primitives-checkpoint-types-"));
  try {
    const file = join(dir, "index.d.ts");
    writeFileSync(
      file,
      [
        "export type Accessor<T> = () => T;",
        "export declare function createCount(): Accessor<number>;",
        "export declare function createPair(): [Accessor<number>, (value: number) => void];",
        "export declare function onTick(handler: (tick: number) => void): void;",
        "export declare function watch(source: Accessor<number>): void;",
        "export declare function asArray<T>(value: T): T[];",
        "export declare function clear(): void;",
        "export declare const version: string;"
      ].join("\n")
    );
    const program = ts.createProgram([file], { strict: true, noEmit: true, types: [], lib: ["lib.es2022.d.ts"] });
    const checker = program.getTypeChecker();
    const moduleSymbol = checker.getSymbolAtLocation(program.getSourceFile(file));
    const classes = {};
    for (const symbol of checker.getExportsOfModule(moduleSymbol)) {
      if (!(symbol.flags & ts.SymbolFlags.Value)) continue;
      const found = typeMisuse(ts, checker, checker.getTypeOfSymbolAtLocation(symbol, symbol.valueDeclaration), node =>
        program.isSourceFileDefaultLibrary(node.getSourceFile())
      );
      classes[symbol.name] = [...found].sort();
    }
    assert.deepEqual(classes, {
      createCount: ["typeAccessorReturn"],
      createPair: ["typeAccessorReturn"],
      onTick: ["typeCallback"],
      watch: ["typeAccessorArgument"],
      asArray: [],
      clear: [],
      version: []
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a tier bundle's host is the host condition it carries", () => {
  assert.equal(hostOfConditions(["import"]), "none");
  assert.equal(hostOfConditions(["import", "solid"]), "none");
  assert.equal(hostOfConditions(["browser", "import"]), "browser");
  assert.equal(hostOfConditions(["import", "node"]), "node");
  const tier = tierEntrypoints({
    bundles: [
      { packageName: "@solid-primitives/a", packageVersion: "1.0.0", requestedEntrypoint: ".", exportConditions: ["import"] },
      { packageName: "@solid-primitives/a", packageVersion: "1.0.0", requestedEntrypoint: "./x", exportConditions: ["browser", "import"] }
    ]
  });
  const hosts = tier.get("@solid-primitives/a@1.0.0");
  assert.deepEqual([...hosts.get("none")], ["."]);
  assert.deepEqual([...hosts.get("browser")], ["./x"]);
  assert.deepEqual([...hosts.get("node")], []);
});

test("refusals, published-package defects and graph-lane refusals are named without temporary paths", () => {
  assert.equal(refusalOf({ certificationAttempt: { status: "certified", reason: "x" } }), null);
  assert.equal(
    refusalOf({ certificationAttempt: { status: "refused", reason: "at /private/tmp/claude-1/x/node_modules/y failed" } }),
    "at <tmp> failed"
  );
  assert.equal(refusalOf({ certificationAttempt: null, signature: "no certifiable artifact case", class: "c" }), "no certifiable artifact case");
  assert.deepEqual(
    publishedDefectOf({
      class: "unavailable-published-target",
      artifactCaseRefusals: [{ reason: "resolved target <package-root>/dist/index.js is not a file" }]
    }),
    { kind: "runtime target not published", detail: "dist/index.js is not in the tarball" }
  );
  assert.deepEqual(
    publishedDefectOf({
      certificationAttempt: {
        status: "refused",
        reason: "artifact case . [import] imports dependency-target-not-exported: solid-js/web is not exported by solid-js@2.0.0-rc.9 under conditions [import]"
      }
    }),
    { kind: "imports a specifier the runtime does not export", detail: "solid-js/web (solid-js@2.0.0-rc.9)" }
  );
  assert.equal(publishedDefectOf({ certificationAttempt: { status: "refused", reason: "census refused" } }), null);
  const graph = text => graphRefusalOf({ certificationAttempt: { graphPreparation: { preparationRefusal: text } } });
  assert.equal(
    graph(
      'published dependency graph prepared no artifact case: graph node solid-js@2.0.0-rc.9 . [import,node] refused: published dependency graph node solid-js@2.0.0-rc.9 . [import,node] refused: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: contract identity does not match the resolved import: Declaration target for export "action" is re-exported from dependency "@solidjs/signals" (module "dist/types/core/action.d.ts"), which no planned dependency binds because the resolved package has none'
    ).key,
    'graph node solid-js@2.0.0-rc.9 [import,node]: export "action" re-exported from @solidjs/signals, which no planned dependency binds'
  );
  assert.equal(
    graph("published dependency graph prepared no artifact case: @tauri-apps/api is not installed above /private/tmp/x/node_modules/@solid-primitives/filesystem/dist/a.d.ts").key,
    "@tauri-apps/api is not installed (imported by @solid-primitives/filesystem)"
  );
  assert.equal(graphRefusalOf({ certificationAttempt: { graphPreparation: {} } }), null);
});

test("the misuse ledger is well formed and pins the corpus versions", () => {
  const corpus = JSON.parse(readFileSync(CORPUS_PATH, "utf8"));
  const ledger = JSON.parse(readFileSync(MISUSE_LEDGER_PATH, "utf8"));
  assert.deepEqual(ledgerProblems(ledger, corpus), []);
  const broken = {
    cases: [
      { id: "x", package: "@solid-primitives/timer", version: "0.0.0", class: "owner", rule: "missing-owner", misuse: "a", correct: "b", hosts: ["none"] },
      { id: "x", package: "@solid-primitives/none", version: "1", class: "made-up", misuse: "", correct: "b", hosts: ["deno"] }
    ]
  };
  const problems = ledgerProblems(broken, corpus);
  assert.ok(problems.some(problem => /pins 0\.0\.0/.test(problem)));
  assert.ok(problems.some(problem => /unique id/.test(problem)));
  assert.ok(problems.some(problem => /not in the checkpoint corpus/.test(problem)));
  assert.ok(problems.some(problem => /unknown misuse class/.test(problem)));
  assert.ok(problems.some(problem => /unknown host deno/.test(problem)));
  assert.ok(problems.some(problem => /misuse code is required/.test(problem)));
});

test("a misuse case reports correctly only with tsc silent, the rule's violation on misuse, and nothing on correct use", () => {
  const silent = { misuse: [], correct: [] };
  const owner = { rule: "missing-owner", kind: "violation" };
  const incomplete = { rule: "package-contract-incomplete", kind: "uncertifiable" };
  assert.equal(misuseVerdict({ rule: "missing-owner", tsc: silent, misuseFindings: [owner], correctFindings: [] }).status, "reports correctly");
  assert.equal(misuseVerdict({ rule: "missing-owner", tsc: { misuse: [2345], correct: [] }, misuseFindings: [owner], correctFindings: [] }).status, "tsc reports");
  assert.equal(misuseVerdict({ rule: "missing-owner", tsc: silent, misuseFindings: [], correctFindings: [] }).status, "misuse silent");
  assert.deepEqual(misuseVerdict({ rule: "missing-owner", tsc: silent, misuseFindings: [incomplete], correctFindings: [] }), {
    status: "wrong finding",
    detail: "package-contract-incomplete"
  });
  assert.equal(
    misuseVerdict({ rule: "missing-owner", tsc: silent, misuseFindings: [{ rule: "missing-owner", kind: "uncertifiable" }], correctFindings: [] }).status,
    "misuse only uncertifiable"
  );
  assert.deepEqual(misuseVerdict({ rule: "missing-owner", tsc: silent, misuseFindings: [incomplete, owner], correctFindings: [incomplete] }), {
    status: "correct use not clean",
    detail: "package-contract-incomplete"
  });
});

const hostMeasurement = (host, exports, fields = {}) => ({
  host,
  run: { durationMs: 1000 },
  packages: [
    {
      package: "@solid-primitives/a",
      certification: "certified",
      certifiable: true,
      refusedEntrypoints: 0,
      lane: "reused-proposal",
      exports,
      ...fields
    }
  ]
});

test("the checkpoint needs every host certified and in the tier, every export clean, and every misuse path reporting", () => {
  const corpus = { measuredOn: "2026-09-28", packages: [{ package: "@solid-primitives/a", version: "1.0.0" }], withoutSolid2: [] };
  const value = { entrypoint: ".", export: "isServer", bucket: "clean", causes: [], misuse: [] };
  const owner = { class: "owner", source: "accepted", rule: "missing-owner", code: "SC4001", misuse: "m" };
  const create = { entrypoint: ".", export: "createA", bucket: "clean", causes: [], misuse: [owner] };
  const tier = tierEntrypoints({
    bundles: HOSTS.map(host => ({
      packageName: "@solid-primitives/a",
      packageVersion: "1.0.0",
      requestedEntrypoint: ".",
      exportConditions: host === "none" ? ["import"] : [host, "import"]
    }))
  });
  const hosts = HOSTS.map(host => hostMeasurement(host, [value, create]));
  const ledger = { cases: [{ id: "a-owner", package: "@solid-primitives/a", entrypoint: ".", export: "createA" }] };
  const reporting = { results: [{ id: "a-owner", hosts: Object.fromEntries(HOSTS.map(host => [host, { status: "reports correctly" }])) }] };

  const done = checkpoint({ hosts, tier, misuseResults: reporting, ledger, corpus });
  assert.equal(done.packages[0].atCheckpoint, true);
  assert.deepEqual(done.progress.misuseFixtures, { absent: 0, present: 0, reporting: 1 });
  assert.equal(done.progress.exportsAccounted, 2);

  // No case: criterion 3 fails and the fixture is absent.
  const absent = checkpoint({ hosts, tier, ledger: { cases: [] }, corpus });
  assert.equal(absent.packages[0].criteria.misuse, false);
  assert.equal(absent.packages[0].exports.find(item => item.export === "createA").fixture, "absent");

  // A case that has not run is present, not reporting.
  assert.equal(checkpoint({ hosts, tier, ledger, corpus }).packages[0].exports.find(item => item.export === "createA").fixture, "present, not run");

  // One host missing from the tier fails criterion 1 for that host only.
  const partialTier = tierEntrypoints({ bundles: [{ packageName: "@solid-primitives/a", packageVersion: "1.0.0", requestedEntrypoint: ".", exportConditions: ["import"] }] });
  const untiered = checkpoint({ hosts, tier: partialTier, misuseResults: reporting, ledger, corpus }).packages[0];
  assert.equal(untiered.criteria.certified, false);
  assert.equal(untiered.certification.none.met, true);
  assert.deepEqual(untiered.certification.node.missingTier, ["."]);

  // An export open in one host is not accounted for.
  const open = { ...create, bucket: "partial", causes: [{ domain: "reads", class: "recipe", key: "no probe recipe (reads)" }] };
  const nodeOpen = [hostMeasurement("none", [value, create]), hostMeasurement("browser", [value, create]), hostMeasurement("node", [value, open])];
  const opened = checkpoint({ hosts: nodeOpen, tier, misuseResults: reporting, ledger, corpus });
  assert.equal(opened.packages[0].criteria.accounted, false);
  assert.deepEqual(opened.packages[0].exports.find(item => item.export === "createA").status.node.open, ["reads"]);
  assert.equal(opened.progress.exportsAccounted, 1);

  // A host not measured leaves the package off the checkpoint, and is named.
  const missing = checkpoint({ hosts: hosts.slice(0, 2), tier, misuseResults: reporting, ledger, corpus });
  assert.deepEqual(missing.missingHosts, ["node"]);
  assert.equal(missing.packages[0].atCheckpoint, false);
});

test("walls join hosts by cause, and an unaccepted dependency behind a refused graph lane is attributed to that refusal", () => {
  const behind = 'graph node solid-js@2.0.0-rc.9 [import,node]: export "action" re-exported';
  const item = (name, causes) => ({ entrypoint: ".", export: name, bucket: "partial", causes, misuse: [] });
  const recipe = { domain: "reads", class: "recipe", key: "no probe recipe (reads)" };
  const dependency = { domain: "creates", class: "unaccepted dependency", key: "@solid-primitives/utils", behind };
  const walls = combineWalls([
    hostMeasurement("none", [item("a", [recipe]), item("b", [recipe])]),
    hostMeasurement("node", [item("a", [dependency]), item("b", [recipe])])
  ]);
  const byKey = Object.fromEntries(walls.map(wall => [`${wall.class}|${wall.key}`, wall]));
  assert.deepEqual(byKey["recipe|no probe recipe (reads)"].hosts, { none: 2, browser: 0, node: 1 });
  assert.equal(byKey["recipe|no probe recipe (reads)"].exports, 2);
  assert.equal(byKey[`graph lane refused|${behind}`].onlyIn, "node");
  assert.equal(byKey["unaccepted dependency|@solid-primitives/utils"].exports, 1);
  assert.equal(walls[0].key, "no probe recipe (reads)");
});
