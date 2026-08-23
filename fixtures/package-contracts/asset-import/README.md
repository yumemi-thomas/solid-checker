# An asset import is not a hole in the closure, and the attestation says so

**The trap this fixture exists for: `styles.css` must stay on disk and must stay
imported from `index.js`.** Delete either and the fixture stops testing
anything — the walk has nothing to fail on and the record is trivially clean.

`index.js` writes `import "./styles.css"`. The generator's syntax walk sees a
relative specifier, probes for a runtime module, finds none (`.css` is not a
runtime extension and there is no `styles.d.ts`), and — being fail-closed —
records `./styles.css names no runtime module inside the package (looked for
…)`. Before attestation that note blocked the entrypoint from transferring a
review and refused the document outright at `contract verify`.

It was never a hole. The analyzing program resolved nothing for the specifier
either — the pinned producer answers `resolution: "unresolved"` with an empty
`resolvedPath` — so the analysis read no bytes for it and the record naming
`index.js` alone is complete and correct. **And no runtime resolves it to a
module either**, which is the second half of the reason and the one that does the
work: `styles.css` is not a runtime module, so there is no file a runtime could
load here that the analysis did not read. The note is dropped.

The predicate is that second half, not the first. "The compiler resolved nothing"
alone would also drop an unselected conditional `imports` branch whose targets
are real `.mjs` files that Node loads — see
`fixtures/package-contracts/conditional-imports-side-effect`, which rides this
same reconciliation branch and must keep a `runtimeNotes` entry. What separates
the two is `runtimeTargets` (`createModuleResolver` in
packages/cli/scripts/runtime-module-closure.mjs): the existing runtime modules
inside the package a runtime could still select for the specifier. Here there are
none.

This is the class this fixture stands for, and it is why the fixture is
registered in the corpus: it is measured on real packages. The ecosystem run
before attestation carried `./style.css`, `./styles.css` ×2,
`../../../package.json` and `./Chart.svelte` — five of the thirteen sampled
closure notes — in what the design called the asset-import class. Reproducing it
locally showed the class is narrower than that: the compiler *does* resolve
`./Chart.svelte` (to `dist/svelte/Chart.svelte.d.ts`) and `../../../package.json`,
so both keep a **restated** note, and only the stylesheets disappear. See
docs/precision-backlog.md, which records the two wrong predictions.

What all five share, and what this fixture pins, is that none of them acquires a
`runtimeNotes` entry: none names an existing *runtime module* inside the package,
so there is no file a runtime could load here that the analysis did not read.

**What must not happen here.** Dropping the note may not become dropping notes
in general. `attested-specifier-restated` is the same walk failure with the
opposite attested answer, and it must keep its note. Break the reconciliation so
that a resolved specifier is treated like an unresolved one and this fixture
stays green while that one fails; break it the other way and this one fails.
