# ADR 0149: A built-in is named by identity, not by a binding's type

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the Type Facts producer's call census
  (`ImplementationCall::standard_library_identity`,
  `standard_library_identity.go`; handshake protocol 68), the policy-2 call
  walk's `StandardLibrary` disposition and its coercion premise
  (`type_facts.rs`: `census_call_disposition`,
  `census_coercion_rests_on_primitive_calls`,
  `completion_is_primitive_by_evidence`)
- Relation: the two remaining instances of the type-trust hole the 2026-09-28
  amendment to ADR 0113 closed for `plain` returns. It narrows the
  `StandardLibrary` disposition (census plan § 3, ADR 0034) and ADR 0045's
  coercion premise; neither ADR's other premises change.

## Context

The amendment to ADR 0113 established that, in a JavaScript file, a checker
type read off a binding is only that binding's *declaration*: an unchecked
write does not change it. `let x = 0; … x = () => 1; return x` is typed
`number`. Two other premises read the same kind of type.

1. **The call walk's `StandardLibrary` disposition** admitted a call whose
   callee *resolved* to a default-library declaration. Resolution follows the
   receiver's type, so `let m = Math; … m = { min: run }; m.min(1)` resolved
   to `Math.min` -- a member the census treats as running no user code -- and
   ran `run`. The same holds for any instance method: `let s = ""; … s = {
   split: run }; s.split(",")` resolves to `String.split`.
2. **ADR 0045's coercion premise** cleared `helper(v) + 1` when the helper's
   transcript stated `primitiveCompletion`, the checker's return type for the
   helper. In a JavaScript file that type is inferred from return expressions
   typed by their bindings' declarations, so `function helper() { let v = 0; …
   v = { valueOf: run }; return v; }` is `number`, and the coercion reaches
   `run`.

## Decision

### The call walk names a built-in only by identity

The producer states `ImplementationCall::standard_library_identity` (protocol
68) beside a default-library `declaration` exactly when the value the call
invokes is that declaration by identity, never by a binding's type. The
callee's receiver must be one of:

- **nothing** -- an identifier callee (`setTimeout(…)`, `new Map()`) that
  resolves to default-library declarations alone;
- **a default-library global** -- a non-computed, non-optional member chain
  (`Math.min`, `Object.prototype.toString.call`) whose root and every member
  resolve to default-library declarations alone;
- **a fresh built-in value the expression itself makes**, whose own member is
  read before any other code runs: a primitive by grammar (which reaches a
  never-written `const` over one), an array or regular-expression literal,
  `new G(…)` of a built-in constructor by identity (`new Intl.Locale(x)`), or
  a call that itself invokes a built-in by identity and whose
  *uninstantiated* signature declares a primitive or an array result
  (`Date.now().toString(36)`, `Array.from(items).forEach(f)`; `[x].at(0)` does
  not qualify, since its `number` would be only `x`'s declared type);
- **a binding that holds only such values and that no other code can reach**
  -- declared once in the reading file with a plain name, initialized by a
  fresh value, written only by a plain `=` of another, and otherwise
  referenced only as the object of a non-computed member read that is neither
  written nor deleted (`const values = Array.from(items); values.map(f)`;
  `createCallbackStack`'s `let stack = []; … stack = []; stack.push(…)`).
  `return values`, `f(values)`, `values[0] = x` and `values.map = f` each
  make it none.

For every one, no default-library symbol the chain names may be *unstable* in
the file: written or deleted as a global or as a member (`setTimeout = f`,
`Math.min = f`), or -- for a global -- handed out as a value (`f(Math)`, `const
M = Math`), since the receiver may replace a member. A global used as the
object of a member read, the callee of a call or `new`, the right operand of
`instanceof` or the operand of `typeof` hands nothing out
(`unstableLibrarySymbolsLocked`, one walk per file). A package that patches a
built-in from *another* module is the default-library trust ADR 0103 already
states.

A binding, a parameter, a member read, and any other call result state
nothing. The census admits the `StandardLibrary` disposition only with the
fact, or for a call ADR 0103's immutable-alias walk rebound through a chain
that already proves the same identity; otherwise the call refuses by name
("resolves to it only through the declared type of the value it is read
from"). A call whose callee is rooted at a parameter is unaffected: it is the
caller's callable and dispositioned `ParameterRooted` before this arm.

### A helper's completion is a primitive only by evidence

`completion_is_primitive_by_evidence` replaces the bare `primitiveCompletion`
read: the helper's type must still say primitive, over a plain, classified
completion, and every live value it hands back must be one by evidence the
type cannot fake:

- a primitive by grammar, which holds whatever the operands are
  (`originPoint + scaled`);
- the helper's own unwritten whole parameter (`ReturnSite::parameter`), when
  the argument at the coerced call is an unwritten whole parameter of the
  export itself, at census depth 0, whose type is ADR 0038's declared-signature
  premise (`scaleBy(base) + base`); a deeper frame's argument is only as good
  as its own caller's and is refused;
- a primitive type with the amendment's evidence beside it
  (`plain_return_evidence`).

A helper that returns no value completes with `undefined`, which is one.

## Consequences

- Pinned: the producer facts by `TestDefaultLibraryCallsStateTheirIdentity`
  (30 cases: the global, fresh-value and fresh-binding receivers, and the
  reassigned alias, reassigned string, escaped const, written member, written
  element, handed-out global, returned const and instantiated-result
  refusals); the call walk's refusal by name in
  `creates_census_admits_a_standard_library_callee_only_without_an_unseen_callable`;
  the helper evidence in
  `creates_census_grants_a_coercion_only_from_the_callees_own_completion`.
  `implementation-census-creates`' `callNonLibraryReceiver`
  (`identity.call(value)`) now refuses at this arm, before the by-reference
  owner rule; every other tracer there keeps its verdict.
- Contract corpus (111 fixtures) and coverage (131 projects) move nothing.
- `make contract-coverage-census`, against the pin last written at 663e9021:
  the consumer view's "nothing open" 691 -> 694 and "every import" 294 -> 258;
  the one gated regression, owner-requirement sites 37 -> 32, is
  `@solid-primitives/scheduled`'s `debounce` and `throttle`, whose only owner
  operation is the `onCleanup` behind `if (getOwner())` that 7b57b487 stopped
  stating as required. The pin is rewritten. Probe-recipe addressing is
  unchanged (95 of 366, 15 by recipe address, 119 stale), so no recipe was
  orphaned and none is carried over.
- What this ADR withdraws in that run, by the member it refuses to name:
  `@kobalte/core`'s `isRTL` and `getReadingDirection` (`RTL_SCRIPTS.has`, a
  module `Set` the file reads elsewhere as a value) and `resolveRichTemplate`
  (`@solid-primitives/i18n`'s `push` on a returned array), `@kobalte/utils`'
  `getPrecision`, `roundToStepPrecision`, `snapValueToStep` (`indexOf` on the
  string a parameter's `toString()` returned) and `debugPolygon` (`join` on a
  parameter's `map`), `@solid-primitives/trigger`'s `createTriggerCache`
  (`bind` of a user class's method) and `@solid-primitives/utils`'
  `removeItems` (`push` on the array it returns). Each is a receiver whose
  built-in the census could only name by type.

## What still refuses, and what still trusts a type

- A receiver built by the caller's own code (`step.toString().indexOf(…)`), a
  fresh value that escapes by `return` before a nested closure could use it,
  and a module-scope collection read as a value anywhere in the file. Each is
  sound to refuse; each would need its own evidence to return.
- A coercion helper at census depth 1 or more that returns its parameter: the
  argument's premise is only its caller's, and the chain is not followed.
- **Still trusted, the same class of hole on the producer side:** the
  coercion *form* census omits a coercion whose operand is typed non-object
  (`coercionFormLocked`), so `let v = 0; … v = { valueOf: run }; return v - 1`
  records no form for the census to refuse. Closing it records a coercion for
  every member read and binding typed a primitive, which the census would then
  refuse unless it proved the operand; it is recorded here for the owner, not
  changed.
