# leaf-scope-imported-base

**Claim (ADR 0317).** An exact project-class instance's chain resolves through a base class imported from another file (the demand plan requests the compiler symbol at every `extends`, `demand_plan.rs`): a clean imported base is none, a cleanup-registering one an `SC3001` violation, and a base with a computed-key member that may be `open` is `SC9012`. Stub files copy `leaf-scope-exact-method`.
