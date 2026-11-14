# loading-on-accessor

`SC5007` · **warning** · violation

`<Loading on>` received a function, which the boundary never calls.

## What it does

`on` takes the dependency's value. An accessor handed instead is a stable value:
nothing is read, so the boundary never re-arms. rc.21 reports
`LOADING_ON_ACCESSOR` on the boundary's first run (`reportOnAccessor`,
`@solidjs/signals@2.0.0-rc.21` `dist/dev.js`), and `on` is typed `any`, so `tsc`
is silent. Reported on the exact `Loading` for a function literal and for a value
Type Facts call a function, wherever it came from (ADR 0624). `on` is read on every
form that hands it: the attribute, a spread, `Loading({ on })` called by hand and
`createComponent(Loading, { on })` (ADR 0978).
Only a value proven not to be a function states nothing. `any`, `unknown`, a union,
a possibly undefined function, a spread whose declared keys are not closed against
`on` and an options object that is no exact literal are uncertifiable, blocker
`loading-on-value-unproven` (ADR 0650).

A literal closed against `on` (accessors allowed) states `on`'s absence, so a hand call
with only `get children()` has no row; a getter or a method named `on` is not followed
(uncertifiable). Where the boundary is named as a value (`Loading.call(null, props)`,
`[Loading][0](props)`, `dynamicComponent(() => Loading)`, `<Dynamic component={Loading}>`)
or the tag's component is unresolved, the props go to code the analysis does not follow, so
the row is uncertifiable, never a violation (the target may not be the boundary).

```tsx
<Loading on={route} fallback={<Spinner />}><Page /></Loading>   // reported
<Loading on={route()} fallback={<Spinner />}><Page /></Loading> // fine
```
