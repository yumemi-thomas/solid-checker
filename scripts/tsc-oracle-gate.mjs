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

  // Both remaining expectations require a diagnostic, and require it to be one
  // the ledger already accounted for. A *new* code means the case changed
  // meaning, which has to be re-explained rather than absorbed.
  const allow = new Set(testCase.allow ?? []);
  if (!allow.size) {
    failures.push(`${label}: ${testCase.expect} requires 'allow' listing the diagnostic codes`);
    continue;
  }
  if (!seen.length) {
    failures.push(
      `${label}: expected TypeScript to report ${[...allow].map((c) => `TS${c}`).join("/")} but it was silent.` +
        (testCase.expect === "removed-because-redundant"
          ? `\n    The removal of this rule was justified by that diagnostic. It is gone, so the` +
            ` defect is now unreported by anyone -- reconsider the removal.`
          : `\n    The 'distinct-claim' allowance no longer describes anything.`),
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
