# rc9-flush-in-action

SC2006 `flush-in-action`, on the rc.9 installation.

`@solidjs/signals@2.0.0-rc.8` added a dev guard to `flush`
(rc.9 `dist/dev-shared.js:2210-2219`):

```js
if (actionStepDepth > 0) throw new Error("[FLUSH_IN_ACTION] flush() inside an action body is not allowed. …");
```

It precedes the `fn` argument, so `flush()` and `flush(fn)` both throw.
`actionStepDepth` is raised only by `action`'s `step`, around the generator's
own `it.next(v)` / `it.throw(v)` (`dist/dev.js:2016-2024`). The production
build takes the same branch and returns `fn?.()` without draining
(`dist/prod/core/scheduler.js:1182-1184`).

So a position is inside the throw exactly when it runs during a step: anywhere
in a sync generator's body, and in an async generator's body up to its first
suspension (`await`, `for await`, async `yield*`) and again after each `yield`.
Every case in `App.tsx` was probed against the rc.0-rc.9 dev and production
bundles; rc.8 and rc.9 dev reject each positive with `FLUSH_IN_ACTION`, every
other build resolves.

Expected: the six positives (sync head, `flush(fn)`, after a yielded promise,
async head, the awaited operand, the namespace import) report SC2006 as
violations. The negatives stay silent: after an `await`, an `await` in the
argument, `for await`, after an async `yield*`, a loop that awaits (its first
iteration throws; not claimed), a parameter initializer, scheduled callbacks,
`untrack(() => flush())` (throws at runtime; the lexical proof stops at the
nested function), a wrapped generator, a generator not handed to `action`, a
shadowed `flush`, a plain function, and an event handler. The rc.9 triple also
carries its SC9014 release notice.

`solid-js.d.ts` transcribes `flush` (both overloads), `action` and `untrack`
byte-faithfully (see its header). `tsc --noEmit` (5.9.3, `strict`, bundler
resolution, `jsxImportSource: "@solidjs/web"`, `skipLibCheck`) is clean on
`App.tsx` against the stubs and against the published rc.9, rc.8 and rc.3
installs: the three declarations are identical on every release, so the type
system cannot see which release throws. `release-triple-flush-rc8` pins the
first release with the throw, and `release-triple-flush-rc3` the audited
release without it.

`node_modules/` holds the `solid-js` and `@solidjs/signals` manifests at
`2.0.0-rc.9`.
