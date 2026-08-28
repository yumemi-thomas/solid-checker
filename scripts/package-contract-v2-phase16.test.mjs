import assert from "node:assert/strict";
import { test } from "vitest";

import { buildPhase16Report, buildRefusalReport } from "./package-contract-v2-phase16.mjs";

function sources() {
  const results = [
    ...Array.from({ length: 17 }, (_, index) => ({
      probeId: `pkg-${index}@1|solid2|only`,
      package: index === 0 ? "solid-js" : index === 1 ? "@solidjs/signals" : index === 2 ? "@solidjs/web" : `pkg-${index}`,
      version: index < 3 ? "2.0.0-rc.3" : "1.0.0",
      family: index < 3 ? "official-solid" : "solid-primitives",
      status: "official",
      outcome: "success",
      generationDurationMs: index + 1
    })),
    { probeId: "partial", package: "partial", version: "1", family: "solid-primitives", status: "official", outcome: "partial-success", refusedEntrypoints: 1, generationDurationMs: 20 },
    { probeId: "failed", package: "failed", version: "1", family: "kobalte", status: "official", outcome: "failure", class: "export-kind-unresolved", signature: "exact export kind is open", generationDurationMs: 21 }
  ];
  const ecosystem = {
    scope: { kind: "full", probesRun: results.length },
    results,
    combined: {
      contractContent: {
        unknownByDomain: { callbacks: 1, recursiveValue: 1 },
        probesFullyProven: 0,
        wireBytes: { canonicalMain: { count: 18 } }
      }
    },
    finishedAt: "2026-08-28T00:00:00.000Z"
  };
  const phase13 = { rows: Array.from({ length: 16 }, (_, index) => ({ id: `row-${index}`, normalized: { openDomains: ["browser-observation"] } })) };
  return { ecosystem, phase13 };
}

test("refusal report keeps failure, partial entrypoint, and local-domain causes separate", () => {
  const { ecosystem, phase13 } = sources();
  const report = buildRefusalReport(ecosystem, phase13);
  assert.equal(report.generatedProposalFailures.length, 1);
  assert.equal(report.partialCaseRefusals.length, 1);
  assert.equal(report.openClaimDomains.length, 2);
  assert.equal(report.missingEvidenceIsNegativeProof, false);
});

test("coverage gates refuse a regression without changing semantic proof", () => {
  const { ecosystem, phase13 } = sources();
  const accepted = {
    corpus: { receiptIssuedArtifactCases: 24 },
    compactness: {
      canonicalMainBytes: { count: 24, p50: 100, p95: 200, max: 300 },
      proofEvidenceBytes: { count: 24 },
      acceptanceReceiptBytes: { count: 24 },
      rawEvidenceRetainedByOrdinaryAnalysis: 0
    },
    performance: {
      acceptedCorpusLoadNs: { p95: 1 },
      normalizedQueryNsPerExport: { p95: 1 },
      memory: { postLoadPeakResidentKiB: 1 }
    },
    ordinaryAnalysis: {
      input: "AcceptedContractIndex / receipt-validated normalized semantics",
      rawSidecarBytes: 0,
      packageCodeExecution: false,
      networkAccess: false,
      queryFileReads: false
    }
  };
  const manifest = { rows: ["corvu", "kobalte", "motion-solidjs", "official-solid", "solid-devtools", "solid-primitives", "solid-recharts", "tanstack"].map(family => ({ family })) };
  const syntheticCorpus = { fixtures: Array.from({ length: 39 }, () => "fixture") };
  const historicalVerification = { phaseWallMs: { probe: {} } };
  const probeExecution = {
    semanticAcceptance: false,
    iterations: 2,
    millisecondsPerIsolatedSession: { count: 2, p50: 1, p95: 1, max: 1 }
  };
  assert.doesNotThrow(() => buildPhase16Report({ ecosystem, historicalVerification, manifest, syntheticCorpus, phase13, accepted, probeExecution }));
  const regressed = structuredClone(ecosystem);
  for (const result of regressed.results.slice(0, 4)) result.outcome = "failure";
  assert.throws(
    () => buildPhase16Report({ ecosystem: regressed, historicalVerification, manifest, syntheticCorpus, phase13, accepted, probeExecution }),
    /generatable coverage fell/
  );
});
