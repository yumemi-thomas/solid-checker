# sc1003-then-keys

**Claim (ADR 1122).** SC1003's store-key tests read a destructure pattern's properties by value (facts schema 100:
`object_properties`, `pattern_rest`), never its source text. A pattern whose every property reads `then`, in any spelling
(`ThenDefault`, `ThenRenamedDefault`, `ThenNestedEmpty`, `ThenEscaped`, `ThenRenamed`, `ThenQuoted`), or with a computed key
no fact names (`ThenComputedComment`: a `const` behind a comment) is uncertifiable `store-key-undecided`; a pattern with no
property reads nothing, comments aside (`EmptyComment`, `EmptyLineComment`: no row). Another key still warns: `ThenAndV`,
`CommentAndV`, `ComputedLiteralV` are violations. Main reported all nine of the first as violations (the review of ADR 1107
round 2); Chrome (`runtime-oracle/sc1003-then-keys`) is silent at them and warns at the three. Stub: `destructure-store-slots`'s.
