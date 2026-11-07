# impostor-symbol-key-lie

ADR 0875's negative: a key that starts like a symbol and is a string. `Symbol("forEach").description` stays `*`, so `items().forEach` stays
uncertifiable, `standard-member-impostor`. One `*` holds the whole program, so the other spellings (`Symbol.keyFor`, a `let` rewritten, a
shadowed `Symbol`, `Symbol.species`) are the fuzzer's `recv-symbol-*` twins, each its own program.
