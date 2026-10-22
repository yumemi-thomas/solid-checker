# setter-in-root-rc9

rc.3's twin with `@solidjs/signals@2.0.0-rc.9`, which dropped the root exemption (`dist/dev-shared.js:5878`, #3500): every store setter directly in a root body, in `untrack` there, or in a helper only a root body calls is SC2001, plus SC9014. Listeners stay uncertifiable. Stubs from published rc.9 typings; `root.ts` is `tsc --noEmit` clean against them.
