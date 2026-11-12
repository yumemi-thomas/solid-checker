// The boundary registry (ADR 0935): for every item of the checker that can make a site silent, the premise it rests on, and a pair of programs
// that differ only in the fact that premise claims.
//
// Why. The proof log (ADR 0933) fails a clean that no named end explains; it cannot fail a WRONG proof: an arm that answers "silent" through
// a premise that is false for some legal program. ADR 0927's four arms and ADR 0932's two were each reachable by legal programs, and what no
// grid held was one program where one premise failed (`Set.add` shared a hand-off class with `addEventListener` and was never exercised alone;
// a handler literal in a `<For>` row was "off the row's stack" with a synchronous dispatch in the row). So a silence is covered when the
// registry holds, for the item that gives it, a PAIR: a misuse program and a legal one, the same but for the premise's fact.
//
// What is covered (derived from source by boundary-inventory.mjs, so the Rust cannot grow past the test; boundary.test.mjs holds the line):
//   arms        the tagged classifier arms (`role_arm!`) that can answer a silent role: a literal silent role, or a computed (transported) one.
//               Not these, each stated in the test: `Unknown` arms (an obligation, never a clean), reporting arms (a wrong answer is a false
//               positive, which the grids' own judge finds), owner arms (ADR 0886's `ARM_ENDINGS` maps each to an ending).
//   ends        the producers' named ends: the closed `SiteSilence` list (site_accounting.rs, ADR 0933, 0936)
//   kinds       the typed facts a silence rests on (`proof_log::PremiseKind`), and through `DropPass::premise_kinds()` the backend passes
//   tables      the rows of the checker's own tables that decide a role or a silence (TABLES in boundary-inventory.mjs)
//   dialect     the questions `impl Dialect for Solid2` answers (the dialect API tables)
//
// A PREMISE (below) states the fact, lists what rests on it (`arms`, `ends`, `kinds`, `tables`, `dialect`) and its evidence, one of:
//   pairs     generator contexts (PAIRS), per code a misuse side and a legal side
//   fixtures  a positive and a negative fixture under fixtures/ (a premise no generated program can express: a compiler refusal, a tsc rejection)
//   outside   a stated reason neither can express it
// Its `quality` is `exact` when the pair differs only in the fact (authored, or the table family's twins), `family` when the arm is only
// known to answer silent beside a reported sibling in the same placement family (a loose pair). `family` is the shrinking part.
//
// `boundary.mjs verify` runs every pair through the checker (boundary-scan.mjs) with the role log and checks the claims: the legal side silent,
// the misuse side not, and each listed arm answering at the legal side. `boundary.mjs report` prints the registry's state.
import { CALLBACK_CONTEXTS, CALLBACK_FORMS, CONTROL_LAYERS, FILE_LAYERS, LAYERS, SPEC } from "./misuse.mjs";
import { RECALL_MOUNTS } from "./mounts.mjs";
import { DEPTH_FORMS, CHAIN_FORMS, HANDOFF_FORMS } from "./model.mjs";
import { TABLE_CODES, TABLE_GAPS, TABLE_ROWS, tableContexts } from "./table-shapes.mjs";
import { SEAM_FORMS } from "./seam-shapes.mjs";

/** The side of a context for a code: `misuse`, `legal`, or null when the code lists it on neither. */
export function sideOf(code, context) {
  const spec = SPEC[code];
  if (!spec) return null;
  if ([...(spec.contexts ?? []), ...(spec.recallContexts ?? [])].includes(context)) return "misuse";
  if ([...(spec.legal ?? []), ...(spec.recallLegal ?? [])].includes(context)) return "legal";
  return null;
}

/** A pair resolved for its codes: `[{ code, misuse: [contexts], legal: [contexts] }]`, the codes that have both sides. */
export function resolvePair(pair) {
  return pair.codes
    .map((code) => ({ code, misuse: pair.contexts.filter((context) => sideOf(code, context) === "misuse"), legal: pair.contexts.filter((context) => sideOf(code, context) === "legal") }))
    .filter((entry) => entry.misuse.length && entry.legal.length);
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Pairs on contexts the generator already has. `contexts` is a list; each code splits it by its own lists (SPEC), so a context a code
// treats as legal (a read in a handler) lands on the legal side for that code.

const WINDOW_CODES = ["STRICT_READ_UNTRACKED", "REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE", "STORE_WRITE_OUTSIDE_SETTER"];

/** @type {Record<string, { fact: string, codes: string[], contexts: string[] }>} */
export const PAIRS = {
  // ADR 0927 (rows 1-4), ADR 0932 (S1, S2): authored with the fix, exact.
  "kept-collection": { fact: "the collection's function is run by the body (a window) or by a handler/untrack (none)", codes: WINDOW_CODES, contexts: ["kept-set-body", "kept-array-body", "kept-map-body", "kept-set-untrack", "kept-set-handler"] },
  "apply-void": { fact: "the apply calls its parameter inside `void` (`void v()`) or only names it (`void v`)", codes: WINDOW_CODES, contexts: ["apply-void-call-untrack", "apply-void-call-row", "apply-void-value-untrack", "apply-void-value-row"] },
  "flow-in-allowed-region": { oneSided: "the generator has no legal flow child for these codes: a flow child always runs in its own window, so only the misuse side exists (its twin is the same flow element outside the allowed region: `flow-*`, also misuse)", fact: "a flow element created in a closed `untrack` runs its child in its own window", codes: WINDOW_CODES, contexts: ["flow-untrack-for", "flow-untrack-for-unkeyed", "flow-untrack-show", "flow-untrack-show-keyed", "flow-untrack-match", "flow-untrack-repeat"] },
  "owned-write-updater": { fact: "`ownedWrite: true` skips the owned-scope guard, so the updater runs on the setter's stack", codes: WINDOW_CODES, contexts: ["owned-updater-body", "owned-updater-handler"] },
  "row-handler-dispatch": { fact: "a synchronous dispatch in the row runs the row's handler inside the row's window", codes: WINDOW_CODES, contexts: ["evt-row-dispatch", "evt-row-timer-dispatch"] },
  "event-dispatch": { fact: "the listener's event is dispatched in a window (body) or in a handler", codes: WINDOW_CODES, contexts: ["evt-body-dispatch", "evt-handler-dispatch", "evt-apply-dispatch"] },
  // The dispatching members (cleanup.rs:1498): the member's event delivered in a window (body) or in a handler, and the exactness twins (a project method or a
  // local function of the same spelling). The generator's `evt-<position>-fire-<fire>` contexts (misuse.mjs FIRES).
  ...Object.fromEntries(["focus", "blur", "select", "remove", "steal", "reset", "submit", "validity", "modal", "popover", "focus-wrapper", "focus-member"].map((fire) => [`fire-${fire}`,
    { fact: `the \`${fire}\` member's event is delivered in a window (body) or in a handler`, codes: WINDOW_CODES, contexts: [`evt-body-fire-${fire}`, `evt-handler-fire-${fire}`] }])),
  "fire-exactness": { fact: "`focus()` is the lib.dom member, a project method of the same name, or a local function of that name", codes: WINDOW_CODES, contexts: ["evt-body-fire-focus", "evt-body-fire-project", "evt-body-fire-shadow"] },
  // Reachability and callbacks: contexts that differ in whether the site runs (ADR 0458; ADR 0590; the callback-prop axis).
  "site-reach": { fact: "the same site live in the body, or in code nothing runs (`if (false)`, after a `return`, `false &&`, a component never rendered)", codes: WINDOW_CODES, contexts: ["body", "dead-if", "dead-after-return", "dead-and-false", "dead-comp-never"] },
  "deferred-apply-reruns": { fact: "a deferred effect apply whose compute reads nothing never runs, or its compute reruns and the apply with it", codes: WINDOW_CODES, contexts: ["effect-apply-defer", "effect-apply-defer-rerun"] },
  "callback-prop-call": { fact: "the component calls its callback prop in its body (a window), in a handler, or never", codes: WINDOW_CODES, contexts: ["cbp-body", "cbp-handler", "cbp-never", "cbp-attached"] },
  "primitive-contexts": { quality: "family", fact: "the site in the callback of a primitive the dialect places in a window, out of it, or in a body", codes: WINDOW_CODES,
    contexts: ["body", "memo", "effect-compute", "effect-apply", "render-compute", "render-apply", "tracked", "settled", "untrack", "root", "owner-null", "jsx", "handler", "timer", "then", "micro", "async"] },
  // ADR 0935: a user component wrapping `props.children` in a boundary (boundary-shapes.mjs), the children inside (real), beside it or foreign (misuse).
  "wrap-loading-call": { fact: "`createLoadingBoundary(() => props.children, ...)` holds the children, or they stand beside it (or a foreign `.children` is inside)", codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"], contexts: ["wrap-loadingcall-real", "wrap-loadingcall-outside", "wrap-loadingcall-shadow"] },
  "wrap-errored": { fact: "`<Errored fallback={(e) => ...}>` holds the children, or they stand beside it (or a foreign `.children` is inside)", codes: ["CONTEXT_NOT_FOUND"], contexts: ["wrap-errored-real", "wrap-errored-outside", "wrap-errored-shadow"] },
  "wrap-errored-call": { fact: "`createErrorBoundary(() => props.children, ...)` holds the children, or they stand beside it (or a foreign `.children` is inside)", codes: ["CONTEXT_NOT_FOUND"], contexts: ["wrap-erroredcall-real", "wrap-erroredcall-outside", "wrap-erroredcall-shadow"] },
  "wrap-provider": { fact: "`<Ctx value>` holds the children, or they stand beside it (or a foreign `.children` is inside)", codes: ["CONTEXT_NOT_FOUND"], contexts: ["wrap-provider-real", "wrap-provider-outside", "wrap-provider-shadow"] },
  // ADR 0935 (arm-shapes.mjs): the arms no fixture and no other pair of the registry reached (the arm ablation of the fixture projects listed them as unwitnessed).
  "generator-steps": { fact: "the generator's steps run in the body's window (a spread in the body), or out of it (the same spread in a click handler), or nothing steps it", codes: TABLE_CODES, contexts: ["barm-gen-spread", "barm-gen-handler", "barm-gen-never"] },
  "generator-loop-awaits": { fact: "the async function that drains a sync generator awaits between the steps, or does not", codes: ["REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"], contexts: ["barm-gen-drain", "barm-gen-await"] },
  "flow-climb-after-await": { fact: "the layer that holds the read runs in a flow row's body window, or after an await of its outer layer", codes: ["STRICT_READ_UNTRACKED"], contexts: ["barm-climb-sync", "barm-climb-await"] },
  // ADR 0935 (seam-shapes.mjs): `Number.isFinite` is the declared member (a value-only row), or a helper that takes the intrinsic as a parameter replaced it under a computed key.
  "seam-isfinite": { fact: "`Number.isFinite` ignores a function argument (the declared member), or a helper handed `Number` replaced it with one that runs the function at once", codes: TABLE_CODES,
    contexts: SEAM_FORMS.flatMap((form) => [`bseam-isfinite-${form}`, `bseam-isfinite-${form}-other`]) },
  // ADR 0939: members the runtime calls itself, wrapped so every other caller gets the original (the twin wraps a sibling).
  "seam-then": { fact: "`Promise.prototype.then` queues its callback (the declared member), or a helper handed `Promise.prototype` wrapped it to run the program's function at once", codes: TABLE_CODES,
    contexts: SEAM_FORMS.flatMap((form) => [`bseam-then-${form}`, `bseam-then-${form}-other`]) },
  "seam-settimeout": { fact: "`setTimeout` runs its callback on a later task (the declared global), or a helper handed `globalThis` wrapped it to run the program's function at once", codes: TABLE_CODES,
    contexts: SEAM_FORMS.flatMap((form) => [`bseam-settimeout-${form}`, `bseam-settimeout-${form}-other`]) },
  // ADR 0935 (ref-layer-shapes.mjs): a layer that runs in place inside an intrinsic `ref` callback, before an await of the callback (the component's window) or after it.
  "ref-layer-after-await": { fact: "the layer that holds the read runs in the `ref` callback before any await (in the element's creation window) or after one (a later task)", codes: ["STRICT_READ_UNTRACKED"], contexts: ["barm-ref-iife-sync", "barm-ref-iife-await", "barm-ref-each-sync", "barm-ref-each-await"] },
};

// ---------------------------------------------------------------------------------------------------------------------------------------
// The table family (table-shapes.mjs): one set of pairs per row, from the variants the row's kind has.

/** The twins of a kind: `[fact, misuse variant, legal variant]`. */
const TWINS = {
  inline: [
    ["the call sits in a window or in a handler", "b", "h"],
    ["the receiver has an element (the carrier provably runs the callback) or is empty", "b", "zero"],
    ["the call resolves to the standard method or to a project method of the same spelling that only keeps the function", "b", "norun"],
  ],
  fresh: [["the call resolves to the reviewed scheduler or to a project function of the same spelling that calls the callback at once", "shadow-b", "host-b"],
    ["as above, in a closed `untrack`", "shadow-u", "host-u"]],
  host: [
    ["a synchronous dispatch reaches the registered listener, or none does", "sync-b", "host-b"],
    ["the dispatch is in a window (body) or in a handler", "sync-u-b", "sync-u-h"],
    ["the call resolves to the standard registration or to a project function of the same spelling that calls the listener at once", "shadow-b", "host-b"],
  ],
  thenable: [["the thenable calls back at once (a window) or never", "b", "host-b"], ["the call sits in a window or in a handler", "b", "h"]],
  bind: [["the bound function is called (a window) or never", "sync-b", "host-b"], ["the bound function is called in a window or in a handler", "sync-u-b", "sync-u-h"]],
  kept: [
    ["the body runs the kept function or nothing does", "run-b", "host-b"],
    ["the kept function is run by the body or by a handler", "run-u-b", "run-u-h"],
    ["the call resolves to the standard collection method or to a project method of the same spelling that calls the function at once", "shadow-b", "host-b"],
  ],
  value: [
    ["the project also calls the function, or only hands it to the value-only API", "called-b", "host-b"],
    ["the call resolves to the standard API or to a project function of the same spelling that calls its argument", "shadow-b", "host-b"],
  ],
};

/** The table family's pairs: `{ id, row, kind, fact, codes, contexts: [misuse, legal] }`, a pair per twin the row has contexts for. */
export function tablePairs() {
  const have = new Set(tableContexts().map((entry) => entry.context));
  const slug = new Map(TABLE_ROWS.map((row) => [`${row.name}|${row.kind}`, row.slug]));
  const pairs = [];
  for (const row of TABLE_ROWS) {
    for (const [fact, misuse, legal] of TWINS[row.kind]) {
      const contexts = [misuse, legal].map((variant) => `tbl-${slug.get(`${row.name}|${row.kind}`)}-${variant}`);
      if (contexts.every((context) => have.has(context))) pairs.push({ id: `table:${row.name}${row.suffix ?? ""}:${misuse}/${legal}`, row: row.name, kind: row.kind, fact, codes: TABLE_CODES, contexts });
    }
  }
  return pairs;
}

/** The table rows (`<table>:<name>`) a table-family row stands for: the same qualified name in every table that lists it. */
export function rowTableKeys(rowName, tableRows) {
  return tableRows.filter((entry) => entry.name === rowName).map((entry) => `${entry.table}:${entry.name}`);
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// Premises. `arms`, `ends`, `kinds`, `tables`, `dialect` list what rests on the premise; the evidence is `pairs` (PAIRS ids), `tablePairs` (the
// table family's kinds, by row), `layerPairs` (a layer crossed with a misuse and a legal context: the hand-off, depth and chain layers),
// `fixtures` (directories under fixtures/ whose snapshot holds the silent and the reported side) or `outside` (a stated reason).

/** @type {Record<string, object>} */
export const PREMISES = {};
const premise = (id, spec) => { PREMISES[id] = { id, quality: "exact", arms: [], ends: [], kinds: [], tables: [], dialect: [], pairs: [], fixtures: [], ...spec }; };

// ---- the audits' premises (ADR 0927, 0932): the pairs were written with the fixes ---------------------------------------------------------
premise("collection-keeps-function", {
  claim: "A function put in a collection (`Set.add`, `Array.push`, `Map.set`, ...) is kept for project code, which may call it anywhere; only a host registration is run by the host alone.",
  source: "runtime_semantics.rs:526 `timing_behavior` (RetainedValue); local_access.rs:9656 `hand_off_at` (HandOff::Unfollowed)",
  arms: ["callee_roles.allowed-nested-host-kept"], pairs: ["kept-collection"], tablePairs: ["kept"], adr: "0927",
});
premise("apply-parameter-only-voided", {
  claim: "The apply's parameter occurs only as the whole operand of a `void` (never called): `void v` is never called, `void v()` calls it.",
  source: "callee_roles.rs:1999 `returned_to_idle_apply`", arms: ["callee_roles.allowed-nested-never-called"], pairs: ["apply-void"], adr: "0927",
});
premise("flow-element-has-its-own-window", {
  claim: "A flow element's child callback (`<For>` row, `<Show>` child, `<Match>`) runs in its own strict-read window wherever the element is created, an allowed region included.",
  source: "callee_roles.rs:723 (a row runs under `setStrictRead(\"<For>\")`); control_flow.rs; write_roles.rs:1000 `runs_off_callback_stack`",
  arms: ["callee_roles.allowed-nested-off-stack", "callee_roles.allowed-primitive-callback", "callee_roles.allowed-primitive-tracked", "callee_roles.allowed-primitive-flow",
    "callee_roles.control-flow", "callee_roles.control-flow-nested", "callee_roles.control-flow-climb", "callee_roles.control-flow-climb-after-await", "callee_roles.control-flow-tracked-region",
    "control_flow.control_flow_execution_role.deferred-callback", "control_flow.control_flow_execution_role.tracked-jsx", "control_flow.control_flow_execution_role.tracked-jsx-2", "flow_helpers.primitive_slot_role.tracked-jsx"],
  pairs: ["flow-in-allowed-region"], adr: "0927",
});
premise("handler-off-stack-without-dispatch", {
  claim: "A handler literal is off the row's stack only when nothing dispatches its event synchronously inside the row (a dispatch runs it on the dispatcher's stack, inside the window).",
  source: "write_roles.rs:1034 `is_event_handler_literal`, :1000 `runs_off_callback_stack`", arms: [], pairs: ["row-handler-dispatch"], adr: "0932",
});
premise("setter-updater-runs-on-the-setters-stack", {
  claim: "With `ownedWrite: true` the setter skips the owned-scope guard and runs its updater on the setter's own stack: a read in the updater is the setter call's.",
  source: "local_access.rs `setter_callback_role`; @solidjs/signals dev-shared.js `setSignal`", arms: [], pairs: ["owned-write-updater"], adr: "0932",
});

// ---- the table family (table-shapes.mjs): a row's premise, its synchronous path, its exactness twin ------------------------------------------
premise("listener-runs-on-dispatch", {
  claim: "A function registered as a listener runs only when an event is dispatched; a synchronous dispatch in the program runs it on the dispatcher's stack, inside whatever window made the call.",
  source: "runtime_semantics.rs:491 `runs_on_invoker_stack`; listener_reach.rs; cleanup.rs:1498 `synchronously_dispatching_dom_member`",
  arms: [], pairs: ["event-dispatch"], tablePairs: ["host", "thenable", "bind"], quality: "exact",
});
premise("scheduler-runs-on-a-fresh-stack", {
  claim: "A reviewed scheduler (timer, microtask, animation frame, observer, `Promise.then`) runs the callback from a task or microtask on an empty stack, so no caller's window or owner is current.",
  source: "runtime_semantics.rs:71 `FRESH_STACK_SCHEDULERS`", arms: ["callee_roles.fresh-stack-callback", "callee_roles.allowed-nested-fresh-stack", "callee_roles.inheriting-fresh-stack"],
  kinds: ["FreshStackScheduler"], tablePairs: ["fresh"],
});
premise("inline-carrier-runs-the-callback-in-place", {
  claim: "A standard-library carrier invokes its callback before it returns, on the caller's stack, and only on a non-empty literal receiver provably at least once.",
  source: "runtime_semantics.rs:526 `timing_behavior` (InlineCallback); carrier_runs.rs:60 `carrier_provably_runs`",
  arms: ["callee_roles.inline-callback", "callee_roles.named-callback-carried", "callee_roles.named-callback-in-place", "callee_roles.named-callback-called-in-place", "callee_roles.invocation-flow", "callee_roles.callee-use-site"],
  kinds: ["CarrierPlacesRead"], tablePairs: ["inline"],
});
premise("value-only-api-does-not-run-its-argument", {
  claim: "A standard-library API that only reads, copies or compares its argument (`Number.isFinite`, `Object.keys`, `Reflect.get`) never invokes a function argument; the project may still call it.",
  source: "runtime_semantics.rs:526 `timing_behavior` (ValueOnly); local_access.rs:9656 `hand_off_at` (HandOff::Value)", tablePairs: ["value"],
});

// ---- the arm-coverage shapes (arm-shapes.mjs, ref-layer-shapes.mjs): premises of the arms the fixtures left unwitnessed ----------------------------------
premise("generator-steps-run-where-stepped", {
  claim: "Calling a generator runs none of its body; each step runs the next segment on the stack of the code that makes it, so a site in the body takes the role at its steps, a generator nothing steps never runs, and a step the stepping async function makes after one of its suspensions has no owner.",
  source: "callee_roles.rs:203-255 (ADR 0851); write_roles.rs:2520 `generator_step_resumption`, :2556-2666", adr: "0935",
  arms: ["callee_roles.generator-steps", "callee_roles.generator-never-stepped", "write_roles.generator-steps-walked", "write_roles.generator-never-stepped", "write_roles.write_role_walk.unreported-generator"],
  pairs: ["generator-steps", "generator-loop-awaits"],
});
premise("flow-row-layer-after-an-await-leaves-the-window", {
  claim: "A layer that runs in place inside a flow row's body runs in the row's window only until the function that calls it suspends: past an await it runs on a later task, in no window.",
  source: "callee_roles.rs:741-790 `control-flow-climb-after-await`", adr: "0935",
  arms: ["callee_roles.control-flow-climb-after-await"], pairs: ["flow-climb-after-await"],
});
premise("ref-callback-layer-after-an-await-leaves-the-window", {
  claim: "A layer that runs in place inside an intrinsic `ref` callback runs in the element's creation window only until the callback suspends: past an await it runs on a later task, in no window.",
  source: "execution_role/body_roles.rs `layered_window_after_awaits` (ADR 0938); @solidjs/web dist/web.dev.js:1016-1022", adr: "0935",
  arms: ["body_roles.layered_window_after_awaits.unreported", "execution_role.ref_applied_read_role.unreported"], pairs: ["ref-layer-after-await"],
});

// ---- the seam shapes (seam-shapes.mjs): a premise two checks each leave to the other ------------------------------------------------------------------
premise("standard-member-is-the-declared-one", {
  claim: "A standard-library member the rows describe is the declared one: `Number.isFinite` never runs a function argument, and no program replaces it through a helper that takes the intrinsic as a parameter and writes it under an unspelled key.",
  source: "runtime_semantics.rs:526 `timing_behavior` (ValueOnly); accessor_provenance/mutation_sites.rs `handed_to_unseen_code` and `visible_iteration_write`; boundary-seams.mjs", adr: "0935",
  pairs: ["seam-isfinite", "seam-then", "seam-settimeout"],
});

// ---- boundary wrappers (boundary-shapes.mjs): the cover walks consult a user component's body, by the property's spelling ------------------------------
premise("boundary-wrapper-holds-the-children", {
  claim: "A user component whose body wraps `props.children` in a boundary (Loading, Errored, a context provider) covers, handles or provides what it is given; children beside the boundary, or a foreign object's `.children` inside it, are not covered.",
  source: "owners/async_context.rs:48 `loading_cover_at` (`children_in_loading`, by the property's spelling); stored_throw.rs:817 `handled_above`; owners/context_census.rs:640",
  pairs: ["wrap-loading-call", "wrap-errored", "wrap-errored-call", "wrap-provider"], adr: "0935",
  note: "The Loading JSX form is the proof-log agent's (branch claude/proof-log-phase-b, mounts.mjs WRAP_MOUNTS: wrap-real, wrap-alias, wrap-foreign, wrap-outside); not repeated here.",
});

// ---------------------------------------------------------------------------------------------------------------------------------------
// Layer pairs: the hand-off, depth, chain, control and file layers cross every recall context (matrix.mjs `recallShapesOf`). A layer pair is one layer
// written around the same site in a window (body) and out of it (a handler, an `untrack`): the layer is what carries the function to its run, so the
// arms that follow a function to the call that runs it (the climbs, the carriers, the call edges) answer in the legal cell, and the misuse cell must not.

/** @type {Record<string, { fact: string, codes: string[], contexts: string[], layers: string[] }>} */
export const LAYER_PAIRS = {
  "layers-base": { fact: "the site in a closure, a helper or an IIFE of the same file, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], layers: LAYERS.filter((layer) => layer !== "none") },
  "layers-hand-off": { fact: "the site in a function handed by name to a carrier, a wrapper, a member or an assignment, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], layers: HANDOFF_FORMS },
  "layers-depth-chain": { fact: "the site under N closures called in place, or reached through N named helpers, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], layers: [...DEPTH_FORMS, ...CHAIN_FORMS] },
  "layers-control": { fact: "the site in a loop, a `switch`, a `try`, a label or a generator, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], layers: CONTROL_LAYERS },
  // The recall axes that are not layers: the callback handed to a primitive by name (misuse.mjs CALLBACK_FORMS) and the mount that renders the component
  // (mounts.mjs). The same site, the same callback form or mount, in a window context and out of one.
  "callback-forms": { fact: "the callback is passed to the primitive by name (const, function, alias, named, let, bundle, shared), run in a window or out of it", codes: WINDOW_CODES, contexts: CALLBACK_CONTEXTS, callbacks: CALLBACK_FORMS },
  "mounts": { fact: "the component is rendered by a parent through a flow element, a boundary, a portal or `dynamic`, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], mounts: RECALL_MOUNTS },
  "layers-file": { fact: "the site in a function another module of the project exports, run in a window or out of it", codes: WINDOW_CODES, contexts: ["body", "handler", "untrack"], layers: FILE_LAYERS },
  // A callback passed to a primitive by name whose body hands a closure to an inline carrier (the hand-off layers): the closure runs in the callback's pass if at all
  // (callee_roles.rs `named-callback-carried`), a tracked pass for a compute and an untracked one for an apply. The callback axis crosses no layer in the grids.
  "named-callback-carries-closure": { fact: "the named callback hands a closure to an inline carrier, and the callback is a tracked compute (the read is tracked) or an untracked apply (the read warns)", codes: ["STRICT_READ_UNTRACKED"], contexts: ["memo", "tracked", "effect-apply", "render-apply"], callbacks: ["const", "named"], layers: ["handoff-foreach", "handoff-map"] },
};

/** Every pair of the registry: `{ id, quality, fact, codes, contexts, layers? }`. */
export function allPairs() {
  return [
    ...Object.entries(PAIRS).map(([id, pair]) => ({ id, quality: "exact", ...pair })),
    ...tablePairs().map((pair) => ({ ...pair, quality: "exact" })),
    ...Object.entries(LAYER_PAIRS).map(([id, pair]) => ({ id, quality: "family", ...pair })),
  ];
}

/** The cells of a pair: `{ pair, code, side, context, layer }` for every code that has both sides, each context on its side, each layer. */
export function pairCells(pair) {
  const cells = [];
  for (const resolved of resolvePair(pair)) {
    for (const side of ["misuse", "legal"]) {
      for (const context of resolved[side]) {
        for (const layer of pair.layers ?? (pair.callbacks || pair.mounts ? [undefined] : ["none"])) {
          if (pair.callbacks && pair.layers) for (const callback of pair.callbacks) cells.push({ pair: pair.id, code: resolved.code, side, context, layer, callback });
          else if (pair.callbacks) for (const callback of pair.callbacks) cells.push({ pair: pair.id, code: resolved.code, side, context, layer: "none", callback });
          else if (pair.mounts) for (const mount of pair.mounts) cells.push({ pair: pair.id, code: resolved.code, side, context, layer: "none", mount });
          else cells.push({ pair: pair.id, code: resolved.code, side, context, layer });
        }
      }
    }
  }
  return cells;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// verify: the registry against the checker. The cells of every pair run through the checker with the role log (and, with the ablation hook, each
// silent arm of a clean legal cell answering Unknown in turn). The result is the snapshot boundary.test.mjs reads: for each pair and code, what the
// misuse side and the legal side came to, and for each arm the legal cells it answered silent in (`decisive`: removing the arm made a finding appear).

export const WITNESSES = new URL("./boundary-witnesses.json", import.meta.url);
const WITNESS_LIMIT = 3;

/** The snapshot of scan results: `rows` are `{ pair, code, side, context, layer, outcome, programOutcome, decisions, decisive? }`. */
export function snapshotOf(rows, { checker } = {}) {
  const pairs = {};
  const arms = {};
  for (const row of rows) {
    const entry = ((pairs[row.pair] ??= { codes: {} }).codes[row.code] ??= { misuse: { programs: 0, clean: 0, violation: 0 }, legal: { programs: 0, clean: 0, violation: 0 } })[row.side];
    entry.programs += 1;
    if (row.programOutcome === "clean") entry.clean += 1;
    if (row.programOutcome === "violation") entry.violation += 1;
    if (row.side !== "legal" || row.programOutcome !== "clean") continue;
    const decisive = new Set((row.decisive ?? []).map((item) => item.arm));
    for (const arm of new Set(row.decisions.filter((decision) => decision.silent && decision.arm && !decision.default).map((decision) => decision.arm))) {
      const list = (arms[arm] ??= []);
      const witness = { pair: row.pair, code: row.code, context: row.context, ...(row.layer && row.layer !== "none" ? { layer: row.layer } : {}), ...(row.callback ? { callback: row.callback } : {}), ...(row.mount ? { mount: row.mount } : {}), ...(row.decisive ? { decisive: decisive.has(arm) } : {}) };
      const same = list.find((other) => other.pair === witness.pair && other.code === witness.code);
      if (same) { if (witness.decisive && !same.decisive) Object.assign(same, witness); continue; }
      list.push(witness);
    }
  }
  for (const list of Object.values(arms)) list.sort((left, right) => Number(Boolean(right.decisive)) - Number(Boolean(left.decisive)) || left.pair.localeCompare(right.pair) || left.code.localeCompare(right.code)), list.splice(WITNESS_LIMIT * 4);
  return { version: 1, checker: checker ?? null, pairs, arms: Object.fromEntries(Object.entries(arms).sort(([left], [right]) => left.localeCompare(right))) };
}

/**
 * The arms the snapshot covers: those with a witness in a pair whose misuse side, for that code, left no program clean (the pair separates the
 * premise: the legal cell is silent through the arm, and the misuse cell is not silent anywhere).
 */
export function coveredArms(snapshot) {
  const covered = new Map();
  for (const [arm, witnesses] of Object.entries(snapshot.arms ?? {})) {
    const separating = witnesses.filter((witness) => (snapshot.pairs?.[witness.pair]?.codes?.[witness.code]?.misuse?.clean ?? 1) === 0);
    if (separating.length) covered.set(arm, separating);
  }
  return covered;
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// The typed facts (proof_log.rs `PremiseKind`) and the named ends that rest on them. The evidence is a pair of generator contexts when the generator can
// write both programs, a fixture directory (reactive-ir/<name>: its snapshot holds the silent and the reported side) when only a project can, and a stated
// reason when neither can. `axes`: the grids cross it already (every grid crosses every code with the variants and the sources).

const fixtures = (...names) => names.map((name) => `reactive-ir/${name}`);
premise("kind-user-rule-option", { claim: "The user's rule options disable the rule (ADR 0440).", kinds: ["UserRuleOption"], outside: "a choice of the user's configuration, not a fact about the code: the backend process test (solid-facts-backend/tests/proof_log_process.rs) holds `retain_enabled`" });
premise("kind-dependency-source-root", { claim: "The finding's file is under a dependency source root; the package is not the project's code (ADR 0284).", kinds: ["DependencySourceRoot"], fixtures: fixtures("dependency-source", "dependency-source-owner", "dependency-source-program"), quality: "fixture" });
premise("kind-owning-finding-at-site", { claim: "Another enabled rule's finding stands at the same site and owns the claim.", kinds: ["OwningFindingAtSite"], outside: "judged by the grids' wrong-rule outcome (matrix.mjs): a violation under another rule's code at the site is `wrong-rule`, never a pass" });
premise("kind-violation-at-site", { claim: "A violation stands at the same site, so a pending-value obligation there is redundant (ADR 0822).", kinds: ["ViolationAtSite"], fixtures: fixtures("pending-read-accounted", "show-child-pending-read"), quality: "fixture" });
premise("kind-equal-finding", { claim: "An equal finding is already in the list.", kinds: ["EqualFinding"], outside: "deduplication of equal findings: no program fact (projection tests hold it)" });
premise("kind-suppression-comment", { claim: "An authored suppression comment names the rule at the site (ADR 0429).", kinds: ["SuppressionComment"], fixtures: fixtures("suppression-comments", "suppression-limits", "suppression-unknown-name"), quality: "fixture" });
premise("kind-site-reach", { claim: "The whole-program reach query proves no execution reaches the site (ADRs 0458, 0696): dead code, an unrendered component, an absent prop.", kinds: ["SiteReach", "SiteReachAbsentProp"],
  pairs: ["site-reach"], fixtures: fixtures("dead-code-sites", "absent-guard-prop"), ends: ["local_access.unreached_write.cannot-run"] });
premise("kind-instance-field", { claim: "The site is in an instance field initializer, which runs when an instance is constructed.", kinds: ["InstanceFieldInitializer"], fixtures: fixtures("instance-key-writes"), quality: "fixture" });
premise("kind-setter-guard-throws-first", { claim: "A resolved setter call whose write guard provably throws first (`ownedWrite` proven off, and the call's role reports a disallowed write) never runs its callback: a read in it is silent beside the setter call's own violation (ADR 0932).", kinds: ["SetterGuardThrowsFirst"],
  fixtures: fixtures("owned-write-updater-read"), ends: ["local_access.discover_call_as.setter-guard-throws-first"], quality: "fixture",
  note: "The twin is the `ownedWrite: true` setter (pair `owned-write-updater`, whose read is the setter call's). The silent read stands beside the violation, so no program of a generated pair is clean for it: the fixture holds it." });
premise("kind-undecided-component", { claim: "The site is in a function that returns JSX and is also handed to code that may call it as a plain function.", kinds: ["UndecidedComponent"], fixtures: fixtures("undecided-flush-callback", "undecided-flush-closure"), quality: "fixture" });
premise("kind-binding-before-initialization", { claim: "The site names a binding that may not be initialized there (ADR 0464).", kinds: ["BindingBeforeInitialization"], fixtures: fixtures("destructured-binding-tdz", "named-callback-tdz"), quality: "fixture" });
premise("kind-compiler-refused-file", { claim: "The compiler refused the file, so it has no compiler facts (ADR 0360).", kinds: ["CompilerRefusedFile"], fixtures: fixtures("compiler-refused-file", "compiler-refused-dependents"), quality: "fixture" });
premise("kind-client-runtime-reach", { claim: "The rule claims a client runtime window and the program's execution paths decide whether the site is in one (ADR 0659).", kinds: ["ClientRuntimeReach"], fixtures: fixtures("ssr-client-boundary", "ssr-client-boundary-csr"), quality: "fixture" });
premise("kind-runtime-configuration", { claim: "The runtime configuration is not the standard one (ADRs 0266, 0268).", kinds: ["RuntimeConfiguration"], fixtures: fixtures("standard-runtime-configuration"), quality: "fixture" });
premise("kind-apply-never-runs", { claim: "An effect's apply is proven never to run: an exact `defer: true` and a compute that reads nothing (ADR 0590).", kinds: ["ApplyNeverRuns"], pairs: ["deferred-apply-reruns"], ends: ["effect_execution.gate_diagnostics.apply-never-runs"], fixtures: fixtures("deferred-effect-apply") });
premise("kind-closed-program-callers", { claim: "A closed program: every way into the helper is a call expression the graph resolves; an open one has callers nobody sees (ADR 0203).", kinds: ["ClosedProgramCallers"],
  fixtures: fixtures("closed-export-member-dispatch", "closed-export-member-dispatch-open"), ends: ["local_access.project_helper_prop_read.calls-accounted", "shared_reactivity.push_parameter_store_flow.callers-pass-no-store", "attribution.discharge_closed_program_export_dispatch.closed-callers"], quality: "fixture" });
premise("kind-argument-primitive", { claim: "The argument at the site holds a primitive on every run, followed back through at most four functions or the tags that render the component (ADRs 0213, 0214, 0320): it carries no project member, so a use of the parameter nothing resolves reaches none. Closed program only.", kinds: ["ArgumentPrimitive"],
  fixtures: fixtures("closed-export-argument-primitive", "closed-export-argument-primitive-object", "closed-export-argument-primitive-open"), ends: ["attribution.discharge_closed_program_export_dispatch.argument-primitive"], quality: "fixture",
  note: "The twins are the open program (no `package.json`: three SC9012 rows stand) and the closed program where a second tag passes an object (`props.text` is no longer a primitive on every run: two rows stand)." });
premise("kind-argument-member-primitive", { claim: "The member a callee invokes on the argument is not the project's: the argument is a built-in value, a parameter or a prop that holds one on every entry (ADRs 0211-0214), or a literal whose own member is a primitive. Closed program only.", kinds: ["ArgumentMemberPrimitive"],
  fixtures: fixtures("parameter-pass-through-dispatch", "prop-pass-through-dispatch"), ends: ["attribution.discharge_closed_program_export_dispatch.argument-member-primitive"], quality: "fixture" });
premise("kind-props-callers-static", { claim: "Every call site of the component passes the prop a static value, so the property read is not a reactive read (the props index).", kinds: ["PropsCallersStatic"], fixtures: fixtures("props-callers", "component-function-child", "prop-body-call-guard"),
  ends: ["local_access.discover_member.props-static", "shared_reactivity.push_unproven_container_write.props-static"], quality: "fixture" });
premise("kind-prop-views", { claim: "A `merge` view serves the key from a plain source, or an `omit` view hides the key the read names.", kinds: ["MergeViewServesPlain", "ViewHidesKey"], fixtures: fixtures("rc21-merge-landing", "omit-view-reads"),
  ends: ["local_access.discover_member.merge-view-plain", "local_access.discover_member.view-hides-key"], quality: "fixture" });
premise("kind-tracked-position", { claim: "The read sits in a named function at a tracked position (an effect's compute or another place the compiler and the dialect track).", kinds: ["TrackedPosition"], pairs: ["primitive-contexts"],
  ends: ["local_access.unreached_read.named-at-tracked-position", "local_access.discover_call_as.named-at-tracked-position", "local_access.discover_member.named-at-tracked-position"], quality: "family" });
premise("kind-call-edge-places", { claim: "An exact call edge of the call graph enters the function that holds the read: the summaries attribute its reads at the call, where the call's own role decides.", kinds: ["CallEdgePlaces"],
  pairs: ["layers-base", "layers-hand-off", "layers-depth-chain", "layers-control", "layers-file"], ends: ["local_access.unreached_read.placed-elsewhere", "local_access.discover_member.placed-elsewhere", "local_access.discover_call_as.placed-elsewhere", "local_access.discover_call_as.handed-silent"], quality: "family" });
premise("kind-render-enters-component", { claim: "The function that holds the read is a proven component, so its render enters it.", kinds: ["RenderEntersComponent"], pairs: ["primitive-contexts"], quality: "family" });
premise("kind-store-write-standing", { claim: "The write has an exempt standing (the store's own setter writing its draft, a raw value), is tsc's to report, or lands on a shallow root or a mutable proxy.", kinds: ["StoreWriteExemptStanding", "ShallowStoreRoot"],
  fixtures: fixtures("store-mutators", "rc21-shallow-store-writes", "store-write-literal-links"),
  ends: ["shared_reactivity.direct_mutations.exempt-standing", "shared_reactivity.direct_mutations.shallow-store-root",
    "shared_reactivity.push_parameter_store_flow.shallow-store-root", "shared_reactivity.no_store_mutator_call.exempt-standing", "shared_reactivity.no_store_mutator_call.shallow-store-root", "shared_reactivity.no_store_delete.exempt-standing", "shared_reactivity.no_store_delete.shallow-store-root"], quality: "fixture" });
premise("kind-tsc-reports-the-site", { claim: "`tsc` reports a diagnostic at the site, so the claim is TypeScript's (invariant 1).", kinds: ["TscReportsAtSite"], fixtures: fixtures("tsc-suppressed-rejections"),
  ends: ["shared_reactivity.direct_mutations.tsc-rejects-root-write", "shared_reactivity.direct_mutations.tsc-nullish-link", "shared_reactivity.push_parameter_store_flow.tsc-nullish-link", "shared_reactivity.no_store_mutator_call.tsc-nullish-link",
    "shared_reactivity.no_store_delete.tsc-operand-assertion", "shared_reactivity.no_store_delete.tsc-rejects-delete", "shared_reactivity.no_store_delete.tsc-nullish-link"], note: "also fixtures/tsc-oracle (the ledger of cases tsc rejects, `removed-because-redundant` rows)", quality: "fixture" });
premise("kind-optimistic-setter-unguarded", { claim: "The dialect answers that an optimistic setter meets no owned-scope write guard.", kinds: ["OptimisticSetterUnguarded"], fixtures: fixtures("write-scope-optimistic-store-rc0", "write-scope-optimistic-store-rc3"),
  ends: ["local_access.push_setter_write.optimistic-store-unguarded", "local_access.push_setter_write.optimistic-signal-unguarded"], quality: "fixture" });
premise("kind-callee-not-array-method", { claim: "Type Facts resolves a mutator call's callee to a standard-library declaration that is not the `Array` method the rule judges (a typed array's `fill`).", kinds: ["CalleeNotArrayMethod"], fixtures: fixtures("store-mutators"),
  ends: ["shared_reactivity.no_store_mutator_call.not-array-method"], quality: "fixture" });
premise("kind-destructured-prop-value", { claim: "The written root is a binding of an object pattern over the props: it holds one prop's value, which the caller owns.", kinds: ["DestructuredPropValue"], fixtures: fixtures("fp-owner-destructured-prop-mutation"),
  ends: ["shared_reactivity.direct_mutations.destructured-prop-value"], quality: "fixture" });
premise("kind-module-scope", { claim: "The site is at module scope or in a static initializer: no function or instance field owns it (the module-initialization role).", kinds: ["ModuleScope"], fixtures: fixtures("event-dispatch-module-scope", "props-callers", "static-api"),
  ends: ["local_access.no_window.module-scope"], quality: "fixture",
  note: "A module-scope read is silent where the rc.21 runtime installs strict-read contexts only in component and effect bodies (ADR 0936): `props-callers` and `static-api` hold the read at module scope beside the component-body reads that report." });
premise("kind-ref-prop-call", { claim: "The read is a component's `ref` prop called once to publish its handle: not a reactive read (only an intrinsic element's `ref` is applied by the runtime).", kinds: ["RefPropCall"], ends: ["local_access.discover_member.ref-prop-call"], fixtures: fixtures("fp-owner-component-ref"), quality: "fixture" });
premise("kind-store-then-get", { claim: "A store `then` Get the thenable protocol makes exempt (ADR 0293).", kinds: ["StoreThenGetExempt"], ends: ["local_access.discover_member.store-then-exempt"], fixtures: fixtures("store-strict-read-keys"), quality: "fixture" });
premise("kind-compiler-discarded-region", { claim: "The compiler deleted the code: a template-root void child list is an explicit discarded region, a nested void child stays live and tracked.", kinds: ["CompilerDiscardedRegion"], fixtures: fixtures("jsx-void-child-divergence-solid-2"), quality: "fixture" });
// ---- ADR 0936: the closed accounting's kinds and ends --------------------------------------------------------------------------------------
premise("kind-after-await", { claim: "The site follows an `await` of its own async function that every path reaches, or that dominates it: it runs on a later task, off every caller's stack, in no render window and under no tracking scope (ADRs 0320, 0323).",
  kinds: ["AfterAwait"], ends: ["local_access.no_window.after-await"], fixtures: fixtures("reads-entered-unseen", "reads-entered-unseen-closed"), quality: "fixture",
  note: "`later()` reads after an unconditional await and stays silent in both fixtures, beside the exports around it that report (open program) or stay silent for another reason (closed one)." });
premise("kind-nothing-enters", { claim: "Nothing enters the function that holds the site: no call edge, carrier or scheduler names it, it is no possible component, no unseen code calls it (a closed program's export, a private function), and its binding is referenced in no file, so its code never runs (ADR 0936).",
  kinds: ["NothingEnters"], ends: ["local_access.no_window.nothing-enters"], fixtures: fixtures("reads-entered-unseen-closed", "reads-entered-unseen"), quality: "fixture",
  note: "The two fixtures are one pair: the same exports, the program boundary closed (silent: nothing calls them) and open (unseen importers may: SC1001 uncertifiable). A returned or stored closure is no such function: `render-time-reads` keeps its `finish` defaults uncertifiable (`strict-read-unaccounted`)." });
premise("kind-handed-to-silent-slot", { claim: "The function is handed whole to a position the compiler and the dialect place exactly and whose role reports nothing (an event handler slot, a tracked JSX expression): its reads run there (ADR 0936).",
  kinds: ["HandedToSilentSlot"], fixtures: fixtures("helper-prop-read"), quality: "fixture",
  note: "The end it rests on, `local_access.project_helper_prop_read.calls-accounted`, is listed under kind-closed-program-callers; helper-prop-read holds the silent slots (handler, tracked helpers) beside the helpers that report." });
premise("kind-plain-value-on-every-route", { claim: "A container a caller handed in holds a plain value on every route: a literal, a function or a `new` object, or a props declaration every call site of which passes one, so a write into it lands on a plain object (ADR 0919).",
  kinds: ["PlainValueOnEveryRoute"], ends: ["shared_reactivity.handed_container_write.plain-on-every-route"], fixtures: fixtures("store-handed-root"), quality: "fixture",
  note: "`PlainProp` (every site passes a closed plain literal) is silent in store-handed-root; the components beside it, whose sites pass a store, report." });
premise("kind-role-does-not-report", { claim: "The role classifier decided the site's role (the compiler's and the dialect's facts place the code) and that role reports nothing for the rule: an effect's apply, a leaf scope, a handler; the producer returned without a row (ADR 0936).",
  kinds: ["RoleDoesNotReport"], ends: ["local_access.discover_call_as.setter-callback-role-silent", "local_access.discover_member.role-does-not-report"], fixtures: fixtures("owned-write-updater-read", "prop-head-get"), quality: "fixture",
  outside: "the role is the classifier's own answer: every tagged arm that can answer silent has its pair or its stated gap in this registry (arms, ARM_GAPS), so a wrong role is that arm's boundary; these ends only record that a producer returned for a role the classifier decided" });
// ---- ADR 0943: the context census's silence names its fact ---------------------------------------------------------------------------------
premise("kind-context-default-proven", { claim: "The context was created with a default the syntax proves is not `undefined` (a literal, `null`, a function, object or array literal, or a `const` initialized with one), so a read never throws for want of a provider (ADRs 0575, 0943).",
  kinds: ["ContextDefaultProven"], fixtures: fixtures("context-default-undefined", "context-package-no-default"), quality: "fixture",
  note: "`DefaultReads` is silent in context-default-undefined beside the reads of default-less contexts that report. A package's context with no default is an obligation in context-package-no-default (it was silent before ADR 0943), so only a proven default is silent there too." });

/** What the family covers of the dispatch lists, by row name (cleanup.rs:1498, runtime_semantics.rs:925): the pair that holds the member's twins. */
export const DISPATCH_PAIRS = {
  "dispatching-members:focus": ["fire-focus", "fire-focus-wrapper", "fire-focus-member", "fire-steal", "fire-exactness"], "dispatching-members:blur": ["fire-blur"],
  "dispatching-members:checkValidity": ["fire-validity"], "dispatching-members:HTMLFormElement.reset": ["fire-reset"], "dispatching-members:HTMLFormElement.requestSubmit": ["fire-submit"],
  "dispatching-members:ChildNode.remove": ["fire-remove"], "dispatching-members:HTMLInputElement.select": ["fire-select"], "dispatching-members:HTMLElement.showPopover": ["fire-popover"],
  "dispatching-members:HTMLDialogElement.showModal": ["fire-modal"], "dispatching-members:HTMLDialogElement.close": ["fire-modal"],
  "dispatching-members:click": ["event-dispatch"], "dispatching-members:dispatchEvent": ["event-dispatch"],
  "delivers-synchronously:focus": ["fire-focus"], "delivers-synchronously:blur": ["fire-blur"], "delivers-synchronously:checkValidity": ["fire-validity"], "delivers-synchronously:reset": ["fire-reset"],
  "delivers-synchronously:requestSubmit": ["fire-submit"], "delivers-synchronously:remove": ["fire-remove"], "delivers-synchronously:select": ["fire-select"], "delivers-synchronously:showPopover": ["fire-popover"],
  "delivers-synchronously:showModal": ["fire-modal"], "delivers-synchronously:close": ["fire-modal"], "delivers-synchronously:click": ["event-dispatch"], "delivers-synchronously:dispatchEvent": ["event-dispatch"],
};

// ---------------------------------------------------------------------------------------------------------------------------------------
// The dialect API tables (solid_2.rs `impl Dialect for Solid2`, 133 questions): each is a table over (primitive, argument). Three kinds.
//   vocabulary  names, exports, versions, releases, router digests: no role or silence rests on them
//   source      whether a value is reactive (props, stores, accessors, tuples), so whether a read of it is one: the grids cross every code with the variants and the sources
//   role        where a callback runs, in which window, under which owner: the grids' primitive contexts (misuse.mjs `contexts`) are a cell per primitive slot

const names = (text) => text.split(/\s+/).filter(Boolean);
export const DIALECT_VOCABULARY = Object.fromEntries(names(`version modules primitive_defining_packages ecosystem_scopes primitive name_of release_owners router_params_are_url_strings audited_router_releases
  audited_router_digest audited_route_config_digest audited_route_components audited_installation review_installation variant_key variants variant_by_key defaulted_vocabulary models_server_functions
  declares_primitives_in export_modules namespace_import_primitives host_boolean_exports runtime_configuration_api component_prop_renderers intrinsic_tag_renderers boundary_name accessor_getter_length
  options_argument supports_sync_option lazy_option_argument plain_throw compute_throw_reports compute_throw_logged`).map((question) => [question, "vocabulary: names, exports, releases, options or error reporting; no role or silence rests on it"]));
export const DIALECT_SOURCES = Object.fromEntries(names(`props_require_caller_proof merges_props_reactivity splits_props store_key_warns_strict_read store_key_may_skip_strict_read returns_store reactive_result_slot
  creates_reactive_source accessor_yields_only_its_compute returns_reactive_tuple context_role hydration_host_member_read_is_harmless direct_jsx_return_is_component renders_component_argument
  renders_loaded_default_argument reports_member_reads_after_await store_root_properties_are_readonly store_setter_callback_enables_proxy_writes`).map((question) => [question, "source: whether a value is reactive; the grids' variants and sources cross every code"]));

premise("dialect-primitive-tables", {
  claim: "The dialect places each primitive's callback slots (where it runs, in which window, under which owner): a wrong answer moves a cell of the grid's primitive contexts.",
  source: "solid-dialect/src/solid_2.rs:361-3060", quality: "family", pairs: ["primitive-contexts"],
  note: "The questions not in DIALECT_VOCABULARY or DIALECT_SOURCES. The base grid is a cell per (code, primitive slot); no pair isolates one question, so this is family evidence, and a question whose table has no row in any context is the next shape.",
});
premise("dialect-source-tables", {
  claim: "The dialect says which values are reactive sources (props, stores, accessors, tuples, context): a wrong answer moves a cell of the variant and source axes.",
  source: "solid-dialect/src/solid_2.rs", quality: "family", axes: "misuse.mjs SOURCES, SOURCE_AXES and each code's variants (gridVariantsOf), crossed with every code by matrix.mjs --sources",
});

/** The dialect questions of the source, each with its kind and the premise that holds it. */
export function dialectClassification(questions) {
  return questions.map(({ id, line }) => ({ id, line, kind: DIALECT_VOCABULARY[id] ? "vocabulary" : DIALECT_SOURCES[id] ? "source" : "role", premise: DIALECT_VOCABULARY[id] ? null : DIALECT_SOURCES[id] ? "dialect-source-tables" : "dialect-primitive-tables" }));
}

/** Runs every pair's cells through the checker and returns the rows (`verify`). */
export async function runPairs(pairs, { bin, typefacts, install, work, concurrency = 4, ablate = false, only } = {}) {
  const { buildItems, pool, scanProgram, outcomeOf, programOutcomeOf } = await import("./boundary-scan.mjs");
  const cells = pairs.filter((pair) => !only || pair.id.includes(only)).flatMap((pair) => pairCells(pair));
  const unique = new Map();
  const keyOf = (cell) => `${cell.code}|${cell.side}|${cell.context}|${cell.layer}|${cell.callback ?? ""}|${cell.mount ?? ""}`;
  for (const cell of cells) unique.set(keyOf(cell), { code: cell.code, side: cell.side, context: cell.context, layer: cell.layer, ...(cell.callback ? { callback: cell.callback } : {}), ...(cell.mount ? { mount: cell.mount } : {}) });
  const items = buildItems([...unique.values()]);
  const results = await pool(items, concurrency, (item) => scanProgram(item, { bin, typefacts, install, work, ablate: ablate && item.side === "legal" }));
  const byCell = new Map();
  items.forEach((item, at) => {
    const key = keyOf(item);
    if (!byCell.has(key)) byCell.set(key, []);
    byCell.get(key).push({ id: item.built.id, scan: results[at] });
  });
  const rows = [];
  for (const cell of cells) {
    for (const { id, scan } of byCell.get(keyOf(cell)) ?? []) rows.push({ ...cell, id, outcome: outcomeOf(scan), programOutcome: programOutcomeOf(scan), program: scan.program, decisions: scan.decisions, decisive: scan.decisive });
  }
  return rows;
}

function option(argv, name, fallback) {
  const at = argv.indexOf(name);
  return at >= 0 ? argv[at + 1] : fallback;
}

/**
 * The legal programs whose violation is kept on purpose: the site provably never runs (a carrier called on an empty receiver: the table family's
 * `zero` twin), and the rule still reports the code, because a violation means "wrong if it runs". Owner ruling 2026-11-10 (ADR 0563 keeps the read
 * side asymmetric: a read at such a site is silent). The host's outcome for these is `unexecuted`, which `legal.mjs` reports and does not gate.
 * They are listed apart from the candidate false positives, never as one, and never hidden: a row is here only by this predicate.
 */
export const EXPECTED_UNEXECUTED = {
  ruling: "owner ruling 2026-11-10: a violation means \"wrong if it runs\", kept at a site that provably never runs (a zero-run carrier)",
  matches: (row) => row.side === "legal" && /:b\/zero$/.test(row.pair) && row.code !== "STRICT_READ_UNTRACKED",
};

/**
 * The pairs' claims against a run: the misuse programs left clean anywhere (candidate false cleans), the legal programs with a violation
 * (candidate false positives) and, apart, the legal violations the owner keeps at sites that never run (EXPECTED_UNEXECUTED).
 */
export function problemsOf(rows) {
  const seen = new Set();
  const unique = (list) => list.filter((row) => !seen.has(`${row.side}|${row.id}`) && seen.add(`${row.side}|${row.id}`));
  const violating = unique(rows.filter((row) => row.side === "legal" && row.programOutcome === "violation"));
  return {
    silentMisuse: unique(rows.filter((row) => row.side === "misuse" && row.programOutcome === "clean")),
    reportedLegal: violating.filter((row) => !EXPECTED_UNEXECUTED.matches(row)),
    keptUnexecuted: violating.filter((row) => EXPECTED_UNEXECUTED.matches(row)),
  };
}

async function main(argv) {
  const command = argv[0];
  const { dirname, join, resolve } = await import("node:path");
  const { existsSync, readFileSync, rmSync, writeFileSync, mkdirSync } = await import("node:fs");
  const { fileURLToPath } = await import("node:url");
  const here = dirname(fileURLToPath(import.meta.url));
  if (command === "report") {
    const snapshot = JSON.parse(readFileSync(fileURLToPath(WITNESSES), "utf8"));
    const covered = coveredArms(snapshot);
    console.log(`pairs ${allPairs().length}; snapshot covers ${covered.size} arm(s) (${Object.keys(snapshot.arms ?? {}).length} seen silent)`);
    return 0;
  }
  if (command !== "verify") {
    console.error("usage: node scripts/fuzz/boundary.mjs verify --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> [--ablate] [--only <pair id text>] [--concurrency 4] [--update] [--out <rows.json>]\n       node scripts/fuzz/boundary.mjs report");
    return 2;
  }
  const bin = resolve(option(argv, "--checker-bin", process.env.SOLID_CHECKER_NATIVE_BIN ?? ""));
  const typefacts = resolve(option(argv, "--typefacts", process.env.SOLID_TYPEFACTS_BIN ?? ""));
  const work = resolve(option(argv, "--work", join(here, "../../.solid-checker-fuzz", `boundary-verify-${process.pid}`)));
  mkdirSync(work, { recursive: true });
  const rows = await runPairs(allPairs(), { bin, typefacts, install: resolve(option(argv, "--install", join(here, "install"))), work, concurrency: Number(option(argv, "--concurrency", 4)), ablate: argv.includes("--ablate"), only: option(argv, "--only") });
  rmSync(work, { recursive: true, force: true });
  const { silentMisuse, reportedLegal, keptUnexecuted } = problemsOf(rows);
  console.log(`boundary verify: ${rows.length} program(s) over ${allPairs().length} pair(s)`);
  console.log(`  misuse programs with no finding anywhere (candidate false cleans, Chrome decides): ${silentMisuse.length}`);
  for (const row of silentMisuse) console.log(`    ${row.id}  [${row.pair}] arms: ${[...new Set(row.decisions.filter((decision) => decision.arm).map((decision) => decision.arm))].join(", ") || "-"}`);
  console.log(`  legal programs with a violation (candidate false positives, Chrome decides): ${reportedLegal.length}`);
  for (const row of reportedLegal) console.log(`    ${row.id}  [${row.pair}] ${row.program.filter((finding) => finding.kind === "violation").map((finding) => `${finding.id}@${finding.line}`).join(", ")}`);
  console.log(`  legal programs whose violation the owner keeps at a site that never runs (expected \`unexecuted\`; ${EXPECTED_UNEXECUTED.ruling}): ${keptUnexecuted.length}`);
  const snapshot = snapshotOf(rows, { checker: option(argv, "--checker-name") });
  console.log(`  arms seen silent in a clean legal cell: ${Object.keys(snapshot.arms).length}; covered (misuse side never clean): ${coveredArms(snapshot).size}`);
  if (option(argv, "--out")) writeFileSync(option(argv, "--out"), `${JSON.stringify(rows)}\n`);
  if (argv.includes("--update")) {
    const previous = existsSync(fileURLToPath(WITNESSES)) ? JSON.parse(readFileSync(fileURLToPath(WITNESSES), "utf8")) : null;
    writeFileSync(fileURLToPath(WITNESSES), `${JSON.stringify(snapshot, null, 1)}\n`);
    console.log(`  wrote ${fileURLToPath(WITNESSES)}${previous ? ` (was ${Object.keys(previous.arms ?? {}).length} arm(s) seen)` : ""}`);
  }
  return 0;
}

if (process.argv[1]?.endsWith("boundary.mjs")) process.exit(await main(process.argv.slice(2)));

export { TABLE_GAPS };
