# parameter-use-forms

**Claim (ADR 0320, review round 6).** A non-rendering function's parameter
use that no member invocation describes is an obligation at the function's
callers:

- a member read whose value may be called (destructured, extracted, handed
  to another call) is a possible invocation of that member, resolved at each
  call site like a call of it;
- an alias that reaches a call, a wrapper, spread, array element,
  conditional or non-test logical value that is an argument of a call, an
  argument of a callee that is not an exact project function (or a forwarding
  call in another function, such as a returned closure), and `arguments` are
  `unmodeled`: no call site can resolve what they run, and only a primitive
  or nullish argument settles them.

A whole-parameter argument of an exact project callee stays the forwarding
rule's; tests, arithmetic and template uses hand nothing on (implicit
conversions are an analysis-wide premise).

| Case | Finding | Why |
| --- | --- | --- |
| `Alias`, `Spread`, `Wrapper`, `ArrayElement`, `Conditional`, `ArgumentsObject`, `StoredClosure` | `SC9012` uncertifiable | the parameter reaches `helper` (or a wrapper's consumer) in a form no call site resolves |
| `Destructured`, `Extracted`, `HandedMember`, `DestructuredParameter` | `SC9012` uncertifiable | `run` is read off the parameter and called, or handed to `queueMicrotask` |
| `TestedOnly`, `PrimitiveArgument` | none | a test hands nothing on; a primitive literal argument carries no member |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`invocation-flow`; `package.json` is private. The sources pass `tsc
--noEmit` with the fixture stub.
