# A sibling `.d.ts` is joined to its exact runtime module before attribution

**The trap this fixture exists for: `channel.d.ts` must stay beside
`channel.js`.** Delete it and the fixture silently stops testing anything — the
call graph resolves the edge exactly again and `.` narrows back to `forwarded`
alone, which is what `parameter-member-forwarded` already pins.

`index.js` writes `import { channelFor } from "./channel.js"`. TypeScript
resolves that specifier to `channel.d.ts`, not to `channel.js`: a declaration
file wins over an adjacent implementation in every resolution mode the
generator uses. The two files are unrelated modules to the compiler, so the
call in `forwarded` carries the *declaration's* runtime identity and the
implementation's symbol has no reference outside `channel.js` at all.

Without the generator's runtime-resolution fact, everything downstream of that
identity split fails in the same direction:

- `function_call_sites` finds no caller for `channel.js`'s `channelFor`, so the
  call graph enumerates the helper alone;
- `entered_only_through_calls` walks the same symbol's references, finds only
  the declaration and the export specifier, and reports the enumeration
  **complete**;
- the reachability rung then resolved the obligation to *no export*, and both
  `forwarded` — which really does reach it — and `Isolated` were published
  certified.

Since a published package almost always ships a `.d.ts` beside each runtime
module, that silent certification was the normal case, not the exotic one. The
previous soundness repair detected the unaccounted module surface and widened
the whole entrypoint to `fallback-all`; sound, but it also marked `Isolated`.

The package generator now supplies the fact the compiler intentionally does
not: the successful static runtime edge for the exact importer, literal
specifier, and runtime target selected by the same closure walk that seeded the
analysis. The backend accepts it only when Type Facts confirms that exact
specifier resolved to a declaration file with no compiler-provided
`includedPath`, then joins the import binding and runtime export through exact
compiler entities. Reactive IR uses that runtime symbol as the alias root, so
the existing call graph sees `forwarded -> channelFor`; no filename pairing or
name-only fallback is involved.

- `.` — only `forwarded` goes unknown, by the exact `reachability` rung.
  `Isolated` reaches nothing and retains its independently proven summary.
- `./direct` — the control, and the half that stays exact. Its entry file *is*
  `channel.js`, so `channelFor` resolves to an export name and the module
  surface question never arises: the export publishes its exact
  `parameter-member` row and carries no unknown claim, exactly as in
  `parameter-member-forwarded`. A regression that widened unconditionally would
  break here.

## The closure record is part of this fixture's claim

`expected-generation.json` pins it, because here the record is the only place the
split is visible as *bytes*: `.`'s record names `channel.d.ts` **and**
`channel.js`, which is the analysis reading a declaration where a runtime module
sits beside it. A record that dropped declaration files would erase the evidence
for the very finding this fixture is about, and one that reported the pair as a
seeding gap would double-report it (see `attestedClosure`).

`./direct` pins the mirror: its entry file *is* `channel.js`, and its record
names that file alone. The sibling target `./index.js` is excluded from it
exactly as the analysis excludes it, so the two records cannot silently merge.

Neither record names anything under `node_modules/`. The solid-js stub is a
dependency's bytes: excluded from the record deliberately, because hashing it
would bind this fixture's record to the stub's version and to whether the install
was hoisted or nested. What the analysis read from a dependency is described by
that package's own contract; see docs/precision-backlog.md for the residue.
