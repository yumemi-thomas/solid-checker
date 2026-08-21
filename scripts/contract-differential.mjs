#!/usr/bin/env node

// Differential contract audit:
//   1. analyze a package implementation as project source;
//   2. generate its contract;
//   3. analyze the same consumer against only the published runtime artifact,
//      declarations, and generated contract;
//   4. compare the semantic findings at the consumer call site.
//
// This is deliberately a small, executable parity probe rather than a second
// contract implementation. A new contract claim must survive this boundary,
// or the probe fails with the source/consumer outcomes side by side.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const checker = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const auditedSolid = join(root, "rust/target/tsc-oracle/v2/node_modules/solid-js");

if (!existsSync(checker) || !existsSync(typeFacts) || !existsSync(auditedSolid)) {
  throw new Error(
    `contract differential needs checker, TypeFacts, and the provisioned v2 solid-js package (${checker}, ${typeFacts}, ${auditedSolid})`
  );
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function writeJson(path, value) {
  write(path, `${JSON.stringify(value, null, 2)}\n`);
}

function linkSolid(projectRoot) {
  mkdirSync(join(projectRoot, "node_modules"), { recursive: true });
  symlinkSync(auditedSolid, join(projectRoot, "node_modules/solid-js"), "dir");
}

function analyze(project) {
  const output = execFileSync(
    checker,
    ["--format", "json", "--project", join(project, "tsconfig.json")],
    {
      cwd: root,
      encoding: "utf8",
      env: { ...process.env, SOLID_TYPEFACTS_BIN: typeFacts },
      maxBuffer: 32 * 1024 * 1024
    }
  );
  const snapshot = JSON.parse(output);
  return (snapshot.findings ?? [])
    .map(finding => ({
      rule: finding.rule,
      id: finding.id,
      kind: finding.kind,
      severity: finding.severity,
      analysisContext: finding.analysisContext ?? null
    }))
    .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function generateContract(packageRoot, output) {
  execFileSync(
    process.execPath,
    [
      join(root, "packages/cli/bin/solid-checker.mjs"),
      "contract",
      "generate",
      "--package-root",
      packageRoot,
      "--output",
      output
    ],
    {
      cwd: root,
      env: {
        ...process.env,
        SOLID_CHECKER_NATIVE_BIN: checker,
        SOLID_TYPEFACTS_BIN: typeFacts
      },
      stdio: "pipe"
    }
  );
}

function createSourceProject(directory) {
  linkSolid(directory);
  writeJson(join(directory, "tsconfig.json"), {
    compilerOptions: {
      allowJs: true,
      checkJs: true,
      jsx: "preserve",
      module: "ESNext",
      moduleResolution: "Bundler",
      skipLibCheck: true,
      strict: true,
      target: "ES2022"
    },
    include: ["*.ts", "*.tsx"]
  });
  write(
    join(directory, "implementation.ts"),
    `import { createEffect } from "solid-js";\n\nexport function runMixed(callback: () => unknown) {\n  callback();\n  queueMicrotask(callback);\n}\n\nfunction ownedEffectImplementation() {\n  createEffect(() => 0, () => {});\n}\nexport { ownedEffectImplementation as ownedEffect };\n`
  );
  write(
    join(directory, "implementation.js"),
    `export function runMixed(callback) {\n  callback();\n  queueMicrotask(callback);\n}\n`
  );
  write(
    join(directory, "App.tsx"),
    `import { createSignal } from "solid-js";\nimport { ownedEffect, runMixed } from "./implementation";\n\nconst [count] = createSignal(0);\nfunction readCount() {\n  return count();\n}\n\nownedEffect();\nrunMixed(readCount);\n`
  );
}

function createContractProject(directory, contract) {
  linkSolid(directory);
  const packageRoot = join(directory, "node_modules/differential-package");
  mkdirSync(packageRoot, { recursive: true });
  writeJson(join(packageRoot, "package.json"), {
    name: "differential-package",
    version: "1.0.0",
    type: "module",
    exports: "./index.js",
    types: "./index.d.ts"
  });
  write(
    join(packageRoot, "index.js"),
    `import { createEffect } from "solid-js";\n\nexport function runMixed(callback) {\n  callback();\n  queueMicrotask(callback);\n}\n\nfunction ownedEffectImplementation() {\n  createEffect(() => 0, () => {});\n}\nexport { ownedEffectImplementation as ownedEffect };\n`
  );
  write(
    join(packageRoot, "index.d.ts"),
    "export declare function runMixed(callback: () => unknown): void;\nexport declare function ownedEffect(): void;\n"
  );
  writeJson(join(packageRoot, "solid-reactivity.json"), contract);
  writeJson(join(directory, "tsconfig.json"), {
    compilerOptions: {
      jsx: "preserve",
      module: "ESNext",
      moduleResolution: "Bundler",
      skipLibCheck: true,
      strict: true,
      target: "ES2022"
    },
    files: ["App.tsx"]
  });
  write(
    join(directory, "App.tsx"),
    `import { createSignal } from "solid-js";\nimport { ownedEffect, runMixed } from "differential-package";\n\nconst [count] = createSignal(0);\nfunction readCount() {\n  return count();\n}\n\nownedEffect();\nrunMixed(readCount);\n`
  );
}

function promoteReviewed(contract) {
  const reviewed = structuredClone(contract);
  reviewed.evidence = { ...(reviewed.evidence ?? {}), kind: "reviewed" };
  const visit = summary => {
    if (summary.evidence) summary.evidence = { ...summary.evidence, kind: "reviewed" };
    for (const callback of summary.callbacks ?? []) {
      if (callback.evidence) callback.evidence = { ...callback.evidence, kind: "reviewed" };
    }
    for (const requirement of summary.ownerRequirements ?? []) {
      requirement.evidence = { kind: "reviewed" };
    }
    if (summary.returns) visitReturn(summary.returns);
    for (const variant of summary.variants ?? []) visit(variant.summary);
  };
  const visitReturn = returned => {
    if (returned.evidence) returned.evidence = { ...returned.evidence, kind: "reviewed" };
    for (const element of returned.elements ?? []) if (element) visitReturn(element);
    for (const property of Object.values(returned.properties ?? {})) visitReturn(property);
  };
  // Normalized contracts store summaries once at the document root;
  // entrypoint exports contain summary-id -> export-name arrays. Visiting
  // those arrays left every generated row inferred while the harness claimed
  // to have promoted it.
  for (const summary of Object.values(reviewed.summaries ?? {})) visit(summary);
  return reviewed;
}

const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-differential-"));
try {
  const source = join(temporary, "source");
  const consumer = join(temporary, "consumer");
  const packageRoot = join(temporary, "package-artifact");
  const contractPath = join(temporary, "solid-reactivity.json");
  writeJson(join(packageRoot, "package.json"), {
    name: "differential-package",
    version: "1.0.0",
    type: "module",
    exports: "./index.js",
    dependencies: { "solid-js": "2.0.0-rc.0" }
  });
  linkSolid(packageRoot);
  write(
    join(packageRoot, "index.js"),
    `import { createEffect } from "solid-js";\n\nexport function runMixed(callback) {\n  callback();\n  queueMicrotask(callback);\n}\n\nfunction ownedEffectImplementation() {\n  createEffect(() => 0, () => {});\n}\nexport { ownedEffectImplementation as ownedEffect };\n\nexport default function () {\n  createEffect(() => 0, () => {});\n}\n`
  );
  createSourceProject(source);
  generateContract(packageRoot, contractPath);
  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  assert.deepEqual(
    contract.summaries?.["function-1"]?.callbacks,
    [
      { parameter: 0, execution: "inline", evidence: { kind: "inferred" } },
      { parameter: 0, execution: "deferred", evidence: { kind: "inferred" } }
    ],
    "the generated contract must retain both runtime callback paths"
  );
  const ownedEffectSummary = Object.entries(contract.entrypoints?.["."]?.exports ?? {})
    .find(([, names]) => names.includes("ownedEffect"))?.[0];
  assert.ok(ownedEffectSummary, "the generated contract must retain ownedEffect");
  assert.deepEqual(
    contract.summaries?.[ownedEffectSummary]?.ownerRequirements?.map(row => row.operation),
    ["effect"],
    "the generated contract must retain the exported owner requirement"
  );
  const defaultSummary = Object.entries(contract.entrypoints?.["."]?.exports ?? {})
    .find(([, names]) => names.includes("default"))?.[0];
  assert.ok(defaultSummary, "the generated contract must retain the anonymous default export");
  assert.deepEqual(
    contract.summaries?.[defaultSummary]?.ownerRequirements?.map(row => row.operation),
    ["effect"],
    "the generated contract must attach the anonymous default owner requirement by exact export identity"
  );
  createContractProject(consumer, promoteReviewed(contract));

  const sourceFindings = analyze(source);
  const consumerFindings = analyze(consumer);
  assert.deepEqual(
    consumerFindings,
    sourceFindings,
    `contract boundary changed semantic findings\nsource: ${JSON.stringify(sourceFindings)}\nconsumer: ${JSON.stringify(consumerFindings)}`
  );
  console.log(
    `contract differential: source and reviewed-contract consumers agree (${sourceFindings.length} finding${sourceFindings.length === 1 ? "" : "s"})`
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
