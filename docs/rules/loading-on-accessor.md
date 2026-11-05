# loading-on-accessor

`SC5007` · **warning** · violation

`<Loading on>` received a function, which the boundary never calls.

## What it does

`on` takes the dependency's value. An accessor handed instead is a stable value:
nothing is read, so the boundary never re-arms. rc.21 reports
`LOADING_ON_ACCESSOR` on the boundary's first run (`reportOnAccessor`,
`@solidjs/signals@2.0.0-rc.21` `dist/dev.js`), and `on` is typed `any`, so `tsc`
is silent. Reported on the exact `Loading` for a function literal and for a value
Type Facts call a function, wherever it came from (ADR 0624). A union that may be
a function is uncertifiable, and any other value states nothing.

```tsx
<Loading on={route} fallback={<Spinner />}><Page /></Loading>   // reported
<Loading on={route()} fallback={<Spinner />}><Page /></Loading> // fine
```
