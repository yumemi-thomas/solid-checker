# Solid 1.x rootless callback-result consumer

The exact reviewed `@solid-primitives/rootless@1.5.4` contract states that
`createSubRoot` invokes callback parameter 0 inline in a created owner and
returns that callback's result. The delayed `doubled()` read proves that the
schema-v1 `callback-result` relation preserves a returned memo as a reactive
source. Its published signatures are reproduced without loosening the types;
the project is valid under `tsc --noEmit`.
