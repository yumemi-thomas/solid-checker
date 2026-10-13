# tuple-slot-context

**Claim (ADR 0282).** A tuple slot is followed only from the fresh array a
factory call returns. A context provider's value is one array shared by every
consumer, and any of them can replace a slot, so `const [, peek] =
useContext(TupleContext); peek()` claims nothing.

| Case | Finding | Why |
| --- | --- | --- |
| `TupleConsumer` | none | the provider's array is shared; an array pattern of a context value also blocks that context in ADR 0273's receiver census |
| `ObjectConsumer` | `SC1001` violation | control: the same provider census admits a returned object's member |

The program is closed (`"private": true`), as the provider census requires.
`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from
`context-member-identity`, unchanged; its claim-bearing signatures are copied
from published 2.0.0-rc.13. `tsc --noEmit -p` passes.
