# implementation-census-member-callee

The tracer for item B of `docs/package-contract-v2/phase22/2026-09-24-ways-to-improve.md`
§ 3.3: a `callbacks` item for a **call of a literal-keyed member** of the
caller's argument -- `from: {arg: i, path: [key]}`, an ordinary call item at
the call event on the same stack, `ambient-at-execution`, counted per call from
zero to many.

It rests on handshake protocol 63: the producer roots a literal-keyed element
access at the parameter it extends (`handler[0]` is parameter 1 at tuple 0,
`h["run"]` is parameter 0 at property `run`), so the census sees
`handler[0](…)` as a parameter-rooted call with that path instead of an
unresolved callee.

It is two fixtures in one. As a generator-corpus fixture (`corpus.json`) its
`expected.json` pins what the generator derives from its own walk: a call item
at `[key]` for a call, in the export's own body, whose callee is a literal-keyed
member of a parameter's own unwritten binding, and the `get` of that parameter
the member read performs; and a proposed `creates` closure over such a call,
which the walk used to decline as `unresolved-callee computed-member`. As a
certification tracer
(`the_member_callee_census_certifies_exactly_the_described_member_calls` in
`contract_certification.rs`) it is planned with each export's items set by
hand, including claims the walk would not make, and certified against the real
producer with the synthesized member veto.

| export | generator proposes `callbacks` | tracer claims | `callbacks` verdict | `creates: []` |
| --- | --- | --- | --- | --- |
| `callHandler` | call 1, call 1 at `[0]`, get 0, get 1 | the same | certified | certified |
| `callBound` | call 1 at `[0]`, get 1 | the same | certified | certified |
| `stringKey` | call 0 at `["run"]`, get 0 | the same | withheld: no recipe in corpus (the synthesized veto installs members at indices only) | certified |
| `computedKey` | `[]` | call 0 at `[0]`, get 0 | withheld: the item's positive facts find no call rooted at that member | declined by the walk (`h[k]` names no member) |
| `deferredMember` | `[]` (the call is in a returned closure) | call 0 at `[0]`, get 0 | withheld: no uncaptured use of the argument at the call | certified |
| `writtenBinding` | `[]` | call 0 at `[0]` | refused: the member read's subject is a written parameter | refused, the same |
| `composeEventHandlers` | `[]` | the same | refused: the member call is `callHandler`'s, reached from a returned closure | certified (the walk reaches `callHandler` at depth 1) |

`callHandler` and `composeEventHandlers` are `@kobalte/utils@2.0.0-alpha.0`'s
`dist/index.js` bytes. `index.d.ts` declares them as that package does, with
`@solidjs/web`'s `JSX.EventHandlerUnion` inlined and the DOM's `Event` and
`Element` replaced by local stand-ins so the declarations resolve without the
DOM library; the handler parameter keeps its published shape (a function of the
event, a `[function, data]` pair, or undefined), and `tsc --strict` accepts the
file.

## What the census confirms, and what still refuses

ADR 0100's rule 4 refused every call of a member of a parameter. A member call
is now confirmed site for site when the enumeration names a call item at exactly
that path, under rules 5 to 8 (depth 0, uncaptured, every such site described,
every described member called), and only on a parameter the producer states
unwritten. Every other member call still refuses at rule 4 in its own words,
and item A's premise rule and narrowing apply to the non-call items unchanged.

`composeEventHandlers`' `callbacks` stays open: it needs a deferred-invocation
item and a returned-function `returns` shape (ways-to-improve § 3.3). Its
`creates` is now proposed, because `callHandler` no longer declines and so no
longer refuses the generator's fixpoint, and certifies.
