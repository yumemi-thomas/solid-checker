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
  "closure-note",
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
  const claim = `returns=${returned.kind}`;
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
    // handled entirely by `unobservedKindBlockers` below -- a kind claim this
    // run did not observe in every stated mode blocks the promotion outright.
    // Relying on "a disagreeing kind is a failed probe" was the hole: a probe
    // that observed *nothing* disagrees with nothing.

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
        !witness(`returns=${summary.returns.kind}`, summary.returns.evidence)
      ) {
        noteStale("returns", `returns=${summary.returns.kind}`, summary.returns.evidence);
      }
      if (reason) convert("returns", [`returns=${summary.returns.kind}`], reason);
      else {
        probed.push({
          entrypoint,
          export: exportName,
          field: fieldPath(prefix, "returns"),
          claim: `returns=${summary.returns.kind}`,
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

/// The `kind` claims this run did not observe, as promotion blockers.
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
/// every mode the export is stated for **blocks**. The consequence is
/// deliberate and worth stating plainly -- a package this checker cannot import
/// cannot be machine-verified. It can still be reviewed, and `contract review`
/// is where a human's reading of an unimportable package belongs.
///
/// This is a recorded deviation from RFC 0002's taxonomy table, which lists
/// `kind` as plain family (B); see docs/rfcs/0002-machine-verified-contracts.md
/// "Amendments".
export function unobservedKindBlockers(contract, report, contractPath) {
  const index = reasonIndex(report);
  const blockers = [];
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
      if (missing.length) unobserved.push(`${name} (${missing.join(", ")})`);
    }
    if (!unobserved.length) continue;
    const shown = unobserved.slice(0, 5).join(", ");
    const rest = unobserved.length > 5 ? `, and ${unobserved.length - 5} more` : "";
    blockers.push(
      `${entrypoint}: the probe report records no passing kind observation for ${unobserved.length} ` +
        `export(s) in every mode they are stated for: ${shown}${rest}. \`kind\` is the one claim ` +
        "schema v1 has no unknown sentinel for, so it cannot be converted and an unobserved one " +
        "would be certified from nothing. Re-run `solid-checker contract probe " +
        `${contractPath} --write` +
        "` against an installed release the probe can import, in every stated mode"
    );
  }
  return blockers;
}

/// Every reason this contract must not be promoted, one line each.
///
/// RFC 0002 §3's list, plus the two identity conditions without which none of
/// the others mean anything: the probe report has to be *this contract's*, and
/// the review plan -- which is where closure notes live -- has to be as well.
///
/// Refused entrypoints, unbindable artifacts, undrivable claims and missing
/// callback `owner` rows are deliberately absent: each is already an explicit
/// uncertifiable result at the consumer rather than a wrong claim.
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

  blockers.push(...unobservedKindBlockers(contract, report, contractPath));

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
  droppedMarkers
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
      discovery: report.discovery ?? null
    },
    identities,
    blockers: { checked: [...BLOCKERS], raised: [] },
    summary: {
      exports: Object.values(contract.entrypoints ?? {}).reduce(
        (total, entry) => total + Object.keys(entry.exports ?? {}).length,
        0
      ),
      conversions: conversions.length,
      probedRows: probed.length,
      staleProbedMarkers: (staleMarkers ?? []).length,
      droppedInferredMarkers: droppedMarkers
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
