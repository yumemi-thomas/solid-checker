# release-triple-dynamic-rc7

On rc.7 `dynamic(source, { static: true })` gets the default model: `@solidjs/web@2.0.0-rc.7` declares `DynamicOptions { deferStream?: boolean }` (`types/index.d.ts:81-90`; `{ static: true }` is TS2353 against stub and real rc.7) and its runtime never reads `_options`. Each static call behaves as its one-argument twin; the snapshot's `SC2001` and `SC4001` findings are those twins' answers. It also carries the `SC9014` release notice (rc.7 was reviewed with one gap, and is not the audited release).
