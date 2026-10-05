# ADR 0190: A primitive receiver dispatches to its built-in prototype

- Status: accepted and implemented (2026-10-05). Step A1 of
  [the package direction](../2026-10-05-package-direction.md).
- Owners:
  - the call-site resolution of a callee's parameter-member invocations
    (`interprocedural_result_reads_for_file` in
    `solid-reactive-ir/src/interproc.rs`);
  - the record of each invocation (`ParameterMemberInvocation`);
  - the Type Facts demand plan
    (`parameter_rooted_member_call` in `solid-facts-backend/src/demand_plan.rs`).
- Relation: narrows the `parameter-member-target-unresolved` obligation
  (`SC9012`). No wire change and no contract change. One more existing fact is
  demanded at some call sites.

## Context

A helper that invokes a member of its own parameter leaves the choice of
implementation to each call site:

```ts
export const t = (message: string, params?: Params): string =>
  params === undefined ? message : message.replace(PLACEHOLDER, …);
```

At each call `t("Create new project")`, the checker resolves `replace` from
the argument actually passed. It found no summary for
`String.prototype.replace`, so every call site carried an obligation:

> t invokes .replace on a caller-supplied value, but the exact runtime
> implementation cannot be selected …

Over the 38-app corpus (`c0188-browser.json`), this shape accounts for 4,774
of the 5,634 `reactive-dispatch-unresolved` results.

| Member | Results |
|---|---|
| `replace` | 823 |
| `trim` | 336 |
| `toLocaleString` | 155 |
| `toFixed` | 77 |
| `toLowerCase` | 73 |
| `startsWith` | 65 |

Of the sites whose source could be located, about 27% pass a literal.

The obligation for the same call at the helper's declaration
(`exported-parameter-member-dispatch`) already treats a member call that Type
Facts resolves to a standard-library declaration as settled. The call-site
path never asked that question.

## Decision

A call site carries no obligation for a parameter-member invocation in the
first two cases. The third makes the second reachable for argumentless calls.

1. **The argument is a literal.** A string, number, boolean, bigint, template
   or RegExp literal (`RuntimeValueKind::Primitive`), not spread, is a fresh
   value. Every member reached through it is its built-in prototype's.
2. **The callee's member call resolves to a primitive wrapper's built-in.**
   Type Facts resolves the call validly, to one standard-library declaration
   owned by `String`, `Number`, `Boolean` or `BigInt`. This is the same trust
   the declaration-site obligation and the audited standard-library timing
   rows (`runtime_semantics.rs`) already rest on.

   Object-typed built-ins (`Date.getTime`, `Array.map`, `Map.get`) are **not**
   included: a subclass or a structurally compatible object can override them,
   and the declaration does not show which value arrives.

3. **The resolved call is demanded for argumentless parameter-rooted calls.**
   Type Facts resolves a call only when it is asked to. The demand plan asked
   for calls with arguments, returned calls, computed calls and dialect
   primitives, so the `trim()` in `message.trim().split(" ")` was never
   resolved. An argumentless member call whose root names a parameter of an
   enclosing function is now demanded too. The root is matched by spelling: a
   shadowed name over-demands, which costs one fact and decides nothing.

No built-in method of a primitive reads reactive state. A callback the helper
passes to the built-in (`replace`'s replacer) is the helper's own call. It is
decided in the helper's body, by the timing rows (`String.replace` and
`String.replaceAll` are an `InlineCallback` row), and is untouched here.

## Consequences

- **Assumption.** A program that patches `String.prototype` (or another
  primitive prototype) with a member that reads reactive state is not
  modeled. Every standard-library row in this codebase already makes that
  assumption.
- **The `any` hole.** Clause 2 trusts the callee's static type: a caller that
  passes an `any` object where `string` is declared is not modeled. Clause 1
  needs no type.
- Unchanged:
  - object-typed receivers;
  - a structural parameter (`key: { replace(…): string }`) whose argument is
    not a literal;
  - longer paths whose member is not a primitive built-in.

## Evidence

- **Fixture** `fixtures/reactive-ir/literal-argument-member-dispatch`:
  - string and template literal call sites become clean;
  - an object argument and an unresolved caller-supplied value stay `SC9012`;
  - a literal passed to a helper whose replacer reads a signal keeps that
    replacer's own timing obligation (`SC9005`), so it does not become clean;
  - a typed `string` passed to a helper calling `.toUpperCase()`, and to one
    calling `.trim().split(" ")`, is clean;
  - a `Date` passed to a helper calling `.getTime()` stays `SC9012`.
- **Unit test**
  `an_argumentless_parameter_member_call_is_demanded_its_resolved_call`: a
  chained and an optional call through a parameter are demanded; a root that
  is no parameter is not.
- **Coverage:** 164 fixture projects. Only the new fixture's snapshot is new;
  no existing finding moved.
- **38-app browser sweep** (release binary, against `c0188-browser.json`):

  | Clauses | Uncertifiable | Distinct sites removed |
  |---|---|---|
  | 1 (literals) | 10,396 → 9,692 | 704 |
  | 1 and 2 | → 9,017 | 1,241 |
  | 1 to 3 | → 8,297 | 1,854 |

  Violations stay at 273, with nothing added or removed. Nothing is added to
  uncertifiable. The sweep's summed wall time is 169 s before and 163 s after.

  The member-dispatch group falls from 4,774 to 2,684, the object-typed and
  structural rest (`get`, `map`, `filter`, `preventDefault`,
  `getBoundingClientRect`).
- **IR library tests** (304) and **backend library tests** (774) pass.
