# A consumer of non-call `callbacks` items

Pins the consumer's half of item A of
`docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md` § 3.3: an
accepted contract whose closed `callbacks` enumeration includes **non-call**
items -- an `invoke` stating `protocol: get` or `protocol: coerce` -- leaves
nothing open at the import, where the same export with `callbacks` left open
raises `SC9005`.

`App.tsx` exercises these exports:

| component | export | `callbacks` | verdict |
| --- | --- | --- | --- |
| `Counter` | `access` | closed over `call 0`, `get 0`; invocation `min: 0` | SC1001 uncertifiable in the component body; clean in JSX |
| `Counter` | `compare` | closed over `coerce 0`, `coerce 1` | clean, in the component body and in JSX |
| `CounterOpen` | `accessOpen` | open (the same two items, partial) | `SC9005` for `callbacks` |
| `Handler` | `access` | closed over `call 0`, `get 0` | clean in the event handler and for the static argument |
| `Spread` | `access` | closed over `call 0`, `get 0` | no accessor read inferred from a spread slot |
| `NamespaceRead` | `primitives.access` | namespace binding not admitted by this fixture receipt | SC9011; no accessor read inferred from an unaccepted binding |
| `WrapperRead` | `access` | closed over `call 0`, `get 0`; invocation `min: 0` | SC1001 uncertifiable; `satisfies` preserves exact accessor identity |
| `Shadowed` | a local `access` | no package export called | clean; storing the accessor is not invoking it |
| `GuaranteedRead` | fixture-only `accessAlways` | ambient inline invocation `min: 1`, scope `call` | SC1001 violation in the body; clean in JSX |

A non-call item is projected with its protocol and kept, so the domain stays
closed and re-emission republishes it, but no consumer pass reads it as an
invocation of the argument (`ContractCallback::is_invocation`): a property read
or a coercion of the caller's value runs that value's own code, at the call, on
the caller's stack, in the caller's tracking context, and a non-callable
argument used that way raised no obligation before either. The call item of
`access` is the ordinary inline row, so passing `count` itself may read it during
the call even though that accessor has no project function summary. The call
site determines tracking. The `get` and `coerce` rows contribute no accessor
invocation. `package-described-callback-consumer` also pins that passing a bare
accessor to a result-access row does not read it during construction. The
optional invocation exposes a proof obligation, not a proven violation: domain
closure does not strengthen its cardinality. `accessAlways` is a fixture-only
cardinality control, not a claim about the published `utils.access` export.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-plain-return-consumer` does and for the same reasons.
The document states its claims rather than proving them;
`../../package-contracts/implementation-census-described-accessor` is the
census that earns them. Without authorization, the fixture has no accepted
package behavior for these exports, so authorization is what is under test.
A decoder that predates the `protocol`
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
