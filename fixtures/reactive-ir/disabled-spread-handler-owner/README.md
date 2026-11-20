# disabled-spread-handler-owner

ADR 1141, extending ADR 1134 (`disabled-handler-owner`) to controls with spreads. With a spread, the compiler hands every attribute
and spread to rc.21 `@solidjs/web` `spread` as one array of sources; its `assignProp` stores a listener key whose event is in
`DelegatedEvents` on the node for `eventHandler`, which calls it only `if (handler && !node.disabled)`. `SpreadDisabled`: a bare
`disabled` on a `button` or `input` whose spreads each have proven own keys (a closed literal in place, or an unwritten `const` ADR
0358 resolves), none of them `disabled`, `ref` or namespaced, in a program that writes no `.disabled` and calls no attribute writer:
`createEffect` in the control's own `onClick` beside `{...rest}` (1134's own case), in the `onClick` a spread literal holds, and in an
`input`'s `onKeyDown` gives `handler-on-disabled-control` (violations on main a64fc92ad; Chrome quiet, runtime-oracle
`disabled-spread-handler-owner`).

`SpreadStillRuns` keeps every violation, each confirmed by NO_OWNER_EFFECT in Chrome: a spread whose key set holds `disabled`
(`false`: `setAttribute` removes it), a spread whose keys are not proven (a parsed value), a spread literal's `onWheel` (not
delegated: the browser's own listener), and the driven twin without `disabled`.
