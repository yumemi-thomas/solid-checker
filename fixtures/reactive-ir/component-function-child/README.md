# component-function-child

**Claim (ADR 0326).** A function written as a component's sole child
(`<Kids>{(x) => …}</Kids>`) is that component's `children` prop. The compiler
fact projects it `Deferred` because the receiving component, not the compiler,
invokes it; that says nothing about when, so the checker no longer reads it as
a deferred callback, and a tracked region around the element does not place
it either. ADR 0319's walk follows it through the `children` prop: a child the
component calls in tracked JSX or an event handler takes that role. Otherwise
it is uncertifiable (internal, ADR 0328), including when the component's body
calls it: that proof was withdrawn after six review rounds found ways to
replace the child before the call, for zero corpus yield. A control-flow
component's children callback (`<For>`, `<Show>`) is unchanged.

| Case | Finding | Why |
| --- | --- | --- |
| `BodyChild` | `SC1001` uncertifiable | `Kids` calls `props.children(1)` in its body; the body proof is not used for children |
| `BodyChildInRegion` | `SC1001` uncertifiable | the same, inside the caller's tracked `{shown() && …}` expression, which lowers the element but does not run the child (silent before) |
| `OpaqueChild` | `SC1001` uncertifiable | `Handed` passes the child to `opaque`, which has no body |
| `SpreadChild` | `SC1001` uncertifiable | a spread beside the child refuses the body proof |
| `TrackedChild` | none | `InJsx` calls the child inside a tracked JSX expression |
| `EventChild` | none | `OnClick` calls the child only from a click handler |
| `IntrinsicChild` | `SC1001` uncertifiable (unchanged) | an intrinsic element's function child is the compiler's, not a prop |
| `ExpressionChild` | none | a non-function child is the compiler's `children` getter, as before |
| `AliasedChild` | `SC1001` uncertifiable | `props` escapes into an alias that replaces `children`; the body proof needs every use of `props` to be a plain member read (`Aliased` also draws the existing `SC2003` for the write) (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `DeletedChild` | `SC1001` uncertifiable | the body deletes `children` before calling it (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `ReplacedChild` | `SC1001` uncertifiable | `props.replace()` gets `props` as `this` and replaces `children` first (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `ArgumentsChild` | `SC1001` uncertifiable | `arguments[0]` is `props`; a component mentioning `arguments` or `eval` refuses the body proof (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `LoopChild` | `SC1001` uncertifiable | a `for … of` head writes `props.children` (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `RedeclaredChild` | `SC1001` uncertifiable | `var props` replaces the parameter (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `FieldChild` | `SC1001` uncertifiable | the call is a class field initializer, run at construction (on click), not in the body (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |
| `SwappedChild` | `SC1001` uncertifiable | the tag's binding is reassigned (`Swapped = Later`); the body proof needs the tag to run the declaration it names (review-round shape; children never use the body proof, and `prop-body-call-guard` pins the guard) |

`{shown() && …}` also draws the style rule `SC8015` (prefer `<Show>`). `Kids`
reading `props.children` in its body is the existing props-read
uncertifiable (line 6). The runtime warns at both body calls and is quiet for the three
negative cases: `fixtures/runtime-oracle/oracle-children-body`.

`solid-js.d.ts` is `invocation-flow`'s stub, unchanged. The same shapes
type-check against the published rc.13 typings in the oracle case.
