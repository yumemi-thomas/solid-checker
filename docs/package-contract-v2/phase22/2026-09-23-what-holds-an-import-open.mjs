// Why each demanded import still finds a claim domain open.
//
// Read-only: it certifies nothing and runs no checker. It reads one retained
// coverage-census run -- the `run.json` a `make contract-coverage-census` writes,
// whose rows keep their output directories -- and, for every demanded
// in-surface export the census's consumer view does not clear, names why each
// open consumer domain is open:
//
//   bun docs/package-contract-v2/phase22/2026-09-23-what-holds-an-import-open.mjs \
//     [--run rust/target/coverage-census/run.json] [--outputs <dir>] [--json <out.json>]
//
// `--outputs` reads each row's output directory from `<dir>/<basename>`
// instead of the path `run.json` recorded, for a copy taken before the next
// census run replaced the originals.
//
// The best answer per export follows the census exactly (`consumerState`,
// strongest across nameable entrypoints), with two tie-breaks the census does
// not need and this does: more closed consumer domains, then the package's own
// row. A package is also certified as a dependency node of other rows, and those
// documents can close less -- `@solid-primitives/utils`' `noop` closes three
// domains in its own row and two in another's -- so the reason has to come from
// the row whose document gave the answer.
//
// Each open domain gets exactly one status, read from that row:
//
//   withheld: <reason>        the certification audit withdrew the candidate
//   withheld operation: <reason>
//                             the audit withdrew the operation the domain
//                             listed, and the domain opened with it
//   declined: <kind>          the generator recorded a closure decline
//   never proposed            the proposal left the claim unresolved and no
//                             decline names it -- the status a ledger built
//                             from withheld closures cannot see at all
//   proposed, not certified   a candidate with neither outcome recorded
//
// Strict since 2026-09-24 (ways-to-improve § 3.1 a, b). A status is read only
// from records about the artifact cases that gave the answer:
//
// - Plain lane: the cases the proposal lists for the answering entrypoint,
//   which is nameable (the census skips wildcard-reached ones). A withheld
//   closure or operation from any other case -- `@kobalte/utils`' `./src/*.ts`
//   cases, which no consumer imports -- is not this export's status, and there
//   is no fallback to one. An entrypoint with two cases whose statuses differ
//   reports the stronger and names both in `cases`.
// - Graph lane (`lane: published-graph`): the answering document is a graph
//   node's, and the plain lane's proposal, refusals and generated document
//   describe a different artifact case (the root before its dependency was
//   accepted). The node's own records are the audit's withheld closures and
//   withheld operations carrying that `node.digest`. The graph lane retains no
//   decline and no unresolved claim, so a domain with neither record is
//   `never proposed on the graph lane`, inferred from the row's candidate
//   reconciliation (candidates = closed + withheld + withheld operations), and
//   says so when that sum does not reconcile exactly.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";

import {
  consumerState,
  nameableEntrypoint
} from "../../../scripts/contract-coverage-census.mjs";

const root = resolve(import.meta.dirname, "../../..");
const DEMAND = join(root, "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json");
const CONSUMER_DOMAINS = ["callbacks", "reads", "returns", "creates"];
const CONSUMER_RANK = { value: 4, clean: 3, "some-uses": 2, "every-import": 1 };

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

const runPath = resolve(argument("--run", join(root, "rust/target/coverage-census/run.json")));
const jsonPath = argument("--json", null);
const outputsPath = argument("--outputs", null);
const run = JSON.parse(readFileSync(runPath, "utf8"));
const demand = JSON.parse(readFileSync(DEMAND, "utf8")).rows;

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

/// The generator's own operation-id scheme (`inferred_contract.rs`,
/// `{case}:{export}:operation:{return|return-N|callback-N|read-N}`): the domain
/// a withdrawn operation was listed in, where no generated document for its
/// artifact case survives -- a graph node's is regenerated in private scratch.
function domainOfOperationId(local) {
  if (/^return(-\d+)?$/.test(local)) return "returns";
  if (/^callback-\d+$/.test(local)) return "callbacks";
  if (/^read-\d+$/.test(local)) return "reads";
  return null;
}

const rows = [];
for (const result of run.results ?? []) {
  const recorded = result.retainedArtifacts?.outputDir;
  if (!recorded) continue;
  const directory = outputsPath ? join(resolve(outputsPath), basename(recorded)) : recorded;
  const files = filesUnder(directory);
  if (files.length === 0) {
    console.error(`${result.probeId}: ${directory} is gone; re-run make contract-coverage-census`);
    process.exit(1);
  }
  const read = suffix => {
    const path = files.find(file => file.endsWith(suffix));
    return path ? JSON.parse(readFileSync(path, "utf8")) : null;
  };
  const lane = result.certificationAttempt?.lane ?? null;
  const auditRecord = read(".certification-audit.json");
  const graphFile = files.find(file => file.endsWith("/graph.json") && file.includes("/graphs/"));
  const graph = graphFile ? JSON.parse(readFileSync(graphFile, "utf8")) : null;
  // A multi-case row publishes a case set instead of one graph: each case's
  // root under `case-sets/<set>/cases/<import root>/`, its dependency nodes
  // under `case-sets/<set>/graph-nodes/<import root>/<node>/`. The case-set
  // index names the artifact case each import root is.
  const caseByImportRoot = new Map();
  for (const file of files.filter(file => /\/case-sets\/[0-9a-f]+\/accepted-contract-case-set\.json$/.test(file))) {
    for (const entry of JSON.parse(readFileSync(file, "utf8")).cases ?? []) {
      caseByImportRoot.set(String(entry.resolvedImportRoot).replace(/^sha256:/, ""), entry.artifactCaseId);
    }
  }
  const graphRecords = [
    ...(auditRecord?.withheldClosures ?? []),
    ...(auditRecord?.withheldOperations ?? []),
    ...(auditRecord?.certifiedClosures?.closed ?? [])
  ].filter(record => record.node?.digest);
  // Which graph node (or, for a case-set root, which artifact case) each
  // accepted document is. The row's top-level catalog object is a byte copy of
  // the root node's, so it is attributed by bytes, not by where it sits.
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
  const byBytes = new Map();
  for (const file of files.filter(file => file.endsWith(".main.json"))) {
    const where = attribution(file);
    if (where) byBytes.set(readFileSync(file, "utf8"), where);
  }
  const documents = [];
  const seen = new Set();
  for (const file of files.filter(file => file.endsWith(".main.json"))) {
    const bytes = readFileSync(file, "utf8");
    const where = lane === "published-graph" ? (byBytes.get(bytes) ?? { node: null, artifactCase: null }) : null;
    const key = `${JSON.stringify(where)}\u0000${bytes}`;
    if (seen.has(key)) continue;
    seen.add(key);
    documents.push({ document: JSON.parse(bytes), graph: where });
  }
  const candidates = auditRecord?.closureCandidates ?? null;
  const reconciliation =
    lane === "published-graph" && candidates
      ? {
          candidates: candidates.count,
          closed: (auditRecord.certifiedClosures?.closed ?? []).reduce(
            (sum, record) => sum + (record.closed?.length ?? 0),
            0
          ),
          withheld: (auditRecord.withheldClosures ?? []).length,
          withheldOperations: (auditRecord.withheldOperations ?? []).length
        }
      : null;
  rows.push({
    probe: result.probeId,
    lane,
    reconciliation,
    documents,
    audit: auditRecord,
    proposal: read(".proposal.json"),
    refusals: read(".refusals.json"),
    // The generated document itself, the one file whose name ends at `.json`:
    // the only place a withdrawn operation's domain is still written down.
    generated: (() => {
      const path = files.find(file => /\.json$/.test(file) && !/\.json\./.test(file.slice(0, -5)) && !file.endsWith(".main.json"));
      return path ? JSON.parse(readFileSync(path, "utf8")) : null;
    })()
  });
}

const byScore = (left, right) => {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
};

// (package, export) -> the document answer a consumer gets, and where it is from.
const best = new Map();
for (const row of rows) {
  for (const { document, graph } of row.documents) {
    const name = document.package?.name;
    for (const [entrypoint, { cases }] of Object.entries(document.entrypoints ?? {})) {
      if (!nameableEntrypoint(entrypoint)) continue;
      for (const artifactCase of cases ?? []) {
        for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
          const id = typeof reference === "string" ? reference : reference?.summary;
          const closed = new Set(document.summaries?.[id]?.call?.closed ?? []);
          const view = consumerState(document, reference);
          const score = [
            CONSUMER_RANK[view],
            CONSUMER_DOMAINS.filter(domain => closed.has(domain)).length,
            row.probe.startsWith(`${name}@`) ? 1 : 0
          ];
          const key = `${name}|${exportName}`;
          const held = best.get(key);
          if (!held || byScore(score, held.score) > 0) {
            best.set(key, { view, closed, row, entrypoint, graph, score });
          }
        }
      }
    }
  }
}

/// Reasons name the certification's private temporary projects; those paths
/// identify a machine and a moment, not a claim, so evidence keeps the rest.
function scrub(reason) {
  return reason
    .replace(/(?:\/private)?\/var\/folders\/[^\s"]*?\/T\/[^\s"/]+/g, "<tmp>")
    .replace(/\/Users\/[^\s"/]+/g, "<home>");
}

/// The consumer domain whose enumeration lists operation `local` for
/// `exportName` in the generated document, or `null`.
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

/// One plain-lane artifact case's status for (export, domain). Records from
/// every other case are not consulted.
function plainCaseStatus(row, exportName, entrypoint, artifactCase, domain) {
  const hit = (row.audit?.withheldClosures ?? []).find(
    entry =>
      !entry.node &&
      entry.export === exportName &&
      entry.domain === domain &&
      entry.artifactCase === artifactCase
  );
  if (hit) return { status: `withheld: ${hit.reason.split(":")[0]}`, detail: scrub(hit.reason) };
  const operation = (row.audit?.withheldOperations ?? []).find(
    entry =>
      !entry.node &&
      entry.export === exportName &&
      entry.artifactCase === artifactCase &&
      operationDomain(row.generated, exportName, entry.operation.split(":operation:").pop()) === domain
  );
  if (operation) {
    return {
      status: `withheld operation: ${operation.reason.split(":")[0].replace(/^operation /, "")}`,
      detail: scrub(operation.reason)
    };
  }
  const declined = (row.refusals?.declinedClosures ?? []).filter(
    entry => entry.export === exportName && entry.domain === domain && entry.entrypoint === entrypoint
  );
  if (declined.length > 0) {
    return {
      status: `declined: ${declined[0].kind}`,
      detail: scrub(
        [
          ...new Set(
            declined.map(entry =>
              [entry.kind, entry.package, entry.callee, entry.shape, entry.spelling && `\`${entry.spelling}\``, entry.location]
                .filter(Boolean)
                .join(" ")
            )
          )
        ].join("; ")
      )
    };
  }
  const about = claim =>
    claim.subject?.artifactCase === artifactCase &&
    claim.subject?.export === exportName &&
    claim.subject?.path?.domain === domain;
  if ((row.proposal?.unresolvedClaims ?? []).some(about)) return { status: "never proposed", detail: "" };
  if ((row.proposal?.closureCandidates ?? []).some(about)) {
    return { status: "proposed, not certified", detail: "" };
  }
  return { status: "no record in the answering case", detail: "" };
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
  if (cases.length === 0) return { status: "no record in the answering case", detail: "", cases: [] };
  const perCase = cases.map(artifactCase => ({
    artifactCase,
    ...plainCaseStatus(row, exportName, entrypoint, artifactCase, domain)
  }));
  const chosen = [...perCase].sort((left, right) => statusRank(left) - statusRank(right))[0];
  return {
    status: chosen.status,
    detail: chosen.detail,
    cases: perCase.map(({ artifactCase, status }) => ({ artifactCase, status }))
  };
}

/// A graph node's status for (export, domain), from the audit records carrying
/// that node's digest. The plain lane's files describe a different case.
function graphStatus(row, { node, artifactCase }, exportName, domain) {
  if (!node && !artifactCase) {
    return { status: "no graph-lane attribution for the answering document", detail: "", cases: [] };
  }
  const mine = entry =>
    entry.node?.digest &&
    entry.export === exportName &&
    ((node && entry.node.digest === node) || (artifactCase && entry.artifactCase === artifactCase));
  const withheld = (row.audit?.withheldClosures ?? []).filter(entry => mine(entry) && entry.domain === domain);
  const cases = [...new Set(withheld.map(entry => entry.artifactCase))];
  if (withheld.length > 0) {
    const hit = withheld[0];
    return { status: `withheld: ${hit.reason.split(":")[0]}`, detail: scrub(hit.reason), cases };
  }
  const operation = (row.audit?.withheldOperations ?? []).find(
    entry => mine(entry) && domainOfOperationId(entry.operation.split(":operation:").pop()) === domain
  );
  if (operation) {
    return {
      status: `withheld operation: ${operation.reason.split(":")[0].replace(/^operation /, "")}`,
      detail: scrub(operation.reason),
      cases: [operation.artifactCase]
    };
  }
  const reconciliation = row.reconciliation;
  const sum = reconciliation
    ? reconciliation.closed + reconciliation.withheld + reconciliation.withheldOperations
    : null;
  const exact = reconciliation && sum === reconciliation.candidates;
  return {
    status: "never proposed on the graph lane",
    detail: reconciliation
      ? `no withheld record for ${node ? `node ${node.slice(7, 15)}` : artifactCase}; row candidates ${reconciliation.candidates} ` +
        `${exact ? "=" : "vs"} closed ${reconciliation.closed} + withheld ${reconciliation.withheld} + ` +
        `withheld operations ${reconciliation.withheldOperations}` +
        (exact ? "" : ` (${sum}; not exact, so a lost candidate cannot be excluded)`) +
        "; the graph lane retains no declines and no unresolved claims"
      : `no withheld record for ${node ? `node ${node.slice(7, 15)}` : artifactCase} and no candidate count`,
    cases: []
  };
}

function status(answer, exportName, domain) {
  return answer.graph
    ? graphStatus(answer.row, answer.graph, exportName, domain)
    : plainStatus(answer.row, exportName, answer.entrypoint, domain);
}

const exports = [];
const bySiteClass = {};
for (const demandRow of demand) {
  const answer = best.get(`${demandRow.package}|${demandRow.export}`);
  if (!answer || answer.view === "value" || answer.view === "clean") continue;
  const open = CONSUMER_DOMAINS.filter(domain => !answer.closed.has(domain)).map(domain => ({
    domain,
    ...status(answer, demandRow.export, domain)
  }));
  for (const entry of open) {
    const key = `${answer.view} | ${entry.domain} | ${entry.status}`;
    bySiteClass[key] = (bySiteClass[key] ?? 0) + demandRow.sites;
  }
  exports.push({
    package: demandRow.package,
    export: demandRow.export,
    sites: demandRow.sites,
    view: answer.view,
    entrypoint: answer.entrypoint,
    row: answer.row.probe,
    lane: answer.graph ? "published-graph" : "plain",
    ...(answer.graph ? { graph: answer.graph } : {}),
    open
  });
}
exports.sort((left, right) => right.sites - left.sites || left.export.localeCompare(right.export));

const onlyReturns = exports.filter(entry => entry.open.length === 1 && entry.open[0].domain === "returns");

const lines = [`run ${run.finishedAt ?? "?"} (${runPath})`, "", "sites by (view | open domain | status):"];
for (const [key, sites] of Object.entries(bySiteClass).sort((left, right) => right[1] - left[1])) {
  lines.push(`${String(sites).padStart(6)}  ${key}`);
}
lines.push("");
lines.push(
  `exports whose only open consumer domain is returns: ${onlyReturns.length}, ` +
    `${onlyReturns.reduce((sum, entry) => sum + entry.sites, 0)} sites`
);
for (const entry of onlyReturns) {
  lines.push(`${String(entry.sites).padStart(6)}  ${entry.package} ${entry.export} (${entry.open[0].status})`);
}
lines.push("");
lines.push("per export:");
for (const entry of exports) {
  const open = entry.open.map(item => `${item.domain}=${item.status}`).join(", ");
  lines.push(`${String(entry.sites).padStart(6)}  ${entry.package} ${entry.export} [${entry.view}] ${open}`);
}
console.log(lines.join("\n"));

if (jsonPath) {
  writeFileSync(
    resolve(jsonPath),
    `${JSON.stringify({ run: run.finishedAt ?? null, strict: true, reconciliation: Object.fromEntries(rows.filter(row => row.reconciliation).map(row => [row.probe, row.reconciliation])), bySiteClass, onlyReturns: onlyReturns.map(entry => ({ package: entry.package, export: entry.export, sites: entry.sites })), exports }, null, 2)}\n`
  );
}
