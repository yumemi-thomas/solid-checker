// The review plan a generated contract ships with, in both shapes it is read
// in: the `<contract>.review.md` checklist a human reads, and the
// `<contract>.review.json` plan `solid-checker contract review` resolves item
// by item.
//
// Both come from one traversal, because they must not be able to disagree: a
// checklist line with no machine item behind it is a review question the
// promotion gate cannot enforce, and an item with no line is one a reviewer
// never sees. The item `kind`s are exactly the checklist's sections; this file
// adds no review question the checklist does not already ask.

import { createHash } from "node:crypto";

export const REVIEW_PLAN_SCHEMA_VERSION = 1;

/// The claim domains whose value may be the `{"status": "unknown"}` sentinel.
export const SENTINEL_CLAIMS = [
  "reactiveReads",
  "returns",
  "callbacks",
  "ownerRequirements",
  "asyncBehavior"
];

/// Section title per item kind, in the order the checklist renders them.
const SECTIONS = [
  // Generation's refusals, and -- since RFC 0002 amendment A9 -- verification's:
  // `contract verify` pushes the same item for an entrypoint whose `kind` claims
  // no run observed, so the section is no longer the generator's alone.
  ["entrypoints refused as uncertifiable", "refused-entrypoint"],
  ["legacy entrypoint resolution", "legacy-root-field"],
  ["contract artifact binding", "artifact-binding"],
  ["exports with no summary", "no-export-summary"],
  ["unknown export claims", "unknown-sentinel"],
  ["callbacks with no execution row", "no-callback-row"],
  ["callbacks with no owner row", "callback-without-owner-row"],
  ["owner requirements requiring review", "generated-owner-requirement"],
  ["inherited rows", "inherited-claim"],
  ["environment-branching exports", "conditional-environment"],
  ["generated export summaries", "generated-summary"]
];

/// Kinds a contract document alone witnesses, so an edit to it can be checked.
/// The others describe generation itself -- a refused entrypoint, the legacy
/// manifest field a root resolved from, an artifact binding schema v1 cannot
/// carry -- and nothing in the contract's bytes can confirm or deny them.
export const CONTRACT_WITNESSED_KINDS = new Set([
  "unknown-sentinel",
  "no-callback-row",
  "callback-without-owner-row",
  "generated-owner-requirement",
  "inherited-claim",
  "conditional-environment"
]);

export function isUnknownClaim(value) {
  return value?.status === "unknown" && Object.keys(value).length === 1;
}

function knownRows(value) {
  return Array.isArray(value) ? value : [];
}

/// Derived from what the item is *about*, never from its position: regenerating
/// an unchanged package must resolve to the same ids, or every recorded review
/// decision would silently move to another item.
function itemIdentity(kind, target) {
  const digest = createHash("sha256")
    .update(
      JSON.stringify([
        kind,
        target.entrypoint ?? "",
        target.export ?? "",
        target.field ?? ""
      ])
    )
    .digest("hex");
  return `${kind}-${digest.slice(0, 12)}`;
}

function withUniqueIdentities(items) {
  const seen = new Map();
  return items.map(item => {
    const identity = itemIdentity(item.kind, item.target);
    const previous = seen.get(identity) ?? 0;
    seen.set(identity, previous + 1);
    // Two items about the same kind and target are indistinguishable to a
    // reviewer as well as to this file; the suffix keeps them separately
    // resolvable and is deterministic for an unchanged package.
    return { ...item, id: previous === 0 ? identity : `${identity}-${previous + 1}` };
  });
}

function fieldPath(prefix, suffix) {
  return prefix ? `${prefix}.${suffix}` : suffix;
}

export function collectReviewItems(
  entrypoints,
  selected = new Map(),
  refusedEntrypoints = [],
  legacyProvenance = undefined,
  artifactNotes = []
) {
  const items = [];
  const push = (kind, target, text, group, label) => {
    items.push({ kind, target, text, ...(group ? { group, label } : {}) });
  };

  // A partial contract must never be silent about what it omits: a refused
  // entrypoint is the difference between "this package has no such export"
  // and "we could not certify it", and only the second is true here.
  //
  // `contract verify` passes its own refusals through here for the same reason
  // (`rewriteReviewPlan`), so `refusal.reason` may describe an entrypoint
  // generation emitted and verification could not observe.
  for (const refusal of refusedEntrypoints) {
    push(
      "refused-entrypoint",
      { entrypoint: refusal.entrypoint },
      `${refusal.entrypoint}: ${refusal.reason}`
    );
  }
  if (legacyProvenance) {
    push("legacy-root-field", { entrypoint: "." }, legacyProvenance);
  }
  // A contract that carries no artifact hash is bound to the package it
  // describes by a version string alone, and a version string is not a pin:
  // republished or locally patched bytes keep the version. When schema v1
  // cannot express the binding for this output -- or can express only part of
  // it, because the hash covers the entry artifact and not the rest of its
  // runtime-module closure -- the reviewer is the only remaining check, so say
  // so here rather than leaving the gap silent.
  for (const note of artifactNotes) {
    push("artifact-binding", { field: "artifacts.implementation" }, note);
  }
  for (const entrypoint of [...selected.keys()].filter(name => !entrypoints[name])) {
    push(
      "no-export-summary",
      { entrypoint },
      `${entrypoint}: no generated export summary`
    );
  }

  const visit = (summary, location, entrypoint, exportName, prefix) => {
    const target = suffix => ({
      entrypoint,
      export: exportName,
      ...(suffix === undefined ? {} : { field: suffix })
    });
    for (const claim of SENTINEL_CLAIMS.filter(name => isUnknownClaim(summary[name]))) {
      push(
        "unknown-sentinel",
        target(fieldPath(prefix, claim)),
        `${location}: ${claim}`,
        location,
        claim
      );
    }
    // Omitting `callbacks` is a *negative* claim -- "this export never invokes
    // a caller-supplied callback" -- and review is the only thing standing
    // between a callback path the analyzer failed to observe and that claim
    // being promoted to reviewed. An explicit unknown marker is uncertainty
    // the analyzer *did* recognize and is already listed above, so it is not
    // repeated here.
    if (
      summary.kind === "function" &&
      !isUnknownClaim(summary.callbacks) &&
      knownRows(summary.callbacks).length === 0
    ) {
      push(
        "no-callback-row",
        target(fieldPath(prefix, "callbacks")),
        `${location}: no callback execution row`
      );
    }
    if (summary.evidence?.kind === "inherited-from") {
      push(
        "inherited-claim",
        target(prefix || undefined),
        `${location}: ${summary.evidence.package}@${summary.evidence.version}`
      );
    }
    for (const [index, read] of knownRows(summary.reactiveReads).entries()) {
      if (read.evidence?.kind === "inherited-from") {
        push(
          "inherited-claim",
          target(fieldPath(prefix, `reactiveReads[${index}]`)),
          `${location}.reactiveReads[${index}]: ${read.evidence.package}@${read.evidence.version}`
        );
      }
    }
    for (const [index, callback] of knownRows(summary.callbacks).entries()) {
      if (callback.owner == null) {
        push(
          "callback-without-owner-row",
          target(fieldPath(prefix, `callbacks[${index}].owner`)),
          `${location}.callbacks[${index}]: owner behavior requires review`
        );
      }
      if (callback.evidence?.kind === "inherited-from") {
        push(
          "inherited-claim",
          target(fieldPath(prefix, `callbacks[${index}]`)),
          `${location}.callbacks[${index}]: ${callback.evidence.package}@${callback.evidence.version}`
        );
      }
    }
    for (const [index, requirement] of knownRows(summary.ownerRequirements).entries()) {
      push(
        "generated-owner-requirement",
        target(fieldPath(prefix, `ownerRequirements[${index}]`)),
        `${location}.ownerRequirements[${index}]: ${requirement.operation} requires exact caller-owner review`
      );
    }
    const visitReturn = (returned, returnLocation, returnField) => {
      if (!returned) return;
      if (returned.evidence?.kind === "inherited-from") {
        push(
          "inherited-claim",
          target(returnField),
          `${returnLocation}: ${returned.evidence.package}@${returned.evidence.version}`
        );
      }
      for (const [index, element] of (returned.elements ?? []).entries()) {
        visitReturn(
          element,
          `${returnLocation}.elements[${index}]`,
          `${returnField}.elements[${index}]`
        );
      }
      for (const [name, property] of Object.entries(returned.properties ?? {})) {
        visitReturn(
          property,
          `${returnLocation}.properties.${name}`,
          `${returnField}.properties.${name}`
        );
      }
    };
    if (!isUnknownClaim(summary.returns)) {
      visitReturn(summary.returns, `${location}.returns`, fieldPath(prefix, "returns"));
    }
    for (const [index, variant] of (summary.variants ?? []).entries()) {
      visit(
        variant.summary,
        `${location}.variants[${index}].summary`,
        entrypoint,
        exportName,
        fieldPath(prefix, `variants[${index}].summary`)
      );
    }
  };

  for (const [entrypoint, entry] of Object.entries(entrypoints)) {
    if (entry.conditions?.length) {
      push(
        "conditional-environment",
        { entrypoint },
        `${entrypoint}: ${entry.conditions.join(", ")}`
      );
    }
    for (const [name, summary] of Object.entries(entry.exports)) {
      visit(summary, `${entrypoint}:${name}`, entrypoint, name, "");
    }
  }

  // Every export the contract certifies, asked about by name.
  //
  // The sections above are exception-driven: they raise a question where the
  // generator left something undecided. That leaves the ordinary case
  // unreviewed by construction -- a package of plain values produced a plan
  // with zero items and promoted to `reviewed` on zero decisions, and an export
  // whose generated `reactiveReads`/`returns`/callback rows were all positive
  // was promoted without one item naming it. The rows the generator *did* write
  // are claims too, and so is every domain it omitted, which is a certified
  // negative. This item is where a reviewer says yes to both, so promotion can
  // never rest on nothing.
  for (const [entrypoint, entry] of Object.entries(entrypoints)) {
    for (const [name, summary] of Object.entries(entry.exports)) {
      const claimed = [];
      const rows = (label, value) => {
        const known = knownRows(value);
        if (known.length) claimed.push(`${label} (${known.length})`);
      };
      rows("reactiveReads", summary.reactiveReads);
      if (summary.returns && !isUnknownClaim(summary.returns)) claimed.push("returns");
      rows("callbacks", summary.callbacks);
      rows("ownerRequirements", summary.ownerRequirements);
      if (summary.asyncBehavior && !isUnknownClaim(summary.asyncBehavior)) {
        claimed.push("asyncBehavior");
      }
      if (summary.variants?.length) claimed.push(`variants (${summary.variants.length})`);
      const alreadyAsked = items.some(
        item => item.target.entrypoint === entrypoint && item.target.export === name
      );
      if (!claimed.length && alreadyAsked) continue;
      const omitted = SENTINEL_CLAIMS.filter(claim => summary[claim] === undefined);
      push(
        "generated-summary",
        { entrypoint, export: name },
        `${entrypoint}:${name} is certified as a ${summary.kind} claiming ` +
          `${claimed.length ? claimed.join(", ") : "no positive row"}; the omitted domain(s) ` +
          `${omitted.length ? omitted.join(", ") : "none"} are certified negative claims`
      );
    }
  }
  return withUniqueIdentities(items);
}

/// The checklist's sections, in order, from the machine items.
///
/// Grouped items (an export with several unknown claim domains) render as the
/// one line they always have; each domain stays a separately resolvable item.
function reviewPlanSections(items) {
  return SECTIONS.map(([title, kind]) => {
    const rows = [];
    const grouped = new Map();
    for (const item of items) {
      if (item.kind !== kind) continue;
      if (!item.group) {
        rows.push(item.text);
        continue;
      }
      const existing = grouped.get(item.group);
      if (existing) {
        existing.labels.push(item.label);
        continue;
      }
      const row = { group: item.group, labels: [item.label] };
      grouped.set(item.group, row);
      rows.push(row);
    }
    return [
      title,
      rows.map(row =>
        typeof row === "string" ? row : `${row.group}: ${row.labels.join(", ")}`
      )
    ];
  });
}

export function renderReviewPlan(packageName, packageVersion, output, items) {
  const sections = reviewPlanSections(items).map(([title, rows]) => {
    const body = rows.length
      ? rows.map(row => `- [ ] ${row}`).join("\n")
      : "- [x] none observed by the generator";
    return `## ${title}\n\n${body}`;
  });
  return {
    count: items.length,
    text: [
      "# Package contract review plan",
      "",
      `Package: ${packageName}@${packageVersion}`,
      `Contract: ${output}`,
      "",
      ...sections,
      "",
      "Generated evidence is inferred. Check every item against the exact package release before promoting the contract to verified, reviewed, or trusted.",
      ""
    ].join("\n")
  };
}

/// The machine-readable plan, bound to the contract bytes it describes.
///
/// `contract` is the sha256 of the document generation had just written. A plan
/// is a set of questions about one exact contract: its item ids are derived
/// from that document's entrypoints and exports, and its `generation` block
/// names the bytes those summaries came from. Beside a different contract it is
/// a set of questions nobody asked about that one -- and since `contract
/// review` validated the pairing on package *name* alone, copying v1's plan
/// next to v2's contract was enough to resolve v2 by answering v1's questions.
export function renderReviewPlanDocument(
  packageName,
  packageVersion,
  items,
  generation,
  contractHash
) {
  return {
    schemaVersion: REVIEW_PLAN_SCHEMA_VERSION,
    package: { name: packageName, version: packageVersion },
    ...(contractHash ? { contract: contractHash } : {}),
    // `because` is this sidecar's own field, not the contract's: schema v1's
    // `unknownClaim` is `additionalProperties: false`, so an emitter reason
    // recorded there would hard-fail every loader that predates it. Here it
    // costs nothing and is what turns "unknown" into a reviewable claim --
    // which obligation forced it, where, and how emission decided the claim
    // belonged to this export. Absent when the emitter said nothing.
    items: items.map(({ id, kind, target, text, because }) => ({
      id,
      kind,
      target,
      text,
      ...(because ? { because } : {})
    })),
    generation
  };
}

function siblingPath(output, suffix) {
  return output.toLowerCase().endsWith(".json")
    ? `${output.slice(0, -5)}${suffix}`
    : `${output}${suffix}`;
}

/// Where a regeneration keeps the contract a review was recorded against.
///
/// `.previous.json` rather than `.previous`, so that the sibling helpers above
/// derive `<name>.previous.review.json` and `<name>.previous.review-state.json`
/// from it exactly as they do for a live contract -- `--transfer-from` then
/// takes the snapshot path with no special case at all.
export function previousContractPath(output) {
  return siblingPath(output, ".previous.json");
}

export function reviewPlanPath(output) {
  return siblingPath(output, ".review.md");
}

export function reviewPlanJsonPath(output) {
  return siblingPath(output, ".review.json");
}

export function reviewStatePath(output) {
  return siblingPath(output, ".review-state.json");
}

/// The Stage-1 probe audit trail, and the Stage-2 machine-verification record.
///
/// Both are derived the same way as the review siblings so that a regeneration's
/// `.previous` move carries them with the bytes they describe, and so that
/// `<name>.previous.probe.json` and `<name>.previous.verify.json` need no
/// special case anywhere.
export function probeReportPath(output) {
  return siblingPath(output, ".probe.json");
}

export function verifyReportPath(output) {
  return siblingPath(output, ".verify.json");
}
