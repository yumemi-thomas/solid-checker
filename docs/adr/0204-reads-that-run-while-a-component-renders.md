# ADR 0204: Three reads that run while a component renders

- Status: accepted and implemented (2026-10-06). Closes three recall gaps
  the app-pattern ledger found (`fixtures/app-patterns-misuse`).
- Owners:
  - `directly_entered_functions` and the call-site read loop in
    `interprocedural_result_reads_for_file`
    (`solid-reactive-ir/src/interproc.rs`);
  - `prop_literal_invoked_during_render` and `component_invokes_prop_in_body`
    in `solid-reactive-ir/src/execution_role.rs`.
- Relation: extends ADR 0201 (a helper's read in its own body runs during
  the call) and is the counterpart of ADR 0199 (a callback prop forwarded to
  an event runs on dispatch).

## Context

Each of these reads runs in a component body's strict-read window, and
rc.13's dev build warns `STRICT_READ_UNTRACKED` at it. The checker left the
first and third uncertifiable and missed the second:

```tsx
function useInner() { const [n] = createSignal(1); return n(); }
function useOuter() { return useInner(); }      // 1. two levels of helpers

const [count] = createSignal(1);
function label(value = count()) { … }            // 2. a default parameter
… label() …

function Render(props) { const text = props.label(); … }
<Render label={() => count()} />                 // 3. a callback prop the child calls
```

1. ADR 0201 proves a read written directly in the callee's own body. A read
   one call deeper stayed summary-attributed and uncertifiable.
2. A default-parameter initializer of a top-level helper lies outside every
   function body. The helper's summary never held its reads, so nothing was
   reported.
3. A literal written as a component's prop had unproven timing, unless ADR
   0199 proved that it reaches only an event.

## Decision

1. **Two or more levels.** A summary read is direct when its owner is a
   function the call enters synchronously through a chain of plain calls,
   and the read is written directly in that function's own body. Each hop is
   a call written directly in the previous function's body: not in a nested
   function, a default or JSX. Its identifier callee resolves to one
   synchronous project function. Method calls are not followed, because a
   subclass can override the method. The walk is bounded at eight levels.
2. **A default parameter.** A top-level callee's default runs during a call
   exactly when the call omits that argument and spreads nothing. An
   accessor called directly in the default, not in a function the default
   builds, is then read during the call. It is claimed at the call with the
   call's role, like an accessor argument the body calls. A helper nested in
   a rendering function keeps its existing proof, which sees every call site
   (`callee_callback_timing`).
3. **A callback prop the child calls in its own body.** A function literal
   that is exactly a component prop's value runs while that component
   renders when all of these hold:
   - the tag resolves to one project function, and the element has no spread
     and names the prop once;
   - the component's props parameter is one identifier, with no default, and
     neither it nor `props.<prop>` is assigned;
   - the component's own body calls `props.<prop>` directly: not in a nested
     function, not in JSX (a tracked region or a prop getter).

   Rendering the element runs that body untracked, so the literal's direct
   reads keep the role of code written in a rendering body.

4. **"Directly in a body" excludes nested parameter lists.** A nested
   function's default parameter lies inside the outer body but outside every
   nested body, so `containing_ast_function` named the outer function for it.
   Code there runs only when the nested function is called. One test,
   `owners::written_directly_in`, now asks for the span to be in the body
   and in no function written there, parameters included. All four checks
   above use it, as does ADR 0201's one-level check, which had the same
   hole.
5. **A cycle proves nothing.** A walk that reaches a function already on its
   path stops with no answer. Whether a recursive call reaches its read
   depends on the values passed (`readA(2)` may never take the branch that
   reads).

Every other shape keeps its previous answer.

## Consequences

- A conditional call or read inside those bodies is still claimed, as ADR 0201
  already claims a conditional read written in the callee's body. That is a
  may-run read: the dev build warns whenever the branch runs.
- Still uncertifiable:
  - a chain through a method call, an `await`, or a function value;
  - a default read through a store path;
  - a prop called only in JSX, in a handler or from a closure;
  - a prop passed with a spread, or through a `merge`/`omit` view.

## Evidence

- **Fixture** `fixtures/reactive-ir/render-time-reads`: `outer()`,
  `withDefault()` and `<Render label={() => count()} />` are violations;
  `withDefault(2)`, `lazyDefault()` and `deferred()` are clean; the JSX-only
  consumer and the spread element stay uncertifiable.
  `useFinisher()`, whose read is in a nested function's default, is clean.
- **Coverage:** 175 fixture projects, 924 findings. Six existing fixtures
  move from uncertifiable to violation, each a body-time read the fixture
  already described as a defect:
  - `forwarded-event-prop` `UsesCallsDuringRender`;
  - the `inner`/`outer` chains of `fp-exec-stored-literals` and
    `fp-summary-nested-helper`;
  - `fp-summary-returned-closure` `Eager`;
  - `shared-reactivity-v2` `TransitivelyDerivedButDiscarded`;
  - `summary-direct-read` `CallsReadThroughHelper`, and its
    `CallsReadDefault` is now found.

  `recursive` stays uncertifiable (clause 5).
- **rc.13 corpus**, browser host, release binary, against
  `rc13-f-browser.json`. Violations stay at 285, with 4 added and 4 removed;
  uncertifiable stays at 3,435.
  - **Added**, each read reviewed in its source:
    - `queue-management-ui` `routes/index.tsx:15`: `createDataStream` calls
      `startProcessing()` in its body, which reads `isBackendAvailable()`;
    - `probus-hk` `RailDiagram.tsx:353,365`: an effect's apply callback calls
      `centreOn`, which reads `box()` and, through `scale()`, `view()`;
    - `openbot` `SkillsMarketplaceModal.tsx:301`: an apply callback calls
      `showPlugin`, then `enterDetails`, then `detailOpen()`, a store read.
  - **Removed**, all ADR 0201 violations that rested on a nested default
    (clause 4):
    - `app-game` `CardStack.tsx:24`: `expandedOrder(…, held =
      inspectedId())`;
    - `every-deck-of-cards` `ArrangePage.tsx:24`: `settle(cards =
      ordering())`;
    - `openbot` `SettingsModal.stories.tsx:142`: `providerUpdate(…, snapshot
      = providerRuntimeSnapshot())`.

    All four were false positives. The fourth, `app-game`
    `abr-viewer/src/App.tsx:36`, is `selection`, read only in
    `remove(ids = selection())`, a method's default. The site keeps its true
    `root` violation (`createSignal(root())` in `createWorkspace`'s body).
- **Misuse ledger** (`fixtures/app-patterns-misuse`, two cases added):
  31 of 33 misuse twins raise `STRICT_READ_UNTRACKED` or their rule's
  diagnostic in Chrome on rc.13, and all 31 are proven violations, 29 under
  their own rule. `callback-prop-called-in-render` moves from uncertifiable
  to violation. No correct twin gets a violation.
- **IR library tests** (304) and targeted clippy pass.

## Amendment (2026-10-06): the direct origin is kept

A call site reports one read per symbol. The rows arrive in the order the
summary collected them. A symbol read both in a nested default and directly
in the body kept whichever came first, and stayed uncertifiable when that was
the default. Direct rows are now considered first, so the proven origin is
kept.

- Fixture: `render-time-reads` `useBoth()` is a violation.
- rc.13 corpus: no change (285 violations, 3,421 uncertifiable). No corpus
  site had this order.
