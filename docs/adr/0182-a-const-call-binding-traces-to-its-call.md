# ADR 0182: A `const` call binding traces to its call

- Status: accepted and implemented (2026-10-05). Sixth lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners: the Type Facts producer's return value sources
  (`returnValueSourcesLocked`).
- Relation: extends the single-hop binding trace that followed array slots
  (`const [a] = f()`) to a plain binding (`const x = f()`). No wire change.
  The producer's source identity changes, so `bin/solid-typefacts` is rebuilt.

## Context

A returned identifier is traced to the call that produced it in exactly one
shape: a slot of an array binding pattern initialized by a call. The most
common hook shape was missing: bind a memo, then return it.

```js
const page = createMemo(() => Math.max(1, Math.min(rawPage(), opts().pages)));
return [paginationProps, page, setPage];
```

Its member had "no exact primitive, caller parameter or owned accessor
evidence", so ADR 0177 left it `unknown`. A read of `page()` at a component's
top level went unreported. `createPagination` and `createClipboard` (`const
clipboard = createMemo(…)`) are the ledger's two cases.

## Decision

The trace accepts a single variable declaration whose name is an identifier
and whose initializer is a call, under the array branch's premises:

- exactly one declaration;
- never an assignment target, by the checker's own assignment-target symbols;
- the reference sits after the declaration's end, in the same file.

The source is the call's whole result, with no target path. Destructuring
patterns other than an array slot stay untraced.

## Consequences

- A traced whole result is evidence only where a dialect row states it. A
  `createMemo` result is the computed accessor row's. A whole `createSignal`
  result is a tuple, which no accessor row states, so it still refuses.
- `let` or `var` bindings qualify only under the same never-reassigned
  premise as the array branch.

## Evidence

- Producer test `TestReturnValueSourcesTraceAPlainConstCallBinding`: a `const`
  binding traces to its call's whole result; a reassigned `let` does not. The
  whole producer suite passes.

- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 73 to 74 static violations of 123:
    `createPagination`, runtime-detected (`STRICT_READ_UNTRACKED`);
  - no correct twin is flagged, no checkpoint row changes status, and no
    export loses a returned-accessor claim.
- `createClipboard`'s memo member is now proven, but its return is still
  withdrawn: the synthesized structural veto has no sample that completes
  normally in the probe environment (`navigator.clipboard` is absent).
- Coverage, corpus and the 38-app sweep are unchanged.
