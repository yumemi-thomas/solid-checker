# flush-read-role

**Claim.** `flush(fn)`'s callback has the caller's **read** role, as
`write-scope-roots-rc3` pins that it has the caller's write role. `flush(fn)`
runs `fn` inline between a `syncDepth` increment and the drain and touches
neither the owner nor the listener (`@solidjs/signals@2.0.0-rc.0`
`dist/dev.js:1085-1099`, rc.3 `dist/dev.js:1788-1802`, rc.9
`dist/dev-shared.js:2202-2230`), so the dialect no longer lists `flush` among
the callbacks that run outside tracking (`Solid2::runs_callback_deferred`).

Probed on every published `solid-js`/`@solidjs/signals` rc.0-rc.9 client build,
dev and prod, under Node with the `browser` (and `development`) conditions:

- `createMemo(() => flush(() => count()))` re-runs when `count` is written, as
  the bare read does and `untrack(() => count())` does not;
- `getObserver()` inside `flush(fn)` in a memo compute is the memo;
- an effect compute reading through `flush(fn)` re-runs;
- `flush(() => count())` in a component body raises `STRICT_READ_UNTRACKED`
  (dev), as `count()` there does and `untrack(() => count())` does not.

| Case | Finding | Why |
| --- | --- | --- |
| a read in `flush(fn)` in a component body | `SC1001` violation | the body's strict-read window is still open inside `flush(fn)` |
| a read in `flush(fn)` in a memo compute | none | it subscribes the memo |
| a read in `flush(fn)` in an effect compute | none | it subscribes the effect |
| a read in `flush(fn)` inside `untrack` in a memo compute, and a read in `untrack` in the body | none | `untrack` clears the listener, and `flush(fn)` inherits the cleared one |

Before this, the body case was a miss and the rest were silent for the wrong
reason (the callback was classified as running outside tracking). No rule
reports the same read twice: the write rule's `flush(fn)` handling is unchanged.

**Stubs.** `node_modules/` is `write-scope-roots-rc3`'s, byte for byte: every
declaration the cases use is copied from the published `2.0.0-rc.3` typings as
that fixture's README and each file's header describe. `components.tsx`
type-checks cleanly against these stubs and against the real rc.3 and rc.9
installs (`tsc --noEmit`, TypeScript 5.9.3, `strict`,
`jsxImportSource: "@solidjs/web"`), and the checker reports the same findings
against all three.
