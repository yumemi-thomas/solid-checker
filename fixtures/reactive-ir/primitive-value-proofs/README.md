# primitive-value-proofs

**Claim (ADR 0320, primitive-value proofs).** A forwarded (`unmodeled`)
parameter use or a handed-on literal member is settled at a caller only by a
value proven primitive (and not `undefined`) from what produces it, never
from a declared type. In a closed program the proof follows producers across
functions and files (`attribution.rs`, `CallGraph::value_is_primitive`):

| Case | `SC9012` | Why |
| --- | --- | --- |
| `SignalPrimitive` | none | initial value and every setter write (an updater included) are primitive |
| `SignalObjectWrite` | uncertifiable | a setter call writes an object |
| `SignalSetterEscapes` | uncertifiable | the setter is handed to `register`, which may write anything |
| `SignalUndefinedInitial` | uncertifiable | the initial value is `undefined`, which selects a default |
| `MemoPrimitive` | none | the bare memo's compute returns a template |
| `MemoObject` | uncertifiable | the compute returns an object |
| `EffectApplyPrimitive` | none | the apply callback receives what the compute returns, a number |
| `EffectApplyObject` | uncertifiable | the compute returns an object |
| `HelperParameterPrimitive` | none | every call of the local helper passes a primitive |
| `HelperParameterAsserted` | uncertifiable | one call passes `opaque as unknown as number` |
| `Label` (in `Props`) | none | every tag passes a string or a template |
| `LabelAsserted`, `LabelMissing` (in `Props`) | uncertifiable | an asserted value; a tag without the attribute passes `undefined` |
| `FunctionResult` | none | every return of `describe` is a template, on every path |
| `FunctionFallsThrough` | uncertifiable | `maybe` can fall off its end and return `undefined` |
| `LiteralMember` | none | the fresh literal's `id` is a string literal and `readId` only reads members |
| `LiteralMemberAsserted` | uncertifiable | the literal's `id` is asserted |

The `SC2001`/`SC1001` violations in the signal and memo cases are those
rules' own claims about writing and reading in the component body; they are
unchanged by this proof. `solid-js.d.ts`, `tsconfig.json` and `node_modules`
are copied from `obligation-exclusion-boundary`; `package.json` is private.
The sources pass `tsc --noEmit` with the fixture stub.
