# A declaration does not stand for its runtime module

`helper` is entered by a visible direct call and named by a type query, which
alone would clear its caller-supplied-member obligation in this closed
application (ADR 0219). `main.ts` also enumerates the namespace of
`./bridge.js`, whose runtime module re-exports `helper`.

The compiler resolves `./bridge.js` to the hand-written `bridge.d.ts` beside
it and records no pairing with `bridge.js`. A declaration file is not a
project source, and its exports are not the runtime module's. A relative
specifier with an explicit `.js` extension loads exactly `bridge.js` (or its
TypeScript source) at run time: `bridge.js` is a program file, it re-exports
`helper`, and the obligation stays (`SC9012`).

`./bridge2` names a directory. The compiler reads its `types`
(`surface.d.ts`), while its `main` selects `runtime.ts` at run time, which
re-exports `helper2`. Nothing ties the declaration to that runtime module, so
any project file may be it: `helper2` stays (`SC9012`).

`linked` is installed under `node_modules`, and the compiler resolves it to
its declaration (`types`). Its source, `helpers.ts`, is part of this program,
so the package is not outside it, and its runtime `main` may re-export
`helper3`. A package directory that holds a program file is followed like any
other unknown target: `helper3` stays (`SC9012`).
