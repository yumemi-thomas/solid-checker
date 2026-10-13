# A consumer of member returns

Pins the consumer's half of item B round 2 of
`docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md` § 3.3: an
accepted contract whose closed `returns` enumeration hands back **a member of
the caller's argument** -- `parameter 0` at `["defaultPrevented"]`, beside the
exact `undefined` an optional chain short-circuits to, as `@kobalte/utils`'
`callHandler` returns `event?.defaultPrevented` -- leaves nothing open at the
import, and the member is never read as a reactive leaf.

`App.tsx` calls four exports:

| component | call | `returns` of the export | verdict |
| --- | --- | --- | --- |
| `Handler` | `callHandler(event, noop)` | closed: `parameter 0 ["defaultPrevented"]`, `undefined` | clean |
| `HandlerOpen` | `callHandlerOpen(event, noop)` | open (the same two returns, partial) | `SC9005` for `returns` |
| `PassThrough` | `passThrough(count)()` | closed: `parameter 0` (ADR 0075) | `SC1001`: the consumer resolves the whole argument exactly |
| `MemberRead` | `readKey({ key: count })()` | closed: `parameter 0 ["key"]` | clean, and deliberately so |

`MemberRead` is the stated limit, in the hiding direction. The call does return
`count`, and calling it is the untracked read `PassThrough` reports. But a
member return is what the argument holds at that key **when the return reads
it**, and code the call runs before then -- its own body, or a callback it
invokes: `callHandler`'s handler calls `preventDefault()` on the very event
whose `defaultPrevented` it returns -- may have replaced it, so the literal at
the call site does not determine it. The consumer resolves it nowhere
(`contracts.rs` `project_returned_output`), reads the claim as no reactive
return, and never as the argument itself, which is what reading its
`parameter` alone would say. Contract returns are only ever read to *find* a
reactive leaf, so this hides one and never invents one; the projection is
pinned by `argument_containers_project_as_their_one_leaf_or_as_no_reactive_return`.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-member-callbacks-consumer` does. The document states its
claims rather than proving them;
`../../package-contracts/implementation-census-member-returns` is the census
that earns `callHandler`'s and `readKey`'s.

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-member-callbacks-consumer`'s, so the closure digest and package
integrity in the import block are the ones that fixture already pins.
`index.d.ts` is outside the closure and declares `callHandler` as
`@kobalte/utils@2.0.0-alpha.0` does, with `@solidjs/web`'s
`JSX.EventHandlerUnion` reduced to its function and `[function, data]` shapes;
`readKey` and `passThrough` are declared as what they return. Every call in
`App.tsx` is `tsc --noEmit` clean against it. `solid-js.d.ts` is
`package-callback-consumer`'s, whose `createSignal` returns an accessor as the
real package's does, and `node_modules/solid-js` is required because an
authorized fixture is analyzed from a copy at a different depth.
