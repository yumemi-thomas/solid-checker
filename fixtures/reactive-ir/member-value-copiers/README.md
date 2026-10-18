# member-value-copiers

**Claim (ADR 0320).** A literal's member is trusted only while no code in the
program can replace a member of that name: no assignment of the name
(`held.run = …`) and no reflective definition of it. Code copying keys into
existing objects (`Object.assign(target, …)` with a target that is not a fresh
literal) additionally withholds every project-class instance. Both censuses
are program-wide.

| Case | Finding | Why |
| --- | --- | --- |
| `WrittenMember` | `SC9012` uncertifiable | `held.run` is assigned |
| `AssignedLater` | `SC9012` uncertifiable | the same program assigns `run` (in `WrittenMember`) |
| `Untouched` | none | the literal is written as the argument and `helper` only reads its members: no write elsewhere can reach it (ADR 0321) |

The copy into `held` would also make `AssignedLater` open on its own: `held`
is handed to `Object.assign`, an argument no project callee receives.
`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`invocation-flow`; `package.json` is private. The sources pass `tsc --noEmit`
with the fixture stub.
