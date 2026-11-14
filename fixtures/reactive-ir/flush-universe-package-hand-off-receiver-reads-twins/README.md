# flush-universe-package-hand-off-receiver-reads-twins

**Claim (the review of #229, B2; ADR 0967 amended), the silent side.** A function stored under a key is handed only when a receiver reads the key (or reads any key) in code a package may run. Here nothing does: `twinNeverRun` keeps the receiver in a local, and `twinNeverReads` reads the key, in functions no reference names or hands (a package runs neither); `twinKeyTarget[twinKey] = twinKeyBase` stores a prototype through a key the program does not name where no file spells `__proto__`, and the getter that flushes is read in a timer. The three flushes keep no row: no violation and no uncertifiable row.

The silence is the program's, as the stability proof of ADR 0300 is: one receiver used as a value in code a package may run (`flush-universe-package-hand-off-receiver-escapes`) would hand every stored function, so these twins are a fixture of their own.

Published bytes: `node_modules/steplib` is synthetic; `.solid-checker/runtime.json` closes the program boundary.

**Added by the review of #232 (B3).** `twinPattern`: a pattern parameter (`function twinPatternRead({ twinPatternM })`) reads every key where the function runs, and the function that passes the receiver to it is one nothing hands a package, so the read is on no package's stack and the flush keeps no row (the same pattern run by a function a package may run is `flush-universe-package-hand-off-receiver-patterns`).

**ADR 0980 (a named cost).** `twinPattern` is bound to `Object.create(null)`, a call's result, which is no fresh object the program writes, so its store is now handed: the flush at `twinPattern.twinPatternM = function () { flush(); }` keeps the three uncertifiable rows (SC2006, SC2008, SC3001) and the project is `uncertifiable`, not `certified`. The other twins stay silent.
