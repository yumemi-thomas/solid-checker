# package-structured-unresolved

Pins the explicit SC9012 outcome for exported object-return shorthands whose
runtime value cannot be joined exactly: a valid bare package import, a
path-mapped import, an ambiguous relative module, and a global binding. An
unresolved export cycle is deliberately absent because TypeScript reports it
(TS2303). The namespace-import negative proves that an exact
namespace object is not treated as a possibly-reactive leaf.

The local `solid-js@1.9.14` and `bare-package@1.0.0` declarations keep every
case type-correct. They are intentionally narrow but preserve the exact
signatures used by the fixture; no finding depends on a looser callback or
return type.
