# SC9012: a store handed whole to a standard function (ADR 1178)

A store root handed to `Object.values`, `Object.entries`, `Array.from`, `Object.hasOwn`, `deep` or `snapshot`, spread into an
array or object literal, iterated by a `for…of` with a plain binding, or put in a template is read there, and that read has the
whole-store row of ADRs 0620/0621 (none for `snapshot` and `hasOwn`, which read nothing on a settled store). The value-use census
follows the root through the call into what it answers, so `TrackedForms` (a memo) is clean and `BodyForms` keeps its proven
SC1001 violations with no census row. With `SOLID_CHECKER_IR_ABLATE_SILENCE=value_use.whole_store_argument` each of the eleven
references in `TrackedForms` is `value-in-carrier` again.

Stays uncertifiable: a result read further (`ResultUsed`: an element may be a nested store, whose Get has no row), a shadowed
callee, a pattern loop binding, a call spread and a callback on the result (`NotTheForms`), and `snapshot`/`hasOwn` on a store
that may not have settled (`Unsettled`: a derive, an optimistic store, a `const` value). The impostor twin is
`store-whole-standard-census-impostor`.
