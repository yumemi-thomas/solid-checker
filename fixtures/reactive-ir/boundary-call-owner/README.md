# boundary-call-owner

**Claim (ADR 0462).** `Loading(props)` and `Errored(props)` called as functions create their boundary in the call; with no owner rc.17's `createBoundary` warns `NO_OWNER_BOUNDARY`. Violations: `Handler` (event handler), `Timer` (`setTimeout`, `Errored`), `Then` (`Promise.then`, aliased import), `Micro` (`queueMicrotask`, namespace member), `AfterAwait` (after `await`, namespace destructuring), `OwnerNull` (`runWithOwner(null)`), `makeBoundary` (a wrapper only a timer calls). Silent: `Body` (component body), `Shadowed` (a local `Loading`).

Not claimed: JSX `<Loading>` in a handler; rc.17 dev's `createComponent` runs it in a transparent root, so the runtime does not warn. tsc: clean against the rc.17 typings. Runtime: the misuse matrix cells of `NO_OWNER_BOUNDARY` (scripts/fuzz). Stub: copied from the published rc.17 `types` (see `solid-js.d.ts`).
