# rc.17 untracked-read probes (ADR 0443)

Runtime probes behind ADR 0443: what the pinned `solid-js`, `@solidjs/signals` and `@solidjs/web` 2.0.0-rc.17 browser dev builds do with a reactive read in a `mapArray`/`repeat` map function (`1-mapper-window.mjs`), in a context provider's `value` (`2-context-provider.mjs`) and in the unnamed derived shapes of eslint-plugin-solid's `badUnnamedDerivedSignal` (`3-unnamed-derived.mjs`).

```sh
npm ci --ignore-scripts          # integrities are in package-lock.json
node --conditions=browser --conditions=development 1-mapper-window.mjs
```

Without `--conditions=browser` Node takes the server build, whose signals are inert and whose effects never run; its output is not evidence. Each probe prints the resolved files, the dev diagnostics on both channels (`OBSERVE.diagnostics` and the console) and the subscribers of the signal (`DEV.getObservers`). Nothing here is read by a gate; it is the record a later release is compared against.
