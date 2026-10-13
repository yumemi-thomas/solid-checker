# ADR 0169: An import a sibling barrel re-exports belongs to the names the entry publishes

- Status: accepted and implemented (2026-09-30); written with the implementation
- Date: 2026-09-30
- Owners: the chain walk (`rust/crates/solid-facts/src/ast/reexport_chain.rs`,
  `entry_names_publishing_import`), and the attribution ladder
  (`rust/crates/solid-facts-backend/src/main.rs`,
  `export_names_through_sibling_module`, reached from
  `export_names_of_reexported_import`, mechanism `reexported-import`)
- Relation: extends ADR 0133's rung by one condition. It reads ADR 0137 and
  ADR 0158's runtime edges through the ladder's own landing rule
  (`relative_target`). No new rung, no claim form, no protocol change.

## Context

ADR 0133 attributes an obligation filed at an import binding to the names that
publish it, but only when the binding is in the entry file itself. Its
condition 1 says why: a sibling module's export list publishes names the entry
may rename, or not publish at all.

A bundler emits the barrel one module down as well. `@solid-primitives/sse`
1.0.0-next.2 ships:

```js
// dist/index.js
import { json, lines, ndjson, number, pipe, safe } from "./transform.js";
export { …, json, lines, ndjson, number, …, pipe, safe };

// dist/transform.js
import { json, lines, ndjson, number, pipe, safe } from "@solid-primitives/utils";
export { json, lines, ndjson, number, pipe, safe };
```

The obligation for `utils`' `number` is filed at `transform.js`'s import
binding. Condition 1 declines it, no earlier rung answers, and `fallback-all`
marks every export of the entry. `number`'s only recorded cause in the
2026-09-30 checkpoint was that widening (which, being a widening, also hides
any cause behind it; see Consequences).

## Decision

An obligation filed exactly at an import binding's local identifier in a
package module other than the entry file is attributed to the entry names that
are that binding, when all of these hold:

1. The module lies inside the package root, and the same proof ADR 0133 § 2
   requires holds in that module: `reexport_only_import_names` shows the
   binding has at least one reference and every reference is a value specifier
   of a module-level `export { … }` with no `from` clause.
2. **Every name the entry exports is decided**, by walking its export
   declarations, to reach the binding or not to reach it. A name left
   undecided refuses the whole answer, because the undecided name might be one
   the binding reaches.
3. At least one name reaches it, and every such name is in the entrypoint's
   export map (ADR 0133 § 3).

The walk follows, from the entry's export of a name:

- an explicit export specifier of an import (`import { a as x } from "./m";
  export { x as y }`), through the binder's own resolution of the specifier's
  local to the import declaration;
- `export { a as y } from "./m"`;
- `export * from "./m"`, only when the name has no explicit export in that
  module and exactly one star target provides it.

Each relative specifier lands by the ladder's rule: ESM's relative-URL rule
first, then the generator's exact runtime edge for that `(importer, specifier)`
(ADR 0137, ADR 0158). A bare specifier lands elsewhere, except a `#imports`
alias, a self-reference by package name, an absolute path and a `file:` URL,
which can land inside the package and so are unresolved.

Everything else leaves the name undecided, and the ladder widens as before:

- a relative specifier without one exact landing, or a landing on a file this
  analysis did not read;
- a cycle, or a chain deeper than 16 modules;
- two star providers of one name, or a star of another package beside the
  chain (it might provide the name too);
- a namespace of an analyzed module (`import * as t`, `export * as t from`),
  which exposes the binding under every name that module has;
- a type-only export of the same name, a module that does not export what is
  imported from it (a CommonJS module states its exports nowhere this walk
  looks), and an unresolvable export specifier.

The entry's own binding rule (ADR 0133) is unchanged, and the entry file is
never the module this extension answers for.

## Soundness

ADR 0133's argument applies unchanged. An ESM import binding is immutable, and
if publishing is all the module does with it, the dependency's open claims
reach consumers only through the published names, which the dependency's own
contract describes. Nothing local evaluates the binding; an export that
does use a name raises its own obligation at the use.

What this ADR adds is only *which entry names those are*. The answer is exact
because it is a closed-world statement: every entry name is classified, and
each classification rests on either the binder's resolution of one specifier or
the ladder's own landing rule. Where a classification would need a guess (an
unresolved edge, an ambiguous star, a namespace), the walk says undecided and
the rung answers nothing. The shared assumption, with ADR 0137 and ADR 0158, is
that no package `browser` field or bundler alias remaps a relative module path.

## Consequences

- Pinned by `scripts/contract-reexported-import-attribution.test.mjs`
  (each half falsified against a binary built without this change):
  - `sibling`, `siblingRenamed` and `siblingStar` describe `local` exactly as
    the one-statement control does, and the re-exported name closes nothing;
  - `siblingNamespace` (the barrel's namespace is also published) and
    `siblingCycle` (a star cycle between entry and barrel) still mark every
    export.

  Two stars providing one name never reach the ladder in an end-to-end fixture:
  the generator refuses them earlier ("resolves through multiple star
  exports"). That case, an unresolved edge, a bare star and a type-only twin are
  pinned by the walk's unit tests (`reexport_chain.rs`).
- **Measured** with the checkpoint harness (`make primitives-checkpoint`'s
  steps, release binary), before and after at `bbde7700`, 97 probes per host:

  | | host free | `browser` | `node` |
  | --- | ---: | ---: | ---: |
  | exports clean, before / after | 100 / 100 | 100 / 100 | 130 / 130 |
  | exports with an `attribution widening` cause, before / after | 36 / 19 | 36 / 19 | 29 / 12 |

  The widening is gone from 17 exports per host (`sse`'s barrel names and
  `createSSE`, `createSSEStream`, `makeSSE`, `makeSSEAsyncIterable`; and seven
  exports of `form`), and no clean count moved. **`sse` `number` did not turn
  clean**: the widening had masked the next wall, which is the family's
  `returns never proposed`. The root node's record for `number` carries
  unresolved claims for `returns` and six domains the checkpoint does not
  track (`throws`, `cleanups`, `invalidates`, `disposals`, `writes`, a guard
  partition) and no widening: a re-export of a dependency function proposes
  no claim of its own in those domains. Two exports moved from degenerate to
  partial in each of host free and `browser` (`sse` `createSSE`,
  `createSSEStream`), and one in `node` (`form` `createForm`).
