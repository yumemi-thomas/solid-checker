# merge-omit-view-spread-handler

**Claim (ADR 1129), closed program.** The function a caller writes under a listener key reaches an intrinsic element's spread through a `merge`/`omit` view of the component's props, so it is that element's handler: the spread's SC4001 `handler-spread-unresolved` obligation closes and the function joins the owner graph as a handler target (ADR 0358). `Trigger`'s spread and `Defaulted`'s (the listener is the view literal's own arrow) are proved. Misuse twin: the second `Trigger` caller's handler creates a render effect when the event fires; it stays uncertifiable (SC4001 `owner-varies-by-caller`), and Chrome's NO_OWNER_EFFECT there is covered (`scripts/runtime-oracle.mjs`, verdict `covered-uncertifiable`). `Spreaded` is refused: a caller spreads into it, so its keys are not read.

`.solid-checker/runtime.json` closes the program boundary, as `handler-placement-closed` does. Stubs and pinned sources as in `merge-omit-view-spread`.
