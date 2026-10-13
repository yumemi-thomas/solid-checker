# ADR 0213: A parameter holds what every caller passes

- Status: accepted and implemented (2026-10-06). Phase 3 of the value-flow
  work on caller-supplied member dispatch.
- Owners:
  - `CallGraph::parameter_holds_builtin`, run by
    `discharge_closed_program_export_dispatch`
    (`solid-reactive-ir/src/attribution.rs`);
  - `SemanticLookup::member_name_may_be_reassigned`
    (`solid-reactive-ir/src/indexes.rs`).
- Relation: extends ADR 0211 and ADR 0212 across calls, using ADR 0203's
  closed-program proof that a function is entered only through calls. Narrows
  the prototype clause of ADR 0209 and ADR 0211.

## Context

A helper nested in a component often hands its own parameter on:

```ts
const inCalendar = (date: Date) =>
  isYearly(date) ? calendarDate(date.getMonth(), date.getDate()) : date;
inCalendar(new Date(year, month, 1));
```

The obligation at `isYearly(date)` asks which `getFullYear` runs on `date`.
ADR 0211 cannot answer at that site, because `date` is a parameter. Every
call of `inCalendar` passes a `Date`.

A measurement showed that the proofs of ADR 0209, 0211 and 0212 were
switched off across whole applications. Any assignment whose target
mentioned `prototype` refused every member everywhere, so one test file's
`HTMLElement.prototype.focus = focus` disabled them all in openbot.

## Decision

1. **A parameter-member obligation whose argument is a parameter is
   discharged** in a closed program (ADR 0193) when every one of these
   holds:
   - the argument is a plain identifier parameter of the innermost function
     around it, not a rest parameter, and no assignment in its file writes
     it;
   - the function is entered only through call expressions (ADR 0203), so
     its call sites are every entry;
   - at every call site, the argument at the parameter's position, if any,
     is a value `value_origin` proves (ADR 0211, 0212), or a parameter that
     holds one, followed up to four functions back. No spread may come
     before it;
   - a default value holds one too, and nothing assigns the member or
     `__proto__`.
2. **A named write through a prototype refuses only the member it names.**
   `C.prototype.m = f` records `m`, as `x.m = f` always did. Only a write
   that replaces a prototype (`C.prototype = …`), or writes one at a key no
   fact names (`C.prototype[key] = …`), refuses every member.

## Consequences

- A parameter fed only built values from visible calls selects the built-in
  members at its helper's call sites.
- ADR 0209's exact-instance walk and ADR 0211/0212's origins now apply in
  programs that patch one named member somewhere, for every other member.
- Still open:
  - parameters of functions handed out as values or rendered as components
    (`props.items`);
  - destructured parameters;
  - open programs (libraries).

## Evidence

- **Fixture** `fixtures/reactive-ir/parameter-pass-through-dispatch` (a
  closed program):
  - discharged: a helper fed literals, one of them through a second helper's
    parameter, and a helper with a `[]` default called with and without an
    argument;
  - kept: a helper with one caller-supplied argument, a helper that assigns
    its parameter, and a helper handed out as a value.
- **Fixture** `value-origin-member-dispatch` now writes `Live.prototype.extra`.
  Every other case stands.
- **Coverage:** 182 fixture projects, 975 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary:
  - against `rc13-s-browser.json` (ADR 0212), the pass-through alone moves
    nothing. The prototype refinement then takes uncertifiable from 3,375
    to 3,318: 57 sites removed, none added;
  - violations unchanged at 285;
  - the removals are app-game's exact `new Renderer()` instances in leaf
    scopes (ADR 0209), openbot's `RoutineDateField` and `OtpInput` (pass-
    through and helper returns), and dashiboard-ui's spread writes.
