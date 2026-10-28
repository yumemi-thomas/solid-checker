// The Solid dev diagnostic codes, the checker rules that claim them, and the dev-build errors thrown with no
// code. `codeRules`, `thrownCodes` and `withholdingRules` copy the tables in scripts/runtime-oracle.mjs: that file
// is a script with top-level effects and cannot be imported, so codes.test.mjs reads it and fails when the two
// drift. `codeRules` is derived from the policy table below.

// --- The policy table (phase D) -------------------------------------------------------------------------------
// Every rc.17 dev diagnostic code, classified once: which checker rules claim it, what a diagnostic of it means for
// the comparison (`disposition`), and how loud Solid is about it (`severity`: "error" throws or is an engine failure,
// "warn" and "info" report). codes.test.mjs greps `code: "X"` across the pinned dev dists and fails on a code that is
// not here, so a Solid bump cannot add noise silently.
//
// Dispositions:
//   defect-claimed    a defect some checker rule claims (`rules`): a diagnostic with no finding is a false clean
//   defect-unclaimed  a defect no rule claims (a recall gap by "never quieter than Solid"): a diagnostic with no finding
//                     is a false clean; the ledger (known-open.json) holds the accepted ones, with a reference
//   consequence       the engine's reaction to an earlier failure: never a finding of its own, report to Solid
//   advisory          notes Solid documents as legal (performance, attribution, an FYI), not defects; a rule may still claim one
//                     (`rules`), as an optional warning, but a diagnostic with no finding is not a false clean
//   out-of-scope      SSR and server-only, browser-host or typed-away (invariant 1) codes
export const DISPOSITIONS = ["defect-claimed", "defect-unclaimed", "consequence", "advisory", "out-of-scope"];

const claimed = (rules, severity) => ({ rules, disposition: "defect-claimed", severity });
const unclaimed = (severity, note) => ({ rules: [], disposition: "defect-unclaimed", severity, note });
const consequence = (severity) => ({ rules: [], disposition: "consequence", severity });
const advisory = (severity, rules = [], note) => ({ rules, disposition: "advisory", severity, ...(note ? { note } : {}) });
const outOfScope = (severity, note) => ({ rules: [], disposition: "out-of-scope", severity, note });

export const codePolicy = {
  // Claimed. Sources: `@solidjs/signals` dev.js and dev-shared.js; thrown ones are named in `thrownCodes`.
  STRICT_READ_UNTRACKED: claimed(["strict-read-untracked", "no-destructure", "reactive-handler-frozen", "components-return-once"], "warn"),
  NO_OWNER_CLEANUP: claimed(["missing-owner"], "warn"),
  NO_OWNER_EFFECT: claimed(["missing-owner", "primitive-in-directive-application"], "warn"),
  NO_OWNER_BOUNDARY: claimed(["missing-owner"], "warn"),
  NO_OWNER_CONTEXT: claimed(["missing-owner"], "error"),
  CONTEXT_NOT_FOUND: claimed(["missing-context-provider"], "error"),
  SETTLED_CLEANUP_UNOWNED: claimed(["missing-owner"], "error"),
  REACTIVE_WRITE_IN_OWNED_SCOPE: claimed(["no-write-in-pure-computation"], "error"),
  // A write through a store's read proxy is ignored with this warning (`warnWriteOutsideSetter`); SC2003.
  STORE_WRITE_OUTSIDE_SETTER: claimed(["no-store-mutation-outside-setter"], "warn"),
  CLEANUP_IN_FORBIDDEN_SCOPE: claimed(["leaf-owner-forbidden-call"], "error"),
  PRIMITIVE_IN_FORBIDDEN_SCOPE: claimed(["leaf-owner-forbidden-call"], "error"),
  // Legal by Solid's own rules: an async read with no <Loading> above it makes the root wait and attach at once (rc.17
  // `effect.ts`; SPEC-ASYNC-SEMANTICS A6 "warn-only", 08-dev-diagnostics "FYI, not an error"). SC5003 reports it as a warning.
  ASYNC_OUTSIDE_LOADING_BOUNDARY: advisory("warn", ["async-outside-loading-boundary"], "legal: SPEC-ASYNC-SEMANTICS A6; the rule is an optional warning"),
  PENDING_ASYNC_UNTRACKED_READ: claimed(["pending-async-unsuspendable-read"], "error"),
  // A pending read in a leaf owner (createTrackedEffect, onSettled): SC5001's leaf-owner variant, warning severity like Solid's
  // (docs/rules/pending-async-unsuspendable-read.md; @solidjs/signals rc.17 dev-shared.js).
  PENDING_ASYNC_FORBIDDEN_SCOPE: claimed(["pending-async-unsuspendable-read"], "warn"),
  SYNC_NODE_RECEIVED_ASYNC: claimed(["sync-computation-received-async"], "error"),
  UNTRACKED_READ_AFTER_AWAIT: claimed(["reactive-read-after-await"], "warn"),
  MISSING_EFFECT_FN: claimed(["missing-effect-function"], "error"),
  ACTION_CALLED_IN_OWNED_SCOPE: claimed(["action-called-in-owned-scope"], "error"),
  RESOLVE_IN_REACTIVE_SCOPE: claimed(["no-await-primitive-in-tracked-scope"], "error"),
  UNTIL_IN_REACTIVE_SCOPE: claimed(["no-await-primitive-in-tracked-scope"], "error"),
  FLUSH_IN_ACTION: claimed(["flush-in-action"], "error"),
  FLUSH_IN_EFFECT_CALLBACK: claimed(["flush-in-effect-callback"], "warn"),
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: claimed(["leaf-owner-forbidden-call"], "error"),
  DYNAMIC_STATIC_PROMISE: claimed(["static-dynamic-async-source"], "error"),

  // Defects no rule claims.
  PRIMITIVE_IN_EFFECT_CALLBACK: unclaimed("error"),
  RUN_WITH_DISPOSED_OWNER: unclaimed("warn", "decided not to build: docs/open-work.md, `runWithOwner` with a disposed owner"),
  ASYNC_STORE_SETTER: unclaimed("error"),
  UNCOMMITTED_INPUT_OVERWRITTEN: unclaimed("warn", "decided not to build: docs/open-work.md, `UNCOMMITTED_INPUT_OVERWRITTEN`"),
  LOADING_ON_OUTSIDE_HOLD: unclaimed("warn"),

  // The engine's reaction to an earlier failure.
  REACTIVITY_HALTED: consequence("error"),
  INVARIANT_VIOLATION: consequence("error"),
  SETTLE_WALK_UNINITIALIZED_SOURCE: consequence("error"),

  // Performance and attribution notes (HUGE_FAN_*; the rest only the attribution build emits).
  HUGE_FAN_OUT: advisory("warn"),
  HUGE_FAN_IN: advisory("warn"),
  ABANDONED_FLIGHTS: advisory("warn"),
  ASYNC_WATERFALL: advisory("warn"),
  EFFECT_RELAY_TEAR: advisory("warn"),
  EFFECT_WRITES_OWN_SOURCE: advisory("warn"),
  FALLBACK_FLASH: advisory("info"),
  GRAPH_GROWTH: advisory("warn"),
  HOT_SCOPE_FANOUT: advisory("warn"),
  HOT_SCOPE_RERUNS: advisory("warn"),
  HOT_SCOPE_TIME: advisory("warn"),
  IMMUTABLE_UPDATE_IN_STORE: advisory("warn"),
  LONG_HOLD: advisory("warn"),
  OPTIMISTIC_REVERTED: advisory("info"),
  SILENT_HOLD: advisory("warn"),
  STACKED_HOLDS: advisory("warn"),
  UNSTABLE_LIST_IDENTITY: advisory("warn"),
  UNSTABLE_MEMO_OUTPUT: advisory("warn"),
  UNTRACKED_ASYNC_HANDLER: advisory("warn"),
  WASTED_RECOMPUTE: advisory("warn"),
  WIDE_SCOPE_DEPS: advisory("warn"),

  // Server rendering, the browser host's attribute and insert checks, and codes the published typings already reject.
  HEAD_TAG_INVALID: outOfScope("warn", "SSR head"),
  UNSCOPED_HOLE_ALLOCATED_IDS: outOfScope("warn", "SSR"),
  LOWERCASE_EVENT_ATTRIBUTE: outOfScope("warn", "host attribute"),
  UNRECOGNIZED_INSERT_VALUE: outOfScope("warn", "host insert"),
  // The typings reject a bad target, so tsc reports it (invariant 1): the checker stays silent.
  INVALID_REFRESH_TARGET: outOfScope("error", "typed away"),
  INVALID_AFFECTS_TARGET: outOfScope("error", "typed away"),
  DYNAMIC_ASYNC_COMPONENT: outOfScope("error", "SSR"),
  LATE_HEADER_WRITE: outOfScope("error", "SSR"),
  LAZY_ASSET_UNMAPPED: outOfScope("warn", "SSR"),
  PRELOAD_DESCRIPTOR_INVALID: outOfScope("warn", "SSR"),
  REVEAL_IN_RENDER_TO_STRING: outOfScope("warn", "SSR"),
  SERVER_ERROR_SANITIZED: outOfScope("info", "SSR"),
  SERVER_WRITE: outOfScope("warn", "SSR"),
  SSR_BOUNDARY_WATERFALL: outOfScope("warn", "SSR"),
  SSR_CLIENT_CONTENT_MASKED: outOfScope("warn", "SSR"),
  SSR_LIVE_HOLE_LATCHED: outOfScope("warn", "SSR"),
  SSR_RENDER_ERROR_CONTAINED: outOfScope("error", "SSR"),
  SSR_STREAM_ABANDONED: outOfScope("warn", "SSR"),
  SSR_SUBTREE_ABANDONED: outOfScope("warn", "SSR"),
  SSR_UNDECLARED_LIVE_SOURCE: outOfScope("warn", "SSR"),
};

/** The flat code -> rules table of scripts/runtime-oracle.mjs (`codeRules`), derived from the policy; codes.test.mjs compares the two. */
export const codeRules = Object.fromEntries(Object.entries(codePolicy).filter(([, policy]) => policy.rules.length).map(([code, policy]) => [code, policy.rules]));

/** The policy of a code, or null when the table does not know it (a harness bug the drift test is there to prevent). */
export const policyOf = (code) => codePolicy[code] ?? null;
/** A diagnostic of this code, with no finding to cover it, is a false clean: Solid reported a defect the checker did not. */
export const isDefectCode = (code) => ["defect-claimed", "defect-unclaimed"].includes(policyOf(code)?.disposition);
export const severityRank = (code) => ({ error: 0, warn: 1, info: 2 })[policyOf(code)?.severity] ?? 3;

export const thrownCodes = [
  [/^Cannot call resolve inside a reactive scope/, "RESOLVE_IN_REACTIVE_SCOPE"],
  [/^Cannot call until inside a reactive scope/, "UNTIL_IN_REACTIVE_SCOPE"],
  [/^\[FLUSH_IN_ACTION\]/, "FLUSH_IN_ACTION"],
  [/^Cannot call flush\(\) from inside onSettled or createTrackedEffect/, "FLUSH_IN_TRACKED_QUEUE_CALLBACK"],
  [/^dynamic\(\): a static source must resolve synchronously/, "DYNAMIC_STATIC_PROMISE"],
  [/^Context can only be accessed under a reactive root/, "NO_OWNER_CONTEXT"],
  [/^Context must either be created with a default value/, "CONTEXT_NOT_FOUND"],
  [/^Context(?: "[^"]*")? (?:has no provider above this read|was provided with `undefined`)/, "CONTEXT_NOT_FOUND"],
];

/** Codes that are an engine's consequence of an earlier failure, not a defect of their own: they never count as "a diagnostic at the finding". */
export const consequenceCodes = Object.entries(codePolicy).filter(([, policy]) => policy.disposition === "consequence").map(([code]) => code);

/**
 * Rules that name the same kind of defect under different ids: a finding of one and a diagnostic claimed by another
 * are one real defect named wrongly (`candidate-wrong-rule`), not two defects. Only listed families count.
 * Owner calls: no owner (SC3001) versus a call the host forbids (SC4001) versus an apply-phase primitive.
 */
export const defectFamilies = [["missing-owner", "leaf-owner-forbidden-call", "primitive-in-directive-application"]];
export const sameDefectFamily = (a, b) => defectFamilies.some((family) => family.includes(a) && family.includes(b));

/**
 * Defect codes Solid reports from the scheduler, not from the offending call: the stack is the flush's, so no authored line belongs to
 * the diagnostic (Chrome lists it as unattributed; the fast path's frames are the mount's, not the read's). They are judged by file,
 * as the matrix does (`placeless` in misuse.mjs), never by line. `ASYNC_OUTSIDE_LOADING_BOUNDARY` is reported "located at the first
 * pending effect's owner path" once per mount. Every other claimed code is reported at its call with the caller's stack
 * (checked in the pinned `@solidjs/signals` dev dist); `LOADING_ON_OUTSIDE_HOLD` is flush-time too but no rule claims it, so a line
 * never decided its verdict.
 */
export const placelessCodes = ["ASYNC_OUTSIDE_LOADING_BOUNDARY"];
export const isPlacelessCode = (code) => placelessCodes.includes(code);

/** Findings of these rules cover any runtime diagnostic at their site (the checker withheld an answer). */
export const withholdingRules = ["reactive-dispatch-unresolved"];
/** Violations the oracle never holds against the runtime (style rules with no dev diagnostic). */
export const ignoredRules = ["prefer-show", "prefer-for"];

const claimedRules = new Set(Object.values(codeRules).flat());

/** True when some runtime diagnostic can contradict or confirm a violation of `rule`. */
export const ruleHasRuntimeCounterpart = (rule) => claimedRules.has(rule);

/** The codes whose diagnostics substantiate a violation of `rule`. */
export const codesForRule = (rule) => Object.entries(codeRules).filter(([, rules]) => rules.includes(rule)).map(([code]) => code);

/** The code of a thrown error message (an uncaught dev-build error), or null. */
export function codeOfThrown(message) {
  const text = String(message ?? "");
  return thrownCodes.find(([pattern]) => pattern.test(text))?.[1] ?? null;
}
