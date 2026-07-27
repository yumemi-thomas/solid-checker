# Compiler facts

Solid compiler facts describe original-source JSX execution semantics. The
production Rust checker loads the controlled `dom-expressions-compiler`
implementation in-process, so facts come from the same transform branches as
compilation. The crate is a pinned Cargo git dependency on the
[`feat/total-semantic-trace`](https://github.com/yumemi-thomas/dom-expressions/tree/feat/total-semantic-trace)
branch, built without its Node-API feature; the revision is pinned in
`rust/Cargo.toml` and recorded in `THIRD_PARTY_NOTICES.md`.

## The semantic trace

The compiler emits a `SemanticTrace`: a flat list of `ExecutionSite` records,
each pairing an original-source span with a site kind and a terminal decision.
The trace is *total* — the compiler censuses every JSX site before lowering and
fails closed if any censused site reaches the end without a decision, so the
checker never has to guess at an unclassified hole.

`solid-facts-backend` projects the trace onto the checker's `ExecutionMap`
boundary:

| Site decision | Execution map |
| --- | --- |
| `Value(ReactiveRerun)` | tracked region |
| `Value(EagerOnce)`, `Value(Elided)` | untracked region |
| `Value(EagerOnce)` on a component child | deferred callback |
| `Value(CallerContext)` | deferred callback |
| `Callback(LaterEvent)` | `event-handler` callback |
| `Callback(LaterRender)` | `render` callback |
| `Callback(RefApply)` | `directive-apply` callback |

`Value(CallerContext)` is the dynamic component property: the expression is
handed to the child as a getter and re-evaluated in the child's tracking
context, so it is deferred rather than untracked. A component child is invoked
from the component's own render for the same reason, even though the value
itself is built once.

The hardened DOM contract covers these compiler decisions:

- Dynamic native JSX children and attributes are tracked regions.
- Expressions the compiler renders exactly once are explicit untracked
  regions: template-inlined and unwrapped-insert children (including
  `staticMarker` holes), one-shot `setAttr` attribute values, and by-value
  component properties.
- `on*` JSX values are deferred `event-handler` callbacks rather than tracked
  reads at element creation.
- Dynamic component properties and component children are deferred callbacks.
- `hydratable`, `dev`, `effectWrapper`, `wrapConditionals`, `staticMarker`, and
  sorted, unique `builtIns` are forwarded exactly to the compiler.
- Fact arrays are sorted deterministically by original UTF-8 byte spans.

Completeness invariant: every `jsx-expression` operation must be covered by a
tracked region, an untracked region, a callback role, or a
`component-property` operation. Because the trace is total, every site lands in
exactly one category and the invariant holds by construction; the IR builder
still reports any uncovered hole as an `SC9004 execution-map-incomplete`
unresolved obligation rather than assuming untracked rendering.

Only DOM generation is supported. Other renderer modes, malformed options,
unknown fact kinds, invalid UTF-8 boundaries, stale hashes, and incompatible
protocol versions fail closed.

## Moving the pin

Compiler conformance — checking the Rust transform against the reference
implementation — belongs to the dom-expressions repository and runs there. To
adopt new compiler work, update the `rev` of `dom-expressions-compiler` in
`rust/Cargo.toml`, refresh `THIRD_PARTY_NOTICES.md`, and run `make verify`.
