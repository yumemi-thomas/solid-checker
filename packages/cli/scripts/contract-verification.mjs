// Everything `solid-checker contract verify` decides: the blockers, the
// unknown-conversion rule, and the shape of the `<contract>.verify.json`
// sidecar. No I/O, so every judgement is reachable from a unit test with a
// hand-written probe report.
//
// RFC 0002 §2 and §3 are the specification. The rule the whole file exists to
// enforce is one sentence:
//
//   > A machine may certify exactly what it proved or observed. Every other
//   > positive claim it holds must become the unknown sentinel before
//   > promotion. Never a guess, never a downgrade that hides.
//
// "Proved" is family (A) -- the negatives-by-omission, `ownerRequirements`,
// `reactiveReads`, and the `variants` structure, all of which the generator
// derives from exact compiler facts and already fails closed on by emitting the
// sentinel where it cannot. "Observed" is a `probed` row marker that
// `contract probe --write` put there. Everything else is converted, per
// *domain*, because the sentinel is a field value and schema v1 has no
// per-row spelling for "not proven".
//
// The conversion is deliberately lossy, and the loss is the point: it is
// recorded in the sidecar with the claim identity, the value the machine held,
// and the reason the probe could not reach it, so review tooling can say "the
// machine believed `callbacks[0]=inline` here and could not confirm it". The
// contract itself cannot carry that -- `$defs.unknownClaim` permits the single
// property `status`, and `isUnknownClaim` tests for exactly one key -- which is
// RFC 0002 unresolved question 5.

import {
  PROBE_MODES,
  conditionsMatchMode,
  modeApplies,
  returnClaim,
  summaryForMode
} from "./contract-probe-driver.mjs";
import { isUnknownClaim } from "./contract-review-plan.mjs";

export const VERIFY_REPORT_SCHEMA_VERSION = 1;

/// The blockers of RFC 0002 §3, named so a report can list what was checked
/// even when nothing was raised.
export const BLOCKERS = [
  "probe-report-present",
  "probe-report-binds-contract",
  "probe-report-includes-evidence-write",
  "probe-report-includes-discovery",
  "probe-failed",
  "incompleteness",
  "kind-observed",
  // Not in RFC 0002 §3's list: it is the floor under amendment A9's
  // per-entrypoint `kind` refusal. A promoted document has to certify
  // *something*, and "no entrypoint certifies anything" is the one shape the
  // finer refusal can produce that the coarse one could not.
  "certifies-nothing",
  "closure-note",
  // The other half of RFC 0002 §2 condition 4, separated by attestation: the
  // record is established and what the runtime loads is not. Its own kind
  // rather than a second spelling of `closure-note`, because merging the two
  // would make attestation's effect on the corpus unmeasurable -- and its own
  // rule in `blockerClass` (scripts/ecosystem-benchmark/verify-corpus.mjs), or
  // every row whose only blocker is this one lands in `unclassified-refusal`,
  // which is the one number amendment A9's stage 2 gate reads.
  "attested-closure-note",
  "review-under-way",
  "document-validates"
];

function rows(value) {
  return Array.isArray(value) ? value : [];
}

/// Why a domain converted, when the reason is structural rather than something
/// a probe run reported. Each names the missing mechanism, not the claim.
const CONVERSION_REASON = {
  unprobed: "no probed row evidence: the claim was not observed against the installed release",
  modes: statedModes =>
    `probed row evidence does not cover every mode the claim is stated for (${statedModes.join(", ")}); ` +
    "narrowing the stated modes would claim semantics for an environment nobody observed",
  owner:
    "callback owner rows have no probe form: no observation distinguishes inherited from created ownership",
  callbackArguments:
    "callback argument descriptors have no probe form: the claim is about the shape passed to the callback",
  inherited: evidence =>
    `the row's claim is inherited from ${evidence.package}@${evidence.version} and was not observed here; ` +
    "the tier of the contract it came from is not checkable at this point",
  nestedReturn:
    "return leaves have no probe form: probed evidence does not descend into elements or properties",
  asyncBehavior:
    "asyncBehavior has no probe claim string and no evidence slot in schema v1, so a driven observation could not be recorded",
  // The rule this project got wrong for two rounds: a `probed` marker is a
  // *durable* artifact of some earlier run, and verification consumed it as if
  // it were this run's observation. A probe that observed nothing at all --
  // because the package now refuses to import, because `--modes` narrowed the
  // run, because the export moved -- left every marker the previous healthy run
  // wrote exactly where it was, and the promotion certified all of them.
  //
  // The remedy is conversion rather than a blocker, deliberately. RFC 0002 §2's
  // rule is "every other positive claim it holds must become the unknown
  // sentinel before promotion", and from this run's point of view an
  // unwitnessed marker and an absent one are the same epistemic state: nothing
  // in the consumed report observed the claim. Blocking would also make the
  // honest narrow run unrecoverable -- a `--modes client` report could never
  // verify anything, rather than verifying less -- and "verify less" is the
  // direction this design is built to take.
  staleProbe: (claim, evidence) =>
    `the row carries a probed marker for ${claim} in ${(evidence.modes ?? []).join(", ")} that the ` +
    "consumed probe report does not witness: this run recorded no passing observation of that claim " +
    "covering those modes, and a marker an earlier run wrote is not an observation of the artifact " +
    "this verification is about"
};

/// The modes a summary's claims are stated for.
///
/// An entrypoint with no `conditions` states its claims for every environment,
/// so a row probed only under `browser` does not cover it. This is RFC 0002
/// unresolved question 2's rule -- "undrivable in mode X must convert the
/// claim, not silently narrow the stated modes" -- read off the document.
export function statedModes(entry, variantConditions) {
  return PROBE_MODES.filter(
    mode =>
      modeApplies(entry, mode) &&
      (variantConditions ? conditionsMatchMode(variantConditions, mode) : true)
  ).map(mode => mode.name);
}

function probedCovers(evidence, modes) {
  if (evidence?.kind !== "probed") return false;
  const observed = new Set(evidence.modes ?? []);
  return modes.every(mode => observed.has(mode));
}

/// Why this row is not certifiable as observed, or "" when it is.
///
/// `witness` answers whether the *consumed report* records a passing
/// observation of this exact claim covering at least the modes the marker
/// asserts. Without it the marker in the document was self-certifying: it said
/// "observed" and nothing checked which run observed it.
function callbackRowReason(callback, modes, witness) {
  if (callback.evidence?.kind === "inherited-from") {
    return CONVERSION_REASON.inherited(callback.evidence);
  }
  if (callback.owner != null) return CONVERSION_REASON.owner;
  if (Array.isArray(callback.arguments) && callback.arguments.length) {
    return CONVERSION_REASON.callbackArguments;
  }
  if (callback.evidence?.kind !== "probed") return CONVERSION_REASON.unprobed;
  if (!probedCovers(callback.evidence, modes)) return CONVERSION_REASON.modes(modes);
  const claim = `callbacks[${callback.parameter}]=${callback.execution}`;
  if (!witness(claim, callback.evidence)) {
    return CONVERSION_REASON.staleProbe(claim, callback.evidence);
  }
  return "";
}

function hasReturnLeaf(returned) {
  return Boolean(returned.elements?.length) || Boolean(Object.keys(returned.properties ?? {}).length);
}

function returnReason(returned, modes, witness) {
  if (returned.evidence?.kind === "inherited-from") {
    return CONVERSION_REASON.inherited(returned.evidence);
  }
  if (hasReturnLeaf(returned)) return CONVERSION_REASON.nestedReturn;
  if (returned.evidence?.kind !== "probed") return CONVERSION_REASON.unprobed;
  if (!probedCovers(returned.evidence, modes)) return CONVERSION_REASON.modes(modes);
  const claim = returnClaim(returned);
  if (!witness(claim, returned.evidence)) {
    return CONVERSION_REASON.staleProbe(claim, returned.evidence);
  }
  return "";
}

/// A domain whose rows carry an inherited claim is not a domain this machine
/// observed, so it converts along with the rest.
function inheritedRowReason(value) {
  const inherited = rows(value).find(row => row.evidence?.kind === "inherited-from");
  return inherited ? CONVERSION_REASON.inherited(inherited.evidence) : "";
}

function fieldPath(prefix, suffix) {
  return prefix ? `${prefix}.${suffix}` : suffix;
}

/// Index of the probe report's claim records, so a conversion can quote the
/// exact reason the run gave rather than a generic one -- and so the
/// corroboration below can ask the report whether it witnessed a marker.
///
/// The key is the JSON encoding of the triple rather than a joined string:
/// an entrypoint or export name containing the separator would otherwise
/// silently answer for another claim.
function reasonIndex(report) {
  const index = new Map();
  for (const claim of report?.claims ?? []) {
    index.set(JSON.stringify([claim.entrypoint, claim.export, claim.claim]), claim);
  }
  return index;
}

/// The probe report's record for one claim identity, or `undefined`.
function claimRecord(index, entrypoint, exportName, claim) {
  return index.get(JSON.stringify([entrypoint, exportName, claim]));
}

/// The claim identities a converted domain destroys, with the reason each one
/// could not be confirmed.
///
/// The probe report's own reason wins where it has one: "the synthesized call
/// threw: TypeError: fn is not a function" says far more than "not probed".
function claimRecords(index, entrypoint, exportName, claims, fallback) {
  return claims.map(claim => {
    const observed = claimRecord(index, entrypoint, exportName, claim);
    return {
      claim,
      reason:
        observed?.status === "undriven" && observed.reason ? observed.reason : fallback
    };
  });
}

/// Drops one summary's `probed` marker when the claims it covered are gone.
///
/// `writeProbeEvidence` computes the summary-level marker from exactly two
/// domains -- the `callbacks[]` rows and the top-level `returns` -- so the
/// marker means "every probeable claim this summary states was observed". Two
/// later mutations can empty that set without touching the marker: verification
/// converts a domain to the unknown sentinel, and a review promotion deletes a
/// sentinel it certified absent. Either leaves `evidence: {kind: "probed"}`
/// asserting an observation of claims the document no longer contains, and a
/// row with no evidence of its own then inherits it.
///
/// The surviving rule is strict on purpose: the marker stays only when the
/// summary still states at least one probeable claim and *every* one of them
/// carries a `probed` marker of its own. Anything weaker would let one surviving
/// row keep a marker that was written for several.
///
/// Mutates `summary` and returns whether it dropped the marker.
export function pruneSummaryProbedMarker(summary) {
  if (summary?.evidence?.kind !== "probed") return false;
  const covered = [
    ...rows(summary.callbacks),
    ...(summary.returns && !isUnknownClaim(summary.returns) ? [summary.returns] : [])
  ];
  if (covered.length && covered.every(row => row.evidence?.kind === "probed")) return false;
  delete summary.evidence;
  return true;
}

/// `pruneSummaryProbedMarker` over every summary and variant summary of a
/// contract's entrypoints, returning how many markers it dropped.
export function pruneSummaryProbedMarkers(entrypoints) {
  let dropped = 0;
  const visit = summary => {
    if (!summary || typeof summary !== "object") return;
    if (pruneSummaryProbedMarker(summary)) dropped += 1;
    for (const variant of summary.variants ?? []) visit(variant.summary);
  };
  for (const entry of Object.values(entrypoints ?? {})) {
    for (const summary of Object.values(entry.exports ?? {})) visit(summary);
  }
  return dropped;
}

/// Applies the unknown-conversion rule to one expanded contract.
///
/// Returns a new contract, the conversions performed, and the probed rows that
/// survived. Nothing is deleted except a claim that became the sentinel: no
/// entrypoint and no export ever leaves the document, exactly as
/// `--promote reviewed` guarantees.
export function convertUnconfirmedClaims(contract, report) {
  const index = reasonIndex(report);
  const conversions = [];
  const probed = [];
  const staleMarkers = [];

  const convertSummary = (
    summary,
    entrypoint,
    exportName,
    entry,
    variantConditions,
    prefix,
    inheritedFrom
  ) => {
    const modes = statedModes(entry, variantConditions);
    const next = structuredClone(summary);
    /// Whether the consumed report witnesses this marker.
    ///
    /// A `probed` marker in the document is durable and says nothing about
    /// *which* run wrote it. This asks the report the promotion is actually
    /// consuming: does it record a passing observation of this claim, and does
    /// that observation cover at least the modes the marker asserts?
    const witness = (claim, evidence) => {
      const observed = claimRecord(index, entrypoint, exportName, claim);
      if (observed?.status !== "passed") return false;
      const passed = new Set(observed.modes?.passed ?? []);
      return (evidence.modes ?? []).every(mode => passed.has(mode));
    };
    const noteStale = (field, claim, evidence) => {
      staleMarkers.push({
        entrypoint,
        export: exportName,
        field: fieldPath(prefix, field),
        claim,
        marker: structuredClone(evidence)
      });
    };
    const convert = (field, claims, reason) => {
      conversions.push({
        entrypoint,
        export: exportName,
        field: fieldPath(prefix, field),
        modes,
        claimed: structuredClone(summary[field]),
        claims: claimRecords(index, entrypoint, exportName, claims, reason)
      });
      next[field] = { status: "unknown" };
    };

    // `kind` is not converted here, and that is not an exemption: schema v1 has
    // no sentinel for it, so there is nothing honest to convert it *to*. It is
    // handled entirely by `unobservedKindRefusals` below -- a kind claim this
    // run did not observe in every stated mode refuses its entrypoint, and
    // refuses the document when no entrypoint would certify anything. Relying
    // on "a
    // disagreeing kind is a failed probe" was the hole: a probe that observed
    // *nothing* disagrees with nothing.

    if (Array.isArray(summary.callbacks) && summary.callbacks.length) {
      const reasons = summary.callbacks
        .map(callback => [callback, callbackRowReason(callback, modes, witness)])
        .filter(([, reason]) => reason);
      for (const [callback] of reasons) {
        if (callback.evidence?.kind !== "probed") continue;
        const claim = `callbacks[${callback.parameter}]=${callback.execution}`;
        if (!witness(claim, callback.evidence)) {
          noteStale(`callbacks[${callback.parameter}]`, claim, callback.evidence);
        }
      }
      if (reasons.length) {
        // Per domain, not per row: the sentinel is a field value, so one
        // unconfirmable row converts the export's whole `callbacks` field.
        // Unknown is contagious by construction here exactly as it is in the
        // generator's own `mergeClaimRows`.
        convert(
          "callbacks",
          summary.callbacks.map(callback => `callbacks[${callback.parameter}]=${callback.execution}`),
          reasons[0][1]
        );
      } else {
        for (const callback of summary.callbacks) {
          probed.push({
            entrypoint,
            export: exportName,
            field: fieldPath(prefix, `callbacks[${callback.parameter}]`),
            claim: `callbacks[${callback.parameter}]=${callback.execution}`,
            modes: callback.evidence.modes,
            calls: callback.evidence.calls
          });
        }
      }
    }

    if (summary.returns && !isUnknownClaim(summary.returns)) {
      const reason = returnReason(summary.returns, modes, witness);
      if (
        summary.returns.evidence?.kind === "probed" &&
        !witness(returnClaim(summary.returns), summary.returns.evidence)
      ) {
        noteStale("returns", returnClaim(summary.returns), summary.returns.evidence);
      }
      if (reason) convert("returns", [returnClaim(summary.returns)], reason);
      else {
        probed.push({
          entrypoint,
          export: exportName,
          field: fieldPath(prefix, "returns"),
          claim: returnClaim(summary.returns),
          modes: summary.returns.evidence.modes,
          calls: summary.returns.evidence.calls
        });
      }
    }

    // Family (C) with no probe form at all and no evidence slot to record one
    // in, so it converts unconditionally wherever it is stated.
    if (summary.asyncBehavior && !isUnknownClaim(summary.asyncBehavior)) {
      convert("asyncBehavior", ["asyncBehavior"], CONVERSION_REASON.asyncBehavior);
    }

    // Family (A): rows the compiler facts make exact. The generator emits the
    // sentinel where they are not, so an emitted row *is* the proven case --
    // unless it was inherited from another package's contract, which this
    // machine neither proved nor observed.
    for (const field of ["reactiveReads", "ownerRequirements"]) {
      if (!Array.isArray(summary[field]) || !summary[field].length) continue;
      const reason = inheritedRowReason(summary[field]);
      if (reason) {
        convert(
          field,
          summary[field].map((_, position) => `${field}[${position}]`),
          reason
        );
      }
    }

    // The whole summary is another package's claim, so every domain it carries
    // converts -- including the domains its *variants* carry.
    //
    // The recursion used to drop the inheritance on the way down: the five
    // top-level domains converted and then `variants` was walked on its own
    // evidence, so a variant whose rows carried no marker of their own passed
    // straight through and the export ended up certifying, per environment,
    // exactly the claims inheritance says this machine never observed. The flag
    // travels with the walk now, which is what the comment always said the rule
    // was: every domain the summary carries.
    const inherited =
      summary.evidence?.kind === "inherited-from" ? summary.evidence : inheritedFrom;
    if (inherited) {
      for (const field of ["callbacks", "returns", "reactiveReads", "ownerRequirements", "asyncBehavior"]) {
        if (next[field] === undefined || isUnknownClaim(next[field])) continue;
        convert(field, [`${field} (inherited summary)`], CONVERSION_REASON.inherited(inherited));
      }
    }

    if (summary.variants?.length) {
      next.variants = summary.variants.map((variant, position) => ({
        ...variant,
        summary: convertSummary(
          variant.summary,
          entrypoint,
          exportName,
          entry,
          variant.conditions,
          fieldPath(prefix, `variants[${position}].summary`),
          inherited
        )
      }));
    }
    // A summary-level `probed` marker asserts an observation of the claims the
    // summary states. Once those claims are gone -- converted here, or deleted
    // by a review that certified them absent -- the marker asserts an
    // observation of nothing, and `claims_are_certifiable` has no way to notice.
    if (pruneSummaryProbedMarker(next)) {
      staleMarkers.push({
        entrypoint,
        export: exportName,
        field: fieldPath(prefix, "evidence") || "evidence",
        claim: "summary",
        marker: structuredClone(summary.evidence)
      });
    }
    return next;
  };

  const entrypoints = Object.fromEntries(
    Object.entries(contract.entrypoints).map(([entrypoint, entry]) => [
      entrypoint,
      {
        ...entry,
        exports: Object.fromEntries(
          Object.entries(entry.exports).map(([name, summary]) => [
            name,
            convertSummary(summary, entrypoint, name, entry, undefined, "", undefined)
          ])
        )
      }
    ])
  );
  return { contract: { ...contract, entrypoints }, conversions, probed, staleMarkers };
}

/// Drops `inferred` row markers, exactly as `--promote reviewed` does.
///
/// `claims_are_certifiable` rejects any inferred row inside an otherwise
/// certifying document, and a row with no evidence of its own inherits the
/// document's -- so removing the marker is the honest operation, where writing
/// `verified` onto each row would claim a per-row assertion no check made.
/// `probed` and `inherited-from` markers are untouched; rows whose claim
/// converted vanished with the field before this runs.
export function dropInferredRowEvidence(value) {
  if (Array.isArray(value)) {
    let dropped = 0;
    for (const element of value) dropped += dropInferredRowEvidence(element);
    return dropped;
  }
  if (!value || typeof value !== "object") return 0;
  let dropped = 0;
  if (value.evidence?.kind === "inferred") {
    delete value.evidence;
    dropped += 1;
  }
  for (const child of Object.values(value)) dropped += dropInferredRowEvidence(child);
  return dropped;
}

/// Why one entrypoint's `kind` claims are not certifiable, as one line.
///
/// The entrypoint name leads, because the line is an attribution: the sidecar
/// records it against that entrypoint, and the document-level blocker below
/// quotes it verbatim.
function kindRefusalDetail(entrypoint, unobserved) {
  const named = unobserved.map(item => `${item.export} (${item.modes.join(", ")})`);
  const shown = named.slice(0, 5).join(", ");
  const rest = named.length > 5 ? `, and ${named.length - 5} more` : "";
  return (
    `${entrypoint}: the probe report records no passing kind observation for ${unobserved.length} ` +
    `export(s) in every mode they are stated for: ${shown}${rest}`
  );
}

/// The entrypoints whose `kind` claims this run did not observe.
///
/// `kind` is the one family-(B) claim with no sentinel to convert to. Schema v1
/// requires it on every export summary and its two values are the whole
/// vocabulary, so "not proven" is unsayable: there is no honest weaker document
/// to promote. That left it exempt from the conversion rule and therefore
/// exempt from every check -- an import that threw, an export that vanished, a
/// session that crashed, a `--modes` narrowing, all produced zero observations
/// and a contract that verified anyway. A package whose entrypoint refuses to
/// load promoted with none of its claims observed at all.
///
/// So the rule is the other one available: a `kind` claim not probed-passed in
/// every mode the export is stated for cannot be certified. The consequence is
/// deliberate and worth stating plainly -- a package this checker cannot import
/// cannot be machine-verified. It can still be reviewed, and `contract review`
/// is where a human's reading of an unimportable package belongs.
///
/// **The unit of that refusal is the entrypoint, not the document.** This
/// function used to return blocker lines, and one of them refused everything:
/// a package whose `./server` subpath would not load lost the twenty
/// entrypoints the probe *did* observe. Generation has never worked that way --
/// an entrypoint it cannot certify is refused and omitted while the rest are
/// emitted (docs/package-contracts.md "Refused entrypoints versus failed
/// generation") -- and a refused entrypoint is already an explicit
/// uncertifiable result at the consumer rather than a wrong claim, because
/// `exports_for_module` finds no summary for a name the document does not
/// carry. Verification refusing an entrypoint for the same reason generation
/// does is a consistency fix, not new semantics; nothing is newly certified,
/// strictly less is. `collectBlockers` still refuses the *document* when no
/// entrypoint would certify anything, because such a contract certifies nothing
/// and the loader rejects it anyway.
///
/// This is a recorded deviation from RFC 0002's taxonomy table, which lists
/// `kind` as plain family (B); see docs/rfcs/0002-machine-verified-contracts.md
/// "Amendments" A1 and A9.
export function unobservedKindRefusals(contract, report) {
  const index = reasonIndex(report);
  const refusals = [];
  for (const [entrypoint, entry] of Object.entries(contract.entrypoints ?? {})) {
    const modes = new Set(statedModes(entry));
    const unobserved = [];
    for (const [name, summary] of Object.entries(entry.exports ?? {})) {
      const missing = [];
      for (const mode of PROBE_MODES.filter(candidate => modes.has(candidate.name))) {
        const selected = summaryForMode(summary, mode);
        if (!selected) {
          missing.push(`${mode.name} (no unambiguous summary resolves there)`);
          continue;
        }
        const observed = claimRecord(index, entrypoint, name, `kind=${selected.kind}`);
        if (
          observed?.status !== "passed" ||
          !(observed.modes?.passed ?? []).includes(mode.name)
        ) {
          missing.push(mode.name);
        }
      }
      if (missing.length) unobserved.push({ export: name, modes: missing });
    }
    if (!unobserved.length) continue;
    refusals.push({
      entrypoint,
      exports: unobserved,
      blocker: kindRefusalDetail(entrypoint, unobserved)
    });
  }
  return refusals;
}

/// The entrypoints a promoted document would actually certify something through.
///
/// Not `entrypoints.length - refusals.length`: an entrypoint whose export map is
/// **empty** certifies nothing either, so counting it as a survivor is how a
/// document that says literally nothing gets past the empty-set blocker below.
/// The loader agrees -- `rust/crates/solid-reactive-ir/src/lib.rs` rejects an
/// entrypoint with an empty `exports`, and
/// `rust/crates/solid-facts-backend/src/contract_document.rs` rejects the
/// neither-`exports`-nor-`sameAs` shape -- so the alternative to failing here is
/// a `--validate-contract` complaint about document shape instead of a sentence
/// about what happened.
export function certifyingEntrypoints(contract, refused = []) {
  const drop = new Set(refused);
  return Object.entries(contract.entrypoints ?? {})
    .filter(([name, entry]) => !drop.has(name) && Object.keys(entry.exports ?? {}).length > 0)
    .map(([name]) => name);
}

/// The contract minus the entrypoints verification refused.
///
/// Dropped from the *expanded* document, so an entrypoint another one pointed at
/// through `sameAs` keeps its materialized exports and the dedup is recomputed
/// over the survivors when the promoted document is normalized.
export function withoutRefusedEntrypoints(contract, refused) {
  const drop = new Set(refused);
  return {
    ...contract,
    entrypoints: Object.fromEntries(
      Object.entries(contract.entrypoints ?? {}).filter(([name]) => !drop.has(name))
    )
  };
}

/// Why a document that would certify nothing is refused whole, one line each.
///
/// **Shape matters here twice.**
///
/// The classifying phrase leads the line. The corpus harness truncates every
/// blocker to a 260-character head *before* classifying it
/// (scripts/ecosystem-benchmark/verify-corpus.mjs `blockerClass`), so a phrase
/// pushed past that by a long entrypoint name silently reclassifies the row as
/// an unclassified refusal -- and this class is the one number amendment A9's
/// stage 2 gate reads. The enumeration therefore comes last, where growing it
/// costs nothing.
///
/// And every refusal keeps a **named line of its own** beside the summary. One
/// line naming five entrypoints and "and N more" was a real loss of evidence:
/// the refusal sidecar's `blockers.raised` is the only durable record of a
/// refusal, and the corpus has a row with 91 of them. The summary line says why
/// the *document* was refused; the per-entrypoint lines say which entrypoints
/// were unobservable, and in which modes.
function noCertifyingEntrypointBlockers({ contract, kindRefusals, contractPath }) {
  const entrypoints = Object.entries(contract.entrypoints ?? {});
  const refusedNames = kindRefusals.map(refusal => refusal.entrypoint);
  if (certifyingEntrypoints(contract, refusedNames).length) return [];

  const refusedSet = new Set(refusedNames);
  const empty = entrypoints.filter(
    ([name, entry]) => !refusedSet.has(name) && Object.keys(entry.exports ?? {}).length === 0
  );
  const remedy =
    "`kind` is the one claim schema v1 has no unknown sentinel for, so it cannot be converted " +
    "and an unobserved one would be certified from nothing. Re-run `solid-checker contract " +
    `probe ${contractPath} --write\` against an installed release the probe can import, in ` +
    "every stated mode";

  // No `kind` refusal at all, and still nothing to certify: the contract emits
  // no entrypoint, or every entrypoint it emits carries an empty export map.
  // Not reachable from a generated draft today -- generation omits an
  // entrypoint with no summaries and records a `no-export-summary` plan item --
  // so this is the fail-closed floor rather than a path with a population.
  if (!kindRefusals.length) {
    return [
      "no entrypoint certifies anything: " +
        (entrypoints.length
          ? `all ${entrypoints.length} emitted entrypoint(s) carry an empty export map`
          : "the contract emits no entrypoint at all") +
        ", so the promoted document would certify nothing and the loader would reject it. " +
        "Regenerate the contract against an installed release whose entrypoints resolve to " +
        "exports"
    ];
  }

  const details = kindRefusals.map(refusal => refusal.blocker);
  const shown = details.slice(0, 5).join("; ");
  const rest = details.length > 5 ? `; and ${details.length - 5} more entrypoint(s)` : "";
  return [
    "no passing kind observation for any entrypoint that certifies anything: of " +
      `${entrypoints.length} emitted entrypoint(s), ${kindRefusals.length} refused for an ` +
      "unobserved `kind` claim" +
      (empty.length ? ` and ${empty.length} carrying no export at all` : "") +
      ", so the promoted document would certify nothing. " +
      `${remedy}. The refused entrypoint(s): ${shown}${rest}`,
    // HEAD's shape, kept: one attribution per refused entrypoint, naming its
    // unobserved exports and the modes they were stated for.
    ...details.map(detail => `${detail}. ${remedy}`)
  ];
}

/// Every reason this contract must not be promoted, one line each.
///
/// RFC 0002 §3's list, plus the two identity conditions without which none of
/// the others mean anything: the probe report has to be *this contract's*, and
/// the review plan -- which is where closure notes live -- has to be as well.
///
/// Refused entrypoints, unbindable artifacts, undrivable claims and missing
/// callback `owner` rows are deliberately absent: each is already an explicit
/// uncertifiable result at the consumer rather than a wrong claim. That now
/// includes an entrypoint *this* command refuses for an unobserved `kind`
/// claim -- it is dropped from the promoted document instead of blocking it, and
/// the document is refused only when no entrypoint would certify anything at
/// all (`noCertifyingEntrypointBlockers`).
///
/// What stays document-wide, deliberately: a failed probe and an incompleteness
/// finding, even when both name a claim of an entrypoint this run would refuse
/// anyway. Both mean the package answered a claim differently, which is a
/// generator bug or a package change; scoping them to the entrypoint would let
/// a contradiction be dropped rather than fixed, and RFC 0002 §3 does not allow
/// that. A closure note stays document-wide for the same reason it always
/// did -- fail-closed on a file set the generator declines to claim it
/// enumerated -- so a note on a `kind`-refused entrypoint still refuses the
/// whole document.
export function collectBlockers({
  contract,
  contractHash,
  contractPath,
  report,
  reportPath,
  plan,
  planPath,
  reviewState,
  reviewStatePath: statePath
}) {
  const blockers = [];

  if (!report) {
    blockers.push(
      `no probe report at ${reportPath}: mechanical verification certifies what a probe observed, ` +
        `so run \`solid-checker contract probe ${contractPath} --write\` first`
    );
    return blockers;
  }

  if (report.package?.name !== contract.package?.name ||
      report.package?.version !== contract.package?.version) {
    blockers.push(
      `the probe report at ${reportPath} describes ${report.package?.name}@${report.package?.version} ` +
        `and the contract describes ${contract.package?.name}@${contract.package?.version}`
    );
  }

  const probedBytes = report.contract?.afterWrite ?? report.contract?.hash;
  if (probedBytes !== contractHash) {
    blockers.push(
      `the probe report at ${reportPath} was written for contract bytes ${probedBytes} and ` +
        `${contractPath} hashes to ${contractHash}; re-probe these exact bytes before verifying them`
    );
  } else if (report.contract?.afterWrite === undefined && (report.summary?.passed ?? 0) > 0) {
    // The report matches the bytes and records passing claims, yet no evidence
    // write happened -- so the contract carries no `probed` marker and every
    // one of those claims would convert to unknown. That is a probe run the
    // promotion should have included, not a contract with nothing to certify.
    blockers.push(
      `the probe report at ${reportPath} records ${report.summary.passed} passed claim(s) but no ` +
        `evidence write, so none of them reached the contract; re-run ` +
        `\`solid-checker contract probe ${contractPath} --write\``
    );
  }

  // A certification-grade probe has to include the falsifier for the claims it
  // is about to certify. Discovery -- planting a callback where the contract
  // states none -- is the only automated check in this repository that can
  // contradict a negative claim, and negatives are the whole certified surface
  // a contract's omissions create. A report produced with `--no-discovery` has
  // not run it, so the `incompleteness` blocker below is vacuous rather than
  // satisfied: it lists zero findings because nothing looked.
  //
  // A report that records no discovery state at all is refused on the same
  // ground rather than assumed complete. It predates this field, and "it
  // probably ran" is not an observation.
  if (report.discovery?.enabled !== true) {
    blockers.push(
      `the probe report at ${reportPath} ` +
        (report.discovery === undefined
          ? "records no discovery state, so nothing establishes that the negative-claim falsifier ran"
          : "was produced with discovery disabled (--no-discovery), so the probes that plant a " +
            "callback where the contract states none never ran") +
        `; a verified contract certifies every domain its summaries omit, and discovery is the only ` +
        `automated check that can contradict one. Re-run \`solid-checker contract probe ` +
        `${contractPath} --write\` with discovery enabled`
    );
  }

  for (const claim of (report.claims ?? []).filter(claim => claim.status === "failed")) {
    blockers.push(
      `a probe failed: ${claim.entrypoint}:${claim.export} ${claim.claim}: ${claim.reason}. ` +
        "The package does not behave the way the contract says, and converting the claim to " +
        "unknown would hide a generator bug or a package change"
    );
  }

  for (const finding of report.incompleteness ?? []) {
    blockers.push(
      `an incompleteness finding contradicts a negative claim: ${finding.text}. ` +
        "A negative claim a probe falsified is wrong, not incomplete"
    );
  }

  // An unobserved `kind` claim refuses its *entrypoint* (see
  // `unobservedKindRefusals`). It refuses the document only when that leaves no
  // entrypoint that certifies anything -- see `certifyingEntrypoints` for why
  // "survives" is not the same as "was not refused".
  blockers.push(
    ...noCertifyingEntrypointBlockers({
      contract,
      kindRefusals: unobservedKindRefusals(contract, report),
      contractPath
    })
  );

  if (!plan) {
    blockers.push(
      `no review plan at ${planPath}: its generation block is the only record of whether each ` +
        "entrypoint's runtime-module closure could be enumerated, so there is nothing to check " +
        "the closure-note blocker against"
    );
  } else {
    if (plan.contract !== contractHash) {
      blockers.push(
        `the review plan at ${planPath} was written for contract bytes ${plan.contract} and ` +
          `${contractPath} hashes to ${contractHash}; regenerate the contract, re-probe it, and ` +
          "verify the fresh document"
      );
    }
    for (const name of Object.keys(contract.entrypoints ?? {})) {
      for (const note of plan.generation?.entrypoints?.[name]?.notes ?? []) {
        blockers.push(
          `${name} carries a closure note: ${note}. The summaries were derived from a file set ` +
            "the generator itself declines to claim it enumerated, and no probe covers the " +
            "negative claims that file set determines"
        );
      }
      // The other half of RFC 0002 §2 condition 4, separated by attestation.
      // Here the file set *is* claimed -- it is the analyzing program's own --
      // and what is unclaimed is that the runtime loads nothing else. That is a
      // different sentence and it blocks for a different reason: the negative
      // claims still rest on a file set no probe can be shown to be complete,
      // and no module graph can close it. It stays document-wide for the same
      // reason a closure note does.
      //
      // The sentence is one word from the one above ("an *attested* closure
      // note"), which is exactly why it needs its own classifier rule and its
      // own `BLOCKERS` kind: the corpus harness matches on the leading phrase,
      // and a blocker it cannot name is counted as an unclassified refusal --
      // the one number amendment A9's stage 2 gate reads. See the hazard note
      // on `noCertifyingEntrypointBlockers`, `attested-closure-note` in
      // scripts/ecosystem-benchmark/verify-corpus.mjs, and the test in
      // verify-corpus.test.mjs that holds every kind here to being nameable.
      for (const note of plan.generation?.entrypoints?.[name]?.runtimeNotes ?? []) {
        blockers.push(
          `${name} carries an attested closure note: ${note}. The record names every module the ` +
            "analysis read, and nothing establishes that the runtime loads no other one, so no " +
            "probe covers the negative claims that file set determines"
        );
      }
    }
  }

  const answered = Object.keys(reviewState?.resolutions ?? {}).length;
  if (answered > 0 || reviewState?.promoted) {
    blockers.push(
      `${statePath} already records ${
        reviewState.promoted ? `a promotion to ${reviewState.promoted.evidence} evidence` : `${answered} review decision(s)`
      }; verification moves the bytes those decisions were recorded against. Verify before ` +
        "reviewing, or regenerate the contract and verify the fresh document"
    );
  }

  return blockers;
}

/// The `<contract>.verify.json` sidecar.
///
/// Nothing loads it and nothing certifies from it. It is where the loss the
/// conversion rule imposes is recorded -- the claim the machine held, and the
/// reason it could not confirm it -- because schema v1's sentinel cannot carry
/// a reason and a field that tried would hard-fail every older loader.
export function buildVerifyReport({
  contract,
  contractPath,
  before,
  after,
  report,
  reportPath,
  identities,
  conversions,
  probed,
  staleMarkers,
  droppedMarkers,
  refusedEntrypoints = []
}) {
  return {
    schemaVersion: VERIFY_REPORT_SCHEMA_VERSION,
    package: { name: contract.package?.name, version: contract.package?.version },
    contract: { path: contractPath, before, after },
    evidence: { kind: "verified" },
    probeReport: {
      path: reportPath,
      contract: report.contract?.afterWrite ?? report.contract?.hash,
      driven: report.summary?.driven ?? 0,
      passed: report.summary?.passed ?? 0,
      undriven: report.summary?.undriven ?? 0,
      // Recorded because the promotion depends on it: a report with discovery
      // disabled is refused, so a sidecar that says `enabled: true` is the
      // evidence that the negative-claim falsifier ran for these bytes.
      discovery: report.discovery ?? null,
      // Carried forward, not summarized away. A verified contract whose
      // observations were made against a faked `window` certifies something
      // weaker than one observed in a bare process, and this sidecar is the
      // only artifact that can say so -- schema v1 has no place on the
      // contract for it. `null` on a report that predates the field, because
      // "it probably ran without a shim" is not a record.
      environment: report.environment ?? null,
      // Session accounting rides along for the same reason the probe report
      // keeps it: a promotion built on a mode that needed forty restarts is
      // reproducible only if the cost is written down.
      sessions: report.sessions ?? null
    },
    identities,
    // `checked` is the taxonomy this promotion evaluated; `raised` is empty
    // *because this document was promoted*. A run that refused writes a
    // different sidecar -- see `buildRefusalReport` -- where `raised` carries
    // the lines and there is no evidence block at all.
    blockers: { checked: [...BLOCKERS], raised: [] },
    summary: {
      exports: Object.values(contract.entrypoints ?? {}).reduce(
        (total, entry) => total + Object.keys(entry.exports ?? {}).length,
        0
      ),
      conversions: conversions.length,
      probedRows: probed.length,
      staleProbedMarkers: (staleMarkers ?? []).length,
      droppedInferredMarkers: droppedMarkers,
      // The entrypoints this promotion left out. `exports` above counts the
      // promoted document, so this is the only figure that says the document is
      // smaller than the draft it came from.
      refusedEntrypoints: refusedEntrypoints.length
    },
    conversions: [...conversions].sort(
      (left, right) =>
        left.entrypoint.localeCompare(right.entrypoint) ||
        left.export.localeCompare(right.export) ||
        left.field.localeCompare(right.field)
    ),
    probed: [...probed].sort(
      (left, right) =>
        left.entrypoint.localeCompare(right.entrypoint) ||
        left.export.localeCompare(right.export) ||
        left.field.localeCompare(right.field)
    ),
    // The entrypoints verification refused and omitted, each with the blocker
    // that refused it and the exports whose `kind` was unobserved. They are
    // enumerated rather than counted for the same reason `conversions` is: this
    // sidecar is the only artifact that can say what a promotion left out, and
    // "the package verified" without "minus `./server`" is the misreading the
    // file exists to prevent. A consumer importing one gets an explicit
    // uncertifiable result, because the promoted document carries no summary
    // for it at all.
    refusedEntrypoints: [...refusedEntrypoints].sort((left, right) =>
      left.entrypoint.localeCompare(right.entrypoint)
    ),
    // The markers the document carried that this run's report did not witness,
    // and the summary-level markers whose claims are gone. Recorded rather than
    // silently dropped: "the contract said this was observed and nothing in the
    // consumed report says so" is the finding, not an implementation detail.
    staleProbedMarkers: [...(staleMarkers ?? [])].sort(
      (left, right) =>
        left.entrypoint.localeCompare(right.entrypoint) ||
        left.export.localeCompare(right.export) ||
        left.field.localeCompare(right.field)
    )
  };
}

/// The `<contract>.verify.json` a *refusal* writes.
///
/// Before this, a refusal wrote nothing at all: the sidecar was built on the
/// success path only, so the sole record of why a contract was not promoted
/// was the stderr of the process that refused it. That made the most common
/// outcome the least legible one -- a corpus measurement had to recover the
/// blocker taxonomy by pattern-matching English sentences, and anyone running
/// the command in CI kept a log or kept nothing.
///
/// It is deliberately **not** `buildVerifyReport` with a populated `raised`
/// list. Every field that would imply a promotion is absent rather than zeroed:
/// no `evidence`, no `conversions`, no `probed`, no `staleProbedMarkers`, and a
/// `contract` block with `before` but no `after`, because nothing was written.
/// A reader -- or a tool -- that finds `outcome: "refused"` and looks for a
/// promotion finds nothing to misread. `outcome` is the discriminator; the two
/// shapes are never confused by the presence or absence of a count.
export function buildRefusalReport({
  contract,
  contractPath,
  before,
  report,
  reportPath,
  identities,
  blockers
}) {
  return {
    schemaVersion: VERIFY_REPORT_SCHEMA_VERSION,
    kind: "contract-verify-refusal",
    outcome: "refused",
    refusedAt: new Date().toISOString(),
    package: { name: contract?.package?.name ?? null, version: contract?.package?.version ?? null },
    // `after` is absent, not null: the contract was not written, so there is no
    // second hash and no field that could be read as one.
    contract: { path: contractPath, before },
    probeReport: report
      ? {
          path: reportPath,
          present: true,
          contract: report.contract?.afterWrite ?? report.contract?.hash ?? null,
          driven: report.summary?.driven ?? 0,
          passed: report.summary?.passed ?? 0,
          failed: report.summary?.failed ?? 0,
          undriven: report.summary?.undriven ?? 0,
          incompleteness: report.summary?.incompleteness ?? 0,
          discovery: report.discovery ?? null,
          environment: report.environment ?? null,
          sessions: report.sessions ?? null
        }
      : { path: reportPath, present: false },
    identities,
    // The whole point of the file. `checked` is the same taxonomy the success
    // path lists so the two are comparable; `raised` is every line the refusal
    // printed, in the order it printed them.
    blockers: { checked: [...BLOCKERS], raised: [...blockers] }
  };
}
