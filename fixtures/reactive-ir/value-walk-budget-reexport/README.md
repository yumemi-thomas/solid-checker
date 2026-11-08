# value-walk-budget-reexport

ADR 0878 (review D33): as `value-walk-budget-transitive`, `dep-b` walks a signal past the value census's budget and the project reaches
`dep-b` only through `dep-a`; here the project re-exports `dep-a`, never through an import declaration. That load is an edge
of the climb too: the budget row stands at the re-export (`App.tsx` line 2), never internal only.
