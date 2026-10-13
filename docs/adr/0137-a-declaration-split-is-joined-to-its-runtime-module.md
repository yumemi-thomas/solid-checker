# ADR 0137: A `.d.ts` split is joined to the runtime module Node loads

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the runtime edges (`packages/cli/scripts/artifact-resolution.mjs`,
  `runtimeModuleResolutions`, written by `generate-package-contract.mjs`'s
  `writeRuntimeModuleResolutions`), the existing join
  (`rust/crates/solid-facts-backend/src/main.rs`, `runtime_symbol_redirects`),
  and ADR 0135's guard (`imports_join_the_implementation`)
- Relation: restores the generator's half of a seam whose native half never
  went away. It adds no rung and no new join rule.

## Context

A published package ships a `.d.ts` beside each runtime module. TypeScript
resolves `import { Transitioner } from "./Transitioner.js"` to
`Transitioner.d.ts`, so the importer's binding carries the declaration's
symbol, while Node loads `Transitioner.js`. The call graph walks the
implementation's symbol, so it never sees callers across the split. ADR 0135
made the reachability rung refuse to narrow through such a helper, which is
sound. It left seven obligations of `@tanstack/solid-router@2.0.0-rc.8`'s root
node marking every export. Four of them sit in `Transitioner.js`, and every
render of `Transitioner` crosses the split. Even with ADR 0136's render edge,
the argument of `createComponent(Transitioner, {})` named no project
function.

The native side already had the join:

1. `--runtime-module-resolutions` reads `{ importer, specifier, target }`
   triples.
2. `runtime_symbol_redirects` accepts one only where Type Facts confirms that
   the specifier resolved to a declaration file, and only through compiler
   entities: the importer's binding on one side, the target module's export
   of the same name on the other.
3. It hands the redirect to the IR as an alias root.

`fixtures/package-contracts/declaration-sibling-reach` was built for this.

**Why the feed was empty.** `474c101f` ("migrate package contracts to
normalized v2") deleted `packages/cli/scripts/runtime-module-closure.mjs`, the
v1 closure walker whose `resolutions` fed the flag. Its replacement,
`resolvePackageArtifacts`, records closure entries and digests but not the
edges between them. The rewrite wrote a literal
`{"schemaVersion":1,"resolutions":[]}`, and the Phase 18 audit pinned that
literal as the document's version marker. Neither the commit nor any document
gives a soundness reason. The same commit moved
`declaration-sibling-reach`'s snapshot to `fallback-all` while its README kept
describing the feed.

## Decision

1. **The generator writes the runtime edges again, exact or absent.** For each
   analyzed resolution, `runtimeModuleResolutions` records every import or
   `export … from` of a runtime module in the case's closure whose literal
   relative specifier lands, by ESM's relative-URL rule alone, on another
   runtime module of that closure. All of these must hold:
   - both ends are closure entries of the resolution record, and both files'
     bytes hash to the record's digests;
   - the specifier starts with `./` or `../`, and contains no `?`, `#`, `%`
     or backslash;
   - it is joined against the importer's real directory, as Node does, with
     no extension or index guessing (only a bundler resolves `./m`);
   - the landing is not a declaration file;
   - the statement is not type-only.

   A batch writes the union: an edge is a fact about two files, and a
   relative specifier does not depend on conditions. The document keeps
   version 1 (`RUNTIME_MODULE_RESOLUTIONS_SCHEMA_VERSION`), and the Phase 18
   marker now names that constant.
2. **ADR 0135's guard reads the join.** `imports_join_the_implementation`
   compares the importer's binding and the declaration by *runtime*
   canonical symbol, after the redirects. Without a redirect the two stay
   apart and the guard refuses, as before.

## Soundness

The generator's answer names only which file the runtime loads, and only
where that answer does not depend on a resolver's policy. Every symbol-level
claim is still the compiler's:

- the redirect needs Type Facts' own resolution of the same specifier to a
  declaration file with no `includedPath`;
- it needs an exact entity at the importer's binding and at the target's
  export;
- two targets for one declaration root remove the redirect;
- anything the native side cannot join adds nothing.

A missing edge leaves ADR 0135's refusal in place, so every failure direction
is the existing `fallback-all`.

## Consequences

- Pinned by `scripts/contract-runtime-edge-attribution.test.mjs`:
  - `joined`: `caller → helper` across two splits. The generator records
    exactly the two `.js` landings, `local` keeps the control's closures, and
    only `caller` is opened.
  - `extensionless`: `./helper`. It records no edge for it, and stays
    `fallback-all`.

  Each half was falsified separately. With the feed written empty, or with
  the guard comparing plain canonical symbols, `joined` goes `fallback-all`.
- `declaration-sibling-reach` returns to what its README describes:
  `forwarded` is opened by the exact `reachability` rung, and `Isolated`
  now closes `reads`. No other fixture of the contract corpus (107) moves.
  Coverage (127 projects) is unchanged. It does not run the generator.
- On solid-router's root node, replayed exactly, the catch-all count falls
  from 9 to 5. `Transitioner.js`'s four go `reachability`. They are reached
  through ADR 0136's render edges from `Matches.js`, which is now joined.
  Still marking every export:
  - **`getNotFound` (3: `not-found.js` 105, 370, 408).** Its callers are
    `CatchNotFound` and `Match`, and both are entered as values:
    - `Match.js:44` puts `CatchNotFound` in a memo that `Dynamic` renders;
    - `Match.js:200` does the same with `Outlet`, which renders `Match`.

    Removing both value uses in a copy of the package clears all three. So
    what is missing is a rule for a component value that flows into
    `Dynamic`'s `component`, not a join.
  - **`getStoreFactory` (2)**, out of scope: it needs `RouterCore`'s
    constructor contract (ADR 0135).
- `createFileRoute`, `Link` and `Outlet` still propose no closure. Every
  remaining catch-all marks all 97 exports, and between them they cover the
  domains those three would need: `callbacks`, `reads` and `returns`.
