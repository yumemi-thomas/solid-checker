# member-value-alternatives

**Claim (ADR 0320).** A callee that invokes a member of its parameter
(`helper(o)` calling `o.run()`) has its dispatch decided at each call site by
the argument passed there. The argument's implementations are trusted only
when every part of the value resolved and nothing can replace the member
before it runs:

- every alternative resolves (both arms of a conditional, every return of a
  factory); an alternative that does not withholds the whole value, and a
  resolved union keeps every candidate (so diverging implementations stay
  open);
- a literal's member is its last own property with that static key, with no
  spread after it, no getter, no computed or `__proto__:` key, and no function
  that uses its receiver; with no own property, exactly one spread supplies it;
- a `const` holding the object is unexported, not a parameter, and every other
  reference to it is a non-computed member access or an argument of an exact
  project callee that only reads members of it (it is shared with nothing
  that could replace a member);
- the callee itself uses its parameter only as such (or hands it whole to
  another callee of which that holds).

A function that hands its own parameter, whole, to an exact callee invoking a
member of it invokes that member too: the obligation moves to its callers.
The invocation's parameter index is the declaration's, also after a
destructured parameter.

| Case | Finding | Why |
| --- | --- | --- |
| `Resolved`, `TwoLiterals`, `SpreadBefore`, `ConstLiteral`, `ClosedInstance`, `ClosedArray`, `ForwardedResolved` | none | every alternative resolves to an inert implementation the program cannot replace (`TwoLiterals`' `SC1001` is the read of `props.c`) |
| `Conditional`, `ConstConditional`, `Cast` | `SC9012` uncertifiable | one arm is `opaque` |
| `Nullish`, `Or` | `SC9012` uncertifiable | a logical operand is unresolved |
| `SpreadAfter` | `SC9012` uncertifiable | a spread after `run` may replace it |
| `SpreadOnly`, `Factory`, `FactoryConditional`, `Reassigned` | `SC9012` uncertifiable | unresolved value |
| `MutatedByCall`, `EscapedInstance`, `EscapedArray` | `SC9012` uncertifiable | the object is handed to `mutate` before the call |
| `ReceiverMethod` | `SC9012` uncertifiable | a method of the literal hands `this` on |
| `LeakyCallee` | `SC9012` uncertifiable | the callee hands its parameter to `mutate` first |
| `Forwarded` | `SC9012` uncertifiable | `forward` hands its parameter to `helper`; `forward`'s caller passes `opaque` |
| `ShiftedParameter` | `SC9012` uncertifiable | the member runs on the second parameter; the first, destructured, is an inert literal |

`member-value-copiers` pins the program-wide census. `solid-js.d.ts`,
`tsconfig.json` and `node_modules` are copied from `invocation-flow`;
`package.json` is private. The sources pass `tsc --noEmit` with the fixture
stub.
