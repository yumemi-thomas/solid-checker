# Imported object members (ADR 0299)

A direct call of a stable const object's own function has that exact function's owner edge and read summary, including across a named import (`MissingOwner` reaches `onCleanup` after await; `Reads` covers arrows, `Object.freeze`, quoted keys, shorthand, default objects; `Owned` clean). Let/written bindings, replaced members, `Object.assign`, computed keys, getters, trailing spreads, escapes, namespace re-exports, nullable receivers and `this` escapes are refused. Solid declarations copy `dependency-source-owner` (published 2.0.0-rc.13).
