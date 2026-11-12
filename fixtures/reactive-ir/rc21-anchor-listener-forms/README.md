# Every way to the method hears a created anchor's click

ADR 0645, measured in Chrome: a bare `addEventListener`, a `.call` of the method and a literal key each run the listener on the
apply's stack, and its `flush()` warns `FLUSH_IN_EFFECT_CALLBACK`. Each stays SC2008's obligation.

ADR 0967 (B3): the `.call` of the method forwards its arguments to `listen`, a variable that holds the DOM method, and nothing resolves a variable's value to the declaration it was read from, so the flush in that listener (and only that one: the bare and the literal-key forms resolve the standard declaration) has no program-wide reason for SC2006 and SC3001 either: two more obligations beside SC2008's.
