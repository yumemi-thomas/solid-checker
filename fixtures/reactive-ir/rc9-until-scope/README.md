# rc9-until-scope

SC2005 `until-in-tracked-scope`, and the `until` vocabulary row it rests on.

`until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>` is new in
`@solidjs/signals@2.0.0-rc.9` (`dist/types/signals.d.ts:608`) and re-exported
from the `solid-js` root (`types/index.d.ts:1`). Its body
(`dist/dev.js:2717-2785`) is `resolve`'s shape:

1. `if (getObserver()) throw new Error("Cannot call until inside a reactive
   scope; …")` (`:2718-2722`) — dev only; `dist/prod/signals.js:530` has no
   guard;
2. `new Promise(…)`, rejecting at once if `options.signal` is already aborted
   (`:2734`);
3. `createRoot(dispose => { … effect(fn, apply, error, { user: true, … }) })`,
   so `fn` is the tracked compute of a user effect under a fresh root, run for
   the first time during the call and again until one run is truthy.

So the dialect answers `until` exactly as it answers `resolve`: argument 0,
`Deferred` for attribution (the predicate subscribes the effect `until`
creates, never the caller), `Creates` for ownership, listener-clearing, and no
package-contract word.

The rule mirrors the dev throw with `resolve-in-tracked-scope`'s scope proof,
because the guard is the same expression. The positives are the four tracked
scopes `resolve-scope` pins for `resolve`; the negatives are the observer-free
ones, plus the action step rc.9's own `until` documentation shows
(`yield until(...)`).

`tsc --noEmit` is clean against this fixture's stubs and against the published
rc.9 typings. Against rc.3's typings the import is TS2305, so the rule cannot
fire on rc.3-valid code. `node_modules/` holds the `solid-js` and
`@solidjs/signals` manifests at `2.0.0-rc.9`: `until` is a vocabulary name
only where both resolve to rc.5 or later (`release-triple-until-rc3` pins
the rc.3 side).
