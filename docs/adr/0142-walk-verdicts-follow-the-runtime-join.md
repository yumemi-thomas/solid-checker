# ADR 0142: The generator's walk verdicts follow the runtime join

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the emit boundary's attach of the generator's walk verdicts
  (`attach_generated_owner_requirements`, `rust/crates/solid-facts-backend/src/main.rs`)
- Relation: applies ADR 0137's exact declaration-to-runtime redirects to one
  more reader. No new join rule, no new claim form, no producer change, no
  contract-format change.

## Context

`make certification-metric` ranked "`returns` never proposed" (243 exports,
53.9 % package-weighted) and "`creates` never proposed" (177) as the largest
missing claim forms. Grouping the 243 by what they return
(`rust/target/cf-scripts/classify-returns.mjs`, a scratch classifier over the
run's retained trees) found that **137 of them are not a shape at all**: the
export's implementation sits in a sibling module that ships a `.d.ts`, and the
entry file re-exports it (`import { clamp } from "./number.js"; export { clamp }`,
the shape every rolldown and tsup bundle in the corpus emits). 93 are in the
certifying package itself (`@solid-primitives/utils` 53, `event-bus` 10,
`event-listener` 10, `storage` 8, `props` 6, `scroll` 6), 44 in a dependency a
graph root re-exports (`@tanstack/query-core` 24, `@solid-primitives/utils` for
`@kobalte/core`'s `./colors` 20).

Measured on a four-file package: `export function h(x) { return x === 1; }` in
the entry file proposes `returns: [plain]` and `creates: []`; the same body in
`a.js`, imported and re-exported, proposes the same; add `a.d.ts` beside `a.js`
and it proposes **neither**, while `callbacks` and `reads` stay proposed.

TypeScript resolves `./a.js` to `a.d.ts`, so the entry file's export is an alias
of the declaration file's symbol. Every walk whose verdict feeds a proposal --
the valueless-completion walk (ADR 0035), the value-completion walk (ADR 0113),
the argument-container walk (ADR 0115), the merged-props walk (ADR 0109), the
`creates` walk and the owner requirements -- is indexed by the canonical symbol
of the function Node loads, and the attach looked the export up by its plain
canonical symbol. The two never met. `callbacks` and `reads` survived because
they come from the IR's own summary, which has read the export through ADR
0137's redirects since that ADR.

## Decision

**The attach reads the export's symbol through the runtime redirects**
(`runtime_canonical_symbol_from`), exactly as ADR 0137's guard does. The walk
indexes are unchanged: they already name runtime functions.

## Soundness

- Every verdict read here is a *proposal input*. A proposal certifies nothing;
  each closure it enables is decided by the certifier's census over the
  authenticated implementation, and the new proposals are refused by name
  where the census refuses them (`recursive-value-shape`, `unresolved-callee`,
  …).
- The redirect is ADR 0137's: Type Facts must confirm the specifier resolved to
  a declaration file, both ends must be exact compiler entities, and two
  targets for one declaration root remove it. Without a redirect the symbols
  stay apart and the export proposes nothing, as before.
- `export { name } from "./a.js"` is not joined: the redirects are built from
  import bindings only. Such an export still proposes neither domain. No corpus
  package spells a re-export that way; `declaration-sibling-proposal`'s `isOdd`
  pins the gap.

## Consequences

- `fixtures/package-contracts/declaration-sibling-proposal` is the tracer:
  `isEven`, `reset` and `choose`, re-exported across the split, propose exactly
  what `isZero`, declared in the entry file, proposes (`plain`, `returns: []`,
  a return of each argument; `creates: []`), and `isOdd` proposes neither.
- The certification metric, on the release binary, with ADR 0143 in
  (`phase22/2026-09-28-certification-metric-baseline.md`, the section on what
  "`returns` never proposed" was): "`returns` never proposed" 243 → 181,
  "`creates` never proposed" 177 → 76, clean 43 → 44, degenerate 697 → 689,
  misuse-capable 157 → 160. `@solid-primitives/utils`' `./immutable` `clamp`
  certifies clean; most of the rest now carry a named census refusal instead
  of no record, above all `operation census refused: recursive-value-shape`
  (the baseline's wall 5, 88 → 135 exports).
- `esm-barrel`'s `createValue` (`create-value.mjs` beside `create-value.d.mts`)
  now proposes what its sibling `createLocal` does; no other corpus fixture
  moves.
- It exposed ADR 0143 at once: the new plain returns in `@tanstack/query-core`
  and `@solid-primitives/utils`' `./colors` were restated as `returns: []` by
  the graph roots that re-export them, and the empty-return veto refused both
  rows.
