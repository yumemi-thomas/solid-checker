import { test } from "vitest";
import assert from "node:assert/strict";

import { AUDITED_SOLID_1, AUDITED_SOLID_2, familyById } from "./lib/families.mjs";
import { distTagsFor, prereleaseChannel, selectRow, solidReleaseCatalog } from "./lib/select.mjs";
import { maxSatisfying, minSatisfying } from "./lib/semver.mjs";

// Builds a hand-rolled abbreviated packument: { "dist-tags": {...},
// versions: { [version]: { dependencies, peerDependencies,
// optionalDependencies, dist: { integrity }, deprecated } } }.
function packument(versions, distTags = {}) {
  const out = { "dist-tags": distTags, versions: {} };
  for (const [version, spec] of Object.entries(versions)) {
    out.versions[version] = {
      dependencies: spec.dependencies ?? {},
      peerDependencies: spec.peerDependencies ?? {},
      optionalDependencies: spec.optionalDependencies ?? {},
      dist: { integrity: spec.integrity ?? `sha512-${version}` },
      ...(spec.deprecated ? { deprecated: spec.deprecated } : {})
    };
  }
  return out;
}

function runtimePackuments({ solidJs, web, signals } = {}) {
  const map = new Map();
  map.set("solid-js", solidJs ?? null);
  map.set("@solidjs/web", web ?? null);
  map.set("@solidjs/signals", signals ?? null);
  return map;
}

const OFFICIAL_SOLID = familyById("official-solid");
const SOLID_PRIMITIVES = familyById("solid-primitives");
const TANSTACK = familyById("tanstack");

// A representative solid-js packument: 1.x releases up through the audited
// version, plus a 2.x prerelease ladder (experimental -> beta -> rc) that
// mirrors the real registry facts pinned in families.mjs's comment block.
function solidJsPackument() {
  return packument(
    {
      "1.6.12": {},
      "1.8.0": {},
      "1.9.14": {},
      "2.0.0-experimental.0": {},
      "2.0.0-beta.16": {},
      "2.0.0-beta.17": {},
      "2.0.0-beta.34": {},
      "2.0.0-rc.0": {},
      "2.0.0-rc.1": {},
      "2.0.0-rc.3": {},
      "2.0.0-rc.4": {}
    },
    { latest: "1.9.15", next: "2.0.0-rc.1", beta: "1.10.0-beta.0" }
  );
}

test("solidReleaseCatalog splits every runtime package's releases by Solid major", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  assert.deepEqual(catalog["solid-js"].v1, ["1.6.12", "1.8.0", "1.9.14"]);
  // semver orders unequal prerelease identifiers by ASCII string comparison,
  // not by real-world release chronology — "beta" < "experimental" < "rc".
  assert.deepEqual(catalog["solid-js"].v2, [
    "2.0.0-beta.16",
    "2.0.0-beta.17",
    "2.0.0-beta.34",
    "2.0.0-experimental.0",
    "2.0.0-rc.0",
    "2.0.0-rc.1",
    "2.0.0-rc.3"
  ]);
  assert.deepEqual(catalog["solid-js"].distTags, { latest: "1.9.15", next: "2.0.0-rc.1", beta: "1.10.0-beta.0" });
  // Runtime packages missing from the map (or unpublished) get an empty,
  // never-satisfiable catalog rather than throwing.
  // `peers` records what each runtime release requires of its siblings; an
  // absent packument contributes no releases and therefore no peer facts.
  assert.deepEqual(catalog["@solidjs/web"], { v1: [], v2: [], distTags: {}, peers: {} });
  assert.deepEqual(catalog["@solidjs/signals"], { v1: [], v2: [], distTags: {}, peers: {} });
});

test("Solid 2 catalog and official runtime rows are capped at audited RC.3", () => {
  const solidJs = solidJsPackument();
  const web = packument({ [AUDITED_SOLID_2]: { peerDependencies: { "solid-js": `^${AUDITED_SOLID_2}` } } });
  const signals = packument({ [AUDITED_SOLID_2]: {} });
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs, web, signals }));
  assert.equal(catalog["solid-js"].v2.at(-1), AUDITED_SOLID_2);
  assert.equal(catalog["solid-js"].v2.includes("2.0.0-rc.4"), false);

  const result = selectRow({
    packageName: "solid-js",
    packument: solidJs,
    family: OFFICIAL_SOLID,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1,
    auditedSolid2: AUDITED_SOLID_2
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, AUDITED_SOLID_2);
  assert.deepEqual(result.row.compatibleSolidVersions, {
    "@solidjs/signals": [AUDITED_SOLID_2],
    "@solidjs/web": [AUDITED_SOLID_2],
    "solid-js": [AUDITED_SOLID_2]
  });
  assert.deepEqual(result.row.probes, [
    {
      id: `solid-js@${AUDITED_SOLID_2}|solid2|only`,
      kind: "only",
      channel: "rc",
      solid: {
        "@solidjs/signals": AUDITED_SOLID_2,
        "@solidjs/web": AUDITED_SOLID_2,
        "solid-js": AUDITED_SOLID_2
      }
    }
  ]);
});

test("Solid 2 official runtime rows fail closed when the audited tuple is incomplete", () => {
  const solidJs = solidJsPackument();
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs }));
  const result = selectRow({
    packageName: "solid-js",
    packument: solidJs,
    family: OFFICIAL_SOLID,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1,
    auditedSolid2: AUDITED_SOLID_2
  });
  assert.equal(result.kind, "exclusion");
  assert.match(result.exclusion.detail, /@solidjs\/web@2\.0\.0-rc\.3 is missing/);
});

test("prereleaseChannel reads the prerelease identifier, never a string prefix trick", () => {
  assert.equal(prereleaseChannel("1.9.14"), "stable");
  assert.equal(prereleaseChannel("2.0.0-experimental.5"), "experimental");
  assert.equal(prereleaseChannel("2.0.0-beta.17"), "beta");
  assert.equal(prereleaseChannel("2.0.0-rc.1"), "rc");
  assert.equal(prereleaseChannel("2.0.0-nightly.1"), "other");
});

test("distTagsFor returns sorted tags pointing at a version, empty when none", () => {
  const doc = solidJsPackument();
  assert.deepEqual(distTagsFor(doc, "1.9.14"), []);
  assert.deepEqual(distTagsFor(doc, "2.0.0-rc.1"), ["next"]);
  const multiTag = packument({ "1.0.0": {} }, { latest: "1.0.0", beta: "1.0.0" });
  assert.deepEqual(distTagsFor(multiTag, "1.0.0"), ["beta", "latest"]);
});

test("solid1: solid-js itself is pinned to auditedSolid1 exactly, never searched", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const result = selectRow({
    packageName: "solid-js",
    packument: solidJsPackument(),
    family: OFFICIAL_SOLID,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, AUDITED_SOLID_1);
  assert.deepEqual(result.row.compatibleSolidVersions, { "solid-js": [AUDITED_SOLID_1] });
  assert.equal(result.row.probes.length, 1);
  assert.equal(result.row.probes[0].kind, "only");
  assert.equal(result.row.probes[0].id, `solid-js@${AUDITED_SOLID_1}|solid1|only`);
});

test("solid1: solid-js not published at the audited version is excluded not-published", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({}));
  const result = selectRow({
    packageName: "solid-js",
    packument: packument({ "1.0.0": {} }),
    family: OFFICIAL_SOLID,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "exclusion");
  assert.equal(result.exclusion.reason, "not-published");
});

test.each([
  ["@tanstack/charts", "0.15.0", ["./solid"]],
  ["@tanstack/devtools-utils", "0.7.0", ["./solid", "./solid/class"]],
  [
    "@tanstack/devtools-a11y",
    "0.2.2",
    ["./core", "./core/production", "./solid", "./solid/production"]
  ]
])("%s excludes its reviewed non-Solid entrypoints", (packageName, version, entrypoints) => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const result = selectRow({
    packageName,
    packument: packument({
      [version]: { peerDependencies: { "solid-js": ">=1.8" } }
    }),
    family: TANSTACK,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.deepEqual(result.row.probes[0].entrypoints, entrypoints);
});

test("a scoped package release change fails closed until its export scope is reviewed", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  assert.throws(
    () =>
      selectRow({
        packageName: "@tanstack/devtools-utils",
        packument: packument({ "0.8.0": { peerDependencies: { "solid-js": ">=1.8" } } }),
        family: TANSTACK,
        status: "official",
        solidTarget: "solid1",
        catalog,
        auditedSolid1: AUDITED_SOLID_1
      }),
    /reviewed at 0\.7\.0, but discovery selected 0\.8\.0/
  );
});

test("solid1: ^1.6.12 accepts the audited 1.9.14 release, package version selected newest-first", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "1.0.0": { dependencies: { "solid-js": "^1.0.0" } },
    "1.5.0": { dependencies: { "solid-js": "^1.6.12" } }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "1.5.0");
  assert.deepEqual(result.row.dependencies, { "solid-js": "^1.6.12" });
  assert.deepEqual(result.row.compatibleSolidVersions, { "solid-js": [AUDITED_SOLID_1] });
  assert.deepEqual(result.row.unparsedRanges, []);
  assert.equal(result.row.probes.length, 1);
  assert.equal(result.row.probes[0].kind, "only");
  assert.equal(result.row.probes[0].channel, "stable");
});

test("solid1: ^1.6.12 does not accept a 2.x prerelease target (no 2.x catalog entries make it moot, but the range itself must not match)", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "1.0.0": { dependencies: { "solid-js": "^1.6.12" } } });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "exclusion");
  assert.equal(result.exclusion.reason, "no-compatible-release");
});

test("solid2: caret prerelease range accepts same-tuple prereleases up to rc but not an older beta or a 1.x release; the floor is anchored at rc.0", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "3.0.0": { dependencies: { "solid-js": "^2.0.0-beta.17" } } });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  // "2.0.0-experimental.0" is also >= the beta.17 floor here: npm compares
  // unequal prerelease identifiers by ASCII ("beta" < "experimental" < "rc"),
  // not by real release-lifecycle order, so it lands inside the range too.
  assert.deepEqual(result.row.compatibleSolidVersions, {
    "solid-js": [
      "2.0.0-beta.17",
      "2.0.0-beta.34",
      "2.0.0-experimental.0",
      "2.0.0-rc.0",
      "2.0.0-rc.1",
      "2.0.0-rc.3"
    ]
  });
  assert.equal(result.row.probes.length, 2);
  assert.deepEqual(result.row.probes.map(probe => probe.kind), ["floor", "head"]);
  // The range's formal minimum is beta.17, but the floor probe starts at rc.0:
  // the pre-rc 2.x releases are no longer what the ecosystem builds against,
  // and installing one produces peer conflicts that describe nobody's
  // supported window. `compatibleSolidVersions` above still records the full
  // accepted set, so the range fact is not lost -- only the probe moves.
  assert.equal(result.row.probes[0].solid["solid-js"], "2.0.0-rc.0");
  assert.equal(result.row.probes[1].solid["solid-js"], "2.0.0-rc.3");
  assert.equal(result.row.probes[0].channel, "rc");
  assert.equal(result.row.probes[1].channel, "rc");
});

test("solid2: a package that accepts no rc keeps its own beta floor", () => {
  // The rc.0 anchor raises the floor; it never invents a release outside the
  // declared range. A range that stops before rc.0 has no rc to move to, so
  // the oldest accepted beta stays the floor -- the same rule that keeps a
  // beta-only package off a newer rc at the head.
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "3.0.0": { dependencies: { "solid-js": ">=2.0.0-beta.17 <2.0.0-rc.0" } }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.probes[0].solid["solid-js"], "2.0.0-beta.17");
  assert.equal(result.row.probes[0].channel, "beta");
});

test("solid2: floor/head for ^2.0.0-beta.17 are minSatisfying/maxSatisfying directly, confirming the accepted experimental release is neither", () => {
  // Same catalog and range as the test above, but pinned at the semver
  // primitive level rather than observed through selectRow's probes: the
  // experimental release satisfies the range (it is semver-greater than
  // beta.17 within the 2.0.0 tuple) but it is never the floor or the head,
  // since beta.17 stays the minimum and audited rc.3 stays the maximum of the
  // accepted set.
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pool = catalog["solid-js"].v2;
  const range = "^2.0.0-beta.17";
  assert.equal(minSatisfying(pool, range), "2.0.0-beta.17");
  assert.equal(maxSatisfying(pool, range), "2.0.0-rc.3");
});

test("solid2: >=2.0.0 does not accept a 2.0.0-rc.1-only catalog (npm's stable-range/prerelease rule)", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: packument({ "2.0.0-rc.1": {} }) }));
  const pkg = packument({ "1.0.0": { dependencies: { "solid-js": ">=2.0.0" } } });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "exclusion");
  assert.equal(result.exclusion.reason, "no-compatible-release");
});

test("solid2: beta-only compatibility yields exactly one probe pinned to that exact beta, never substituted by a newer rc present in the catalog", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  // An exact pin (no caret) accepts only that one release even though the
  // runtime catalog also contains a newer beta and two rc releases.
  const pkg = packument({ "0.1.0": { dependencies: { "solid-js": "2.0.0-beta.17" } } });
  const result = selectRow({
    packageName: "bleeding-edge-adapter",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.deepEqual(result.row.compatibleSolidVersions, { "solid-js": ["2.0.0-beta.17"] });
  assert.equal(result.row.probes.length, 1);
  assert.equal(result.row.probes[0].kind, "only");
  assert.equal(result.row.probes[0].solid["solid-js"], "2.0.0-beta.17");
  assert.equal(result.row.probes[0].channel, "beta");
});

test("an unparsed range is never treated as a match and never silently dropped: sole-range case excludes with reason unparsed-range", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "1.0.0": { dependencies: { "solid-js": "not a real range!!" } } });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "exclusion");
  assert.equal(result.exclusion.reason, "unparsed-range");
  assert.deepEqual(result.exclusion.unparsedRanges, [{ package: "solid-js", range: "not a real range!!" }]);
});

test("an unparsed range on a non-deciding runtime package is still recorded on an otherwise-compatible row", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "1.0.0": {
      dependencies: { "solid-js": "^2.0.0-beta.17" },
      peerDependencies: { "@solidjs/signals": "garbage!!" }
    }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.deepEqual(result.row.unparsedRanges, [{ package: "@solidjs/signals", range: "garbage!!" }]);
});

test("deprecated versions are skipped in favor of an older non-deprecated compatible release", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "1.0.0": { dependencies: { "solid-js": "^1.6.12" } },
    "2.0.0": { dependencies: { "solid-js": "^1.6.12" }, deprecated: "superseded" }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "1.0.0");
  assert.equal(result.row.deprecated, null);
});

test("a package deprecated at every compatible release is still selected, at its newest, with deprecated recorded", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "1.0.0": { dependencies: { "solid-js": "^1.6.12" }, deprecated: "use the fork instead" },
    "2.0.0": { dependencies: { "solid-js": "^1.6.12" }, deprecated: "use the fork instead" }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "2.0.0");
  assert.equal(result.row.deprecated, "use the fork instead");
});

test("not-published: no packument at all", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const result = selectRow({
    packageName: "@solid-primitives/ghost",
    packument: null,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "exclusion");
  assert.equal(result.exclusion.reason, "not-published");
});

test("a package with no declared range for any SOLID_RUNTIME_PACKAGE is excluded no-solid-dependency", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "1.0.0": { dependencies: { react: "^18.0.0" } } });
  const solid1 = selectRow({
    packageName: "@tanstack/react-query",
    packument: pkg,
    family: TANSTACK,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  const solid2 = selectRow({
    packageName: "@tanstack/react-query",
    packument: pkg,
    family: TANSTACK,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  // A React-only TanStack package never yields a row on either target — this
  // is the mechanism that keeps React/Vue/Svelte TanStack packages out of
  // the corpus even though their npm scope classifies as the tanstack family.
  assert.equal(solid1.kind, "exclusion");
  assert.equal(solid1.exclusion.reason, "no-solid-dependency");
  assert.equal(solid2.kind, "exclusion");
  assert.equal(solid2.exclusion.reason, "no-solid-dependency");
});

test("a genuine tanstack Solid adapter is retained: declares a SOLID_RUNTIME_PACKAGE range and selects normally", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "1.0.0": { peerDependencies: { "solid-js": "^1.6.12" } } });
  const result = selectRow({
    packageName: "@tanstack/solid-query",
    packument: pkg,
    family: TANSTACK,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "1.0.0");
  assert.deepEqual(result.row.peerDependencies, { "solid-js": "^1.6.12" });
});

test("row shape carries only SOLID_RUNTIME_PACKAGES ranges, sorted by key, and a deterministic probe id", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({
    "1.5.0": {
      dependencies: { "solid-js": "^1.6.12", react: "^18.0.0" },
      peerDependencies: { "@solidjs/signals": "^1.0.0" }
    }
  });
  const result = selectRow({
    packageName: "@solid-primitives/scheduled",
    packument: pkg,
    family: SOLID_PRIMITIVES,
    status: "official",
    solidTarget: "solid1",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.deepEqual(Object.keys(result.row.dependencies), ["solid-js"]);
  assert.equal(result.row.dependencies.react, undefined);
  assert.equal(result.row.probes[0].id, `@solid-primitives/scheduled@1.5.0|solid1|only`);
});

test("selection is deterministic: same inputs produce byte-identical output across repeated calls", () => {
  const catalog = solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() }));
  const pkg = packument({ "3.0.0": { dependencies: { "solid-js": "^2.0.0-beta.17" } } });
  const build = () =>
    selectRow({
      packageName: "@solid-primitives/scheduled",
      packument: pkg,
      family: SOLID_PRIMITIVES,
      status: "official",
      solidTarget: "solid2",
      catalog,
      auditedSolid1: AUDITED_SOLID_1
    });
  assert.deepEqual(build(), build());
});

// A package that has LEFT the Solid ecosystem must be ignored, not represented
// by whatever stale release still happened to declare Solid. This is the real
// @tanstack/react-router-devtools case: version 1.121.2 declared
// `solid-js: ^1.9.5`, while the package's current release depends on
// @tanstack/router-devtools-core and peers on React only. Walking back to the
// stale release put a React package into the Solid corpus.
test("a package whose current release dropped Solid is excluded, not represented by a stale release", () => {
  const packument = {
    "dist-tags": { latest: "1.167.1" },
    versions: {
      "1.121.2": { dependencies: { "solid-js": "^1.9.5" }, dist: { integrity: "sha512-stale" } },
      "1.167.1": {
        dependencies: { "@tanstack/router-devtools-core": "1.168.1" },
        peerDependencies: { react: ">=18.0.0" },
        dist: { integrity: "sha512-current" }
      }
    }
  };
  for (const solidTarget of ["solid1", "solid2"]) {
    const result = selectRow({
      packageName: "@tanstack/react-router-devtools",
      packument,
      family: { id: "tanstack", requireSolidDependency: true },
      status: "official",
      solidTarget,
      catalog: solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() })),
      auditedSolid1: "1.9.14"
    });
    assert.equal(result.kind, "exclusion", `${solidTarget} must not produce a row`);
    assert.equal(result.exclusion.reason, "no-solid-dependency");
    assert.match(result.exclusion.detail, /current release 1\.167\.1/);
  }
});

// The gate must not break the legitimate walk-back: a package whose current
// release moved to Solid 2 still declares Solid, so its newest 1.x-compatible
// release is still the right selection for the solid1 target.
test("a package whose current release declares Solid 2 still selects its newest Solid 1 release", () => {
  const packument = {
    "dist-tags": { latest: "3.0.0" },
    versions: {
      "1.0.0": { peerDependencies: { "solid-js": "^1.6.0" }, dist: { integrity: "sha512-one" } },
      "2.0.0": { peerDependencies: { "solid-js": "^1.8.0" }, dist: { integrity: "sha512-two" } },
      "3.0.0": { peerDependencies: { "solid-js": "^2.0.0-beta.17" }, dist: { integrity: "sha512-three" } }
    }
  };
  const result = selectRow({
    packageName: "@example/moved-to-solid2",
    packument,
    family: { id: "solid-primitives", requireSolidDependency: false },
    status: "official",
    solidTarget: "solid1",
    catalog: solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() })),
    auditedSolid1: "1.9.14"
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "2.0.0");
});

// solid-js does not depend on solid-js; the runtime packages must be exempt or
// the gate would delete the corpus's own baseline.
test("the Solid runtime packages themselves are exempt from the current-release gate", () => {
  const packument = {
    "dist-tags": { latest: "1.9.14" },
    versions: { "1.9.14": { dependencies: { csstype: "^3.1.0" }, dist: { integrity: "sha512-solid" } } }
  };
  const result = selectRow({
    packageName: "solid-js",
    packument,
    family: { id: "official-solid", requireSolidDependency: false },
    status: "official",
    solidTarget: "solid1",
    catalog: solidReleaseCatalog(runtimePackuments({ solidJs: solidJsPackument() })),
    auditedSolid1: "1.9.14"
  });
  assert.equal(result.kind, "row");
  assert.equal(result.row.version, "1.9.14");
});

test("solid2: the floor tuple is raised until the runtime packages accept each other", () => {
  // The real @tanstack/solid-router@2.0.0-rc.1 shape: wide peer ranges on both
  // runtime packages, but a hard dependency pinning @solidjs/web to ^2.0.0-rc.1
  // -- and that web release peers solid-js ^2.0.0-rc.1. Flooring each package
  // in isolation yields solid-js rc.0 with web rc.1, which npm refuses before
  // the checker runs, so the resulting install-failure would describe the
  // benchmark's arithmetic rather than the package.
  const catalog = solidReleaseCatalog(
    runtimePackuments({
      solidJs: solidJsPackument(),
      web: packument({
        "2.0.0-rc.0": { peerDependencies: { "solid-js": "^2.0.0-rc.0" } },
        "2.0.0-rc.1": { peerDependencies: { "solid-js": "^2.0.0-rc.1" } }
      })
    }),
    "2.0.0-rc.1"
  );
  const pkg = packument({
    "3.0.0": {
      dependencies: { "@solidjs/web": "^2.0.0-rc.1" },
      peerDependencies: { "solid-js": ">=2.0.0-0 <3.0.0", "@solidjs/web": ">=2.0.0-0 <3.0.0" }
    }
  });
  const result = selectRow({
    packageName: "@tanstack/solid-router",
    packument: pkg,
    family: TANSTACK,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  // web has exactly one compatible release, and it forces solid-js to rc.1.
  // Floor and head then coincide, so the row is one coherent probe rather
  // than two, one of which could never install.
  assert.equal(result.row.probes.length, 1);
  assert.equal(result.row.probes[0].kind, "only");
  assert.equal(result.row.probes[0].solid["solid-js"], "2.0.0-rc.1");
  assert.equal(result.row.probes[0].solid["@solidjs/web"], "2.0.0-rc.1");
});

test("solid2: a floor is only ever raised, never moved outside the compatible set", () => {
  // Same sibling constraint, but the package accepts both rc releases of web.
  // The floor rises to the oldest *coherent* pair (rc.0 + rc.0) rather than
  // jumping to the head, so the older environment is still measured.
  const catalog = solidReleaseCatalog(
    runtimePackuments({
      solidJs: solidJsPackument(),
      web: packument({
        "2.0.0-rc.0": { peerDependencies: { "solid-js": "^2.0.0-rc.0" } },
        "2.0.0-rc.1": { peerDependencies: { "solid-js": "^2.0.0-rc.1" } }
      })
    }),
    "2.0.0-rc.1"
  );
  const pkg = packument({
    "3.0.0": {
      peerDependencies: { "solid-js": ">=2.0.0-0 <3.0.0", "@solidjs/web": ">=2.0.0-0 <3.0.0" }
    }
  });
  const result = selectRow({
    packageName: "@tanstack/solid-router",
    packument: pkg,
    family: TANSTACK,
    status: "official",
    solidTarget: "solid2",
    catalog,
    auditedSolid1: AUDITED_SOLID_1
  });
  assert.equal(result.kind, "row");
  assert.deepEqual(result.row.probes.map(probe => probe.kind), ["floor", "head"]);
  assert.equal(result.row.probes[0].solid["solid-js"], "2.0.0-rc.0");
  assert.equal(result.row.probes[0].solid["@solidjs/web"], "2.0.0-rc.0");
  assert.equal(result.row.probes[1].solid["solid-js"], "2.0.0-rc.1");
  assert.equal(result.row.probes[1].solid["@solidjs/web"], "2.0.0-rc.1");
});
