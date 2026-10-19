# Imported constants and CommonJS (ADR 0325 refusals)

A named import of an exported `const` holds a primitive only when the declaring module is an ES module whose export is that `const` (the namespace is frozen); CommonJS gives no such guarantee, so a script setting `module.exports`, an `export =` assignment, and an ES module also assigning `module.exports.X`/`exports.X` are refused and keep `SC9012`; `Cluster` is the settled control. Published rc.13 declarations from `standard-runtime-configuration`.
