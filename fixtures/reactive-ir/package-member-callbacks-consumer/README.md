# A consumer of member-path `callbacks` items

Pins the consumer's half of item B of
`docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md` § 3.3: an
accepted contract whose closed `callbacks` enumeration includes a **call of a
member of an argument** -- an `invoke` item `from: {arg: 1, path: ["0"]}`, the
`handler[0](handler[1], event)` of `@kobalte/utils`' `callHandler` -- folds the
member's value as an inline call exactly when the call's own syntax names it,
and never reads the item as a call of the argument itself.

`App.tsx` calls three exports:

| component | call | `callbacks` of the export | verdict |
| --- | --- | --- | --- |
| `Direct` | `readCount()` | -- | `SC1001`: the reference untracked read |
| `Handler` | `callHandler(event, readCount)` | closed: call 1, call 1 at `[0]`, get 0, get 1 | `SC1001`, through the call item of argument 1 |
| `HandlerPair` | `callHandler(event, [readCount, 1])` | the same | `SC1001`, through the member item: element 0 of the literal is `readCount` |
| `BoundPair` | `callBoundOnly(event, [readCount, 1])` | closed: call 1 at `[0]`, get 1 | `SC1001`, through the member item |
| `BoundWhole` | `callBoundOnly(event, readCount)` | the same | clean: the package never calls argument 1 itself |
| `BoundStored` | `callBoundOnly(event, stored)` | the same | clean: the call does not show what `stored` holds at 0 |
| `BoundOpen` | `callBoundOnlyOpen(event, readCount)` | open (the same two items, partial) | `SC9005` for `callbacks` |

Each `SC1001` is the one an inline call of `readCount` at that site would
raise: the member item is an `inline` row of the member's value, so its reads
are attributed to the call exactly as `readCount()` attributes them.

`BoundWhole` is the regression the item's path exists to prevent. Before the
consumer carried the path (`contracts.rs` `project_callbacks` dropped it), the
item read as "argument 1 is invoked inline", and `readCount` passed whole was
folded as a call the package never makes. `BoundStored` is the stated limit:
a member the call's own array or object literal does not name is not folded
at all (`contract_callback_invoked_value`), which is what an inline row whose
argument resolves to nothing has always done, and before item B the export
could not close `callbacks` for a consumer to read any row of it. `BoundOpen`
is `BoundWhole`'s control: with `callbacks` open the package may call the
function it was handed.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-protocol-callbacks-consumer` does. The document states
its claims rather than proving them;
`../../package-contracts/implementation-census-member-callee` is the census
that earns `callHandler`'s. Unauthorized, the fixture reports only `Direct`'s
`SC1001`, so the authorization is what is under test.

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-protocol-callbacks-consumer`'s, so the closure digest and package
integrity in the import block are the ones that fixture already pins.
`index.d.ts` is outside the closure and declares `callHandler` as
`@kobalte/utils@2.0.0-alpha.0` does, with `@solidjs/web`'s
`JSX.EventHandlerUnion` reduced to its function and `[function, data]` shapes;
every call in `App.tsx` is `tsc`-clean against it. `solid-js.d.ts` is
`package-callback-consumer`'s, whose `createSignal` returns an accessor as the
real package's does, and `node_modules/solid-js` is required because an
authorized fixture is analyzed from a copy at a different depth.
