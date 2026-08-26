// The gate for the other half of the precision contract.
//
// `tsc-oracle-gate.mjs` holds a *reported* finding to being this checker's
// claim rather than TypeScript's. Nothing held an *unreported* one to anything.
// An uncertifiable finding says "the evidence that would settle this is not
// here" -- and that is a falsifiable claim, because you can supply the evidence
// and look. Until this gate existed, nobody did, and three separate
// over-conservatisms survived a 76-project corpus precisely because supplying
// the evidence was a thing only a human ever did by hand:
//
//   * an exported component's in-project dynamic caller was discarded, so a
//     proven violation was reported as an obligation;
//   * a read sealed inside a tracked callback was attributed to the callers of
//     the function containing it, which is a *false violation*;
//   * a Date nested in an object literal argument was shrugged at, because the
//     library-identity fact was never demanded below the top level.
//
// Each case here therefore states an obligation and then, for every way the
// obligation can be discharged, the evidence and what the checker must say
// once it is present. A case whose obligation quietly closes on its own fails
// too: that is a real change in behaviour and it should be recorded, not
// absorbed.
//
//   bun scripts/obligation-audit.mjs           enforce every case
//   bun scripts/obligation-audit.mjs --json    machine-readable result
//
// Cases run against the *audited published typings*, in the oracle's own
// provisioned installs, for the reason AGENTS.md gives: a fixture stub that is
// looser than the real package manufactures defects no real project can
// produce. An obligation that only exists against a loosened stub is not an
// obligation.
//
// There is no silent skip. A missing binary or an unprovisioned oracle fails
// loudly.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { oracleCompilerOptions, oracleProject } from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const LEDGER = join(ROOT, "fixtures/obligation-cases/cases.json");
const CASE_ROOT = join(ROOT, "rust/target/obligation-cases");

const json = process.argv.includes("--json");
const ledger = JSON.parse(readFileSync(LEDGER, "utf8"));

const locate = (variable, ...candidates) => {
  const found = process.env[variable] ?? candidates.find((candidate) => existsSync(candidate));
  if (!found || !existsSync(found)) {
    console.error(
      `missing the binary ${variable} names (tried ${candidates.join(", ")}).\n` +
        `This gate supplies evidence to an obligation and asks what changed; without` +
        ` the checker it can ask nothing at all.`
    );
    process.exit(2);
  }
  return found;
};
const CHECKER = locate("SOLID_CHECKER_BIN", join(ROOT, "rust/target/debug/solid-checker-rust"));
const TYPEFACTS = locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin/solid-typefacts"));

const canonicalRule = (testCase) =>
  testCase.dialect === "v1" && !testCase.rule.startsWith("v1/")
    ? `v1/${testCase.rule}`
    : testCase.rule;

// One directory per dialect holding a symlink to that dialect's audited
// install, so the tree the case resolves against is also the tree that decides
// which catalog runs. A symlink, not a copy: the audited install stays
// read-only.
const prepared = new Map();
const dialectBase = (dialect) => {
  if (prepared.has(dialect)) return prepared.get(dialect);
  const { root } = oracleProject(dialect);
  const base = join(CASE_ROOT, dialect);
  mkdirSync(base, { recursive: true });
  const link = join(base, "node_modules");
  if (!existsSync(link)) symlinkSync(join(root, "node_modules"), link, "dir");
  prepared.set(dialect, { base });
  return prepared.get(dialect);
};

/** Run the checker over one snippet, in a project of its own. */
const analyze = (testCase, label, code, args) => {
  const { base } = dialectBase(testCase.dialect);
  const dir = join(base, label);
  mkdirSync(dir, { recursive: true });
  const source = `case.${testCase.sourceExtension ?? "tsx"}`;
  const body = code.endsWith("\n") ? code : `${code}\n`;
  writeFileSync(
    join(dir, "tsconfig.json"),
    `${JSON.stringify(
      { compilerOptions: oracleCompilerOptions(testCase.dialect, true), files: [source] },
      null,
      2
    )}\n`
  );
  writeFileSync(join(dir, source), body);
  const output = execFileSync(
    CHECKER,
    ["--format", "json", "--project", join(dir, "tsconfig.json"), ...args],
    {
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: { ...process.env, SOLID_TYPEFACTS_BIN: TYPEFACTS }
    }
  );
  const rule = canonicalRule(testCase);
  return (JSON.parse(output).findings ?? []).filter((finding) => finding.rule === rule);
};

const slug = (name) => name.replace(/[^a-z0-9]+/gi, "-");
const failures = [];
const record = (testCase, index, message) => {
  failures.push(`${canonicalRule(testCase)} [${index}]: ${message}`);
};

let closures = 0;
ledger.cases.forEach((testCase, index) => {
  const args = testCase.args ?? [];
  const obligation = analyze(testCase, `${slug(testCase.rule)}-${index}-obligation`, testCase.obligation, args);
  const uncertifiable = obligation.filter((finding) => finding.kind === "uncertifiable");

  if (uncertifiable.length !== 1) {
    record(
      testCase,
      index,
      `the obligation snippet must produce exactly one uncertifiable finding of this rule,` +
        ` saw ${obligation.length} finding(s) (${
          obligation.map((finding) => finding.kind).join(", ") || "none"
        }).` +
        ` If the checker now settles this on its own, that is a real change: move the case to a` +
        ` closure with the evidence it now uses, and say so in docs/precision-backlog.md.`
    );
    return;
  }

  const closes = testCase.closes ?? [];
  if (closes.length === 0 && !testCase.irreducible) {
    record(
      testCase,
      index,
      `no closure and no "irreducible" reason. Every obligation owes one of the two: either` +
        ` evidence that settles it, or a written reason why no evidence can.`
    );
    return;
  }

  closes.forEach((closure, closureIndex) => {
    closures += 1;
    const found = analyze(
      testCase,
      `${slug(testCase.rule)}-${index}-closes-${closureIndex}`,
      closure.code,
      closure.args ?? args
    );
    const stillOpen = found.filter((finding) => finding.kind === "uncertifiable");
    if (stillOpen.length > 0) {
      record(
        testCase,
        index,
        `closure ${closureIndex} (${closure.evidence}) supplies the evidence and the finding is` +
          ` still uncertifiable. The obligation is an over-conservatism, not a missing fact:` +
          ` the evidence is present and unused.`
      );
      return;
    }
    if (closure.expect === "silent") {
      if (found.length > 0) {
        record(
          testCase,
          index,
          `closure ${closureIndex} (${closure.evidence}) must certify, saw ${found
            .map((finding) => finding.kind)
            .join(", ")}.`
        );
      }
      return;
    }
    if (closure.expect === "violation") {
      if (!found.some((finding) => finding.kind === "violation")) {
        record(
          testCase,
          index,
          `closure ${closureIndex} (${closure.evidence}) must prove a violation, saw ${
            found.map((finding) => finding.kind).join(", ") || "nothing"
          }.`
        );
      }
      return;
    }
    record(testCase, index, `closure ${closureIndex} has an unknown expect ${closure.expect}.`);
  });
});

if (json) {
  console.log(JSON.stringify({ cases: ledger.cases.length, closures, failures }, null, 2));
} else if (failures.length > 0) {
  console.error("obligation audit failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
} else {
  console.log(
    `obligation audit: ${ledger.cases.length} obligation(s) hold, ${closures} closure(s) discharge` +
      ` theirs, and every irreducible one carries a written reason`
  );
}
process.exit(failures.length > 0 ? 1 : 0);
