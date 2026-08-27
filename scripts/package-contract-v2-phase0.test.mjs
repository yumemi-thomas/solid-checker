import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "vitest";

import {
  assertSuccessfulCacheDisabledMeasurements,
  assertFrozenBaseline,
  classifyVerificationRows,
  freezeFixture,
  measureContract,
  schemaMetrics,
  typeFactsIdentityFromCargo
} from "./package-contract-v2-phase0.mjs";

const readJson = path => JSON.parse(readFileSync(path));

describe("Phase 0 baseline", () => {
  test("validates the immutable historical report without current dependency pins", () => {
    const baseline = readJson("benchmarks/package-contract-v2/phase0/baseline.json");
    assert.doesNotThrow(() => assertFrozenBaseline(baseline));

    const corrupted = structuredClone(baseline);
    corrupted.fixtureFreeze.fixtures[0].treeSha256 = "0".repeat(64);
    assert.throws(() => assertFrozenBaseline(corrupted), /tree hash is inconsistent/);
  });

  test("uses the source-manifest digest for repatriated Type Facts", () => {
    const cargo = 'typefacts = { path = "crates/typefacts" }';
    const buildInfo = JSON.stringify({ sourceDigest: "a".repeat(64) });
    assert.equal(
      typeFactsIdentityFromCargo(cargo, buildInfo),
      `source-manifest-sha256:${"a".repeat(64)}`
    );
  });

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
    assert.doesNotThrow(() =>
      assertSuccessfulCacheDisabledMeasurements({
        generation: {
          command: ["env", "SOLID_CHECKER_GATE_CACHE=0", "bun", "scripts/ecosystem-benchmark/run.mjs"],
          exitCode: 0
        }
      })
    );
    assert.throws(
      () =>
        assertSuccessfulCacheDisabledMeasurements({
          generation: { command: ["bun", "scripts/ecosystem-benchmark/run.mjs"], exitCode: 0 }
        }),
      /generation is not a successful cache-disabled measurement/
    );
    assert.throws(
      () =>
        assertSuccessfulCacheDisabledMeasurements({
          generation: {
            command: ["env", "SOLID_CHECKER_GATE_CACHE=0", "bun", "scripts/ecosystem-benchmark/run.mjs"],
            exitCode: 1
          }
        }),
      /generation is not a successful cache-disabled measurement/
    );
  });
});
