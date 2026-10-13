# Package effectful member consumer

ADR 0235 consumer controls. The authorized contract states that `createTicker`
returns `[running, start]`: `running` is the accessor of a signal it creates,
and `start` is an `effectful-callable` whose own call graph reads that signal
once, at the call, in the caller's tracking context.

| Case | Finding | Why |
| --- | --- | --- |
| `StartInBody`: `start()` in the component body | `strict-read-untracked` violation | the call is bound to the member's effects, and the read is untracked there |
| `StartAsHandler`: `start` as an `onClick` value | none | it runs later, outside the render |
| `StartInHandlerClosure`: `() => start()` as a handler | none | the read happens in the handler |
| `LetBinding`: a `let` destructuring | `reactive-dispatch-unresolved` uncertifiable (ADR 0234) | a reassignable binding is not bound to the effects |
| `EscapedStart`: `start` passed to a function | `reactive-dispatch-unresolved` uncertifiable | it may be called anywhere |
| `ThroughTheTuple`: `ticker[1]()` | `reactive-dispatch-unresolved` uncertifiable | only destructured `const` members are bound |
| `ThroughTheTuple`: `ticker[0]()` in JSX | `reactive-dispatch-unresolved` uncertifiable | an existing rule: a call through a computed member is not resolved |

The manifest, declarations and authorization are
`partial-structural-return-consumer`'s, byte for byte, so every digest the
authorization pins is unchanged; only the contract document differs. The
fixture issuer supplies the contract; it claims no runtime census of a
published package.
