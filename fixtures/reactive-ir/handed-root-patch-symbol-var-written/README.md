# handed-root-patch-symbol-var-written

ADR 1045 negative (written): as `handed-root-patch-symbol-var`, but `_m$` is reassigned to a string after its
declaration, so the key may be `"isFinite"`: the write stays `OpenMemberWrite(*)` and `Number.isFinite` stays
`standard-member-impostor`.
