# Retained callback parameter

Pins the fail-closed rule for a caller-supplied callback the generator can
neither prove invoked nor prove inert, and the precision boundary that keeps it
from becoming a blanket sentinel.

`contract generate` summarizes a local callee transitively and lets a caller
inherit its callback answer. That inheritance is only sound when the callee
*accounts for* the parameter. `createNode` here is the shape solid-js 1.9.14's
`createComputation` has: it stores `fn` in an object literal and never calls
it, so its callback summary is empty — and an omitted `callbacks` list is the
negative claim "invokes no caller-supplied function". Before the fix,
`forwardsIntoRetainingHelper` published that silence, exactly as the generated
solid-js contract published it for `createMemo`, `createEffect`, `children`,
`createSelector`, `createDeferred`, `createRenderEffect` and `createComputed`,
each of which `contract probe`'s discovery pass then contradicted.

Expected generation:

| Export | `callbacks` | Why |
| --- | --- | --- |
| `forwardsIntoRetainingHelper` | `{"status":"unknown"}` | forwarded into a helper that retains it |
| `retainsInModuleBinding` | `{"status":"unknown"}` | assigned into a module binding |
| `absorbsRest` | `{"status":"unknown"}` | a rest element has no statable parameter index |
| `invokesCallback` | `[{parameter:0, execution:"inline"}]` | invocation is proven |
| `observesCallback` | *absent* | every reference only observes the value |
| `storesIntoCallerContainer` | *absent* | written into a container the caller supplied |

The last two are the precision half of the claim and matter as much as the
first three: a rule that called every unrecognized reference an escape turned
a third of `@solidjs/web`'s exports into sentinels while proving nothing.
