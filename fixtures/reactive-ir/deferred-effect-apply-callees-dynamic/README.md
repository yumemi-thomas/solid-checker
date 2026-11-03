# Deferred effect apply: callees (static dynamic)

`deferred-effect-apply-callees` for `static-dynamic-async-source` (ADR 0590):
a static `dynamicComponent` of a promise in a function only a never-run
`defer: true` apply calls is silent; the same call from an apply whose compute
reads a retained writable source is a violation. Declarations as in
`rc17-dynamic-component` (byte-faithful rc.17); tsc-clean against them and
against published rc.17.
