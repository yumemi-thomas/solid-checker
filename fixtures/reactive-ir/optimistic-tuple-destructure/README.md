# Destructuring 2.0's optimistic tuples binds a reactive source

ADR 0111 replaced a hardcoded `CreateSignal | CreateStore | CreateResource` in
`source_discovery.rs` — 1.x's list, carried in shared code — with
`Dialect::returns_reactive_tuple`. The old list named `createResource`, which
2.0 does not have, and named neither `createOptimistic` nor
`createOptimisticStore`, which it does. The ADR recorded the fix as "correct,
and exercised by nothing", and said a fixture for it "needs a rule whose finding
depends on source recognition, which is its own piece of work."

This is that fixture. The rule is `SC1001 strict-read-untracked`: it can only
report a read it can trace to a source, so a source the discovery pass does not
recognise produces silence rather than a wrong answer — the failure shape a
snapshot of *expected* findings catches and a passing build does not.

## The two forms are different paths, and only one was broken

| File | Form | Guard |
| --- | --- | --- |
| `App.tsx` | `const [count] = createOptimistic(0)` | `Dialect::creates_reactive_source` |
| `Assign.tsx` | `let count!: …; [count] = createOptimistic(0)` | `Dialect::returns_reactive_tuple` |

Only the second was ever wrong. `creates_reactive_source` has always named the
optimistic primitives, so the declaration form worked throughout; the assignment
form went through the 1.x list and bound nothing.

Both are here on purpose. `App.tsx` is what makes `Assign.tsx` mean something
specific: when the optimistic entries are dropped from `returns_reactive_tuple`,
`Assign.tsx` loses exactly `count` and `row.label` while `App.tsx` keeps all
three. Without that control a regression anywhere in destructuring would look
the same.

`createSignal` (both files) and `createStore` (`Assign.tsx`) are the other
controls: both were in the old list, so a regression that took out the whole
path would show on them too.

## Why each function reads twice

Every function reads its source in the `createEffect` compute and again in the
apply callback. The compute is tracked and silent; the apply is not tracked and
is the SC1001. Pairing them means a fixture that stopped analyzing altogether
does not look the same as one where the source stopped being recognised —
the compute-phase halves are silent either way, but the file's findings going
from four to zero and from four to two are different diffs.

## Stub faithfulness

`solid-js.d.ts` is reduced, and what it reduces cannot manufacture a finding.
The published `createOptimistic` returns `Signal<T> = [get: SourceAccessor<T>,
set: Setter<T>]` and `createOptimisticStore` returns
`[get: Store<T>, set: StoreSetter<T>]`; the stub keeps **the two-slot tuple
shape and the first slot's callability or member access**, which is what the
claim depends on, and flattens the accessor brand (`SourceAccessor` is
`Refreshable<Accessor<T>>`) and the setter overload set, which it does not.
`createOptimistic`'s `value: Exclude<T, Function>` parameter is kept
byte-faithful because that constraint is what makes the plain overload
selectable at all (ADR 0110 found the 1.x spelling is a type error under it).
`createEffect`'s two overloads, including the deprecated one-argument form
returning `never`, are copied from the reference `dialect-solid-2` stub.

Both source files were type-checked against the **real audited typings** before
being written here — `bun scripts/tsc-oracle.mjs check --dialect v2 --file …`,
0 diagnostics in both the strict and loose passes. So every finding this fixture
expects is something `tsc` does not already say (AGENTS.md's absolute rule).
