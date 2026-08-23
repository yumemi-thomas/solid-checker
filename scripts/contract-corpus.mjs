#!/usr/bin/env node

import { execFile } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

import { ancestorChainDigest, hashTree, openGateCache } from "./lib/gate-cache.mjs";
import { gateConcurrency, mapPool } from "./lib/pool.mjs";

const run = promisify(execFile);
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
  "uncaptured-source-return",
  // Pins that a local callee's summary is only inheritable where it accounts
  // for the parameter -- a callee that *retains* the value opens the sentinel
  // for everything forwarding into it -- and the precision half: an observed
  // value and a caller-owned container stay the honest omission.
  "retained-callback-parameter",
  // Pins that an exported class is `kind: "function"` through all three
  // resolution shapes, with its callbacks domain fail-closed, while a real
  // non-callable value stays `kind: "value"`.
  "exported-class"
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

// One coverage directory per fixture, not one shared by all of them.
//
// V8 already writes one file per process, so a shared directory is safe to
// write concurrently -- but it makes the dumps unattributable, and attribution
// is what the cache needs: a replayed fixture must contribute the coverage
// *that fixture* produced. Per-fixture directories give that for free and cost
// nothing.
const fixtureCoverage = name => join(coverage, name);

/**
 * The V8 coverage this fixture's processes contributed to the generator.
 *
 * Reduced to the one range per function that `generatorCoverage` consumes, so
 * a cache entry stores a few hundred bytes rather than a full dump.
 */
function collectFixtureCoverage(name) {
  const directory = fixtureCoverage(name);
  const contributions = [];
  if (!existsSync(directory)) return contributions;
  for (const file of readdirSync(directory)) {
    if (!file.endsWith(".json")) continue;
    const document = JSON.parse(readFileSync(join(directory, file), "utf8"));
    for (const script of document.result ?? []) {
      if (script.url !== expectedGenerator) continue;
      for (const entry of script.functions ?? []) {
        const range = entry.ranges?.[0];
        if (!range) continue;
        contributions.push({
          functionName: entry.functionName,
          startOffset: range.startOffset,
          endOffset: range.endOffset,
          count: range.count
        });
      }
    }
  }
  return contributions;
}

async function generate(name) {
  const packageRoot = join(root, "fixtures/package-contracts", name);
  const output = join(temporary, `${name}.json`);
  const directory = fixtureCoverage(name);
  mkdirSync(directory, { recursive: true });
  // A non-zero exit throws, so nothing is cached for a fixture whose
  // generation crashed -- the cache never learns a result the run did not
  // actually produce.
  try {
    await run(
      process.execPath,
      [cli, "contract", "generate", "--package-root", packageRoot, "--output", output],
      {
        cwd: root,
        env: {
          ...process.env,
          SOLID_CHECKER_NATIVE_BIN: native,
          SOLID_TYPEFACTS_BIN: typeFacts,
          NODE_V8_COVERAGE: directory
        },
        encoding: "utf8",
        maxBuffer: 256 * 1024 * 1024
      }
    );
  } catch (error) {
    throw new Error(
      `${name} generation failed:\n${error.stdout ?? ""}\n${error.stderr ?? error.message}`.trim()
    );
  }
  return {
    contract: JSON.parse(readFileSync(output, "utf8")),
    coverage: collectFixtureCoverage(name)
  };
}

/** Compares one fixture's generated contract against its checked-in pin. */
function compare(name, contract) {
  const expectedPath = join(root, "fixtures/package-contracts", name, "expected.json");
  const expected = JSON.parse(readFileSync(expectedPath, "utf8"));
  if (JSON.stringify(contract) !== JSON.stringify(expected)) {
    throw new Error(
      `${name} drifted from ${expectedPath}; review the generated contract before updating the pin`
    );
  }
}

function lineAt(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

/**
 * The generator coverage the corpus achieved, over live and replayed runs alike.
 *
 * Why unioning a cache entry's recorded coverage with this run's live coverage
 * is sound: the cache key includes the content digest of the whole
 * `packages/cli` tree, which is where the generator lives. A replayable entry
 * is therefore an entry produced by *these exact generator bytes*, so "this
 * function executed" is a claim about the same function at the same offsets.
 * Change the generator and every entry's key changes, so no recorded coverage
 * can outlive the code it attests. The assertion below then holds over the
 * union, which is exactly the set of functions the corpus executes -- a warm
 * cache cannot turn an uncovered claim emitter green, and cannot turn a covered
 * one red either.
 */
function generatorCoverage(contributions) {
  const source = readFileSync(fileURLToPath(expectedGenerator), "utf8");
  const functions = new Map();
  for (const entry of contributions) {
    const range = {
      startOffset: entry.startOffset,
      endOffset: entry.endOffset,
      count: entry.count
    };
    const current = functions.get(entry.functionName) ?? {
      count: 0,
      line: lineAt(source, range.startOffset),
      ranges: []
    };
    current.count += range.count;
    current.ranges.push(range);
    functions.set(entry.functionName, current);
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

// Each fixture is a self-contained package directory the generator reads and
// nothing writes to, so the fixtures are independent -- and a fixture's
// generated contract is a function of exactly its tree, the dialect-selection
// chain above it (the checker walks ancestors for the nearest
// `node_modules/solid-js`, exactly as coverage's key comment explains), the
// CLI that generates it, and the two binaries underneath. That is the cache key.
const cache = openGateCache({
  gate: "contract-corpus",
  scriptPath: import.meta.filename,
  binaries: [native, typeFacts, `${typeFacts}.buildinfo`],
  trees: [join(root, "packages/cli")]
});
const concurrency = gateConcurrency();

try {
  const computed = await mapPool(
    fixtures,
    fixture =>
      // A thunk, not an array: the tree digest is of mutable state, so the
      // cache re-evaluates it after `generate` has read the fixture and refuses
      // to store a unit whose tree moved mid-run. See `openGateCache().run`.
      cache.run(
        () => [
          `fixture:${fixture}`,
          hashTree(join(root, "fixtures/package-contracts", fixture)),
          ancestorChainDigest(
            join(root, "fixtures/package-contracts", fixture),
            "node_modules/solid-js/package.json"
          )
        ],
        () => generate(fixture)
      ),
    { concurrency }
  );
  // Comparison against the checked-in pin runs fresh, in fixture order,
  // whichever fixtures were replayed: `expected.json` is never in the key.
  const generated = [];
  const contributions = [];
  for (const [index, fixture] of fixtures.entries()) {
    compare(fixture, computed[index].value.contract);
    contributions.push(...computed[index].value.coverage);
    generated.push(fixture);
  }
  const coverageResult = generatorCoverage(contributions);
  const uncovered = coverageResult.uncoveredRanges.length;
  console.log(
    `contract corpus: ${generated.length} packages, ${uncovered} uncovered generator ranges`
  );
  if (uncovered) console.log(`uncovered ranges: ${coverageResult.uncoveredRanges.join(", ")}`);
  console.log(`${cache.summary()}; concurrency ${concurrency}`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
