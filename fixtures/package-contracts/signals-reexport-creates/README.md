# signals-reexport-creates

The generator's `creates` proposal on the path real packages take: every callee
is imported from `solid-js`, and four of them resolve through its re-export
into `@solidjs/signals`. The walk asks the audits about the package that
*declares* a callee (`creates_walk.rs`), so this is the only corpus fixture on
which the `@solidjs/signals` rows are reached; every other one declares its
primitives in a `solid-js` stub.

It is a generator-corpus fixture (`corpus.json`) and nothing here is certified.
What it pins is each export's `creates` state in `expected.json` and the records
in `expected-refusals.json`.

| export | callee | `creates` | why |
| --- | --- | --- | --- |
| `trackEach` | `createTrackedEffect` | proposed, beside one `compute` | the walk is clean (the row is audited), and the call registers a computation on the caller's owner, which is stated |
| `cleanUp` | `onCleanup` | proposed | the control: its requirement is published as a `cleanups` item. Its callback row is `queued` and `ambient-at-execution`: the dialect states nothing about the listener a cleanup runs under |
| `runUnder` | `runWithOwner` | proposed | 2026-09-23 audit § 1, keyed on `@solidjs/signals` |
| `makeContext` | `createContext` | proposed | same audit § 2, `solid-js`' own |
| `readContext` | `useContext` | proposed | same audit § 3, `solid-js`' own |
| `currentOwner` | `getOwner` | proposed | a returned call is demanded a resolved call |
| `hasOwner` | `getOwner` | proposed | an argumentless primitive call is demanded its resolved call (`demand_plan.rs`); before that it declined as `dialect-silent`, with no declaration to name its package |

## `trackEach`: why `creates` waits for the requirement

A consumer reads a closed `creates` as "no owner requirement beyond the
published items" (`project_owner_requirements`), so `creates` may close only
where every requirement the export has is stated. Until ADR 0114 this
generation withheld the `Effect` requirement, because version 1 had no domain
for it, and only the walk kept `creates` open beside it, by declining a call it
had no row for — which `createTrackedEffect` has. Built without
`requirements_published` in `inferred_contract.rs`, the export proposed
`creates` closed beside a withheld requirement. Since ADR 0114 the requirement
is a `compute` in `computations`, so the closure states everything and is
proposed; the gate still holds it open for a `Boundary` requirement or an
owner census that did not decide.

## Stubs

Both stubs carry version `2.0.0-rc.3`; `solid-js`' selects the Solid 2.0
dialect. Every signature is byte-faithful to the published declarations, which
each stub's header cites by file and line. Two types are trimmed, and no claim
here reads either: `Owner` keeps two of its internal members, and `Context`
keeps its two data members and its callable provider shape with its props and
result types reduced.
