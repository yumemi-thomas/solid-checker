# instance-key-writes

**Claim (ADR 0320, round 6).** An assignment through a non-literal key (`target[key] = value`) on a receiver that is not a proven built-in may write any member of whatever it reaches; a project constructor can hand `this` to such code, so the program-wide key-copier census counts it and no project-class instance's member is trusted (`xs[i] = v` on a proven built-in array does not count). `KeyWrite` is `SC9012`. Scaffolding from `invocation-flow` (private).
