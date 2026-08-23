// The generic contract probe driver: everything `solid-checker contract probe`
// decides, with no I/O and no package code in scope.
//
// RFC 0002 §1 is the specification. Its finding about the existing machinery is
// the reason this file exists: `scripts/lib/contract-probe-harness.mjs` is a
// recorder and a report writer, and every probe body in
// `scripts/contract-probes*.mjs` is hand-authored against one export's exact
// signature. Nothing in the repository constructs a call to an *arbitrary*
// export, so the driver is new work rather than an extension.
//
// The split is deliberate: this module turns a contract into a set of probe
// requests and turns raw observations back into pass/fail/undriven, evidence,
// and a report. The worker that actually imports the package and runs the
// bodies (`contract-probe-worker.mjs`) returns raw counters and classifies
// nothing, so every judgement in the command is reachable from a unit test with
// a fake runtime and none of it needs an install.
//
// Two rules are load-bearing and are enforced here rather than in the caller:
//
//   * Drivability is empirical and fail-closed. A claim is `undriven` with a
//     reason whenever the synthesized call did not reach it -- an import that
//     threw, a call that threw, a callback that was never invoked. Undriven is
//     never a failure and never evidence.
//   * A probe confirms; it never writes behavior. Evidence is written only onto
//     claims the contract already states, and an observation the contract does
//     not state is an incompleteness finding, which is a failure.

import { createHash } from "node:crypto";

export const PROBE_REPORT_SCHEMA_VERSION = 1;

/// The four condition modes a claim can be stated for, spelled exactly as
/// `scripts/check-bundled-contracts.mjs` spells them.
export const PROBE_MODES = [
  { name: "client", conditions: ["browser"] },
  { name: "server", conditions: ["node"] },
  { name: "development", conditions: ["browser", "development"] },
  { name: "production", conditions: ["browser", "production"] }
];

/// How many leading parameters a discovery probe plants a callback in when the
/// contract states no row there. A sampling bound, not a proof: RFC 0002 is
/// explicit that incompleteness detection "only sees behavior the probe driver
/// happened to elicit".
export const DISCOVERY_PARAMETERS = [0, 1];

export function sha256Bytes(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

/// Ported from `scripts/check-bundled-contracts.mjs`. Which of the four modes an
/// entrypoint's recorded conditions resolve under.
export function modeApplies(entrypoint, mode) {
  const conditions = new Set(entrypoint?.conditions ?? []);
  const environment = new Set(["browser", "node", "client", "server", "development", "production"]);
  const selected = [...environment].filter(condition => conditions.has(condition));
  if (selected.length === 0) return true;
  if (
    conditions.has("development") &&
    !conditions.has("browser") &&
    !conditions.has("node") &&
    !conditions.has("client") &&
    !conditions.has("server")
  ) {
    return mode.name === "development";
  }
  if (
    conditions.has("production") &&
    !conditions.has("browser") &&
    !conditions.has("node") &&
    !conditions.has("client") &&
    !conditions.has("server")
  ) {
    return mode.name === "production";
  }
  if (mode.name === "server") return conditions.has("server") || conditions.has("node");
  if (mode.name === "client") return conditions.has("client") || conditions.has("browser");
  if (mode.name === "development") {
    return conditions.has("development") || conditions.has("client") || conditions.has("browser");
  }
  if (mode.name === "production") {
    return conditions.has("production") || conditions.has("client") || conditions.has("browser");
  }
  return selected.some(condition => mode.conditions.includes(condition));
}

export function conditionsMatchMode(conditions, mode) {
  const active = new Set([...mode.conditions, "import"]);
  return conditions.every(condition => condition === "default" || active.has(condition));
}

/// The one summary a mode resolves to, or `undefined` when the variant set is
/// ambiguous or covers no branch for this mode.
export function summaryForMode(summary, mode) {
  if (!summary.variants?.length) return summary;
  const matches = summary.variants
    .filter(variant => conditionsMatchMode(variant.conditions, mode))
    .sort((left, right) => right.conditions.length - left.conditions.length);
  if (!matches.length) return undefined;
  const mostSpecific = matches.filter(
    variant => variant.conditions.length === matches[0].conditions.length
  );
  if (mostSpecific.length > 1) {
    const canonical = new Set(mostSpecific.map(variant => JSON.stringify(variant.summary)));
    if (canonical.size > 1) return undefined;
  }
  return mostSpecific[0].summary;
}

function isUnknown(value) {
  return value?.status === "unknown" && Object.keys(value).length === 1;
}

function rows(value) {
  return Array.isArray(value) ? value : [];
}

/// The argument synthesis vocabulary, in full.
///
/// RFC 0002 makes argument synthesis the boundary of "drivable" and records
/// that the only sound source for a non-callback argument is the package's own
/// declarations, which this generator never resolves. So the driver synthesizes
/// only from the contract's *own structured vocabulary* and never from a type:
///
///   probe-callback  the parameter this probe is about
///   noop-callback   a parameter another `callbacks[]` row names
///   empty-object    a parameter a `reactiveReads[]` `parameter-member` row
///                   names, because the export reads a member of it
///   undefined       every other slot
///
/// There is deliberately no ladder of retries. Trying `{}`, then `[]`, then `0`
/// until something completes would make drivability depend on which shape
/// happened to survive, and a call that completes for the wrong reason observes
/// the wrong thing. A slot the vocabulary cannot fill is `undefined`, and if the
/// export refuses it the claim is undriven with the throw as its reason -- which
/// is the measurement RFC 0002's unresolved question 1 asks for.
export const ARGUMENT_SYNTHESIS = ["probe-callback", "noop-callback", "empty-object", "undefined"];

export function synthesizeArguments(summary, probedParameter) {
  const callbackParameters = rows(summary.callbacks)
    .map(callback => callback.parameter)
    .filter(parameter => Number.isInteger(parameter));
  const memberParameters = rows(summary.reactiveReads)
    .filter(read => read.kind === "parameter-member" && Number.isInteger(read.parameter))
    .map(read => read.parameter);
  const named = [...callbackParameters, ...memberParameters];
  if (Number.isInteger(probedParameter)) named.push(probedParameter);
  const arity = named.length ? Math.max(...named) + 1 : 0;
  const descriptors = [];
  for (let index = 0; index < arity; index += 1) {
    if (index === probedParameter) descriptors.push("probe-callback");
    else if (callbackParameters.includes(index)) descriptors.push("noop-callback");
    else if (memberParameters.includes(index)) descriptors.push("empty-object");
    else descriptors.push("undefined");
  }
  return descriptors;
}

/// Where an undriven claim's reason comes from. Each string names the missing
/// mechanism, not the claim, so a report reader can tell a permanent gap
/// (owner rows) from one Stage 2+ could close (a store-path probe form).
const UNDRIVABLE = {
  owner: "callback owner rows have no probe form: no observation distinguishes inherited from created ownership",
  callbackArguments:
    "callback argument descriptors have no probe form: the claim is about the shape passed to the callback, and no claim string names one",
  reactiveRead:
    "reactive reads are proven from compiler facts and have no probe claim string: confirming one at runtime means synthesizing a reactive source and observing the subscription",
  ownerRequirement:
    "owner requirements are proven from the compiler's canonical symbol identity; they are a static claim about the caller's owner and no runtime observation names one",
  asyncBehavior:
    "asyncBehavior has no evidence slot in schema v1, so a driven observation could not be recorded",
  storePath:
    "no generic store-path observation: confirming a store path means writing through the package's own setter, which the contract does not name",
  nestedReturn:
    "writeProbeEvidence does not descend into return leaves and no claim string names one",
  argumentReturn: "an identity claim about a parameter; no probe form exists"
};

/// The family label a report row carries, spelled so the report, this driver,
/// `contract-verification.mjs` and RFC 0002's taxonomy table agree.
///
/// The alignment is load-bearing and used to be wrong: `reactiveReads[]` and
/// `ownerRequirements[]` were reported as family (C) -- "a positive claim the
/// harness cannot drive, converted to the unknown sentinel before promotion" --
/// while verification treated them as family (A) and kept them. One of the two
/// had to move, and the RFC's table says (A): both are derived from exact
/// compiler facts, the generator already emits the sentinel where they are not,
/// and neither has a probe claim string. They are therefore *undrivable and
/// certified anyway*, which is what family (A) means; the reason string above
/// says why no probe covers them rather than implying one should.
///
/// Family (B) claims stay (B) and family (C) claims stay (C).
const UNDRIVABLE_FAMILY = {
  owner: "C",
  callbackArguments: "C",
  reactiveRead: "A",
  ownerRequirement: "A",
  asyncBehavior: "C",
  storePath: "C",
  nestedReturn: "C",
  argumentReturn: "C"
};

/// The opaque identity of one claim.
///
/// JSON-encoded rather than joined on a separator, because an entrypoint or
/// export name containing the separator would otherwise silently answer for
/// another claim -- and because a literal separator byte makes this file
/// undiffable.
function claimKey(entrypoint, exportName, claim) {
  return JSON.stringify([entrypoint, exportName, claim]);
}

/// How many times a probe actually invoked the export under test.
///
/// The worker used to stamp a per-probe-type constant, so a `deferred` claim --
/// whose whole shape is that the call-site memo does *not* re-run -- recorded
/// `calls: 2` for a single invocation, and `evidence.calls` in the contract was
/// a table lookup rather than a measurement. The worker counts now, and a
/// result that carries no count contributes none: an import that failed, a call
/// that threw, or a `typeof` reading invoked nothing.
export function measuredCalls(result) {
  return Number.isInteger(result?.calls) && result.calls >= 0 ? result.calls : 0;
}

/// Every claim of a contract, split into the probe requests the worker runs and
/// the undrivable records that only ever carry a reason.
///
/// `modes` restricts which of `PROBE_MODES` are attempted at all; an
/// entrypoint's own conditions restrict it further through `modeApplies`, and a
/// variant's conditions through `summaryForMode`.
export function buildProbePlan(contract, { modes = PROBE_MODES, discovery = true } = {}) {
  const claims = new Map();
  const requests = new Map();
  // Probe ids are opaque counters and the parent keeps the map back to what
  // they were about. Encoding the target in the id would make an export or
  // entrypoint name containing the separator silently reassign a result.
  const index = new Map();
  let counter = 0;
  const packageName = contract.package?.name ?? "";

  const claimRecord = (entrypoint, exportName, claim, family) => {
    const key = claimKey(entrypoint, exportName, claim);
    const existing = claims.get(key);
    if (existing) return existing;
    const record = {
      entrypoint,
      export: exportName,
      claim,
      family,
      status: family === "C" ? "undriven" : "undriven",
      modesAttempted: [],
      modesPassed: [],
      observations: []
    };
    claims.set(key, record);
    return record;
  };

  const request = (mode, about, probe) => {
    counter += 1;
    const id = `p${counter}`;
    const list = requests.get(mode.name) ?? [];
    list.push({ id, ...probe });
    requests.set(mode.name, list);
    index.set(id, { mode: mode.name, ...about });
  };

  const specifierOf = entrypoint =>
    entrypoint === "." ? packageName : `${packageName}${entrypoint.slice(1)}`;

  for (const [entrypoint, entry] of Object.entries(contract.entrypoints ?? {})) {
    const applicable = modes.filter(mode => modeApplies(entry, mode));
    for (const [exportName, summary] of Object.entries(entry.exports ?? {})) {
      // Undrivable records are per claim, not per mode: the reason a claim has
      // no probe form does not change with the environment. The family comes
      // from `UNDRIVABLE_FAMILY`, so a row the generator proves statically is
      // labelled (A) here and kept at verification, and a row nothing grounds
      // is labelled (C) here and converted there.
      recordUndrivable(summary, (claim, reason, family) => {
        const record = claimRecord(entrypoint, exportName, claim, family);
        record.family = family;
        record.reason = reason;
      });

      for (const mode of applicable) {
        const selected = summaryForMode(summary, mode);
        if (!selected) {
          const record = claimRecord(entrypoint, exportName, "summary", "C");
          record.reason = `no unambiguous summary in ${mode.name}`;
          continue;
        }
        const specifier = specifierOf(entrypoint);
        const kindClaim = `kind=${selected.kind}`;
        claimRecord(entrypoint, exportName, kindClaim, "B").modesAttempted.push(mode.name);
        request(
          mode,
          { entrypoint, export: exportName, claim: kindClaim },
          { type: "kind", entrypoint, specifier, export: exportName }
        );

        if (!isUnknown(selected.callbacks)) {
          for (const callback of rows(selected.callbacks)) {
            const claim = `callbacks[${callback.parameter}]=${callback.execution}`;
            const record = claimRecord(entrypoint, exportName, claim, "B");
            record.modesAttempted.push(mode.name);
            record.arguments = synthesizeArguments(selected, callback.parameter);
            request(
              mode,
              { entrypoint, export: exportName, claim },
              {
                type: "callback",
                entrypoint,
                specifier,
                export: exportName,
                parameter: callback.parameter,
                arguments: record.arguments,
                // A lazily-computed export (a memo) never runs its callback
                // until the accessor it returned is read, and the contract is
                // what says an accessor was returned. Reading it is therefore
                // contract-led, not a guess about the return value.
                callAccessor: selected.returns?.kind === "accessor"
              }
            );
          }
        }

        if (selected.returns && !isUnknown(selected.returns)) {
          const claim = `returns=${selected.returns.kind}`;
          const record = claimRecord(entrypoint, exportName, claim, "B");
          if (selected.returns.kind === "accessor") {
            const plant = rows(selected.callbacks)[0]?.parameter;
            if (!Number.isInteger(plant)) {
              record.family = "C";
              record.reason =
                "no plantable reactive source: proving the returned value is an accessor needs a signal read inside a callback the contract states, and this export states none";
            } else {
              record.modesAttempted.push(mode.name);
              record.arguments = synthesizeArguments(selected, plant);
              request(
                mode,
                { entrypoint, export: exportName, claim },
                {
                  type: "returns-accessor",
                  entrypoint,
                  specifier,
                  export: exportName,
                  parameter: plant,
                  arguments: record.arguments
                }
              );
            }
          } else {
            record.family = "C";
            record.reason =
              selected.returns.kind === "store-path"
                ? UNDRIVABLE.storePath
                : selected.returns.kind === "argument"
                  ? UNDRIVABLE.argumentReturn
                  : UNDRIVABLE.nestedReturn;
          }
        }

        // Discovery is not restricted to `kind: function` summaries.
        //
        // A `value` summary is the maximal negative claim -- it says the export
        // is not callable at all, and therefore that it invokes no
        // caller-supplied callback in any slot, in any mode. The kind probe is
        // the primary falsifier and a disagreement there is already a failed
        // claim, but a discovery probe is what turns "callable after all" into
        // a *named* callback observation rather than a bare kind mismatch, and
        // the worker answers `not-callable` without executing anything when the
        // value really is inert. Skipping it on the strength of the claim under
        // test was the negative claim exempting itself from its own falsifier.
        if (discovery && !isUnknown(selected.callbacks)) {
          const stated = new Set(rows(selected.callbacks).map(callback => callback.parameter));
          for (const parameter of DISCOVERY_PARAMETERS) {
            if (stated.has(parameter)) continue;
            request(
              mode,
              { entrypoint, export: exportName, discovery: true, parameter },
              {
                type: "discovery",
                entrypoint,
                specifier,
                export: exportName,
                parameter,
                arguments: synthesizeArguments(selected, parameter),
                callAccessor: selected.returns?.kind === "accessor"
              }
            );
          }
        }
      }
    }
  }

  return {
    claims: [...claims.values()],
    index,
    // Recorded, not implied. `--no-discovery` removes the only automated check
    // in the repository that can contradict a negative claim, and a report that
    // did not say so let `contract verify` list the incompleteness blocker as
    // "checked" when nothing had been checked. The parameter bound rides along
    // because it is a sampling bound and not a proof: discovery that ran only
    // over parameters 0 and 1 says nothing about parameter 2.
    discovery: { enabled: Boolean(discovery), parameters: discovery ? [...DISCOVERY_PARAMETERS] : [] },
    sessions: [...requests.entries()].map(([mode, probes]) => ({
      mode,
      conditions: PROBE_MODES.find(candidate => candidate.name === mode).conditions,
      probes
    }))
  };
}

/// The claims a summary holds that no probe drives, with the reason each has no
/// probe form and the family that decides whether verification keeps it.
function recordUndrivable(summary, push, prefix = "") {
  const at = suffix => (prefix ? `${prefix}.${suffix}` : suffix);
  const record = (claim, key) => push(claim, UNDRIVABLE[key], UNDRIVABLE_FAMILY[key]);
  for (const [index, callback] of rows(summary.callbacks).entries()) {
    if (callback.owner != null) record(at(`callbacks[${index}].owner`), "owner");
    if (Array.isArray(callback.arguments) && callback.arguments.length) {
      record(at(`callbacks[${index}].arguments`), "callbackArguments");
    }
  }
  for (const [index] of rows(summary.reactiveReads).entries()) {
    record(at(`reactiveReads[${index}]`), "reactiveRead");
  }
  for (const [index] of rows(summary.ownerRequirements).entries()) {
    record(at(`ownerRequirements[${index}]`), "ownerRequirement");
  }
  if (summary.asyncBehavior && !isUnknown(summary.asyncBehavior)) {
    record(at("asyncBehavior"), "asyncBehavior");
  }
  for (const [index, variant] of (summary.variants ?? []).entries()) {
    recordUndrivable(variant.summary, push, at(`variants[${index}].summary`));
  }
}

/// What a callback observation says the execution mode is, or `null` when the
/// counters do not name one.
///
/// The three modes classify **attribution, not timing**, which is what makes a
/// single generic body able to tell them apart:
///
///   * the call-site memo re-ran     -> the reads landed on the caller: inline
///   * only the callback re-ran      -> it holds its own subscription: tracked
///   * neither, but it ran during
///     the call                      -> synchronous with the listener cleared,
///                                      which `untrack`/`createRoot` do and
///                                      which is still inline
///   * neither, and it ran only
///     after the call returned       -> deferred
///
/// A call-site re-run necessarily re-invokes the callback, so `inline` is
/// checked first and the two counters are not treated as independent.
export function classifyExecution(observation) {
  const ranAtAll = observation.runsAfterWrite > 0;
  if (!ranAtAll) return null;
  if (observation.siteRunsAfterWrite > observation.siteRunsBeforeWrite) return "inline";
  if (observation.runsAfterWrite > observation.runsBeforeWrite) return "tracked";
  if (observation.ranDuringCall) return "inline";
  return "deferred";
}

const OUTCOME_REASON = {
  // The worker never ran, or never answered: a mode-wide fact, not a fact about
  // this claim, so it is still only ever `undriven`.
  "session-failed": result => result.error,
  "import-failed": result => `import of ${result.specifier} threw: ${result.error}`,
  "export-missing": result => `${result.export} is not exported by ${result.specifier} in this mode`,
  threw: result => `the synthesized call threw: ${result.error}`,
  "not-callable": result => `${result.export} is not callable, so no call could be synthesized`
};

/// Folds one session's raw observations into the claim records, and collects the
/// incompleteness findings.
///
/// `results` are the worker's raw records; nothing in them is a judgement.
export function interpretSession({ claims, index, mode, results }) {
  const byKey = new Map(
    claims.map(claim => [claimKey(claim.entrypoint, claim.export, claim.claim), claim])
  );
  const incompleteness = [];
  const evidence = [];
  for (const result of results) {
    const about = index.get(result.id);
    // A result for a probe this plan did not request cannot be attributed to
    // any claim, and guessing which one it meant is exactly the name-shaped
    // inference the precision contract forbids.
    if (!about) continue;
    if (about.discovery) {
      if (result.outcome !== "observed") continue;
      const observed = classifyExecution(result.observation);
      if (!observed) continue;
      incompleteness.push({
        entrypoint: about.entrypoint,
        export: about.export,
        claim: `callbacks[${about.parameter}]=${observed}`,
        mode,
        calls: measuredCalls(result),
        text:
          `${about.entrypoint}:${about.export} invoked the callback passed at parameter ` +
          `${about.parameter} in ${mode} (observed ${observed}), and the contract states no such claim`
      });
      continue;
    }
    const { entrypoint, export: exportName, claim } = about;
    const record = byKey.get(claimKey(entrypoint, exportName, claim));
    if (!record) continue;
    const observation = { mode, calls: measuredCalls(result) };
    if (result.outcome !== "observed") {
      observation.status = "undriven";
      observation.reason = (OUTCOME_REASON[result.outcome] ?? (() => result.outcome))(result);
      record.observations.push(observation);
      continue;
    }
    const verdict = verdictFor(claim, result.observation);
    observation.status = verdict.status;
    if (verdict.observed !== undefined) observation.observed = verdict.observed;
    if (verdict.reason) observation.reason = verdict.reason;
    record.observations.push(observation);
    if (verdict.status === "passed" || verdict.status === "failed") {
      evidence.push({
        entrypoint,
        export: exportName,
        claim,
        mode,
        calls: observation.calls,
        ok: verdict.status === "passed"
      });
    }
  }
  return { incompleteness, evidence };
}

/// One driven observation against one claim.
///
/// `failed` is reserved for a contradiction the run actually witnessed: a claim
/// the package answered differently. Everything inconclusive is `undriven`, so
/// a probe the driver could not construct never masquerades as a package defect.
function verdictFor(claim, observation) {
  if (claim.startsWith("kind=")) {
    const observed = observation.typeofValue === "function" ? "function" : "value";
    return observed === claim.slice("kind=".length)
      ? { status: "passed", observed }
      : { status: "failed", observed, reason: `runtime kind is ${observed}` };
  }
  if (claim.startsWith("returns=")) {
    if (observation.typeofValue !== "function") {
      return {
        status: "failed",
        observed: observation.typeofValue,
        reason: `the call returned a ${observation.typeofValue}, which cannot be an accessor`
      };
    }
    if (!observation.reactive) {
      return {
        status: "undriven",
        observed: "function",
        reason:
          "the returned value is callable but no re-read followed the planted write, so nothing observed it as a reactive accessor"
      };
    }
    // Reactivity alone does not distinguish an accessor from a closure that
    // forwards to the planted callback.
    //
    // The observation plants the signal read *inside the callback the contract
    // states*, so `(cb) => () => cb()` re-reads the signal on every read of the
    // returned value and the outer memo re-runs -- indistinguishable from a
    // memo accessor on the re-run counter alone. What separates them is
    // caching: within one evaluation of a single tracked scope, a memo accessor
    // read twice recomputes at most once, while a forwarding closure re-invokes
    // the callback on every read. So the observation reads the returned value
    // twice inside one memo body and reports how many times the planted
    // callback ran across those reads.
    //
    // An uncached derived accessor -- 1.x `mapArray`'s plain tracked function
    // is the real example -- re-invokes on every read and therefore lands
    // `undriven` here. That is the safe direction: the claim stays unproven and
    // its domain converts to the unknown sentinel, rather than being certified
    // by a property a forwarding closure also has.
    if (!Number.isInteger(observation.plantedRunsWithinOneRead)) {
      return {
        status: "undriven",
        observed: "function",
        reason:
          "the probe runtime reported no caching measurement for the returned value, so nothing " +
          "distinguished a reactive accessor from a closure forwarding to the planted callback"
      };
    }
    if (observation.plantedRunsWithinOneRead > 1) {
      return {
        status: "undriven",
        observed: "function",
        reason:
          `reading the returned value ${observation.trackedReadCalls ?? 2} times inside one tracked ` +
          `scope re-invoked the planted callback ${observation.plantedRunsWithinOneRead} times, which ` +
          "a plain closure forwarding to that callback does as well; caching could not distinguish " +
          "the returned value from a forwarding closure"
      };
    }
    return { status: "passed", observed: "accessor" };
  }
  const expected = claim.slice(claim.indexOf("=") + 1);
  const observed = classifyExecution(observation);
  if (!observed) {
    return {
      status: "undriven",
      reason:
        "the synthesized call completed without invoking the callback, so the claim was not exercised"
    };
  }
  if (observed !== expected) {
    // A mismatch is a failure only when the driver did not have to supply the
    // read scope itself.
    //
    // A lazily computed export runs its callback only when something reads what
    // it returned, so the driver reads it -- inside a memo of its own. Where
    // that callback's reads then land is partly a property of the scope the
    // driver created, not only of the export: 1.x `createSelector`'s comparator
    // runs synchronously inside the selector call (the bundled probe confirms
    // exactly that) while its reads register on the selector's internal
    // computation, and the counters cannot separate that from a callback with
    // a subscription of its own. Asserting a package defect on the strength of
    // the driver's own scaffolding is the wrong-is-dangerous direction, so the
    // disagreement is recorded and the claim stays unproven.
    if (observation.forcedByAccessorRead) {
      return {
        status: "undriven",
        observed,
        reason:
          `the callback ran only once the returned accessor was read, and in the driver's own read ` +
          `scope it looked ${observed} rather than ${expected}; which computation owns those reads ` +
          "is not something a synthesized read scope can settle"
      };
    }
    return { status: "failed", observed, reason: `observed ${observed}` };
  }
  return { status: "passed", observed };
}

/// Collapses each claim's per-mode observations into its overall status.
export function settleClaims(claims) {
  for (const claim of claims) {
    const driven = claim.observations.filter(observation => observation.status !== "undriven");
    claim.modesPassed = driven
      .filter(observation => observation.status === "passed")
      .map(observation => observation.mode);
    if (driven.some(observation => observation.status === "failed")) {
      claim.status = "failed";
      claim.reason = driven.find(observation => observation.status === "failed").reason;
    } else if (driven.length > 0) {
      claim.status = "passed";
      claim.calls = Math.max(...driven.map(observation => observation.calls ?? 0));
    } else {
      claim.status = "undriven";
      claim.reason ??=
        claim.observations[0]?.reason ??
        (claim.family === "C" ? "no probe form" : "no mode was attempted");
    }
  }
  return claims;
}

/// Ported from `scripts/check-bundled-contracts.mjs`: evidence only for a claim
/// that passed in every mode it was observed in, recording those modes and the
/// highest *measured* call count.
///
/// `calls` is floored at 1 because the schema's `probedEvidence.calls` is a
/// positive integer and a claim that passed was necessarily exercised at least
/// once. The floor is a schema accommodation, never a substitute for counting:
/// every passing callback and return observation reports a measurement above 0.
export function probeEvidence(resultsForClaim) {
  if (resultsForClaim.length === 0 || resultsForClaim.some(result => !result.ok)) {
    return undefined;
  }
  return {
    kind: "probed",
    modes: [...new Set(resultsForClaim.map(result => result.mode))].sort(),
    calls: Math.max(1, ...resultsForClaim.map(result => result.calls ?? 0))
  };
}

/// Which claims this run actually attempted, per claim string, as the set of
/// mode names the plan tried to drive them in.
///
/// Supersession needs this and passing evidence cannot supply it: a claim that
/// was driven and did *not* pass produces no evidence at all, which is exactly
/// the case where an older `probed` marker must not survive.
export function attemptedModes(claims) {
  const attempted = new Map();
  for (const claim of claims ?? []) {
    if (!claim.modesAttempted?.length) continue;
    const key = claimKey(claim.entrypoint, claim.export, claim.claim);
    const modes = attempted.get(key) ?? new Set();
    for (const mode of claim.modesAttempted) modes.add(mode);
    attempted.set(key, modes);
  }
  return attempted;
}

/// Ported from `scripts/check-bundled-contracts.mjs`'s `writeProbeEvidence`,
/// with one rule added and one rule kept.
///
/// **Kept:** a human's `reviewed` marker and an `inherited-from` marker are
/// never overwritten. Those are claims this command did not make and has no
/// standing to move.
///
/// **Added: a write supersedes.** A `probed` marker asserts "this claim was
/// observed, in these modes". When *this* run drove the same claim and it did
/// not pass -- the package changed, the import now throws, the run narrowed its
/// modes with `--modes` -- leaving the old marker standing publishes an
/// observation the current artifact does not support, and `contract verify`
/// then certifies it. So a re-driven claim either refreshes its marker with
/// what this run observed or loses it. A claim this run did not attempt at all
/// keeps what it had; verification separately refuses to certify a marker its
/// own report does not witness, so nothing stale survives to the verified tier
/// either way.
export function writeProbeEvidence(
  summary,
  results,
  entrypoint,
  name,
  allowedModes = PROBE_MODES,
  { attempted = new Map(), superseded = [], written = [], path = "" } = {}
) {
  const claimResults = claim =>
    results.filter(
      result =>
        result.entrypoint === entrypoint &&
        result.export === name &&
        result.claim === claim &&
        allowedModes.some(mode => mode.name === result.mode)
    );
  const drivenHere = claim => {
    const modes = attempted.get(claimKey(entrypoint, name, claim));
    return Boolean(modes && allowedModes.some(mode => modes.has(mode.name)));
  };
  const field = suffix => (path ? `${path}.${suffix}` : suffix);
  /// The marker a row should carry after this run, or `undefined` for none.
  const settleMarker = (existing, fresh, claim, where) => {
    if (existing && existing.kind !== "inferred" && existing.kind !== "probed") return existing;
    if (fresh) {
      written.push({ entrypoint, export: name, field: where, claim, evidence: fresh });
      return fresh;
    }
    if (existing?.kind === "probed" && drivenHere(claim)) {
      superseded.push({
        entrypoint,
        export: name,
        field: where,
        claim,
        previous: existing
      });
      return undefined;
    }
    return existing;
  };
  const next = { ...summary };
  const callbackClaims = rows(summary.callbacks).map(
    callback => `callbacks[${callback.parameter}]=${callback.execution}`
  );
  const returnClaim =
    summary.returns && !isUnknown(summary.returns) ? `returns=${summary.returns.kind}` : undefined;
  const exportClaims = [...callbackClaims, ...(returnClaim ? [returnClaim] : [])];
  const exportResults = exportClaims.map(claim => claimResults(claim)).flat();
  const summaryMarker = settleMarker(
    summary.evidence,
    probeEvidence(exportResults),
    // The summary marker covers every claim the export states, so it is
    // superseded as soon as any of them was re-driven.
    exportClaims.find(claim => drivenHere(claim)) ?? exportClaims[0] ?? "",
    field("evidence") || "evidence"
  );
  if (summaryMarker) next.evidence = summaryMarker;
  else delete next.evidence;
  if (Array.isArray(summary.callbacks)) {
    next.callbacks = summary.callbacks.map((callback, index) => {
      const claim = `callbacks[${callback.parameter}]=${callback.execution}`;
      const marker = settleMarker(
        callback.evidence,
        probeEvidence(claimResults(claim)),
        claim,
        field(`callbacks[${index}]`)
      );
      if (marker === callback.evidence) return callback;
      const row = { ...callback };
      if (marker) row.evidence = marker;
      else delete row.evidence;
      return row;
    });
  }
  if (returnClaim) {
    const marker = settleMarker(
      summary.returns.evidence,
      probeEvidence(claimResults(returnClaim)),
      returnClaim,
      field("returns")
    );
    if (marker !== summary.returns.evidence) {
      next.returns = { ...summary.returns };
      if (marker) next.returns.evidence = marker;
      else delete next.returns.evidence;
    }
  }
  if (summary.variants?.length) {
    next.variants = summary.variants.map((variant, index) => ({
      ...variant,
      summary: writeProbeEvidence(
        variant.summary,
        results,
        entrypoint,
        name,
        PROBE_MODES.filter(mode => conditionsMatchMode(variant.conditions, mode)),
        { attempted, superseded, written, path: field(`variants[${index}].summary`) }
      )
    }));
  }
  return next;
}

/// Applies probed evidence across a whole expanded contract, returning a new
/// document, the number of rows whose marker this run wrote (a refresh counts:
/// the run did observe the claim), and the markers a re-driven claim
/// superseded.
export function applyProbeEvidence(contract, results, claims = []) {
  const superseded = [];
  const written = [];
  const attempted = attemptedModes(claims);
  const entrypoints = Object.fromEntries(
    Object.entries(contract.entrypoints).map(([entrypoint, entry]) => [
      entrypoint,
      {
        ...entry,
        exports: Object.fromEntries(
          Object.entries(entry.exports).map(([name, summary]) => [
            name,
            writeProbeEvidence(summary, results, entrypoint, name, PROBE_MODES, {
              attempted,
              superseded,
              written,
              path: ""
            })
          ])
        )
      }
    ])
  );
  // Counted, not differenced. A net count of probed markers reported -6 for a
  // run that wrote none and superseded six, which reads as nonsense; the two
  // are separate facts and both are worth saying.
  return { contract: { ...contract, entrypoints }, written: written.length, superseded };
}

/// The `<contract>.probe.json` audit trail. Nothing certifies from it: it is the
/// record of what the machine believed, what it observed, and what it could not
/// reach.
export function buildProbeReport({
  contract,
  contractHash,
  contractPath,
  installed,
  generator,
  probeDriver,
  dialect,
  runtime,
  modes,
  discovery,
  claims,
  incompleteness
}) {
  const counted = kind => claims.filter(claim => claim.status === kind).length;
  return {
    schemaVersion: PROBE_REPORT_SCHEMA_VERSION,
    package: {
      name: contract.package?.name,
      version: contract.package?.version,
      ...(installed?.version ? { installedVersion: installed.version } : {}),
      ...(installed?.integrity ? { integrity: installed.integrity } : {})
    },
    contract: { path: contractPath, hash: contractHash },
    identities: {
      generator: generator ?? null,
      probeDriver,
      dialect,
      runtime
    },
    modes: modes.map(mode => mode.name),
    // The discovery state, recorded rather than implied. Discovery is the only
    // automated check in this repository that can contradict a negative claim,
    // so a report that ran without it has not checked the incompleteness
    // condition at all -- and `contract verify` refuses such a report rather
    // than listing a blocker it could not evaluate.
    discovery: discovery ?? { enabled: false, parameters: [] },
    summary: {
      claims: claims.length,
      driven: claims.filter(claim => claim.status !== "undriven").length,
      passed: counted("passed"),
      failed: counted("failed"),
      undriven: counted("undriven"),
      incompleteness: incompleteness.length
    },
    claims: claims
      .map(claim => ({
        entrypoint: claim.entrypoint,
        export: claim.export,
        claim: claim.claim,
        family: claim.family,
        status: claim.status,
        ...(claim.arguments ? { arguments: claim.arguments } : {}),
        modes: { attempted: [...new Set(claim.modesAttempted)].sort(), passed: [...new Set(claim.modesPassed)].sort() },
        ...(claim.calls ? { calls: claim.calls } : {}),
        ...(claim.reason ? { reason: claim.reason } : {}),
        ...(claim.observations.length ? { observations: claim.observations } : {})
      }))
      .sort(
        (left, right) =>
          left.entrypoint.localeCompare(right.entrypoint) ||
          left.export.localeCompare(right.export) ||
          left.claim.localeCompare(right.claim)
      ),
    incompleteness
  };
}
