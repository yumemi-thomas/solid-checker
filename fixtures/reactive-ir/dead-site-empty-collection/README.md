# dead-site-empty-collection

**Claim (ADR 1019).** A callback handed to a standard-library inline iteration of a literal nothing can fill never runs, so a write (SC2001), an action call (SC2002) or `onFlush` (SC3002) in it is uncertifiable (`function-unentered`), never a violation (`carrier_runs::carrier_provably_never_runs`, read by `SiteReach`).

Uncertifiable, each a violation on main 993e83f74: `EmptyForEach` (`[].forEach(tf)`), `EmptyInlineLiteral` (a literal handed to `[].map`), `EmptyReadonlyFilter`, `EmptySort`, `ReduceOneNoInitial` (`[1].reduce(tf)`), `FromEmpty` (`Array.from([], tf)`, `Uint8Array.from([], tf)`), `FreshSet` (`new Set().forEach`), `FreshMapAction` (SC2002), `EmptyOnFlush` (SC3002), `ReplaceNoMatch` (`"x".replace("z", tf)`).

Violations (the callback runs, or may): `NonEmpty`, `ReduceWithInitial`, `SetWithElement`, `ReplaceMatch`, `AlsoCalled` (a second reference calls it) run in Chrome; `NamedEmpty`, `ChildNodes` never run but are not literals (a known false positive, docs/open-work.md). `Wrapped` stays uncertifiable (`function-run-unplaced`). The shadowed variants are `dead-site-empty-collection-shadowed`: a project carrier withdraws the standard member's facts program-wide, so it lives apart.

Each signal is made in its component: a module-scope tuple is withdrawn by the program's array writes. Chrome: `fixtures/runtime-oracle/dead-site-empty-collection` (quiet) and `-live` (5 of 5 covered). `tsc --noEmit` clean.
