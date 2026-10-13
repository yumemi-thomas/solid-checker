# release-triple-flush-rc8

SC2006 `flush-in-action` on the `2.0.0-rc.8` triple, the first release that
throws. `@solidjs/signals@2.0.0-rc.8` added the guard to `flush`
(`dist/dev-shared.js:1904-1913`) and the step brackets around `it.next(v)` in
`action` (`dist/dev.js:1682-1690`); rc.7 has neither (no `actionStepDepth` in
any rc.7 bundle). Probe R of the rc.1-rc.8 release review (§ 3): `resolved` on
rc.0-rc.7, `rejected FLUSH_IN_ACTION` on rc.8 and rc.9 dev, `resolved` in every
production build.

Expected: the two positives (sync generator head; async generator head, with
`flush(fn)`) report SC2006 as violations; the after-`await` call is silent; and
the rc.8 triple carries its SC9014 notice (signals rc.8 has no negative row).

`solid-js.d.ts` transcribes rc.8's `flush` and `action` byte-faithfully.
`tsc --noEmit` (5.9.3, `strict`, bundler resolution) is clean on `App.ts`
against the stubs and against the published rc.8 install.

`node_modules/` holds the three package manifests at `2.0.0-rc.8`; the answer
is read from `@solidjs/signals`.
