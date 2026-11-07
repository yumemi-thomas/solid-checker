# value-walk-budget

ADR 0878 (review D33): a value walk the 512-step budget cuts owes a row of its own, `value-use-unclassified` with detail `budget`, at the
walk's root, filed past every deduplication and cover: what lies past the cut is unknown, so the walk is never silent. `useLong`'s
parameter has more uses than the budget: one budget row, which replaces the plain row the cut left at the same span. `useShort` (negative)
stays under the budget: no budget row. A root in dependency source with no project reference puts the row at the project's import of
that package (corpus: compass-ui's imports of `@kobalte/core`).
