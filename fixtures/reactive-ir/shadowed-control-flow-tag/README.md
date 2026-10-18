# shadowed-control-flow-tag

**Claim.** A JSX tag names a dialect primitive only by symbol identity. A
local component spelled `For` shadows the imported `For`. It never has to
call its child, so a read in that child is not a strict read
(`jsx_primitive_name` in `solid-reactive-ir/src/lib.rs`). The spelling
fallback still applies to a tag the entity table does not resolve.

| Case | `SC1001` | Why |
| --- | --- | --- |
| `Shadowed` | uncertifiable | the local `For` is not the primitive; the child is its `children` prop, and nothing proves when it runs it (ADR 0326; silent until then) |
| `Real` | violation | the imported `For` runs its child while rendering |

Before this fixture, `Shadowed` was a proven violation: a false positive. Until
ADR 0326 it was silent, which claimed a placement nothing proved. The
stubs are copied from `prop-head-get`.
