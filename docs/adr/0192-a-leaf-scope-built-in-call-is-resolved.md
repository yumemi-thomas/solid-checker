# ADR 0192: A built-in call in a leaf scope is resolved

- Status: accepted and implemented (2026-10-05). Step A2 of
  [the package direction](../2026-10-05-package-direction.md), first slice.
- Owners: the Type Facts demand plan (`plan_file` in
  `solid-facts-backend/src/demand_plan.rs`). The leaf-scope check that reads
  the answer (`helper_forbidden_operations` in `solid-reactive-ir/src/cleanup.rs`)
  is unchanged.
- Relation: extends ADR 0190's demand clause from parameter-rooted calls to
  calls written directly in a leaf-owner callback. No wire change, no
  semantic change.

## Context

Each call written directly in a leaf owner's callback (`onSettled`,
`createTrackedEffect`) runs in that leaf scope. The checker must know it
creates no primitive and registers no cleanup. A call it cannot follow leaves
an obligation:

> onSettled receives a type-correct callback whose exact synchronous body
> cannot be resolved …

There are 813 of these over the 38-app corpus. A standard-library call is
already accepted, on its resolved declaration. But Type Facts resolves a call
only when asked, and the demand plan did not ask for an argumentless call. So
`dialog.focus()` and `el.getBoundingClientRect()` were obligations, while
`setAttribute("x", "1")` was not.

## Decision

The demand plan also asks for the resolved call of an argumentless method call
written inside a leaf-owner callback.

- The callbacks are found by the dialect's own answer per argument
  (`callback_semantics_at(…).owner == Leaf`), on calls whose spelling names a
  dialect primitive. A name that only shares the spelling over-demands, which
  decides nothing.
- **Not every method call.** A first version asked for every argumentless
  method call. That lost two proven violations in
  `fixed-structural-return-consumer` and `partial-structural-return-consumer`
  (`result.value()`, `panel.value()`). A resolved declaration also enters the
  symbol index (`indexes.rs`, the declaration-location map). Adding one for
  every member call gave a structural member a second candidate, so the read
  stopped being proven. The demand is therefore scoped to the two places that
  ask the question: this one, and ADR 0190's parameter-rooted calls.

## Consequences

- **Optional calls remain open.** Type Facts states no resolved call for an
  optional call, with or without arguments (`maybe?.focus()`,
  `anchor?.querySelector(selector)`). That is a producer gap, left for a
  separate change to the producer.
- A member call that is not a standard-library method (`renderer.setSize()`
  on a declared object) stays an obligation, as before.
- Interaction to remember: demanding a resolved call is not free of meaning.
  It can change symbol resolution through the declaration index, so any
  future widening of resolved-call demand needs a coverage run that looks for
  lost violations, not just new ones.

## Evidence

- **Fixture** `fixtures/reactive-ir/leaf-scope-builtin-method`:
  - `dialog.focus()` and `document.body.getBoundingClientRect()` in
    `onSettled` become clean;
  - `maybeDialog?.focus()` (the optional gap) and `renderer.setSize()` (no
    body, not a built-in) stay `SC9012`.
- **Unit test**
  `an_argumentless_method_call_is_demanded_its_resolved_call`:
  - a chained and an optional call through a parameter are demanded, and so
    is one directly in an `onSettled` callback;
  - a method call elsewhere is not, and neither is an argumentless plain
    call.
- **Coverage:** 165 fixture projects. Only the new fixture's snapshot is new;
  no existing finding moved.
- **38-app browser sweep** (release binary, against ADR 0191's
  `tierenv-browser.json`):
  - leaf-scope obligations go from 813 to 766;
  - uncertifiable goes from 8,297 to 8,250 (29 distinct sites removed, none
    added);
  - violations stay at 273, with nothing added or removed;
  - summed wall time is 161 s before and 163 s after.

  The rest of the group is a long tail of methods on objects the project
  declares or imports without a body (`renderer.setSize`,
  `camera.perspective`, `backend.subscribe`), optional calls, and helpers
  whose own bodies are incomplete.
- **Misuse ledger:** unchanged at 79 of 123, with no twin moved.
- **Backend library tests** (775) and targeted clippy pass.
