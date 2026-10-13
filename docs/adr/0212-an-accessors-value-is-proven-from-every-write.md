# ADR 0212: An accessor's value is proven from every write

- Status: accepted and implemented (2026-10-06). Phase 2 of the value-flow
  work on caller-supplied member dispatch.
- Owners:
  - `accessor_origin`, `setter_writes_builtin`, `returns_builtin` and the
    origins `value_origin_assuming` adds (`solid-reactive-ir/src/indexes.rs`);
  - `AstFacts::array_literals` and `ReturnFact::runtime_value_kind`
    (`solid-facts/src/ast/mod.rs`).
- Relation: extends ADR 0211's origins from fresh values to values held by
  signals, memos and project functions.

## Context

ADR 0211 proves that an argument is a fresh built-in value when its origin is
written at the call or bound by a `const`. Most remaining arguments are
accessor calls (`items()`, `sorted()`) or calls of project functions
(`parseItems(text)`), so their value is whatever the accessor or the
function holds or returns.

## Decision

A value is also a proven built-in value when it is one of the following.

1. **An accessor of a signal**, `items()`. The tuple is `const [items,
   setItems] = createSignal(initial)`, declared in a function and read in
   its own file. Every value the signal can hold is proven:
   - the initial value is proven and is not a function (`createSignal(fn)` is
     a writable memo);
   - every reference to the setter in its file is the callee of a call. The
     binder's resolution is complete for the file, so a setter put in an
     object, passed as a prop or returned is found, and the proof fails;
   - every write is proven: a written value, or an updater whose every return
     is proven with its previous value assumed proven. The assumption is the
     induction step from a proven initial value.
2. **An accessor of a memo**, `sorted()`. It is `const sorted = createMemo(fn)`
   with `fn` a synchronous function literal, and every value `fn` returns is
   proven.
3. **A plain call of an exact project function**, where every value its
   synchronous body returns is proven. A returned parameter proves nothing.
   A method call is not followed, because its receiver selects it.
4. **`a ?? b`, `a || b`, `a && b` and `c ? a : b`**, where both operands are
   proven.
5. **An array literal anywhere**, and `null` or an unshadowed `undefined`,
   which have no members to run. Array literals are now a fact of their own,
   and returns carry their runtime kind.
6. **A member of a proven receiver.** `sort`, `reverse`, `fill` and
   `copyWithin` return their receiver. `map`, `filter`, `slice`, `concat`,
   `flat`, `flatMap`, `toSorted`, `toReversed`, `toSpliced`, `with` and
   `split` return a fresh value of a reviewed class. Name and receiver are
   enough here, and no declaration is needed. ADR 0211's checks still apply:
   nothing assigns the member, and nothing assigns `constructor` for a
   species-built result.

## Consequences

- An accessor or helper whose every value is built in the program selects
  the built-in members at a parameter-member call site.
- Still open:
  - values that cross a component boundary (`props.items`) or arrive as
    parameters;
  - store members;
  - accessors declared at module level or read from another file;
  - a memo's previous value;
  - method calls on project objects.

## Evidence

- **Fixture** `fixtures/reactive-ir/accessor-value-origin`:
  - the dispatch obligation leaves for an updated signal, a sorted memo, a
    derived function, a helper and a `?? []` fallback over a memo;
  - it stays for an escaped setter, a written store array, a writable memo,
    a memo returning a store array, and a helper returning its parameter.
- **Coverage:** 181 fixture projects, 970 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-p-browser.json` (ADR 0211):
  - violations unchanged at 285;
  - uncertifiable 3,386 to 3,375: 10 sites removed, none added.
  - For example, probus-hk's `takenAt` is `null` or `new Date()` on every
    write, so `clockTime(at)`'s `toLocaleTimeString` is `Date`'s.
