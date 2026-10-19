# member-value-copiers

**Claim (ADR 0320).** A literal's member is trusted only while no code in the program can replace a member of that name (no assignment such as `held.run = ...`, no reflective definition); `Object.assign(target, ...)` into a non-fresh target also withholds every project-class instance; both censuses are program-wide. `WrittenMember`, `AssignedLater` are `SC9012`; `Untouched` (the literal is the argument and `helper` only reads members) is none (ADR 0321). Scaffolding from `invocation-flow` (private).
