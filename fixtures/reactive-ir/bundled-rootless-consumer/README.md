# Solid 1.x rootless relational-return consumer

The exact reviewed `@solid-primitives/rootless@1.5.4` contract states that
`createSubRoot` invokes callback parameter 0 inline in a created owner and
returns that callback's result. The delayed `doubled()` read proves that the
schema-v1 `callback-result` relation preserves a returned memo as a reactive
source. `createSingletonRoot` and `createRootPool` instead return functions;
invoking those functions yields their factory callback's result. The delayed
`tripled()` and `quadrupled()` reads prove the narrower
`callback-result-function` relation without claiming that every callable
factory result is reactive. The ambient `opaqueFactory` control has no local
body and therefore stays fail-closed. Published signatures are reproduced
without loosening the types; the project is valid under `tsc --noEmit`.
