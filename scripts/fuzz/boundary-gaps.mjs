// The stated gaps of the boundary registry (ADR 0935): what boundary.test.mjs lets stand without a pair, each with its reason. The file only shrinks: a
// gap whose item is covered now fails the test as stale, an item without a pair or a gap fails it as new. Sizes are pinned in the test by these lists.
//
// Reading the arm gaps. An arm is "separated" when the snapshot (boundary-witnesses.json, `node scripts/fuzz/boundary.mjs verify --update`) holds a clean
// legal program of a registered pair in which the arm answered silent, and the pair's misuse side for that code left no program clean. A gap is an arm no
// registered pair separates: the pairs' programs never reach it, or reach it where their misuse side is silent too. Whether some grid program reaches it
// is in the audits' role-log folds (`roles.json`, `neverHit`), not decided here.

/** Premise kinds with no pair, fixture or stated outside reason. */
export const KIND_GAPS = {
  CreateMutableProxy: "No fixture and no generated shape: `createMutable` appears in no fixture and in no template. A pair needs a project that writes through a mutable proxy (the legal side) and one that writes to a store it merely reads (the misuse side).",
  LogicalAssignmentSkipsWrite: "No dedicated fixture or shape for ADR 0718's logical assignment that provably does not write (`a ||= b` over a truthy literal). The fixtures that use logical assignment (handed-net-guards, summary-completion-read) are about reads.",
  DormantCallbackArgument: "No dedicated fixture or shape: lazy-default-consumer is the nearest. The stored-value positions and discarded lazy adapters (`createReaction`, `lazy`) have no pair of a program that runs the function and one that only names it.",
};

/** Named ends whose kind has no pair (the rest are covered through their premise). */
export const END_GAPS = {
  "local_access.discover_call_as.dormant-callback-argument": "Rests on DormantCallbackArgument, which has no pair or fixture yet (see KIND_GAPS).",
  "local_access.async_sites.dormant-callback-argument": "Rests on DormantCallbackArgument, which has no pair or fixture yet (see KIND_GAPS).",
  "shared_reactivity.direct_mutations.create-mutable": "Rests on CreateMutableProxy, which has no fixture or shape (see KIND_GAPS).",
  "shared_reactivity.direct_mutations.logical-assignment-skips": "Rests on LogicalAssignmentSkipsWrite, which has no fixture or shape (see KIND_GAPS).",
  "local_access.discover_call_as.role-does-not-report": "Rests on RoleDoesNotReport (kind-role-does-not-report), but no fixture or generated shape reaches this return: a call read whose role the classifier decided reports nothing still gets its silent row, so the producer returns without one only where another pass owns the site. A pair needs a program that hits it; none is known.",
};

/** Table rows the table family and the dispatch pairs do not cover, `<table>:<row>`, with the reason shared by the group. */
export const TABLE_GAP_GROUPS = [
  {
    reason: "A cancel or a removal decides whether a registered callback ever runs. No window code tells a callback that runs on a fresh stack from one that never runs (both are legal), so the family's three codes have no misuse twin; NO_OWNER_CLEANUP does (a fresh-stack run has no owner, a cancelled one never runs). The next slice adds that code to the table family.",
    rows: ["callback-host:clearTimeout", "callback-host:WindowOrWorkerGlobalScope.clearTimeout", "callback-host:clearInterval", "callback-host:WindowOrWorkerGlobalScope.clearInterval", "callback-host:cancelIdleCallback",
      "callback-host:Window.cancelIdleCallback", "callback-host:cancelAnimationFrame", "callback-host:AnimationFrameProvider.cancelAnimationFrame", "callback-host:removeEventListener"],
  },
  {
    reason: "ADR 0893's throw reaches the host: whether an error thrown in a scheduled callback is observable by the program. The family's codes are not throw codes; the stored-error contexts (`st-*`, flip-read.mjs) hold Promise.then/catch/finally and none holds a timer, a microtask or an observer callback.",
    rows: ["throw-reaches-host:queueMicrotask", "throw-reaches-host:setTimeout", "throw-reaches-host:setInterval", "throw-reaches-host:requestAnimationFrame", "throw-reaches-host:requestIdleCallback",
      "throw-reaches-host:WindowOrWorkerGlobalScope.queueMicrotask", "throw-reaches-host:WindowOrWorkerGlobalScope.setTimeout", "throw-reaches-host:WindowOrWorkerGlobalScope.setInterval",
      "throw-reaches-host:AnimationFrameProvider.requestAnimationFrame", "throw-reaches-host:Window.requestIdleCallback", "throw-reaches-host:IntersectionObserver.construct",
      "throw-reaches-host:ResizeObserver.construct", "throw-reaches-host:MutationObserver.construct", "throw-reaches-host:PerformanceObserver.construct", "throw-reaches-host:ReportingObserver.construct"],
  },
  {
    reason: "Delivers a blur or an input only if an element is focused, and a `focus()` call is itself a hazard that withholds the program on every build (ADR 0645; the header of flip-write.mjs): no clean program holds the member.",
    rows: ["dispatching-members:Node.removeChild", "dispatching-members:ChildNode.replaceWith", "dispatching-members:Node.replaceChild", "dispatching-members:ParentNode.replaceChildren", "dispatching-members:HTMLSelectElement.remove",
      "dispatching-members:Document.execCommand", "delivers-synchronously:removeChild", "delivers-synchronously:replaceWith", "delivers-synchronously:replaceChild", "delivers-synchronously:replaceChildren", "delivers-synchronously:execCommand"],
  },
  {
    reason: "Navigation ends the fuzz page (the header of flip-write.mjs: `History.go` and `Location.*` end the page), so no program runs on after the member.",
    rows: ["dispatching-members:History.go", "dispatching-members:Location.assign", "dispatching-members:Location.reload", "dispatching-members:Location.replace",
      "delivers-synchronously:go", "delivers-synchronously:assign", "delivers-synchronously:reload", "delivers-synchronously:replace"],
  },
  {
    reason: "Held by flip-write.mjs (`flw-settled-form-submit`, `flw-settled-report-error`) for a leaf-owner code, not for a window code: a form submit needs an iframe target to keep the page, and `reportError` needs its error cancelled. No pair for the family's codes.",
    rows: ["dispatching-members:HTMLFormElement.submit", "dispatching-members:reportError", "dispatching-members:WindowOrWorkerGlobalScope.reportError", "delivers-synchronously:submit", "delivers-synchronously:reportError"],
  },
  {
    reason: "No shape yet. The FIRES machinery (misuse.mjs) writes the element, the listener and the call for twelve members; these need a setup of their own (an abort signal, an XMLHttpRequest, a FileReader mid-read, a custom element's internals, a popover or dialog opened first, a textarea).",
    rows: ["dispatching-members:reportValidity", "dispatching-members:HTMLTextAreaElement.select", "dispatching-members:HTMLElement.hidePopover", "dispatching-members:HTMLElement.togglePopover", "dispatching-members:HTMLDialogElement.show",
      "dispatching-members:HTMLDialogElement.requestClose", "dispatching-members:AbortController.abort", "dispatching-members:XMLHttpRequest.open", "dispatching-members:XMLHttpRequest.abort", "dispatching-members:XMLHttpRequest.send",
      "dispatching-members:FileReader.abort", "dispatching-members:ElementInternals.checkValidity", "dispatching-members:ElementInternals.reportValidity",
      "delivers-synchronously:reportValidity", "delivers-synchronously:show", "delivers-synchronously:requestClose", "delivers-synchronously:hidePopover", "delivers-synchronously:togglePopover"],
  },
];

// ARM_GAPS: generated by scratch/gen-arm-gaps from the snapshot (node scripts/fuzz/boundary.mjs verify --update); each group states its reason.
// <arm-gaps>
export const ARM_GAPS = [
  {
    reason: "The compiler's Elided regions (code it deletes) are produced by no generated program; the one fixture, jsx-void-child-divergence-solid-2, holds the template-root void-child case and not this arm's use. A pair needs a program the compiler deletes a value of, and the same program with the value live.",
    arms: [
      "execution_role.execution_role_where.discarded-rendering",
      "callee_roles.discarded-region",
      "write_roles.first-run-discarded",
    ],
  },
  {
    reason: "No registered pair's programs reach the arm in a clean legal program (the scan of 2026-11-10 on 71b8ec190 found it silent nowhere in the plain, layered, table, wrapper, dispatch, callback and mount shapes).",
    arms: [
      "execution_role.execution_role_where.deferred-callback",
      "execution_role.execution_role_where.compiler-region",
      "execution_role.dynamic_read_role.exclusive-entry",
      "callback_standing.returned_callback_execution_role.tracked",
      "callback_standing.returned_callback_execution_role.deferred",
      "callback_standing.context_provider_value_role.stored-function",
      "callee_roles.forwarded_tracked_compute_role.tracked-compute",
      "callee_roles.inheriting_call_role.clears-tracking-in-tracked-compute",
      "event_literals.function_role.non-reporting-mix",
      "event_literals.write_function_role.non-reporting-mix",
      "event_literals.owner_site_role.registered-test-entry",
      "event_literals.owner_site_role.compiler-event-handler",
      "events.forwarded_event_prop_role.reaches-only-events",
      "read_context.async_execution_role.deferred-in-jsx",
    ],
  },
  {
    reason: "Placing a layer inside an async `ref` callback past its await: the arm is never reached. Since ADR 0938 `body_roles.layered_window_after_awaits.unreported` answers the registered pair (`ref-layer-after-await`, the `barm-ref-*` contexts) first, inside `semantic_execution_role`, before `ref_applied_read_role` runs. The group goes when that dead arm is deleted.",
    arms: [
      "execution_role.ref_applied_read_role.unreported",
    ],
  },
  {
    reason: "A self-declared default (`arm_id!(default ...)`), unreachable in Solid 2: its only caller needs a callback with `requires_return_invocation`, which no Solid 2 primitive has (silence audit row 10).",
    arms: [
      "callback_standing.returned-invocations-incompatible",
    ],
  },
  {
    reason: "Await dominance: the site follows an unconditional or dominating await on its stack, so the strict-read window is closed. The async contexts (`async`, `async-loop`, `memo-async`) have no twin that is the same function with the site before its first await, and the `aw-*` contexts belong to other codes (UNTRACKED_READ_AFTER_AWAIT, CONTEXT_NOT_FOUND). A pair is the same site after and before one `await`.",
    arms: [
      "callee_roles.unwindowed_role_at_call.unreported",
      "callee_roles.unwindowed_role_at_call.unreported-2",
      "callee_roles.named-climb-after-await",
      "callee_roles.allowed-climb-after-await",
    ],
  },
  {
    reason: "A callback of a call that clears tracking, opens a root or an untracked window, or is an effect apply (`createRoot`, `runWithOwner`, `untrack`, `createEffect`'s apply): the root, owner and untrack contexts differ in the primitive as well as in the window, so no pair isolates the premise (ADR 0932's `createRoot` case is the model of one that does).",
    arms: [
      "callee_roles.unwindowed_role_at_call.untracked-unwindowed",
      "callee_roles.effect-apply-argument",
      "callee_roles.inheriting-call",
      "callee_roles.untracked-unwindowed-argument",
      "callee_roles.untracked-argument",
      "callee_roles.inheriting-cleared-tracked-region",
      "callee_roles.inheriting-invalid-call",
      "callee_roles.inheriting-tracked-root",
      "callee_roles.inheriting-module-initialization",
      "callee_roles.inheriting-fresh-stack",
      "callee_roles.inheriting-holder-silent",
    ],
  },
  {
    reason: "A context provider's `value` position and module-scope initialization: no generated program for these codes has a provider value read or a module-scope site.",
    arms: [
      "callee_roles.context-provider-value",
      "callee_roles.module-initialization",
    ],
  },
  {
    reason: "A function passed to a primitive by name (`const f = ...; createMemo(f)`, a returned callback, a mount root): the callback-form pairs cross the forms with the primitive contexts, which differ in the primitive as well as the form, and none of their legal cells is answered by this arm.",
    arms: [
      "callee_roles.named-climb",
      "callee_roles.named-mount-root",
      "callee_roles.named-callback-write",
      "callee_roles.named-callback-called-in-place",
      "callee_roles.named-callback-untracked-nested",
      "callee_roles.named-callback-in-place",
      "callee_roles.returned-callback",
      "callee_roles.returned-factory-callback",
      "callee_roles.forwarded-tracked-compute",
      "named_callbacks.named_callback_execution_role.deferred-callback",
      "named_callbacks.named_callback_execution_role.ref-applied",
      "named_callbacks.named_callback_execution_role.untracked-callback",
      "named_callbacks.named_callback_execution_role.untracked-unwindowed",
      "named_callbacks.named_callback_execution_role.tracked-jsx",
      "named_callbacks.named_callback_execution_role.tracked-jsx-2",
      "named_callbacks.inherits_callback_role.unreported",
    ],
  },
  {
    reason: "No registered pair's programs reach the arm in a clean legal program (the scan of 2026-11-10 on 71b8ec190 found it silent nowhere in the plain, layered, table, wrapper, dispatch, callback and mount shapes). The pair that reaches it leaves a misuse program clean (a candidate false clean, or a pair that does not isolate the premise), so the witness does not count.",
    arms: [
      "callee_roles.inline-callback",
      "callee_roles.tracked-region",
    ],
  },
  {
    reason: "An allowed region's nested function (`onSettled`, `onCleanup`, a closed `untrack`): the registered pairs reach the arm only where their misuse side is silent too, or not at all in a clean legal program.",
    arms: [
      "callee_roles.allowed-climb",
      "callee_roles.allowed-nested-disposal",
      "callee_roles.allowed-primitive-tracked",
      "callee_roles.allowed-nested-fresh-stack",
    ],
  },
  {
    reason: "A flow component's child callback runs in its own window, always: the `flow-*` contexts are misuse for every window code, so the generator has no legal flow twin (the `flow-in-allowed-region` pair is one-sided, stated there).",
    arms: [
      "callee_roles.control-flow-climb",
      "callee_roles.control-flow-tracked-region",
      "callee_roles.control-flow",
      "callee_roles.allowed-primitive-flow",
      "control_flow.control_flow_execution_role.deferred-callback",
      "control_flow.control_flow_execution_role.tracked-jsx",
      "control_flow.control_flow_execution_role.tracked-jsx-2",
      "flow_helpers.primitive_slot_role.tracked-jsx",
      "flow_helpers.primitive_slot_role.deferred",
    ],
  },
  {
    reason: "Action steps (`action_step_inherits_calling_window`): the ACTION_STEP contexts (`action-from-apply`, `action-from-reaction`, `action-plain-yield-from-apply`, `action-from-settled`, `action-from-tracked`) are on the window codes' sides but differ in the primitive that calls the action, not only in the step's premise, and none of their legal cells is answered by this arm: no pair isolates it.",
    arms: [
      "callee_roles.action-step-continuation",
      "callee_roles.action-step-callers",
    ],
  },
  {
    reason: "The write walk (write_roles.rs): the climb from a write to the position that runs it, with its `walk-*` transport arms, the first-run, created-computation and component-mount cases. The layer and context pairs reach few of them in a clean legal program; the transport arms answer the role of the position the climb reached, so the premise is the climb's, which no pair isolates yet.",
    arms: [
      "write_roles.walk-role-1",
      "write_roles.created-computation-suspended",
      "write_roles.created-computation",
      "write_roles.walk-role-2",
      "write_roles.write_role_walk.untracked-callback",
      "write_roles.created-root-disposal-stands-down",
      "write_roles.first-run-unplaced",
      "write_roles.first-run-reporting",
      "write_roles.walk-suspended-1",
      "write_roles.walk-role-3",
      "write_roles.write_role_walk.untracked-callback-3",
      "write_roles.walk-unknown-1",
      "write_roles.walk-unknown-2",
      "write_roles.write_role_walk.unreported-layered",
      "write_roles.walk-unknown-3",
      "write_roles.walk-unknown-4",
      "write_roles.walk-role-4",
      "write_roles.walk-unknown-5",
      "write_roles.walk-suspended-compute",
      "write_roles.write_role_walk.tracked-jsx",
      "write_roles.component-mounted-in-disposal",
      "write_roles.component-mount-unplaced",
      "write_roles.walk-unknown-6",
      "write_roles.walk-unknown-entry",
      "write_roles.walk-unknown-7",
      "write_roles.named-tracked-owned",
      "write_roles.walk-unknown-8",
      "write_roles.walk-unknown-9",
      "write_roles.walk-unknown-10",
      "write_roles.walk-unknown-11",
    ],
  },
  {
    reason: "Generator stepping: the `generator` layer steps the first segment only. The stepped-generator arms need the `gstep-*` contexts (flip-gen.mjs), which exist for MISSING_EFFECT_FN alone and are all misuse, so no pair has a legal side for the window codes.",
    arms: [
      "write_roles.stepped-generator-reaches",
      "write_roles.stepped-generator-never",
      "write_roles.generator-steps-unknown",
      "write_roles.generator-steps",
    ],
  },
];
// </arm-gaps>
