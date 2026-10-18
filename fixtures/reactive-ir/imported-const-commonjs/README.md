# Imported constants and CommonJS (ADR 0325 refusals)

A named import of an exported `const` holds a primitive only when the
declaring module is an ES module whose export is that `const`: the module
namespace is frozen. CommonJS gives no such guarantee, so each form below is
refused and keeps its `SC9012`: a script that sets `module.exports`
(`script.cjs`), an `export =` assignment (`equals.ts`), and an ES module that
also assigns `module.exports.CLUSTER` or `exports.CLUSTER` (`mixed.ts`,
`mixed-exports.ts`), which a CommonJS transform turns into a replacement of the
export. `Cluster` (`constants.ts`) is the settled control, so the fixture
cannot pass vacuously.

Types: the published rc.13 declarations of `solid-js` and `@solidjs/signals`,
copied unchanged from `standard-runtime-configuration`.
