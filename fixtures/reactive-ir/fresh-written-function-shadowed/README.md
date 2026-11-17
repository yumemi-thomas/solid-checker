# fresh-written-function-shadowed

ADR 1041 negative: a function declaration's name is shadowed by a `let` holding the caller's object, and the write goes through the `let`. `Number.isFinite` keeps its `standard-member-impostor` row.
