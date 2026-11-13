# flush-universe-package-hand-off-receiver-reads-twins

**Claim (the review of #229, B2; ADR 0967 amended), the silent side.** A function stored under a key is handed only when a receiver reads the key (or reads any key) in code a package may run. Here nothing does: `twinNeverRun` keeps the receiver in a local, and `twinNeverReads` reads the key, in functions no reference names or hands (a package runs neither); `twinKeyTarget[twinKey] = twinKeyBase` stores a prototype through a key the program does not name where no file spells `__proto__`, and the getter that flushes is read in a timer. The three flushes keep no row: no violation and no uncertifiable row.

The silence is the program's, as the stability proof of ADR 0300 is: one receiver used as a value in code a package may run (`flush-universe-package-hand-off-receiver-escapes`) would hand every stored function, so these twins are a fixture of their own.

Published bytes: `node_modules/steplib` is synthetic; `.solid-checker/runtime.json` closes the program boundary.
