// Node-side behavior of `solid-checker contract generate` that does not depend
// on real analysis: which failures are refusals and which are bugs, and
// whether the emitted contract is bound to the bytes it describes.
//
// These run against a stub native checker installed through the documented
// `SOLID_CHECKER_NATIVE_BIN` override, not against the real engine. That is
// deliberate and is the only way to pin the two behaviors here: a *crashing*
// native process and a *refusing* one both leave the generator with a non-zero
// exit and a message, and no real fixture can produce a panic on demand. The
// stub emits the exact stdout/stderr shapes the real binary emits (see
// `emit_package_contract` in rust/crates/solid-facts-backend/src/main.rs) --
// nothing here loosens what the generator accepts.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";

import {
  createModuleResolver,
  moduleSpecifiers,
  openDynamicImportReachability,
  runtimeModuleClosure
} from "../packages/cli/scripts/runtime-module-closure.mjs";

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");

// One stub per behavior, keyed by the entry file the generator hands it:
// "ok" writes a minimal normalized contract document, "refuse" reproduces a
// native fail-closed contract-emission refusal, "crash" reproduces a panic.
const STUB_NATIVE = `#!/usr/bin/env node
import { appendFileSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const args = process.argv.slice(2);
const value = flag => {
  const index = args.indexOf(flag);
  return index === -1 ? undefined : args[index + 1];
};

// Every invocation's argv, one JSON line each, when a test asks for it. Which
// *flag* carries a dependency contract is a semantic channel, not a spelling
// (see \`--generated-contract\` below), and nothing else in a generation's
// observable output records the choice.
if (process.env.STUB_ARGV_LOG) {
  appendFileSync(process.env.STUB_ARGV_LOG, \`\${JSON.stringify(args)}\\n\`);
}

if (args.includes("--validate-contract")) process.exit(0);

// The contract-coverage report the sweep enumerates packages from, in the
// shape rust/crates/solid-facts-backend/src/main.rs prints for
// \`--check-contracts --format json\`: exit 1 when any package needs action.
if (args.includes("--check-contracts")) {
  const report = JSON.parse(process.env.STUB_CONTRACT_REPORT ?? '{"packages":[]}');
  const actionable = report.packages.filter(entry =>
    ["missing", "unverified", "stale", "unbound"].includes(entry.status)
  );
  const stale = report.packages.filter(entry => entry.status === "stale");
  process.stdout.write(
    JSON.stringify({ ...report, missing: actionable.length, stale: stale.length })
  );
  process.exit(actionable.length ? 1 : 0);
}

const entryFile = value("--contract-entry-file") ?? "";
const plan = JSON.parse(process.env.STUB_NATIVE_PLAN ?? "{}");
// Keyed by package name as well as entry file, because a sweep generates
// several packages whose entry files share a basename.
const action = plan[value("--package-name")] ?? plan[basename(entryFile)] ?? "ok";

if (action === "refuse") {
  process.stderr.write(
    \`solid-checker-rust: emit package contract: entry file \${entryFile} is not part of the TypeScript project\\n\`
  );
  process.exit(2);
}
if (action === "crash") {
  process.stderr.write("thread 'main' panicked at src/main.rs:1:1:\\ninternal invariant violated\\n");
  process.exit(101);
}
// The missing-dependency boundary, as the real binary writes it: the
// machine-readable marker line first, then the human refusal. The prose here
// deliberately carries no module name, so only the marker can drive the
// retry -- a fallback that happened to still match would not prove the
// marker works.
// \`--generated-contract\` is \`--contract\` plus provenance the document cannot
// carry: both spellings push onto \`Request::contract_paths\`, and only the
// second also lands in \`generated_contract_paths\` (rust/crates/
// solid-facts-backend/src/main.rs). So either channel satisfies the boundary
// this refusal is about, and gating on one flag alone would refuse a retry the
// real binary accepts.
const carriesDependencyContract =
  args.includes("--contract") || args.includes("--generated-contract");

if (action === "needs-dependency" && !carriesDependencyContract) {
  process.stderr.write(
    \`solid-checker:unresolved-dependency-module=\${process.env.STUB_DEPENDENCY_MODULE}\\n\`
  );
  process.stderr.write(
    \`solid-checker-rust: emit package contract: this entrypoint re-exports a package with no contract\\n\`
  );
  process.exit(2);
}

writeFileSync(
  value("--emit-contract"),
  JSON.stringify({
    schemaVersion: 1,
    package: { name: value("--package-name"), version: value("--package-version") },
    compilerFactsProtocol: 1,
    summaries: { value: { kind: "value" } },
    entrypoints: { ".": { exports: { value: ["thing"] } } },
    evidence: { kind: "inferred" }
  })
);

// The module inventory \`--emit-module-inventory\` writes, in the shape
// \`write_module_inventory\` writes it (rust/crates/solid-facts-backend/src/
// main.rs): realpaths, sorted, with \`complete\` carrying the producer's own
// \`ModuleGraph::is_complete\`.
//
// The stub cannot resolve modules, so its default answer is the tsconfig
// \`files\` list and *no* import facts. That is not a loosening: for every
// specifier shape these tests drive -- an asset import, a relative specifier
// naming nothing, an \`#imports\` branch no bundler condition selects -- the real
// compiler answers \`resolution: "unresolved"\`, which reconciles identically to
// no fact at all (verified against the pinned producer). A test that needs the
// compiler to have resolved something the walk missed says so through
// \`STUB_MODULE_IMPORTS\` / \`STUB_INVENTORY_EXTRA_MODULES\` rather than getting it
// for free, and \`STUB_INVENTORY_ABSENT\` / \`STUB_INVENTORY_INCOMPLETE\` drive the
// two fail-closed shapes no real run can produce on demand.
const inventoryPath = value("--emit-module-inventory");
if (inventoryPath && !process.env.STUB_INVENTORY_ABSENT) {
  const packageRoot = realpathSync(value("--contract-package-root") ?? ".");
  const project = JSON.parse(readFileSync(value("--project"), "utf8"));
  const extra = JSON.parse(process.env.STUB_INVENTORY_EXTRA_MODULES ?? "[]");
  const declared = JSON.parse(process.env.STUB_MODULE_IMPORTS ?? "[]");
  const modules = [
    ...(project.files ?? []).map(file => ({ path: realpathSync(file) })),
    ...extra.map(entry =>
      typeof entry === "string"
        ? { path: resolve(packageRoot, entry) }
        : { path: resolve(packageRoot, entry.path), declarationFile: true }
    )
  ].sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
  writeFileSync(
    inventoryPath,
    JSON.stringify({
      schemaVersion: 1,
      projectId: value("--project"),
      packageRoot,
      complete: !process.env.STUB_INVENTORY_INCOMPLETE,
      modules,
      imports: declared.map(entry => ({
        path: resolve(packageRoot, entry.from),
        startByte: 0,
        endByte: 0,
        text: entry.text,
        resolution: entry.resolution ?? "relative",
        ...(entry.resolved ? { resolvedPath: resolve(packageRoot, entry.resolved) } : {}),
        ...(entry.extension ? { extension: entry.extension } : {})
      })),
      unknownImportPaths: process.env.STUB_INVENTORY_INCOMPLETE
        ? [resolve(packageRoot, process.env.STUB_INVENTORY_INCOMPLETE)]
        : []
    })
  );
}
`;

function makeWorkspace(exports_, { dependency, files } = {}) {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-generation-"));
  const packageRoot = join(directory, "package");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    join(packageRoot, "package.json"),
    `${JSON.stringify(
      { name: "stub-package", version: "1.0.0", type: "module", exports: exports_ },
      null,
      2
    )}\n`
  );
  for (const target of new Set(Object.values(exports_))) {
    writeFileSync(join(packageRoot, target.replace("./", "")), "export const thing = 1;\n");
  }
  // Extra or replacement runtime files, for the shapes a flat "one file per
  // target" package cannot express -- a barrel entry re-exporting a sibling.
  for (const [name, contents] of Object.entries(files ?? {})) {
    writeFileSync(join(packageRoot, name), contents);
  }
  // An *installed* dependency, because demand-driven generation only recurses
  // into an artifact it can find on disk under the package root.
  if (dependency) {
    const dependencyRoot = join(packageRoot, "node_modules", dependency);
    mkdirSync(dependencyRoot, { recursive: true });
    writeFileSync(
      join(dependencyRoot, "package.json"),
      `${JSON.stringify(
        {
          name: dependency,
          version: "1.0.0",
          type: "module",
          exports: { ".": "./dependency.mjs" }
        },
        null,
        2
      )}\n`
    );
    writeFileSync(join(dependencyRoot, "dependency.mjs"), "export const thing = 1;\n");
  }
  const stub = join(directory, "stub-native.mjs");
  writeFileSync(stub, STUB_NATIVE);
  chmodSync(stub, 0o755);
  return { directory, packageRoot, stub };
}

function generate({
  packageRoot,
  stub,
  plan = {},
  args = [],
  dependencyModule,
  argvLog,
  inventory = {}
}) {
  return spawnSync(process.execPath, [cli, "contract", "generate", ...args], {
    cwd: packageRoot,
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: stub,
      STUB_NATIVE_PLAN: JSON.stringify(plan),
      ...(dependencyModule ? { STUB_DEPENDENCY_MODULE: dependencyModule } : {}),
      ...(argvLog ? { STUB_ARGV_LOG: argvLog } : {}),
      // What the analyzing program attests it opened, for the shapes a stub
      // cannot produce by resolving anything. See `STUB_MODULE_IMPORTS` in the
      // stub for why the default is deliberately empty rather than generous.
      ...(inventory.absent ? { STUB_INVENTORY_ABSENT: "1" } : {}),
      ...(inventory.incomplete ? { STUB_INVENTORY_INCOMPLETE: inventory.incomplete } : {}),
      ...(inventory.imports
        ? { STUB_MODULE_IMPORTS: JSON.stringify(inventory.imports) }
        : {}),
      ...(inventory.extraModules
        ? { STUB_INVENTORY_EXTRA_MODULES: JSON.stringify(inventory.extraModules) }
        : {})
    },
    encoding: "utf8"
  });
}

// Each native invocation's argv, in order, from a run given `argvLog`.
function nativeInvocations(argvLog) {
  return readFileSync(argvLog, "utf8")
    .split("\n")
    .filter(line => line.length)
    .map(line => JSON.parse(line));
}

test("a deliberate refusal omits one entrypoint and generation continues", () => {
  const { directory, packageRoot, stub } = makeWorkspace({
    ".": "./index.mjs",
    "./b": "./b.mjs"
  });
  try {
    const result = generate({ packageRoot, stub, plan: { "b.mjs": "refuse" } });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /1 entrypoint\(s\) refused and omitted/);

    const contract = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.json"), "utf8"));
    assert.deepEqual(Object.keys(contract.entrypoints), ["."]);

    // The refused entrypoint is absent from the contract, so the review plan
    // is the only place a reviewer learns it was refused rather than absent
    // from the package.
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    // The section is no longer the generator's alone: `contract verify` pushes
    // the same item kind for an entrypoint whose `kind` claims no run observed
    // (RFC 0002 amendment A9), so its title names the refusal, not the refuser.
    assert.match(review, /## entrypoints refused as uncertifiable/);
    assert.match(review, /- \[ \] \.\/b: .*is not part of the TypeScript project/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an unexpected native failure fails generation instead of refusing an entrypoint", () => {
  const { directory, packageRoot, stub } = makeWorkspace({
    ".": "./index.mjs",
    "./b": "./b.mjs"
  });
  try {
    const result = generate({ packageRoot, stub, plan: { "b.mjs": "crash" } });
    assert.equal(result.status, 2, result.stdout);
    assert.match(result.stderr, /panicked/);
    // Crucially: no contract at all. A crash on one entrypoint must never
    // ship the other one as a complete, exit-0 contract.
    assert.equal(existsSync(join(packageRoot, "solid-reactivity.json")), false);
    assert.equal(existsSync(join(packageRoot, "solid-reactivity.review.md")), false);
    assert.doesNotMatch(result.stdout, /refused and omitted/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an in-package contract binds to the implementation artifact's exact bytes", () => {
  const { directory, packageRoot, stub } = makeWorkspace({ ".": "./index.mjs" });
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);

    const contract = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.json"), "utf8"));
    const expected = `sha256:${createHash("sha256")
      .update(readFileSync(join(packageRoot, "index.mjs")))
      .digest("hex")}`;
    assert.deepEqual(contract.artifacts, {
      implementation: { path: "index.mjs", hash: expected }
    });
    // Schema v1 resolves the path inside the contract's own directory and
    // rejects anything absolute or escaping, so neither may ever be emitted.
    assert.doesNotMatch(contract.artifacts.implementation.path, /^(?:\/|\.\.\/)/);

    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /## contract artifact binding\n\n- \[x\] none observed by the generator/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a barrel entry is bound at the entry artifact only and counts the rest", () => {
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.mjs" },
    {
      files: {
        "index.mjs": 'export { thing } from "./internal.mjs";\n',
        "internal.mjs": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);

    // The hash keeps being emitted: it is real evidence about the entry file,
    // and schema v1 has no second pair to carry the rest.
    const contract = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.json"), "utf8"));
    assert.equal(contract.artifacts.implementation.path, "index.mjs");

    // But the semantics come from `internal.mjs` too, and nothing pins its
    // bytes -- so the review plan must not read as full byte binding.
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    // The count is the attested one -- how many modules the analyzing program
    // opened under this package beyond the entry -- not a second walk of the
    // same entrypoint.
    assert.match(
      review,
      /- \[ \] contract is byte-bound to its entry artifact only: \.\/index\.mjs pulls in 1 further module\(s\) the analysis read/
    );
    assert.doesNotMatch(review, /## contract artifact binding\n\n- \[x\] none observed/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a contract emitted outside the package stays unbound and says so", () => {
  const { directory, packageRoot, stub } = makeWorkspace({ ".": "./index.mjs" });
  const output = join(directory, "project", ".solid-checker/contracts/stub-package/solid-reactivity.json");
  try {
    const result = generate({ packageRoot, stub, args: ["--output", output] });
    assert.equal(result.status, 0, result.stderr);

    const contract = JSON.parse(readFileSync(output, "utf8"));
    assert.equal(contract.artifacts, undefined);

    const review = readFileSync(output.replace(/\.json$/, ".review.md"), "utf8");
    assert.match(review, /## contract artifact binding/);
    assert.match(review, /- \[ \] contract is not byte-bound: \.\/index\.mjs is outside the contract's own directory/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a contract backed by several runtime artifacts stays unbound and says so", () => {
  const { directory, packageRoot, stub } = makeWorkspace({
    ".": "./index.mjs",
    "./b": "./b.mjs"
  });
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);

    const contract = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.json"), "utf8"));
    assert.equal(Object.keys(contract.entrypoints).length, 2);
    // Hashing one of two artifacts would claim byte identity the contract
    // does not have; schema v1 carries exactly one implementation pair.
    assert.equal(contract.artifacts, undefined);

    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(
      review,
      /- \[ \] contract is not byte-bound: 2 runtime artifacts back this contract/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// Demand-driven dependency generation is the one native failure that is not
// an outcome: the entrypoint is generable, this run just lacks the boundary
// package's contract. The generator generates exactly that installed
// dependency and retries. Two properties make or break it, and both fail
// silently -- as an entrypoint "refused and omitted", which exits 0.
test("the native dependency marker drives recursive dependency generation", () => {
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./main.mjs" },
    { dependency: "boundary-package" }
  );
  const argvLog = join(directory, "native-argv.log");
  try {
    const result = generate({
      packageRoot,
      stub,
      plan: { "main.mjs": "needs-dependency" },
      dependencyModule: "boundary-package",
      argvLog
    });
    assert.equal(result.status, 0, result.stderr);
    assert.doesNotMatch(result.stdout, /refused and omitted/);

    const contract = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.json"), "utf8"));
    assert.deepEqual(Object.keys(contract.entrypoints), ["."]);

    // The retried invocation must carry the dependency contract on the
    // *trusted* channel. This run generated that contract from the
    // dependency's own installed sources, so its `kind` claims were decided by
    // this engine's rule; `--generated-contract` is the only thing that says
    // so (`kind_claims_are_trusted`). Sending the same path as `--contract`
    // would make the engine re-decide those claims as if the file had merely
    // been discovered on disk -- and every assertion above would still pass.
    const invocations = nativeInvocations(argvLog);
    assert.ok(
      invocations.some(args => args.includes("--generated-contract")),
      "no invocation carried the generated dependency contract"
    );
    assert.ok(
      invocations.every(args => !args.includes("--contract")),
      "a generated dependency contract was passed on the untrusted channel"
    );

    // The marker is addressed to this script. A reviewer reading the plan
    // must never see it, and the human sentence must survive intact when a
    // refusal does reach the plan.
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.doesNotMatch(review, /solid-checker:unresolved-dependency-module/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a marker naming a dependency that is not installed refuses instead of looping", () => {
  // No `dependency:` -- the boundary package is named but absent from disk,
  // so there is nothing to generate. That is a refusal about this entrypoint,
  // not a bug, and it must terminate rather than retry forever.
  const { directory, packageRoot, stub } = makeWorkspace({
    ".": "./main.mjs",
    "./b": "./b.mjs"
  });
  try {
    const result = generate({
      packageRoot,
      stub,
      plan: { "main.mjs": "needs-dependency" },
      dependencyModule: "boundary-package"
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /1 entrypoint\(s\) refused and omitted/);

    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /- \[ \] \.: .*re-exports a package with no contract/);
    assert.doesNotMatch(review, /solid-checker:unresolved-dependency-module/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// `contract generate --missing` sweeps the contract-coverage report. The stub
// supplies both halves of it: the report itself, and the per-package
// generation outcome.

function makeSweepWorkspace(packages) {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-sweep-"));
  const project = join(directory, "project");
  mkdirSync(project, { recursive: true });
  writeFileSync(join(project, "tsconfig.json"), "{}\n");
  for (const [name, exports_] of Object.entries(packages)) {
    const packageRoot = join(project, "node_modules", ...name.split("/"));
    mkdirSync(packageRoot, { recursive: true });
    writeFileSync(
      join(packageRoot, "package.json"),
      `${JSON.stringify({ name, version: "1.0.0", type: "module", exports: exports_ }, null, 2)}\n`
    );
    for (const target of new Set(Object.values(exports_))) {
      writeFileSync(join(packageRoot, target.replace("./", "")), "export const thing = 1;\n");
    }
  }
  const stub = join(directory, "stub-native.mjs");
  writeFileSync(stub, STUB_NATIVE);
  chmodSync(stub, 0o755);
  return { directory, project, stub };
}

function sweep({ project, stub, report, plan = {}, args = ["--missing"] }) {
  return spawnSync(process.execPath, [cli, "contract", "generate", ...args], {
    cwd: project,
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: stub,
      STUB_NATIVE_PLAN: JSON.stringify(plan),
      STUB_CONTRACT_REPORT: JSON.stringify(report ?? { packages: [] })
    },
    encoding: "utf8"
  });
}

function localContract(project, name) {
  return join(project, ".solid-checker/contracts", name, "solid-reactivity.json");
}

test("the sweep generates the missing packages and leaves every other status alone", () => {
  const { directory, project, stub } = makeSweepWorkspace({
    alpha: { ".": "./index.mjs" },
    "@scope/gamma": { ".": "./index.mjs" },
    delta: { ".": "./index.mjs" },
    epsilon: { ".": "./index.mjs" }
  });
  // An unverified contract is a draft a reviewer owns. Regenerating it would
  // discard that work, so the sweep must not write these bytes.
  const draft = localContract(project, "delta");
  mkdirSync(dirname(draft), { recursive: true });
  writeFileSync(draft, "{ /* reviewer's draft */ }\n");
  try {
    const result = sweep({
      project,
      stub,
      report: {
        packages: [
          { name: "alpha", status: "missing", remedy: "generate a contract", contractPath: "" },
          { name: "@scope/gamma", status: "missing", remedy: "generate a contract", contractPath: "" },
          { name: "delta", status: "unverified", detail: "evidence inferred", remedy: "review the contract", contractPath: draft },
          { name: "epsilon", status: "stale", detail: "1.0.0 versus 2.0.0", remedy: "regenerate and re-review the contract", contractPath: "" },
          { name: "solid-js", status: "bundled", contractPath: "bundled://solid-v2/solid-js.json" }
        ]
      }
    });
    assert.equal(result.status, 0, result.stderr);

    for (const name of ["alpha", "@scope/gamma"]) {
      const contract = JSON.parse(readFileSync(localContract(project, name), "utf8"));
      assert.equal(contract.package.name, name);
      // The sweep never promotes evidence, and every generated contract still
      // gets its review checklist.
      assert.equal(contract.evidence.kind, "inferred");
      assert.equal(
        existsSync(localContract(project, name).replace(/\.json$/, ".review.md")),
        true
      );
    }

    assert.equal(readFileSync(draft, "utf8"), "{ /* reviewer's draft */ }\n");
    assert.equal(existsSync(localContract(project, "epsilon")), false);
    assert.equal(existsSync(localContract(project, "solid-js")), false);

    assert.match(result.stdout, /^delta: unverified, left alone; review the contract$/m);
    assert.match(
      result.stdout,
      /^epsilon: stale, left alone; regenerate and re-review the contract$/m
    );
    assert.match(
      result.stdout,
      /swept 2 missing package\(s\): 2 generated, 0 generated with refused entrypoints, 0 failed/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a package generated with refused entrypoints is a success for the sweep", () => {
  const { directory, project, stub } = makeSweepWorkspace({
    zeta: { ".": "./index.mjs", "./b": "./b.mjs" }
  });
  try {
    const result = sweep({
      project,
      stub,
      plan: { "b.mjs": "refuse" },
      report: {
        packages: [{ name: "zeta", status: "missing", remedy: "generate a contract", contractPath: "" }]
      }
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /1 entrypoint\(s\) refused and omitted/);
    assert.match(
      result.stdout,
      /swept 1 missing package\(s\): 0 generated, 1 generated with refused entrypoints, 0 failed/
    );

    const contract = JSON.parse(readFileSync(localContract(project, "zeta"), "utf8"));
    assert.deepEqual(Object.keys(contract.entrypoints), ["."]);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("one package failing outright does not stop the sweep and exits non-zero", () => {
  const { directory, project, stub } = makeSweepWorkspace({
    alpha: { ".": "./index.mjs" },
    beta: { ".": "./index.mjs" },
    "@scope/gamma": { ".": "./index.mjs" }
  });
  try {
    const result = sweep({
      project,
      stub,
      plan: { beta: "crash" },
      report: {
        packages: ["alpha", "beta", "@scope/gamma"].map(name => ({
          name,
          status: "missing",
          remedy: "generate a contract",
          contractPath: ""
        }))
      }
    });
    // The failure is a bug-class error for beta only: it proves nothing about
    // alpha or gamma, and it must not pass as a complete run either.
    assert.equal(result.status, 1, result.stdout);
    assert.equal(existsSync(localContract(project, "alpha")), true);
    assert.equal(existsSync(localContract(project, "@scope/gamma")), true);
    assert.equal(existsSync(localContract(project, "beta")), false);
    assert.match(result.stderr, /^solid-checker: beta: .*panicked/m);
    assert.match(
      result.stdout,
      /swept 3 missing package\(s\): 2 generated, 0 generated with refused entrypoints, 1 failed/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a sweep with no missing package generates nothing and exits 0", () => {
  const { directory, project, stub } = makeSweepWorkspace({ delta: { ".": "./index.mjs" } });
  try {
    const result = sweep({
      project,
      stub,
      report: {
        packages: [
          { name: "delta", status: "unverified", remedy: "review the contract", contractPath: "" },
          { name: "solid-js", status: "bundled", contractPath: "bundled://solid-v2/solid-js.json" }
        ]
      }
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /no package contract is missing; nothing to generate\./);
    assert.equal(existsSync(join(project, ".solid-checker")), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("--missing rejects every single-package generation flag", () => {
  const { directory, project, stub } = makeSweepWorkspace({ alpha: { ".": "./index.mjs" } });
  try {
    for (const args of [
      ["--missing", "--package-root", "node_modules/alpha"],
      ["--missing", "--output", "contract.json"],
      ["--missing", "--entrypoint", "./b"],
      ["--missing", "--conditions", "browser,import"],
      ["--missing", "--contract", "other.json"]
    ]) {
      const result = sweep({ project, stub, args });
      assert.equal(result.status, 2, `${args[1]}: ${result.stdout}`);
      assert.match(
        result.stderr,
        new RegExp(`${args[1]} generates one package and --missing sweeps every missing one`)
      );
      assert.equal(existsSync(join(project, ".solid-checker")), false);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("--format json reports the sweep as one document and keeps stdout parseable", () => {
  const { directory, project, stub } = makeSweepWorkspace({ alpha: { ".": "./index.mjs" } });
  try {
    const result = sweep({
      project,
      stub,
      args: ["--missing", "--format", "json"],
      report: {
        packages: [
          { name: "alpha", status: "missing", remedy: "generate a contract", contractPath: "" },
          { name: "delta", status: "unverified", remedy: "review the contract", contractPath: "" }
        ]
      }
    });
    assert.equal(result.status, 0, result.stderr);
    // The per-package generation line would otherwise land in the middle of
    // the document.
    const report = JSON.parse(result.stdout);
    assert.equal(report.generated.length, 1);
    assert.equal(report.generated[0].package, "alpha");
    assert.equal(report.generated[0].refusedEntrypoints, 0);
    assert.deepEqual(report.failed, []);
    assert.deepEqual(
      report.skipped.map(entry => entry.name),
      ["delta"]
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

// What the closure record says, and what it refuses to leave unsaid.
//
// The walk still decides the TypeScript project's `files` list -- it is the
// seeder, because a published ESM barrel's `.js` specifiers resolve to adjacent
// `.d.ts` files when only the entry is seeded. What it no longer decides is the
// record: the unpinned-module count and the per-entrypoint hash set a review
// transfers against come from the analyzing program's own module inventory, and
// the walk's own problems are reconciled against it. A module either side names
// and the other does not is a named note, in both directions.
//
// The cases below therefore pin two things at once: that the record is the
// attested one, and that reconciliation drops exactly the notes the compiler
// agrees name nothing -- never a note about a module it did resolve.

function closureOf(packageRoot, entrypoint = ".") {
  const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
  return plan.generation.entrypoints[entrypoint];
}

test("finite conditional dynamic imports enumerate every literal branch", () => {
  assert.deepEqual(
    moduleSpecifiers(
      'export const module = import(server ? "./server.js" : nested ? "./dev.js" : "./web.js");\n'
    ),
    {
      specifiers: ["./server.js", "./dev.js", "./web.js"],
      problems: []
    }
  );
});

test("finite static-table dynamic imports require a finite selector", () => {
  const table = 'Object.freeze({ server: "./server.js", web: "./web.js" })';
  assert.deepEqual(moduleSpecifiers(`import(${table}[server ? "server" : "web"]);\n`), {
    specifiers: ["./server.js", "./web.js"],
    problems: []
  });
  assert.deepEqual(moduleSpecifiers(`import(${table}[runtimeName]);\n`), {
    specifiers: [],
    problems: [
      {
        kind: "dynamic-import",
        reason:
          "a dynamic import() whose specifier is not statically bounded to a finite set of string literals"
      }
    ]
  });
});

test("an open branch keeps a dynamic import unbounded", () => {
  assert.deepEqual(moduleSpecifiers('import(server ? "./server.js" : runtimeName);\n'), {
    specifiers: [],
    problems: [
      {
        kind: "dynamic-import",
        reason:
          "a dynamic import() whose specifier is not statically bounded to a finite set of string literals"
      }
    ]
  });
});

test("finite dynamic imports seed and attest every reachable runtime module", () => {
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js":
          'export const loaded = import(server ? "./server.js" : "./web.js");\n',
        "server.js": "export const mode = 'server';\n",
        "web.js": "export const mode = 'web';\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), [
      "index.js",
      "server.js",
      "web.js"
    ]);
    assert.equal(closure.notes, undefined);
    assert.equal(closure.runtimeNotes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the closure exposes the exact static edges used to seed analysis", () => {
  const { directory, packageRoot } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'import { helper } from "./helper.js"; export { helper };\n',
        "helper.js": "export function helper() {}\n"
      }
    }
  );
  try {
    const resolver = createModuleResolver({ packageRoot });
    const closure = runtimeModuleClosure({
      packageRoot,
      entryFile: join(packageRoot, "index.js"),
      excludedFiles: new Set(),
      resolver
    });
    assert.deepEqual(closure.resolutions, [
      {
        importer: join(packageRoot, "index.js"),
        specifier: "./helper.js",
        target: join(packageRoot, "helper.js")
      }
    ]);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an open dynamic import withdraws only exports that reach its containing function", () => {
  const source = `
function loadAssets(url) { return import(url); }
function hydrateImpl() { runtime.load = loadAssets; }
const hydrate = (...args) => hydrateImpl(...args);
function ssrGroup(fn, n) { fn.$g = n; return fn; }
export { hydrate, ssrGroup };
`;
  assert.deepEqual(openDynamicImportReachability(source, ["hydrate", "ssrGroup"]), {
    affectedExports: ["hydrate"]
  });
});

test("an open top-level import or escaped loader remains entrypoint-wide", () => {
  const escaped = openDynamicImportReachability(
    `const load = url => import(url);\nregistry.load = load;\nexport { load };\n`,
    ["load"]
  );
  assert.match(escaped.ambiguous, /escapes at module scope/);
  const topLevel = openDynamicImportReachability(
    `const pending = import(runtimeUrl);\nexport { pending };\n`,
    ["pending"]
  );
  assert.match(
    topLevel.ambiguous,
    /outside an attributable function/
  );
});

test("a .js specifier that resolves to a TypeScript sibling is recorded", () => {
  // TypeScript resolves an ESM-spelled `./impl.js` against the source that
  // exists, and the analysis reads `impl.ts`. Recording only the entry left the
  // module every summary came from outside the hash set.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "./impl.js";\n',
        "impl.ts": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.ts", "index.js"]);
    assert.equal(closure.notes, undefined);

    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /pulls in 1 further module\(s\) the analysis read/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a #imports specifier is resolved through the package's imports map", () => {
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "#internal";\n',
        "internal.mjs": "export const thing = 1;\n"
      }
    }
  );
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  manifest.imports = { "#internal": "./internal.mjs" };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), [
      "index.js",
      "internal.mjs"
    ]);
    assert.equal(closure.notes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an unresolvable #imports branch is a runtime claim, not a record note", () => {
  // Two conditional targets and no selection: the walk refuses to guess, because
  // picking one would put a browser build's bytes behind a node build's
  // summaries. The record, though, is not a guess either way -- the analyzing
  // program resolved `#internal` to nothing (`bundler` resolution selects
  // neither `browser` nor `node`), so it read neither branch, and naming only
  // `index.js` is the complete and correct answer *about the bytes the analysis
  // read*.
  //
  // And that is where saying nothing would be a wrong certification. Node, under
  // its own conditions, loads `node.mjs`; a bundler targeting the browser loads
  // `browser.mjs`. Both are package code the analysis never read, and a
  // side-effect import of such a branch is exactly where a package patches a
  // global or calls into `solid-js/web`. So the claim that survives is the same
  // one a non-literal `import()` makes -- the runtime may load a module the
  // analysis never read -- and it rides the same field: `runtimeNotes`, blocking
  // promotion, not a transfer between two identical records.
  //
  // What separates this from `./styles.css` is a fact and not an extension
  // guess: `runtimeTargets` names the existing runtime modules a runtime can
  // select for the specifier. An asset import names none, so no runtime loads a
  // module for it and there is nothing left to say.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "#internal";\n',
        "browser.mjs": "export const thing = 1;\n",
        "node.mjs": "export const thing = 2;\n"
      }
    }
  );
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  manifest.imports = { "#internal": { browser: "./browser.mjs", node: "./node.mjs" } };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path), ["index.js"]);
    // Not a record note: the record is not short, and saying it might be would
    // be false now that the program's own file list is what it names.
    assert.equal(closure.notes, undefined);
    assert.equal(closure.runtimeNotes.length, 1);
    assert.match(
      closure.runtimeNotes[0],
      /^index\.js: the module record is attested .* except for what #internal may load at runtime: the analyzing program resolved nothing for it \(.*\), while browser\.mjs, node\.mjs exist on disk and a runtime selecting one of them reads package bytes this analysis did not/
    );

    // And no branch was *recorded*: naming the reachable branches in the note is
    // not a way of hashing one of them into the record after all.
    assert.deepEqual(
      closure.modules.filter(module => /browser\.mjs|node\.mjs/.test(module.path)),
      []
    );
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /- \[ \] \. index\.js: the module record is attested/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a string containing a comment opener does not hide the imports below it", () => {
  // The predecessor stripped `/* */` with a regular expression that knows
  // nothing about strings, so one URL ate the rest of the file: the closure
  // recorded the entry alone, with no note to say it had.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js":
          'export const docs = "https://example.com/*";\n' +
          "export const pattern = /[\"']/g;\n" +
          'export { thing } from "./impl.mjs";\n',
        "impl.mjs": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.mjs", "index.js"]);
    assert.equal(closure.notes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a specifier naming nothing drops its note; a non-literal import() keeps one", () => {
  // The two halves of what the walk used to say with one sentence each.
  //
  // `./gone.js` names no file, and the analyzing program resolved nothing for it
  // either -- so the analysis read no bytes for it and the record is complete.
  // The note goes.
  //
  // A non-literal `import()` is the compiler resolving nothing too, and the
  // record is complete for the same reason. What is *not* established is that
  // the runtime loads no module the analysis never read, and no module graph can
  // establish it. That is a different claim, it rides `runtimeNotes`, and it
  // still refuses promotion.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js":
          'export { a } from "./gone.js";\n' +
          "export const b = await import(`./${name}.js`);\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.equal(closure.notes, undefined);
    assert.equal(closure.runtimeNotes.length, 1);
    assert.match(
      closure.runtimeNotes[0],
      /the module record is attested .* and complete except for what a dynamic import\(\) whose specifier is not statically bounded to a finite set of string literals may load at runtime/
    );

    // A reviewer sees it in the same section a closure note appears in: the two
    // kinds differ in which gate they block, not in whether a human must look.
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /## contract artifact binding/);
    assert.match(review, /- \[ \] \. index\.js: the module record is attested/);
    assert.doesNotMatch(review, /gone\.js/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a module the analyzing program opened and the walk never seeded is noted", () => {
  // The residue an attested record exists to expose, in the shape that produced
  // it: the walk's clause scanner gives up after 300 tokens at depth zero, so a
  // long import clause hides its own `from` specifier -- silently, with no note,
  // because the scanner never saw a specifier to fail on. The compiler resolved
  // it and read the module. Nothing before this could observe that.
  const names = Array.from({ length: 200 }, (_, index) => `z${index + 1}`);
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js":
          `import { ${names.join(", ")} } from "./big.js";\n` +
          "export const thing = z1;\n",
        "big.js": `${names.map(name => `export const ${name} = 1;`).join("\n")}\n`
      }
    }
  );
  try {
    const result = generate({
      packageRoot,
      stub,
      inventory: { extraModules: ["big.js"] }
    });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    // The record names it, because the analysis read it.
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["big.js", "index.js"]);
    assert.equal(closure.notes.length, 1);
    assert.match(
      closure.notes[0],
      /^big\.js: the analyzing program opened this module and the closure walk did not seed it/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a specifier the walk could not resolve and the program did keeps a restated note", () => {
  // The other reconciliation branch: the compiler resolved what the walk could
  // not, so the note stays -- and it stays with the attested path attached,
  // which is strictly more than "names no runtime module inside the package"
  // could say. `.cjs`/`.cts` is the real shape (the walk's runtime extensions
  // omit them deliberately; `bundler` resolution substitutes them).
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "./impl.cjs";\n',
        "impl.cts": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({
      packageRoot,
      stub,
      inventory: {
        extraModules: ["impl.cts"],
        imports: [
          {
            from: "index.js",
            text: "./impl.cjs",
            resolution: "relative",
            resolved: "impl.cts",
            extension: ".cts"
          }
        ]
      }
    });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.cts", "index.js"]);
    assert.equal(closure.notes.length, 1);
    assert.match(
      closure.notes[0],
      /^index\.js: closure could not be fully enumerated: \.\/impl\.cjs names no runtime module inside the package \(.*\); the analyzing program resolved it to impl\.cts \(relative, \.cts\), so the analysis read a module this walk did not seed$/
    );

    // One cause, one note: the module is not also reported as an unseeded
    // module by the inventory sweep.
    assert.equal(
      closure.notes.filter(note => /did not seed it/.test(note)).length,
      0
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a dependency's own bytes are not recorded as this package's", () => {
  // The record answers "which bytes of *this* package did the summaries come
  // from", and a nested `node_modules/` inside the package root is not this
  // package. Hashing it would bind the record to the install layout -- hoisted
  // and the file is absent, nested and it is present -- and to a dependency's
  // version, so two generations over byte-identical package bytes would refuse
  // to transfer a review. What the analysis read from a dependency is described
  // by that package's own contract; see docs/precision-backlog.md for the
  // residue when it has none.
  //
  // And the exclusion is not a hole in the seeding sweep either: an excluded
  // module must not come back as "the program opened this and the walk did not
  // seed it".
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    { files: { "index.js": 'import { dep } from "dep";\nexport const thing = dep;\n' } }
  );
  mkdirSync(join(packageRoot, "node_modules", "dep"), { recursive: true });
  writeFileSync(join(packageRoot, "node_modules", "dep", "index.js"), "export const dep = 1;\n");
  try {
    const result = generate({
      packageRoot,
      stub,
      inventory: { extraModules: ["node_modules/dep/index.js"] }
    });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path), ["index.js"]);
    assert.equal(closure.notes, undefined);
    assert.equal(closure.runtimeNotes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("two spellings of one file are one module, on either kind of filesystem", () => {
  // A case-insensitive filesystem -- APFS, HFS+, NTFS -- accepts `./Impl.js` for
  // a file named `impl.js`, so the walk seeds two roots for one file and the
  // analyzing program answers with whichever spelling it was handed. The record
  // used to name the wrong-cased one, which exists on no case-sensitive
  // filesystem, *and* report the real one as seeded-but-never-opened -- a false
  // note about a file that was read.
  //
  // Both assertions below hold on both kinds of filesystem, which is the point:
  // on a case-sensitive one the walk resolves only `./impl.js` and the other
  // specifier names nothing (no existing runtime module, so no runtime loads one
  // either, so no note); on a case-insensitive one both spellings resolve and
  // `realpathSync.native` folds them onto the name the filesystem holds.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { helper } from "./Impl.js";\nexport { other } from "./impl.js";\n',
        "impl.js": "export const helper = 1;\nexport const other = 2;\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.js", "index.js"]);
    assert.equal(closure.notes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a module the analysis read from outside the package is named, not dropped", () => {
  // The third reconciliation direction, and the one that had no note at all: a
  // file the analyzing program opened that the record's own scope excludes. The
  // record may exclude it -- it is not this package's bytes and no hash here
  // pins it -- but it may not be silent about having excluded something the
  // summaries were derived from.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    { files: { "index.js": "export const thing = 1;\n" } }
  );
  writeFileSync(join(directory, "outside.js"), "export const helper = 1;\n");
  try {
    const result = generate({
      packageRoot,
      stub,
      inventory: { extraModules: ["../outside.js"] }
    });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.deepEqual(closure.modules.map(module => module.path), ["index.js"]);
    assert.equal(closure.notes.length, 1);
    assert.match(
      closure.notes[0],
      /^\.\.\/outside\.js: the analyzing program opened this module and it is not inside this package, so the record excludes bytes the summaries were derived from$/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an absent module inventory leaves the record unattested and says so", () => {
  // The fail-closed half. A generation that cannot read the analyzing program's
  // own file list must not present its own walk as the record: it records the
  // walk, names it as unattested, and the entrypoint transfers and verifies
  // nothing. Silently trusting the weaker source is the one outcome this must
  // never have.
  //
  // **This pins a contract, not an observed behavior, and the stub is the only
  // way to reach it.** Against the pinned producer the shape cannot occur: a run
  // that cannot write an inventory exits non-zero and aborts the generation
  // before any contract or plan exists. What the test defends is what must
  // happen the day a producer answers differently -- see `readModuleInventory`,
  // and docs/package-contracts.md, which says the same thing in prose rather
  // than advertising a tier users can see.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "./impl.mjs";\n',
        "impl.mjs": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({ packageRoot, stub, inventory: { absent: true } });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    // The walk's own answer is still recorded -- a reviewer should see which
    // bytes were found -- but it is labelled, not passed off as attested.
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.mjs", "index.js"]);
    assert.equal(closure.notes.length, 1);
    assert.match(
      closure.notes[0],
      /^\.\/index\.js: closure not attested: the analyzing program wrote no module inventory \(.*\)\. The record below is this generator's own syntax walk/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("an incomplete module graph is fail-closed, not reconciled against the walk", () => {
  // `ModuleGraph::is_complete` false means the answer covers fewer files than it
  // asked about. Reconciling a short answer against the walk would let the walk
  // decide the difference, which is the weaker source deciding.
  //
  // Defensive, like the case above, and for a structural reason: the producer
  // builds its import request out of the program's own inventory answer, so the
  // request is always a subset of the holdings and `complete` is always `true`.
  // The stub drives the branch because nothing else can; it pins the contract a
  // future producer must be met with, not a shape this repository has observed.
  const { directory, packageRoot, stub } = makeWorkspace(
    { ".": "./index.js" },
    {
      files: {
        "index.js": 'export { thing } from "./impl.mjs";\n',
        "impl.mjs": "export const thing = 1;\n"
      }
    }
  );
  try {
    const result = generate({
      packageRoot,
      stub,
      inventory: { incomplete: "impl.mjs" }
    });
    assert.equal(result.status, 0, result.stderr);
    const closure = closureOf(packageRoot);
    assert.equal(closure.notes.length, 1);
    assert.match(
      closure.notes[0],
      /closure not attested: the analyzing program reported its resolved module graph incomplete/
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the review plan is bound to the contract bytes generation wrote", () => {
  const { directory, packageRoot, stub } = makeWorkspace({ ".": "./index.mjs" });
  try {
    assert.equal(generate({ packageRoot, stub }).status, 0);
    const contract = join(packageRoot, "solid-reactivity.json");
    const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
    assert.equal(
      plan.contract,
      `sha256:${createHash("sha256").update(readFileSync(contract)).digest("hex")}`
    );
    // And who wrote it, which a transfer requires to be the same on both sides.
    assert.match(plan.generation.generator, /^solid-checker@/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("regenerating over a reviewed contract keeps the previous triple", () => {
  const { directory, packageRoot, stub } = makeWorkspace({ ".": "./index.mjs" });
  const contract = join(packageRoot, "solid-reactivity.json");
  const sibling = suffix => contract.replace(/\.json$/, suffix);
  try {
    assert.equal(generate({ packageRoot, stub }).status, 0);
    // A review state beside the contract is what makes a regeneration
    // destructive: without the snapshot, `--transfer-from` has no old contract
    // and no old plan left to read, so the documented upgrade sequence --
    // regenerate, then transfer -- could not be run at all.
    writeFileSync(
      sibling(".review-state.json"),
      `${JSON.stringify({ schemaVersion: 1, contract: "sha256:x", resolutions: {} }, null, 2)}\n`
    );
    const contractBefore = readFileSync(contract);
    const planBefore = readFileSync(sibling(".review.json"));

    const again = generate({ packageRoot, stub });
    assert.equal(again.status, 0, again.stderr);
    assert.match(again.stdout, /--transfer-from .*solid-reactivity\.previous\.json/);
    assert.deepEqual(readFileSync(sibling(".previous.json")), contractBefore);
    assert.deepEqual(readFileSync(sibling(".previous.review.json")), planBefore);
    assert.equal(existsSync(sibling(".previous.review-state.json")), true);
    assert.equal(existsSync(sibling(".previous.review.md")), true);
    // The fresh triple has no review state: a regenerated contract is
    // unreviewed until something is transferred onto it or decided about it.
    assert.equal(existsSync(sibling(".review-state.json")), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a contract with no review state beside it is regenerated in place", () => {
  const { directory, packageRoot, stub } = makeWorkspace({ ".": "./index.mjs" });
  const contract = join(packageRoot, "solid-reactivity.json");
  try {
    assert.equal(generate({ packageRoot, stub }).status, 0);
    const again = generate({ packageRoot, stub });
    assert.equal(again.status, 0, again.stderr);
    // Nothing to carry, and a `.previous` pair with no review beside it would
    // be a transfer source that looks reviewed and is not.
    assert.equal(existsSync(contract.replace(/\.json$/, ".previous.json")), false);
    assert.equal(again.stdout.trim().split(/\r?\n/).length, 1, again.stdout);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("--missing=VALUE reaches the sweep's own rejection", () => {
  const { directory, project, stub } = makeSweepWorkspace({ alpha: { ".": "./index.mjs" } });
  try {
    // The router matched `--missing` exactly, so any `=` spelling fell through
    // to the single-package parser and came back as "unknown contract
    // generation argument" -- and the sweep's own "takes no value" message,
    // the one that says what the flag means, could never be reached.
    const result = sweep({ project, stub, args: ["--missing=1"] });
    assert.equal(result.status, 2, result.stdout);
    assert.match(result.stderr, /--missing takes no value/);
    assert.doesNotMatch(result.stderr, /unknown contract generation argument/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a sweep failure keeps its whole message in the report", () => {
  const { directory, project, stub } = makeSweepWorkspace({ beta: { ".": "./index.mjs" } });
  try {
    const result = sweep({
      project,
      stub,
      plan: { beta: "crash" },
      args: ["--missing", "--format", "json"],
      report: {
        packages: [{ name: "beta", status: "missing", remedy: "generate a contract", contractPath: "" }]
      }
    });
    assert.equal(result.status, 1, result.stdout);
    const report = JSON.parse(result.stdout);
    // A panic's useful part is on the lines after the first. Truncating the
    // report as well as stderr left CI with no record of why a package failed.
    assert.match(report.failed[0].reason, /panicked/);
    assert.match(report.failed[0].reason, /internal invariant violated/);
    assert.ok(report.failed[0].reason.includes("\n"));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
