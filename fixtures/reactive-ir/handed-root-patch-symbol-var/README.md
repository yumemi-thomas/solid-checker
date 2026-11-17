# handed-root-patch-symbol-var

ADR 1045 positive: `Number` is handed on, and the file's only write under a computed key is `this[_m$] = _p`, with
`var _m$ = Symbol()` in a module, never written and declared once: the shape of `@tanstack/solid-start-server`
2.0.0-rc.9 `dist/esm/StartServer.js:12`. The key is a symbol (a read before the declarator ran gives `undefined`,
which names no standard member), so the write is no `OpenMemberWrite(*)` and `Number.isFinite` is certified.
