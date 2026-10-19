# Local helper resolved-call demand (ZA)

Leaf-owner expectations: demand widens through local lexical references but a function is not certified because it was demanded. `LocalHelperForbidden` is an SC3001 violation; clean helpers (two hops to `String.trim`, setter updater, `satisfies`-wrapped) none; shadowed helpers, opaque members and imported helpers SC9012. The stub is a byte copy of `leaf-scope-host-callbacks/solid-js.d.ts` (its cleanup/leaf-callback signatures must not be loosened).
