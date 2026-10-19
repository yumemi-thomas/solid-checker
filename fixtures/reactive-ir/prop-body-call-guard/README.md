# prop-body-call-guard

**Claim (ADR 0204, guard hardened in the ADR 0326 review rounds).** A function
passed as a component prop runs during the component's render, and its reads
are a proven `SC1001`, only when the component's own body calls that prop
directly and nothing can have replaced the prop before the call. Each case
below replaces it first. The checker reported every one as a proven violation
before 2026-10-19; each is now uncertifiable (internal, ADR 0328).

The guard (`component_invokes_prop_in_body`): the tag runs the declaration it
names (`function_value_is_current`, exact member tag); the props parameter is
never written or redeclared; every use of `props` is a static member read that
is not written, deleted or called, except the prop's own call; no member of
that name is written or deleted in the body; no `arguments` or `eval`; and the
call is not inside a class written in the body.

| Case | Finding | Why |
| --- | --- | --- |
| `Called` | `SC1001` violation | control: `Runs` calls `props.render()` in its body |
| `AliasedProps` | uncertifiable | `const alias = props; alias.render = …` (review round 1) |
| `DeletedProp` | uncertifiable | `delete props.render` (round 2) |
| `ReplacedThroughThis` | uncertifiable | `props.replace()` gets `props` as `this` (round 2) |
| `ThroughArguments` | uncertifiable | `Reflect.set(arguments[0], …)` (round 3) |
| `LoopHead` | uncertifiable | `for (props.render of …)` (round 4) |
| `RedeclaredProps` | uncertifiable | `var props = …` (round 4) |
| `ClassField` | uncertifiable | the call is a class field initializer, run on click (round 5) |
| `SwappedTag` | uncertifiable | the `let` tag is reassigned to a component that calls it on click (round 6) |

The other uncertifiable rows are the existing props-read results. `Aliased`
also draws the existing `SC2003` for the write. The stubs are
`component-function-child`'s. A call in a statically dead branch stays a
violation under the feasible-path premise direct reads use (ADR 0326).
