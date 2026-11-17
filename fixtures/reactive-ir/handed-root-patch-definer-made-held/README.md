# handed-root-patch-definer-made-held

ADR 1045 negative (held as a value): as `handed-root-patch-definer-made`, but the definer is held in a binding and called
through it; the call is not the spelled standard definer, so the definer census keeps `ReflectiveDefinition(*)`.
`Number.isFinite` stays `standard-member-impostor`.
