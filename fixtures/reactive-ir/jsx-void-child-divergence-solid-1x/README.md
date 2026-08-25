# JSX discarded-child coverage — Solid 1.x

This fixture revalidates discarded child-content behavior at
`solid1-dom-expressions-compiler`
`ca3bbfae7d1e00e28ef73f9af58bdb46e248b512`.

Ordinary HTML void children and `<noscript>` children carry positive trace
facts. Positions the compiler keeps are tracked; positions it deletes are one
`Elided` child-list range. The cleanup inside `<br>` exercises the ownership
equivalent: the deleted operation is absent rather than inferred safe from
compiler silence.

Solid 1.x now treats `<keygen>` and `<menuitem>` as void, matching its shipped
Babel compiler, and positively reports their child lists discarded. The paired
Solid 2 fixture uses identical source and tracks those children because Ryan's
`next` semantics treat both tags as non-void. Both outcomes are certified.

The ordinary `<span>` cleanup is a certified-owner control. The module-scope
`onCleanup` remains a proven violation.

The JSX-valued shadowed-`children` arm retains the outer elided decision while
retracting the nested read, so the deleted operation is silent without hiding
its live source-child control.
