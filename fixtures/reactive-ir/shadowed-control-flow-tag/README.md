# shadowed-control-flow-tag

**Claim.** A JSX tag names a dialect primitive only by symbol identity. A
local component spelled `For` shadows the imported `For`. It never has to
call its child, so a read in that child is not a strict read
(`jsx_primitive_name` in `solid-reactive-ir/src/lib.rs`). The spelling
fallback still applies to a tag the entity table does not resolve.

| Case | `SC1001` | Why |
| --- | --- | --- |
| `Shadowed` | none | the local `For` is not the primitive |
| `Real` | violation | the imported `For` runs its child while rendering |

Before this fixture, `Shadowed` was a proven violation: a false positive. The
stubs are copied from `prop-head-get`.
