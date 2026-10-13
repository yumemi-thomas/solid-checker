# A consumer of a returned memo accessor

Pins the consumer's half of ADR 0162: an accepted contract whose closed
`returns` hands back a described callable that reads an **owned memo** leaves
nothing open at the import, and the consumer treats what it returns as an
accessor -- calling it is a reactive read in whatever scope calls it.

| component | call | `returns` of the export | verdict |
| --- | --- | --- | --- |
| `UntrackedDoubled` | `createDoubled(count)()` in the component body | closed: a described callable that reads an owned memo and returns the value read | `SC1001`: an untracked reactive read |
| `TrackedDoubled` | `createDoubled(count)()` inside JSX | the same | clean: the read is tracked |
| `DoubledOpen` | `createDoubledOpen(count)()` in the component body | open (the same return, partial) | `SC9005` for `returns`, and still `SC1001`: a partial claim keeps the positive fact it states, that the return is an accessor |

The memo read is not inert (a stale read re-runs the registered computation,
which the export's own `creates` and `callbacks` claims account for), but that
is the export's business and not the reader's: what the caller's scope has to
get right is that this is a tracked read, which is exactly what `accessor` says.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-described-callable-consumer` does. The document states
its claims rather than proving them;
`../../package-contracts/implementation-census-memo-accessors` is the
generation that proposes them and the census in `type_facts.rs` earns them.

## Stubs

`node_modules/reactive-package/package.json`, `node_modules/solid-js` and
`node_modules/@solidjs/signals` are byte-identical to
`package-described-callable-consumer`'s, so the closure digest and package
integrity in the import block are the ones that fixture already pins.
`solid-js.d.ts` declares `createSignal` as `solid-js` does for a plain value.
Every call in `App.tsx` is `tsc --noEmit` clean against it.
