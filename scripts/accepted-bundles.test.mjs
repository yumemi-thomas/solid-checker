// The compiled-in accepted-contract tier, checked without building anything.
//
// `accepted_bundles::every_bundle_this_build_carries_authenticates` is the
// authority on whether these bundles load — it runs the real loader. What it
// cannot see is the repository around them: an object left behind by a previous
// generation, a `include_bytes!` list that drifted from the index, an index
// entry naming bytes nobody wrote. Each of those is a review hazard rather than
// a load failure, and each is cheap to catch here.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "vitest";
import { fileURLToPath } from "node:url";

const REPOSITORY = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLE_ROOT = join(REPOSITORY, "pkg/contracts/accepted");
const EMBEDDED = join(
  REPOSITORY,
  "rust/crates/solid-facts-backend/src/accepted_bundles/embedded.rs"
);

const index = JSON.parse(readFileSync(join(BUNDLE_ROOT, "index.json"), "utf8"));
const embedded = readFileSync(EMBEDDED, "utf8");

function objectMembers() {
  return readdirSync(join(BUNDLE_ROOT, "objects"))
    .filter(name => name.endsWith(".json"))
    .map(name => `objects/${name}`)
    .sort();
}

function named() {
  return [...new Set(index.bundles.flatMap(bundle => [bundle.document, bundle.receipt]))].sort();
}

// Version 2 states every bundle's dependency environment. Version 1 predates
// that binding; the loader still reads it, as inert (no bundle of it is ever
// admitted), so a checked-in version-1 tier builds and supplies nothing.
// Version 3 states what version 2 does, each fact once: environments in one
// table keyed by the root the receipts sign, and bindings only in the receipt.
const stated = index.bundleIndexVersion >= 2;
const indexed = index.bundleIndexVersion === 3;

/** A bundle's bindings: its receipt's payload in version 3, restated before. */
function bindingsOf(bundle) {
  if (!indexed) return bundle.bindings;
  return JSON.parse(readFileSync(join(BUNDLE_ROOT, bundle.receipt), "utf8")).payload;
}

/** A bundle's dependency environment, wherever its index version keeps it. */
function environmentOf(bundle) {
  return indexed ? index.environments[bundle.dependencyEnvironmentRoot] : bundle.dependencyEnvironment;
}

test("the index is the format the checker compiles in", () => {
  assert.equal(index.format, "solid-checker-accepted-contract-bundle-index");
  assert.ok([1, 2, 3].includes(index.bundleIndexVersion), `version ${index.bundleIndexVersion}`);
  assert.ok(Array.isArray(index.bundles));
  if (indexed) {
    assert.ok(index.environments && typeof index.environments === "object", "version 3 has an environments table");
    for (const bundle of index.bundles) {
      assert.equal(bundle.bindings, undefined, `${bundle.packageName} restates its receipt's bindings`);
      assert.equal(bundle.dependencyEnvironment, undefined, `${bundle.packageName} states its environment inline`);
    }
  }
});

test("a version-3 environments table is exactly what the bundles name, once each, in key order", () => {
  // The loader refuses a reference with no table entry and an entry that is
  // not the environment its root names. What it does not refuse is an entry
  // nobody names, or two keys for one environment -- dead weight and a second
  // spelling, each a review hazard rather than a load failure.
  if (!indexed) return;
  const keys = Object.keys(index.environments);
  assert.deepEqual(keys, [...keys].sort(), "the table is not in key order");
  const named = new Set(index.bundles.map(bundle => bundle.dependencyEnvironmentRoot));
  assert.deepEqual([...named].sort(), keys, "the table and the bundles' references disagree");
  const spelled = Object.values(index.environments).map(environment => JSON.stringify(environment));
  assert.equal(new Set(spelled).size, spelled.length, "one environment is stored under two keys");
  for (const bundle of index.bundles) {
    assert.equal(
      bundle.dependencyEnvironmentRoot,
      bindingsOf(bundle).dependencyEnvironmentRoot,
      `${bundle.packageName} names an environment its receipt does not sign`
    );
  }
});

test("a version-2 or later bundle states the environment its receipt binds", () => {
  if (!stated) return;
  for (const bundle of index.bundles) {
    assert.ok(
      bindingsOf(bundle)?.dependencyEnvironmentRoot,
      `${bundle.packageName} binds no dependency environment, so no project could be checked against it`
    );
    const environment = environmentOf(bundle);
    assert.ok(Array.isArray(environment), `${bundle.packageName} publishes no environment`);
    const spelled = environment.map(entry => JSON.stringify([entry.name, entry.version, entry.integrity]));
    // ADR 0126: an environment is edged on every entry or on none. An edged one
    // may name one package more than once, under different importers, so it is
    // canonical when its entries are sorted and no whole entry repeats.
    const edged = environment.filter(entry => entry.resolvedFrom).length;
    assert.ok(
      edged === 0 || edged === environment.length,
      `${bundle.packageName}'s environment is edged on some entries only`
    );
    if (edged === 0) {
      assert.deepEqual(spelled, [...new Set(spelled)].sort(), `${bundle.packageName}'s environment is not canonical`);
    } else {
      assert.deepEqual(spelled, [...spelled].sort(), `${bundle.packageName}'s environment is not sorted`);
      const whole = environment.map(entry => JSON.stringify(entry));
      assert.equal(new Set(whole).size, whole.length, `${bundle.packageName}'s environment repeats an entry`);
    }
  }
});

test("every object is at its content address and is named by the index", () => {
  // The address is how the loader tells the pair apart from a hand-edited copy,
  // and the digest the index states is what it checks against.
  const members = objectMembers();
  assert.deepEqual(members, named(), "an object nobody names is dead weight in every build");
  for (const bundle of index.bundles) {
    for (const [member, digest] of [
      [bundle.document, bundle.documentDigest],
      [bundle.receipt, bundle.receiptDigest]
    ]) {
      const bytes = readFileSync(join(BUNDLE_ROOT, member));
      const actual = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
      assert.equal(actual, digest, `${member} does not match the digest the index states`);
      assert.ok(member.includes(digest.slice("sha256:".length)), `${member} is not content-addressed`);
    }
  }
});

test("the compiled-in object list is exactly what the index names", () => {
  // A drift here is invisible until a build: the index would name bytes this
  // binary does not carry, and the tier would refuse at a user's first run.
  for (const member of named()) {
    assert.ok(embedded.includes(JSON.stringify(member)), `${member} is not compiled in`);
    assert.ok(
      embedded.includes(`pkg/contracts/accepted/${member}`),
      `${member} has no include_bytes! path`
    );
  }
  const compiled = [...embedded.matchAll(/"(objects\/[^"]+)"/g)].map(match => match[1]).sort();
  assert.deepEqual(compiled, named(), "the generated list and the index disagree");
});

test("a bundle states every field admission recomputes", () => {
  for (const bundle of index.bundles) {
    for (const field of [
      "packageName",
      "packageVersion",
      "packageIntegrity",
      "specifier",
      "requestedEntrypoint",
      "runtimeTarget"
    ]) {
      assert.ok(bundle[field], `${bundle.packageName ?? "?"} states no ${field}`);
    }
    assert.ok(
      Array.isArray(bundle.exportConditions) && bundle.exportConditions.length > 0,
      `${bundle.packageName} states no export conditions, which select the artifact`
    );
    assert.ok(
      bindingsOf(bundle)?.artifactAcceptanceRoot,
      `${bundle.packageName} has no artifactAcceptanceRoot, so no project could match it`
    );
    // Package-relative, or the comparison against a consumer's resolved file is
    // between an absolute path on this machine and one on theirs.
    for (const target of [bundle.runtimeTarget, bundle.declarationTarget ?? ""]) {
      assert.ok(!target.startsWith("/"), `${bundle.packageName} states an absolute target`);
    }
  }
});

test("no two bundles claim the same artifact in the same environment", () => {
  // Two contracts for one acceptance root in one environment is two answers
  // about the same bytes. `AcceptedContractIndex` drops such a pair rather than
  // choosing, so a duplicate here silently removes both from every build. The
  // same artifact in two environments is two acceptances (a floor and a head
  // certification), and the loader keys them apart.
  const seen = new Map();
  for (const bundle of index.bundles) {
    const root = stated
      ? `${bindingsOf(bundle).artifactAcceptanceRoot} ${bindingsOf(bundle).dependencyEnvironmentRoot}`
      : bindingsOf(bundle).artifactAcceptanceRoot;
    const previous = seen.get(root);
    assert.equal(
      previous,
      undefined,
      `${bundle.packageName} and ${previous} share the acceptance root ${root}`
    );
    seen.set(root, bundle.packageName);
  }
});

test("the runtime foundation is never bundled", () => {
  // ADR 0027: ordinary analysis takes solid-js, @solidjs/signals and
  // @solidjs/web from the dialect. The Rust loader refuses one too; this is the
  // half that fails in review rather than at build time.
  const core = ["solid-js", "@solidjs/signals", "@solidjs/web"];
  for (const bundle of index.bundles) {
    assert.ok(
      !core.includes(bundle.packageName),
      `${bundle.packageName} is the runtime foundation and cannot be a package contract`
    );
  }
});
