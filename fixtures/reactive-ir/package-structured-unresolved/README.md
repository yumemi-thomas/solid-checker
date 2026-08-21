# package-structured-unresolved

Pins the explicit SC9012 outcome for an exported object-return shorthand whose
runtime value cannot be joined exactly: a global binding. The reviewed
`bare-package@1.0.0` contract certifies both a direct import and that same
structured external value behind a relative project re-export.
Compiler runtime identity now resolves both the tsconfig
path-mapped import and TypeScript's selected target for an otherwise ambiguous
relative spelling. An unresolved export cycle is deliberately absent because
TypeScript reports it (TS2303). The namespace-import negative proves that an
exact namespace object is not treated as a possibly-reactive leaf.

The local `solid-js@1.9.14` and `bare-package@1.0.0` declarations keep every
case type-correct. They are intentionally narrow but preserve the exact
signatures used by the fixture; no finding depends on a looser callback or
return type. The external re-export case proves that a reviewed package
summary survives a project barrel without turning an uncontracted value into
project-owned proof.
