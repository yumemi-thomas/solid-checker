// Reads what a successfully generated contract actually *says*, as opposed to
// whether it was emitted at all.
//
// The rest of the benchmark measures generation reachability: a probe either
// produced a contract or it did not, and if it did not, which failure class
// explains that. That question is answered entirely by the process exit status
// and stderr, so nothing outside this module ever opens the emitted document.
//
// This module answers the different question a machine-verification scheme
// asks: given that a contract exists, how much of it is a *claim* and how much
// is the `{"status": "unknown"}` sentinel — the schema's explicit "this stays
// uncertifiable" marker (schema/solid-reactivity.schema.json's `unknownClaim`).
// A contract full of unknowns is a completely successful generation and a
// nearly worthless proof, and the reachability numbers cannot tell those apart.
//
// Three deliberate choices, because each one is a place where a number could
// quietly mean something else:
//
// - **Counting is per export NAME, never per summary id.** A contract's
//   `entrypoints[ep].exports` maps one summary id to every export that shares
//   it, so `@tanstack/solid-query` has 17 summaries covering 57 exports. A
//   consumer imports a name, not a summary id, so an unknown on a shared
//   summary is unknown for every name that resolves to it.
// - **A variant's unknown is the export's unknown.** An export with
//   `variants` carries one summary per condition set; a domain is counted
//   unknown for that export when the default summary OR any variant says
//   unknown, because a consumer resolving to that condition gets the
//   uncertifiable answer. It is counted ONCE per (export, domain) either way,
//   so an export with the same unknown in five variants is one unknown, not
//   five. (The review plan deliberately does the opposite and lists each
//   variant separately — those are review questions, not claim counts.)
// - **An ABSENT domain is a positive claim, not an unknown.** A function
//   summary with no `callbacks` key asserts that the export never invokes a
//   caller-supplied callback. That is exactly the claim the review plan files
//   under `no-callback-row` for a human to confirm; it is not the
//   `{"status":"unknown"}` sentinel and must never be counted as one, or the
//   measurement would report the checker as uncertain precisely where it was
//   most confident.

import { readFileSync } from "node:fs";

// The five claim domains whose value may be the `{"status": "unknown"}`
// sentinel, in schema order. Kept as this module's own list rather than
// imported from packages/cli/scripts/contract-review-plan.mjs's
// `SENTINEL_CLAIMS`: the benchmark measures the CLI as an external artifact
// through its published output, and importing the generator's own constant
// would make the measurement agree with the generator by construction. The two
// lists must match; schema/solid-reactivity.schema.json is what they both
// answer to.
export const CLAIM_DOMAINS = ["callbacks", "reactiveReads", "returns", "ownerRequirements", "asyncBehavior"];

// The behavioral row kinds a future probe step would have to drive to turn an
// inferred claim into a probed one. These are the *positive* rows: what the
// contract asserts happens, as opposed to what it declines to assert.
export const BEHAVIORAL_ROW_KINDS = [
  "callbackExecution",
  "reactiveRead",
  "returnTree",
  "ownerRequirement",
  "asyncBehavior"
];

export function isUnknownClaim(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    value.status === "unknown" &&
    Object.keys(value).length === 1
  );
}

function knownRows(value) {
  return Array.isArray(value) ? value : [];
}

export function emptyDomainCounts() {
  return Object.fromEntries(CLAIM_DOMAINS.map(domain => [domain, 0]));
}

export function emptyBehavioralRows() {
  return Object.fromEntries(BEHAVIORAL_ROW_KINDS.map(kind => [kind, 0]));
}

// Default summary first, then every variant summary. Order matters only for
// determinism of the samples; the counts are order-independent.
function summaryNodes(summary) {
  const nodes = [summary];
  for (const variant of Array.isArray(summary?.variants) ? summary.variants : []) {
    if (variant?.summary && typeof variant.summary === "object") nodes.push(variant.summary);
  }
  return nodes;
}

/**
 * Counts the claim content of one emitted `solid-reactivity.json`.
 *
 * Returns `null` for anything that is not a parsed object, so a caller can
 * distinguish "measured and empty" from "could not be measured" rather than
 * reporting an unreadable contract as a perfectly clean one.
 */
export function summarizeContractDocument(contract) {
  if (!contract || typeof contract !== "object") return null;

  const summaries = contract.summaries ?? {};
  const entrypoints = contract.entrypoints ?? {};

  const unknownByDomain = emptyDomainCounts();
  const behavioralRows = emptyBehavioralRows();
  let exportsTotal = 0;
  let exportsProven = 0;
  let exportsWithUnknown = 0;
  let exportsWithoutSummary = 0;
  // Two shapes worth separating from a plain unknown count, because they
  // describe different amounts of missing knowledge:
  //
  // - all five domains unknown at once is not five independent gaps, it is one
  //   export the generator could say nothing at all about. This is the shape
  //   that dominates the real corpus (a conditional-export branch pointing at
  //   TypeScript source, whose whole summary comes back uncertifiable), and a
  //   per-domain table alone reports it as five even-looking columns.
  // - an unknown present ONLY inside variants means the default resolution is
  //   fully claimed and the uncertainty is confined to condition sets a given
  //   consumer may never select.
  let exportsAllDomainsUnknown = 0;
  let exportsUnknownOnlyInVariants = 0;

  for (const entrypoint of Object.values(entrypoints)) {
    const exportsMap = entrypoint?.exports ?? {};
    for (const [summaryId, names] of Object.entries(exportsMap)) {
      const summary = summaries[summaryId];
      const exportNames = Array.isArray(names) ? names : [];
      for (const _name of exportNames) {
        exportsTotal += 1;
        if (!summary || typeof summary !== "object") {
          // A dangling summary id is a hole, not a proof. It cannot be counted
          // as proven and it cannot be attributed to a claim domain either.
          exportsWithoutSummary += 1;
          continue;
        }

        const nodes = summaryNodes(summary);
        let unknownDomainsHere = 0;
        let unknownOnDefault = false;
        for (const domain of CLAIM_DOMAINS) {
          if (nodes.some(node => isUnknownClaim(node?.[domain]))) {
            unknownByDomain[domain] += 1;
            unknownDomainsHere += 1;
            if (isUnknownClaim(summary[domain])) unknownOnDefault = true;
          }
        }
        if (unknownDomainsHere > 0) {
          exportsWithUnknown += 1;
          if (unknownDomainsHere === CLAIM_DOMAINS.length) exportsAllDomainsUnknown += 1;
          if (!unknownOnDefault) exportsUnknownOnlyInVariants += 1;
        } else {
          exportsProven += 1;
        }

        for (const node of nodes) {
          behavioralRows.callbackExecution += knownRows(node?.callbacks).length;
          behavioralRows.reactiveRead += knownRows(node?.reactiveReads).length;
          behavioralRows.ownerRequirement += knownRows(node?.ownerRequirements).length;
          if (node?.returns && !isUnknownClaim(node.returns)) behavioralRows.returnTree += 1;
          if (typeof node?.asyncBehavior === "string") behavioralRows.asyncBehavior += 1;
        }
      }
    }
  }

  return {
    entrypointsEmitted: Object.keys(entrypoints).length,
    exportsTotal,
    exportsProven,
    exportsWithUnknown,
    exportsAllDomainsUnknown,
    exportsUnknownOnlyInVariants,
    exportsWithoutSummary,
    unknownByDomain,
    unknownTotal: CLAIM_DOMAINS.reduce((total, domain) => total + unknownByDomain[domain], 0),
    behavioralRows
  };
}

/**
 * Counts the parts of the sibling `<contract>.review.json` that describe what
 * generation could NOT bind or certify.
 *
 * `refusedEntrypoints` here comes from the plan's own `refused-entrypoint`
 * items, which name each refused subpath and why. That is strictly more than
 * the stdout `; N entrypoint(s) refused and omitted` count the classifier
 * reads, and the two must agree — a disagreement is reported by the caller
 * rather than silently preferring one.
 *
 * `closureNotes` are the `generation.entrypoints[*].notes` entries. Each one
 * says the runtime-module closure behind an entrypoint could not be fully
 * enumerated or hashed, so the contract is bound to fewer bytes than it
 * describes. Under a machine-verification scheme that is decisive: a contract
 * carrying a closure note cannot be byte-attested, however few unknowns its
 * claims contain.
 */
export function summarizeReviewPlan(plan) {
  if (!plan || typeof plan !== "object") return null;

  const items = Array.isArray(plan.items) ? plan.items : [];
  const itemsByKind = {};
  for (const item of items) {
    const kind = typeof item?.kind === "string" ? item.kind : "unknown-kind";
    itemsByKind[kind] = (itemsByKind[kind] ?? 0) + 1;
  }

  const refusedEntrypointNames = items
    .filter(item => item?.kind === "refused-entrypoint")
    .map(item => item?.target?.entrypoint)
    .filter(name => typeof name === "string")
    .sort();

  const closures = plan.generation?.entrypoints ?? {};
  const closureNotes = [];
  for (const [entrypoint, record] of Object.entries(closures)) {
    for (const note of Array.isArray(record?.notes) ? record.notes : []) {
      closureNotes.push(`${entrypoint} ${note}`);
    }
  }
  closureNotes.sort();

  return {
    checklistItems: items.length,
    itemsByKind,
    refusedEntrypoints: refusedEntrypointNames.length,
    refusedEntrypointNames,
    closureNotes: closureNotes.length,
    // Bounded: a note is prose of unbounded length and the JSON report already
    // carries one line per probe. Three is enough to recognize the shape; the
    // count above is the measurement.
    closureNoteSamples: closureNotes.slice(0, 3)
  };
}

/**
 * The per-probe content block the report carries alongside the existing
 * reachability fields.
 *
 * `measured` is the field every consumer must read first. A probe that emitted
 * a contract this module could not parse is `measured: false` with a `note`,
 * never a row of zeroes — a zero here would read as "no unknowns", which is
 * the single most misleading thing this measurement could say.
 *
 * `fullyProven` is the strictest reading available from the emitted artifacts:
 * no unknown sentinel in any domain, no export missing a summary, no refused
 * entrypoint, and no closure note. It is a property of the generated *draft*,
 * not of the package — see docs/ecosystem-benchmark.md on demand sensitivity.
 */
export function summarizeContract({ contract, reviewPlan, refusedEntrypointsFromStdout = null }) {
  const document = summarizeContractDocument(contract);
  if (!document) {
    return {
      measured: false,
      note: "contract document missing or unparsable",
      fullyProven: null
    };
  }

  const plan = summarizeReviewPlan(reviewPlan);
  const refusedEntrypoints = plan ? plan.refusedEntrypoints : refusedEntrypointsFromStdout;
  const closureNotes = plan ? plan.closureNotes : null;

  // The two independent statements about refusals. They come from different
  // artifacts (the generator's stdout line and the review plan's items), so a
  // disagreement means one of them is being read wrong, and that is worth
  // saying rather than resolving by preference.
  const refusalDisagreement =
    plan && typeof refusedEntrypointsFromStdout === "number" && refusedEntrypointsFromStdout !== plan.refusedEntrypoints
      ? { stdout: refusedEntrypointsFromStdout, reviewPlan: plan.refusedEntrypoints }
      : null;

  const fullyProven =
    document.exportsWithUnknown === 0 &&
    document.exportsWithoutSummary === 0 &&
    (refusedEntrypoints ?? 0) === 0 &&
    (closureNotes ?? 0) === 0 &&
    plan !== null;

  return {
    measured: true,
    fullyProven,
    entrypointsEmitted: document.entrypointsEmitted,
    entrypointsRefused: refusedEntrypoints ?? null,
    refusedEntrypointNames: plan?.refusedEntrypointNames ?? [],
    exportsTotal: document.exportsTotal,
    exportsProven: document.exportsProven,
    exportsWithUnknown: document.exportsWithUnknown,
    exportsAllDomainsUnknown: document.exportsAllDomainsUnknown,
    exportsUnknownOnlyInVariants: document.exportsUnknownOnlyInVariants,
    exportsWithoutSummary: document.exportsWithoutSummary,
    unknownByDomain: document.unknownByDomain,
    unknownTotal: document.unknownTotal,
    behavioralRows: document.behavioralRows,
    closureNotes: closureNotes ?? null,
    closureNoteSamples: plan?.closureNoteSamples ?? [],
    reviewPlanItems: plan?.checklistItems ?? null,
    reviewPlanItemsByKind: plan?.itemsByKind ?? null,
    ...(plan === null ? { note: "review plan missing or unparsable" } : {}),
    ...(refusalDisagreement ? { refusalDisagreement } : {})
  };
}

// `<contract>.json` -> `<contract>.review.json`, matching
// packages/cli/scripts/contract-review-plan.mjs#reviewPlanJsonPath exactly.
export function reviewPlanPathFor(contractPath) {
  return contractPath.toLowerCase().endsWith(".json")
    ? `${contractPath.slice(0, -5)}.review.json`
    : `${contractPath}.review.json`;
}

function readJsonOrNull(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

/**
 * Reads the emitted contract and its sibling review plan off disk. Must be
 * called before the probe's temporary directories are cleaned up.
 */
export function readContractContent(contractPath, refusedEntrypointsFromStdout = null) {
  return summarizeContract({
    contract: readJsonOrNull(contractPath),
    reviewPlan: readJsonOrNull(reviewPlanPathFor(contractPath)),
    refusedEntrypointsFromStdout
  });
}
