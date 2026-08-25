# JSX discarded-child coverage — Solid 1.x

This fixture revalidates discarded child-content behavior at
`solid1-dom-expressions-compiler`
`98d265c38dbf63e363c9846048a93461e66f44c7`.

Ordinary HTML void children and `<noscript>` children are no longer known
transform divergences. Where the producer emits no execution entry for their
source expressions, SC1001 fails closed with the ordinary missing-census
wording. The cleanup inside `<br>` exercises the ownership equivalent:
compiler silence must yield an uncertifiable obligation, not a proven
missing-owner violation.

The two genuine divergence controls are `<keygen>` and `<menuitem>`. Solid
1.x Babel treats those legacy tags as void while the Rust producer uses the
modern 14-tag set and lowers their children. Their SC1001 rows therefore retain
the compiler-disagreement wording. The paired Solid 2 fixture uses identical
source and stays silent for those reads because both Solid 2 compilers treat the
tags as non-void.

The ordinary `<span>` cleanup is a certified-owner control. The module-scope
`onCleanup` remains a proven violation.
