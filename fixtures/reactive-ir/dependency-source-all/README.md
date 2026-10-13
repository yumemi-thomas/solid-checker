# Dependency source: every Solid package (ADR 0291)

Claim: `dependencySources: ["*"]` selects every Solid package the project
loads at run time, and every Solid package those load, each resolved from
where its importer is. A Solid package is one whose `dependencies` or
`peerDependencies` name the dialect's runtime (`solid-js`, `@solidjs/web`,
`@solidjs/signals`) or an `@solidjs/` package.

| Case | Package | Expected |
| --- | --- | --- |
| `LabelNow` | `dep-hooks` (peer `solid-js`) | SC1001 at the call |
| `Inner` | `dep-hooks` → `dep-inner`, from `dep-hooks/node_modules` | SC1001 at the call |
| `Plain` | `plain-utils` (no Solid dependency) | SC9011: not selected, an unknown package |
| `Pattern` | `dep-pattern` (`exports` pattern `./*`) | SC1001 at the call: the pattern resolves as Node resolves it |
| `ArrayExports` | `dep-array` (`exports` array fallback) | SC9011 and SC9005: skipped with a note, an unknown package |

`solid-js.d.ts` is `context-member-identity`'s stub (claim-bearing signatures
copied from published 2.0.0-rc.13). `App.tsx` passes `tsc --noEmit`.
