# Solid 1.x rootless artifact-identity control

This project contains a reduced local implementation named
`@solid-primitives/rootless`; it is not byte-identical to the published
`1.5.4` artifact covered by the first-party receipt. The analyzer must use only
facts established from the local implementation and must not import relational
return claims by package name.

Local inference preserves the delayed `doubled()` and `tripled()` reactive
reads. It cannot prove the additional `createRootPool` relation for
`quadrupled()`, so the old name-derived finding disappears rather than being
guessed. The ambient `opaqueFactory` control remains opaque.

The exact published rootless contract and receipt are checked independently by
the first-party bundle conformance gate. The fixture is valid under the
published TypeScript signatures.
