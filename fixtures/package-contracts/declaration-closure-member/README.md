# An erased edge is a declarations-axis edge; a value edge into a declaration-only target refuses

**The trap this fixture exists for: neither `src/options.d.ts` nor
`src/phantom.d.ts` may have a `.ts`, `.tsx` or `.js` sibling.** Add one and the
matching case silently stops testing anything — the runtime axis then resolves
the specifier to the implementation and no declaration file is reached on that
axis at all, which is what `declaration-sibling-reach` already pins from the
other side.

Both cases start from the same mechanism. `localModuleTarget`
(`packages/cli/scripts/artifact-resolution.mjs`) substitutes `.d.ts` for a
`.js` specifier **on the runtime axis**, so a specifier whose implementation the
package does not ship resolves to its declaration sibling. What happens next
depends on whether the edge survives compilation, and the two answers are
different:

## `.` — an erased edge, resolved on the declarations axis

`src/index.ts` writes `import type { Options } from "./options.js"`. TypeScript
deletes the statement whole: the emitted JavaScript never mentions
`./options.js`, and nothing about `src/options.d.ts` is a runtime fact. The
closure census records the specifier's `type` modifier (`specifierIsTypeOnly`)
and reaches the target on the **declarations** axis, so the closure entry's role
is `declaration` — visible in the case's `closureSha256`, and the assertion this
case exists for.

That role is load-bearing three times over:

1. `projectFiles` (`packages/cli/scripts/generate-package-contract.mjs`) selects
   runtime-role entries for each batch target's `sourceFiles` list. A
   declaration file with the runtime role landed in that list, and the Type
   Facts producer never reports a declaration file in `Sources`
   (`apps/solid-typefacts/internal/typefacts/tsgo/project.go`), so the emission
   batch refused the case with a message that was false in both halves —
   "names source outside its configured project", of a file the generator had
   written into the batch tsconfig's own `files` list, named because
   `sourceFiles` is sorted and not because it was the refused entrypoint's
   module. That is phase 21's M1.
2. The claim itself. The runtime role asserted that the emitted JavaScript loads
   a module it does not mention.
3. The verifier must agree. `replay_snapshot_closure`
   (`rust/crates/solid-facts-backend/src/contract_certification/module_closure.rs`)
   recomputes every role from archive bytes and compares digests, so it reads
   the same `type` modifier through `solid_facts::ast`. A one-sided fix would
   refuse every package with a type-only import in a runtime module.

`.` certifies, and `./label` — whose closure holds no declaration file at all —
certifies unchanged, so a snapshot where only `.` moved is the evidence that
this rule and nothing else changed the answer.

## `./phantom-consumer` — a value edge, refused

`src/phantom-consumer.ts` writes `import { phantom } from "./phantom.js"` and
calls it. This edge survives compilation: the emitted JavaScript really contains
`import { phantom } from "./phantom.js"`, and `src/phantom.js` does not exist —
`localModuleTarget` tries every runtime sibling first, so reaching
`src/phantom.d.ts` *proves* none is published. Every consumer that imports this
entrypoint fails at load.

That is the 08-31 doctrine's criterion for a refusal rather than a disposition —
a consumer really reaches it and really breaks — so the case refuses with
`local-runtime-target-is-declaration-only`, carrying
`applicability: "unavailable-published-target"`: the same standing as an absent
published target, because the module the entrypoint imports is absent. The
declaration file remains a program input; nothing here is deleted from the
census.

Before this fixture, the same shape was **masked**: the emission batch refused
it with M1's false message, so it looked like a scoping bug rather than a
missing module. Repairing only the message would have certified `createWidget`
against an import that cannot resolve.

## The four boundaries

- **Erased edge vs value edge into the same kind of target.** `.` and
  `./phantom-consumer` differ in exactly one token, `type`, and get opposite
  answers. This is the fixture's load-bearing pair.
- **A source that does not exist is not skipped.** In the Rust batch lookup the
  path is canonicalized before the fact-source lookup, so a missing published
  target fails there — and now names the target that asked for it instead of
  surfacing a bare `os error 2`.
- **A target's own entry file is never skipped.** A declaration file *there*
  means the target has no fact source, and it refuses with that reason.
  `non-emitting-module-target-control`'s `./evaluated-default` and
  `./implemented` are exactly that case — two `.d.ts` entrypoints whose
  non-emitting premise fails, so they reach emission — and they must stay
  refused. They carried M1's false message until this fix.
- **A non-declaration source absent from the producer's report still refuses.**
  No package can construct one: `projectFiles` is a subset of the batch
  tsconfig's `files` and the producer reports every non-declaration program
  file.

The last three are pinned by unit tests in
`rust/crates/solid-facts-backend/src/main.rs`
(`contract_emission_fact_program_tests`), because none of them is constructible
from a package. With the role rule in place the Rust tolerance for a
declaration closure member is **defense in depth**: the generator no longer puts
one in a target's `sourceFiles` at all. It stays because the emitter must not
assume the generator's invariant.

## Why the batch path was the only one affected by M1

`analyzeArtifact`, the singleton fallback, writes the same tsconfig but passes
no per-target source list: it takes `configured_sources()` whole, so a
declaration file was never looked up and never refused. The batch path is the
primary path for every artifact case, so the defect was reachable for every
package while the behavior the two paths implement was supposed to be
identical.
