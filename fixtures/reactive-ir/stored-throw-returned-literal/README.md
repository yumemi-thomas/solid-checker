# stored-throw-returned-literal

**Claim (ADR 1058).** A plain throw (ADR 0893: a static `dynamic` source that is a promise) in a function boxed by a helper ends where the box's member is called. The helper is an exact same-file function declaration whose one completion is a straight-line `return` of an object literal that holds the parameter as a property value (`return { cb }`, `return { cb: cb, size: 1 }`), with the parameter read nowhere else. Each call makes a fresh literal, so a closed `const box = boxed(f)` (ADR 0351's conditions on the binding and on the literal) is that literal, and `box.cb()` runs `f` on its stack (ADR 1044's closed-literal member).

Violations: `BoxedCalled`, `KeyedCalled` (SC2007).

Uncertifiable: `HelperAlsoStores`, `TwoReturns`, `HelperSpreads` (no boxing helper), `BoxHandedOn`, `BoxWritten` (the box is not closed), `BoxCaught` (the call is caught), `ImportedHelper` (the helper is in another module: open). `LiteralBeforeDeclarator`, `BoxBeforeDeclarator` (ADR 1059's review: a hoisted declaration calls the holder before its declarator, a `ReferenceError`; a closed literal or box names its member only where every use runs after the declarator).

Chrome: `fixtures/runtime-oracle/stored-throw-returned-literal`. Stubs: `stored-throw-returned-param`'s. `tsc --noEmit` clean.
