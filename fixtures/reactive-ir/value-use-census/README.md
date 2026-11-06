# value-use-census

**Claim (ADR 0692).** Every reference of a proven source is in a position the analysis follows, or it is an uncertifiable `SC9012` naming that position. A form no table of `solid-facts` holds answers `value-use-unclassified`, never a silent clean.

`AtModule`, `InInner`, `InNamed`, `InDefault` and `InArrow` hold the same forms. A test written only at module scope proves nothing about the export arm: an export's span is its whole statement, and the first draft answered "followed" for any position inside one. Chrome warns at the `throw` forms in all three export placements.

`Followed` is the negative side: nothing there may answer a `value-*` blocker. `Object.keys` and `Object.getOwnPropertyNames` of a store run no getter (ADR 0810); `InNamed`'s `Object.entries` and `Object.values` Get every value, two SC1001 violations. `Created` starts the walk at a call with no binding (P13). `Held` and `Importer` walk a value through an export, a prop and a context (P12).

The stub is rc.21's published `solid-js` version; `solid-js.d.ts` is the sibling fixtures', plus `createContext` and `useContext`.

`Review` holds the five holes of W's census review (H1-H5), each a program Chrome warns on that the census once answered followed: a default-exported literal or creating call (`ReviewArray`, `ReviewObject`, `ReviewCreated`), a pattern inside a parameter hop, a pending accessor at a tracked position, a store member through a slot, and a bare `For` child.

`HeldSlots` and `HeldUses` hold a source where no row reads it, in another module: an IIFE's tuple, a `let` (initialized, and assigned later) and a class field. The holder's reference keeps its row, and so does each use the walk reaches in `HeldUses` (R1e's `wd-*` shapes, where only the holder had one): the body reads and writes are `SC9012` uncertifiable; the handler's write and the tracked JSX read have none.
