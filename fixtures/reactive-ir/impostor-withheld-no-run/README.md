# impostor-withheld-no-run

ADR 0874, review R30-R32: a call the census withholds proves no run of its callback. `names` is the program's own object typed as a list, and
its `forEach` queues the callback, or hands it to `onCleanup`. Each site stays uncertifiable at its own place, never a violation at a caller
or a step:
- `HelperRead`: SC1001 in the callback, none at `h()`, the helper's call.
- `ActionFlush`: `step`'s SC2006 uncertifiable at `flush()`; `provenStep`, on a real array, keeps its violation.
- `RootWrite`: SC2001 in `createRoot` in the callback.
R33, `Answered`: `reverse` answers with its receiver, so the exemption ends and `xs.forEach` is uncertifiable. `Discarded` keeps it.
