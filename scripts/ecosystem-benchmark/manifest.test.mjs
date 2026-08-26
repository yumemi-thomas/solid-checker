import { test } from "vitest";
import assert from "node:assert/strict";

import {
  MANIFEST_SCHEMA_VERSION,
  validateManifest,
  serializeManifest,
  sortRows,
  diffManifests,
  manifestStats
} from "./lib/manifest.mjs";

// A minimal, self-consistent family table injected via the `{ families }`
// option so these tests never depend on the real families.mjs table (owned
// and authored elsewhere). It exercises the same shape manifest.mjs expects:
// scopes/packages for official classification, searchTerms for supplemental
// fallback, and minimumPackages for completeness.
const FAKE_FAMILIES = [
  {
    id: "alpha",
    label: "Alpha",
    order: 0,
    scopes: ["alpha"],
    packages: ["alpha-core"],
    supplementalScopes: [],
    searchTerms: ["alpha"],
    requireSolidDependency: false,
    minimumPackages: ["alpha-core"]
  },
  {
    id: "beta",
    label: "Beta",
    order: 1,
    scopes: ["beta"],
    packages: [],
    supplementalScopes: [],
    searchTerms: ["beta"],
    requireSolidDependency: false,
    minimumPackages: []
  }
];

function validManifest() {
  return {
    schemaVersion: MANIFEST_SCHEMA_VERSION,
    generatedAt: "2026-08-21T00:00:00.000Z",
    registry: "https://registry.npmjs.org",
    auditedSolid1: "1.9.14",
    auditedSolid2: "2.0.0-rc.3",
    solidReleases: {
      "solid-js": {
        distTags: { latest: "1.9.14", next: "2.0.0-rc.1" },
        v1: ["1.9.14"],
        v2: ["2.0.0-beta.10", "2.0.0-rc.1"]
      }
    },
    rows: [
      {
        family: "alpha",
        status: "official",
        package: "alpha-core",
        solidTarget: "solid1",
        version: "1.0.0",
        distTags: ["latest"],
        integrity: "sha512-AAA",
        deprecated: null,
        dependencies: { "solid-js": "^1.6.0" },
        peerDependencies: {},
        optionalDependencies: {},
        compatibleSolidVersions: { "solid-js": ["1.9.14"] },
        unparsedRanges: [],
        probes: [{ id: "alpha-core@1.0.0|solid1|only", kind: "only", channel: "stable", solid: { "solid-js": "1.9.14" } }]
      },
      {
        family: "alpha",
        status: "official",
        package: "alpha-core",
        solidTarget: "solid2",
        version: "2.0.0",
        distTags: ["next"],
        integrity: "sha512-BBB",
        deprecated: null,
        dependencies: { "solid-js": "^2.0.0-beta.10" },
        peerDependencies: {},
        optionalDependencies: {},
        compatibleSolidVersions: { "solid-js": ["2.0.0-beta.10", "2.0.0-rc.1"] },
        unparsedRanges: [],
        probes: [
          { id: "alpha-core@2.0.0|solid2|floor", kind: "floor", channel: "beta", solid: { "solid-js": "2.0.0-beta.10" } },
          { id: "alpha-core@2.0.0|solid2|head", kind: "head", channel: "rc", solid: { "solid-js": "2.0.0-rc.1" } }
        ]
      }
    ],
    exclusions: [
      {
        family: "beta",
        status: "supplemental",
        package: "beta-thing",
        solidTarget: "solid1",
        reason: "not-published",
        detail: "never released"
      }
    ],
    supplemental: [],
    limitations: []
  };
}

function clone(value) {
  return structuredClone(value);
}

test("validateManifest accepts a well-formed manifest", () => {
  const problems = validateManifest(validManifest(), { families: FAKE_FAMILIES });
  assert.deepEqual(problems, []);
});

test("validateManifest rejects the wrong schemaVersion", () => {
  const manifest = clone(validManifest());
  manifest.schemaVersion = 2;
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes("schemaVersion must be 1")), problems.join("\n"));
});

test("validateManifest rejects a row with an unknown family", () => {
  const manifest = clone(validManifest());
  manifest.rows[0].family = "gamma";
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes('unknown family "gamma"')), problems.join("\n"));
});

test("validateManifest rejects a row whose classifyPackage family disagrees with its declared family", () => {
  const manifest = clone(validManifest());
  // "alpha-core" classifies as "alpha" via the fake table; declaring it under
  // the (otherwise valid) "beta" family must be flagged even though "beta"
  // itself is a known family.
  manifest.rows[0].family = "beta";
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(
    problems.some(problem => problem.includes("disagrees with classifyPackage result")),
    problems.join("\n")
  );
});

test("validateManifest rejects a missing or empty integrity", () => {
  const manifest = clone(validManifest());
  manifest.rows[0].integrity = "";
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes("integrity is missing or empty")), problems.join("\n"));
});

test("validateManifest rejects a version that is a range rather than an exact semver", () => {
  const manifest = clone(validManifest());
  manifest.rows[0].version = "^1.0.0";
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(
    problems.some(problem => problem.includes("is not an exact semver version")),
    problems.join("\n")
  );
});

test("validateManifest rejects a probe whose Solid version is absent from compatibleSolidVersions", () => {
  const manifest = clone(validManifest());
  manifest.rows[0].probes[0].solid["solid-js"] = "9.9.9";
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(
    problems.some(problem => problem.includes("is not in compatibleSolidVersions")),
    problems.join("\n")
  );
});

test("validateManifest rejects a probe count other than 1 or 2", () => {
  const manifest = clone(validManifest());
  manifest.rows[0].probes = [];
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes("expected 1 or 2 probes, got 0")), problems.join("\n"));
});

test("validateManifest rejects duplicate probe ids", () => {
  const manifest = clone(validManifest());
  manifest.rows[1].probes[1].id = manifest.rows[1].probes[0].id;
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes("duplicate probe id")), problems.join("\n"));
});

test("validateManifest rejects rows that are out of deterministic order", () => {
  const manifest = clone(validManifest());
  manifest.rows.reverse();
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(problems.some(problem => problem.includes("is not in deterministic order")), problems.join("\n"));
});

test("validateManifest rejects a required family with zero rows and zero exclusions", () => {
  const manifest = clone(validManifest());
  manifest.exclusions = [];
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(
    problems.some(problem => problem.includes('family "beta" has zero rows and zero exclusions')),
    problems.join("\n")
  );
});

test("validateManifest rejects a family whose minimumPackages are missing on one Solid target", () => {
  const manifest = clone(validManifest());
  manifest.rows.pop(); // drop the alpha-core@solid2 row; no exclusion covers it either
  const problems = validateManifest(manifest, { families: FAKE_FAMILIES });
  assert.ok(
    problems.some(problem => problem.includes('minimum package "alpha-core" is missing') && problem.includes("solid2")),
    problems.join("\n")
  );
});

test("serializeManifest is byte-deterministic across repeated calls", () => {
  const manifest = validManifest();
  assert.equal(serializeManifest(manifest), serializeManifest(clone(manifest)));
});

test("serializeManifest is byte-deterministic regardless of input key order", () => {
  const manifest = validManifest();
  const shuffled = {
    limitations: manifest.limitations,
    supplemental: manifest.supplemental,
    exclusions: manifest.exclusions.map(exclusion => ({
      detail: exclusion.detail,
      reason: exclusion.reason,
      solidTarget: exclusion.solidTarget,
      package: exclusion.package,
      status: exclusion.status,
      family: exclusion.family
    })),
    rows: manifest.rows.map(row => ({
      probes: row.probes.map(probe => ({ solid: probe.solid, channel: probe.channel, kind: probe.kind, id: probe.id })),
      unparsedRanges: row.unparsedRanges,
      compatibleSolidVersions: row.compatibleSolidVersions,
      optionalDependencies: row.optionalDependencies,
      peerDependencies: row.peerDependencies,
      dependencies: row.dependencies,
      deprecated: row.deprecated,
      integrity: row.integrity,
      distTags: row.distTags,
      version: row.version,
      solidTarget: row.solidTarget,
      package: row.package,
      status: row.status,
      family: row.family
    })),
    solidReleases: {
      "solid-js": {
        v2: manifest.solidReleases["solid-js"].v2,
        v1: manifest.solidReleases["solid-js"].v1,
        distTags: { next: "2.0.0-rc.1", latest: "1.9.14" }
      }
    },
    auditedSolid1: manifest.auditedSolid1,
    auditedSolid2: manifest.auditedSolid2,
    registry: manifest.registry,
    generatedAt: manifest.generatedAt,
    schemaVersion: manifest.schemaVersion
  };
  assert.equal(serializeManifest(manifest), serializeManifest(shuffled));
});

test("serializeManifest ends in exactly one trailing newline", () => {
  const text = serializeManifest(validManifest());
  assert.ok(text.endsWith("}\n"));
  assert.ok(!text.endsWith("\n\n"));
});

test("sortRows orders by family order, then package name, then solid1 before solid2", () => {
  const rows = [
    { family: "beta", package: "beta-thing", solidTarget: "solid1" },
    { family: "alpha", package: "zed", solidTarget: "solid2" },
    { family: "alpha", package: "zed", solidTarget: "solid1" },
    { family: "alpha", package: "abc", solidTarget: "solid1" }
  ];
  const sorted = sortRows(rows, { families: FAKE_FAMILIES });
  assert.deepEqual(
    sorted.map(row => `${row.family}/${row.package}/${row.solidTarget}`),
    ["alpha/abc/solid1", "alpha/zed/solid1", "alpha/zed/solid2", "beta/beta-thing/solid1"]
  );
});

test("diffManifests reports an integrity change as its own changed entry even when the version is unchanged", () => {
  const previous = validManifest();
  const next = clone(previous);
  next.rows[0].integrity = "sha512-CHANGED";
  const diff = diffManifests(previous, next, { families: FAKE_FAMILIES });
  assert.deepEqual(diff.added, []);
  assert.deepEqual(diff.removed, []);
  assert.equal(diff.changed.length, 1);
  assert.equal(diff.changed[0].kind, "integrity");
  assert.equal(diff.changed[0].package, "alpha-core");
  assert.equal(diff.changed[0].from, "sha512-AAA");
  assert.equal(diff.changed[0].to, "sha512-CHANGED");
});

test("diffManifests reports added, removed, and version-changed rows", () => {
  const previous = validManifest();
  const next = clone(previous);
  next.rows[0].version = "1.1.0"; // version change on alpha-core@solid1
  const removedRow = next.rows.pop(); // remove alpha-core@solid2
  next.rows.push({ ...clone(removedRow), package: "alpha-new", version: "1.0.0" }); // add alpha-new@solid2

  const diff = diffManifests(previous, next, { families: FAKE_FAMILIES });

  assert.equal(diff.added.length, 1);
  assert.equal(diff.added[0].package, "alpha-new");

  assert.equal(diff.removed.length, 1);
  assert.equal(diff.removed[0].package, "alpha-core");
  assert.equal(diff.removed[0].solidTarget, "solid2");

  const versionChange = diff.changed.find(entry => entry.kind === "version");
  assert.ok(versionChange, JSON.stringify(diff.changed));
  assert.equal(versionChange.package, "alpha-core");
  assert.equal(versionChange.from, "1.0.0");
  assert.equal(versionChange.to, "1.1.0");

  assert.deepEqual(diff.summary, {
    addedCount: 1,
    removedCount: 1,
    changedCount: 1,
    exclusionsAddedCount: 0,
    exclusionsRemovedCount: 0,
    exclusionsChangedCount: 0,
    limitationsAddedCount: 0,
    limitationsRemovedCount: 0
  });
});

test("manifestStats counts rows, probes, and per-family totals", () => {
  const stats = manifestStats(validManifest(), { families: FAKE_FAMILIES });
  assert.equal(stats.rowCount, 2);
  assert.equal(stats.probeCount, 3); // one "only" probe + one floor/head pair
  assert.equal(stats.exclusionCount, 1);
  assert.equal(stats.supplementalCount, 0);

  const alpha = stats.families.find(family => family.family === "alpha");
  const beta = stats.families.find(family => family.family === "beta");
  assert.deepEqual(alpha, { family: "alpha", rowCount: 2, probeCount: 3, exclusionCount: 0, supplementalCount: 0 });
  assert.deepEqual(beta, { family: "beta", rowCount: 0, probeCount: 0, exclusionCount: 1, supplementalCount: 0 });
  assert.deepEqual(stats.families.map(family => family.family), ["alpha", "beta"]);
});

// A refresh can change only which packages are excluded, or only which
// registry gaps were hit. If the printed diff ignores those, a reviewer is
// told "no changes" while `--check` refuses the same file as drifted.
test("diffManifests reports an exclusion appearing, disappearing, and changing reason", () => {
  const base = { schemaVersion: 1, rows: [], exclusions: [], limitations: [] };
  const previous = {
    ...base,
    exclusions: [
      { family: "tanstack", status: "official", package: "@tanstack/gone", solidTarget: "solid1", reason: "no-compatible-release", detail: "d" },
      { family: "tanstack", status: "official", package: "@tanstack/shifted", solidTarget: "solid1", reason: "unparsed-range", detail: "d" }
    ]
  };
  const next = {
    ...base,
    exclusions: [
      { family: "tanstack", status: "official", package: "@tanstack/new", solidTarget: "solid2", reason: "no-solid-dependency", detail: "d" },
      { family: "tanstack", status: "official", package: "@tanstack/shifted", solidTarget: "solid1", reason: "no-compatible-release", detail: "d" }
    ]
  };
  const diff = diffManifests(previous, next);
  assert.deepEqual(diff.exclusions.added.map(entry => entry.package), ["@tanstack/new"]);
  assert.deepEqual(diff.exclusions.removed.map(entry => entry.package), ["@tanstack/gone"]);
  assert.equal(diff.exclusions.changed.length, 1);
  assert.equal(diff.exclusions.changed[0].kind, "exclusion-reason");
  assert.equal(diff.exclusions.changed[0].from, "unparsed-range");
  assert.equal(diff.exclusions.changed[0].to, "no-compatible-release");
  assert.equal(diff.summary.exclusionsAddedCount, 1);
  assert.equal(diff.summary.exclusionsRemovedCount, 1);
  assert.equal(diff.summary.exclusionsChangedCount, 1);
});

test("diffManifests reports limitations appearing and disappearing", () => {
  const base = { schemaVersion: 1, rows: [], exclusions: [] };
  const diff = diffManifests(
    { ...base, limitations: ["gap a", "gap b"] },
    { ...base, limitations: ["gap b", "gap c"] }
  );
  assert.deepEqual(diff.limitations.added, ["gap c"]);
  assert.deepEqual(diff.limitations.removed, ["gap a"]);
  assert.equal(diff.summary.limitationsAddedCount, 1);
  assert.equal(diff.summary.limitationsRemovedCount, 1);
});

test("diffManifests on two identical manifests reports nothing anywhere", () => {
  const manifest = {
    schemaVersion: 1,
    rows: [],
    exclusions: [{ family: "corvu", status: "official", package: "corvu", solidTarget: "solid2", reason: "no-compatible-release", detail: "d" }],
    limitations: ["a gap"]
  };
  const diff = diffManifests(manifest, structuredClone(manifest));
  assert.deepEqual(diff.summary, {
    addedCount: 0,
    removedCount: 0,
    changedCount: 0,
    exclusionsAddedCount: 0,
    exclusionsRemovedCount: 0,
    exclusionsChangedCount: 0,
    limitationsAddedCount: 0,
    limitationsRemovedCount: 0
  });
});

test("diffManifests reports a probe change so --check never refuses a manifest the diff called unchanged", () => {
  // `--check` compares the whole serialized document, so a row whose probes
  // moved is already refused as stale. Leaving probes out of the diff told the
  // reviewer nothing had changed and then told them the file was out of date --
  // the same contradiction that put exclusions and limitations in this diff.
  const row = {
    family: "tanstack",
    package: "@tanstack/solid-router",
    solidTarget: "solid2",
    status: "official",
    version: "2.0.0-rc.1",
    integrity: "sha512-same",
    probes: [
      { kind: "floor", solid: { "solid-js": "2.0.0-rc.0", "@solidjs/web": "2.0.0-rc.1" } },
      { kind: "head", solid: { "solid-js": "2.0.0-rc.1", "@solidjs/web": "2.0.0-rc.1" } }
    ]
  };
  const collapsed = {
    ...row,
    probes: [{ kind: "only", solid: { "solid-js": "2.0.0-rc.1", "@solidjs/web": "2.0.0-rc.1" } }]
  };
  const diff = diffManifests({ rows: [row] }, { rows: [collapsed] });
  assert.equal(diff.summary.changedCount, 1);
  assert.equal(diff.changed[0].kind, "probes");
  assert.match(diff.changed[0].from, /floor=.*solid-js@2\.0\.0-rc\.0/);
  assert.equal(diff.changed[0].to, "only=@solidjs/web@2.0.0-rc.1+solid-js@2.0.0-rc.1");

  // Version and integrity unchanged plus probes unchanged is still no change.
  assert.equal(diffManifests({ rows: [row] }, { rows: [row] }).summary.changedCount, 0);
});
