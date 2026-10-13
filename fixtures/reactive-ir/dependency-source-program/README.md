# Dependency source: the analyzed program (ADR 0284, review round 2)

Claim: dependency source mode analyzes exactly the project files the project's
own program holds, plus the selected package's closure, and binds only
imports that resolve into the selected installed copy.

- `App.tsx` (positive): SC1001 at `useLabel(name)`, read through the selected
  package's `solid` source.
- `e2e/Harness.tsx`: outside the `include` inherited through `extends`
  (`tsconfig.base.json`). It is never analyzed, in either mode. Composing the
  sibling tsconfig from the base's own text once replaced that `include` with
  `**/*`.
- `legacy/Legacy.tsx`: imports another installed copy of the package,
  `legacy/node_modules/dep-hooks` 2.0.0, whose runtime reads nothing. It stays
  SC9011 in both modes. A redirect once bound it to the selected copy's
  source because the import resolved to *a* declaration file.

`solid-js.d.ts` is `context-member-identity`'s stub (claim-bearing signatures
copied from published 2.0.0-rc.13). The sources pass `tsc --noEmit`.
