# 0022 — Continue declaration selection after a branch has no declaration file

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

The Node-conditioned Kobalte graph stops at seroval 1.5.6. Its exports object
places import before types. The import target is an .mjs file with no .d.mts
sibling; a later types branch names the published dist/types/index.d.ts.
The repository's pinned TypeScript NodeNext resolver selects that .d.ts from
the retained installation. The checker instead selects the runtime branch and
only then checks for a declaration, refusing before considering types.

Move declaration-candidate selection into conditional target traversal on the
declarations axis, in both the acquisition adapter and native snapshot replay.
When a selected string has no declaration candidate, continue to the next
active conditional branch or array alternative. Keep runtime resolution
unchanged: an absent runtime target still refuses and cannot fall through to
types. Null and invalid-target refusals retain their existing conservative
rules; this is not a claim of complete TypeScript resolver parity.

Bind only the successful branch's exact trace, file and digest. Do not select
types by name alone, prefer a later types branch over an existing format-correct
declaration sibling, or cross .mjs/.cjs declaration formats. Missing every
declaration candidate remains a refusal. No package bytes, compiler evidence,
probe policy or sandbox fields change.

Pin the late-types package, an earlier valid .d.mts sibling, and a missing
runtime control. Compare the fixture's resolution with the pinned TypeScript
resolver. The real package already exports declarations; this blocker is a
checker resolution limitation, not a requirement to change the package.
