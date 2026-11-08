// The Solid dev diagnostic codes, the checker rules that claim them, and the dev-build errors thrown with no
// code. `codeRules`, `thrownCodes` and `withholdingRules` copy the tables in scripts/runtime-oracle.mjs: that file
// is a script with top-level effects and cannot be imported, so codes.test.mjs reads it and fails when the two
// drift. `codeRules` is derived from the policy table below.

// --- The policy table (phase D) -------------------------------------------------------------------------------
// Every rc.21 dev diagnostic code, classified once: which checker rules claim it, what a diagnostic of it means for
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

// `declining`: rules whose *uncertifiable* finding at the site is the checker declining this code without claiming it (a violation of
// one is still a wrong rule): SC4001's obligation on a `useContext` read is SC4002's provider question left unasked, since the provider
// proof is gated on an owner proof (docs/rules/missing-context-provider.md).
const claimed = (rules, severity, { declining } = {}) => ({ rules, disposition: "defect-claimed", severity, ...(declining ? { declining } : {}) });
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
  CONTEXT_NOT_FOUND: claimed(["missing-context-provider"], "error", { declining: ["missing-owner"] }),
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
  // ADR 0517: SC5006 records an obligation for a `<Loading on>` whose hold it cannot prove its own; it never claims a violation,
  // so a runtime diagnostic it covers is a recall gap, never agree.
  LOADING_ON_OUTSIDE_HOLD: claimed(["loading-on-outside-hold"], "warn"),
  // New on rc.21. `<Loading on={fn}>` (SC5007) and `onFlush` under a children-capable owner (SC3002).
  LOADING_ON_ACCESSOR: claimed(["loading-on-accessor"], "warn"),
  ON_FLUSH_IN_OWNED_SCOPE: claimed(["on-flush-in-owned-scope"], "warn"),
  // SC2009 and SC5008 (ADRs 0636, 0637): each proven in the pass that creates the source.
  OPTIMISTIC_WRITE_BEFORE_LOAD: claimed(["optimistic-write-before-load"], "error"),
  CAUGHT_REACTIVE_READ: claimed(["caught-reactive-read"], "warn"),
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: claimed(["leaf-owner-forbidden-call"], "error"),
  DYNAMIC_STATIC_PROMISE: claimed(["static-dynamic-async-source"], "error"),

  // Defects no rule claims.
  PRIMITIVE_IN_EFFECT_CALLBACK: claimed(["primitive-in-effect-callback"], "error"),
  RUN_WITH_DISPOSED_OWNER: unclaimed("warn", "decided not to build: docs/open-work.md, `runWithOwner` with a disposed owner"),
  // Invariant 1: the setter callback is `(state) => void | T`, so an async one is TS2345 and tsc reports it.
  ASYNC_STORE_SETTER: { rules: [], disposition: "out-of-scope", severity: "error", note: "typed away: the published types reject an async setter callback" },
  UNCOMMITTED_INPUT_OVERWRITTEN: unclaimed("warn", "decided not to build: docs/open-work.md, `UNCOMMITTED_INPUT_OVERWRITTEN`"),

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
  ASYNC_MEMO_ELEMENT: outOfScope("error", "SSR"),
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
 * (checked in the pinned `@solidjs/signals` dev dist). `LOADING_ON_OUTSIDE_HOLD` is reported from the boundary's notification in the
 * flush too (ADR 0517). `SETTLED_CLEANUP_UNOWNED` is reported by the one-shot's `fire` after the callback returned: 642 of 642 smoke rows
 * that fired it had no authored position (ADR 0815).
 */
export const placelessCodes = ["ASYNC_OUTSIDE_LOADING_BOUNDARY", "LOADING_ON_OUTSIDE_HOLD", "LOADING_ON_ACCESSOR", "CAUGHT_REACTIVE_READ", "SETTLED_CLEANUP_UNOWNED"];
export const isPlacelessCode = (code) => placelessCodes.includes(code);

/**
 * Findings of these rules cover a runtime diagnostic at its recorded position only (the checker withheld an answer there; review 4c,
 * R5): `withheldAtStack`. A withholding elsewhere withheld some other answer. A placeless code has no position, so none covers it.
 */
export const withholdingRules = ["reactive-dispatch-unresolved"];
/** Codes about a value a call was handed, which code with no authored frame then read or wrote: a withholding at that argument covers them (R20). */
export const handedCodes = ["STRICT_READ_UNTRACKED", "UNTRACKED_READ_AFTER_AWAIT", "PENDING_ASYNC_UNTRACKED_READ", "STORE_WRITE_OUTSIDE_SETTER"];
/** The flat code -> declining rules table (the policy's `declining`), mirrored as `decliningRules` in scripts/runtime-oracle.mjs. */
export const decliningRules = Object.fromEntries(Object.entries(codePolicy).filter(([, policy]) => policy.declining?.length).map(([code, policy]) => [code, policy.declining]));
/**
 * Whether `finding` covers a diagnostic of `code` (review 4c, R5, R8): a finding of a rule that claims it when `atSite` (the caller found
 * it at the diagnostic's recorded position, `withheldAtStack`), anywhere for a placeless code (it has no position); an uncertifiable
 * finding of a rule that declines it, on the same terms (R21); or a withholding when `atSite`, never for a placeless code. Only a claiming rule's violation
 * answers the code; this is the "the checker said something" test.
 */
export const coversCodeAt = (code, finding, atSite) => ((codeRules[code] ?? []).includes(finding.rule) ? isPlacelessCode(code) || atSite === true
  : (declinesCode(code, finding) && (isPlacelessCode(code) || atSite === true)) || (withholdingRules.includes(finding.rule) && atSite === true && !isPlacelessCode(code)));

/**
 * Whether a withholding `finding` (`line`, `column`, 1-based: its start) is at a position of `stack` (innermost first: `{ line, column }`,
 * a source-mapped stack frame): the top frame's position (the read, write or throw), or a deeper frame's exact call position (the call
 * that leads to it) when `mount(frame)` says it mounts nothing (call-starts.mjs `mount`: every read below a mount runs through it, so a withholding there covers nothing deeper). Positions, never line ranges: a multi-line finding covers nothing
 * inside it. A frame with no column matches nothing, except a hand-read `closedAt` line (`handRead`, matched by line). A frame's
 * `starts` (`withStarts`) are the starts it stands for, its own position first.
 */
/**
 * Whether `finding` at a handed argument `at` covers `code`: a withholding on a code about a handed value. A store write's target is the
 * receiver of a member call (`list.push(x)` says nothing about `x`, R22) and a project or standard function's first argument, so no other
 * argument counts, and no argument of a dependency's function: a judge does not assume what an unread body writes. Known limit: the judge
 * cannot tell which argument a read went through (`Object.assign(out, st, holder.v)`).
 */
const handedCovers = (at, finding, code) => handedCodes.includes(code) && withholdingRules.includes(finding.rule)
  && (code !== "STORE_WRITE_OUTSIDE_SETTER" || (!at.noTarget && at.index === 0));

export const withheldAtStack = (finding, stack, mount = () => true, code = null) => (stack ?? []).some((frame, depth) =>
  (frame.handRead ? finding.line === frame.line
    : Number.isInteger(frame.column) && (frame.starts ?? [frame]).some((at) => finding.line === at.line && finding.column === at.column
      && (!at.handed || handedCovers(at, finding, code))))
  && (depth === 0 || !mount(frame)));

/**
 * The stack of a diagnostic's positions, innermost first, with another file's frames kept but blank: they place nothing, and depth stays
 * the depth in the whole stack, so the top authored frame is the real one (R20) and a deeper App.tsx frame still takes the mount test.
 */
export const appStack = (positions) => (positions ?? []).map((position) => (position.file === "App.tsx" || position.file === undefined ? position : { file: position.file, line: -1 }));

/**
 * A stack's frames with `starts` (call-starts.mjs `callStarts`): a frame in the callee of a call, `new`, tagged template or JSX tag also
 * stands for that expression's start and its callee object's (V8 puts a member call's frame at the property). `startsOf(frame)` gives
 * the starts for a frame (at its depth) of the program's text, or null when the frame's file has none.
 */
export const withStarts = (stack, startsOf) => (stack ?? []).map((frame, depth) => {
  if (!Number.isInteger(frame.column) || !startsOf) return frame;
  const starts = startsOf(frame, depth);
  return starts ? { ...frame, starts } : frame;
});

/** Whether `finding` is an uncertifiable finding of a rule that declines `code` (the policy's `declining`). */
export const declinesCode = (code, finding) => finding.kind === "uncertifiable" && (decliningRules[code] ?? []).includes(finding.rule);
/** Violations the oracle never holds against the runtime (style rules with no dev diagnostic). */
export const ignoredRules = ["prefer-show", "prefer-for"];

const claimedRules = new Set(Object.values(codeRules).flat());

/** True when some runtime diagnostic can contradict or confirm a violation of `rule`. */
export const ruleHasRuntimeCounterpart = (rule) => claimedRules.has(rule) && !partlySilentRules.includes(rule);

/** The codes whose diagnostics substantiate a violation of `rule`. */
export const codesForRule = (rule) => Object.entries(codeRules).filter(([, rules]) => rules.includes(rule)).map(([code]) => code);

/** The code of a thrown error message (an uncaught dev-build error), or null. */
export function codeOfThrown(message) {
  const text = String(message ?? "");
  return thrownCodes.find(([pattern]) => pattern.test(text))?.[1] ?? null;
}

/**
 * A logged or uncaught error's text without its class prefix: `Error: `, and Solid's own classes (`NoOwnerError: `, `ContextNotFoundError: `),
 * which a console.error of the error object prints (R3e, Part 16: the `Error: `-only strip dropped those reports).
 */
export const stripErrorClass = (text) => String(text ?? "").replace(/^[A-Za-z]*Error: /, "");
/** The code of a logged or reported error's text (`codeOfThrown` after `stripErrorClass`), or null. */
export const codeOfLogged = (text) => codeOfThrown(stripErrorClass(text));

/**
 * ADR 0858: the Solid code an uncaught throw's message carries, or null: a `[CODE]` tag naming a code of the policy table, or a message
 * `thrownCodes` knows. Such a throw is the program's own runtime result; any other failure is the harness's.
 */
export function codeOfUncaught(message) {
  const text = stripErrorClass(message);
  const tag = /^\[([A-Z][A-Z_]+)\]/.exec(text)?.[1];
  return tag && codePolicy[tag] ? tag : codeOfThrown(text);
}

/**
 * Rules whose violation stands beside a code as a claim of its own, so it never names that code wrongly: an effect created in an apply throws
 * PRIMITIVE_IN_EFFECT_CALLBACK and is an unowned effect too, the leak a production build keeps (ADR 0483).
 */
export const besideCode = {
  // ADR 0696, owner: a destructure over a pending prop is a true read, and Chrome warns under the pending code.
  PENDING_ASYNC_UNTRACKED_READ: ["no-destructure"],
  PRIMITIVE_IN_EFFECT_CALLBACK: ["missing-owner"],
  // ADR 0641: masked, not reported. In these programs Chrome reports only the row's code, because the first throw stops the run; each
  // twin with that cause removed reports the other rule's code. `createEffect(fn, undefined)`, given an apply: PRIMITIVE_IN_EFFECT_CALLBACK
  // in an apply or a reaction, PRIMITIVE_IN_FORBIDDEN_SCOPE in a leaf. A helper shared with a tracked effect, the effect removed:
  // NO_OWNER_CLEANUP and NO_OWNER_EFFECT, three times each.
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["missing-owner"], CLEANUP_IN_FORBIDDEN_SCOPE: ["missing-owner"],
  MISSING_EFFECT_FN: ["primitive-in-effect-callback", "leaf-owner-forbidden-call"],
};
/**
 * Rules whose defect the runtime names for only some of what they claim, so its silence contradicts nothing. `primitive-in-directive-application`:
 * a `ref` apply has no owner (`onCleanup` there reports NO_OWNER_CLEANUP), an effect created there warns NO_OWNER_EFFECT and a memo, the same leak, warns
 * nothing (rc.17, measured 2026-11-04).
 */
export const partlySilentRules = ["primitive-in-directive-application"];
/** Outcomes that need a look: a misuse grid's and a legal grid's. */
export const PROBLEMS = ["false-clean", "misplaced", "unplaced", "wrong-rule", "harness", "unjudged-claim", "halted-claim", "stacked-claim", "false-positive", "not-legal", "tsc-overlap"];
/**
 * What a grid row needs a look for, or null: its outcome, or a second code on the site's lines missed in a file with no uncertifiable
 * finding (matrix.mjs `judge`, `secondary`), a false clean of that code whatever the row's own outcome. The one test of focus, the audit and smoke.
 */
export const rowProblem = (row) => (PROBLEMS.includes(row.outcome) ? row.outcome : Object.values(row.detail?.secondary ?? {}).includes("false-clean-certified") ? "secondary-false-clean" : null);
