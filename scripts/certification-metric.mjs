#!/usr/bin/env bun

// The north-star certification metric: of the export surface of the most
// downloaded Solid 2 packages, how much does the checker certify clean, and
// what blocks the rest?
//
//   bun scripts/certification-metric.mjs --select [--downloads <counts.json>]
//                                        network: re-pin the corpus
//   bun scripts/certification-metric.mjs --print-probes         probe ids, one per line
//   bun scripts/certification-metric.mjs --run <run.json> [--json <out>] [--markdown <out>]
//                                        [--clean-retained] [--corpus <file>]
//   bun scripts/certification-metric.mjs --summarize-hosts <metric.json,...> [--markdown <out>]
//                                        one row per host measurement (ADR 0140)
//
// `make certification-metric` runs the ecosystem benchmark's certification
// pipeline over exactly the corpus probes and then this script's measurement.
// Nothing here certifies anything; it reads one `--keep-temp` benchmark run.
//
// # The corpus
//
// `scripts/ecosystem-benchmark/certification-metric-corpus.json`, pinned: the
// top packages by last-week npm downloads among the benchmark manifest's
// `solid2` rows and a reviewed list of Solid 2 packages outside its families,
// minus the runtime foundation (ADR 0027: `solid-js`, `@solidjs/web` and
// `@solidjs/signals` are dialect vocabulary, never package contracts) and
// build tooling. Each package is pinned to the manifest row's exact version and
// integrity, and to its `head` probe (the audited rc.9 triple) or, where the row
// has one compatible Solid release, its `only` probe. `--print-probes` refuses a
// manifest that no longer agrees with the pin, so a manifest refresh cannot
// silently change what the metric measured.
//
// # What one export is
//
// An export is an `(entrypoint, name)` pair at an entrypoint a consumer can
// name (the census's `nameableEntrypoint`). Kobalte's `./accordion` `Root` and
// `./dialog` `Root` are different components, so the census's name-only key
// would collapse them. The surface is the generated contract's own export list,
// so an entrypoint that certification refused still counts its exports.
//
// Each export gets one bucket, from the strongest summary its own package row
// certified for it across that entrypoint's artifact cases (the census's
// best-answer rule):
//
//   clean        the import finds nothing open: a non-callable value, or a
//                callable with callbacks, reads, returns and creates all closed
//                (`consumerState` value/clean)
//   partial      something is stated or closed, and something is still open
//   degenerate   a summary that states nothing and closes nothing
//   uncertified  in the generated surface, and no accepted summary answers it
//
// A package whose row produced no generated surface at all is *not
// certifiable*; it scores 0 in the per-package mean, and its failure class is
// its wall.
//
// # Walls
//
// Every non-clean export's open consumer domains get one status each, read
// strictly from the records of the artifact cases that gave the answer (the
// method of docs/package-contract-v2/phase22/2026-09-23-what-holds-an-import-open.mjs),
// and each status maps to one blocking cause (`causeOf`). An export is blocked
// by a cause when one of its open domains is; it is *solely* blocked when every
// open domain is. A reason names what was asked, not what would help: a claim
// withheld for want of a probe recipe was weakened out before its census ran,
// so a recipe may only uncover the census refusal underneath
// (probe-recipe-scaffold's two-pass procedure is the check).
//
// A published-graph answer is read from the answering node's own records: the
// native withheld closures and operations (which carry their node), then the
// node's generation records the audit keeps under `graphNodes` (declines,
// unresolved claims, closure candidates), joined by artifact case -- directly,
// or through the node digest the native run mapped to one. The plain lane's
// sidecars answer only a retained proposal root, which is that lane's own
// case. `graph lane: unrecorded` is left for an older audit with no node
// records, an answer no record names, and a capped record list.

import { existsSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

import {
  consumerState,
  misuseClasses,
  nameableEntrypoint,
  summaryState
} from "./contract-coverage-census.mjs";

const root = resolve(import.meta.dirname, "..");
export const CORPUS_PATH = join(root, "scripts/ecosystem-benchmark/certification-metric-corpus.json");
const MANIFEST_PATH = join(root, "scripts/ecosystem-benchmark/manifest.json");
const DEMAND_PATH = join(root, "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json");

const CONSUMER_DOMAINS = ["callbacks", "reads", "returns", "creates"];
const CONSUMER_RANK = { value: 4, clean: 3, "some-uses": 2, "every-import": 1 };
export const BUCKETS = ["clean", "partial", "degenerate", "uncertified"];
const BUCKET_RANK = { clean: 4, partial: 3, degenerate: 2, uncertified: 1 };

/// The runtime foundation (ADR 0027). Its behaviour is the dialect's reviewed
/// vocabulary; a package contract for it is refused by generator and loader.
export const RUNTIME_FOUNDATION = ["solid-js", "@solidjs/web", "@solidjs/signals"];

function fail(message) {
  console.error(`certification-metric: ${message}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Corpus selection
// ---------------------------------------------------------------------------

/// The probe a corpus package is measured by: `head` (the audited Solid 2
/// release) when the row has one, else its single `only` probe. A `floor`
/// probe measures the oldest compatible runtime and never answers this metric.
export function chooseProbe(row) {
  const probes = row?.probes ?? [];
  return probes.find(probe => probe.kind === "head") ?? probes.find(probe => probe.kind === "only") ?? null;
}

/// Ranks candidates by weekly downloads and takes the first `size` that are
/// neither excluded nor missing a manifest row. `candidates` are
/// `{ package, weeklyDownloads, row?, family? }`; `exclusions` maps a package
/// name to `{ reason, detail }`. Every candidate ranked above the cut-off that
/// was skipped is returned with its reason, so the pin shows what it left out.
export function selectCorpus({ candidates, exclusions = {}, size = 30 }) {
  const ranked = [...candidates].sort(
    (left, right) =>
      (right.weeklyDownloads ?? -1) - (left.weeklyDownloads ?? -1) || left.package.localeCompare(right.package)
  );
  const packages = [];
  const skipped = [];
  for (const candidate of ranked) {
    if (packages.length >= size) break;
    const excluded = exclusions[candidate.package];
    if (excluded) {
      skipped.push({ package: candidate.package, weeklyDownloads: candidate.weeklyDownloads, ...excluded });
      continue;
    }
    const probe = candidate.row ? chooseProbe(candidate.row) : null;
    if (!candidate.row || !probe) {
      skipped.push({
        package: candidate.package,
        weeklyDownloads: candidate.weeklyDownloads,
        reason: "not-in-manifest",
        detail: candidate.detail ?? "no solid2 row in the ecosystem manifest carries this package"
      });
      continue;
    }
    packages.push({
      rank: packages.length + 1,
      package: candidate.package,
      version: candidate.row.version,
      integrity: candidate.row.integrity,
      family: candidate.row.family,
      weeklyDownloads: candidate.weeklyDownloads,
      releaseWeeklyDownloads: candidate.releaseWeeklyDownloads ?? null,
      probe: probe.id,
      probeKind: probe.kind,
      solid: probe.solid
    });
  }
  return { packages, skipped };
}

/// Refuses a manifest that disagrees with the pinned corpus: a missing row, a
/// moved version or integrity, or a probe that no longer exists.
export function corpusProblems(corpus, manifest) {
  const problems = [];
  const rows = (manifest?.rows ?? []).filter(row => row.solidTarget === "solid2");
  for (const entry of corpus?.packages ?? []) {
    const row = rows.find(candidate => candidate.package === entry.package);
    if (!row) {
      problems.push(`${entry.package}: no solid2 manifest row`);
      continue;
    }
    if (row.version !== entry.version || row.integrity !== entry.integrity) {
      problems.push(`${entry.package}: manifest has ${row.version} (${row.integrity}), the corpus pins ${entry.version} (${entry.integrity})`);
    }
    if (!(row.probes ?? []).some(probe => probe.id === entry.probe)) {
      problems.push(`${entry.package}: manifest row carries no probe ${entry.probe}`);
    }
  }
  return problems;
}

// ---------------------------------------------------------------------------
// Reading one retained row
// ---------------------------------------------------------------------------

function filesUnder(directory, found = []) {
  let entries;
  try {
    entries = readdirSync(directory, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) filesUnder(path, found);
    else found.push(path);
  }
  return found;
}

/// Everything the measurement reads from one row's retained output directory:
/// its generated contract, proposal, refusals, certification audit, and every
/// accepted document with its graph-lane attribution. Ported from the phase22
/// what-holds-an-import-open script, which established the attribution rules.
export function readRetainedRow(result) {
  const directory = result.retainedArtifacts?.outputDir;
  if (!directory || !existsSync(directory)) return null;
  const files = filesUnder(directory);
  const read = suffix => {
    const path = files.find(file => file.endsWith(suffix));
    return path ? JSON.parse(readFileSync(path, "utf8")) : null;
  };
  const lane = result.certificationAttempt?.lane ?? null;
  const audit = read(".certification-audit.json");
  const graphFile = files.find(file => file.endsWith("/graph.json") && file.includes("/graphs/"));
  const graph = graphFile ? JSON.parse(readFileSync(graphFile, "utf8")) : null;
  const caseByImportRoot = new Map();
  for (const file of files.filter(file => /\/case-sets\/[0-9a-f]+\/accepted-contract-case-set\.json$/.test(file))) {
    for (const entry of JSON.parse(readFileSync(file, "utf8")).cases ?? []) {
      caseByImportRoot.set(String(entry.resolvedImportRoot).replace(/^sha256:/, ""), entry.artifactCaseId);
    }
  }
  const graphRecords = [
    ...(audit?.withheldClosures ?? []),
    ...(audit?.withheldOperations ?? []),
    ...(audit?.certifiedClosures?.closed ?? [])
  ].filter(record => record.node?.digest);
  const attribution = file => {
    const node =
      /\/graphs\/[0-9a-f]+\/nodes\/([0-9a-f]+)\/objects\//.exec(file)?.[1] ??
      /\/case-sets\/[0-9a-f]+\/graph-nodes\/[0-9a-f]+\/([0-9a-f]+)\/objects\//.exec(file)?.[1];
    if (node) return { node: `sha256:${node}`, artifactCase: null };
    if (/\/graphs\/[0-9a-f]+\/root\/objects\//.test(file) && graph?.rootNode) {
      return { node: graph.rootNode, artifactCase: null };
    }
    const importRoot = /\/case-sets\/[0-9a-f]+\/cases\/([0-9a-f]+)\/objects\//.exec(file)?.[1];
    const artifactCase = importRoot ? caseByImportRoot.get(importRoot) : null;
    if (artifactCase) {
      const record = graphRecords.find(entry => entry.artifactCase === artifactCase);
      return { node: record?.node.digest ?? null, artifactCase };
    }
    return null;
  };
  const mains = files.filter(file => file.endsWith(".main.json"));
  const byBytes = new Map();
  for (const file of mains) {
    const where = attribution(file);
    if (where) byBytes.set(readFileSync(file, "utf8"), where);
  }
  const documents = [];
  const seen = new Set();
  for (const file of mains) {
    const bytes = readFileSync(file, "utf8");
    const where = lane === "published-graph" ? (byBytes.get(bytes) ?? { node: null, artifactCase: null }) : null;
    const key = `${JSON.stringify(where)}\u0000${bytes}`;
    if (seen.has(key)) continue;
    seen.add(key);
    documents.push({ document: JSON.parse(bytes), graph: where });
  }
  const candidates = audit?.closureCandidates ?? null;
  const reconciliation =
    lane === "published-graph" && candidates
      ? {
          candidates: candidates.count,
          closed: (audit.certifiedClosures?.closed ?? []).reduce((sum, record) => sum + (record.closed?.length ?? 0), 0),
          withheld: (audit.withheldClosures ?? []).length,
          withheldOperations: (audit.withheldOperations ?? []).length
        }
      : null;
  // The generated contract sits at the top of the output directory as
  // `<probe>.json`; every sidecar extends that name (`<probe>.json.proposal.json`).
  const generatedPath = files.find(
    file => dirname(file) === directory && file.endsWith(".json") && !basename(file).slice(0, -5).includes(".json")
  );
  return {
    probe: result.probeId,
    lane,
    reconciliation,
    documents,
    audit,
    // The graph lane's per-node generation records (audit only): the audit's
    // own copy, else the one the run report carries.
    graphNodes: audit?.graphNodes ?? result.certificationAttempt?.graphNodes ?? null,
    retainedProposalCases: audit?.graphPreparation?.retainedProposalCases ?? 0,
    proposal: read(".proposal.json"),
    refusals: read(".refusals.json"),
    generated: generatedPath ? JSON.parse(readFileSync(generatedPath, "utf8")) : null
  };
}

// ---------------------------------------------------------------------------
// Per-domain status (strict per answering case), then cause
// ---------------------------------------------------------------------------

function domainOfOperationId(local) {
  if (/^return(-\d+)?$/.test(local)) return "returns";
  if (/^callback-\d+$/.test(local)) return "callbacks";
  if (/^read-\d+$/.test(local)) return "reads";
  return null;
}

function operationDomain(generated, exportName, local) {
  for (const { cases } of Object.values(generated?.entrypoints ?? {})) {
    for (const artifactCase of cases ?? []) {
      const reference = artifactCase.exports?.[exportName];
      const id = typeof reference === "string" ? reference : reference?.summary;
      const call = generated.summaries?.[id]?.call ?? {};
      const domain = CONSUMER_DOMAINS.find(name => (call[name] ?? []).includes(local));
      if (domain) return domain;
    }
  }
  return null;
}

const STATUS_RANK = ["withheld", "withheld operation", "declined", "never proposed", "proposed, not certified"];
const statusRank = value => {
  const index = STATUS_RANK.findIndex(prefix => value.status.startsWith(prefix));
  return index === -1 ? STATUS_RANK.length : index;
};

function plainCaseStatus(row, exportName, entrypoint, artifactCase, domain) {
  const hit = (row.audit?.withheldClosures ?? []).find(
    entry => !entry.node && entry.export === exportName && entry.domain === domain && entry.artifactCase === artifactCase
  );
  if (hit) return { status: "withheld", reason: hit.reason };
  const operation = (row.audit?.withheldOperations ?? []).find(
    entry =>
      !entry.node &&
      entry.export === exportName &&
      entry.artifactCase === artifactCase &&
      operationDomain(row.generated, exportName, entry.operation.split(":operation:").pop()) === domain
  );
  if (operation) return { status: "withheld operation", reason: operation.reason };
  const declined = (row.refusals?.declinedClosures ?? []).find(
    entry => entry.export === exportName && entry.domain === domain && entry.entrypoint === entrypoint
  );
  if (declined) return { status: "declined", declined };
  const about = claim =>
    claim.subject?.artifactCase === artifactCase &&
    claim.subject?.export === exportName &&
    claim.subject?.path?.domain === domain;
  if ((row.proposal?.unresolvedClaims ?? []).some(about)) return { status: "never proposed" };
  if ((row.proposal?.closureCandidates ?? []).some(about)) return { status: "proposed, not certified" };
  return { status: "no record" };
}

function plainStatus(row, exportName, entrypoint, domain) {
  const claims = [...(row.proposal?.unresolvedClaims ?? []), ...(row.proposal?.closureCandidates ?? [])];
  const cases = [
    ...new Set(
      claims
        .filter(claim => claim.artifact?.entrypoint === entrypoint && claim.subject?.export === exportName)
        .map(claim => claim.subject.artifactCase)
    )
  ].sort();
  if (cases.length === 0) return { status: "no record" };
  const perCase = cases.map(artifactCase => plainCaseStatus(row, exportName, entrypoint, artifactCase, domain));
  return [...perCase].sort((left, right) => statusRank(left) - statusRank(right))[0];
}

/// The artifact case a graph-attributed answer selects: its own, or the one
/// the native transaction reported for its node digest (`graphNodes.cases`),
/// or one a node-attributed audit record pairs with that digest.
function graphCaseOf(row, { node, artifactCase }) {
  if (artifactCase) return artifactCase;
  if (!node) return null;
  const mapped = (row.graphNodes?.cases ?? []).find(entry => entry.digest === node)?.artifactCase;
  if (mapped) return mapped;
  const records = [
    ...(row.audit?.withheldClosures ?? []),
    ...(row.audit?.withheldOperations ?? []),
    ...(row.audit?.certifiedClosures?.closed ?? [])
  ];
  return records.find(entry => entry.node?.digest === node && entry.artifactCase)?.artifactCase ?? null;
}

function graphStatus(row, where, exportName, entrypoint, domain) {
  const { node, artifactCase } = where;
  if (!node && !artifactCase) return { status: "graph: no attribution" };
  const mine = entry =>
    entry.node?.digest &&
    entry.export === exportName &&
    ((node && entry.node.digest === node) || (artifactCase && entry.artifactCase === artifactCase));
  const withheld = (row.audit?.withheldClosures ?? []).find(entry => mine(entry) && entry.domain === domain);
  if (withheld) return { status: "withheld", reason: withheld.reason };
  const operation = (row.audit?.withheldOperations ?? []).find(
    entry => mine(entry) && domainOfOperationId(entry.operation.split(":operation:").pop()) === domain
  );
  if (operation) return { status: "withheld operation", reason: operation.reason };
  // An audit written before the lane kept per-node records: an absence of
  // records, not a verdict.
  if (!row.graphNodes) return { status: "graph: never proposed" };
  const selected = graphCaseOf(row, where);
  const records = selected ? (row.graphNodes.records ?? []).filter(record => record.artifactCase === selected) : [];
  if (records.length === 0) {
    // A retained proposal root (ADR 0073) is the plain lane's own proposal
    // case, published through the graph transaction without regeneration, so
    // the plain lane's records for that exact artifact case are its records.
    if (selected && row.retainedProposalCases > 0) {
      const plain = plainCaseStatus(row, exportName, entrypoint, selected, domain);
      if (plain.status !== "no record") return plain;
    }
    return { status: "graph: no node record" };
  }
  // The same order as the plain lane (`plainCaseStatus`): a decline, then a
  // claim the planner never resolved, then a candidate certification did not
  // close. Importer variants of one artifact record identically, so any
  // matching record answers.
  // A capped list cannot prove an absence, so a miss in one stops the
  // search there rather than answering with a lower-ranked status.
  const capped = list => records.some(record => (record.truncated?.[list] ?? 0) > 0);
  for (const record of records) {
    const declined = (record.declinedClosures ?? []).find(entry => entry.export === exportName && entry.domain === domain);
    if (declined) return { status: "declined", declined };
  }
  if (capped("declinedClosures")) return { status: "graph: record truncated", list: "declinedClosures" };
  const about = claim => claim.export === exportName && claim.path?.domain === domain;
  if (records.some(record => (record.unresolvedClaims ?? []).some(about))) return { status: "never proposed" };
  if (capped("unresolvedClaims")) return { status: "graph: record truncated", list: "unresolvedClaims" };
  if (records.some(record => (record.closureCandidates ?? []).some(about))) return { status: "proposed, not certified" };
  if (capped("closureCandidates")) return { status: "graph: record truncated", list: "closureCandidates" };
  return { status: "no record" };
}

/// The package a path under `node_modules/` belongs to, or null.
export function packageOfPath(path) {
  const match = /node_modules\/((?:@[^/]+\/)?[^/]+)\//.exec(String(path ?? ""));
  return match ? match[1] : null;
}

/// One blocking cause per open-domain status. The `class` groups the ranking;
/// `key` is the finer wall inside it; `location` is the obligation's own
/// location when the reason names one.
export function causeOf(status, domain) {
  const reason = status.reason ?? "";
  if (status.status === "withheld") {
    if (reason.startsWith("no recipe in corpus")) {
      return { class: "recipe", key: `no probe recipe (${domain})` };
    }
    if (reason.startsWith("veto did not complete")) {
      const thrown = /the worker threw: ([A-Za-z]+Error: [^)]{0,60})/.exec(reason)?.[1];
      return { class: "recipe", key: `veto did not complete${thrown ? `: ${thrown}` : ""}` };
    }
    if (reason.startsWith("veto")) return { class: "recipe", key: reason.split(":")[0] };
    const catchAll = /neither a default-library member, a dialect primitive under the negative authority, nor a declaration in this artifact's own runtime source: "([^"]+)" declared at (\S+?)(?::\d+\.\.\d+)?, called at ([^\s;]+?)(?::\d+\.\.\d+)?(?:;|$)/.exec(reason);
    if (catchAll) {
      const declaring = packageOfPath(catchAll[2]) ?? "<no package>";
      // The dialect has a row for this primitive, scoped to host targets the
      // requested condition set does not name: a different wall from a callee
      // no authority speaks for at all.
      const scoped = /the dialect row \S+ is scoped to the `([^`]+)` host target/.exec(reason)?.[1];
      return {
        class: "attribution catch-all",
        key: scoped ? `dialect row scoped to the ${scoped} host (${declaring})` : `callee in ${declaring}`,
        callee: `${declaring}:${catchAll[1]}`,
        location: catchAll[3].replace(/^.*node_modules\//, "")
      };
    }
    if (reason.startsWith("census refused")) {
      const body = reason.replace(/^census refused:\s*/, "");
      const form =
        /uncensused invoking form: ([a-z-]+)/.exec(body)?.[1] ??
        /premise required: the ([a-z-]+) form/.exec(body)?.[1];
      if (/uncensused invoking form/.test(body)) return { class: "census refusal", key: `uncensused invoking form: ${form}` };
      if (/premise required/.test(body)) return { class: "census refusal", key: `reads premise required: ${form}` };
      if (/unresolved callee/.test(body)) return { class: "census refusal", key: "unresolved callee" };
      if (/parameter-rooted family/.test(body)) {
        return { class: "missing claim form", key: "callbacks: invokes a caller-supplied callable" };
      }
      if (/refuses a call through "/.test(body)) return { class: "census refusal", key: "call through a local binding" };
      if (/creates census refuses "[^"]+" declared at/.test(body)) {
        return { class: "census refusal", key: "declared callee refused" };
      }
      if (/^domain-exhaustiveness/.test(body)) return { class: "census refusal", key: "domain-exhaustiveness" };
      if (/no function-like declaration node/.test(body)) {
        return { class: "census refusal", key: "no function-like declaration" };
      }
      if (/member invocation/.test(body)) return { class: "census refusal", key: "member invocation of a parameter" };
      return { class: "census refusal", key: body.replace(/\/[^\s"]+/g, "<path>").replace(/\d+/g, "N").slice(0, 80) };
    }
    return { class: "other withheld", key: reason.split(":")[0].slice(0, 80) };
  }
  if (status.status === "withheld operation") {
    const narrowed = /^narrowed out of a closed \w+ enumeration: ([a-z-]+)/.exec(reason)?.[1];
    if (narrowed) return { class: "withheld operation", key: `narrowed out: ${narrowed}` };
    const refused = /^operation census refused: ([a-z-]+)/.exec(reason)?.[1];
    if (refused) return { class: "withheld operation", key: `operation census refused: ${refused}` };
    return { class: "withheld operation", key: reason.split(":")[0].slice(0, 80) };
  }
  if (status.status === "declined") {
    const entry = status.declined;
    if (entry.kind === "unaccepted-external-dependency") {
      const dependency = String(entry.location ?? "").split(":").pop() || entry.package || "<unknown>";
      return { class: "unaccepted dependency", key: dependency };
    }
    if (entry.kind === "dialect-silent") {
      return { class: "dialect-silent", key: `${entry.package}:${entry.callee}` };
    }
    return { class: "declined", key: entry.kind };
  }
  if (status.status === "never proposed") return { class: "missing claim form", key: `${domain} never proposed` };
  if (status.status === "graph: never proposed") {
    // An audit from before the graph lane kept per-node records: an absence
    // of records, not a verdict; `measureRow` attaches what the plain lane
    // recorded for the same export and domain as `inferred`.
    return { class: "graph lane: unrecorded", key: domain };
  }
  if (status.status === "graph: no node record") {
    // The audit keeps per-node records, and none names the artifact case this
    // answer selects: an attribution gap, reported as one.
    return { class: "graph lane: unrecorded", key: `${domain} (no node record for the answering case)` };
  }
  if (status.status === "graph: record truncated") {
    return { class: "graph lane: unrecorded", key: `${domain} (node record truncated: ${status.list})` };
  }
  return { class: "no record", key: status.status };
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

function compareScores(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

function bucketOf(document, reference) {
  const view = consumerState(document, reference);
  if (view === "value" || view === "clean") return "clean";
  const { state } = summaryState(document, reference);
  return state === "degenerate" ? "degenerate" : "partial";
}

/// Nameable `(entrypoint, export)` pairs a contract document lists.
export function surfaceOf(document) {
  const pairs = new Set();
  for (const [entrypoint, value] of Object.entries(document?.entrypoints ?? {})) {
    if (!nameableEntrypoint(entrypoint)) continue;
    for (const artifactCase of value.cases ?? []) {
      for (const exportName of Object.keys(artifactCase.exports ?? {})) pairs.add(`${entrypoint}\u0000${exportName}`);
    }
  }
  return pairs;
}

/// Why an entrypoint that has a generated surface published no accepted
/// summary: the generator's own refusal for it, the certification audit's, or
/// the row's certification outcome.
function uncertifiedCause(row, result, entrypoint) {
  const fromRefusals = [...(row?.refusals?.refusals ?? []), ...(row?.audit?.refusals ?? [])].find(
    entry => entry.entrypoint === entrypoint
  );
  if (fromRefusals) {
    const text = String(fromRefusals.reason ?? fromRefusals.class ?? "refused");
    const key = /accepted dependency \S+ has no exact runtime binding/.test(text)
      ? "accepted dependency has no exact runtime binding"
      : `${fromRefusals.class ?? fromRefusals.stage ?? "refused"}`;
    return { class: "not certified", key };
  }
  const attempt = result.certificationAttempt ?? {};
  if (attempt.status && attempt.status !== "certified") {
    return { class: "not certified", key: `certification ${attempt.status}${attempt.stage ? ` at ${attempt.stage}` : ""}` };
  }
  return { class: "not certified", key: "no accepted case for this entrypoint" };
}

/// Measures one row: its surface, buckets, misuse classes, and every
/// non-clean export's causes.
export function measureRow(result, row) {
  const name = result.package;
  const generated = row?.generated ?? null;
  const surface = generated ? surfaceOf(generated) : new Set();
  const answers = new Map();
  for (const { document, graph } of row?.documents ?? []) {
    if (document.package?.name !== name) continue;
    for (const [entrypoint, value] of Object.entries(document.entrypoints ?? {})) {
      if (!nameableEntrypoint(entrypoint)) continue;
      for (const artifactCase of value.cases ?? []) {
        for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
          const key = `${entrypoint}\u0000${exportName}`;
          surface.add(key);
          const id = typeof reference === "string" ? reference : reference?.summary;
          const closed = new Set(document.summaries?.[id]?.call?.closed ?? []);
          const view = consumerState(document, reference);
          const bucket = bucketOf(document, reference);
          const score = [BUCKET_RANK[bucket], CONSUMER_RANK[view], CONSUMER_DOMAINS.filter(domain => closed.has(domain)).length];
          const held = answers.get(key);
          const better = !held || compareScores(score, held.score) > 0;
          const misuse = new Set([...(held?.misuse ?? []), ...misuseClasses(document, reference)]);
          if (better) answers.set(key, { bucket, view, closed, graph, score, misuse });
          else held.misuse = misuse;
        }
      }
    }
  }
  const exports = [];
  for (const key of [...surface].sort()) {
    const [entrypoint, exportName] = key.split("\u0000");
    const answer = answers.get(key);
    if (!answer) {
      exports.push({ entrypoint, export: exportName, bucket: "uncertified", misuse: [], causes: [uncertifiedCause(row, result, entrypoint)] });
      continue;
    }
    const entry = { entrypoint, export: exportName, bucket: answer.bucket, misuse: [...answer.misuse].sort(), causes: [] };
    if (answer.bucket !== "clean") {
      for (const domain of CONSUMER_DOMAINS.filter(domain => !answer.closed.has(domain))) {
        const status = answer.graph
          ? graphStatus(row, answer.graph, exportName, entrypoint, domain)
          : plainStatus(row, exportName, entrypoint, domain);
        const cause = { domain, ...causeOf(status, domain) };
        if (cause.class === "graph lane: unrecorded") {
          // The plain lane certified the same artifact before its dependency
          // was accepted; its record for this domain is the best available
          // hint, and is reported apart from the strict ranking.
          // A plain-lane block on a dependency is exactly what the graph lane
          // went on to accept, so it explains nothing about the graph answer.
          const { class: group, key, callee, location } = causeOf(plainStatus(row, exportName, entrypoint, domain), domain);
          cause.inferred =
            group === "unaccepted dependency" || group === "no record"
              ? { class: "graph lane: unrecorded", key: `${domain} (the plain lane stopped at ${group === "no record" ? "no record" : `the unaccepted dependency ${key}`})` }
              : { class: group, key, ...(callee ? { callee, location } : {}) };
        }
        entry.causes.push(cause);
      }
      if (entry.causes.length === 0) {
        // Every consumer domain is closed and the export is still not clean:
        // the shape may be callable and the summary states nothing callable
        // about it. No domain status explains that, so it is named as such.
        entry.causes.push({ class: "no record", key: "all consumer domains closed, not clean" });
      }
    }
    exports.push(entry);
  }
  const counts = Object.fromEntries(BUCKETS.map(bucket => [bucket, exports.filter(entry => entry.bucket === bucket).length]));
  const misuse = {};
  for (const entry of exports) for (const kind of entry.misuse) misuse[kind] = (misuse[kind] ?? 0) + 1;
  return {
    package: name,
    version: result.version,
    probe: result.probeId,
    rowClass: result.class,
    certification: result.certificationAttempt?.status ?? null,
    lane: result.certificationAttempt?.lane ?? null,
    certifiable: Boolean(generated) || exports.length > 0,
    refusedEntrypoints: (result.refusedEntrypoints ?? []).length,
    // Why a row with no surface is not certifiable, without the private
    // temporary paths that identify a machine rather than a wall.
    refusal:
      generated || exports.length > 0
        ? null
        : String(result.certificationAttempt?.reason ?? result.detail ?? result.class ?? "")
            .replace(/(?:\/private)?\/var\/folders\/\S+/g, "<tmp>")
            .slice(0, 400),
    exports,
    counts,
    total: exports.length,
    misuseCapable: exports.filter(entry => entry.misuse.length > 0).length,
    misuse,
    // Graph lane only: closure candidates against closed, withheld and
    // withheld-operation records. A root that states nothing with zero
    // candidates was never proposed anything, not refused.
    reconciliation: row?.reconciliation ?? null,
    // Graph lane only: how many per-node records the audit kept, how many it
    // capped away, and how many node digests the native run mapped to cases.
    graphNodeRecords: row?.graphNodes
      ? {
          records: (row.graphNodes.records ?? []).length,
          truncatedRecords: row.graphNodes.truncated ?? 0,
          truncatedLists: (row.graphNodes.records ?? []).filter(record => record.truncated).length,
          nodeCases: (row.graphNodes.cases ?? []).length
        }
      : null,
    withheldClosureReasons: result.certificationAttempt?.withheldClosureReasons ?? null
  };
}

const ratio = (part, whole) => (whole === 0 ? 0 : part / whole);

/// Headline shares. `perPackage` is the mean of each package's clean share,
/// an uncertifiable package counting 0; `byDownloads` weights the same shares
/// by weekly downloads; `pooled` is clean exports over all exports.
export function headline(packages) {
  const shares = packages.map(entry => ({ share: ratio(entry.counts?.clean ?? 0, entry.total ?? 0), weight: entry.weeklyDownloads ?? 0 }));
  const weight = shares.reduce((sum, entry) => sum + entry.weight, 0);
  const clean = packages.reduce((sum, entry) => sum + (entry.counts?.clean ?? 0), 0);
  const total = packages.reduce((sum, entry) => sum + (entry.total ?? 0), 0);
  return {
    packages: packages.length,
    exports: total,
    cleanExports: clean,
    perPackage: ratio(shares.reduce((sum, entry) => sum + entry.share, 0), shares.length),
    byDownloads: ratio(shares.reduce((sum, entry) => sum + entry.share * entry.weight, 0), weight),
    pooled: ratio(clean, total)
  };
}

/// Ranks blocking causes over every non-clean export. `exports` counts exports
/// with at least one open domain blocked by the cause; `sole` those whose every
/// open domain it blocks (fixing it alone would clear them); `sites` the
/// pinned demand's call sites that reach a blocked export by (package, name).
export function rankWalls(packages, demandRows = [], { inferred = false } = {}) {
  const demand = new Map();
  for (const row of demandRows) {
    const key = `${row.package}\u0000${row.export}`;
    demand.set(key, (demand.get(key) ?? 0) + row.sites);
  }
  const walls = new Map();
  const touch = (group, key) => {
    const id = `${group}\u0000${key}`;
    if (!walls.has(id)) {
      walls.set(id, { class: group, key, exports: 0, sole: 0, sites: 0, packageWeighted: 0, packages: new Set(), examples: [], locations: new Map(), names: new Set() });
    }
    return walls.get(id);
  };
  const classes = new Map();
  for (const entry of packages) {
    // `packageWeighted` gives each export 1 / its package's surface, summed
    // and divided by the package count below: the share of the per-package
    // headline a wall holds, so one 568-export package does not rank alone.
    const weight = 1 / Math.max(1, (entry.exports ?? []).length);
    for (const item of entry.exports ?? []) {
      if (item.bucket === "clean") continue;
      const nameKey = `${entry.package}\u0000${item.export}`;
      // The inferred view reads a graph-lane domain's plain-lane record in
      // place of its absence, and says so in the class.
      const causes = inferred
        ? item.causes.map(cause =>
            cause.inferred ? { ...cause.inferred, class: `${cause.inferred.class} (inferred from the plain lane)` } : cause
          )
        : item.causes;
      const seenKeys = new Set();
      const seenClasses = new Set();
      const distinct = new Set(causes.map(cause => `${cause.class}\u0000${cause.key}`));
      const distinctClasses = new Set(causes.map(cause => cause.class));
      for (const cause of causes) {
        const id = `${cause.class}\u0000${cause.key}`;
        if (!seenKeys.has(id)) {
          seenKeys.add(id);
          const wall = touch(cause.class, cause.key);
          wall.exports += 1;
          wall.packageWeighted += weight;
          if (distinct.size === 1) wall.sole += 1;
          wall.packages.add(entry.package);
          if (!wall.names.has(nameKey)) {
            wall.names.add(nameKey);
            wall.sites += demand.get(nameKey) ?? 0;
          }
          if (wall.examples.length < 3) wall.examples.push(`${entry.package} ${item.entrypoint} ${item.export}`);
        }
        if (cause.location) {
          const wall = touch(cause.class, cause.key);
          const label = `${cause.callee} at ${cause.location}`;
          wall.locations.set(label, (wall.locations.get(label) ?? 0) + 1);
        }
        if (!seenClasses.has(cause.class)) {
          seenClasses.add(cause.class);
          const group = classes.get(cause.class) ?? { class: cause.class, exports: 0, sole: 0, sites: 0, packageWeighted: 0, packages: new Set(), names: new Set() };
          group.exports += 1;
          group.packageWeighted += weight;
          group.packages.add(entry.package);
          if (distinctClasses.size === 1) group.sole += 1;
          if (!group.names.has(nameKey)) {
            group.names.add(nameKey);
            group.sites += demand.get(nameKey) ?? 0;
          }
          classes.set(cause.class, group);
        }
      }
    }
  }
  const order = (left, right) => right.exports - left.exports || right.sole - left.sole || String(left.key ?? left.class).localeCompare(String(right.key ?? right.class));
  const share = value => Number((value / Math.max(1, packages.length)).toFixed(4));
  return {
    classes: [...classes.values()]
      .map(({ names, packages: set, packageWeighted, ...rest }) => ({ ...rest, packageWeighted: share(packageWeighted), packages: set.size }))
      .sort(order),
    walls: [...walls.values()]
      .map(({ names, packages: set, locations, packageWeighted, ...rest }) => ({
        ...rest,
        packageWeighted: share(packageWeighted),
        packages: [...set].sort(),
        topLocations: [...locations].sort((left, right) => right[1] - left[1]).slice(0, 5).map(([label, count]) => ({ label, count }))
      }))
      .sort(order)
  };
}

/// Most non-clean exports have several open domains with different causes, so
/// no single wall clears many exports on its own. This orders cause *classes*
/// greedily: each step adds the class that, together with the classes before
/// it, leaves the most exports with no remaining cause. The counts are upper
/// bounds -- a cleared cause may uncover another one it was masking (a recipe
/// that lets its census run, a dependency whose acceptance exposes its own
/// claims) -- and they say which combinations matter, not what a fix yields.
export function unlockCurve(packages, { steps = 8 } = {}) {
  const exportsCauses = [];
  for (const entry of packages) {
    for (const item of entry.exports ?? []) {
      if (item.bucket === "clean" || item.causes.length === 0) continue;
      exportsCauses.push(new Set(item.causes.map(cause => cause.class)));
    }
  }
  const chosen = new Set();
  const curve = [];
  const cleared = () => exportsCauses.filter(classes => [...classes].every(group => chosen.has(group))).length;
  const candidates = new Set(exportsCauses.flatMap(classes => [...classes]));
  for (let step = 0; step < steps && chosen.size < candidates.size; step += 1) {
    let best = null;
    for (const group of [...candidates].sort()) {
      if (chosen.has(group)) continue;
      chosen.add(group);
      const value = cleared();
      chosen.delete(group);
      if (!best || value > best.value) best = { group, value };
    }
    chosen.add(best.group);
    curve.push({ add: best.group, exportsCleared: best.value, of: exportsCauses.length });
  }
  return curve;
}

/// How many withheld claims (audit records, not exports) name a missing or
/// failed probe recipe, and how many exports only a recipe would clear.
export function recipeBlocked(packages, results) {
  let claims = 0;
  let vetoIncomplete = 0;
  for (const result of results) {
    const reasons = result.certificationAttempt?.withheldClosureReasons ?? {};
    claims += reasons.noRecipe ?? 0;
    vetoIncomplete += (reasons.vetoUnreproducible ?? 0) + (reasons.vetoThrew ?? 0) + (reasons.vetoTimedOut ?? 0) + (reasons.vetoRunRefused ?? 0) + (reasons.vetoIncomplete ?? 0);
  }
  let exportsOnlyRecipe = 0;
  let exportsAnyRecipe = 0;
  for (const entry of packages) {
    for (const item of entry.exports ?? []) {
      if (item.bucket === "clean" || item.causes.length === 0) continue;
      const recipe = item.causes.filter(cause => cause.class === "recipe").length;
      if (recipe > 0) exportsAnyRecipe += 1;
      if (recipe === item.causes.length) exportsOnlyRecipe += 1;
    }
  }
  return { noRecipeClaims: claims, vetoIncompleteClaims: vetoIncomplete, exportsAnyRecipe, exportsOnlyRecipe };
}

export function measure({ run, corpus, rows, demandRows = [] }) {
  const byProbe = new Map((run.results ?? []).map(result => [result.probeId, result]));
  const packages = [];
  const missing = [];
  for (const entry of corpus.packages) {
    const result = byProbe.get(entry.probe);
    if (!result) {
      missing.push(entry.probe);
      continue;
    }
    const measured = measureRow(result, rows.get(entry.probe) ?? null);
    packages.push({ ...measured, rank: entry.rank, weeklyDownloads: entry.weeklyDownloads, family: entry.family });
  }
  const totals = Object.fromEntries(BUCKETS.map(bucket => [bucket, packages.reduce((sum, entry) => sum + entry.counts[bucket], 0)]));
  const misuse = {};
  for (const entry of packages) for (const [kind, count] of Object.entries(entry.misuse)) misuse[kind] = (misuse[kind] ?? 0) + count;
  return {
    format: "solid-checker-certification-metric",
    metricVersion: 2,
    run: { startedAt: run.startedAt ?? null, finishedAt: run.finishedAt ?? null, durationMs: run.durationMs ?? null },
    // ADR 0140: the host every case of the run was certified for, or null for
    // the host-free certification. Each host is its own measurement.
    host: run.scope?.host ?? null,
    corpus: { measuredOn: corpus.measuredOn, packages: corpus.packages.length },
    missingProbes: missing,
    headline: headline(packages),
    totals: {
      ...totals,
      misuseCapable: packages.reduce((sum, entry) => sum + entry.misuseCapable, 0),
      misuse,
      notCertifiable: packages
        .filter(entry => !entry.certifiable)
        .map(entry => ({ package: entry.package, rowClass: entry.rowClass, refusal: entry.refusal })),
      refusedEntrypoints: packages.reduce((sum, entry) => sum + entry.refusedEntrypoints, 0)
    },
    recipes: recipeBlocked(packages, [...byProbe.values()].filter(result => corpus.packages.some(entry => entry.probe === result.probeId))),
    ...rankWalls(packages, demandRows),
    inferred: rankWalls(packages, demandRows, { inferred: true }),
    unlockCurve: unlockCurve(packages),
    packages
  };
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const percent = value => `${(100 * value).toFixed(1)}%`;

export function renderMarkdown(result) {
  const lines = [];
  const head = result.headline;
  lines.push("# Certification metric");
  lines.push("");
  lines.push(`Run ${result.run.startedAt ?? "?"} .. ${result.run.finishedAt ?? "?"}, harness wall ${Math.round((result.run.durationMs ?? 0) / 1000)} s. Corpus pinned ${result.corpus.measuredOn}, ${result.corpus.packages} packages.`);
  lines.push("");
  lines.push(
    result.host
      ? `Host: **${result.host}** (ADR 0140). Every case carries the \`${result.host}\` condition, so these answers reach only a consumer that declares that host.`
      : "Host: none (the host-free certification, what a consumer that declares no host receives; ADR 0140)."
  );
  lines.push("");
  lines.push(`- exports certified clean, per-package mean: **${percent(head.perPackage)}**`);
  lines.push(`- exports certified clean, weighted by weekly downloads: **${percent(head.byDownloads)}**`);
  lines.push(`- exports certified clean, pooled: ${head.cleanExports} of ${head.exports} (${percent(head.pooled)})`);
  const totals = result.totals;
  lines.push(`- buckets: clean ${totals.clean}, partial ${totals.partial}, degenerate ${totals.degenerate}, uncertified ${totals.uncertified}`);
  lines.push(`- misuse-capable exports: ${totals.misuseCapable} ${JSON.stringify(totals.misuse)}`);
  lines.push(`- not certifiable: ${totals.notCertifiable.length ? totals.notCertifiable.map(entry => `${entry.package} (${entry.rowClass})`).join(", ") : "none"}`);
  lines.push(`- entrypoints refused before a surface existed: ${totals.refusedEntrypoints}`);
  lines.push(`- recipe-blocked claims: ${result.recipes.noRecipeClaims} no recipe, ${result.recipes.vetoIncompleteClaims} veto incomplete; exports only a recipe would clear: ${result.recipes.exportsOnlyRecipe}`);
  if (result.missingProbes.length) lines.push(`- **probes missing from the run:** ${result.missingProbes.join(", ")}`);
  lines.push("");
  lines.push("| # | package | version | weekly downloads | exports | clean | partial | degenerate | uncertified | clean share | misuse-capable | lane |");
  lines.push("| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const entry of result.packages) {
    lines.push(
      `| ${entry.rank} | \`${entry.package}\` | ${entry.version} | ${entry.weeklyDownloads?.toLocaleString("en-US") ?? "?"} | ${entry.total} | ${entry.counts.clean} | ${entry.counts.partial} | ${entry.counts.degenerate} | ${entry.counts.uncertified} | ${percent(ratio(entry.counts.clean, entry.total))} | ${entry.misuseCapable} | ${entry.certifiable ? entry.lane ?? entry.certification ?? "-" : `not certifiable (${entry.rowClass})`} |`
    );
  }
  lines.push("");
  lines.push("## Blocking causes by class");
  lines.push("");
  lines.push("`package-weighted` is the share of the per-package headline the class blocks: each export weighs 1 / its package's surface, averaged over packages.");
  lines.push("");
  lines.push("| class | exports blocked | solely blocked | package-weighted | packages | demanded sites |");
  lines.push("| --- | ---: | ---: | ---: | ---: | ---: |");
  for (const group of result.classes) {
    lines.push(`| ${group.class} | ${group.exports} | ${group.sole} | ${percent(group.packageWeighted ?? 0)} | ${group.packages ?? ""} | ${group.sites} |`);
  }
  if (result.unlockCurve?.length) {
    lines.push("");
    lines.push("## Cause classes, greedily combined (upper bound)");
    lines.push("");
    lines.push("| step | add class | non-clean exports with no remaining cause |");
    lines.push("| ---: | --- | ---: |");
    result.unlockCurve.forEach((step, index) => {
      lines.push(`| ${index + 1} | ${step.add} | ${step.exportsCleared} of ${step.of} |`);
    });
  }
  lines.push("");
  lines.push("## Walls");
  lines.push("");
  lines.push("| class | wall | exports blocked | solely blocked | package-weighted | demanded sites | packages | example |");
  lines.push("| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const wall of result.walls.slice(0, 40)) {
    lines.push(`| ${wall.class} | ${wall.key.replace(/\|/g, "\\|")} | ${wall.exports} | ${wall.sole} | ${percent(wall.packageWeighted ?? 0)} | ${wall.sites} | ${wall.packages.length} | ${wall.examples[0] ?? ""} |`);
  }
  if (result.inferred) {
    lines.push("");
    lines.push("## Walls with graph-lane domains read from the plain lane");
    lines.push("");
    lines.push("A graph-lane row retains no decline and no unresolved claim, so `graph lane: unrecorded` above is an absence of records. This view substitutes the plain lane's record for the same export and domain, and marks the class.");
    lines.push("");
    lines.push("| class | wall | exports blocked | solely blocked | demanded sites | packages |");
    lines.push("| --- | --- | ---: | ---: | ---: | ---: |");
    for (const wall of result.inferred.walls.slice(0, 30)) {
      lines.push(`| ${wall.class} | ${wall.key.replace(/\|/g, "\\|")} | ${wall.exports} | ${wall.sole} | ${wall.sites} | ${wall.packages.length} |`);
    }
  }
  const located = result.walls.filter(wall => wall.topLocations.length > 0).slice(0, 8);
  if (located.length) {
    lines.push("");
    lines.push("## Obligation locations of the attribution catch-all");
    lines.push("");
    for (const wall of located) {
      lines.push(`- ${wall.key}: ${wall.topLocations.map(item => `\`${item.label}\` (${item.count})`).join(", ")}`);
    }
  }
  return `${lines.join("\n")}\n`;
}

// ---------------------------------------------------------------------------
// Corpus refresh (network)
// ---------------------------------------------------------------------------

/// Reviewed Solid 2 packages outside every benchmark family, and build tooling
/// that is never imported by Solid application code. Checked against the
/// registry on each `--select`; these lists are the review, the registry is the
/// fact.
export const EXTERNAL_CANDIDATES = ["solid-use", "cmdk-solid", "@dschz/solid-flow", "@rimelight/ui"];
export const TOOLING = [
  "@solidjs/vite-plugin",
  "babel-preset-solid",
  "solid-refresh",
  "storybook-solidjs-vite",
  "unplugin-solid",
  "vite-plugin-solid"
];

async function select(size, corpusPath, downloadsFile = null) {
  const { Registry } = await import("./ecosystem-benchmark/lib/registry.mjs");
  const { selectRow, solidReleaseCatalog } = await import("./ecosystem-benchmark/lib/select.mjs");
  const manifest = JSON.parse(readFileSync(MANIFEST_PATH, "utf8"));
  const registry = new Registry({ concurrency: 12 });
  const runtime = new Map();
  for (const name of RUNTIME_FOUNDATION) runtime.set(name, await registry.packument(name));
  const catalog = solidReleaseCatalog(runtime);
  const rows = manifest.rows.filter(row => row.solidTarget === "solid2");
  const names = [...new Set([...rows.map(row => row.package), ...EXTERNAL_CANDIDATES, ...TOOLING])];
  const downloadsWindow = {};
  // The downloads API rate-limits bursts (HTTP 429), and a count that silently
  // came back missing would rank a 2.5M-download package last. So requests go
  // four at a time with backoff, and a count still missing fails the selection.
  const getJson = async (url, { required = true } = {}) => {
    let last = null;
    for (let attempt = 0; attempt < 6; attempt += 1) {
      try {
        const response = await fetch(url);
        if (response.ok) return await response.json();
        last = `HTTP ${response.status}`;
        if (response.status === 404) break;
        const retryAfter = Number(response.headers.get("retry-after"));
        if (Number.isFinite(retryAfter) && retryAfter > 0) {
          await new Promise(done => setTimeout(done, Math.min(retryAfter, 60) * 1000));
          continue;
        }
      } catch (error) {
        last = error.message;
      }
      await new Promise(done => setTimeout(done, 500 * 2 ** attempt));
    }
    if (required) throw new Error(`${url} failed: ${last}`);
    return null;
  };
  const downloadsApi = "https://api.npmjs.org";
  const throttled = new Registry({ concurrency: 3 });
  // `--downloads <file>` supplies counts fetched earlier from these same two
  // endpoints (`{ window: { start, end }, counts: { <name>: { weekly, release } } }`),
  // for when the API is rate-limiting this host; the corpus records which.
  const recorded = downloadsFile ? JSON.parse(readFileSync(downloadsFile, "utf8")) : null;
  if (recorded) Object.assign(downloadsWindow, recorded.window ?? {});
  const candidates = await throttled.mapConcurrent(names, async name => {
    let downloads;
    if (recorded) {
      const count = recorded.counts?.[name]?.weekly;
      if (typeof count !== "number") throw new Error(`${downloadsFile} has no last-week download count for ${name}`);
      downloads = { count };
    } else {
      // The scoped name goes unencoded: `@scope/name` is what the API
      // documents, and the `%2F` spelling misses its cache.
      const point = await getJson(`${downloadsApi}/downloads/point/last-week/${name}`);
      if (typeof point?.downloads !== "number") throw new Error(`no last-week download count for ${name}`);
      Object.assign(downloadsWindow, { start: point.start, end: point.end });
      downloads = { count: point.downloads };
    }
    const row = rows.find(candidate => candidate.package === name) ?? null;
    let releaseWeeklyDownloads = null;
    if (row && recorded) {
      releaseWeeklyDownloads = recorded.counts?.[name]?.release ?? null;
    } else if (row) {
      // Per-version counts are context, not the ranking, so a failure here
      // records null rather than failing the selection.
      const versions = await getJson(`${downloadsApi}/versions/${encodeURIComponent(name)}/last-week`, { required: false });
      releaseWeeklyDownloads = versions ? (versions.downloads?.[row.version] ?? 0) : null;
    }
    let detail = null;
    if (!row) {
      const packument = await registry.packument(name);
      const selected = selectRow({ packageName: name, packument, family: "external", status: "official", solidTarget: "solid2", catalog });
      detail =
        selected.kind === "row"
          ? `${name}@${selected.row.version} is Solid 2 compatible (${selected.row.probes.map(probe => probe.kind).join("/")}) but belongs to no benchmark family in lib/families.mjs, so no manifest row exists`
          : `${selected.exclusion.reason}: ${selected.exclusion.detail}`;
    }
    return { package: name, weeklyDownloads: downloads?.count ?? null, releaseWeeklyDownloads, row, detail };
  });
  const exclusions = {};
  for (const name of RUNTIME_FOUNDATION) {
    exclusions[name] = { reason: "runtime-foundation", detail: "ADR 0027: the dialect's reviewed vocabulary, never a package contract" };
  }
  for (const name of TOOLING) {
    exclusions[name] = { reason: "build-tooling", detail: "compile-, dev- or test-runner-time tooling; no Solid application code imports it" };
  }
  const { packages, skipped } = selectCorpus({ candidates, exclusions, size });
  const corpus = {
    format: "solid-checker-certification-metric-corpus",
    corpusVersion: 1,
    measuredOn: new Date().toISOString().slice(0, 10),
    downloads: {
      source: "https://api.npmjs.org/downloads/point/last-week (ranking) and /versions/<name>/last-week (release)",
      fetched: recorded ? (recorded.fetchedAt ?? "recorded earlier") : "live",
      ...downloadsWindow
    },
    manifest: { path: "scripts/ecosystem-benchmark/manifest.json", generatedAt: manifest.generatedAt, auditedSolid2: manifest.auditedSolid2 },
    selection: {
      size,
      rule:
        "rank the manifest's solid2 rows plus EXTERNAL_CANDIDATES and TOOLING by last-week downloads; skip the runtime foundation, build tooling and packages with no manifest row; take the first `size`; measure each by its head probe, or its only probe when the row has one compatible Solid release"
    },
    packages,
    skipped,
    external: candidates
      .filter(candidate => !candidate.row)
      .map(candidate => ({ package: candidate.package, weeklyDownloads: candidate.weeklyDownloads, detail: candidate.detail }))
      .sort((left, right) => (right.weeklyDownloads ?? 0) - (left.weeklyDownloads ?? 0))
  };
  writeFileSync(corpusPath, `${JSON.stringify(corpus, null, 2)}\n`);
  console.log(`pinned ${packages.length} packages to ${corpusPath}`);
}

/// One row per host measurement (ADR 0140): the headline, the buckets, and
/// the dialect-silent class, side by side. `results` are `measure()` outputs.
export function renderHostSummary(results) {
  const lines = [];
  lines.push("# Certification metric, per host");
  lines.push("");
  lines.push("Each host is its own certification of the same 30 packages (ADR 0140); a consumer receives the cases of the host it declares, and a consumer that declares none receives the host-free ones.");
  lines.push("");
  lines.push("| host | clean, per-package mean | clean, by downloads | clean, pooled | exports | clean / partial / degenerate / uncertified | dialect-silent exports | misuse-capable |");
  lines.push("| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |");
  for (const result of results) {
    const head = result.headline;
    const totals = result.totals;
    const silent = (result.classes ?? []).find(entry => entry.class === "dialect-silent")?.exports ?? 0;
    lines.push(
      `| ${result.host ?? "none"} | ${percent(head.perPackage)} | ${percent(head.byDownloads)} | ${head.cleanExports} (${percent(head.pooled)}) | ${head.exports} | ${totals.clean} / ${totals.partial} / ${totals.degenerate} / ${totals.uncertified} | ${silent} | ${totals.misuseCapable} |`
    );
  }
  lines.push("");
  lines.push("## Dialect-silent walls by host");
  lines.push("");
  lines.push("| host | wall | exports blocked | packages |");
  lines.push("| --- | --- | ---: | ---: |");
  for (const result of results) {
    for (const wall of (result.walls ?? []).filter(entry => entry.class === "dialect-silent")) {
      const packages = Array.isArray(wall.packages) ? wall.packages.length : wall.packages;
      lines.push(`| ${result.host ?? "none"} | ${wall.key} | ${wall.exports} | ${packages} |`);
    }
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArguments(argv) {
  const options = { select: false, printProbes: false, run: null, json: null, markdown: null, cleanRetained: false, size: 30, corpus: CORPUS_PATH, downloads: null, summarizeHosts: null };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--select") options.select = true;
    else if (argument === "--print-probes") options.printProbes = true;
    else if (argument === "--clean-retained") options.cleanRetained = true;
    else if (argument === "--run") options.run = argv[++index];
    else if (argument === "--summarize-hosts") options.summarizeHosts = argv[++index].split(",").map(path => resolve(path));
    else if (argument === "--json") options.json = argv[++index];
    else if (argument === "--markdown") options.markdown = argv[++index];
    else if (argument === "--size") options.size = Number(argv[++index]);
    else if (argument === "--corpus") options.corpus = resolve(argv[++index]);
    else if (argument === "--downloads") options.downloads = resolve(argv[++index]);
    else if (argument === "-h" || argument === "--help") {
      console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n\n")[1]);
      process.exit(0);
    } else fail(`unknown argument ${JSON.stringify(argument)}`);
  }
  return options;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.select) {
    await select(options.size, options.corpus, options.downloads);
    return;
  }
  if (options.summarizeHosts) {
    const markdown = renderHostSummary(options.summarizeHosts.map(path => JSON.parse(readFileSync(path, "utf8"))));
    if (options.markdown) writeFileSync(resolve(options.markdown), markdown);
    console.log(markdown);
    return;
  }
  const corpus = JSON.parse(readFileSync(options.corpus, "utf8"));
  if (options.printProbes) {
    const problems = corpusProblems(corpus, JSON.parse(readFileSync(MANIFEST_PATH, "utf8")));
    if (problems.length) fail(`the manifest no longer agrees with the pinned corpus:\n  ${problems.join("\n  ")}\nre-pin with --select`);
    console.log(corpus.packages.map(entry => entry.probe).join("\n"));
    return;
  }
  if (!options.run) fail("one of --select, --print-probes or --run <run.json> is required");
  const run = JSON.parse(readFileSync(resolve(options.run), "utf8"));
  const rows = new Map();
  for (const result of run.results ?? []) {
    if (!corpus.packages.some(entry => entry.probe === result.probeId)) continue;
    const row = readRetainedRow(result);
    if (!row && result.retainedArtifacts?.outputDir) fail(`${result.probeId}: ${result.retainedArtifacts.outputDir} is gone; re-run make certification-metric`);
    rows.set(result.probeId, row);
  }
  const demand = JSON.parse(readFileSync(DEMAND_PATH, "utf8")).rows ?? [];
  const result = measure({ run, corpus, rows, demandRows: demand });
  const markdown = renderMarkdown(result);
  if (options.json) writeFileSync(resolve(options.json), `${JSON.stringify(result, null, 2)}\n`);
  if (options.markdown) writeFileSync(resolve(options.markdown), markdown);
  console.log(markdown);
  if (options.cleanRetained) {
    // The retained trees are hundreds of megabytes each and, left in $TMPDIR,
    // are indexed by Spotlight and slow the next timed run. The measurement is
    // written above; nothing else reads them.
    for (const result of run.results ?? []) {
      for (const directory of [result.retainedArtifacts?.projectDir, result.retainedArtifacts?.outputDir]) {
        if (directory && /\/solid-checker-ecosystem-[^/]+$/.test(directory) && existsSync(directory) && statSync(directory).isDirectory()) {
          rmSync(directory, { recursive: true, force: true });
        }
      }
    }
  }
  if (result.missingProbes.length) fail(`the run is missing ${result.missingProbes.length} corpus probe(s)`);
}

if (import.meta.main) await main();
