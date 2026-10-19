# Partial structural return consumer

ADR 0177 consumer controls: contracts whose returned container lists members without closing the enumeration (`[accessor, unknown]`, `{ value: accessor, stop: unknown }`) prove each listed member at its position (`BadTupleRead`/`BadObjectRead` violations, good reads and handler uses clean, unlisted positions no violation) while `unknown` members and escaped whole returns are `reactive-dispatch-unresolved` (ADR 0234). Manifest bytes are `fixed-structural-return-consumer`'s.
