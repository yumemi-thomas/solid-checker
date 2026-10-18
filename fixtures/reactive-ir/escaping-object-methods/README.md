# escaping-object-methods

**Claim (ADR 0320, review round 6).** An object literal's shorthand method is
a named helper whose reads are claimed where it is called only while every
use of the literal is a member call the analysis resolves: the literal is a
call's receiver, a `const` of this module whose every reference is one, or
an argument of an exact callee in this file that only reads members of its
parameter. Anywhere else (handed to unknown code, enumerated, returned,
stored) the method is a stored callback, like a function-expression property:
its reads are uncertifiable.

| Case | Finding | Why |
| --- | --- | --- |
| `HandedToUnknown`, `HeldThenHanded` | `SC1001` uncertifiable | the literal reaches `opaqueFn`, which has no body in the program |
| `Enumerated` | `SC1001` uncertifiable | `Object.values` hands the method to a callback that runs it in the body |
| `DispatchedOnly` | `SC1001` violation | control: the only use is `held.run()`, resolved to the method, whose read runs untracked in the body |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`invocation-flow`; `package.json` is private. The sources pass `tsc
--noEmit` with the fixture stub.
