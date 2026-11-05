# Every way to the method hears a created anchor's click

ADR 0645, measured in Chrome: a bare `addEventListener`, a `.call` of the method and a literal key each run the listener on the
apply's stack, and its `flush()` warns `FLUSH_IN_EFFECT_CALLBACK`. Each stays SC2008's obligation.
