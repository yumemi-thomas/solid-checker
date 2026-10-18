# `children(fn)` is a tracked, lazy memo (ADR 0323)

`children(fn)` is `createMemo(fn, { lazy: true })` wrapped once more
(`solid-js@2.0.0-rc.13` `dist/solid.js:21-24`, `dist/solid.dev.js:20-23`; the
same bytes in rc.0, rc.6, rc.8 and rc.9). `fn` is the computation of a memo of
its own, so a read in it subscribes that memo. Before the row, the dialect had
no answer for the position and a literal kept the lexical body role: a signal
read in `children(() => n())` was a proven `SC1001` violation the runtime does
not raise.

Claim: no finding for a signal read or a props read in the callback, for the
conditional and the statement forms. Controls: `onCleanup` (deferred
primitive, clean), a read written directly in the body (still a violation: the
row did not silence the rule), and `children2`, a project helper that merely
resembles the primitive (its read is the proven violation it always was: the
helper calls its parameter in its own body).

Types: `node_modules/solid-js` and `@solidjs/signals` are the published rc.13
declarations copied unchanged from `standard-runtime-configuration`
(`@solidjs/web` is that fixture's `jsx-runtime.d.ts` stub; it is why the
SC9014 notice is in the snapshot). The tree passes `tsc --noEmit`.
