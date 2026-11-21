# effect-cleanup-disposal-reachable

**Claim (ADR 1167, the premise it keeps).** A `createRoot` that takes its `dispose` lets project code dispose an owner on a stack it
chooses (`DisposedInBody` calls it in the component body), which ADR 0695 does not place: the never-rerunning effect's returned
cleanup keeps SC1001 uncertifiable. Chrome rc.21 (`runtime-oracle/effect-cleanup-disposal-reachable`) is quiet at this call; the case
pins that the proof stands down, not a warning. Stub: `effect-cleanup-disposal-only`'s.
