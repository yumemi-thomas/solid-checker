# ADR 0329: A DOM call that dispatches synchronously is not trusted

- Status: accepted and implemented (2026-10-19). Owner delegated the decision
  ("do what you think correct") after ADR 0328 made uncertifiable internal.
- Owners: `synchronously_dispatching_dom_member` and `dispatches_dom_event`
  (`solid-reactive-ir/src/cleanup.rs`), consulted by `member_call_operations`
  and the global standard-library call paths of the leaf-scope walk.
- Relation: extends ADR 0318, which refused every optional DOM member outside
  the script-free table and kept the non-optional form trusted. Closes the
  owner decision recorded there ("refusing non-optional re-entrant members
  would add 44 uncertifiable findings").

## Context

The leaf-scope walk trusts a standard-library call to run only the functions
handed to it (ADR 0210). A DOM member that dispatches an event before it
returns runs listeners the program registered elsewhere, with no callback at
the call site: `el.focus()` runs `focus`/`focusin` listeners, `el.click()` a
click handler, `dispatchEvent` any listener. Trusting them certified leaf
scopes whose listeners may perform a forbidden operation: an unproven clean
claim.

## Decision

A call whose exact `lib.dom` declaration is one of `focus`, `blur`, `click`,
`dispatchEvent`, `reset`, `requestSubmit`, `checkValidity`,
`reportValidity`, `showPopover`, `hidePopover`, `togglePopover`, `show`,
`showModal`, `close` (on any interface, or as a `lib.dom` global) refuses the
walk, optional form or not. Every other non-optional DOM member keeps the
standard-library trust, and optional members keep ADR 0318's allowlist.

## Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| synchronous dispatch members | refused by member name on any `lib.dom` interface | they run registered listeners before returning (DOM/HTML specs) | `leaf-scope-builtin-method` `BuiltinMethods`, `leaf-scope-optional-dom` `DirectFocus` |
| `lib.dom` globals | the same names on the global declarations | `dispatchEvent(…)` is `window.dispatchEvent` | `leaf-scope-host-timing` `GlobalSynchronousDispatch`, `MediaQuerySynchronousDispatch` |
| tree mutation and attribute writes | still trusted when non-optional | custom-element reactions and removing a focused element are not modeled | — (recorded as open) |
| asynchronously queued events (`select`, scrolling) | still trusted | their listeners run on a later task | — |

## Consequences

Corpus: 0 removed, +20 `SC9012` uncertifiable (kui 10, ai-memory-ui 4, app-game
3, other apps 3). Findings move only to uncertifiable, which is internal (ADR
0328); users see
no change. Not modeled and recorded open: custom-element reactions on tree
mutation and attribute writes, and focus loss when a focused element is
removed.
