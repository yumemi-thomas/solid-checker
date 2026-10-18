# instance-key-writes

**Claim (ADR 0320, review round 6).** An assignment through a non-literal
key (`target[key] = value`) on a receiver that is not a proven built-in value
may write any member of whatever it reaches. A project constructor can hand
`this` to such code, so the program-wide key-copier census counts it and no
project-class instance's member is trusted. `xs[i] = v` on a proven built-in
array does not count.

| Case | Finding | Why |
| --- | --- | --- |
| `KeyWrite` | `SC9012` uncertifiable | `Box` registers each instance; `put` writes `run` through a key nothing names |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`invocation-flow`; `package.json` is private. The sources pass `tsc
--noEmit` with the fixture stub.
