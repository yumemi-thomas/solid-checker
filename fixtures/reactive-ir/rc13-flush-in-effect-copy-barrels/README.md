# rc13-flush-in-effect-copy-barrels

Two installed copies of `solid-js` at the same version (`node_modules` and `lib/node_modules`), as in `rc13-flush-in-effect-same-version-copies`, with the `createEffect` import reached through re-exports. TypeScript merges the copies' declarations, so the declaration package directories are equal; the runtime loads both. SC2008 follows the `createEffect` import through every barrel and `export *` to the file that imports the package, and compares the `solid-js` that file loads with the one the `flush` import loads (`PackageInstanceEvidence`). A hop it cannot resolve exactly is not claimed.

Reported: the `createEffect` from `solid-js` directly, and through `barrel.ts` (same copy as `flush`). Silent: through `lib/bridge.ts`, through `lib/star.ts` (`export *`), and through a namespace import of `lib/bridge.ts`, each running the second copy's effect while `flush` drains the first.

Silent too: a `createEffect` imported through another package (`foreign-effects`, whose `exports` conditions pick the file it loads and so the copy it imports), and, in the backend's evidence, a file reached through a symlinked workspace package resolves `solid-js` from its real directory.
