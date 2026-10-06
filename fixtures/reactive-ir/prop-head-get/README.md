# prop-head-get

**Claim (ADR 0221, G1).** In `props.value.text`, the head `props.value` is a
Get the runtime performs first, before the suffix is read or checked for
nullishness. When the read is written directly in the exact children
callback of a dialect control-flow primitive, the head is a strict read
whenever the prop is backed reactively (`local_access`,
`execution_role::direct_control_flow_body_role`). The ADR 0216
per-property forwarding fixpoint decides the backing. It is queried, not
replaced.

| Case | `SC1001` | Why |
| --- | --- | --- |
| `NamespaceChild` (`Solid.For`) | violation | exact primitive through a namespace import |
| `KeyedShow` | violation | the captured prop, not the keyed item, is read |
| `Rows` through `Forwarder` | violation | exact cross-file forward of a live value |
| `PlainInner` ← `PlainMiddle` ← `PlainOuter` | none | complete plain forwarding stays static |
| `Shadowed` | none | a local `For` names no primitive and never runs its child |
| `WrappedChild` | uncertifiable | `replaceChild(fn)` passes its result, not the literal |
| `DormantDefault` | uncertifiable | a nested function's unused default is not a body read |
| `MergedHead`, `AliasedHead` | uncertifiable | a view is not the parameter's own keys |
| `ReplacedRoot`, `ReplacedProperty` | uncertifiable | the incoming witness is withdrawn after a write |
| `TypedInner`, `ThenInner`, `SymbolInner`, `DynamicStoreInner` | uncertifiable | type-only labels and protocol or dynamic store keys prove no backing |
| `Dynamic` | uncertifiable | a dynamic key is not its spelling's sibling |
| `SuffixInner`, `WholeInner`, `ChildrenInner`, `EscapingInner` | uncertifiable | ADR 0216 Unknown is kept |
| `StableFunction`, `CapturedCorrect` | none | a bound handler or ref, and tracked or sampled reads |

`Cooked` (an escaped literal key behind `as`) is meant as a positive but is
not proven yet. The finding is absent there, which is a recall gap and not a
clean result.

The stubs are byte-faithful to the published 2.0.0-rc.13 declarations, as
`solid-js.d.ts` says. The sources also pass `tsc --noEmit` against the real
installed rc.13 typings.
