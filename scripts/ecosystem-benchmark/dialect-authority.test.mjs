import { test } from "vitest";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildDialectAuthorityCoverage, loadAuditedArchives } from "./lib/dialect-authority.mjs";

// Hand-built pins in the shape the module expects. Two dialects, one of which
// audited nothing: the asymmetry the corpus had until 2026-09-12 (solid-v1's
// authority was empty) and the one that makes "audited nothing" distinguishable
// from "no such dialect". The checked-in file is tested separately below.
const PINS = {
  schemaVersion: 1,
  dialects: [
    { id: "solid-v1", archives: [], negativeRowCount: 0 },
    {
      id: "solid-v2",
      archives: [
        {
          name: "solid-js",
          version: "2.0.0-rc.3",
          integrity: "sha512-pin",
          manifestSha256: "digest"
        }
      ],
      negativeRowCount: 47
    }
  ]
};

function makeRow(overrides) {
  return {
    probeId: overrides.probeId,
    status: overrides.status ?? "official",
    solidTarget: overrides.solidTarget,
    installedVersions: overrides.installedVersions ?? {}
  };
}

test("the checked-in pins load and name the archives the Rust tables audit", () => {
  const document = loadAuditedArchives();
  assert.equal(document.schemaVersion, 1);
  const solid2 = document.dialects.find(dialect => dialect.id === "solid-v2");
  // The mirror carries one entry per dialect the Rust tables define, so
  // retiring the 1.x dialect (ADR 0110) removed its entry here too. Asserting
  // the whole id list rather than only the surviving entry is deliberate: an
  // entry appearing or vanishing is exactly what this pin exists to catch,
  // and a `find` that silently returns `undefined` would not.
  assert.deepEqual(
    document.dialects.map(dialect => dialect.id).sort(),
    ["solid-v2"]
  );
  assert.deepEqual(
    solid2.archives.map(archive => `${archive.name}@${archive.version}`).sort(),
    ["@solidjs/signals@2.0.0-rc.3", "@solidjs/web@2.0.0-rc.3", "solid-js@2.0.0-rc.3"]
  );
});

test("a pin file that parses but pins nothing is refused, not read as full coverage", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-authority-"));
  const write = (name, body) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(body));
    return path;
  };
  assert.throws(
    () => loadAuditedArchives(write("version.json", { schemaVersion: 2, dialects: [] })),
    /unsupported schemaVersion 2/
  );
  assert.throws(
    () => loadAuditedArchives(write("empty.json", { schemaVersion: 1, dialects: [] })),
    /no dialects/
  );
  assert.throws(
    () =>
      loadAuditedArchives(
        write("unnamed.json", { schemaVersion: 1, dialects: [{ id: "solid-v2" }] })
      ),
    /has no archives array/
  );
  assert.throws(
    () =>
      loadAuditedArchives(
        write("untupled.json", {
          schemaVersion: 1,
          dialects: [{ id: "solid-v2", archives: [{ name: "solid-js" }] }]
        })
      ),
    /no name@version/
  );
});

test("coverage counts a row only when its own dialect audited the version installed", () => {
  const results = [
    makeRow({
      probeId: "covered",
      solidTarget: "solid2",
      installedVersions: { "solid-js": "2.0.0-rc.3", corvu: "0.7.0" }
    }),
    makeRow({
      probeId: "unaudited-prerelease",
      solidTarget: "solid2",
      installedVersions: { "solid-js": "2.0.0-rc.0" }
    }),
    // The pinned tuple, installed under a dialect whose own authority does not
    // carry it. The identity gate refuses exactly this, so the coverage number
    // must too -- otherwise a 1.x row would be counted as answerable by rows
    // that can never be consulted for it.
    makeRow({
      probeId: "right-version-wrong-dialect",
      solidTarget: "solid1",
      installedVersions: { "solid-js": "2.0.0-rc.3" }
    }),
    makeRow({
      probeId: "solid-1",
      solidTarget: "solid1",
      installedVersions: { "solid-js": "1.9.14" }
    }),
    // Reported but never mixed in, exactly as every other corpus figure treats
    // a fork or lookalike.
    makeRow({
      probeId: "supplemental",
      status: "supplemental",
      solidTarget: "solid2",
      installedVersions: { "solid-js": "2.0.0-rc.3" }
    })
  ];

  const coverage = buildDialectAuthorityCoverage(results, PINS);
  assert.equal(coverage.rows, 4);
  assert.equal(coverage.rowsCovered, 1);
  assert.equal(coverage.coveragePercentage, 25);
  assert.equal(coverage.rowsWithoutDialect, 0);
  assert.deepEqual(coverage.byDialect, [
    { id: "solid-v1", auditedArchives: 0, negativeRowCount: 0, rows: 2, rowsCovered: 0 },
    { id: "solid-v2", auditedArchives: 1, negativeRowCount: 47, rows: 2, rowsCovered: 1 }
  ]);
  // Only audited *names* are listed -- an install tree's other packages say
  // nothing about the authority's reach -- and each says whether it is the pin.
  assert.deepEqual(coverage.installedVersions, [
    { package: "solid-js", version: "2.0.0-rc.3", rows: 2, audited: true },
    { package: "solid-js", version: "1.9.14", rows: 1, audited: false },
    { package: "solid-js", version: "2.0.0-rc.0", rows: 1, audited: false }
  ]);
  assert.deepEqual(coverage.archives, [
    { dialect: "solid-v2", package: "solid-js", version: "2.0.0-rc.3" }
  ]);
});

test("a solid target no dialect claims is counted apart, never given a default", () => {
  const coverage = buildDialectAuthorityCoverage(
    [
      makeRow({
        probeId: "unknown-target",
        solidTarget: "solid3",
        installedVersions: { "solid-js": "2.0.0-rc.3" }
      })
    ],
    PINS
  );
  assert.equal(coverage.rows, 1);
  assert.equal(coverage.rowsCovered, 0);
  assert.equal(coverage.rowsWithoutDialect, 1);
  assert.equal(
    coverage.installedVersions.length,
    0,
    "a row attributed to no dialect contributes no install evidence either"
  );
});
