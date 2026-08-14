# Compiler facts

Solid compiler facts describe original-source JSX execution semantics. The
production Rust checker loads the controlled `dom-expressions-compiler`
implementation in-process, so facts come from the same transform branches as
compilation. Solid 2.0 and Solid 1.x use separate pinned Cargo git dependencies
from [`dom-expressions`](https://github.com/yumemi-thomas/dom-expressions) and
[`solid-1x-compiler`](https://github.com/yumemi-thomas/solid-1x-compiler), both
built without their Node-API feature. The revisions are pinned in
`rust/Cargo.toml` and recorded in `THIRD_PARTY_NOTICES.md`.

## The semantic trace

The compiler emits a `SemanticTrace`: a total list of `ExecutionSite` records
plus a proof-only list of `OwnershipSite` records. Each pairs an
original-source span with a typed semantic decision. The compiler censuses
every JSX execution site before lowering and fails closed if any censused site
reaches the end without a decision, so the checker never has to guess at an
unclassified hole.

Each dialect compiler adapter projects the trace onto the checker's
`ExecutionMap` boundary:

| Site decision | Execution map |
| --- | --- |
| `Value(ReactiveRerun)` | tracked region |
| `Value(EagerOnce)`, `Value(Elided)` | untracked region |
| `Value(EagerOnce)` on a component child | deferred callback |
| `Value(CallerContext)` | deferred callback |
| `Callback(LaterEvent)` | `event-handler` callback |
| `Callback(LaterRender)` | `render` callback |
| `Callback(RefApply)` | `directive-apply` callback |

With the compiler's default effect wrapper, every `ReactiveRerun` also carries
an `OwnershipSite(Owned)` proof: the generated wrapper establishes a reactive
owner while that source region executes. Custom and disabled effect wrappers
emit no ownership claim. Absence is deliberately unknown, not unowned;
component, control-flow, event, and ref ownership continues to be composed
from exact TypeFacts identity and runtime contracts.

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
- Dynamic component properties and component children are deferred callbacks;
  their operations retain distinct `component-property` and `component-child`
  kinds so a component value prop is not confused with a render callback.
- `hydratable`, `dev`, `effectWrapper`, `wrapConditionals`, `staticMarker`, and
  sorted, unique `builtIns` are forwarded exactly to the compiler.
- Fact arrays are sorted deterministically by original UTF-8 byte spans.

Completeness invariant: every `jsx-expression` operation must be covered by a
tracked region, an untracked region, a callback role, or a
`component-property`, `component-spread`, or `component-child` operation.
Because the trace is total, every site lands in
exactly one category and the invariant holds by construction; the IR builder
still reports any uncovered hole as an `SC9004 execution-map-incomplete`
unresolved obligation rather than assuming untracked rendering.

Only DOM generation is supported. Other renderer modes, malformed options,
unknown fact kinds, invalid UTF-8 boundaries, stale hashes, and incompatible
protocol versions fail closed.

## Moving the pin

Compiler conformance — checking the Rust transform against the reference
implementation — belongs to the dom-expressions repository and runs there. To
adopt new compiler work, update the appropriate compiler `rev` in
`rust/Cargo.toml`, refresh `THIRD_PARTY_NOTICES.md`, and run `make verify`.
