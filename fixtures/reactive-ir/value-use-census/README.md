# value-use-census

**Claim (ADR 0692).** Every reference of a proven source is in a position the analysis follows, or an uncertifiable `SC9012` naming it. A form no table of `solid-facts` holds answers `value-use-unclassified`, never a silent clean.

`AtModule`, `InInner`, `InNamed`, `InDefault` and `InArrow` hold the same forms: an export's span is its whole statement, and a first draft answered "followed" anywhere inside one. Chrome warns at the `throw` forms in all three export placements.

`Followed` is the negative side: nothing there may answer a `value-*` blocker. `Object.keys` and `Object.getOwnPropertyNames` of a store run no getter (ADR 0810); `InNamed`'s `Object.entries` and `Object.values` Get every value, two SC1001 violations. `Created` starts at a call with no binding (P13). `Held` and `Importer` walk a value through an export, a prop and a context (P12).

Stub: rc.21; `solid-js.d.ts` is the siblings' plus `createContext`, `useContext`.

`Review` holds W's census review H1-H5, each once answered followed while Chrome warned: a default-exported literal or creating call (`Review*`; its importers walked, ADR 0923), a pattern in a parameter hop, a pending accessor at a tracked position, a store member through a slot, a bare `For` child.

`HeldSlots` and `HeldUses` hold a source where no row reads it, in another module: an IIFE's tuple, a `let` (initialized, and assigned later) and a class field. The holder's reference keeps its row, and so does each use the walk reaches in `HeldUses` (R1e's `wd-*` shapes, where only the holder had one): the body reads and writes are `SC9012` uncertifiable; the handler's write and the tracked JSX read have none.

`ReviewTwo` holds four positions ADR 0812 found followed unproven, each beside a negative: a pattern over a store root through a slot (`value-held-in-binding`; over the root, SC1003), a namespace member thrown and called (`ReviewNs`; called in JSX, followed), a pending bare child (`value-pending-in-jsx`; a signal, followed), a computed props key (`value-forwarded`).
