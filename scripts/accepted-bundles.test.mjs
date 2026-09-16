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

test("the index is the format the checker compiles in", () => {
  assert.equal(index.format, "solid-checker-accepted-contract-bundle-index");
  assert.equal(index.bundleIndexVersion, 1);
  assert.ok(Array.isArray(index.bundles));
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
      bundle.bindings?.artifactAcceptanceRoot,
      `${bundle.packageName} has no artifactAcceptanceRoot, so no project could match it`
    );
    // Package-relative, or the comparison against a consumer's resolved file is
    // between an absolute path on this machine and one on theirs.
    for (const target of [bundle.runtimeTarget, bundle.declarationTarget ?? ""]) {
      assert.ok(!target.startsWith("/"), `${bundle.packageName} states an absolute target`);
    }
  }
});

test("no two bundles claim the same artifact", () => {
  // Two contracts for one acceptance root is two answers about the same bytes.
  // `AcceptedContractIndex` drops such a pair rather than choosing, so a
  // duplicate here silently removes both from every build.
  const seen = new Map();
  for (const bundle of index.bundles) {
    const root = bundle.bindings.artifactAcceptanceRoot;
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
