# ADR 0219: A type query and a namespace call are accounted entries

- Status: accepted and implemented (2026-10-06).
- Owners:
  - `AstFacts::type_queries` and `ModuleLoadFact::specifier_span`
    (`solid-facts/src/ast/mod.rs`);
  - the entry census in `solid-reactive-ir/src/attribution.rs`
    (`entered_only_through_call_expressions`,
    `compute_entered_only_through_calls`, `module_namespace_escapes`,
    `specifier_reaches`, `module_exposes`);
  - `runtime_semantics::is_node_builtin_module`;
  - `contract_identity_scope` (`solid-facts-backend/src/lib.rs`).
- Relation: widens the entry census that ADR 0203 uses to drop a closed
  program's exported caller-supplied-member obligation (`SC9012`,
  `exported-parameter-member-dispatch`), and the reach walk's census.

## Context

In a closed application (ADR 0193), an exported helper that calls a member of
its parameter keeps a declaration obligation unless every reference to it is a
call expression the graph resolves. Each such call then resolves the member
from its own argument, or files its own obligation (ADR 0203).

Two kinds of reference were not accounted for, so the obligation stayed even
when every runtime entry was a visible call:

```ts
export function createGpuCanvas(gpu: GpuRoot, canvas: HTMLCanvasElement) { … }
export type GpuContext = ResultValue<ReturnType<typeof createGpuCanvas>>;
```

```ts
import * as Findings from "./findings";
Findings.list({ findings: pool });
```

- A TypeScript type query (`typeof createGpuCanvas` in a type position) is
  erased. It names the function without entering it.
- A namespace call's graph edge spans the whole callee `Findings.list`. The
  helper's own symbol reference spans only the property `list`, so it never
  matched a known call site.

Admitting either is sound only if no other runtime code reaches the module as
a namespace object, which enters an export without naming it. That question
needs to know which module each import specifier loads, and only the
compiler knows that.

## Decision

1. **Type queries are a syntax fact.** `AstFacts::type_queries` records the
   span of every `TSTypeQuery` (facts schema 49). A runtime `typeof value`
   expression is a value and is not recorded. A reference inside a type
   query is accounted for: it is erased.
2. **A namespace call accounts for its property reference** only when the
   property's exact semantic entity resolves to the same declaration as the
   resolved call edge. A computed member, another property, or a value read
   of the property accounts for nothing.
3. **The admissions hold only while the module is not reachable as a
   namespace object.** A census that needed decision 1 or 2 to close also
   requires that no runtime code reaches the module as a namespace. The
   forms checked are:
   - a namespace import (`import * as`) or `import ns = require(…)` that a
     runtime reference uses other than as the object of a static member, or
     that is exported;
   - an `export * as` namespace re-export;
   - a dynamic `import()` or `require`.

   Each counts when it names the module, or a module that exposes it
   transitively, through `export … from`, `export *`, or an export
   (`export { x }`, `export default x`) of a binding it imported. A census
   that closes with direct calls alone is decided as before.
4. **The compiler's module resolution names the module.** Every project
   file's specifiers are attested from the Type Facts module graph
   (`contract_identity_scope` now covers every file). A dynamic load is
   joined by its runtime argument literal's own span
   (`ModuleLoadFact::specifier_span`, facts schema 50), so a type argument
   with the same text (`require<typeof import("x")>("x")`) cannot answer for
   it. For an attested occurrence:
   - an unresolved specifier may name anything;
   - a project file is followed;
   - a relative specifier with an explicit runtime extension
     (`../dist/index.js`) loads exactly that file, or the TypeScript source a
     bundler maps it to (`index.ts`). Those that are project files are
     followed; any other is a runtime file outside the program;
   - a file whose real path lies under `node_modules` is an installed
     package and exposes no project module;
   - any other target, such as a declaration whose runtime module a package
     `main`, a link or a `paths` alias selects, may expose anything.

   A declaration file is never traversed as runtime evidence.

   Without a row (TypeScript records none for `require` in a `.ts` file):
   - a relative specifier is resolved against the project's files, failing
     closed when that does not resolve;
   - a Node built-in name (`fs`, or any `node:` specifier) loads the
     runtime's module, plus any project file the compiler maps that name to
     elsewhere;
   - any other bare specifier may name anything.
5. **A reference in a file the project does not analyze is not accounted
   for**, and **a symbol Type Facts does not know bounds nothing.** A known
   symbol with no references has no entries in the analyzed program.
6. **Premises.** These are stated, not checked. The analysis has
   TypeScript's resolution only, not the runtime's, and decision 4's two
   negative answers rest on them:
   - the analyzed program is the whole runtime program (ADR 0193), so a
     package file, or a runtime file outside the program, imports no project
     module;
   - a relative specifier with an explicit extension loads that exact file or
     its TypeScript source. This excludes:
     - a bundler alias on a relative path;
     - a directory named like a file (`bridge.js/`);
     - a symlinked relative directory;
     - a file reached under another spelling, such as different case.

     A specifier with a backslash, an empty segment, a query or fragment, or
     `..` above the root is not taken as exact;
   - a file whose real path lies under `node_modules` is not an analyzed
     source;
   - neither `paths` nor a bundler remaps a Node built-in name onto a project
     file, except where the compiler attests such a mapping for that name;
   - under `preserveSymlinks`, a workspace package reached through its link
     is not identified with its source.

## Consequences

- An exported helper named in a type alias, or called through a namespace
  import, no longer keeps a declaration obligation when its runtime entries
  are all visible calls and no namespace object reaches its module.
- **Conservative, not exact:**
  - a qualified type reference (`ns.Item`) counts as a runtime use of the
    namespace;
  - any non-relative dynamic import or `require` the compiler did not
    resolve may name any module;
  - a module loaded dynamically as a whole keeps its obligations.
- **Open, not caused by this change:** a census that ADR 0203 closes with
  direct calls alone does not consult namespace escapes, so a module both
  called directly and enumerated through its namespace still clears.
  Guarding every census refused 237 closures on the rc.13 corpus. See
  docs/precision-backlog.md.
- Kept for later, all still uncertifiable: components entered through JSX,
  class members, functions registered with a package (Hono, Convex), local
  value escapes, tests and stories.

## Evidence

- **Research proposal** `rust/target/research/volume/entries/` (a Codex
  agent). As delivered it refused closure for every symbol with no
  references; decision 5 narrows that guard.
- **Adversarial review rounds** by Codex agents (`review-entries.md`,
  `review-r4.md`, `review-r5.md`, `review-r6.md`, `review-r7.md`, `review-r8.md` under
  `rust/target/research/`). They found the namespace escapes behind
  decision 3, then:
  - `import = require`;
  - barrels, including `export default` of an imported binding;
  - non-relative specifiers;
  - a declaration beside its runtime module;
  - another occurrence's resolution standing for an unattested one;
  - a built-in name mapped onto a project file;
  - a type argument joined as the runtime specifier;
  - a workspace link whose declaration hides a program bridge;
  - a directory whose `main` and `types` differ.

  Decision 4 answers the last seven. A last round (`review-r8.md`) showed
  that the remaining negative answers cannot be made exact without runtime
  resolution facts: a relative alias, a `.js`-named directory, a symlinked
  relative directory, or a workspace source under `node_modules` defeats
  them. They are stated as premises (decision 6) rather than failing closed,
  which would clear no corpus site. A first version, which refused every
  non-relative specifier, cleared none of the 30 corpus candidates.
- **Diagnosis:** the 16 sites recovered in `app-game` were all blocked by
  two loads:
  - `import('./js/abr-js-runtime.js')`, which only the compiler maps to
    `.ts`;
  - `require("fs")`.

  These were found with temporary instrumentation, removed before commit.
- **Fixtures:**
  - `closed-export-erased-query`: a type query clears; a runtime `typeof`
    and an array escape stay.
  - `closed-export-namespace-call`: `ns.describe` clears. These stay:
    - modules reached through `Object.values(ns)`, `ns["picked"]` and
      `import("./loaded")`;
    - a barrel's named re-export and a barrel's default export.
  - `closed-export-alias-load`: `paths`-aliased `import("@entry")` and a
    `paths`-mapped `fs` keep their obligations. `kept` clears despite
    `import("@other")`, `import("./other.js")` and `require("fs")`.
  - `closed-export-declaration-sibling`: these keep their obligations:
    - a namespace of `./bridge.js`, resolved to a hand-written declaration
      whose runtime module re-exports the helper;
    - a namespace of the directory `./bridge2`, whose `main` and `types`
      differ.

  All type-check clean.
- **Coverage:** only the new snapshots move. The widened reach census moves
  no existing fixture.
- **rc.13 corpus**, browser host, release binary, against the ADR 0218
  sweep: 16 `reactive-dispatch-unresolved` obligations removed, none added,
  violations unchanged at 195. The sweep's summed project wall time did not
  rise with every file attested (156.8 s against 159.9 s).
