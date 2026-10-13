# ADR 0199: A callback prop forwarded to an event runs on dispatch

- Status: accepted (2026-10-05). Track A, step A3, of
  `docs/2026-10-05-package-direction.md`.
- Owner: `forwarded_event_prop_role` and `prop_reaches_only_events` in
  `solid-reactive-ir/src/execution_role.rs`.
- Fixtures: `fixtures/reactive-ir/forwarded-event-prop`; the two
  forwarded-handler cases of `fixtures/reactive-ir/callee-callback-timing`
  move.

## Context

Take a function literal written as a component's prop, such as
`<Button onClick={() => save(draft())}>`. It is constructed while the caller
renders, but it runs whenever the consumer calls it. `callee_callback_timing`
therefore leaves every read in such a literal uncertifiable
(`SC1001 … passed to a consumer that is not proven to invoke it …`). On the
rc.13 corpus this was the largest class of uncertifiable strict reads: 680
sites. About 330 of them sit in a callback prop on a component, and the most
common shape by far is a button-like component that hands `props.onClick` to
a `<button>`.

The intrinsic case is already decided. The compiler classifies
`<button onClick={() => …}>` as an event handler, and such code takes
`ExecutionRole::EventCallback`: it runs on dispatch, outside every strict-read
window, and reports no untracked read.

## Decision

1. **A literal written as a project component's prop takes the event role when
   that component only ever hands the prop to DOM event dispatch.** The arm
   sits ahead of the compiler role and the lexical fallbacks in
   `semantic_execution_role`, so a nested primitive inside the literal is
   still classified by its own arm first.
2. **The consumer is resolved exactly.** The JSX tag's symbol names one
   project function (`SemanticLookup::function_called_at`). An unresolved tag,
   a package component seen only through its declarations, or an intrinsic
   tag answers nothing.
3. **Every use of the prop is proven.** The component's props parameter is one
   identifier (not destructured, no default, no rest), and every reference to
   it is a static member access. Each `props.<prop>` must be one of:
   - exactly the value of an intrinsic element's attribute that the compiler
     classifies as an event handler (`<button onClick={props.onPress}>`);
   - inside the function written as such a handler
     (`onClick={(event) => props.onPress?.(event)}`);
   - exactly the value of another project component's prop that satisfies
     this same proof.

   A props object that escapes whole (a call argument, a destructuring, a
   computed key) proves nothing, and neither does a forwarding cycle.
4. **Props views and spreads are followed.**
   - A `const` bound to a props merge or split the dialect names
     (`Dialect::merges_props_reactivity`, `Dialect::splits_props`: rc.13's
     `merge` and `omit`) is a view of the props and is checked the same way.
     Both are live views whose reads forward to their sources; neither
     invokes a property's value (`@solidjs/signals` rc.13
     `store/utils.d.ts`).
   - A spread of the props, or of such a view, onto an intrinsic element
     carries an un-namespaced `on…` prop to an event listener. rc.13's
     `@solidjs/web` `assignProp` attaches such a property with `addEvent`;
     the one property a spread runs, `ref`, is handled before it and is not
     an `on…` name.
   - A spread onto a project component must satisfy the same proof there.
5. **It withholds nothing it cannot prove.** A consumer that calls the prop
   while rendering keeps the read uncertifiable, not a violation. That read is
   a true defect, but this arm proves only the event case.

## Consequences

- A read in a callback that can only run on an event is no longer reported,
  exactly as when it is written on the element itself.
- Still uncertifiable:
  - a props object handed to a helper or destructured;
  - a non-`on…` callback prop reaching a spread;
  - components resolved only through declarations (a published package
    rather than workspace source);
  - callback props invoked from effects, timers or while rendering.

## Evidence

- Fixtures: `forwarded-event-prop` has 7 clean and 7 uncertifiable cases.
  `callee-callback-timing`'s two forwarded-handler reads (its `Handler`
  invokes the prop inside a `<button>` click handler) leave the snapshot. No
  other fixture moves: 168 projects, 892 findings.
- rc.13 corpus, browser host, against `rc13-r4-authored-browser.json`:
  - violations unchanged at 267 (+0, -0);
  - uncertifiable 7,070 -> 6,901, 106 distinct sites removed and none added,
    all `SC1001` callback-prop reads. By app: kui 45, app-game 26,
    derp-media-server 9, then 12 apps with 1 to 4 each.
  - Spot-checked consumers: `IconButton` (`onClick={props.onClick}`), kui's
    `Button` (`merge` view, read inside the `<button>` click handler, `omit`
    spread), `AppToolbar`, `ReaderFrame` and `WorkspaceWindowTitlebar` (the
    prop called only inside a `<div>` or `<button>` handler).

## Extension (2026-10-05)

Three widenings, each matching what the compiler's own handler role already
does:

1. **Any enclosing literal, not only the innermost.** Code nested anywhere in
   an event-only literal (an updater passed to a setter, a callback passed to
   an async helper) runs only after dispatch. This is how code nested in a
   handler written on the element is classified. Nested primitives and inline
   callbacks are still classified by the arms ahead of this one.
2. **A prop invoked inside another event-only prop literal.**
   `<Button onClick={() => props.onConfirm()} />` makes `onConfirm`
   event-only when `Button`'s `onClick` is.
3. **A `const` alias of the props or of a view** (`const root = rest as
   Props`) is the same object under another name.

Accepted limit: a function *stored* by an event-only literal and later called
during some render would be missed. That is a possible false negative, never a
false positive, and the same limit applies to a handler written on the
element.

Evidence:

- Fixture: three new clean cases (`UsesConfirm`, `UsesAliasSpread`,
  `NestedInHandler`) and two new uncertifiable ones (`UsesConfirmEagerly`,
  `NestedOutsideHandler`).
- rc.13 corpus: 28 more sites leave `SC1001` uncertifiable (6,886 to 6,856
  findings), 17 of them in `ai-memory-ui`'s `workspace-detail.tsx`. No
  violation moved.
