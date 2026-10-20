# invocation-flow-scripts

**Claim (ADR 0319, round 3).** The invocation census counts only references the function's own file can see, complete only for a module's unexported binding: a script's top-level declarations are globals, a CommonJS file publishes through `exports`, and a closure on `globalThis` is callable anywhere. The flow refuses holders, parameters and censuses in files without `import`/`export` syntax and treats every top-level binding there as exported: global, `globalThis` and CommonJS shapes are `SC1001` uncertifiable.
