# One package-contract claim, imported twice

Pins the collapse of *repeated* package-contract claim findings.

Both files import `runUnknown` from `partial-package`, whose catalog entry is
`obsolete-policy1` — so each import raises the same claim-gate defect. The
message names the package, the export and the reason, and nothing about the
site, so the second occurrence carries no information a reader could act on
separately. That is the acceptance gate's per-package argument one level down,
and it is why these collapse to one finding per `(package, export, claims)`.

**The claim: one finding, not two.** The second site is not dropped — it travels
in `relatedLocations`, anchored at `App.ts` with `Other.ts` beside it. This
snapshot format records neither the context nor the related locations, so what
it pins is the count.

**The negative control is the sibling.**
`package-unknown-callback-consumer` raises the same gate at *one* import span
for *two* exports, and still reports two findings, because different exports are
different findings. A collapse that grouped by package alone would merge those
and say the package has no contract when it does.

**Why this pins the branch and not the wording.** `unknown-contract-claims:`
takes the identical path — the grouping key is `(package, export,
analysisContext)` for every claim gate — but reaching it needs a policy-2
accepted contract with an open domain, and those cannot be committed: the
receipt binds absolute paths, so an authorized tree is bound to where it sits
(see `solid-contract-authorize`). Measured there instead: a five-file project
importing four `@kobalte/utils` exports under a bundled contract reported 35
findings that were 7 distinct sentences, each repeated five times with an empty
`relatedLocations`.
