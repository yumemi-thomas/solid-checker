# value-walk-budget-transitive

ADR 0878: `dep-b` creates a signal and walks it past the value census's 512 steps, so the cut's root is in
`dep-b`, which the project never imports: it reaches `dep-b` only through `dep-a`. Both are selected dependency sources
(`.solid-checker/runtime.json`). The budget row climbs the dependency import edges from `dep-b` to `dep-a` and stands at the project's
import of `dep-a` (`App.tsx` line 1), so the cut is reported, never internal only. Synthetic packages, as `dependency-source`'s `dep-hooks`.
