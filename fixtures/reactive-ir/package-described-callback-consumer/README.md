# A consumer of a described callable's callback items

Pins the consumer's half of ADR 0152: an accepted contract whose closed
`returns` hands back a described callable that runs the export's own
arguments, and whose closed `callbacks` keeps those arguments at
`result-access`, places a callback written at such an argument wherever the
returned function is called.

`App.tsx` passes a callback that reads a signal to `pipe`, declared exactly as
`@solid-primitives/utils@7.0.0-next.4` declares it, and uses what it returns:

| component | use of the returned function | contract | verdict |
| --- | --- | --- | --- |
| `UntrackedPipe` | called in the component body | `callbacks` closed at `result-access`, `returns` a described callable running both arguments once | `SC1001` at the read inside the first callback: it runs untracked in the body |
| `TrackedPipe` | called inside JSX | the same | clean: the read runs tracked |
| `UnusedPipe` | never called | the same | clean: nothing runs the callback |
| `OpenPipe` | called in the component body | the same with `callbacks` open | `SC9005` at the call; the read is of unproven timing, never a proven untracked read |

Before ADR 0152 a callback handed to a contract export whose row for the slot
is not `inline` was of unproven timing wherever the returned value ran, which
is what `OpenPipe` still shows.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-described-callable-consumer` does. The document states
its claims rather than proving them;
`../../package-contracts/implementation-census-described-callbacks` is the
census that earns `pipe`'s, against `pipe`'s published bytes.

## Stubs

`node_modules/reactive-package/package.json`, `node_modules/solid-js` and
`node_modules/@solidjs/signals` are byte-identical to
`package-described-callable-consumer`'s, so the closure digest and package
integrity in the import block are the ones that fixture already pins.
`index.d.ts` is outside the closure. Every call in `App.tsx` is `tsc --noEmit`
clean against it (TypeScript 5.9.3), and the same file with `pipe` imported
from the published `@solid-primitives/utils@7.0.0-next.4` beside
`solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` is clean too.
