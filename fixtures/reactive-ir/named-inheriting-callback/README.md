# named-inheriting-callback

**Claim (ADR 0517).** A callback a primitive names by its binding runs code its in-place layers (a closure called once, a helper's callback, an IIFE) hold where the layer's call in the callback's body runs, as the inline callback does (ADR 0469, 0515):

- `Root.tsx`: a `createRoot`/`runWithOwner` position (ADR 0323: the callback runs in its call's own window) answers the role at the call when it is the function's sole by-name entry (`by_name_position`); SC1001 violations in `RootClosure`, `RootDeclarationHelper`, `RunWithOwnerClosure`. Any other entry is unknown: `RootShared`, `RootCopy`, `RootInHandler` are uncertifiable. Clean: JSX the callback or a layer returns (`RootJsx`, `RootLayerJsx`: the compiler's role), a timer (`RootTimer`), past the callback's own await (`RootAfterAwait`). Uncertifiable: a library callback that may run zero times (`RootEmptyForEach`), a function named at its own `createRoot` (`RootSelf`).
- `Untrack.tsx`: `named_callback_write_role` climbs the layers before scanning the callback's positions, so `UntrackShared` is an SC2001 violation; several owner-keeping positions with no rejection are unknown (`UntrackSharedInHandlers`); a handler stays clean.
- `Resolve.tsx`: `resolve()` climbed to a function a tracked position names (`named_at_tracked_position`) is SC2004 (`MemoResolveClosure`), through `[1].forEach` an obligation (`MemoResolveForEach`); the tracked read is clean.

Before, every one of these was silent and the file certified. Probes: `scripts/fuzz/probes/named-callback/`.
