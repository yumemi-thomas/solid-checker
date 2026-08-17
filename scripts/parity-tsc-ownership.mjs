// Who owns each upstream case: this checker, or TypeScript?
//
// `scripts/parity.mjs` records every divergence from eslint-plugin-solid in
// `deviations.json`, and 59 of them now carry `status: "policy"` with a prose
// reason of the form "TypeScript already reports this". Nothing checked that
// those reasons were true. A hand-written justification that no gate verifies is
// exactly the shape of the problem the absolute rule exists to fix — so this
// script compiles the *same bytes* parity lints, against the real published
// Solid 1.x typings, and holds the ledger to its own claims.
//
//   node scripts/parity.mjs                         first: writes the run artifact
//   node scripts/parity-tsc-ownership.mjs           enforce the ledger's claims
//   node scripts/parity-tsc-ownership.mjs --report  print the partition
//
// Both directions compare **spans**, not files. That distinction is the whole
// design: the upstream corpus is untyped JavaScript, so almost every case also
// carries an incidental implicit-any or cannot-find-name error, and "TypeScript
// reports something in this file" is worth nothing. Direction one requires the
// diagnostic to land inside the case's *own* text rather than the harness
// imports; direction two requires it to overlap a span the checker actually
// reported, which it reads from `parity.mjs`'s run artifact.
//
// Both gate:
//
//   a `policy` deviation whose case TypeScript does *not* report
//       The reason is false. Either the rule should still fire there, or the
//       reason names the wrong mechanism. This caught two `class:mt-10` cases
//       declared covered by TS2322 when TypeScript in fact declines to check any
//       hyphenated JSX attribute name at all -- a hole in a narrowing that no
//       other gate could see.
//
//   a finding whose span a TypeScript diagnostic covers
//       A live duplicate: the checker and TypeScript are speaking about the same
//       expression. Every one needs either a narrowing or an ACKNOWLEDGED entry
//       saying what the finding claims that the type error does not.
//
// The partition printed under `--report` counts cases that do not type-check at
// all, which is a lower bound on corpus cleanliness and *not* an ownership
// claim -- most of it is incidental. The span-matched duplicates below it are
// the ownership claim.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { CORPUS, caseId, corpusCases, materialize } from "./lib/upstream-cases.mjs";
import { runOracle } from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const report = process.argv.includes("--report");

const deviations = JSON.parse(readFileSync(join(CORPUS, "deviations.json"), "utf8"));
const corpus = corpusCases();

// Written by `scripts/parity.mjs`. Absent means nobody has run parity in this
// build tree, and guessing would be worse than stopping: without spans this
// check degrades to the file-level comparison it exists to replace.
const ARTIFACT = join(ROOT, "rust/target/upstream-parity/findings.json");
if (!existsSync(ARTIFACT)) {
  console.error(
    `no parity run artifact at ${ARTIFACT}.\n` +
      `Run 'node scripts/parity.mjs' first -- this check needs the spans the checker` +
      ` actually reported, not just which files it spoke about.`,
  );
  process.exit(2);
}
const checkerFindings = JSON.parse(readFileSync(ARTIFACT, "utf8")).cases;

// One program over the whole corpus. Per-case oracle runs would copy the
// audited install 465 times; the diagnostics are per file either way.
const inputs = [];
const meta = new Map();
for (const rule of corpus) {
  for (const kind of ["valid", "invalid"]) {
    rule[kind].forEach((testCase, index) => {
      const id = caseId(rule.rule, kind, index);
      const name = `${id}.tsx`;
      const materialized = materialize(rule.rule, testCase);
      inputs.push({ name, code: materialized.source });
      // The case's own bytes, excluding the imports the harness prepends and the
      // `export {}` it appends. A diagnostic in an affix is the harness's, never
      // the rule's subject.
      const start = Buffer.byteLength(materialized.prefix, "utf8");
      meta.set(name, {
        id,
        rule: rule.rule,
        kind,
        ownText: [start, start + Buffer.byteLength(testCase.code, "utf8")],
      });
    });
  }
}

const result = runOracle("v1", inputs);
// Both passes: "only under `strict`" is not an exception the absolute rule
// recognises, so a diagnostic in either pass means TypeScript speaks.
const speaks = new Map();
for (const pass of ["strict", "loose"]) {
  for (const diagnostic of result.passes[pass]) {
    if (diagnostic.category !== "error") continue;
    if (!speaks.has(diagnostic.file)) speaks.set(diagnostic.file, []);
    speaks.get(diagnostic.file).push({ pass, ...diagnostic });
  }
}

const CORPUS_ARTEFACT =
  "the diagnostic is about a name the corpus invents, not the expression the finding is about. Upstream's cases are untyped fragments: this one leans on a fake attribute (`prop1`, `prop2`, `a`, `b`, `c`) or an undeclared local, and the finding's span is wide enough to overlap it. Written against a real prop the case would type-check and only the finding would remain.";
const USE_DIRECTIVE =
  "TS2322 says the `use:` *attribute* is not declared, because `JSX.Directives` ships empty and is meant to be augmented -- the documented way to use a directive. The finding says no lexical *value* binding exists for the name. In a project that has augmented `Directives` (the only kind that compiles) TypeScript is silent and the finding stands alone, so the two questions only coincide in code no real project has.";
const ARRAY_HANDLER =
  "TS2322 is about the array's *element* types, not the bound-handler form: upstream's cases pass an untyped or mismatched second element, so the tuple fails to match `BoundEventHandler`'s parameter. The finding's claim is that the bound form defeats handler identity comparison, which holds for a *well typed* tuple -- and there TypeScript is silent, which is the case pinned in fixtures/tsc-oracle/rule-cases.json.";

// Findings a TypeScript diagnostic covers by span, kept deliberately because the
// two make *different* claims about the same code. Keyed `<case id>:<rule>`, and
// held to the same standard as the oracle gate's `distinct-claim`: name what the
// finding asserts that the type error does not.
const ACKNOWLEDGED = {
  "imports__invalid__02:v1/no-proxy-apis":
    "TS2305 says `solid-js` does not export `createMutable`; the finding says the Proxy-based store API is unavailable on the configured target runtime. The finding's span is the whole import statement, which is why it overlaps. Written as the correct `solid-js/store` import the type error disappears and the runtime-support claim remains.",
  "jsx-no-duplicate-props__invalid__02:v1/jsx-no-duplicate-props":
    "TS2322 says `a` is not an attribute of `<div>`; the finding says the spread-carried `a` and the attribute `a` land in one slot and one is dead. Upstream invents the attribute name, so the type error is a corpus artefact -- with a real attribute (`id`) TypeScript is silent in this order, which is the very case the narrowing kept.",
  "jsx-no-undef__invalid__01:v1/jsx-no-undef": USE_DIRECTIVE,
  "jsx-no-undef__invalid__02:v1/jsx-no-undef": USE_DIRECTIVE,
  "jsx-no-undef__invalid__03:v1/jsx-no-undef": USE_DIRECTIVE,
  "jsx-no-undef__invalid__04:v1/jsx-no-undef": USE_DIRECTIVE,
  "no-unknown-namespaces__valid__05:v1/jsx-no-undef": USE_DIRECTIVE,
  "no-unknown-namespaces__valid__06:v1/jsx-no-undef": USE_DIRECTIVE,
  "no-array-handlers__invalid__02:v1/no-array-handlers": ARRAY_HANDLER,
  "no-array-handlers__invalid__03:v1/no-array-handlers": ARRAY_HANDLER,
  "no-array-handlers__invalid__05:v1/no-array-handlers": ARRAY_HANDLER,
  "no-array-handlers__invalid__07:v1/no-array-handlers": ARRAY_HANDLER,
  "no-array-handlers__invalid__05:v1/event-handlers": CORPUS_ARTEFACT,
  "no-destructure__invalid__05:v1/no-destructure": CORPUS_ARTEFACT,
  "no-destructure__invalid__06:v1/no-destructure": CORPUS_ARTEFACT,
  "no-destructure__invalid__11:v1/no-destructure": CORPUS_ARTEFACT,
  "no-destructure__invalid__12:v1/no-destructure": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__03:v1/no-innerhtml": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__05:v1/no-innerhtml": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__06:v1/no-innerhtml": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__07:v1/no-innerhtml": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__08:v1/no-innerhtml": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__05:v1/jsx-no-duplicate-props": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__06:v1/jsx-no-duplicate-props": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__07:v1/jsx-no-duplicate-props": CORPUS_ARTEFACT,
  "no-innerhtml__invalid__08:v1/jsx-no-duplicate-props": CORPUS_ARTEFACT,
  "no-innerhtml__valid__04:v1/self-closing-comp": CORPUS_ARTEFACT,
  "prefer-classlist__invalid__04:v1/prefer-classlist":
    "TS2322 says `className` is not an attribute of `<div>` -- which is SC8011's territory, and there it is TypeScript's. This finding's claim is the stylistic one: an object-valued class expression is better written as `classList`. On the `class` spelling upstream's own valid cases use, TypeScript is silent and the preference stands alone.",
  "prefer-for__invalid__02:v1/prefer-for": CORPUS_ARTEFACT,
  "prefer-for__invalid__06:v1/prefer-for": CORPUS_ARTEFACT,
};

// Findings a TypeScript diagnostic covers by span where the two make the **same**
// claim. These are confirmed duplicates awaiting a narrowing, each with an entry
// in docs/precision-backlog.md. Suppressing the failure here is a debt marker,
// not a resting place -- the same role `redundant-pending-narrowing` plays in the
// oracle gate.
const PENDING_NARROWING = {
  "jsx-no-duplicate-props__invalid__06:v1/jsx-no-duplicate-props":
    "TS2710 \"'children' are specified twice. The attribute named 'children' will be overwritten.\" is word for word this finding's claim. Only the `children`-prop-plus-JSX-children pair is covered; the `innerHTML` and `textContent` combinations are not, so this is a narrowing of the child-content arm rather than its removal.",
};

// Codes that are pure artefacts of linting untyped upstream JavaScript. They are
// never a rule's subject, so they never establish that TypeScript owns a defect.
const INCIDENTAL = new Set([
  7006, // Parameter implicitly has an 'any' type
  7031, // Binding element implicitly has an 'any' type
  2304, // Cannot find name
  18004, // No value exists in scope for the shorthand property
  2554, // Expected N arguments
  2559, // Type has no properties in common
]);
const owned = (id) => speaks.get(`${id}.tsx`) ?? [];
const byId = new Map([...meta.values()].map((entry) => [entry.id, entry]));
const overlaps = (diagnostic, [start, end]) =>
  diagnostic.startByte < end && start < diagnostic.endByte;
const failures = [];

// Direction one: every `policy` reason must be true — and true *about the case's
// own code*. A diagnostic in the harness's prepended imports is not the rule's
// subject, and neither is an incidental untyped-JavaScript error.
for (const [id, entry] of Object.entries(deviations.fired ?? {})) {
  // Only `typescript-owned`. Plain `policy` predates this check and covers any
  // deliberate difference from upstream -- a name-only allowlist this checker
  // does not carry, a stricter requirement for an explicit `untrack` -- none of
  // which claims TypeScript reports anything. Gating those on a diagnostic would
  // demand evidence for a claim they never made.
  if (entry.status !== "typescript-owned") continue;
  const own = byId.get(id)?.ownText;
  const diagnostics = owned(id).filter(
    (diagnostic) => !INCIDENTAL.has(diagnostic.code) && (!own || overlaps(diagnostic, own)),
  );
  if (!diagnostics.length) {
    failures.push(
      `${id}: declared status "typescript-owned" — "${entry.reason.slice(0, 90)}…" — but TypeScript reports` +
        ` nothing in this case's own code against solid-js@${result.versions["solid-js"]}.` +
        `\n    Either the rule should still fire here, or the reason names the wrong mechanism.`,
    );
  }
}

// Direction two: a finding whose span a TypeScript diagnostic covers. Both are
// talking about the same expression, which is the duplication the absolute rule
// forbids. Span overlap is deliberately permissive about *which* span is wider:
// TypeScript often reports the whole argument where the checker reports the
// returned expression inside it, and that is still the same defect.
const duplicates = [];
for (const { id, rule } of meta.values()) {
  const diagnostics = owned(id).filter((diagnostic) => !INCIDENTAL.has(diagnostic.code));
  if (!diagnostics.length) continue;
  for (const finding of checkerFindings[id] ?? []) {
    const covering = diagnostics.filter((diagnostic) =>
      overlaps(diagnostic, [finding.startByte, finding.endByte]),
    );
    if (!covering.length) continue;
    duplicates.push({
      id,
      rule,
      finding,
      codes: [...new Set(covering.map((diagnostic) => diagnostic.code))].sort(),
    });
  }
}
for (const { id, finding, codes } of duplicates) {
  const key = `${id}:${finding.rule}`;
  if (ACKNOWLEDGED[key] || PENDING_NARROWING[key]) continue;
  failures.push(
    `${id}: ${finding.rule} reports bytes ${finding.startByte}..${finding.endByte} and` +
      ` TypeScript reports ${codes.map((code) => `TS${code}`).join("/")} over the same span.` +
      `\n    The same expression, two diagnostics. Narrow the rule, or add an ACKNOWLEDGED` +
      ` entry keyed "${id}:${finding.rule}" saying what the finding claims that the type` +
      ` error does not.`,
  );
}

if (report) {
  const perRule = new Map();
  for (const { id, rule, kind } of meta.values()) {
    if (!perRule.has(rule)) perRule.set(rule, { total: 0, typescript: 0, invalid: 0 });
    const row = perRule.get(rule);
    row.total += 1;
    if (kind === "invalid") row.invalid += 1;
    if (owned(id).length) row.typescript += 1;
  }
  console.log(`corpus against solid-js@${result.versions["solid-js"]}, TypeScript ${result.typescript}\n`);
  console.log("  cases  ts-owned  rule");
  for (const [rule, row] of [...perRule].sort((a, b) => b[1].typescript - a[1].typescript)) {
    console.log(
      `  ${String(row.total).padStart(5)}  ${String(row.typescript).padStart(8)}  ${rule}`,
    );
  }
  const total = meta.size;
  const tsOwned = [...meta.values()].filter(({ id }) => owned(id).length).length;
  console.log(
    `\n  ${tsOwned} of ${total} cases do not type-check against the real typings (${Math.round(
      (100 * tsOwned) / total,
    )}%) -- mostly incidentally, since the upstream corpus is untyped JavaScript.` +
      `\n  This is NOT "TypeScript owns the defect in these cases": that needs the` +
      ` diagnostic to overlap the finding's span, which this comparison does not check.`,
  );
  console.log(
    `\n  ${duplicates.length} span-matched duplicate(s): a finding and a TypeScript` +
      ` diagnostic over the same bytes.`,
  );
  for (const { id, finding, codes } of duplicates) {
    console.log(
      `    ${id} ${finding.rule} [${finding.startByte}..${finding.endByte}]` +
        ` ${codes.map((code) => `TS${code}`).join("/")}`,
    );
  }
}

if (failures.length) {
  console.error(`parity ownership: ${failures.length} problem(s)\n`);
  for (const failure of failures) console.error(`  - ${failure}\n`);
  process.exit(1);
}
console.log(
  `parity ownership: ${
    Object.values(deviations.fired ?? {}).filter((entry) => entry.status === "typescript-owned")
    .length
  } "typescript-owned" reasons verified against solid-js@${result.versions["solid-js"]};` +
    ` ${Object.keys(ACKNOWLEDGED).length} span matches acknowledged as distinct claims,` +
    ` ${Object.keys(PENDING_NARROWING).length} confirmed duplicates awaiting a narrowing`,
);
if (!existsSync(join(ROOT, "fixtures/tsc-oracle/packages.json"))) process.exit(1);
