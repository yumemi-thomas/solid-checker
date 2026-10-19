# release-triple-dynamic-rc3

On rc.3 `dynamic(source, { static: true })` gets the default model: `@solidjs/web@2.0.0-rc.3` declares `dynamic(source)` (`types/index.d.ts:81`; two-argument calls are TS2554 against the byte-faithful stub and real rc.3) and no build before rc.9 reads `options?.static`. Each static call behaves as its one-argument twin; no SC9014 (rc.3 is audited).
