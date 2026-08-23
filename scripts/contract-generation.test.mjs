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

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");

// One stub per behavior, keyed by the entry file the generator hands it:
// "ok" writes a minimal normalized contract document, "refuse" reproduces a
// native fail-closed contract-emission refusal, "crash" reproduces a panic.
const STUB_NATIVE = `#!/usr/bin/env node
import { appendFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";

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
    ["missing", "unverified", "stale"].includes(entry.status)
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

function generate({ packageRoot, stub, plan = {}, args = [], dependencyModule, argvLog }) {
  return spawnSync(process.execPath, [cli, "contract", "generate", ...args], {
    cwd: packageRoot,
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: stub,
      STUB_NATIVE_PLAN: JSON.stringify(plan),
      ...(dependencyModule ? { STUB_DEPENDENCY_MODULE: dependencyModule } : {}),
      ...(argvLog ? { STUB_ARGV_LOG: argvLog } : {})
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
    assert.match(review, /## entrypoints the generator refused/);
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
    assert.match(
      review,
      /- \[ \] contract is byte-bound to its entry artifact only: \.\/index\.mjs pulls in 1 further runtime module\(s\)/
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

// What the closure walker records, and what it refuses to leave unsaid.
//
// The walk decides three things at once: the TypeScript project's `files` list,
// the unpinned-module count on the review plan, and the per-entrypoint hash set
// a review transfers against. A module it misses is not a smaller closure but a
// false one -- the hash set says "these are the bytes the summaries came from"
// while the file that produced them sits outside it. Each shape below silently
// produced that record before.

function closureOf(packageRoot, entrypoint = ".") {
  const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
  return plan.generation.entrypoints[entrypoint];
}

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
    assert.match(review, /pulls in 1 further runtime module\(s\)/);
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

test("an unresolvable #imports branch is noted rather than guessed", () => {
  // Two conditional targets and no selection: picking one would put a browser
  // build's bytes behind a node build's summaries.
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
    assert.equal(closure.notes.length, 1);
    assert.match(closure.notes[0], /closure could not be fully enumerated: #internal resolves to 2 conditional targets/);

    // A note is not only a machine fact: it is the difference between "the
    // contract is bound to its entry artifact" and "bound to less than that",
    // and the artifact-binding section is where a reviewer is told which.
    const review = readFileSync(join(packageRoot, "solid-reactivity.review.md"), "utf8");
    assert.match(review, /## contract artifact binding/);
    assert.match(review, /- \[ \] \. index\.js: closure could not be fully enumerated: #internal/);
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

test("a relative specifier that resolves to nothing is noted, never dropped", () => {
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
    assert.equal(closure.notes.length, 2);
    assert.ok(
      closure.notes.some(note => /\.\/gone\.js names no runtime module inside the package/.test(note))
    );
    assert.ok(
      closure.notes.some(note =>
        /a dynamic import\(\) whose specifier is not a string literal/.test(note)
      )
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
