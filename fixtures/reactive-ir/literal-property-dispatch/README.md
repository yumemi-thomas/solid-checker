# literal-property-dispatch

**Claim (ADR 0321).** A function literal that is exactly the value of a
static-keyed property of an object literal written as a call's argument is
that property's implementation: `useThing({ onClose: () => … })` dispatches
`options.onClose()` to the arrow. Its identity is its file and span. Its
reads stay attributed where it is written, as a callback's, so the dispatch
adds none of them; it only stops being unresolved. A literal written as the
argument itself is held by nothing but the callee's parameter, which the
caller proved is only member-read (ADR 0320), so a write of the same member
name elsewhere cannot reach it.

| Case | `SC9012` | Why |
| --- | --- | --- |
| `InertArrow`, `FunctionExpression` | none | the arrow (or receiver-free `function` expression) is the implementation |
| `ReadingArrow` | none (its read stays `SC1001` uncertifiable where it is written) | |
| `WrittenElsewhere` | none | `writeOnClose` writes another object's `onClose` |
| `ReceiverFunction` | uncertifiable | a `function` expression using `this` may hand the object on |
| `SpreadAfter` | uncertifiable | a later spread may replace the member |
| `HandedOn` | uncertifiable | the callee hands its parameter to `sink` |
| `Alternatives` | uncertifiable | two literals behind a conditional are not the argument itself, so the name census applies |
| `BoundLiteral` | uncertifiable | a literal bound to a `const` keeps ADR 0320's census (the write in `writeOnClose`) |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`obligation-exclusion-boundary`; `package.json` is private. The sources pass
`tsc --noEmit` with the fixture stub.
