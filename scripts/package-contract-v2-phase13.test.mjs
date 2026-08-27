import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  assertPhase13Conformance,
  readPhase13Conformance,
  readPublishedAudit
} from "./package-contract-v2-phase13.mjs";

describe("Phase 13 Solid 2 RC.3 conformance", () => {
  test("covers every row with exact authority and all six refusal-safe fixtures", () => {
    assert.doesNotThrow(() =>
      assertPhase13Conformance(readPhase13Conformance(), readPublishedAudit())
    );
  });

  test("refuses missing local uncertainty and negative proof from probe absence", () => {
    const missingOpenDomain = structuredClone(readPhase13Conformance());
    missingOpenDomain.rows[0].normalized.openDomains = [];
    assert.throws(
      () => assertPhase13Conformance(missingOpenDomain, readPublishedAudit()),
      /exact open domains/
    );

    const falseNegative = structuredClone(readPhase13Conformance());
    falseNegative.rows[0].observation.absenceIsNegativeProof = true;
    assert.throws(
      () => assertPhase13Conformance(falseNegative, readPublishedAudit()),
      /missing observation cannot be negative proof/
    );
  });

  test("keeps experimental and mixed-framework refusal boundaries explicit", () => {
    const stableByDefault = structuredClone(readPhase13Conformance());
    stableByDefault.rows[13].stability = "stable";
    assert.throws(
      () => assertPhase13Conformance(stableByDefault, readPublishedAudit()),
      /Expected values to be strictly equal/
    );

    const nameOnlyFramework = structuredClone(readPhase13Conformance());
    nameOnlyFramework.rows[15].authorityCases = ["@formkit/auto-animate"];
    assert.throws(
      () => assertPhase13Conformance(nameOnlyFramework, readPublishedAudit()),
      /Expected values to be strictly deep-equal/
    );
  });
});
