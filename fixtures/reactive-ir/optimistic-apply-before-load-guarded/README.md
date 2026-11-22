# SC2009: a guarded write in an apply stays open (ADR 1175)

`if (v > 1) setO(2)` is not on every completion of the apply's first run, and Chrome is silent: uncertifiable.
