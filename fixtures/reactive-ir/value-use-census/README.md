# value-use-census

**Claim (ADR 0692).** Every reference of a proven source is in a position the analysis follows, or an uncertifiable `SC9012` naming it; a form no `solid-facts` table holds is `value-use-unclassified`, never silent.

`AtModule`, `InInner`, `InNamed`, `InDefault`, `InArrow`: the same forms in each export placement (an export's span is its whole statement). Chrome warns at the `throw` forms in all three.

`Followed` is the negative side: no `value-*` blocker. `Object.keys`/`getOwnPropertyNames` of a store run no getter (ADR 0810); `InNamed`'s `Object.entries`/`values` Get every value (two SC1001 V). `Created`: a call with no binding (P13). `Held`, `Importer`: through an export, a prop, a context (P12).

Stub: rc.21, the siblings' plus `createContext`, `useContext`.

`Review`: W's census review H1-H5 (default-exported literal or call, ADR 0923; a pattern in a parameter hop; a pending accessor at a tracked position; a store member through a slot; a bare `For` child).

`HeldSlots`, `HeldUses`: a source held in another module (an IIFE's tuple, a `let`, a class field). The holder's reference and each use the walk reaches keep a row: body reads and writes U; the handler's write and the tracked JSX read none.

`ReviewTwo`: ADR 0812's four, each beside a negative: a pattern over a store root through a slot (`value-held-in-binding`; over the root, SC1003), a namespace member thrown and called (`ReviewNs`; in JSX, followed), a pending bare child (`value-pending-in-jsx`; a signal, followed), a computed props key (`value-forwarded`).
