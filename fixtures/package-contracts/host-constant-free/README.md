# host-constant-{free,browser,node}: `isServer` is folded per certified host

ADR 0166. The three directories hold one package (`index.js`, `index.d.ts`,
`package.json` byte-identical) and differ only in the host the corpus certifies
it under (`hosts` in `../corpus.json`): none, `browser`, `node`. Read the
snapshots side by side.

`node_modules/@solidjs/web` is `@solidjs/web@2.0.0-rc.9` as published:
`package.json` and the six `dist/*.js` files are byte-identical to the archive
(sha256 `5de9244e…` for `package.json`; `web.js` `32083d72…`, `web.dev.js`
`bcbe0218…`, `web.observe.js` `b5b2950b…`, `server.js` `b96fc839…`,
`server.dev.js` `4636ed78…`, `server.observe.js` `9919deaa…`). Its `exports["."]`
selects `dist/web*.js` under `browser`, where the module states `const isServer =
false`, and `dist/server*.js` under `node`, where it states `const isServer =
true`. The fold reads that value from those bytes, for every file the host's
conditions may select, and only when they agree.

`types/index.d.ts` is reduced to the two declarations the package imports
(`isServer`, `isDev`), byte-faithful to the published lines. In the published
typings `isServer` is `boolean` on every host, so nothing in a declaration
decides it, and `tsc` has nothing to say about any export here. The
`solid-js` stub transcribes `onCleanup` and `Disposable` unreduced.

`onElementConnect` is `@solid-primitives/lifecycle@1.0.0-next.2`'s, byte for
byte.

| export | host free | `browser` | `node` |
| --- | --- | --- | --- |
| `onElementConnect` | `returns` a value, cleanup min 0 | unchanged: `el.isConnected` still decides | `returns: []`, no cleanup: the guard always returns |
| `guardedCleanup` | cleanup min 0 | cleanup **min 1**: the guard never returns | nothing runs |
| `shadowedGuard` | as host free | as host free | as host free: a local `isServer` is not the import |
| `namespaceGuard` | as host free | as host free | as host free: `web.isServer` is not a named import |

The host-free case is the control: both arms of every guard stay live, so
nothing about `isServer` is claimed. The two negatives stay unfolded under a
declared host, which is what pins that the fold binds the import, not the name.
