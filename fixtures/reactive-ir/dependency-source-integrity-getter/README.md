# Package integrity: getter

ADR 0300: a selected package's sole provider and stable accessor-reading getter prove SC1001 reads, with two exactly rooted literal targets replacing `dependency-source-context-getter`'s wildcard. `Conditional.tsx`: both alternatives must close (`BothKnown` violation; unknown/inert branches SC9012; tracked clean); stable `location` object-binding destructuring is a violation (`MutableObject` SC9012). `solid-js.d.ts` is byte-identical to the original fixture (published rc.13 signatures).
