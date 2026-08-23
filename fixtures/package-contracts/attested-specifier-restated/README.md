# A specifier the walk could not resolve and the program did keeps its note

**The trap this fixture exists for: the specifier must stay `./impl.cjs` and the
file on disk must stay `impl.cts`.** Rename either to `.js`/`.ts` and both sides
resolve it, and the fixture stops testing the branch it exists for.

The opposite half of `asset-import`. There the walk failed on a specifier the
compiler also resolved to nothing, so the note was dropped. Here the walk fails
and the compiler *succeeds*: `RUNTIME_EXTENSIONS` in
packages/cli/scripts/runtime-module-closure.mjs deliberately omits `.cjs`/`.cts`
(CJS contract generation is unsupported), while TypeScript's `bundler` resolution
substitutes `.cjs` → `.cts` and opens the file.

So the note survives — and it survives *restated*, carrying the attested answer
the walk could not produce:

```
index.js: closure could not be fully enumerated: ./impl.cjs names no runtime
module inside the package (looked for impl.cjs, impl.cts); the analyzing program
resolved it to impl.cts (relative, .cts), so the analysis read a module this walk
did not seed
```

That is strictly more than "names no runtime module inside the package" could
say: it names the file, how the resolver classified the resolution, and which
extension it landed on, all read off the producer's `ModuleImportFact` rather
than guessed from a path.

**One cause, one note.** `impl.cts` is also a module the program opened and the
walk never seeded, which is `seed-attestation-discrepancy`'s note. It must not
appear twice: `attestedClosure` records the resolved path of every restated note
and excludes those from the unseeded-module sweep. `expected-generation.json`
pins exactly one note here, which is what catches a regression of that
suppression.

**And the record still names it.** `impl.cts` is in `modules` because the
analysis read it. A reconciliation that kept the note but left the file out of
the hash set would be back to the original defect — a record claiming which bytes
the summaries came from while the file that produced them sits outside it.
