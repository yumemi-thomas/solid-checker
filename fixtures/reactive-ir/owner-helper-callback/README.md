# owner-helper-callback

**Claim (ADR 0474).** A function literal handed to an exact project helper that calls that parameter in its own synchronous body runs in the helper's call, on its stack and under the caller's owner (ADR 0457's census). So `onCleanup` in it is `NO_OWNER_CLEANUP` where the caller has no owner. Violations (rc.17 warns `NO_OWNER_CLEANUP`, fast path): `NullOwner` (`runWithOwner(null, …)`), `Timer`, `Then`, `Apply` (an effect apply), `Imported` (a helper from another file), `AlsoKeptTimer` (a helper that also keeps the callback: one run is in place), `Handler` (a dispatched handler; nothing here dispatches synchronously). Before, every case was silent: the owner graph had an edge to the helper, none to the callback.

Legal (rc.17 silent): the helper called in a component body (`Body`, `MaybeBody`) or a created root (`InRoot`); a helper that only keeps the callback (`NeverCalledTimer`) or runs it under its own root (`OwnRootTimer`); a same-named local helper that only keeps it (`ShadowedTimer`).

Uncertifiable (internal): a helper that calls it on some paths only (`MaybeTimer`, rc.17 warns: `on` holds), and one that also keeps it for a later run (`AlsoKeptBody`, rc.17 silent: nothing calls `kept`). Stub: rc.17 shapes (copied from `read-in-place-layers`).
