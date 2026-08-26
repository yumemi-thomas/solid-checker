import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "vitest";

import {
  buildBaseline,
  classifyVerificationRows,
  freezeFixture,
  measureContract,
  schemaMetrics
} from "./package-contract-v2-phase0.mjs";

const readJson = path => JSON.parse(readFileSync(path));

describe("Phase 0 baseline", () => {
  test("pins the legacy schema structure", () => {
    assert.deepEqual(schemaMetrics(readJson("schema/solid-reactivity.schema.json")), {
      definitions: 6,
      namedProperties: 69,
      requiredNames: 47,
      refs: 23,
      oneOf: 9,
      anyOf: 4,
      allOf: 1,
      enumDeclarations: 11,
      enumValues: 35,
      maximumObjectDepth: 13
    });
  });

  test("classifies every selected ecosystem row exactly once", () => {
    const rows = classifyVerificationRows(
      readJson("benchmarks/ecosystem/verification-report.json"),
      readJson("benchmarks/ecosystem/report.json")
    );
    assert.equal(rows.length, 418);
    assert.equal(new Set(rows.map(row => row.probeId)).size, 418);
    assert.equal(rows.filter(row => row.outcome === "verified").length, 309);
    assert.equal(rows.filter(row => row.owner === "schema").length, 56);
    assert.equal(rows.filter(row => row.owner === "probe").length, 30);
    assert.equal(rows.filter(row => row.owner === "type-facts").length, 11);
    assert.ok(rows.every(row => row.reason.length > 0));
  });

  test("measures compact, expanded, and evidence bytes independently", () => {
    const result = measureContract("pkg/contracts/bundled/solid-v1/solid-primitives-debounce.json");
    assert.deepEqual({
      prettyBytes: result.prettyBytes,
      minifiedBytes: result.minifiedBytes,
      minifiedExpandedBytes: result.minifiedExpandedBytes,
      inlineEvidenceDeltaBytes: result.inlineEvidenceDeltaBytes,
      summaries: result.summaries,
      entrypoints: result.entrypoints,
      expandedExports: result.expandedExports
    }, {
      prettyBytes: 1083,
      minifiedBytes: 692,
      minifiedExpandedBytes: 883,
      inlineEvidenceDeltaBytes: 195,
      summaries: 1,
      entrypoints: 1,
      expandedExports: 2
    });
  });

  test("fixture freeze includes ignored semantic inputs", () => {
    const fixture = freezeFixture("declaration-sibling-reach", "test");
    assert.ok(fixture.files.some(file => file.path.includes("/node_modules/")));
    assert.match(fixture.treeSha256, /^[0-9a-f]{64}$/);
  });

  test("requires successful cache-disabled measurements", () => {
    const baseline = buildBaseline({ loadIterations: 2, queryIterations: 2_000 });
    assert.equal(baseline.ecosystem.classifications.rows.length, 418);
    assert.equal(baseline.fixtureFreeze.fixtureCount, 13);
    assert.equal(baseline.rc3Audit.integrityVerified, true);
    assert.equal(baseline.rc3Audit.allConcreteExportTargetsExist, true);
    assert.ok(
      baseline.measurements.ecosystemGeneration.command.includes("SOLID_CHECKER_GATE_CACHE=0")
    );
  });
});
