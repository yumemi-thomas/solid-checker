# component-alias-entries

ADR 0331: a visible call proves no identity unless the census finds every entry. Exact aliases (`const`, unwritten `let`) are followed.

- Violation: `Plain`, `Chained` (`App.tsx`) and `Imported` (`Importer.tsx`) are called in tracked JSX and rendered by alias (runtime-oracle `component-alias-entries`).
- Uncertifiable (`Escapes.tsx`, `Exported.tsx`): also passed, stored, aliased by a written `let`, or exported from an open program.
- `Kept.tsx`: call-proven SC1001/SC4001 violations stay; a write legal at its calls is SC2001 uncertifiable.
- Clean (`Negatives.tsx`): alias only called, unused, `typeof`, in a handler, shadowed.
