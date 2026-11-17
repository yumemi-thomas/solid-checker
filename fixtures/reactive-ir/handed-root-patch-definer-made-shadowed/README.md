# handed-root-patch-definer-made-shadowed

ADR 1045 negative (shadowed): as `handed-root-patch-definer-made`, but `Object` is the file's own object, whose
`defineProperty` is no standard definer and may write anything. `Number.isFinite` stays `standard-member-impostor`.
