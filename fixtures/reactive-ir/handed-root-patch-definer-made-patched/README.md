# handed-root-patch-definer-made-patched

ADR 1045 negative (patched definer): as `handed-root-patch-definer-made`, but the file replaces `Object.defineProperty`,
so a call of it on a made object may run anything: the first-patch induction's hypothesis fails and
`ReflectiveDefinition(*)` holds again. `Number.isFinite` stays `standard-member-impostor`.
