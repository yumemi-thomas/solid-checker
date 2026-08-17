// The gate for AGENTS.md's absolute rule: a rule's positive case must not also
// be a `tsc` error.
//
// An absolute rule with no gate is a comment. This one compiles every case in
// `fixtures/tsc-oracle/rule-cases.json` against the *real published* Solid
// typings and enforces the case's declared expectation, so a new rule that
// duplicates a TypeScript diagnostic cannot be merged, and a removal that was
// justified by a diagnostic cannot silently outlive it.
//
//   node scripts/tsc-oracle-gate.mjs           enforce every case
//   node scripts/tsc-oracle-gate.mjs --json    machine-readable result
//   node scripts/tsc-oracle-gate.mjs --report  print the oracle's actual output
//                                              per case (for writing a ledger
//                                              entry, not for gating)
//
// The three expectations are documented in the ledger file itself. The escape
// hatch is `distinct-claim`, and it is deliberately noisy: it demands the
// diagnostic codes *and* a written reason naming what the finding asserts that
// the type error does not. There is no silent skip -- an unprovisioned oracle
// fails loudly, the same way `SOLID_TYPEFACTS_BIN`'s canary does.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { runOracle } from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const LEDGER = join(ROOT, "fixtures/tsc-oracle/rule-cases.json");

const json = process.argv.includes("--json");
const report = process.argv.includes("--report");
const ledger = JSON.parse(readFileSync(LEDGER, "utf8"));

const EXPECTATIONS = new Set(Object.keys(ledger.expectations));

// Both passes matter. "Only under `strict`" is not an exception the absolute
// rule recognises, so a case is redundant if *either* pass reports it; a case
// claiming silence has to be silent in both.
const passes = (result) => [
  ["strict", result.passes.strict],
  ["loose", result.passes.loose],
];

const errorsOnly = (diagnostics) => diagnostics.filter((d) => d.category === "error");

const failures = [];
const results = [];

for (const [index, testCase] of ledger.cases.entries()) {
  const label = `${testCase.rule} [${index}]`;
  if (!EXPECTATIONS.has(testCase.expect)) {
    failures.push(`${label}: unknown expectation ${JSON.stringify(testCase.expect)}`);
    continue;
  }
  if (!testCase.why || testCase.why.length < 20) {
    failures.push(`${label}: every case needs a written 'why'`);
    continue;
  }
  const result = runOracle(testCase.dialect, [
    { name: `${testCase.rule.replace(/[^a-z0-9]+/gi, "-")}-${index}.tsx`, code: testCase.code },
  ]);
  const perPass = passes(result).map(([name, diagnostics]) => [name, errorsOnly(diagnostics)]);
  const seen = [...new Set(perPass.flatMap(([, diagnostics]) => diagnostics.map((d) => d.code)))].sort(
    (a, b) => a - b,
  );
  results.push({ rule: testCase.rule, index, expect: testCase.expect, codes: seen, perPass });

  if (testCase.expect === "silent") {
    for (const [name, diagnostics] of perPass) {
      if (diagnostics.length) {
        failures.push(
          `${label}: expected silence but the ${name} pass reported ` +
            diagnostics.map((d) => `TS${d.code} at ${d.line}:${d.column} (${d.message})`).join("; ") +
            `\n    This rule now duplicates a TypeScript diagnostic. Narrow it to the uncovered` +
            ` spellings or delete it -- see AGENTS.md's absolute rule.`,
        );
      }
    }
    continue;
  }

  // The remaining expectations all require a diagnostic, and require it to be
  // one the ledger already accounted for. A *new* code means the case changed
  // meaning, which has to be re-explained rather than absorbed.
  const allow = new Set(testCase.allow ?? []);
  if (!allow.size) {
    failures.push(`${label}: ${testCase.expect} requires 'allow' listing the diagnostic codes`);
    continue;
  }
  if (!seen.length) {
    failures.push(
      `${label}: expected TypeScript to report ${[...allow].map((c) => `TS${c}`).join("/")} but it was silent.` +
        {
          "removed-because-redundant":
            `\n    The removal of this rule was justified by that diagnostic. It is gone, so the` +
            ` defect is now unreported by anyone -- reconsider the removal.`,
          "redundant-pending-narrowing":
            `\n    This case was recorded as a duplicate awaiting narrowing. TypeScript no longer` +
            ` speaks, so the duplicate is gone and the debt entry should be closed.`,
          "distinct-claim": `\n    The 'distinct-claim' allowance no longer describes anything.`,
        }[testCase.expect],
    );
    continue;
  }
  const unexpected = seen.filter((code) => !allow.has(code));
  if (unexpected.length) {
    failures.push(
      `${label}: unexpected ${unexpected.map((c) => `TS${c}`).join("/")} (declared: ` +
        `${[...allow].map((c) => `TS${c}`).join("/")}); re-explain the case`,
    );
  }
}

// The mechanism that makes the rule permanent rather than documented: a catalog
// rule with no case here cannot be judged, so the gate refuses to let one exist
// silently. A new rule must arrive with its positive spelling and the oracle's
// verdict on it.
const EXEMPT = {
  "package-contract-export-missing": "asks whether a package ships a usable reactivity contract, which is an analyzability fact about an external artifact; no snippet against real Solid typings can express it",
  "package-contract-callback-missing": "same -- the subject is a third-party package's contract, not Solid's types",
  "package-contract-missing": "same",
  "v1/package-contract-export-missing": "same",
  "v1/package-contract-callback-missing": "same",
  "v1/package-contract-missing": "same",
  "execution-map-incomplete": "unreachable from real source by construction (docs/precision-backlog.md): it defends against externally produced or partial compiler facts, so no snippet can trigger it",
  "v1/execution-map-incomplete": "same",
  "ssr-client-source-outside-loading-boundary": "needs a server-rendering project, not a snippet: the finding depends on the project rendering mode rather than on any type",
  "http-response-after-flush": "same -- needs a server-rendering project",
  "server-function-module-directive": "needs a module-level \"use server\" prologue and the project's server surface",
  "server-function-rich-argument": "same, plus a project-wide argument-serializer search",
  "pending-async-forbidden-scope": "needs a pending-state observer in a project with a Loading boundary above it",
  "v1/jsx-uses-vars": "no diagnostic of its own: it marks a JSX-referenced binding used so an unused-variable pass does not flag it",
};

const catalogRules = [
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v1.json"), "utf8")).rules,
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v2.json"), "utf8")).rules,
].map((rule) => rule.name);
const declared = new Set(ledger.cases.map((testCase) => testCase.rule));
const uncovered = [...new Set(catalogRules)].filter(
  (name) =>
    !declared.has(name) &&
    !declared.has(name.replace(/^v1\//, "")) &&
    !(name in EXEMPT) &&
    !(name.replace(/^v1\//, "") in EXEMPT),
);
for (const name of uncovered) {
  failures.push(
    `${name}: no case in fixtures/tsc-oracle/rule-cases.json.` +
      `\n    Every catalog rule needs its positive spelling here so the oracle can answer` +
      ` "does TypeScript already report this?". Add a case, or an EXEMPT entry in this` +
      ` script saying why no snippet can express the rule's subject.`,
  );
}

if (report) {
  for (const result of results) {
    console.log(`\n=== ${result.rule} [${result.index}] expect=${result.expect}`);
    for (const [name, diagnostics] of result.perPass) {
      console.log(`  ${name}: ${diagnostics.length || "clean"}`);
      for (const d of diagnostics) {
        console.log(`    TS${d.code} ${d.line}:${d.column} [${d.startByte}..${d.endByte}] ${d.message}`);
      }
    }
  }
}

if (json) {
  console.log(
    JSON.stringify(
      { cases: results.map(({ perPass, ...rest }) => rest), failures },
      null,
      2,
    ),
  );
} else if (failures.length) {
  console.error(`tsc oracle gate: ${failures.length} case(s) failed\n`);
  for (const failure of failures) console.error(`  - ${failure}\n`);
} else {
  console.log(`tsc oracle gate: ${ledger.cases.length} case(s) hold`);
}

process.exit(failures.length ? 1 : 0);
