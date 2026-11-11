# handed-root-patch-definer-key-any

ADR 0951: the helper defines its parameter under a key no literal names (`ReflectiveDefinition(*)`): any member may be
replaced, so `Number.isFinite` and `Object.defineProperty` stay withheld, as before the narrowing. Chrome: STRICT_READ_UNTRACKED.
