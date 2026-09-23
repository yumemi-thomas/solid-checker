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
| `trackEach` | `createTrackedEffect` | open, `effect` withheld | the walk is clean (the row is audited), but the call registers a computation on the caller's owner |
| `cleanUp` | `onCleanup` | proposed | the control: its requirement is published as a `cleanups` item |
| `runUnder` | `runWithOwner` | proposed | 2026-09-23 audit § 1, keyed on `@solidjs/signals` |
| `makeContext` | `createContext` | proposed | same audit § 2, `solid-js`' own |
| `readContext` | `useContext` | proposed | same audit § 3, `solid-js`' own |
| `currentOwner` | `getOwner` | proposed | a returned call is demanded a resolved call |
| `hasOwner` | `getOwner` | declined, `dialect-silent` | a call with no argument, not returned, has no resolved call to name its package |

## `trackEach`: why `creates` stays open

A consumer reads a closed `creates` as "no owner requirement beyond the
published items" (`project_owner_requirements`). This generation withholds an
`Effect` requirement, because version 1 has no domain for it
(`semantic-model.md` § creates), so a closed `creates` beside a withheld one
would tell a consumer the export needs no owner. Until 2026-09-23 only the walk
kept the domain open, by declining a call it had no row for, and
`createTrackedEffect` has one. Built without `requirements_published` in
`inferred_contract.rs`, this export proposes `creates` closed.

## Stubs

Both stubs carry version `2.0.0-rc.3`; `solid-js`' selects the Solid 2.0
dialect. Every signature is byte-faithful to the published declarations, which
each stub's header cites by file and line. Two types are trimmed, and no claim
here reads either: `Owner` keeps two of its internal members, and `Context`
keeps its two data members and its callable provider shape with its props and
result types reduced.
