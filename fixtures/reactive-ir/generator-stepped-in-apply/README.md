# generator-stepped-in-apply

**Claim (ADR 0961, F2 and R1).** A generator's body runs where an iterator of it is stepped. In an effect's apply a `for…of`, a spread, a `yield*` chain, `.next()`, `Array.from`, `new Set`, a stored iterator's loop, or a generator literal handed to another function runs it there, and Chrome (rc.21) fires FLUSH_IN_EFFECT_CALLBACK and PRIMITIVE_IN_EFFECT_CALLBACK at the flush or the creation. The effect-phase origin walk followed no stepping and the generator was dropped (main 29b3f9312 certified it clean): the rows are now uncertifiable (SC2008, SC6002, `generator-step-unplaced`), never silence. The plain-function twins are the walk's own rows (a violation for the flush, an uncertifiable for the creation).

Silent: a generator the apply calls and discards, one stepped in the effect's compute function (the apply is not running there), one stepped by a handler.
