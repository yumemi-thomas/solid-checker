# dead-site-empty-collection-shadowed

**Claim (ADR 1019).** Only the standard-library carrier is read as never running: a project `Map` whose `forEach` calls its callback, and a local `Array` whose `from` does, are no carrier. `ShadowedArray`'s write stays a violation; `ShadowedMap`'s stays uncertifiable (`function-run-unplaced`, as on main). Both identical to main 993e83f74. Stubs: `dead-site-empty-collection`'s.
