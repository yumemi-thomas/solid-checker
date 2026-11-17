# owner-boxed-callers

**Claim (ADR 1059).** A function a helper boxes in a fresh literal (ADR 1058: an exact same-file `function boxed(cb) { return { cb } }`, a closed `const box = boxed(f)`) runs where `box.cb()` is called, so the owner graph gives `f` that call as its caller (`owner_discovery.rs`, `callee_closed_member_function`). `useContext` in `f` reads the call's owner.

Violations: `BodyContextNotFound` (SC4002: the component's owner, no provider), `TimerNoOwner` (SC4001: no owner on a timer's stack). Clean: `BodyWithDefault`. Uncertifiable: `TimerBoxHandedOn` (the box is handed on, not closed), `TimerImportedHelper` (the helper is in another module), `BodyBeforeDeclarator` (a hoisted declaration calls the box before its declarator: a `ReferenceError`, caught).

Chrome: `fixtures/runtime-oracle/owner-boxed-callers`. Stubs: `stored-throw-returned-literal`'s. `tsc --noEmit` clean.
