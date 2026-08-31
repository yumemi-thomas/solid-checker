// Wire-level measurements for stable-schema-v1 generator proposals.
//
// This module deliberately does not normalize contract semantics. Rust owns
// that boundary. The benchmark counts only explicit wire rows: public export
// names, locally open call-domain markers, recursive unknown value leaves, and
// operation kinds. Proof and acceptance are measured from the separate
// proposal plan; absence is never reinterpreted as a negative claim.

import { readFileSync } from "node:fs";

export const CLAIM_DOMAINS = [
  "callbacks",
  "reads",
  "writes",
  "creates",
  "invalidates",
  "throws",
  "returns",
  "cleanups",
  "disposals",
  "recursiveValue"
];

export const BEHAVIORAL_ROW_KINDS = [
  "invoke",
  "return",
  "read",
  "write",
  "invalidate",
  "create",
  "cleanup",
  "dispose"
];

export function isUnknownClaim(value) {
  return value === "unknown" || (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    value.kind === "unknown" &&
    Object.keys(value).length === 1
  );
}

export function emptyDomainCounts() {
  return Object.fromEntries(CLAIM_DOMAINS.map(domain => [domain, 0]));
}

export function emptyBehavioralRows() {
  return Object.fromEntries(BEHAVIORAL_ROW_KINDS.map(kind => [kind, 0]));
}

function artifactCases(entrypoint) {
  return Array.isArray(entrypoint?.cases) ? entrypoint.cases : [entrypoint];
}

function referenceId(reference) {
  if (typeof reference === "string") return reference;
  return typeof reference?.summary === "string" ? reference.summary : null;
}

function hasRecursiveUnknown(value, seen = new Set()) {
  if (isUnknownClaim(value)) return true;
  if (!value || typeof value !== "object" || seen.has(value)) return false;
  seen.add(value);
  if (Array.isArray(value)) return value.some(item => hasRecursiveUnknown(item, seen));
  return Object.entries(value).some(([key, child]) =>
    key !== "closed" && hasRecursiveUnknown(child, seen)
  );
}

export function summarizeContractDocument(contract) {
  if (
    !contract ||
    typeof contract !== "object" ||
    contract.format !== "solid-reactivity-contract" ||
    contract.schemaVersion !== 1 ||
    contract.semanticModelVersion !== 1 ||
    !contract.entrypoints ||
    !contract.summaries
  ) return null;

  const unknownByDomain = emptyDomainCounts();
  const behavioralRows = emptyBehavioralRows();
  let exportsTotal = 0;
  let exportsProven = 0;
  let exportsWithUnknown = 0;
  let exportsWithoutSummary = 0;
  let exportsAllDomainsUnknown = 0;
  let artifactCasesTotal = 0;
  const artifactCaseRecords = [];

  for (const [entrypointName, entrypoint] of Object.entries(contract.entrypoints)) {
    for (const [caseIndex, artifactCase] of artifactCases(entrypoint).entries()) {
      artifactCasesTotal += 1;
      artifactCaseRecords.push({
        entrypoint: entrypointName,
        caseIndex,
        artifact: artifactCase?.artifact ?? null,
        declarations: artifactCase?.declarations ?? null,
        resolution: artifactCase?.resolution ?? null
      });
      for (const reference of Object.values(artifactCase?.exports ?? {})) {
        exportsTotal += 1;
        const summary = contract.summaries[referenceId(reference)];
        if (!summary || typeof summary !== "object") {
          exportsWithoutSummary += 1;
          continue;
        }
        const call = summary.call ?? {};
        const closed = new Set(Array.isArray(call.closed) ? call.closed : []);
        let open = 0;
        for (const domain of CLAIM_DOMAINS.slice(0, -1)) {
          if (!closed.has(domain)) {
            unknownByDomain[domain] += 1;
            open += 1;
          }
        }
        if (hasRecursiveUnknown(summary.shape) || hasRecursiveUnknown(call.operations ?? [])) {
          unknownByDomain.recursiveValue += 1;
          open += 1;
        }
        if (open === 0) exportsProven += 1;
        else {
          exportsWithUnknown += 1;
          if (open === CLAIM_DOMAINS.length) exportsAllDomainsUnknown += 1;
        }
        for (const operation of Array.isArray(call.operations) ? call.operations : []) {
          if (Object.hasOwn(behavioralRows, operation?.kind)) behavioralRows[operation.kind] += 1;
        }
      }
    }
  }

  return {
    entrypointsEmitted: Object.keys(contract.entrypoints).length,
    artifactCasesTotal,
    artifactCases: artifactCaseRecords,
    exportsTotal,
    exportsProven,
    exportsWithUnknown,
    exportsAllDomainsUnknown,
    exportsUnknownOnlyInVariants: 0,
    exportsWithoutSummary,
    unknownByDomain,
    unknownTotal: Object.values(unknownByDomain).reduce((total, count) => total + count, 0),
    behavioralRows
  };
}

export function summarizeReviewPlan(plan) {
  if (
    !plan ||
    typeof plan !== "object" ||
    plan.format !== "solid-checker-contract-proposal-plan" ||
    plan.planVersion !== 1
  ) return null;
  const closure = Array.isArray(plan.closureCandidates) ? plan.closureCandidates : [];
  const proof = Array.isArray(plan.proofCandidates) ? plan.proofCandidates : [];
  const probe = Array.isArray(plan.probeCandidates) ? plan.probeCandidates : [];
  return {
    checklistItems: closure.length + proof.length + probe.length,
    itemsByKind: {
      closureCandidate: closure.length,
      proofCandidate: proof.length,
      probeCandidate: probe.length
    },
    refusedEntrypoints: 0,
    refusedEntrypointNames: [],
    closureNotes: 0,
    closureNoteSamples: [],
    attestedRuntimeNotes: 0,
    attestedRuntimeNoteSamples: []
  };
}

function byteLength(value) {
  return Buffer.byteLength(value);
}

export function summarizeContract({
  contract,
  reviewPlan,
  refusals = null,
  inapplicable = null,
  refusedEntrypointsFromStdout = null,
  mainBytes = null,
  planBytes = null
}) {
  const document = summarizeContractDocument(contract);
  if (!document) {
    return { measured: false, note: "stable-v1 proposal missing or unparsable", fullyProven: null };
  }
  const plan = summarizeReviewPlan(reviewPlan);
  const refusedEntrypoints = refusedEntrypointsFromStdout ?? 0;
  const refusedArtifactCases =
    refusals?.format === "solid-checker-contract-proposal-refusals" &&
    refusals?.refusalVersion === 1 &&
    Array.isArray(refusals.refusals)
      ? refusals.refusals
      : [];
  // Recorded artifact-case dispositions. They are census decisions, never
  // refusals: an entrypoint no consumer can reach as a module asserts nothing
  // about certifiable behavior, so it must not be counted as one.
  const inapplicableArtifactCases = Array.isArray(inapplicable) ? inapplicable : [];
  const operationCount = Object.values(document.behavioralRows).reduce(
    (total, count) => total + count,
    0
  );
  const canonicalMainBytes = byteLength(`${JSON.stringify(contract)}\n`);
  return {
    measured: true,
    // A generator output is a proposal. Even an entirely closed candidate is
    // not proven until Rust verifies proofs and issues a receipt.
    fullyProven: false,
    entrypointsEmitted: document.entrypointsEmitted,
    artifactCasesTotal: document.artifactCasesTotal,
    artifactCases: document.artifactCases,
    entrypointsRefused: refusedEntrypoints,
    artifactCasesRefused: refusedArtifactCases.length,
    artifactCaseRefusals: refusedArtifactCases,
    artifactCasesInapplicable: inapplicableArtifactCases.length,
    artifactCaseInapplicabilities: inapplicableArtifactCases,
    refusedEntrypointNames: [],
    exportsTotal: document.exportsTotal,
    exportsProven: document.exportsProven,
    exportsWithUnknown: document.exportsWithUnknown,
    exportsAllDomainsUnknown: document.exportsAllDomainsUnknown,
    exportsUnknownOnlyInVariants: 0,
    exportsWithoutSummary: document.exportsWithoutSummary,
    unknownByDomain: document.unknownByDomain,
    unknownTotal: document.unknownTotal,
    behavioralRows: document.behavioralRows,
    closureNotes: 0,
    closureNoteSamples: [],
    attestedRuntimeNotes: 0,
    attestedRuntimeNoteSamples: [],
    reviewPlanItems: plan?.checklistItems ?? null,
    reviewPlanItemsByKind: plan?.itemsByKind ?? null,
    wireBytes: {
      prettyMain: mainBytes ?? canonicalMainBytes,
      canonicalMain: canonicalMainBytes,
      proposalPlan: planBytes,
      perExport:
        document.exportsTotal > 0
          ? Math.round((canonicalMainBytes / document.exportsTotal) * 100) / 100
          : null,
      perOperation:
        operationCount > 0
          ? Math.round((canonicalMainBytes / operationCount) * 100) / 100
          : null
    },
    ...(plan === null ? { note: "proposal plan missing or unparsable" } : {})
  };
}

export function reviewPlanPathFor(contractPath) {
  return `${contractPath}.proposal.json`;
}

export function refusalPathFor(contractPath) {
  return `${contractPath}.refusals.json`;
}

function readJsonBytesOrNull(path) {
  try {
    const bytes = readFileSync(path);
    return { value: JSON.parse(bytes.toString("utf8")), bytes: bytes.length };
  } catch {
    return null;
  }
}

export function readProposalRefusalAudit(contractPath) {
  const audit = readJsonBytesOrNull(refusalPathFor(contractPath));
  if (
    audit?.value?.format !== "solid-checker-contract-proposal-refusals" ||
    audit.value.refusalVersion !== 1 ||
    !Array.isArray(audit.value.refusals)
  ) return null;
  return {
    package: audit.value.package ?? null,
    refusals: audit.value.refusals,
    // Additive under the same envelope version: a sidecar written before the
    // disposition census existed simply has none.
    inapplicable: Array.isArray(audit.value.inapplicable) ? audit.value.inapplicable : [],
    bytes: audit.bytes
  };
}

export function readContractContent(contractPath, refusedEntrypointsFromStdout = null) {
  const contract = readJsonBytesOrNull(contractPath);
  const reviewPlan = readJsonBytesOrNull(reviewPlanPathFor(contractPath));
  const refusalAudit = readProposalRefusalAudit(contractPath);
  return summarizeContract({
    contract: contract?.value ?? null,
    reviewPlan: reviewPlan?.value ?? null,
    refusals: refusalAudit === null
      ? null
      : {
          format: "solid-checker-contract-proposal-refusals",
          refusalVersion: 1,
          refusals: refusalAudit.refusals
        },
    inapplicable: refusalAudit?.inapplicable ?? null,
    refusedEntrypointsFromStdout,
    mainBytes: contract?.bytes ?? null,
    planBytes: reviewPlan?.bytes ?? null
  });
}
