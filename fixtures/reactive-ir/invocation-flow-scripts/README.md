# invocation-flow-scripts

**Claim (ADR 0319, review round 3).** The invocation census of a function
counts only the references its own file can see. That is complete for a
module's unexported binding, and for nothing else: a script's top-level
declarations are globals any file can call, a CommonJS file publishes through
`exports`, and a closure stored on `globalThis` can be called by name from
anywhere. The flow refuses every holder, parameter and census in a file
without `import`/`export` syntax, and treats every top-level binding there as
exported.

| Case | Finding | Why |
| --- | --- | --- |
| `ScriptGlobal` | `SC1001` uncertifiable | `fireGlobal` is a global of the script `globals.ts`; `App` calls it in its body with the `now` closure |
| `ModuleControl` | `SC1001` uncertifiable | the same through an exported module function (unchanged control) |
| `GlobalThisRelay` | `SC1001` uncertifiable | the module stores a closure that runs the callback on `globalThis`; `App` calls it by name in its body |
| `CommonJsExport` | `SC1001` uncertifiable | `cjs-lib.js` publishes `fireCommon` through `exports` |

`solid-js.d.ts` and `node_modules` are copied from `invocation-flow`;
`package.json` is private, which closes the program (ADR 0193). The sources
pass `tsc --noEmit` (`allowJs`) with the fixture stub.
