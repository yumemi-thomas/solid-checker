// Per-node audit records for the published-dependency-graph lane.
//
// The plain lane keeps its proposal's records beside the generated contract:
// the generator's closure declines and artifact-case refusals
// (`<output>.refusals.json`) and the planner's unresolved claims and closure
// candidates (`<output>.proposal.json`). The graph lane regenerates every node
// -- its root included -- in private certification scratch, against the
// accepted contracts of the nodes below it, and removes that scratch when the
// transaction ends. Without these records a graph-lane answer that leaves a
// domain open says nothing about why: the node never proposed it, the
// generator declined it, or the node refused.
//
// Everything here is diagnostic. It reads the untrusted proposal sidecars the
// lane already wrote, is never an input to certification, and changes no byte
// of any proposal, receipt or accepted document.
//
// Bounded: identical records are deduplicated, and every list is capped at
// `GRAPH_NODE_RECORD_LIMIT` with the number it left out stated under
// `truncated`, so nothing is dropped silently.

import { existsSync, readFileSync } from "node:fs";

/// Per list, per node. Sized from the 2026-09-28 certification-metric run: a
/// root's own unresolved claims reach about 600 (`@tanstack/solid-query`), and
/// the largest dependency node, `@solid-primitives/utils`, declines about 1,700
/// closures; a cap below either leaves an answer the metric cannot classify.
export const GRAPH_NODE_RECORD_LIMIT = 2048;
/// A graph carries at most 1024 nodes (the policy-2 node limit), and importer
/// variants of one artifact share one record, so this cap is never expected to
/// bind; it is stated for the same reason as the per-list cap.
export const GRAPH_NODE_LIMIT = 1024;

const RECIPE_ADDRESS_MARKER = "solid-checker:recipe-addresses=";

function readJsonIfPresent(path) {
  if (!path || !existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

/// Deduplicates `items` by their exact JSON bytes, keeps the first `limit`,
/// and says how many distinct records it left out.
export function boundedRecords(items, limit = GRAPH_NODE_RECORD_LIMIT) {
  const seen = new Set();
  const distinct = [];
  for (const item of Array.isArray(items) ? items : []) {
    const key = JSON.stringify(item);
    if (seen.has(key)) continue;
    seen.add(key);
    distinct.push(item);
  }
  return { items: distinct.slice(0, limit), dropped: Math.max(0, distinct.length - limit) };
}

/// A claim of the proposal plan, without the artifact identity every claim of
/// a one-case node repeats.
/// The claim id is left out too: it hashes the claim's bytes, a reader of this
/// record classifies by export and path, and the full id is in the recipe
/// addresses for every claim a recipe could serve. An operation path names its
/// operation relative to the node's one artifact case.
function claimRecord(claim) {
  const path = claim?.subject?.path ?? null;
  return {
    export: claim?.subject?.export ?? null,
    path:
      path && typeof path.operation === "string"
        ? { ...path, operation: path.operation.replace(/^artifact-case:[0-9a-f]+:/, "") }
        : path
  };
}

/// The record of one graph node, read from its generation's sidecars.
///
/// `node` is the lane's node identity (package, version, entrypoint,
/// conditions); `artifactCase` the one case its plan names, when generation
/// got that far; `refusal` the node's own refusal (acquisition, generation, or
/// a refused dependency below it). Either sidecar may be absent: a node that
/// refused before generating has neither, and says so by its `refusal`.
export function graphNodeAuditRecord({
  node,
  isRoot = false,
  artifactCase = null,
  output = null,
  plan = null,
  statesNothing = false,
  refusal = null,
  limit = GRAPH_NODE_RECORD_LIMIT
}) {
  const census = readJsonIfPresent(output ? `${output}.refusals.json` : null);
  const proposal = readJsonIfPresent(plan);
  const record = {
    package: node?.packageName ?? null,
    version: node?.packageVersion ?? null,
    entrypoint: node?.entrypoint ?? null,
    conditions: [...(node?.conditions ?? [])],
    root: Boolean(isRoot),
    artifactCase
  };
  if (statesNothing) record.statesNothing = true;
  if (refusal) {
    record.refusal = {
      stage: refusal.stage ?? null,
      reason: String(refusal.reason ?? "")
    };
  }
  const truncated = {};
  const lists = {
    // The generator's closure declines, as the plain lane's census keeps them:
    // export, domain, kind, and the package, callee and location the decline
    // names (the node's own root folded to `<package-root>` by the generator).
    // Columns the kind leaves empty are omitted rather than written as "".
    declinedClosures: (census?.declinedClosures ?? []).map(entry => {
      const record = { export: entry.export, domain: entry.domain, kind: entry.kind };
      for (const field of ["package", "callee", "location", "declaration", "shape", "spelling"]) {
        if (typeof entry[field] === "string" && entry[field] !== "") record[field] = entry[field];
      }
      return record;
    }),
    // Artifact-case refusals and the claims the generator withheld, with the
    // refusal class it decided from the error itself.
    refusals: (census?.refusals ?? []).map(entry => ({
      stage: entry.stage ?? null,
      class: entry.class ?? null,
      reason: entry.reason ?? null
    })),
    withheldClaims: (census?.withheldClaims ?? []).map(entry => ({
      export: entry.export ?? null,
      role: entry.role ?? null,
      reason: entry.reason ?? null
    })),
    // What the planner could not resolve (never proposed as a closure) and
    // what it proposed; certification decides the second.
    unresolvedClaims: (proposal?.unresolvedClaims ?? []).map(claimRecord),
    closureCandidates: (proposal?.closureCandidates ?? []).map(claimRecord)
  };
  for (const [name, items] of Object.entries(lists)) {
    const bounded = boundedRecords(items, limit);
    record[name] = bounded.items;
    if (bounded.dropped) truncated[name] = bounded.dropped;
  }
  if (Object.keys(truncated).length) record.truncated = truncated;
  return record;
}

/// The lane's node records, one per distinct record (importer variants of one
/// artifact generate once and record identically), in a stable order, capped
/// at `GRAPH_NODE_LIMIT` with the remainder counted.
export function graphNodeRecordSet(records, limit = GRAPH_NODE_LIMIT) {
  const sorted = [...(records ?? [])].filter(Boolean).sort((left, right) =>
    String(left.package).localeCompare(String(right.package)) ||
    String(left.entrypoint).localeCompare(String(right.entrypoint)) ||
    JSON.stringify(left.conditions).localeCompare(JSON.stringify(right.conditions)) ||
    String(left.artifactCase ?? "").localeCompare(String(right.artifactCase ?? "")) ||
    JSON.stringify(left).localeCompare(JSON.stringify(right))
  );
  const bounded = boundedRecords(sorted, limit);
  return { nodes: bounded.items, truncated: bounded.dropped };
}

/// The certification audit's `graphNodes` field: the prepared graph's node
/// records (a `graphNodeRecordSet`) and the native transaction's node-digest
/// to artifact-case join, or `null` when no graph was prepared.
export function graphNodesAuditField(recordSet, nodeCases) {
  if (!recordSet) return null;
  return {
    records: recordSet.nodes,
    ...(recordSet.truncated ? { truncated: recordSet.truncated } : {}),
    cases: Array.isArray(nodeCases) ? nodeCases : []
  };
}

/// Which artifact case each graph node the native transaction certified
/// selects, read from the per-node recipe-address lines (printed once per
/// planned node, with or without addresses). This is what joins a node the
/// retained catalog names only by digest to the node record above, which the
/// lane knows by artifact case. Diagnostic only.
export function graphNodeCasesFromNativeOutput(stdout) {
  const seen = new Map();
  for (const line of String(stdout ?? "").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith(RECIPE_ADDRESS_MARKER)) continue;
    try {
      const record = JSON.parse(trimmed.slice(RECIPE_ADDRESS_MARKER.length));
      const digest = record?.node?.digest;
      if (typeof digest !== "string" || typeof record.artifactCase !== "string") continue;
      const entry = {
        digest,
        package: record.node.package ?? null,
        version: record.node.version ?? null,
        artifactCase: record.artifactCase
      };
      seen.set(JSON.stringify(entry), entry);
    } catch {
      // Malformed is not a record.
    }
  }
  return [...seen.values()].sort((left, right) =>
    left.digest.localeCompare(right.digest) || left.artifactCase.localeCompare(right.artifactCase)
  );
}
