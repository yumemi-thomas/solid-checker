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

/// The import-time browser surface a client-mode probe session provides.
///
/// **This is a weakening of the observation, and it is recorded as one.** A
/// module that reads `window` at import time in a bare Node process throws
/// `ReferenceError`, the worker stops, and every probe of that entrypoint is
/// undriven -- so nothing about the package is observed at all. Defining a
/// small inert surface lets the module load, and what is then observed is the
/// package's behavior *given a fake DOM*, which is not the same fact as its
/// behavior in a browser. `environment` in `<contract>.probe.json` and in
/// `<contract>.verify.json` says which names were faked, per mode, so the
/// difference stays legible instead of being absorbed into the numbers.
///
/// Three rules keep the weakening bounded:
///
///   * **Mode-scoped.** Only modes whose conditions include `browser` get it.
///     A `server`/`node` mode import that throws on `window` is a *truthful*
///     observation of that entrypoint in that mode, and faking it there would
///     manufacture a pass the package never earns.
///   * **Never at generation.** `contract generate` imports nothing at all, so
///     no shim exists on the static path. This lives in the worker only.
///   * **Empirical, not speculative.** The list is derived from what the
///     corpus's failing packages actually touch at import time -- see
///     docs/package-contracts.md's probe section for the derivation -- and a
///     name nothing reached is not added on the theory that a browser has it.
///     A module that still throws with the shim in place is left exactly as it
///     was: undriven, `import-failed`, with the throw as its reason.
export const BROWSER_SHIM_GLOBALS = [
  "window",
  "document",
  "navigator",
  "self",
  "location",
  "screen",
  "history",
  "localStorage",
  "sessionStorage",
  "matchMedia",
  "requestAnimationFrame",
  "cancelAnimationFrame",
  "getComputedStyle",
  "MutationObserver",
  "ResizeObserver",
  "IntersectionObserver"
];

/// The environment a session runs under: which globals, if any, the worker
/// fakes before it imports anything.
///
/// `shim: false` reproduces the bare-Node environment every measurement before
/// this one ran in, which is what makes the shim's effect separable rather
/// than baked into a single number.
export function environmentForMode(mode, { shim = true } = {}) {
  const browserish = (mode?.conditions ?? []).includes("browser");
  return shim && browserish
    ? { kind: "browser-globals", globals: [...BROWSER_SHIM_GLOBALS] }
    : { kind: "none", globals: [] };
}

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
/// A Type Facts construction plan may now name several proven inhabitants for
/// one slot. This is not a guess-and-survive ladder: every candidate is derived
/// from the package's declarations, every attempt remains visible in the probe
/// report, and a contradiction from any attempt fails the claim. A slot the
/// vocabulary cannot fill stays `undefined`.
export const ARGUMENT_SYNTHESIS = [
  "probe-callback",
  "probe-value",
  "noop-callback",
  "empty-object",
  "null",
  "empty-array",
  "empty-map",
  "empty-set",
  "undefined"
];

/// Multiple type-directed candidates improve reach, but the product of several
/// union parameters must not turn one claim into an unbounded probe session.
export const MAX_CONSTRUCTION_ATTEMPTS = 8;

export function isArgumentRecipe(recipe) {
  if (ARGUMENT_SYNTHESIS.includes(recipe)) return true;
  if (!recipe || typeof recipe !== "object" || Array.isArray(recipe)) return false;
  if (recipe.kind !== "literal" || Object.keys(recipe).sort().join(",") !== "kind,value") {
    return false;
  }
  return (
    typeof recipe.value === "string" ||
    typeof recipe.value === "boolean" ||
    (typeof recipe.value === "number" && Number.isFinite(recipe.value))
  );
}

export function applyConstructionPlan(descriptors, recipes) {
  return applyConstructionPlans(descriptors, recipes, 1)[0];
}

export function applyConstructionPlans(
  descriptors,
  recipes,
  limit = MAX_CONSTRUCTION_ATTEMPTS
) {
  const baseline = [...descriptors];
  if (!recipes || typeof recipes !== "object" || limit <= 0) return [baseline];
  const slots = [];
  for (const [rawIndex, rawCandidates] of Object.entries(recipes).sort(
    ([left], [right]) => Number(left) - Number(right)
  )) {
    const index = Number(rawIndex);
    if (!Number.isInteger(index) || index < 0 || index >= baseline.length) continue;
    if (baseline[index] !== "undefined") continue;
    const candidates = (Array.isArray(rawCandidates) ? rawCandidates : [rawCandidates])
      .filter(isArgumentRecipe)
      .filter((recipe, index, all) =>
        all.findIndex(candidate => JSON.stringify(candidate) === JSON.stringify(recipe)) === index
      );
    if (!candidates.length) continue;
    baseline[index] = candidates[0];
    slots.push({ index, candidates });
  }
  const plans = [baseline];
  const seen = new Set([JSON.stringify(baseline)]);
  const add = candidate => {
    const key = JSON.stringify(candidate);
    if (seen.has(key) || plans.length >= limit) return;
    seen.add(key);
    plans.push(candidate);
  };

  // First expose every alternate independently. If the budget permits, add
  // combinations in deterministic Cartesian order. This avoids hiding a later
  // parameter's second candidate merely because an earlier union was wide.
  for (let offset = 1; plans.length < limit; offset += 1) {
    let found = false;
    for (const { index, candidates } of slots) {
      if (offset >= candidates.length) continue;
      found = true;
      const candidate = [...baseline];
      candidate[index] = candidates[offset];
      add(candidate);
    }
    if (!found) break;
  }
  const combine = (slotIndex, candidate, changed) => {
    if (plans.length >= limit) return;
    if (slotIndex === slots.length) {
      if (changed >= 2) add(candidate);
      return;
    }
    const { index, candidates } = slots[slotIndex];
    for (let choice = 0; choice < candidates.length && plans.length < limit; choice += 1) {
      const next = [...candidate];
      next[index] = candidates[choice];
      combine(slotIndex + 1, next, changed + Number(choice > 0));
    }
  };
  combine(0, baseline, 0);
  return plans;
}

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

/// The exact identity of a return claim.
///
/// Relational returns include their parameter because the parameter is part of
/// the schema claim: evidence for `argument[0]` must never corroborate a later
/// artifact that says `argument[1]`. Concrete return shapes have no parameter
/// and keep the original spelling.
function returnPathSuffix(path) {
  return path
    .map(segment =>
      typeof segment === "number"
        ? `.elements[${segment}]`
        : `.properties[${JSON.stringify(segment)}]`
    )
    .join("");
}

export function returnClaim(returned, path = []) {
  if (!returned || isUnknown(returned) || typeof returned.kind !== "string") return undefined;
  const prefix = `returns${returnPathSuffix(path)}`;
  if (
    ["argument", "callback-result", "callback-result-function"].includes(returned.kind) &&
    Number.isInteger(returned.parameter)
  ) {
    return `${prefix}=${returned.kind}[${returned.parameter}]`;
  }
  return `${prefix}=${returned.kind}`;
}

export function returnLeaves(returned, path = []) {
  if (!returned || isUnknown(returned)) return [];
  if (returned.kind === "tuple") {
    return (returned.elements ?? []).flatMap((leaf, index) =>
      leaf ? returnLeaves(leaf, [...path, index]) : []
    );
  }
  if (returned.kind === "object") {
    return Object.entries(returned.properties ?? {}).flatMap(([name, leaf]) =>
      returnLeaves(leaf, [...path, name])
    );
  }
  return [{ returned, path }];
}

function synthesizeReturnArguments(summary, returned) {
  const callbackRelation = ["callback-result", "callback-result-function"].includes(returned.kind);
  const descriptors = synthesizeArguments(
    summary,
    callbackRelation ? returned.parameter : undefined
  );
  while (descriptors.length <= returned.parameter) descriptors.push("undefined");
  if (returned.kind === "argument") descriptors[returned.parameter] = "probe-value";
  return descriptors;
}

/// Where an undriven claim's reason comes from. Each string names the missing
/// mechanism, not the claim, so a report reader can tell a permanent gap
/// (owner rows) from one Stage 2+ could close (a store-path probe form).
///
/// Exported only so the corpus harness's reason-bucket test can be total over
/// this table rather than over a copied list of it: a reason nobody classified
/// lands in the harness's `other` bucket, which is worst exactly when a new
/// withdrawal class is the largest one in a run.
export const UNDRIVABLE = {
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
    "writeProbeEvidence does not descend into return leaves and no claim string names one"
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
  nestedReturn: "C"
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
export function buildProbePlan(
  contract,
  { modes = PROBE_MODES, discovery = true, environmentShim = true, constructionPlan } = {}
) {
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
      const construct = descriptors =>
        applyConstructionPlans(
          descriptors,
          constructionPlan?.entrypoints?.[entrypoint]?.[exportName]
        );
      const recordAttempts = (record, descriptors) => {
        const attempts = construct(descriptors);
        record.arguments = attempts[0];
        if (attempts.length > 1) record.argumentAttempts = attempts;
        return attempts;
      };
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
            for (const arguments_ of recordAttempts(
              record,
              synthesizeArguments(selected, callback.parameter)
            )) {
              request(
                mode,
                { entrypoint, export: exportName, claim },
                {
                  type: "callback",
                  entrypoint,
                  specifier,
                  export: exportName,
                  parameter: callback.parameter,
                  arguments: arguments_,
                  // A lazily-computed export (a memo) never runs its callback
                  // until the accessor it returned is read, and the contract is
                  // what says an accessor was returned. Reading it is therefore
                  // contract-led, not a guess about the return value.
                  callAccessor: selected.returns?.kind === "accessor"
                }
              );
            }
          }
        }

        if (selected.returns && !isUnknown(selected.returns)) {
          for (const { returned, path } of returnLeaves(selected.returns)) {
            const claim = returnClaim(returned, path);
            const record = claimRecord(entrypoint, exportName, claim, "B");
            if (returned.kind === "accessor") {
              const plant = rows(selected.callbacks)[0]?.parameter;
              if (!Number.isInteger(plant)) {
                record.family = "C";
                record.reason =
                  "no plantable reactive source: proving the returned value is an accessor needs a signal read inside a callback the contract states, and this export states none";
              } else {
                record.modesAttempted.push(mode.name);
                for (const arguments_ of recordAttempts(
                  record,
                  synthesizeArguments(selected, plant)
                )) {
                  request(
                    mode,
                    { entrypoint, export: exportName, claim },
                    {
                      type: "returns-accessor",
                      entrypoint,
                      specifier,
                      export: exportName,
                      parameter: plant,
                      arguments: arguments_,
                      returnPath: path
                    }
                  );
                }
              }
            } else if (
              ["argument", "callback-result", "callback-result-function"].includes(returned.kind)
            ) {
              record.modesAttempted.push(mode.name);
              for (const arguments_ of recordAttempts(
                record,
                synthesizeReturnArguments(selected, returned)
              )) {
                request(
                  mode,
                  { entrypoint, export: exportName, claim },
                  {
                    type: `returns-${returned.kind}`,
                    entrypoint,
                    specifier,
                    export: exportName,
                    parameter: returned.parameter,
                    arguments: arguments_,
                    returnPath: path
                  }
                );
              }
            } else {
              record.family = "C";
              record.reason =
                returned.kind === "store-path"
                  ? UNDRIVABLE.storePath
                  : UNDRIVABLE.nestedReturn;
            }
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
            for (const arguments_ of construct(synthesizeArguments(selected, parameter))) {
              request(
                mode,
                { entrypoint, export: exportName, discovery: true, parameter },
                {
                  type: "discovery",
                  entrypoint,
                  specifier,
                  export: exportName,
                  parameter,
                  arguments: arguments_,
                  callAccessor: selected.returns?.kind === "accessor"
                }
              );
            }
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
    sessions: [...requests.entries()].map(([mode, probes]) => {
      const definition = PROBE_MODES.find(candidate => candidate.name === mode);
      return {
        mode,
        conditions: definition.conditions,
        // Carried on the session rather than resolved in the worker so that the
        // decision "this mode gets a fake DOM" is made once, in the parent that
        // also writes the record of it, and the worker only applies what it was
        // told. A worker that chose for itself could observe under a shim the
        // report does not mention.
        environment: environmentForMode(definition, { shim: environmentShim }),
        probes
      };
    })
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

/// Why a set of counters names no execution mode. Each reason distinguishes one
/// unattributable observation from another, because "the callback never ran" and
/// "the callback ran, and nothing here can say on whose behalf" are different
/// facts about the package and about this probe.
///
/// Each reason states **what the counters showed**, never a mechanism inferred
/// from them. These reasons are published per claim in `<contract>.probe.json`
/// and aggregated by the corpus harness, so a reason is a claim about a package
/// and is held to the same standard as a verdict: "the callback re-ran in an
/// interval where nothing was written" is an observation, while "the callback
/// schedules itself" is a guess that the same three counts do not license -- a
/// genuinely tracked callback whose subscription starts late produces them too.
export const EXECUTION_UNATTRIBUTABLE = {
  neverRan:
    "the synthesized call completed without invoking the callback, so the claim was not exercised",
  unwrittenRerun:
    "the callback re-ran across a settle interval in which nothing was written, and re-ran again " +
    "after the write, so it re-runs without a write and no re-run can be attributed to the write",
  firstRunAfterWrite:
    "the callback had not run by the time of the write and ran only after it, so the write cannot " +
    "have caused a re-run and a first run alone does not say whether it holds a subscription",
  transitiveSubscription:
    "the callback ran more times than the call site re-invoked the export, so a subscription other " +
    "than the call site's re-ran it and the counters cannot say which reads were the call site's",
  runtimeInert:
    "the reactive runtime this observation was made in re-runs nothing, so inline, tracked and " +
    "deferred are indistinguishable and a matching observation would not be evidence",
  noControlInterval:
    "the observation reports no count for the settle interval in which nothing was written, so a " +
    "re-run the write caused cannot be separated from activity it did not cause"
};

/// What a callback observation says the execution mode is, and when it says
/// nothing, why.
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
/// Three things have to be ruled out before those readings mean anything, and
/// none of them used to be.
///
/// **A re-run has to be caused by the write.** `runsAfterControl` is the count
/// after a settle interval in which nothing was written, so a callback that ran
/// again there ran again without a write. If it also ran again after the write,
/// no re-run can be attributed and the observation names nothing --
/// `createTimeoutLoop`, which reschedules itself, is one such shape, and the
/// counters do not say that is why: `raf(() => raf(() => createEffect(cb)))` is
/// a genuinely tracked callback whose subscription starts late and it produces
/// the same three counts, so the withdrawal is forced and naming a mechanism in
/// the reason would not be. If it did *not* run again after the write, the write
/// caused nothing: `afterPaint`'s double `requestAnimationFrame` merely landed
/// its first run late, and it is `deferred`, which is what the contract said.
/// Both used to read `tracked`.
///
/// **A first run is not a re-run.** A callback that had not run by the time of
/// the write -- not during the call, and not across the control interval -- held
/// no subscription to the probe's signal: the only read of that signal is in the
/// callback body, so a callback that never executed never subscribed, and the
/// write cannot have caused the first run it is being credited with. This used
/// to read `tracked` about a callback that had never run at all: a plain
/// `setTimeout(cb, 3)` and a triple `requestAnimationFrame` land their first run
/// in the write interval rather than the control interval, and a package that
/// defers by three macrotask hops was reported as defective against the
/// `deferred` claim it honours -- on some runs and not others, since which
/// interval the run lands in is a property of the machine's load.
///
/// It is not `deferred` either, and this is the reason a first run names nothing
/// rather than the other mode. The `deferred` reading of `rb 0, rc 1, ra 1` is
/// earned: the callback ran, and so read the signal, *before* the write, and the
/// write then did not re-run it -- which is a subscription's absence, observed.
/// No such test exists for a run that happens only after the write. A callback
/// whose subscription is established late -- `raf(() => raf(() => raf(() =>
/// createEffect(cb))))`, "start tracking after paint", an ordinary idiom -- runs
/// exactly once, in the write interval, having never run before it, and is
/// genuinely `tracked`. The counters are identical, so neither mode is provable
/// and the observation names none.
///
/// **A call-site re-run is not proof of `inline`.** It is implied by `inline` and
/// the converse was assumed: the site also re-runs when the export reads its own
/// tracked derivation of the callback *during the call*, which subscribes the
/// caller transitively -- `mergeProps({...defaults}, props)` then reading a
/// defaulted member is exactly this, and so is an export that invokes the
/// parameter once inline and once inside an effect. What separates them is that
/// the callback then ran more often than the site re-invoked the export:
/// `runDelta > siteDelta > 0` proves a subscription the call site does not own,
/// and which of the two the reads belonged to is not something these counters
/// can settle. So it is unattributable rather than `inline`.
///
/// The residual conservatism is deliberate: an export that invokes the callback
/// twice per call is `inline` and reads as unattributable here. Failing closed on
/// a shape whose counters a genuinely tracked callback also produces is the safe
/// direction; certifying one because the arithmetic happened to agree is not.
export function classifyExecutionResult(observation) {
  if (!(observation.runsAfterWrite > 0)) {
    return { execution: null, reason: EXECUTION_UNATTRIBUTABLE.neverRan };
  }
  // The control interval is a measurement this classification requires, not one
  // it can do without: reading a missing count as the baseline would restore the
  // pre-control-interval classifier for that observation, which is the source of
  // the wrong verdicts the interval exists to remove. The worker always reports
  // it, so this is fail-closed on a malformed observation rather than a
  // compatibility path.
  const control = observation.runsAfterControl;
  if (typeof control !== "number") {
    return { execution: null, reason: EXECUTION_UNATTRIBUTABLE.noControlInterval };
  }
  if (control > observation.runsBeforeWrite && observation.runsAfterWrite > control) {
    return { execution: null, reason: EXECUTION_UNATTRIBUTABLE.unwrittenRerun };
  }
  const siteDelta = observation.siteRunsAfterWrite - observation.siteRunsBeforeWrite;
  const runDelta = observation.runsAfterWrite - control;
  if (siteDelta > 0 && runDelta > siteDelta) {
    return { execution: null, reason: EXECUTION_UNATTRIBUTABLE.transitiveSubscription };
  }
  if (siteDelta > 0) return { execution: "inline" };
  // A callback that had not run by the time of the write held no subscription to
  // the probe's signal -- subscribing to it means reading it, and only the
  // callback body reads it -- so its run in the write interval is a first run
  // and not a re-run the write caused. `ranDuringCall` is false by construction
  // here: the baseline is taken after the call returned, so a run during the
  // call would have raised `runsBeforeWrite`.
  if (observation.runsBeforeWrite === 0 && control === 0) {
    return { execution: null, reason: EXECUTION_UNATTRIBUTABLE.firstRunAfterWrite };
  }
  if (runDelta > 0) return { execution: "tracked" };
  if (observation.ranDuringCall) return { execution: "inline" };
  return { execution: "deferred" };
}

/// The execution mode a callback observation names, or `null` when it names
/// none. `classifyExecutionResult` carries the reason for the `null`.
export function classifyExecution(observation) {
  return classifyExecutionResult(observation).execution;
}

/// Whether the runtime an observation was made in could re-run anything.
///
/// Fail-closed on absence: the worker stamps every driven observation with the
/// capability of the runtime that produced it, so a callback observation with no
/// stamp is an observation whose runtime was never asked -- and an unasked
/// runtime is not a re-running one. The stamp is per observation rather than per
/// session because one session holds more than one runtime: probing
/// solid-js@1.9.14 in `server` mode, `.` resolves to the non-reactive
/// `dist/server.js` while `./jsx-dev-runtime` resolves unconditionally to
/// `dist/solid.js` and is driven by its own fully reactive primitives.
function runtimeReran(result) {
  return result?.runtime?.reruns === true;
}

/// The reason a non-observing probe outcome carries.
///
/// Exported for the same reason `UNDRIVABLE` is: the corpus harness buckets
/// these strings, and the test that keeps its buckets total reads this table
/// instead of restating it. `session-failed` forwards the session layer's own
/// text (packages/cli/scripts/probe-contract.mjs), so that one entry is not a
/// string this file owns.
export const OUTCOME_REASON = {
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
      // A discovery finding is a claim string this run would put to a reviewer,
      // so it needs the same attribution the claim it contradicts needs. In a
      // runtime that re-runs nothing the mode in that string would be whatever
      // the inert scaffolding defaulted to.
      if (!runtimeReran(result)) continue;
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
    // A callback execution claim is a claim about attribution, and attribution
    // is not observable in a runtime where nothing re-runs. Both directions are
    // withdrawn, not just the failures: an `inline` or `deferred` claim probed
    // against an inert runtime *matches* -- the scaffolding can produce nothing
    // else -- and recording that as a pass certifies a row nothing observed.
    //
    // `kind` claims read `typeof` and need no reactivity. A `returns` claim
    // keeps its verdict because it already requires a re-read to pass, and
    // because `the call returned an object` is a real observation an inert
    // runtime can still make.
    if (claim.startsWith("callbacks[") && !runtimeReran(result)) {
      observation.status = "undriven";
      observation.reason = EXECUTION_UNATTRIBUTABLE.runtimeInert;
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
  if (claim.startsWith("returns=argument[") || claim.startsWith("returns=callback-result[")) {
    return observation.identityMatched
      ? { status: "passed", observed: "identity" }
      : {
          status: "failed",
          observed: observation.returnedType,
          reason: "the completed call did not return the planted value by identity"
        };
  }
  if (claim.startsWith("returns=callback-result-function[")) {
    if (observation.returnedType !== "function") {
      return {
        status: "failed",
        observed: observation.returnedType,
        reason: `the call returned a ${observation.returnedType}, so there was no returned function to invoke`
      };
    }
    return observation.identityMatched
      ? { status: "passed", observed: "identity" }
      : {
          status: "failed",
          observed: observation.invocationResultType,
          reason: "invoking the returned function did not return the planted callback value by identity"
        };
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
  const { execution: observed, reason } = classifyExecutionResult(observation);
  if (!observed) return { status: "undriven", reason };
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
  const returnClaimNames = returnLeaves(summary.returns).map(({ returned, path }) =>
    returnClaim(returned, path)
  );
  const exportClaims = [...callbackClaims, ...returnClaimNames];
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
  if (returnClaimNames.length) {
    const visit = (returned, returnPath = [], where = "returns") => {
      if (returned.kind === "tuple") {
        return {
          ...returned,
          elements: (returned.elements ?? []).map((leaf, index) =>
            leaf ? visit(leaf, [...returnPath, index], `${where}.elements[${index}]`) : leaf
          )
        };
      }
      if (returned.kind === "object") {
        return {
          ...returned,
          properties: Object.fromEntries(
            Object.entries(returned.properties ?? {}).map(([property, leaf]) => [
              property,
              visit(
                leaf,
                [...returnPath, property],
                `${where}.properties[${JSON.stringify(property)}]`
              )
            ])
          )
        };
      }
      const claim = returnClaim(returned, returnPath);
      const marker = settleMarker(
        returned.evidence,
        probeEvidence(claimResults(claim)),
        claim,
        field(where)
      );
      const leaf = { ...returned };
      if (marker) leaf.evidence = marker;
      else delete leaf.evidence;
      return leaf;
    };
    next.returns = visit(summary.returns);
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
  environment,
  sessions,
  claims,
  incompleteness
}) {
  const counted = kind => claims.filter(claim => claim.status === kind).length;
  const perMode = environment ?? {};
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
    // Which globals each mode's worker faked before it imported anything.
    //
    // Recorded for the same reason `discovery` is: a claim observed against a
    // fake DOM is a weaker observation than one observed against a browser,
    // and the difference is invisible in the claim record itself. `shimmed`
    // names what this process invented; `present` names what Node already had
    // and the worker therefore left alone. A mode with an empty `shimmed` list
    // observed in a bare Node process, which is what every `server` session
    // does by construction.
    environment: {
      shimmedAnyMode: Object.values(perMode).some(entry => (entry?.shimmed ?? []).length > 0),
      modes: Object.fromEntries(
        Object.entries(perMode).map(([mode, entry]) => [
          mode,
          {
            kind: entry?.kind ?? "none",
            shimmed: [...(entry?.shimmed ?? [])].sort(),
            present: [...(entry?.present ?? [])].sort()
          }
        ])
      )
    },
    // How many worker processes each mode cost, and how many of those were
    // restarts after a probe threw. A restart is not a failure -- it is the
    // only way to un-halt a Solid 2.0 development runtime -- but a mode that
    // needed dozens of them is the shape behind a slow or timed-out row, and
    // nothing recorded it before.
    //
    // `runtime` is the capability the worker measured for the runtime that drove
    // that mode's ordinary packages -- `{ reruns: false }` for a mode whose
    // artifact re-runs nothing. Attribution is still decided per observation,
    // because one session can hold two runtimes with opposite answers; this is
    // the mode-level record that says a batch of `undriven` rows was withdrawn
    // because the runtime was *asked and answered*, which no per-claim reason
    // can establish. `null` on a mode whose processes all died before importing
    // `solid-js`: "not measured" is not "measured, and nothing re-ran".
    sessions: {
      started: (sessions ?? []).reduce((total, entry) => total + (entry.started ?? 0), 0),
      restarts: (sessions ?? []).reduce((total, entry) => total + (entry.restarts ?? 0), 0),
      failed: (sessions ?? []).reduce((total, entry) => total + (entry.failed ?? 0), 0),
      byMode: Object.fromEntries(
        (sessions ?? []).map(entry => [
          entry.mode,
          {
            started: entry.started ?? 0,
            restarts: entry.restarts ?? 0,
            failed: entry.failed ?? 0,
            completed: Boolean(entry.completed),
            runtime: entry.runtime ?? null
          }
        ])
      )
    },
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
        ...(claim.argumentAttempts ? { argumentAttempts: claim.argumentAttempts } : {}),
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
