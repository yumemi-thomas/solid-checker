# Interprocedural methods in Solid 2

Class, generic parameter dispatch, conditional candidates and ordinary function
controls retain their source-based read summaries. The unresolved computed
target stays uncertifiable.

ADR 0320: GenericCall and EquivalentConditionalCall pass `objectReader`, an
exported `const` of `source.ts`. A binding referenced from other files is no
longer resolved through its initializer (a retained answer here would not see
those files' edits), so the dispatch is an `SC9012` obligation instead of the
`SC1001` read through `objectReader.read`; both stay uncertifiable.

ADR 0299 review: ObjectCall is now silent. objectReader escapes into the generic
helper calls, so its receiver census does not admit the direct member's original
body. TypeScript's selected property declaration cannot bypass that census.
