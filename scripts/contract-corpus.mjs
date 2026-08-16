#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
const defaultNative = join(root, "rust/target/debug/solid-checker-rust");
const defaultTypeFacts = join(root, "bin/solid-typefacts");
const fixtures = [
  "torture-runtime-namespace",
  "torture-conditional-semantics",
  "torture-getter-exports",
  "torture-deep-barrel",
  "torture-dts-disagreement"
];

const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? defaultNative;
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? defaultTypeFacts;
if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    `contract corpus needs SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN (missing ${
      !existsSync(native) ? native : typeFacts
    })`
  );
}

const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-corpus-"));
const coverage = join(temporary, "coverage");
const expectedGenerator = pathToFileURL(
  join(root, "packages/cli/scripts/generate-package-contract.mjs")
).href;
const generated = [];

function runFixture(name) {
  const packageRoot = join(root, "fixtures/package-contracts", name);
  const output = join(temporary, `${name}.json`);
  const result = spawnSync(
    process.execPath,
    [cli, "contract", "generate", "--package-root", packageRoot, "--output", output],
    {
      cwd: root,
      env: {
        ...process.env,
        SOLID_CHECKER_NATIVE_BIN: native,
        SOLID_TYPEFACTS_BIN: typeFacts,
        NODE_V8_COVERAGE: coverage
      },
      encoding: "utf8"
    }
  );
  if (result.status !== 0) {
    throw new Error(
      `${name} generation failed:\n${result.stdout}\n${result.stderr}`.trim()
    );
  }
  const expectedPath = join(packageRoot, "expected.json");
  const actual = JSON.parse(readFileSync(output, "utf8"));
  const expected = JSON.parse(readFileSync(expectedPath, "utf8"));
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `${name} drifted from ${expectedPath}; review the generated contract before updating the pin`
    );
  }
  generated.push(name);
}

function lineAt(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

function generatorCoverage() {
  const source = readFileSync(fileURLToPath(expectedGenerator), "utf8");
  const functions = new Map();
  for (const file of readdirSync(coverage)) {
    if (!file.endsWith(".json")) continue;
    const document = JSON.parse(readFileSync(join(coverage, file), "utf8"));
    for (const script of document.result ?? []) {
      if (script.url !== expectedGenerator) continue;
      for (const entry of script.functions ?? []) {
        const range = entry.ranges?.[0];
        if (!range) continue;
        const current = functions.get(entry.functionName) ?? {
          count: 0,
          line: lineAt(source, range.startOffset),
          ranges: []
        };
        current.count += range.count;
        current.ranges.push(range);
        functions.set(entry.functionName, current);
      }
    }
  }
  const claimEmitters = [
    "mergeSummaries",
    "annotateReturnEvidence",
    "annotateClaimEvidence",
    "analyzeTarget",
    "generatePackageContract"
  ];
  const uncovered = claimEmitters.filter(name => !functions.has(name) || functions.get(name).count === 0);
  if (uncovered.length) {
    throw new Error(
      `contract corpus does not execute claim-emitting generator functions: ${uncovered.join(", ")}`
    );
  }
  const zeroRanges = new Set([...functions.entries()]
    .flatMap(([name, value]) =>
      value.ranges
        .filter(range => range.count === 0)
        .map(range => `${name}@${lineAt(source, range.startOffset)}`)
    )
    .filter(item => !item.startsWith("@"))
    .sort());
  return { functions: functions.size, uncoveredRanges: [...zeroRanges] };
}

try {
  for (const fixture of fixtures) runFixture(fixture);
  const coverageResult = generatorCoverage();
  const uncovered = coverageResult.uncoveredRanges.length;
  console.log(
    `contract corpus: ${generated.length} packages, ${uncovered} uncovered generator ranges`
  );
  if (uncovered) console.log(`uncovered ranges: ${coverageResult.uncoveredRanges.join(", ")}`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
