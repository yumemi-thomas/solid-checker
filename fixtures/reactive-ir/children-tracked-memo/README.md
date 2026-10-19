# `children(fn)` is a tracked, lazy memo (ADR 0323)

`children(fn)` is `createMemo(fn, { lazy: true })` wrapped once more (`solid-js@2.0.0-rc.13` `dist/solid.js:21-24`; same bytes in rc.0, rc.6, rc.8, rc.9), so a read in `fn` subscribes that memo: no finding for signal or props reads in the callback (conditional and statement forms); controls: `onCleanup` clean, a body read a violation, `children2` (a project lookalike) a violation. Published rc.13 declarations from `standard-runtime-configuration` (`@solidjs/web` is its `jsx-runtime.d.ts` stub, hence SC9014).
