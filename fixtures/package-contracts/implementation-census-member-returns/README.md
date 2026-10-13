# implementation-census-member-returns

The tracer for item B round 2 of
`docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md` § 3.3: a
`returns` enumeration for **a property of the caller's argument, or
undefined** -- one `return` whose output is `parameter i` at a one-segment path
`[key]`, the value the caller's argument holds at that key when the return reads
it, and, for an optional chain, one whose output is exactly `undefined`.

It rests on handshake protocol 64: the producer decomposes a returned non-call
property access or literal-keyed element access of an unwritten parameter into
one arm carrying the member's path (`handler[0]` is tuple `0`, `h["run"]` is
property `run`, rooted exactly as protocol 63 roots a callee), and an optional
chain into that arm and a second, `undefined`, arm. The census reads those arms
under ADR 0115's premises unchanged.

It is two fixtures in one. As a generator-corpus fixture (`corpus.json`) its
`expected.json` pins what the generator derives from its own walk: a member
return for a returned member of a parameter's own identifier, beside
`undefined` when the link is optional. As a certification tracer
(`the_member_returns_census_certifies_exactly_the_enumerated_members` in
`contract_certification.rs`) it is planned with each export's containers set by
hand, including claims the walk would not make, and certified against the real
producer with the synthesized container veto.

| export | generator proposes `returns` | tracer claims | verdict |
| --- | --- | --- | --- |
| `callHandler` | `parameter 0 ["defaultPrevented"]`, `undefined` | the same | certified |
| `readKey` | `parameter 0 ["key"]` | the same | certified |
| `firstOrUndefined` | `parameter 0 ["0"]`, `undefined` | the same | certified |
| `stringKey` | `parameter 0 ["run"]` | the same | certified |
| `keyOrSelf` | `parameter 0`, `parameter 0 ["key"]` | the same | certified |
| `writtenMember` | `parameter 0 ["key"]` | the same | certified: the claim is what the member holds at return time |
| `computedKey` | `plain` (the walk names no member for `options[key]`) | `parameter 0 ["key"]` | withheld: neither the caller's unchanged argument, a literal member of one, ... |
| `writtenBinding` | `parameter 0 ["key"]` | the same | withheld: the binding is written, so the producer states no arm |
| `longerPath` | `plain` (the receiver is a member) | `parameter 0 ["inner"]` | withheld: reads the argument through 2 member segments |
| `memberCall` | `plain` | `parameter 0 ["key"]` | withheld: a call of the member is not the member |
| `readBeforeWrite` | `plain` | `parameter 0 ["key"]` | withheld: the returned `before` is no member read |
| `overclaimedUndefined` | `parameter 0 ["key"]` | that, and `undefined` | withheld: no completion hands back `undefined` |

`callHandler` is `@kobalte/utils@2.0.0-alpha.0`'s `dist/index.js` bytes, as in
`../implementation-census-member-callee`. `index.d.ts` declares it as that
package does, with `@solidjs/web`'s `JSX.EventHandlerUnion` inlined and the
DOM's `Event` and `Element` replaced by local stand-ins so the declarations
resolve without the DOM library; the return is the published `boolean`. Every
other export is declared as what it returns, and `tsc --noEmit --strict`
accepts the file.

## What the veto observes

The synthesized container veto samples a claimed member's slot with an object
whose member at the claimed key holds a fresh token, and, where the claim
enumerates `undefined`, with `undefined` and `null`. A completion holds when it
is, by SameValue, what the argument holds at the key once the call has returned
(read once with the member operator after it), or `undefined` for a claimed
`undefined`, or any other claimed container. Reading the member after the call
rather than comparing with the token is what the model says -- the member at
return time -- and is why `writtenMember` holds. A set naming neither shape is
synthesized byte for byte as before.

## What still refuses

A member two segments deep, a computed key that is not a literal, a call of the
member, a member of a written binding, and a value read from the member before
the return: each withholds its claim by name. A consumer reads a member return
as naming no reactive leaf (`../../reactive-ir/package-member-returns-consumer`).
