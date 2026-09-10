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
