# The walk seeded one module, the program opened two, and the record says so

**The trap this fixture exists for: `index.js` must keep importing more than 150
names in one clause.** Shorten the clause and the walk resolves `./big.js` like
any other specifier, the two sides agree, and the fixture silently becomes a
duplicate of `attested-record-matches-walk`.

This is the residue the attested record exists to expose, in the exact shape that
produced it. `moduleSpecifiers` (packages/cli/scripts/runtime-module-closure.mjs)
scans an `import`/`export` clause for its `from` under a 300-token bound at
depth 0. `index.js` imports 200 names, so the clause is ~400 tokens and the
`from "./big.js"` is never reached — and because the scanner never saw a
specifier, it recorded **no problem either**. The walk simply returned
`[index.js]`, with an empty `notes`, which is a *claim*: these are the bytes the
summaries came from.

They were not. `big.js` is a tsconfig-resolved import from a root file, so the
analyzing program opened it and every summary depends on its bytes. The
attestation names it, the record now names it, and the disagreement between the
seed and the program is its own note:

```
big.js: the analyzing program opened this module and the closure walk did not
seed it, so the analysis read package bytes the walk did not enumerate
```

Nothing before attestation could observe this. That is the whole point of the
backlog entry it closes: "a syntax walk can still disagree with the compiler in
ways neither side reports … because the process that resolved the modules is the
other one."

**Two directions, one rule.** `attestedClosure` reports the mirror case too — a
module the walk seeded that the program never opened. Neither direction is
reconciled away, and neither is inferred from a path shape.

**What is deliberately *not* noted here.** A declaration file the program opened
is excluded from this check. TypeScript preferring an adjacent `.d.ts` over the
`.js` beside it is why the walk seeds runtime files at all (see
`analyzeTarget`'s pinned comment), and the identity split that creates is the
analyzer's own incompleteness finding — `declaration-sibling-reach` pins it.
Reporting it here as a seeding gap would double-report it on nearly every
published package. The note above fires only for a non-declaration module.
