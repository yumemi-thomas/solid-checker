import { test } from "vitest";
import assert from "node:assert/strict";

import { discover } from "./discover.mjs";
import { Registry } from "./lib/registry.mjs";
import { validateManifest, serializeManifest } from "./lib/manifest.mjs";

// A small, self-consistent fake registry exercised through the REAL
// families.mjs table (discover.mjs always classifies through the real
// `classifyPackage`/`familyById`, never through an injected substitute — see
// the comment on `discover`'s `families` parameter), so this fixture has to
// satisfy the real FAMILIES' `minimumPackages` completeness requirement, not
// a trimmed-down stand-in. That is why the official-solid and kobalte org
// listings below carry their full real membership rather than just the two
// or three packages each test actually cares about.

const NOW = "2026-08-21T09:00:00.000Z";

function sha(label) {
  return `sha512-${Buffer.from(label).toString("base64url")}`;
}

function versionDoc({ dependencies, integrity, deprecated }) {
  const doc = { dist: { integrity } };
  if (dependencies && Object.keys(dependencies).length > 0) doc.dependencies = dependencies;
  if (deprecated) doc.deprecated = deprecated;
  return doc;
}

// Compatible on both solid1 (audited at 1.9.14) and solid2 (any of the fake
// solid-js 2.x line) via one declared range, so most "just needs to exist"
// fixture packages need only a single published version.
const BOTH_TARGETS_RANGE = "^1.6.0 || ^2.0.0-beta.10";

function soloPackument(name, version = "1.0.0") {
  return {
    "dist-tags": { latest: version },
    versions: {
      [version]: versionDoc({ dependencies: { "solid-js": BOTH_TARGETS_RANGE }, integrity: sha(`${name}@${version}`) })
    }
  };
}

// Real official-solid minimumPackages, minus the four given a specific test
// role below (solid-js, @solidjs/router, @solidjs/meta, @solidjs/web,
// @solidjs/signals) — these exist purely so validateManifest's
// minimum-package completeness check has something to find.
const FILLER_OFFICIAL_SOLID = [
  "@solidjs/element",
  "@solidjs/h",
  "@solidjs/html",
  "@solidjs/image",
  "@solidjs/start",
  "@solidjs/start-devtools",
  "@solidjs/testing-library",
  "@solidjs/universal",
  "@solidjs/vite-plugin",
  "@solidjs/vite-plugin-nitro-2"
];
const FILLER_KOBALTE = ["@kobalte/solidbase", "@kobalte/tailwindcss", "@kobalte/themes", "@kobalte/utils", "@kobalte/vanilla-extract"];

const PACKUMENTS = {
  "solid-js": {
    "dist-tags": { latest: "1.9.14", next: "2.0.0-rc.1", beta: "1.10.0-beta.0" },
    versions: {
      "1.9.14": versionDoc({ integrity: sha("solid-js@1.9.14") }),
      "2.0.0-beta.10": versionDoc({ integrity: sha("solid-js@2.0.0-beta.10") }),
      "2.0.0-beta.17": versionDoc({ integrity: sha("solid-js@2.0.0-beta.17") }),
      "2.0.0-rc.1": versionDoc({ integrity: sha("solid-js@2.0.0-rc.1") })
    }
  },
  "@solidjs/web": {
    "dist-tags": { next: "2.0.0-rc.1" },
    versions: {
      "2.0.0-beta.10": versionDoc({ integrity: sha("@solidjs/web@2.0.0-beta.10") }),
      "2.0.0-rc.1": versionDoc({ integrity: sha("@solidjs/web@2.0.0-rc.1") })
    }
  },
  "@solidjs/signals": {
    "dist-tags": { next: "2.0.0-rc.1" },
    versions: {
      "2.0.0-beta.10": versionDoc({ integrity: sha("@solidjs/signals@2.0.0-beta.10") }),
      "2.0.0-rc.1": versionDoc({ integrity: sha("@solidjs/signals@2.0.0-rc.1") })
    }
  },
  "@solidjs/router": soloPackument("@solidjs/router", "1.2.0"),
  "@solidjs/meta": soloPackument("@solidjs/meta", "0.29.4"),

  "@kobalte/core": soloPackument("@kobalte/core", "0.13.6"),
  // The fork: matches the "kobalte" search term but is not under the
  // @kobalte scope, so classifyPackage must land it in "supplemental".
  "kobalte-community-forms": soloPackument("kobalte-community-forms", "3.0.0"),

  "@solid-primitives/scheduled": soloPackument("@solid-primitives/scheduled"),
  "@solid-primitives/resize-observer": soloPackument("@solid-primitives/resize-observer"),
  // Declares solid-js only for the 1.x line: solid1 gets a row, solid2 must
  // produce an explicit "no compatible release" exclusion rather than a
  // silent omission.
  "@solid-primitives/storage": {
    "dist-tags": { latest: "1.4.0" },
    versions: { "1.4.0": versionDoc({ dependencies: { "solid-js": "^1.6.0" }, integrity: sha("@solid-primitives/storage@1.4.0") }) }
  },
  // "@solid-primitives/broken-link" is deliberately absent from PACKUMENTS —
  // its packument() call must resolve to null (a 404), never be dropped
  // silently.

  corvu: soloPackument("corvu", "0.4.0"),
  // The most important fixture in this file: an exact pin to
  // "2.0.0-beta.17" (not a caret range) so its compatible set is that one
  // version alone, even though a newer "2.0.0-rc.1" exists in the fake
  // solid-js catalog. This is the beta-only-stays-pinned case.
  "@corvu/dialog": {
    "dist-tags": { latest: "0.1.0" },
    versions: {
      "0.1.0": versionDoc({ dependencies: { "solid-js": "^1.6.0" }, integrity: sha("@corvu/dialog@0.1.0") }),
      "1.0.0-beta.5": versionDoc({ dependencies: { "solid-js": "2.0.0-beta.17" }, integrity: sha("@corvu/dialog@1.0.0-beta.5") })
    }
  },

  // The Solid adapter: declares solid-js on separate 1.x- and 2.x-oriented
  // releases, so it must become a row (never an exclusion) on both targets.
  "@tanstack/solid-query": {
    "dist-tags": { latest: "5.50.0" },
    versions: {
      "5.50.0": versionDoc({ dependencies: { "solid-js": "^1.6.0" }, integrity: sha("@tanstack/solid-query@5.50.0") }),
      "6.0.0-beta.1": versionDoc({ dependencies: { "solid-js": "^2.0.0-beta.10" }, integrity: sha("@tanstack/solid-query@6.0.0-beta.1") })
    }
  },
  // The non-Solid sibling: no SOLID_RUNTIME_PACKAGES range declared anywhere
  // in its packument. requireSolidDependency must exclude it with
  // "no-solid-dependency" on both targets and it must never reach `rows`.
  "@tanstack/react-query": {
    "dist-tags": { latest: "5.50.0" },
    versions: {
      "5.50.0": versionDoc({ dependencies: { "@tanstack/query-core": "^5.50.0" }, integrity: sha("@tanstack/react-query@5.50.0") })
    }
  },

  "solid-devtools": soloPackument("solid-devtools", "0.29.2"),
  "solid-recharts": soloPackument("solid-recharts", "1.0.1"),
  "motion-solidjs": soloPackument("motion-solidjs", "0.6.0")
};

for (const name of FILLER_OFFICIAL_SOLID) PACKUMENTS[name] = soloPackument(name);
for (const name of FILLER_KOBALTE) PACKUMENTS[name] = soloPackument(name);

const ORG_LISTINGS = {
  solidjs: Object.fromEntries(
    ["@solidjs/router", "@solidjs/meta", "@solidjs/web", "@solidjs/signals", ...FILLER_OFFICIAL_SOLID].map(name => [name, "read"])
  ),
  kobalte: Object.fromEntries(["@kobalte/core", ...FILLER_KOBALTE].map(name => [name, "read"])),
  "solid-primitives": {
    "@solid-primitives/scheduled": "read",
    "@solid-primitives/resize-observer": "read",
    "@solid-primitives/storage": "read",
    "@solid-primitives/broken-link": "read"
  },
  corvu: { "@corvu/dialog": "read" },
  // Deliberately empty: exercises the "org listing that returned nothing"
  // limitations trigger. "corvu" itself is still discovered via the
  // family's explicit `packages` entry, not this scope.
  "corvu-next": {},
  tanstack: { "@tanstack/solid-query": "read", "@tanstack/react-query": "read" },
  // Also deliberately empty, for the same reason.
  "solid-devtools": {}
};

const SEARCH_RESULTS = {
  kobalte: ["@kobalte/core", "kobalte-community-forms"],
  corvu: []
};

function jsonResponse(status, body) {
  return {
    status,
    ok: status >= 200 && status < 300,
    async json() {
      return body;
    }
  };
}

function makeFetchImpl(calls) {
  return async function fakeFetch(url) {
    calls.push(url);
    const parsed = new URL(url);

    const orgMatch = /^\/-\/org\/([^/]+)\/package$/.exec(parsed.pathname);
    if (orgMatch) {
      const scope = decodeURIComponent(orgMatch[1]);
      const listing = ORG_LISTINGS[scope];
      if (!listing) return jsonResponse(404, null);
      return jsonResponse(200, listing);
    }

    if (parsed.pathname === "/-/v1/search") {
      const text = parsed.searchParams.get("text");
      const objects = (SEARCH_RESULTS[text] ?? []).map(name => ({ package: { name } }));
      return jsonResponse(200, { objects, total: objects.length });
    }

    // Packument request: invert registry.mjs's encodePackageName exactly.
    const name = decodeURIComponent(parsed.pathname.slice(1));
    const packument = PACKUMENTS[name];
    if (!packument) return jsonResponse(404, null);
    return jsonResponse(200, packument);
  };
}

function buildRegistry(calls) {
  return new Registry({
    registry: "https://registry.example.test",
    fetchImpl: makeFetchImpl(calls),
    concurrency: 6
  });
}

function findRow(manifest, packageName, solidTarget) {
  return manifest.rows.find(row => row.package === packageName && row.solidTarget === solidTarget);
}

function findExclusion(manifest, packageName, solidTarget) {
  return manifest.exclusions.find(exclusion => exclusion.package === packageName && exclusion.solidTarget === solidTarget);
}

function findSupplemental(manifest, packageName, solidTarget) {
  return manifest.supplemental.find(row => row.package === packageName && row.solidTarget === solidTarget);
}

test("discover touches only the injected fetchImpl, never the global fetch", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    throw new Error("global fetch must never be invoked by discover()");
  };
  try {
    const calls = [];
    const manifest = await discover({ registry: buildRegistry(calls), now: NOW });
    assert.ok(calls.length > 0, "the injected fetchImpl should have been used");
    assert.ok(manifest.rows.length > 0);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("discover is deterministic across repeated runs with the same registry snapshot and now", async () => {
  const manifestA = await discover({ registry: buildRegistry([]), now: NOW });
  const manifestB = await discover({ registry: buildRegistry([]), now: NOW });
  assert.equal(serializeManifest(manifestA), serializeManifest(manifestB));
});

test("the discovered manifest passes validateManifest with zero problems", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  const problems = validateManifest(manifest);
  assert.deepEqual(problems, [], problems.join("\n"));
});

test("a beta-only package keeps its single exact beta probe and is not substituted by a newer rc", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  const row = findRow(manifest, "@corvu/dialog", "solid2");
  assert.ok(row, "expected a @corvu/dialog solid2 row");
  assert.equal(row.version, "1.0.0-beta.5");
  assert.deepEqual(row.compatibleSolidVersions, { "solid-js": ["2.0.0-beta.17"] });
  assert.equal(row.probes.length, 1);
  assert.equal(row.probes[0].kind, "only");
  assert.equal(row.probes[0].channel, "beta");
  assert.deepEqual(row.probes[0].solid, { "solid-js": "2.0.0-beta.17" });
});

test("a package with no compatible release on one target produces an explicit exclusion there", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  assert.ok(findRow(manifest, "@solid-primitives/storage", "solid1"), "solid1 should still be a row");
  const exclusion = findExclusion(manifest, "@solid-primitives/storage", "solid2");
  assert.ok(exclusion, "expected an explicit solid2 exclusion, not a silent omission");
  assert.equal(exclusion.reason, "no-compatible-release");
  assert.equal(findRow(manifest, "@solid-primitives/storage", "solid2"), undefined);
});

test("a TanStack package with no declared Solid dependency is excluded and never reaches rows", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  for (const solidTarget of ["solid1", "solid2"]) {
    const exclusion = findExclusion(manifest, "@tanstack/react-query", solidTarget);
    assert.ok(exclusion, `expected a ${solidTarget} exclusion for @tanstack/react-query`);
    assert.equal(exclusion.reason, "no-solid-dependency");
    assert.equal(findRow(manifest, "@tanstack/react-query", solidTarget), undefined);
  }
  // The Solid adapter sibling, by contrast, must become a row on both targets.
  assert.ok(findRow(manifest, "@tanstack/solid-query", "solid1"));
  assert.ok(findRow(manifest, "@tanstack/solid-query", "solid2"));
});

test("a fork name lands in supplemental with status supplemental and never in rows", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  for (const solidTarget of ["solid1", "solid2"]) {
    const row = findSupplemental(manifest, "kobalte-community-forms", solidTarget);
    assert.ok(row, `expected a supplemental entry for kobalte-community-forms on ${solidTarget}`);
    assert.equal(row.status, "supplemental");
    assert.equal(row.family, "kobalte");
    assert.equal(findRow(manifest, "kobalte-community-forms", solidTarget), undefined);
  }
});

test("a null packument adds a limitations sentence instead of being dropped silently", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  assert.ok(
    manifest.limitations.some(line => line.includes("@solid-primitives/broken-link")),
    manifest.limitations.join("\n")
  );
  // It still shows up as an explicit exclusion, not an omission.
  for (const solidTarget of ["solid1", "solid2"]) {
    const exclusion = findExclusion(manifest, "@solid-primitives/broken-link", solidTarget);
    assert.ok(exclusion);
    assert.equal(exclusion.reason, "not-published");
  }
});

test("an empty org listing adds a limitations sentence", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  assert.ok(
    manifest.limitations.some(line => line.includes('scope "corvu-next"')),
    manifest.limitations.join("\n")
  );
});

test("official rows and forks are never mixed up: @kobalte/core is official, not supplemental", async () => {
  const manifest = await discover({ registry: buildRegistry([]), now: NOW });
  assert.ok(findRow(manifest, "@kobalte/core", "solid1"));
  assert.equal(findSupplemental(manifest, "@kobalte/core", "solid1"), undefined);
});
