# component-alias-entries

ADR 0331: a call proves no identity unless the census finds all entries; exact aliases are followed.

- Violation: `Plain`, `Chained`, `Imported`, `Handled`: called and rendered by alias (runtime-oracle `component-alias-entries`); a handler entry clears none.
- Uncertifiable: `Escapes.tsx`, `Exported.tsx` (passed, stored, written `let`, exported); `Reassigned2.tsx` (aliased binding written); `NestedWriter.tsx` (its host component proves nothing).
- `Kept.tsx`: call-proven SC1001/SC4001 violations stay; a write its calls allow is SC2001 uncertifiable.
- Clean (`Negatives.tsx`): alias only called, unused, `typeof`, in a handler, shadowed
