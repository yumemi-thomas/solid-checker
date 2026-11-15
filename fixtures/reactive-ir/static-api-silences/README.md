# static API: every call ends (SC7001, SC7002)

ADR 0989 (batch 11, slice 8b). A call of an effect primitive is a site of `effect-fn` (SC7001), and a call of a primitive that takes `sync` a site of `sync-option`
(SC7002). Each ends in a row or in a silence that names the fact (`static_api.effect_function.*`, `static_api.sync_option.*`), or in `effect-fn-unaccounted`
/ `sync-option-unaccounted` (an obligation, never a violation: `SOLID_CHECKER_IR_ABLATE_SILENCE=all` turns each silent case into one).

The `sync` option is read as the runtime reads it: the exact literal's LAST `sync` (`{ sync: true, sync: false }` is not `sync: true`), and a spread or a
computed key after `true` may override it, so the call is an obligation, not a violation (Chrome rc.21: the spread cases are quiet). A duplicate key is
TS1117 in every configuration, so it is TypeScript's unless a directive silences it. `createTrackedEffect`'s `sync` is inert: its node's compute returns
`undefined` (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js:2878-2892`), so SYNC_NODE_RECEIVED_ASYNC is never raised there and the call is no site.
Variants: namespace and alias callees (sites), a shadowed local `createEffect` (no site), every primitive that takes `sync` over an async computation and over
a spread.
