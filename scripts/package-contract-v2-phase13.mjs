import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPORT_PATH = join(
  ROOT,
  "benchmarks/package-contract-v2/phase13/conformance.json"
);
const AUDIT_PATH = join(ROOT, "benchmarks/package-contract-v2/phase0/rc3/audit.json");

const EXPECTED_ROWS = [
  "split-create-effect",
  "tracked-effect-on-settled",
  "batching-flush",
  "control-flow-callbacks",
  "async-computations",
  "loading-refresh",
  "actions-optimism",
  "stores-projections",
  "refs-directives",
  "root-event-delegation",
  "browser-server-rendering",
  "request-response-mutation",
  "server-functions",
  "experimental-server-components",
  "conditional-adapters",
  "mixed-framework-selection"
];

const EXPECTED_PROOF_FAMILIES = [
  "package-identity",
  "manifest-entrypoint",
  "export-resolution",
  "artifact-declarations",
  "export-identity",
  "module-closure",
  "selected-signature",
  "argument-binding",
  "rest-spread-coverage",
  "callable-path",
  "operation-reachability",
  "operation-cardinality",
  "recursive-value-shape",
  "guard-partition",
  "compiler-reconciliation",
  "accepted-dependency-composition",
  "domain-exhaustiveness",
  "probe-consistency"
];

const SHA256 = /^[0-9a-f]{64}$/;

export function readPhase13Conformance() {
  return JSON.parse(readFileSync(REPORT_PATH, "utf8"));
}

export function readPublishedAudit() {
  return JSON.parse(readFileSync(AUDIT_PATH, "utf8"));
}

function assertText(value, field) {
  assert.equal(typeof value, "string", `${field} must be text`);
  assert.notEqual(value.length, 0, `${field} must not be empty`);
}

function assertEvidence(evidence, field) {
  assert.ok(Array.isArray(evidence) && evidence.length > 0, `${field} must be non-empty`);
  for (const [index, item] of evidence.entries()) {
    assertText(item.path, `${field}[${index}].path`);
    assert.match(item.sha256, SHA256, `${field}[${index}].sha256`);
    assertText(item.contains, `${field}[${index}].contains`);
  }
}

function closureDigest(closure) {
  const lines = closure.components
    .map(
      component =>
        `${component.name}@${component.version}\t${component.integrity}\t${component.filesManifestSha256}`
    )
    .sort();
  return sha256(Buffer.from(`solid-checker:phase13-rc3-closure:v1\n${lines.join("\n")}\n`));
}

export function assertPhase13Conformance(report, audit) {
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.documentKind, "solid-2-rc3-normalized-conformance-corpus");
  assert.equal(report.semanticModelVersion, 1);
  assert.equal(report.authority.solidVersion, "2.0.0-rc.3");
  assert.equal(report.authority.gitHead, "af6fee86e6dcfbf41869da2c607c82b1fd0939ce");
  assert.equal(audit.version, report.authority.solidVersion);
  assert.equal(audit.gitHead, report.authority.gitHead);
  assert.deepEqual(report.closureProofFamilies, EXPECTED_PROOF_FAMILIES);
  assert.equal(new Set(report.closureProofFamilies).size, EXPECTED_PROOF_FAMILIES.length);
  assert.deepEqual(Object.keys(report.closureIdentities), [
    "signals",
    "solid",
    "web",
    "autoAnimateSolid"
  ]);
  for (const [name, closure] of Object.entries(report.closureIdentities)) {
    assert.ok(closure.components.length > 0, `${name}: empty closure`);
    assert.equal(new Set(closure.components.map(component => component.name)).size, closure.components.length);
    for (const component of closure.components) {
      assertText(component.name, `${name}: closure package name`);
      assertText(component.version, `${name}: closure package version`);
      assert.match(component.integrity, /^sha512-/, `${name}: closure integrity`);
      assert.match(component.filesManifestSha256, SHA256, `${name}: file-manifest digest`);
    }
    assert.equal(closure.digest, closureDigest(closure), `${name}: closure digest`);
  }

  const audited = new Map(audit.packages.map(packageRow => [packageRow.name, packageRow]));
  for (const [name, integrity] of [
    ["solid-js", "sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ=="],
    ["@solidjs/signals", "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA=="],
    ["@solidjs/web", "sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg=="]
  ]) {
    assert.equal(audited.get(name)?.version, "2.0.0-rc.3");
    assert.equal(audited.get(name)?.registry.integrity, integrity);
    assert.equal(audited.get(name)?.integrity.verified, true);
  }

  assert.deepEqual(report.rows.map(row => row.id), EXPECTED_ROWS);
  assert.equal(new Set(report.rows.map(row => row.id)).size, EXPECTED_ROWS.length);
  assert.deepEqual(report.requiredFixtureKinds, [
    "positive",
    "negative",
    "partial",
    "refusal",
    "consumer",
    "typescriptOracle"
  ]);

  for (const row of report.rows) {
    assert.ok(Array.isArray(row.apis) && row.apis.length > 0, `${row.id}: APIs`);
    assert.ok(
      Array.isArray(row.authorityCases) && row.authorityCases.length > 0,
      `${row.id}: authority cases`
    );
    assert.ok(report.closureIdentities[row.closureIdentity], `${row.id}: closure identity`);
    assertEvidence(row.declarationEvidence, `${row.id}.declarationEvidence`);
    assertEvidence(row.runtimeEvidence, `${row.id}.runtimeEvidence`);
    assert.ok(Array.isArray(row.normalized.operations), `${row.id}: operations`);
    assert.ok(Array.isArray(row.normalized.edges), `${row.id}: edges`);
    assert.ok(Array.isArray(row.normalized.resources), `${row.id}: resources`);
    assert.ok(Array.isArray(row.normalized.guards), `${row.id}: guards`);
    assert.ok(
      Array.isArray(row.normalized.openDomains) && row.normalized.openDomains.length > 0,
      `${row.id}: exact open domains`
    );
    assert.ok(Array.isArray(row.proof.families) && row.proof.families.length > 0);
    for (const family of row.proof.families) {
      assert.ok(
        EXPECTED_PROOF_FAMILIES.includes(family),
        `${row.id}: unknown proof family ${family}`
      );
    }
    assert.equal(
      row.observation.absenceIsNegativeProof,
      false,
      `${row.id}: missing observation cannot be negative proof`
    );
    assert.ok(Array.isArray(row.observation.events));
    for (const kind of report.requiredFixtureKinds) {
      const fixture = row.fixtures[kind];
      assert.ok(fixture, `${row.id}: missing ${kind} fixture`);
      assertText(fixture.source, `${row.id}.${kind}.source`);
      if (kind !== "typescriptOracle") {
        assertText(fixture.expect, `${row.id}.${kind}.expect`);
      }
    }
    assert.ok(
      [0, 2].includes(row.fixtures.typescriptOracle.tscExitCode),
      `${row.id}: tsc oracle exit code`
    );
    if (row.fixtures.typescriptOracle.tscExitCode !== 0) {
      assert.equal(row.fixtures.typescriptOracle.ownership, "typescript");
      assertText(
        row.fixtures.typescriptOracle.tscDiagnosticContains,
        `${row.id}.typescriptOracle.tscDiagnosticContains`
      );
    }
    assert.equal(
      row.fixtures.typescriptOracle.checkerFinding,
      null,
      `${row.id}: conformance must not manufacture a TypeScript-owned finding`
    );
  }

  const experimental = report.rows.find(row => row.id === "experimental-server-components");
  assert.equal(experimental.stability, "experimental");
  assert.ok(experimental.normalized.openDomains.includes("unstable-frame-protocol"));
  assert.equal(experimental.proof.probeScenario, null);

  const mixed = report.rows.find(row => row.id === "mixed-framework-selection");
  assert.deepEqual(mixed.authorityCases, ["@formkit/auto-animate:./solid-import"]);
  assert.ok(mixed.normalized.openDomains.includes("rc3-incompatible-solid-imports"));
  assert.match(report.externalAuthorities[0].refusalPremise, /onMount.*onCleanup/);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function packageFilesManifest(packageRoot) {
  const files = [];
  const visit = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const file = join(directory, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile()) files.push(file);
      else throw new Error(`unsupported published package entry ${file}`);
    }
  };
  visit(packageRoot);
  files.sort((left, right) =>
    left.slice(packageRoot.length + 1).localeCompare(right.slice(packageRoot.length + 1))
  );
  const manifest = files
    .map(file => `${sha256(readFileSync(file))}  ${file.slice(packageRoot.length + 1)}\n`)
    .join("");
  return sha256(Buffer.from(manifest));
}

export function replayPublishedArtifactEvidence(report, nodeModulesRoot) {
  const observed = [];
  const evidence = report.rows.flatMap(row => [
    ...row.declarationEvidence.map(item => ({ row: row.id, kind: "declaration", ...item })),
    ...row.runtimeEvidence.map(item => ({ row: row.id, kind: "runtime", ...item }))
  ]);
  for (const item of evidence) {
    const file = join(nodeModulesRoot, item.path);
    const bytes = readFileSync(file);
    assert.equal(sha256(bytes), item.sha256, `${item.row}: stale ${item.kind} ${item.path}`);
    assert.ok(
      bytes.includes(Buffer.from(item.contains)),
      `${item.row}: selector ${JSON.stringify(item.contains)} is absent from ${item.path}`
    );
    observed.push({ row: item.row, kind: item.kind, path: item.path, sha256: item.sha256 });
  }

  const packageRows = [
    ["solid-js", report.authority.solidVersion],
    ["@solidjs/signals", report.authority.solidVersion],
    ["@solidjs/web", report.authority.solidVersion],
    ["@formkit/auto-animate", "0.10.0"]
  ];
  for (const [name, version] of packageRows) {
    const packageRoot = join(nodeModulesRoot, name);
    assert.equal(statSync(packageRoot).isDirectory(), true);
    const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
    assert.equal(manifest.name, name);
    assert.equal(manifest.version, version);
  }
  const checkedComponents = new Map();
  for (const closure of Object.values(report.closureIdentities)) {
    for (const component of closure.components) {
      if (checkedComponents.has(component.name)) {
        assert.equal(checkedComponents.get(component.name), component.filesManifestSha256);
        continue;
      }
      const actual = packageFilesManifest(join(nodeModulesRoot, component.name));
      assert.equal(actual, component.filesManifestSha256, `${component.name}: closure file manifest`);
      checkedComponents.set(component.name, actual);
    }
    assert.equal(closure.digest, closureDigest(closure));
  }
  return observed;
}

export function runTypeScriptOracles(report, nodeModulesRoot, tscPath) {
  const work = mkdtempSync(join(tmpdir(), "solid-checker-phase13-oracle-"));
  try {
    symlinkSync(resolve(nodeModulesRoot), join(work, "node_modules"), "dir");
    const results = [];
    for (const [index, row] of report.rows.entries()) {
      const source = join(work, `${String(index).padStart(2, "0")}-${row.id}.ts`);
      writeFileSync(source, `${row.fixtures.typescriptOracle.source}\n`);
      const result = spawnSync(
        process.execPath,
        [
          resolve(tscPath),
          "--noEmit",
          "--strict",
          "--skipLibCheck",
          "false",
          "--target",
          "ES2022",
          "--module",
          "ESNext",
          "--moduleResolution",
          "Bundler",
          "--customConditions",
          "browser,development",
          "--lib",
          "ES2022,DOM,DOM.Iterable",
          source
        ],
        { cwd: work, encoding: "utf8" }
      );
      const output = `${result.stdout}${result.stderr}`;
      assert.equal(
        result.status,
        row.fixtures.typescriptOracle.tscExitCode,
        `${row.id}: unexpected TypeScript oracle result:\n${output}`
      );
      if (row.fixtures.typescriptOracle.tscDiagnosticContains) {
        assert.ok(
          output.includes(row.fixtures.typescriptOracle.tscDiagnosticContains),
          `${row.id}: missing pinned TypeScript diagnostic:\n${output}`
        );
      }
      results.push({ row: row.id, exitCode: result.status });
    }
    return { cases: results.length, results };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

export function runNodeRuntimeEvidence(nodeModulesRoot, nodePath) {
  const scenarios = [
    {
      id: "split-create-effect",
      source:
        "import { createRoot, createSignal, createEffect, flush } from 'solid-js'; const events=[]; let setValue; let dispose; createRoot(d => { dispose=d; const pair=createSignal(0); setValue=pair[1]; createEffect(() => { const value=pair[0](); events.push('compute:'+value); return value; }, value => { events.push('apply:'+value); return () => events.push('cleanup:'+value); }); }); events.push('after-create'); flush(); events.push('after-flush'); setValue(1); events.push('after-set'); flush(); events.push('after-second-flush'); dispose(); events.push('after-dispose'); process.stdout.write(JSON.stringify(events));",
      expected: [
        "compute:0",
        "after-create",
        "apply:0",
        "after-flush",
        "after-set",
        "compute:1",
        "cleanup:0",
        "apply:1",
        "after-second-flush",
        "cleanup:1",
        "after-dispose"
      ]
    },
    {
      id: "tracked-effect-on-settled",
      source:
        "import { createRoot, createSignal, createTrackedEffect, onSettled, flush } from 'solid-js'; const events=[]; let setValue; let dispose; createRoot(d => { dispose=d; const pair=createSignal(0); setValue=pair[1]; createTrackedEffect(() => { events.push('tracked:'+pair[0]()); return () => events.push('tracked-cleanup'); }); onSettled(() => { events.push('settled'); return () => events.push('settled-cleanup'); }); }); flush(); setValue(1); flush(); dispose(); process.stdout.write(JSON.stringify(events));",
      expected: [
        "tracked:0",
        "settled",
        "tracked-cleanup",
        "tracked:1",
        "settled-cleanup",
        "tracked-cleanup"
      ]
    },
    {
      id: "batching-flush",
      source:
        "import { createRoot, createSignal, createTrackedEffect, flush } from 'solid-js'; const events=[]; let read; let setValue; let dispose; createRoot(d => { dispose=d; const pair=createSignal(0); read=pair[0]; setValue=pair[1]; createTrackedEffect(() => { events.push('effect:'+read()); }); }); flush(); setValue(1); events.push('read:'+read()); events.push('before-flush'); flush(() => { events.push('inside:'+read()); }); events.push('after-flush'); dispose(); process.stdout.write(JSON.stringify(events));",
      expected: ["effect:0", "read:0", "before-flush", "inside:0", "effect:1", "after-flush"]
    }
  ];
  const cwd = dirname(resolve(nodeModulesRoot));
  const results = [];
  for (const scenario of scenarios) {
    const result = spawnSync(
      resolve(nodePath),
      ["--conditions=browser", "--conditions=development", "--input-type=module", "-e", scenario.source],
      { cwd, encoding: "utf8" }
    );
    assert.equal(
      result.status,
      0,
      `${scenario.id}: exact RC.3 runtime replay failed:\n${result.stdout}${result.stderr}`
    );
    assert.deepEqual(JSON.parse(result.stdout), scenario.expected, `${scenario.id}: runtime trace`);
    results.push({ row: scenario.id, events: scenario.expected });
  }
  return results;
}

function usage() {
  process.stderr.write(
    "usage: bun scripts/package-contract-v2-phase13.mjs --check | --replay <node_modules-root> [--tsc <tsc-js>] [--node <node>]\n"
  );
}

if (import.meta.main) {
  const report = readPhase13Conformance();
  const audit = readPublishedAudit();
  assertPhase13Conformance(report, audit);
  const [mode, rootFlag, ...options] = process.argv.slice(2);
  if (mode === "--check" && rootFlag === undefined) {
    process.stdout.write(`Phase 13 conformance corpus: ${report.rows.length} rows valid\n`);
  } else if (mode === "--replay" && rootFlag) {
    const observations = replayPublishedArtifactEvidence(report, resolve(rootFlag));
    let oracle = null;
    let runtime = null;
    for (let index = 0; index < options.length; index += 2) {
      const flag = options[index];
      const value = options[index + 1];
      if (!value) {
        usage();
        process.exitCode = 2;
        break;
      }
      if (flag === "--tsc") oracle = runTypeScriptOracles(report, resolve(rootFlag), value);
      else if (flag === "--node") runtime = runNodeRuntimeEvidence(resolve(rootFlag), value);
      else {
        usage();
        process.exitCode = 2;
        break;
      }
    }
    if (process.exitCode !== 2) {
      process.stdout.write(
        `${JSON.stringify({ rows: report.rows.length, observations: observations.length, oracle, runtime })}\n`
      );
    }
  } else {
    usage();
    process.exitCode = 2;
  }
}
