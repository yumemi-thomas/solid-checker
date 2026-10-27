// The Solid dev diagnostic codes and the checker rules that claim them, and the
// dev-build errors thrown with no code. This is a copy of the tables in
// scripts/runtime-oracle.mjs (`codeRules`, `thrownCodes`, `withholdingRules`):
// that file is a script with top-level effects and cannot be imported, so
// codes.test.mjs reads it and fails when the two drift.

export const codeRules = {
  STRICT_READ_UNTRACKED: ["strict-read-untracked", "no-destructure", "reactive-handler-frozen", "components-return-once"],
  NO_OWNER_CLEANUP: ["missing-owner"],
  NO_OWNER_EFFECT: ["missing-owner", "primitive-in-directive-application"],
  NO_OWNER_BOUNDARY: ["missing-owner"],
  NO_OWNER_CONTEXT: ["missing-owner"],
  CONTEXT_NOT_FOUND: ["missing-context-provider"],
  SETTLED_CLEANUP_UNOWNED: ["missing-owner"],
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["no-write-in-pure-computation"],
  // A write through a store's read proxy is ignored with this warning (`@solidjs/signals` rc.17 dev.js, `warnWriteOutsideSetter`); SC2003.
  STORE_WRITE_OUTSIDE_SETTER: ["no-store-mutation-outside-setter"],
  CLEANUP_IN_FORBIDDEN_SCOPE: ["leaf-owner-forbidden-call"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["leaf-owner-forbidden-call"],
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["async-outside-loading-boundary"],
  PENDING_ASYNC_UNTRACKED_READ: ["pending-async-unsuspendable-read"],
  SYNC_NODE_RECEIVED_ASYNC: ["sync-computation-received-async"],
  UNTRACKED_READ_AFTER_AWAIT: ["reactive-read-after-await"],
  MISSING_EFFECT_FN: ["missing-effect-function"],
  ACTION_CALLED_IN_OWNED_SCOPE: ["action-called-in-owned-scope"],
  RESOLVE_IN_REACTIVE_SCOPE: ["no-await-primitive-in-tracked-scope"],
  UNTIL_IN_REACTIVE_SCOPE: ["no-await-primitive-in-tracked-scope"],
  FLUSH_IN_ACTION: ["flush-in-action"],
  FLUSH_IN_EFFECT_CALLBACK: ["flush-in-effect-callback"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["leaf-owner-forbidden-call"],
  DYNAMIC_STATIC_PROMISE: ["static-dynamic-async-source"],
};

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
