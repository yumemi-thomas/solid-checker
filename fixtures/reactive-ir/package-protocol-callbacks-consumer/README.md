# A consumer of non-call `callbacks` items

Pins the consumer's half of item A of
`docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md` § 3.3: an
accepted contract whose closed `callbacks` enumeration includes **non-call**
items -- an `invoke` stating `protocol: get` or `protocol: coerce` -- leaves
nothing open at the import, where the same export with `callbacks` left open
raises `SC9005`.

`App.tsx` calls three exports:

| component | export | `callbacks` | verdict |
| --- | --- | --- | --- |
| `Counter` | `access` | closed over `call 0`, `get 0` | clean, in the component body and in JSX |
| `Counter` | `compare` | closed over `coerce 0`, `coerce 1` | clean, in the component body and in JSX |
| `CounterOpen` | `accessOpen` | open (the same two items, partial) | `SC9005` for `callbacks` |

A non-call item is projected with its protocol and kept, so the domain stays
closed and re-emission republishes it, but no consumer pass reads it as an
invocation of the argument (`ContractCallback::is_invocation`): a property read
or a coercion of the caller's value runs that value's own code, at the call, on
the caller's stack, in the caller's tracking context, and a non-callable
argument used that way raised no obligation before either. The call item of
`access` is the ordinary inline row, so `count` is still read inline.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-plain-return-consumer` does and for the same reasons.
The document states its claims rather than proving them;
`../../package-contracts/implementation-census-described-accessor` is the
census that earns them. Unauthorized, the fixture reports five `SC9011`
findings instead (no contract describes the three exports), so the
authorization is what is under test. A decoder that predates the `protocol`
field refuses the document outright.

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-plain-return-consumer`'s, so the closure digest and package integrity
in the import block are the ones that fixture already pins. `index.d.ts` is
outside the closure and declares `access` and `compare` exactly as
`@solid-primitives/utils@7.0.0-next.4` does; every call in `App.tsx` is
`tsc`-clean against it. `solid-js.d.ts` is `package-callback-consumer`'s, whose
`createSignal` returns an accessor as the real package's does, and
`node_modules/solid-js` is required because an authorized fixture is analyzed
from a copy at a different depth.
