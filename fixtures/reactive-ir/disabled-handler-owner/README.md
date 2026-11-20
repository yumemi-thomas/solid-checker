# disabled-handler-owner

ADR 1134. rc.21 `@solidjs/web` `eventHandler` calls a delegated handler only `if (handler && !node.disabled)`, so the handler of a
form control disabled from its mount, which nothing in the program enables, never runs, for a user's event and a dispatched one
alike. `MountDisabled`: `createEffect` in the handler is no longer an SC4001 `missing-owner` violation but an obligation,
`handler-on-disabled-control` (a bare `disabled`, `disabled={true}`, an `input`'s `onKeyDown`, `onPointerDown`). On main
f79f19e72 each was a violation, which Chrome never confirms (runtime-oracle `handler-effect-driven` `MountDisabled`: quiet).

`StillRuns` keeps every violation: a `div` (no `disabled` property), `onWheel` (not delegated; the browser delivers a user's wheel to a disabled control),
a `disabled` that follows a signal, and the driven twin. `disabled-handler-owner-enabled` keeps the row where the program
writes `.disabled`. A control with spreads is ADR 1141's (`disabled-spread-handler-owner`).
