# A consumer of described callables

Pins the consumer's half of ADR 0145 and ADR 0146: an accepted contract whose
closed `returns` hands back a **described callable** -- a callable together
with what one invocation of it does -- leaves nothing open at the import, and
the consumer projects the nested claims onto the call site of the returned
value.

`App.tsx` calls three exports:

| component | call | `returns` of the export | verdict |
| --- | --- | --- | --- |
| `Ids` | `createIdGenerator()()` in the component body | closed: a described callable that reads nothing and returns `plain` | clean |
| `IdsOpen` | the same | open (the same return, partial) | `SC9005` for `returns` |
| `UntrackedCount` | `createCounter()()` in the component body | closed: a described callable that reads an owned signal and returns the value read | `SC1001`: calling it is an untracked reactive read |
| `TrackedCount` | `createCounter()()` inside JSX | the same | clean: the read is tracked |

A described callable that reads nothing names no reactive leaf, so calling it
anywhere asks nothing of the caller. One that reads a signal is what the
consumer calls an accessor (`contracts.rs` `project_return_shape`): a returned
accessor stays an accessor, and the consumer's own rules decide where reading
it is a misuse.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-member-returns-consumer` does. The document states its
claims rather than proving them;
`../../package-contracts/implementation-census-described-callables` is the
census that earns `createIdGenerator`'s.

## Stubs

`node_modules/reactive-package/package.json`, `node_modules/solid-js` and
`node_modules/@solidjs/signals` are byte-identical to
`package-member-returns-consumer`'s, so the closure digest and package
integrity in the import block are the ones that fixture already pins.
`index.d.ts` is outside the closure and declares `createIdGenerator` exactly as
`@solid-primitives/utils@7.0.0-next.4` does (`() => () => string`) and
`createCounter` as a signal accessor is typed (`() => () => number`). Every
call in `App.tsx` is `tsc --noEmit` clean against it (checked with TypeScript
5.9.3).
