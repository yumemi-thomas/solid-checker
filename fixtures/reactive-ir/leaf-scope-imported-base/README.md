# leaf-scope-imported-base

**Claim (ADR 0317).** An exact project-class instance's chain resolves
through a base class imported from another file. The demand plan requests the
compiler symbol at every `extends` expression (`demand_plan.rs`), so
`class_instance_is_exact` finds the imported base class instead of reading the
chain as unresolved.

| Case | Finding | Why |
| --- | --- | --- |
| `ImportedBase` | none | `Camera extends Transform` (imported); `perspective` creates nothing in the leaf scope |
| `ImportedBaseForbidden` | `SC3001` violation | `Leaky.start` registers a cleanup in the leaf scope |
| `ImportedBaseShadowed` | `SC9012` uncertifiable | the imported base has a computed-key member, which may be `open` |

Before ADR 0317 all three were `SC9012` uncertifiable: the base was
unresolved. `solid-js.d.ts` and `tsconfig.json` are copied from
`leaf-scope-exact-method`. The sources pass `tsc --noEmit` with the stub.
