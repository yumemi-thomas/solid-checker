# owner-emission-eval

**Claim (batch 11, ADR 0970; B3 of the review of #221).** A program that calls direct `eval` reaches any binding by name, so the owner emission's entry walk (`layer_entry_open`) takes every layer of it for an open entry: it asked `program_has_eval()` only after the two stops that close a layer (a component entered by tags alone, an owner-creating callback entered by its primitive alone), so `function Evaled() { onCleanup(..); return <p/> }` rendered by a tag and called by `setTimeout(() => { eval("Evaled()") })` stayed certified while rc.21 warns NO_OWNER_CLEANUP. The test now comes before both stops.

| site | outcome | why |
|---|---|---|
| `onCleanup` in `Evaled`, rendered by a tag and called through `eval` from a timer | SC4001 uncertifiable (`owner-callers-unenumerated`) | the eval reaches it by name with no owner. **The false clean this fixes** |
| `onCleanup` in `Other`, rendered by a tag | SC4001 uncertifiable | the eval may reach any binding by name: every layer of the program is open |

The control (a component only a tag renders, in a program with no `eval`) is `TaggedOnly` in fixture `owner-emission-layer-stops`: silent. Chrome: oracle case `fixtures/runtime-oracle/owner-emission-eval`: the eval'd component is `unproven-clean` on the head of the review and covered after the fix.
