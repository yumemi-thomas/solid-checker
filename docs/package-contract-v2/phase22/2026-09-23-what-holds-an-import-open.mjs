// Why each demanded import still finds a claim domain open.
//
// Read-only: it certifies nothing and runs no checker. It reads one retained
// coverage-census run -- the `run.json` a `make contract-coverage-census` writes,
// whose rows keep their output directories -- and, for every demanded
// in-surface export the census's consumer view does not clear, names why each
// open consumer domain is open:
//
//   bun docs/package-contract-v2/phase22/2026-09-23-what-holds-an-import-open.mjs \
//     [--run rust/target/coverage-census/run.json] [--json <out.json>]
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
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

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

const rows = [];
for (const result of run.results ?? []) {
  const directory = result.retainedArtifacts?.outputDir;
  if (!directory) continue;
  const files = filesUnder(directory);
  if (files.length === 0) {
    console.error(`${result.probeId}: ${directory} is gone; re-run make contract-coverage-census`);
    process.exit(1);
  }
  const read = suffix => {
    const path = files.find(file => file.endsWith(suffix));
    return path ? JSON.parse(readFileSync(path, "utf8")) : null;
  };
  rows.push({
    probe: result.probeId,
    documents: files
      .filter(file => file.endsWith(".main.json"))
      .map(file => JSON.parse(readFileSync(file, "utf8"))),
    audit: read(".certification-audit.json"),
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
  for (const document of row.documents) {
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
            best.set(key, { view, closed, row, entrypoint, score });
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

function status(row, exportName, entrypoint, domain) {
  const claims = [...(row.proposal?.unresolvedClaims ?? []), ...(row.proposal?.closureCandidates ?? [])];
  const cases = new Set(
    claims
      .filter(claim => claim.artifact?.entrypoint === entrypoint && claim.subject?.export === exportName)
      .map(claim => claim.subject.artifactCase)
  );
  const withheld = (row.audit?.withheldClosures ?? []).filter(
    entry => entry.export === exportName && entry.domain === domain
  );
  const hit = withheld.find(entry => cases.has(entry.artifactCase)) ?? withheld[0];
  if (hit) return { status: `withheld: ${hit.reason.split(":")[0]}`, detail: scrub(hit.reason) };
  const withdrawn = (row.audit?.withheldOperations ?? []).filter(
    entry =>
      entry.export === exportName &&
      operationDomain(row.generated, exportName, entry.operation.split(":operation:").pop()) === domain
  );
  const operation = withdrawn.find(entry => cases.has(entry.artifactCase)) ?? withdrawn[0];
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
        [...new Set(declined.map(entry => `${entry.kind} ${entry.package} ${entry.callee}`.trim()))].join("; ")
      )
    };
  }
  const about = claim =>
    claim.artifact?.entrypoint === entrypoint &&
    claim.subject?.export === exportName &&
    claim.subject?.path?.domain === domain;
  if ((row.proposal?.unresolvedClaims ?? []).some(about)) return { status: "never proposed", detail: "" };
  if ((row.proposal?.closureCandidates ?? []).some(about)) {
    return { status: "proposed, not certified", detail: "" };
  }
  return { status: "no record in the answering row", detail: "" };
}

const exports = [];
const bySiteClass = {};
for (const demandRow of demand) {
  const answer = best.get(`${demandRow.package}|${demandRow.export}`);
  if (!answer || answer.view === "value" || answer.view === "clean") continue;
  const open = CONSUMER_DOMAINS.filter(domain => !answer.closed.has(domain)).map(domain => ({
    domain,
    ...status(answer.row, demandRow.export, answer.entrypoint, domain)
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
    `${JSON.stringify({ run: run.finishedAt ?? null, bySiteClass, onlyReturns: onlyReturns.map(entry => ({ package: entry.package, export: entry.export, sites: entry.sites })), exports }, null, 2)}\n`
  );
}
