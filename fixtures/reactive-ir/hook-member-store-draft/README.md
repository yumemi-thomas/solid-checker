# hook-member-store-draft

**Claim (ADR 1093).** A store setter an expression-bodied hook returns in a literal, called through a member of the hook's result with a draft callback (`thing.useIt((s) => { s.v = 7; })`), writes through its draft: no row. Before, the container-root walk (`container_root::handed_store_setter`) reached the setter reference in the returned literal and found no symbol for it (the Type Facts entity table holds none there; a block-bodied hook's `return` does), so the write was SC2003 uncertifiable, `unproven-container-root`. The walk now asks the binder's symbol for the reference, the same question.

| component | outcome |
|---|---|
| `Draft` | proved: no row |
| `Handed` (the member hands the store itself to the callback) | SC2003 uncertifiable, `unproven-container-root` |
| `Misuse` (a write through the store the member holds) | SC2003 violation |

`solid-js.d.ts` is `async-creation-pass`'s with `createStore` (its setter takes a partial or a draft function).
