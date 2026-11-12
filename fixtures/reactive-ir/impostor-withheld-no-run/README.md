# impostor-withheld-no-run

ADR 0874, R30-R33: a withheld call proves no run of its callback (`names`: the program's object typed as a list, whose
`forEach` queues it or hands it to `onCleanup`). Each site is uncertifiable in place: `HelperRead` SC1001 in the callback;
`ActionFlush` SC2006 at `flush()` (`provenStep`, on a real array, stays a violation); `RootWrite` SC2001. `Answered`: `reverse`
returns its receiver, so `xs.forEach` is uncertifiable (`impostor-host-intact` has the discarded answer).

ADR 0967 (B3): the program-wide veto (`handed-root-patch`) withholds every `forEach` of the program, so the callee of `names.forEach(() => { flush(); })` and of `list.forEach(() => { flush(); })` is a callee nothing resolves, which a package may be: no program-wide reason covers a flush in a function handed to it, so each flush also has an uncertifiable `SC3001` row (`leaf-operation-unaccounted`).
