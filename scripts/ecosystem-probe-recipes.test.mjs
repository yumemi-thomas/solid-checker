// Pins the shape of the checked-in ecosystem probe-recipe corpus
// (`scripts/ecosystem-benchmark/probe-recipes/`).
//
// It lives in `scripts/` rather than beside the corpus deliberately:
// `scripts/verify.sh` runs `scripts/*.test.mjs`, while
// `scripts/ecosystem-benchmark/*.test.mjs` runs only under
// `make ecosystem-benchmark-test`, which `make verify` does not invoke. A
// corpus whose manifest Rust rejects would otherwise be discovered by a
// twenty-minute benchmark run instead of by handoff verification.
//
// What it cannot pin: whether a `claimId` still addresses a live claim. Claim
// ids are content digests of the normalized claim, so a generator change
// silently turns a recipe into an unaddressed module and the candidate is
// withheld rather than censused. Only a real run measures that, and the
// corpus README says so.

import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, test } from "vitest";

import { UNFINISHED_MARKER } from "./probe-recipe-scaffold.mjs";

const corpusDirectory = join(
  dirname(fileURLToPath(import.meta.url)),
  "ecosystem-benchmark",
  "probe-recipes"
);

// Mirrors `RecipeCorpus::load` and `WireRecipeCorpus` in
// rust/crates/solid-facts-backend/src/contract_certification/probe_harness.rs.
// Rust deserializes with `deny_unknown_fields`, so an extra key is a refusal
// there and must be one here.
const FORMAT = "solid-checker-probe-recipe-corpus";
const SCHEMA_VERSION = 1;
const IMPORT_KINDS = new Set(["esm", "require"]);
const SCENARIOS = new Set([
  "operation",
  "cleanup-lifecycle",
  "repeated-async-iterable",
  "transition-lifecycle",
  "request-response-lifecycle",
  "root-lifetime"
]);
const EVENT_CLASSES = new Set([
  "call",
  "render",
  "flush",
  "callback",
  "cleanup",
  "settlement",
  "emission",
  "transition",
  "request",
  "response",
  "stream"
]);
/// A coverage limitation opening with this declares that the module cannot
/// emit its expected event, and that the silence is intended rather than a
/// mistyped marker. It is prose on purpose: the manifest's field set is
/// mirrored by a `deny_unknown_fields` struct in Rust, so a new key would be a
/// wire change, and this distinction is a claim about the *author's* intent
/// that only the author can make.
const NEVER_EMITS = "NEVER EMITS:";

const RECIPE_KEYS = new Set([
  "claimId",
  "module",
  "importKind",
  "dependencySpecifiers",
  "scenario",
  "expectedEvent",
  "drain",
  "coverageLimitations"
]);

/// The events a recipe module can emit, as literal `marker`/`kind` pairs.
///
/// Comments are stripped first, and that is load-bearing rather than tidy: a
/// scaffold carries its real marker *only* in a commented-out `harness.emit`
/// line, so a scanner that read comments would call an unfinished scaffold
/// agreed with its manifest.
///
/// A marker built at run time rather than written as a literal is not found
/// here, and the caller treats that as a failure. That is the fail-closed
/// direction: this corpus has none, and the alternative is a recipe whose
/// ability to veto nobody can check.
export function emittedEvents(source) {
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .split("\n")
    .map(line => line.replace(/^\s*\/\/.*$/, ""))
    .join("\n");
  const events = [];
  for (const match of code.matchAll(/emit\(\s*\{([^}]*)\}/g)) {
    const marker = match[1].match(/marker\s*:\s*"([^"]*)"/);
    const kind = match[1].match(/kind\s*:\s*"([^"]*)"/);
    if (marker) events.push({ marker: marker[1], kind: kind ? kind[1] : null });
  }
  return events;
}

const manifest = JSON.parse(
  readFileSync(join(corpusDirectory, "recipes.json"), "utf8")
);

describe("the ecosystem probe-recipe corpus", () => {
  test("declares the format and schema version Rust requires", () => {
    assert.equal(manifest.format, FORMAT);
    assert.equal(manifest.schemaVersion, SCHEMA_VERSION);
    assert.deepEqual(Object.keys(manifest).sort(), [
      "format",
      "policy",
      "recipes",
      "schemaVersion"
    ]);
  });

  test("declares a bounded probe policy", () => {
    assert.deepEqual(Object.keys(manifest.policy).sort(), [
      "maxEvents",
      "maxMacrotaskTurns",
      "maxMicrotaskTurns",
      "repeatRuns",
      "timeoutMillis"
    ]);
    for (const [field, value] of Object.entries(manifest.policy)) {
      assert.equal(
        Number.isInteger(value) && value > 0,
        true,
        `policy.${field} must be a positive integer, got ${value}`
      );
    }
  });

  test("names every recipe exactly once, and each field Rust reads", () => {
    const seen = new Set();
    for (const recipe of manifest.recipes) {
      assert.deepEqual(
        Object.keys(recipe).filter(key => !RECIPE_KEYS.has(key)),
        [],
        `${recipe.module} carries a field Rust would refuse`
      );
      assert.match(recipe.claimId, /^claim:v1:sha256:[0-9a-f]{64}$/);
      assert.equal(seen.has(recipe.claimId), false, `${recipe.claimId} repeats`);
      seen.add(recipe.claimId);
      // `importKind` is required and not defaulted: the two kinds resolve
      // under different export-condition sets, so a corpus that does not say
      // which one it uses is not saying what the gate observed.
      assert.equal(IMPORT_KINDS.has(recipe.importKind), true, recipe.module);
      assert.equal(SCENARIOS.has(recipe.scenario), true, recipe.module);
      assert.equal(
        EVENT_CLASSES.has(recipe.expectedEvent.class),
        true,
        recipe.module
      );
      assert.equal(typeof recipe.expectedEvent.marker, "string");
      assert.notEqual(recipe.expectedEvent.marker.length, 0);
      assert.equal(Array.isArray(recipe.dependencySpecifiers), true);
      assert.equal(Array.isArray(recipe.drain), true);
      // A recipe that declares no limitation is claiming a coverage it does
      // not have: every launch is one Node build against one artifact case.
      assert.notEqual(recipe.coverageLimitations.length, 0, recipe.module);
    }
  });

  test("declares every module in the directory, and no module it lacks", () => {
    const declared = manifest.recipes.map(recipe => recipe.module).sort();
    const present = readdirSync(corpusDirectory)
      .filter(name => name.endsWith(".mjs"))
      .sort();
    assert.deepEqual(declared, present);
    // A module name has to be a single relative component: Rust refuses a
    // path, and the workspace copies each module by file name.
    for (const name of declared) {
      assert.match(name, /^[a-z0-9][a-z0-9-]*\.mjs$/);
    }
  });

  test("holds no unfinished scaffold", () => {
    // `scripts/probe-recipe-scaffold.mjs` emits modules that throw until an
    // author writes their observation, so an unfinished one withholds its
    // candidate exactly as no recipe at all does. That makes a scaffold a
    // safe *working* state and a pointless shipped one: every run would
    // launch a worker per gate to arrive back at the withholding it started
    // from. Committing one is therefore a mistake, not a decision.
    for (const name of readdirSync(corpusDirectory).filter(file => file.endsWith(".mjs"))) {
      assert.equal(
        readFileSync(join(corpusDirectory, name), "utf8").includes(UNFINISHED_MARKER),
        false,
        `${name} is still a scaffold: write its observation or remove it`
      );
    }
  });

  test("every module exports runProbeSession and never hands the transcript away", () => {
    for (const recipe of manifest.recipes) {
      const source = readFileSync(join(corpusDirectory, recipe.module), "utf8");
      assert.match(
        source,
        /export async function runProbeSession\(_session, harness\)/,
        `${recipe.module} must export the probe entry point`
      );
      // ADR 0006's one obligation nothing in the design can enforce: a recipe
      // must never hand `session` or `harness` to the package under test. A
      // bare `harness` in an argument list is the shape that does it, so the
      // only permitted uses in *code* are the harness's own `emit` calls.
      // Comments are stripped first — every recipe's header states the rule it
      // obeys, and a header would otherwise fail the check it documents.
      const code = source
        .replaceAll(/\/\*[\s\S]*?\*\//g, " ")
        .replaceAll(/(^|[^:])\/\/.*$/gm, "$1");
      const stray = (code.match(/\bharness\b(?!\.emit\b)/g) ?? []).length;
      assert.equal(
        stray,
        1,
        `${recipe.module} mentions harness in code outside an emit call ` +
          "(the one permitted mention is the parameter itself)"
      );
      assert.equal(
        /\bsession\b/.test(code.replaceAll("_session", "")),
        false,
        `${recipe.module} names session outside the ignored parameter`
      );
    }
  });
});

// Every *other* checked-in recipe corpus manifest.
//
// Here rather than beside the fixtures for the same reason as the block
// above: `scripts/verify.sh` runs `scripts/*.test.mjs`, and nothing under
// `fixtures/package-contracts/*/probe-recipes/` is otherwise loaded by a
// gate. The fixture corpora are assembled in-process by the Rust tracer
// tests, which build their own manifest from the modules and the live claim
// ids, so a checked-in `recipes.json` there is documentary — and one of them
// carried `"policy": 2` where Rust requires the policy object, which is a
// manifest `RecipeCorpus::load` refuses outright and no gate ever read.
//
// Claim ids are deliberately not checked. They are content digests of the
// normalized claim, so only a live run can say whether one still addresses
// anything; each corpus README says so.
describe("the checked-in fixture recipe corpora", () => {
  const fixtureRoot = join(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    "fixtures",
    "package-contracts"
  );
  const manifests = readdirSync(fixtureRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => join(fixtureRoot, entry.name, "probe-recipes", "recipes.json"))
    .filter(path => existsSync(path));

  test("each declares the envelope Rust requires", () => {
    for (const path of manifests) {
      const manifest = JSON.parse(readFileSync(path, "utf8"));
      assert.equal(manifest.format, FORMAT, path);
      assert.equal(manifest.schemaVersion, SCHEMA_VERSION, path);
      assert.deepEqual(Object.keys(manifest.policy ?? {}).sort(), [
        "maxEvents",
        "maxMacrotaskTurns",
        "maxMicrotaskTurns",
        "repeatRuns",
        "timeoutMillis"
      ], path);
      for (const recipe of manifest.recipes) {
        assert.deepEqual(
          Object.keys(recipe).filter(key => !RECIPE_KEYS.has(key)),
          [],
          `${path} ${recipe.module} carries a field Rust would refuse`
        );
        assert.match(recipe.claimId, /^claim:v1:sha256:[0-9a-f]{64}$/);
        assert.equal(IMPORT_KINDS.has(recipe.importKind), true, recipe.module);
        assert.equal(SCENARIOS.has(recipe.scenario), true, recipe.module);
        assert.equal(EVENT_CLASSES.has(recipe.expectedEvent.class), true, recipe.module);
        assert.equal(
          existsSync(join(dirname(path), recipe.module)),
          true,
          `${path} names a module that is not there: ${recipe.module}`
        );
      }
    }
  });

  // The one mistake nothing caught, and the corpus README names it: a module
  // and its manifest entry have to agree on the event, and `event_matches` in
  // rust/crates/solid-facts-backend/src/runtime_probes.rs requires the marker
  // *and* the class to match. Disagree on either and no emitted event ever
  // matches the gate's expectation.
  //
  // That failure is silent and it fails **open**. A run that completes without
  // a matching event is a `CleanNonObservation` -- "a complete, isolated,
  // deterministic, scenario-satisfying execution did not observe the
  // contradiction the recipe was written to provoke" -- which *satisfies* the
  // mandatory gate, and the closure then certifies on the census alone. So a
  // recipe mis-typed in one character does not refuse and does not warn: it
  // stops being able to veto, and everything downstream reads as proven.
  //
  // Static agreement is checkable and this checks it. What it cannot check is
  // whether the emit is *reached* at run time; a recipe whose emit sits behind
  // a condition that never holds is the same failure, and only a real run that
  // contradicts something can find that one.
  test("emits, in every module, the event its manifest entry expects", () => {
    for (const recipe of manifest.recipes) {
      const source = readFileSync(join(corpusDirectory, recipe.module), "utf8");
      const events = emittedEvents(source);
      const agreed = events.some(
        event =>
          event.marker === recipe.expectedEvent.marker &&
          event.kind === recipe.expectedEvent.class
      );
      const declaredSilent = recipe.coverageLimitations.some(limitation =>
        limitation.startsWith(NEVER_EMITS)
      );
      if (declaredSilent) {
        // Declared silent, so the agreement rule is waived -- but the
        // declaration has to stay true, or it is worse than no declaration at
        // all: it would waive the check for a recipe that *does* emit and
        // whose marker later drifts. `access` is the real case, and its own
        // limitation says why: the read it counts is the caller's under
        // ADR 0034, so emitting would assert a contradiction that is not this
        // package's.
        assert.equal(
          agreed,
          false,
          `${recipe.module} declares ${NEVER_EMITS} but does emit its expected event; remove the declaration`
        );
        continue;
      }
      assert.notEqual(
        events.length,
        0,
        `${recipe.module} emits no event with a literal marker, so its gate can never match`
      );
      assert.equal(
        agreed,
        true,
        `${recipe.module} never emits {marker: ${JSON.stringify(recipe.expectedEvent.marker)}, ` +
          `kind: ${JSON.stringify(recipe.expectedEvent.class)}}; it emits ` +
          `${JSON.stringify(events)}. An unmatchable gate passes, so this recipe cannot veto. ` +
          `If the silence is deliberate, say so with a "${NEVER_EMITS}" coverage limitation.`
      );
    }
  });

  // How many of this corpus's mandatory vetoes cannot veto, stated as a
  // number rather than left to be discovered.
  //
  // A recipe that declares `NEVER EMITS:` is not unsound -- closure is the
  // implementation census's job (ADR 0008), and a vacuous recipe can only fail
  // to contradict. But its gate passes by construction, and the per-recipe
  // check above cannot say how much of the corpus is in that state. § 65 of
  // `phase21/2026-09-10-reads-veto-observation-design.md` found all five of
  // them in the `reads` domain and none anywhere else, which is § 6's
  // argument measured: an unenumerated read is a read of a source the export
  // *owns*, and no observation from outside can see it.
  //
  // Pinning the list is what makes a sixth one a decision. A `creates` or
  // `returns` recipe going silent would land here as a diff, where today the
  // only trace is a gate that quietly stops being able to fail.
  test("says how many of its vetoes cannot veto, and which", () => {
    const silent = [];
    for (const recipe of manifest.recipes) {
      if (recipe.coverageLimitations.some(limitation => limitation.startsWith(NEVER_EMITS))) {
        silent.push(recipe.module);
      }
    }
    assert.deepEqual(silent.sort(), [
      "corvu-next-utils-dom-after-paint-reads-7496b629.mjs",
      "corvu-next-utils-dom-after-paint-reads-95369190.mjs",
      "corvu-next-utils-dom-call-event-handler-reads-7496b629.mjs",
      "corvu-next-utils-dom-call-event-handler-reads-95369190.mjs",
      "corvu-utils-data-if-reads-1b8ce990.mjs",
      "corvu-utils-data-if-reads-77550142.mjs",
      "corvu-utils-dom-after-paint-reads-2c64155d.mjs",
      "corvu-utils-dom-after-paint-reads-84d6a8cd.mjs",
      "corvu-utils-dom-after-paint-reads-f34410e8.mjs",
      "corvu-utils-dom-after-paint-reads-fd42a1d9.mjs",
      "corvu-utils-dom-call-event-handler-reads-2c64155d.mjs",
      "corvu-utils-dom-call-event-handler-reads-84d6a8cd.mjs",
      "corvu-utils-dom-call-event-handler-reads-f34410e8.mjs",
      "corvu-utils-dom-call-event-handler-reads-fd42a1d9.mjs",
      "corvu-utils-is-button-reads-1b8ce990.mjs",
      "corvu-utils-is-button-reads-77550142.mjs",
      "corvu-utils-is-function-reads-1b8ce990.mjs",
      "corvu-utils-is-function-reads-77550142.mjs",
      "corvu-utils-reactivity-access-reads-9e170a86.mjs",
      "corvu-utils-reactivity-access-reads-a0d201c0.mjs",
      "corvu-utils-reactivity-access-reads-f240869a.mjs",
      "corvu-utils-reactivity-access-reads-f5afb395.mjs",
      "corvu-utils-reactivity-access-reads-fa65fd49.mjs",
      "corvu-utils-reactivity-chain-reads-9e170a86.mjs",
      "corvu-utils-reactivity-chain-reads-a0d201c0.mjs",
      "corvu-utils-reactivity-chain-reads-f240869a.mjs",
      "corvu-utils-reactivity-chain-reads-f5afb395.mjs",
      "corvu-utils-reactivity-chain-reads-fa65fd49.mjs",
      "corvu-utils-reactivity-merge-refs-reads-9e170a86.mjs",
      "corvu-utils-reactivity-merge-refs-reads-a0d201c0.mjs",
      "corvu-utils-reactivity-merge-refs-reads-f240869a.mjs",
      "corvu-utils-reactivity-merge-refs-reads-f5afb395.mjs",
      "corvu-utils-reactivity-merge-refs-reads-fa65fd49.mjs",
      "corvu-utils-reactivity-some-reads-9e170a86.mjs",
      "corvu-utils-reactivity-some-reads-a0d201c0.mjs",
      "corvu-utils-reactivity-some-reads-f240869a.mjs",
      "corvu-utils-reactivity-some-reads-f5afb395.mjs",
      "corvu-utils-reactivity-some-reads-fa65fd49.mjs",
      "floating-ui-utils-clamp-reads-9bc68a12.mjs",
      "floating-ui-utils-create-coords-reads-9bc68a12.mjs",
      "floating-ui-utils-evaluate-reads-9bc68a12.mjs",
      "floating-ui-utils-expand-padding-object-reads-9bc68a12.mjs",
      "floating-ui-utils-get-alignment-axis-reads-9bc68a12.mjs",
      "floating-ui-utils-get-alignment-sides-reads-9bc68a12.mjs",
      "floating-ui-utils-get-axis-length-reads-9bc68a12.mjs",
      "floating-ui-utils-get-expanded-placements-reads-9bc68a12.mjs",
      "floating-ui-utils-get-opposite-axis-reads-9bc68a12.mjs",
      "floating-ui-utils-get-padding-object-reads-9bc68a12.mjs",
      "floating-ui-utils-get-side-axis-reads-9bc68a12.mjs",
      "floating-ui-utils-rect-to-client-rect-reads-9bc68a12.mjs",
      "motion-utils-anticipate-reads-9a3a41a5.mjs",
      "motion-utils-circ-in-reads-9a3a41a5.mjs",
      "motion-utils-clamp-reads-9a3a41a5.mjs",
      "motion-utils-cubic-bezier-reads-9a3a41a5.mjs",
      "motion-utils-easing-definition-to-function-reads-9a3a41a5.mjs",
      "motion-utils-get-easing-for-segment-reads-9a3a41a5.mjs",
      "motion-utils-has-warned-reads-9a3a41a5.mjs",
      "motion-utils-invariant-reads-9a3a41a5.mjs",
      "motion-utils-is-bezier-definition-reads-9a3a41a5.mjs",
      "motion-utils-is-easing-array-reads-9a3a41a5.mjs",
      "motion-utils-is-numerical-string-reads-9a3a41a5.mjs",
      "motion-utils-is-object-reads-9a3a41a5.mjs",
      "motion-utils-is-zero-value-string-reads-9a3a41a5.mjs",
      "motion-utils-memo-reads-9a3a41a5.mjs",
      "motion-utils-milliseconds-to-seconds-reads-9a3a41a5.mjs",
      "motion-utils-mirror-easing-reads-9a3a41a5.mjs",
      "motion-utils-move-item-reads-9a3a41a5.mjs",
      "motion-utils-noop-reads-9a3a41a5.mjs",
      "motion-utils-pipe-reads-9a3a41a5.mjs",
      "motion-utils-progress-reads-9a3a41a5.mjs",
      "motion-utils-reverse-easing-reads-9a3a41a5.mjs",
      "motion-utils-seconds-to-milliseconds-reads-9a3a41a5.mjs",
      "motion-utils-steps-reads-9a3a41a5.mjs",
      "motion-utils-velocity-per-second-reads-9a3a41a5.mjs",
      "motion-utils-warn-once-reads-9a3a41a5.mjs",
      "motion-utils-warning-reads-9a3a41a5.mjs",
      "motion-utils-wrap-reads-9a3a41a5.mjs",
      "solid-primitives-utils-access-reads-1bea9ecd.mjs",
      "solid-primitives-utils-access-reads-2bf41ff6.mjs",
      "solid-primitives-utils-access-reads-6cd714eb.mjs",
      "solid-primitives-utils-access-reads-9887e137.mjs",
      "solid-primitives-utils-access-reads-f81b5488.mjs",
      "solid-primitives-utils-access-with-reads-1bea9ecd.mjs",
      "solid-primitives-utils-access-with-reads-2bf41ff6.mjs",
      "solid-primitives-utils-access-with-reads-6cd714eb.mjs",
      "solid-primitives-utils-access-with-reads-9887e137.mjs",
      "solid-primitives-utils-access-with-reads-f81b5488.mjs",
      "solid-primitives-utils-after-paint-reads-1bea9ecd.mjs",
      "solid-primitives-utils-after-paint-reads-2bf41ff6.mjs",
      "solid-primitives-utils-after-paint-reads-6cd714eb.mjs",
      "solid-primitives-utils-after-paint-reads-f81b5488.mjs",
      "solid-primitives-utils-array-equals-reads-1bea9ecd.mjs",
      "solid-primitives-utils-array-equals-reads-2bf41ff6.mjs",
      "solid-primitives-utils-as-accessor-reads-1bea9ecd.mjs",
      "solid-primitives-utils-as-accessor-reads-2bf41ff6.mjs",
      "solid-primitives-utils-as-accessor-reads-6cd714eb.mjs",
      "solid-primitives-utils-as-accessor-reads-9887e137.mjs",
      "solid-primitives-utils-as-accessor-reads-f81b5488.mjs",
      "solid-primitives-utils-as-array-reads-1bea9ecd.mjs",
      "solid-primitives-utils-as-array-reads-2bf41ff6.mjs",
      "solid-primitives-utils-as-array-reads-6cd714eb.mjs",
      "solid-primitives-utils-as-array-reads-9887e137.mjs",
      "solid-primitives-utils-as-array-reads-f81b5488.mjs",
      "solid-primitives-utils-chain-reads-1bea9ecd.mjs",
      "solid-primitives-utils-chain-reads-2bf41ff6.mjs",
      "solid-primitives-utils-chain-reads-6cd714eb.mjs",
      "solid-primitives-utils-chain-reads-9887e137.mjs",
      "solid-primitives-utils-chain-reads-f81b5488.mjs",
      "solid-primitives-utils-clamp-reads-1bea9ecd.mjs",
      "solid-primitives-utils-clamp-reads-2bf41ff6.mjs",
      "solid-primitives-utils-clamp-reads-6cd714eb.mjs",
      "solid-primitives-utils-clamp-reads-9887e137.mjs",
      "solid-primitives-utils-clamp-reads-f81b5488.mjs",
      "solid-primitives-utils-colors-color-scale-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-color-scale-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-contrast-ratio-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-contrast-ratio-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-darken-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-darken-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-desaturate-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-desaturate-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-get-color-channels-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-get-color-channels-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-is-readable-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-is-readable-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-is-valid-color-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-is-valid-color-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-normalize-color-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-normalize-color-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-normalize-hue-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-normalize-hue-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-parse-color-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-parse-color-reads-cfb40777.mjs",
      "solid-primitives-utils-colors-try-parse-color-reads-8a0d569b.mjs",
      "solid-primitives-utils-colors-try-parse-color-reads-cfb40777.mjs",
      "solid-primitives-utils-compare-reads-1bea9ecd.mjs",
      "solid-primitives-utils-compare-reads-2bf41ff6.mjs",
      "solid-primitives-utils-compare-reads-6cd714eb.mjs",
      "solid-primitives-utils-compare-reads-9887e137.mjs",
      "solid-primitives-utils-compare-reads-f81b5488.mjs",
      "solid-primitives-utils-create-callback-stack-reads-1bea9ecd.mjs",
      "solid-primitives-utils-create-callback-stack-reads-2bf41ff6.mjs",
      "solid-primitives-utils-create-callback-stack-reads-6cd714eb.mjs",
      "solid-primitives-utils-create-callback-stack-reads-9887e137.mjs",
      "solid-primitives-utils-create-callback-stack-reads-f81b5488.mjs",
      "solid-primitives-utils-create-id-generator-reads-1bea9ecd.mjs",
      "solid-primitives-utils-create-id-generator-reads-2bf41ff6.mjs",
      "solid-primitives-utils-create-id-generator-reads-6cd714eb.mjs",
      "solid-primitives-utils-create-id-generator-reads-f81b5488.mjs",
      "solid-primitives-utils-false-fn-reads-1bea9ecd.mjs",
      "solid-primitives-utils-false-fn-reads-2bf41ff6.mjs",
      "solid-primitives-utils-false-fn-reads-6cd714eb.mjs",
      "solid-primitives-utils-false-fn-reads-9887e137.mjs",
      "solid-primitives-utils-false-fn-reads-f81b5488.mjs",
      "solid-primitives-utils-immutable-clamp-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-filter-out-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-get-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-merge-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-push-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-remove-items-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-shallow-copy-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-shallow-object-copy-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-splice-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-with-array-copy-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-with-copy-reads-b70ad6d1.mjs",
      "solid-primitives-utils-immutable-with-object-copy-reads-b70ad6d1.mjs",
      "solid-primitives-utils-is-non-nullable-reads-1bea9ecd.mjs",
      "solid-primitives-utils-is-non-nullable-reads-2bf41ff6.mjs",
      "solid-primitives-utils-is-non-nullable-reads-6cd714eb.mjs",
      "solid-primitives-utils-is-non-nullable-reads-9887e137.mjs",
      "solid-primitives-utils-is-non-nullable-reads-f81b5488.mjs",
      "solid-primitives-utils-is-object-reads-1bea9ecd.mjs",
      "solid-primitives-utils-is-object-reads-2bf41ff6.mjs",
      "solid-primitives-utils-is-object-reads-6cd714eb.mjs",
      "solid-primitives-utils-is-object-reads-9887e137.mjs",
      "solid-primitives-utils-is-object-reads-f81b5488.mjs",
      "solid-primitives-utils-json-reads-1bea9ecd.mjs",
      "solid-primitives-utils-json-reads-2bf41ff6.mjs",
      "solid-primitives-utils-json-reads-6cd714eb.mjs",
      "solid-primitives-utils-json-reads-9887e137.mjs",
      "solid-primitives-utils-json-reads-f81b5488.mjs",
      "solid-primitives-utils-noop-reads-1bea9ecd.mjs",
      "solid-primitives-utils-noop-reads-2bf41ff6.mjs",
      "solid-primitives-utils-noop-reads-6cd714eb.mjs",
      "solid-primitives-utils-noop-reads-9887e137.mjs",
      "solid-primitives-utils-noop-reads-f81b5488.mjs",
      "solid-primitives-utils-number-reads-1bea9ecd.mjs",
      "solid-primitives-utils-number-reads-2bf41ff6.mjs",
      "solid-primitives-utils-number-reads-6cd714eb.mjs",
      "solid-primitives-utils-number-reads-9887e137.mjs",
      "solid-primitives-utils-number-reads-f81b5488.mjs",
      "solid-primitives-utils-of-class-reads-1bea9ecd.mjs",
      "solid-primitives-utils-of-class-reads-2bf41ff6.mjs",
      "solid-primitives-utils-of-class-reads-6cd714eb.mjs",
      "solid-primitives-utils-of-class-reads-9887e137.mjs",
      "solid-primitives-utils-of-class-reads-f81b5488.mjs",
      "solid-primitives-utils-pipe-reads-1bea9ecd.mjs",
      "solid-primitives-utils-pipe-reads-2bf41ff6.mjs",
      "solid-primitives-utils-pipe-reads-6cd714eb.mjs",
      "solid-primitives-utils-pipe-reads-9887e137.mjs",
      "solid-primitives-utils-pipe-reads-f81b5488.mjs",
      "solid-primitives-utils-reverse-chain-reads-1bea9ecd.mjs",
      "solid-primitives-utils-reverse-chain-reads-2bf41ff6.mjs",
      "solid-primitives-utils-reverse-chain-reads-6cd714eb.mjs",
      "solid-primitives-utils-reverse-chain-reads-9887e137.mjs",
      "solid-primitives-utils-reverse-chain-reads-f81b5488.mjs",
      "solid-primitives-utils-safe-reads-1bea9ecd.mjs",
      "solid-primitives-utils-safe-reads-2bf41ff6.mjs",
      "solid-primitives-utils-safe-reads-6cd714eb.mjs",
      "solid-primitives-utils-safe-reads-9887e137.mjs",
      "solid-primitives-utils-safe-reads-f81b5488.mjs",
      "solid-primitives-utils-true-fn-reads-1bea9ecd.mjs",
      "solid-primitives-utils-true-fn-reads-2bf41ff6.mjs",
      "solid-primitives-utils-true-fn-reads-6cd714eb.mjs",
      "solid-primitives-utils-true-fn-reads-9887e137.mjs",
      "solid-primitives-utils-true-fn-reads-f81b5488.mjs",
      "solid-primitives-utils-with-access-reads-1bea9ecd.mjs",
      "solid-primitives-utils-with-access-reads-2bf41ff6.mjs",
      "solid-primitives-utils-with-access-reads-6cd714eb.mjs",
      "solid-primitives-utils-with-access-reads-9887e137.mjs",
      "solid-primitives-utils-with-access-reads-f81b5488.mjs",
      "tanstack-store-batch-reads-0bdfa1cb.mjs",
      "tanstack-store-create-async-atom-reads-0bdfa1cb.mjs",
      "tanstack-store-create-atom-reads-0bdfa1cb.mjs",
      "tanstack-store-create-store-reads-0bdfa1cb.mjs"
    ]);
    assert.equal(
      manifest.recipes.length - silent.length,
      // Fourteen, plus `web-with-meta.mjs` (the `@solidjs/web` `returns`
      // recipe that landed with the with-meta work).
      15,
      "the recipes that can still contradict something"
    );
  });

  test("holds no unfinished scaffold either", () => {
    for (const path of manifests) {
      for (const name of readdirSync(dirname(path)).filter(file => file.endsWith(".mjs"))) {
        assert.equal(
          readFileSync(join(dirname(path), name), "utf8").includes(UNFINISHED_MARKER),
          false,
          `${join(dirname(path), name)} is still a scaffold`
        );
      }
    }
  });
});
