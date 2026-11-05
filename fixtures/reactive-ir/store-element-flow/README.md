# store-element-flow

ADR 0720: a call's result or a loop's element as a store value. Violations: identity functions and a memo returning the store.
Uncertifiable: elements (`find`, `for…of`, `filter`, `Object.values`, `For` rows), a declaration's identity. Else silent.
