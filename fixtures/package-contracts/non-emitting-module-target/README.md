# An entrypoint whose authenticated bytes emit no JavaScript

`"./types/*": "./types/*"` is the published shape of `@kobalte/utils`'s
`"./src/*"` and of `@solidjs/h`'s and `@solidjs/universal`'s `"./types/*"`: an
unconditional wildcard over a directory the package also ships. The finite
wildcard census walks the artifact's members and expands the pattern over each
one, so every file under `types/` becomes an artifact-case candidate on the
**runtime** axis.

Four of those members emit no JavaScript at all, and all four are recorded in
the refusal sidecar's `inapplicable` array as `non-emitting-module-target`. The
recorded `reason` names which of the two premises answered, because a member's
suffix selects exactly one:

**`erasable-statements`** — every module-level statement in the bytes is
erasable. The suffix is not read at all.

- `types/kinds.ts` — two type aliases and an interface. This is
  `@kobalte/utils@2.0.0-alpha.0`'s `src/types.ts`.

**`declaration-file`** — the member's suffix is `.d.ts`/`.d.mts`/`.d.cts`, its
bytes parse under declaration-file grammar, and they carry no implementation
body, initializer, expression statement or side-effect import. TypeScript
decides declaration-file semantics by suffix and emits nothing for such a file
*at all*, including for the re-export forms a plain module would emit.

- `types/renderer.d.ts` — two interfaces and an `export declare function`. This
  is `@solidjs/universal@2.0.0-rc.3`'s `types/universal.d.ts`, and it is the
  case that makes the premise necessary rather than convenient: `createRenderer`
  is a *value* name, so the export census records it as a runtime binding on
  both axes, and any claim the emitter made about it would be a claim about a
  module that never executes. It is also answered by the *erasable* premise, so
  it needs no suffix.
- `types/barrel.d.ts` — `export * from` plus a named re-export. This is
  `@solidjs/universal`'s `types/index.d.ts` and `@solidjs/h`'s. It is answered
  **only** by the declaration-file premise: the identical bytes in a runtime
  member are a working barrel, and `non-emitting-module-target-control` ships
  exactly those bytes as `./runtime-barrel`, where the case **certifies**.
- `types/hyperscript.d.ts` — a default export naming a binding these same bytes
  declare ambiently. This is `@solidjs/h`'s `types/hyperscript.d.ts`.
  `export default 1;` is not this shape and the control pins its refusal.

`types/widget.js` is reached through the same wildcard key and **certifies**, as
does `.`. That is what makes the four dispositions census decisions rather than
suppressions: an inapplicable case never refuses, never certifies, never counts
as a refusal, and never suppresses a sibling case or the proposal. The fixture
therefore records zero refusals.

## Why the suffix is admissible for the second premise, and only there

The 2026-09-02 revert rejected classifying a `.d.ts` runtime target *before
authentication*: the suffix named a file nothing had bound to the archive, so a
member-kind or symlink substitution could decide the answer. Two things make the
second premise here different, and both are required:

1. **The member is authenticated first.** `ArtifactSnapshot::from_archive` has
   already refused a non-regular member, a case-folding collision and a
   duplicate member whose bytes differ, so the suffix names bytes nothing can
   substitute.
2. **The suffix never acts alone.** It is conjoined with a declaration-grammar
   parse *and* with a gate that refuses an implementation body, an initializer,
   an expression statement and a side-effect import anywhere in the tree — the
   TS1183/TS1039 shapes that make a publisher's `.d.ts` claim false. The control
   fixture's `./implemented` and `./evaluated-default` are exactly those, and
   they refuse.

The first premise remains blind to the filename, which the control fixture pins
from the other direction: a `.js` member whose bytes are
`export declare function createRenderer(): void;` is inapplicable, because a
suffix is the publisher's claim about a file while the statement list is
evidence about it.

**The deliberate trade** (recorded in `docs/precision-backlog.md`): both
premises say "there is no runtime surface to certify", not "a consumer succeeds
here". A Node consumer that reaches one of these members fails —
`ERR_UNKNOWN_FILE_EXTENSION` for a `.ts`/`.d.ts`, a `SyntaxError` for ambient
bytes in a `.js` — exactly as one that reaches the `.css` entrypoint
`non-module-target` already covers. `interface X {}` with no export at all, and
a bodyless overload signature with no implementation (a `tsc` error), are
inapplicable here by design: this rule tells a written module from a broken
build, not an exported surface from a private one.

## What certification owes for these two cases

Both rows carry `applicability: "verifier-proved-type-only"`, and that tag is a
debt, not a decoration. The disposition is decided here from the *installed*
tree, which nothing has authenticated, so each case travels to certification as
a declared claim in the planning request's `inapplicableCases`, and Rust
re-proves the identical predicate against `snapshot.read(path)` of the
authenticated archive
(`ArtifactSnapshot::prove_non_emitting_module_target`), selecting the premise
from the authenticated member path rather than from anything the claim says. A
claim the archive refutes refuses the whole proposal, naming the case and the
first emitting statement with its byte range.

The generator and the verifier must therefore agree statement for statement, and
they are two separate implementations — `statementEmits`/`statementDeclares` in
`packages/cli/scripts/artifact-resolution.mjs` over TypeScript's AST, and
`solid_facts::ast::module_emission` over Oxc's. What holds them together is one
shared corpus, `fixtures/module-emission/cases.json`: 146 snippets, each with a
verdict per premise, read by the tests on **both** sides. A divergence is a test
failure there rather than a whole-proposal refusal in the field. Never edit a
corpus verdict to match an implementation.

The fixture corpus here runs generation only, so it pins the generator half. The
verifier half is pinned by unit tests in
`rust/crates/solid-facts-backend/src/contract_certification.rs`, the two
`prove_declared_applicability` call sites by
`a_planning_request_refuses_a_declared_applicability_the_archive_refutes`
(`tests/contracts_process.rs`) and
`a_graph_node_refuses_a_declared_applicability_the_archive_refutes`
(`src/main.rs`).

## The boundaries, and where each is pinned

- **A module that declares nothing is not a type module.** An empty statement
  list, or a body that is only `export {}`, is indistinguishable from a broken
  build. `wildcard-asset-entrypoints` ships a zero-byte and a comments-only
  member (both refused) and `multi-entrypoint`'s `./empty` is `export {}`
  (refused).
- **An empty *export surface* is not the premise.** A side-effect-only module
  exports nothing and emits everything;
  `non-emitting-module-target-control` pins that refusal, which is the exact
  regression the 2026-09-02 revert was about.
- **The declarations axis is never asked.** The disposition selects
  `axis: "runtime"` only, so a package whose `types` arm is a non-emitting
  `.d.ts` beside a real runtime arm is untouched — `declaration-sibling-reach`
  certifies exactly as before.
- **The two inapplicable classes do not overlap.** An asset suffix stays
  `non-module-target`; `wildcard-asset-entrypoints` pins that, and this fixture
  ships no asset.
