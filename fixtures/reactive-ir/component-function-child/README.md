# component-function-child

**Claim (ADR 0326).** A function written as a component's sole child
(`<Kids>{(x) => …}</Kids>`) is that component's `children` prop. The compiler
fact projects it `Deferred` because the receiving component, not the compiler,
invokes it; that says nothing about when, so the checker no longer reads it as
a deferred callback, and a tracked region around the element does not place
it either. It is placed like a function attribute: proven to run in the
receiving component's body when that body calls `props.children` directly
(ADR 0204's proof), followed by ADR 0319's walk through the `children` prop
otherwise, and uncertifiable when neither places it. A control-flow
component's children callback (`<For>`, `<Show>`) is unchanged.

| Case | Finding | Why |
| --- | --- | --- |
| `BodyChild` | `SC1001` violation | `Kids` calls `props.children(1)` in its body |
| `BodyChildInRegion` | `SC1001` violation | the same, inside the caller's tracked `{shown() && …}` expression, which lowers the element but does not run the child |
| `OpaqueChild` | `SC1001` uncertifiable | `Handed` passes the child to `opaque`, which has no body |
| `SpreadChild` | `SC1001` uncertifiable | a spread beside the child refuses the body proof |
| `TrackedChild` | none | `InJsx` calls the child inside a tracked JSX expression |
| `EventChild` | none | `OnClick` calls the child only from a click handler |
| `IntrinsicChild` | `SC1001` uncertifiable (unchanged) | an intrinsic element's function child is the compiler's, not a prop |
| `ExpressionChild` | none | a non-function child is the compiler's `children` getter, as before |
| `AliasedChild` | `SC1001` uncertifiable | `props` escapes into an alias that replaces `children`; the body proof needs every use of `props` to be a plain member read (`Aliased` also draws the existing `SC2003` for the write) |
| `DeletedChild` | `SC1001` uncertifiable | the body deletes `children` before calling it |
| `ReplacedChild` | `SC1001` uncertifiable | `props.replace()` gets `props` as `this` and replaces `children` first |
| `ArgumentsChild` | `SC1001` uncertifiable | `arguments[0]` is `props`; a component mentioning `arguments` or `eval` refuses the body proof |

`{shown() && …}` also draws the style rule `SC8015` (prefer `<Show>`). `Kids`
reading `props.children` in its body is the existing props-read
uncertifiable (line 6). The runtime confirms the two violations and the three
quiet cases: `fixtures/runtime-oracle/oracle-children-body`.

`solid-js.d.ts` is `invocation-flow`'s stub, unchanged. The same shapes
type-check against the published rc.13 typings in the oracle case.
