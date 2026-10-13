# ADR 0164: A valueless body closes `returns` whatever the reactive analysis could not describe

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners: the generator's `returns` proposal (`inferred_contract.rs`)
- Relation: widens where ADR 0035's empty closure is proposed. The census, the
  positive fact and the veto are ADR 0035's, unchanged; no producer change, no
  handshake protocol.

## Context

The @solid-primitives checkpoint's largest single blocking cause is "`returns`
never proposed". Classified host free from the published declarations
with TypeScript from each retained install, 171 exports in 64 packages (the
same 171 at 8b490f30 and at 9b7a432b, identical in every host):

| class | exports | packages | claim form |
| --- | ---: | ---: | --- |
| tuple holding an accessor or value (signal-like) | 27 | 17 | none; ADR 0157 parks the signal tuple |
| a returned accessor `() => T` | 25 | 15 | only ADR 0146's owned `createSignal`; these are `createHydratableSignal`, `createMemo`, singleton roots and `Proxy` wrappers |
| an object holding accessors | 16 | 14 | none |
| a tuple of functions or objects | 16 | 11 | none |
| another object | 15 | 11 | none |
| an object of primitives | 14 | 7 | none certified (`ValueShape::Object` has no census) |
| another function (with parameters) | 12 | 10 | ADR 0145, but each body calls a helper's result or its own argument |
| a cleanup function `() => void` | 12 | 10 | ADR 0145, but every body calls a helper's returned cleanup or returns an imported `noop` |
| a JSX element or a union of shapes | 11 | 8 | none |
| a class (construction) | 7 | 4 | none |
| `void` | 7 | 5 | **ADR 0035's empty closure** |
| a promise | 2 | 2 | none |
| a primitive | 2 | 2 | ADR 0113, but both are values chosen by a conditional of arrows |
| other (`unknown`, a parameter, an array) | 5 | 5 | -- |

None of the 171 has `returns` as its only open domain, and no withheld
`returns` operation is among them (a withheld operation outranks "never
proposed" in the metric, so those are counted elsewhere).

So the eleven largest classes need a value shape no census decides yet --
above all the signal-like tuple and the returned accessor, which are the owner's
(ADR 0157) -- or, for the two function classes, composition evidence the
described-callable census does not have (a helper's or a dependency's returned
cleanup, an imported `noop`), and none of those is built here.

The only class whose claim form is complete -- a value shape, a census, a
positive fact and a veto -- and whose proposal alone is missing is `void`:
`createEventListenerMap`, `createFocusRestore`, `createFocusTrap`,
`createPointerListeners`, `createPerPointerListeners`, `createResizeObserver`
and `styles`' `setServerRemSize`. Each body's every completion is valueless;
the generator's valueless-completion walk clears it; and still nothing was
proposed, because an unresolved call in the body (`createEventListener`,
`split`, an owner helper) erases the export's reactive description of the
return to `Open` -- for these, the attribution widening of an import whose
dependency contract describes no matching export (`PackageContractExportMissing`,
all five domains) -- and the empty closure was proposed only from
`Known(None)`.

## Decision

The empty closure is proposed over an `Open` return exactly as over an
undescribed one: a function export in a scope that publishes bootstrapped
domains, not inherited, whose valueless-completion walk cleared it. It is
tried before the argument-container, alias and described-callable proposals,
which never overlap it (each is a value).

The erasure is about the *reactive description* of a returned value: an
unresolved call can hand a reactive member to a value the export returns. A
body that returns no value hands nothing, and the walk that says so is about
the export's own completions, which no unresolved call can change. Nothing is
trusted that was not before: the walk only proposes, and ADR 0035's census
proves `returns: []` from the producer's control-flow census -- a plain
completion form, a classified census, no reachable value-carrying completion
-- beside its synthesized veto.

## Consequences

- `a_valueless_body_proposes_the_empty_closure_over_an_open_return`
  (`inferred_contract/tests.rs`) pins the proposal and its falsifiers: a body
  the walk did not clear, a component, a dialect's own archive.
- No corpus fixture can show it: the stable corpus declines every closure of an
  artifact case whose closure imports a dependency outside the accepted tier
  (`unaccepted-external-dependency`), which is how every fixture with a missing
  contract is built, and the erasure this ADR looks past arises only for an
  import of an *accepted* dependency's undescribed export. Contract corpus
  (115 fixtures) and coverage move nothing.
- `make primitives-checkpoint`'s three host runs, the same release binary
  built with and without this change at 6c66c825 (the checkpoint's own
  procedure, without the misuse ledger): clean exports **90 -> 90** (none and
  browser) and **100 -> 100** (node); "`returns` never proposed" 169 -> 163
  (none, browser; 63 packages) and 171 -> 165 (node; 64 packages); degenerate
  228 -> 220 (none), 229 -> 221 (browser), 224 -> 215 (node), each to partial.
  Six packages move, identically in every host: `event-listener` 6/5 -> 7/4
  partial/degenerate, `focus` 3/5 -> 4/4, `form` 2/5 -> 3/4, `keyboard` 0/7 ->
  2/5, `pointer` 3/4 -> 5/2, `url` 0/12 -> 1/11. `returns` now certifies
  closed over nothing for `event-listener`'s `createEventListenerMap`,
  `focus`' `createFocusTrap`, `pointer`'s `createPointerListeners` and
  `createPerPointerListeners` and `resize-observer`'s `createResizeObserver`,
  and -- through the same widening, where the cause read "fallback-all:
  PackageContractExportMissing" -- `form`'s `createFormResetListener`,
  `keyboard`'s `createKeyDown` and `createShortcut` and `url`'s
  `setLocationFallback`. `focus`' `createFocusRestore` certifies in `node` and
  its veto does not complete host free or in `browser` (`document is not
  defined` at import), and `styles`' `setServerRemSize`, a value chosen by a
  conditional, is not a function the walk reads. (The same comparison at
  9b7a432b, before 9c25ea1c..6c66c825, read clean 96 -> 96 and 171 -> 165.)
- Clean does not move because no export on the wall had `returns` as its only
  open domain: each of these still has `reads`, `callbacks` or `creates` open
  for its own reasons.
- The valueless walk's own tests (`returns_walk.rs`,
  `the_valueless_walk_clears_only_a_body_no_path_of_which_returns_a_value`)
  pin what it clears -- an empty body, a bare `return;`, a thrown path, a
  `try`/`catch`, a nested callable's `return` -- and what it leaves open: a
  `return` carrying an expression on any path, an expression body (even
  `() => undefined` or `() => void g()`, whose value the walk cannot prove),
  and every `async` function, thrown body included.

## What still refuses

Every class but `void` in the table above: it needs a value shape the census
cannot yet decide, or composition evidence for a returned callable's calls.
