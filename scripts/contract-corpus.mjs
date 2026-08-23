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
  "torture-dts-disagreement",
  // Pins that the generator passes the analyzed target's export-map conditions
  // to the native checker. Its dependency contract advertises host conditions,
  // so it resolves only when a condition is selected -- suppress the
  // propagation and this fixture fails with
  // `PackageContractEnvironmentDependent` instead of generating.
  "torture-environment-conditions",
  // Pins that a callback forwarded into a callee with no resolvable identity
  // becomes an explicit unknown claim. Silence here is a negative claim
  // ("never invoked"), so a regression is invisible in the contract itself.
  "unresolved-callee-callback",
  // Pins that an export present in only one conditional branch keeps its
  // variant instead of being republished as an unconditional summary.
  "conditional-export-absence",
  // Pins `mergeSummaries`, which was uncovered by the corpus and wrong in the
  // one-sided direction: a branch that *proved* a return merged against a
  // branch that proved none handed the base the proving branch's claim, which
  // is false in the other environment. The pair pins both shapes -- one-sided
  // presence and two branches proving different values -- because collapsing
  // the merge rule back breaks exactly one of them.
  "conditional-returns-divergence",
  "conditional-returns-divergence-both",
  // Pins legacy root resolution when `module` and `main` name different
  // artifacts: the ESM build is analyzed and the review plan records which
  // field it came from.
  "legacy-dual-root",
  // Pins the unknown-claim attribution ladder and the claim domains an
  // unresolved dispatch actually invalidates. Both used to be wrong in the
  // same direction: every export of the entrypoint, every domain.
  "unresolved-dispatch-attribution",
  "unresolved-dispatch-domains-control",
  // Pins the call-graph rung: an obligation in a private helper belongs to the
  // exports that reach it, and to no others.
  "unresolved-dispatch-reachability",
  // Pins the one obligation class that keeps every claim domain -- a contract
  // with no summary for the export behind the call -- and the exact-symbol
  // attribution that replaced a callee name-text scan.
  "unresolved-contract-export-attribution",
  // Pins that an arrow-bound export is nameable at every rung. Reading only
  // `name`/`method_name` made `export const X = () => {}` unnameable, and the
  // reachability rung read that as "not an export" and marked nothing.
  "arrow-export-attribution",
  // Pins the escape test. Accepting any reference inside an export
  // declaration's span accepted `apply(Panel)`, `return Panel` and `<Panel/>`
  // as export surface, so a value-escaped helper kept a "complete" caller
  // enumeration and every export beside the caller published as certified.
  "escaping-private-helper",
  // Pins the identity join: a private helper never inherits an unrelated
  // same-named export's claim, and an aliased pair is marked as one.
  "export-identity-join",
  // Pins where the `parameter-member` row does and does not discharge the
  // exported-helper obligation -- the row is published by the helper, not by
  // an export one hop above it.
  "parameter-member-forwarded",
  // Pins the declaration-file identity split. A `.d.ts` beside an internal
  // runtime module makes every importer bind to the declaration, so the
  // implementation's caller edges vanish while the graph still reported
  // `complete` -- and every export that reaches the obligation was published
  // certified. The enumeration now reports itself incomplete instead.
  "declaration-sibling-reach",
  // The same shape with identity intact: one entry file that both re-exports
  // and calls the helper must resolve the obligation to both published names
  // and leave the third export certified.
  "entry-reexport-identity",
  // Pins that `execution: "inline"` is written only for an invocation proven
  // in the declaring function's own body; a callback reached through a closure
  // handed elsewhere or returned opens the sentinel instead.
  "callback-execution-boundary",
  // Pins that an obligation the ladder resolves to no export leaves a
  // review-plan note. The contract is identical either way, so a silent
  // narrowing is invisible in the bytes.
  "unreached-private-obligation",
  // Pins the ReactiveSourceUncaptured arm's domains, and that they are today
  // masked by the missing-contract-export obligation on the same call.
  "uncaptured-source-return"
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
