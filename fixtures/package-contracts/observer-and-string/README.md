# Every observer constructor defers its callback

The four DOM observer constructors take their callback at argument 0 and invoke
it only when the platform reports an observation, never on the constructing
call. All four summaries are therefore the same `queued`, `0..many` shape.

`runtime-semantics` carries the wider built-in table but only reaches
`ReportingObserver` and `IntersectionObserver`; `ResizeObserver`,
`MutationObserver` and `PerformanceObserver` are pinned only here, so a table
entry dropped for one of those three is invisible without this fixture.
`runtime-semantics-shadowed` is the negative control for the whole family.
