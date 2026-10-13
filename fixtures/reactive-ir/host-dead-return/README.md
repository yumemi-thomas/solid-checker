# host-dead-return

**Claim (ADR 0288).** Under a declared browser host, a package analyzed as
source (`--dependency-source`, ADR 0284) has its `isServer` guard folded
before any fact is read (ADR 0166's span-preserving fold, applied to package
files by ADR 0284 § 5). An `if (isServer) return [...]` early return is
therefore not a return site of the factory at all, and ADR 0282's
single-unconditional-return rule sees the live return only. No IR rule
decides host constants: the fold already did.

`dep-loop` is `@solid-primitives/raf@4.0.0-next.2`'s `createRAF` shape
(`dist/index.js`), with its frame loop reduced to a callback, plus variants.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | `SC1001` violation | `if (isServer) return [...]` is folded to `if (!1) ;`; `start` reads `running` first |
| `DefaultLoop` | `SC1001` violation | the same factory through `export { createLoop as default }`, as `createRAF` is imported |
| `IdleLoop` | `SC1001` violation | the server arm returns closures the factory declares (see `host-dead-return-server`) |
| `BlockLoop` | `SC1001` violation | the guard as a block |
| `ClientLoop` | none | the live return sits inside `if (!isServer) { ... }`: after the fold it is still under a control test, so it is not one unconditional return |
| `FlaggedLoop` | none | `if (flag)`: no host decides an ordinary parameter |
| `EitherLoop` | none | `isServer \|\| flag`: the constant takes part but does not decide |
| `NamespacedLoop` | none | `web.isServer`: the fold binds only a named import |
| `ProjectLoop` | none | the guard in a project file, which is not folded (ADR 0284 folds package files only); two return sites |

Twins: `host-dead-return-server` is the same package and components under a
Node SSR host, where every guarded server return is the live one and nothing
is reported. `host-dead-return-wildcard` is `Loop` in a project that also
holds an `Object.assign` with an open source, which vetoes every tuple slot
(ADR 0282's premise): this is what kept `createRAF`'s `start` silent in the
rc.13 corpus.

`.solid-checker/runtime.json` declares a browser development host and
`dependencySources: ["dep-loop"]`.

Stubs: `solid-js.d.ts`, `tsconfig.json`, `node_modules/solid-js` and
`node_modules/@solidjs/signals` are copied from `tuple-slot-provenance`,
unchanged. `node_modules/@solidjs/web/package.json` is the published
`@solidjs/web@2.0.0-rc.13` manifest reduced to `name`, `version`, `type`,
`types` and its `"."` export, byte-faithful in every condition and target.
Its six `dist` files keep only the published `const isServer = false;`
(browser entries) or `const isServer = true;` (server entries) and its export;
`types/index.d.ts` keeps only the published `export declare const isServer:
boolean;`. The value the fold reads is the published runtime value, and the
type says nothing about it, as published. `tsc --noEmit -p` passes; the
package's JavaScript is not type-checked (`checkJs: false`).
