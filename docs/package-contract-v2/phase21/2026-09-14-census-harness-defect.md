# The two-pass census measured the wrong condition (2026-09-14)

Every `reads` premise ranked in this phase was chosen with a two-pass scratch
census: pass 1 certifies a case and scaffolds each candidate withheld as
`no recipe in corpus`, pass 2 certifies against that scratch corpus, and the
candidates that come back `census refused` are unserviceable while those that
come back `veto did not complete … unfinished` are decidable and worth a hand
recipe.

Pass 1 was never the condition it claimed to be, and the classification pass 2
produced is therefore not the one that was read off it.

## What was wrong

The driver passed `""` for pass 1's corpus. That is falsy, so
`--probe-recipe-corpus` was dropped from the argument list entirely rather than
naming an empty directory. Two things follow in the analyzer, and both are by
design:

- `CertificationPlan::recipe_gated_with(None, …)` has no corpus to consult, so
  it withholds **every** proposable closure candidate as `no recipe in corpus`
  without asking the implementation census anything.
- veto synthesis is reached only through `let Some(base) = probes`, so with no
  configured harness it never runs at all.

The withheld set that comes back is complete, clean, and says nothing. One
recorded pass-1 audit carries **1,416** withheld closures, every one of them
`no recipe in corpus`, `solid-js`'s own well-covered claims included. Nothing
in the audit distinguished that from a real corpus's answer.

## Why it changed the conclusions and not just the counts

Over-enumeration in pass 1 is not harmless, because a scaffolded candidate is
no longer eligible for synthesis in pass 2: it has a recipe, so it is not
withheld as `no recipe in corpus`, so `synthesize` never offers it a veto. The
scaffold then throws its `UNFINISHED` guard, and the candidate is counted
**decidable — worth a hand recipe**, when a synthesized veto would have closed
it with no recipe at all.

Re-measured on `@tanstack/store@0.11.1`, plain lane, everything else identical:

| | pass 1 withheld | scaffolds | pass 2 `reads` withheld | refused | decidable |
| --- | ---: | ---: | ---: | ---: | ---: |
| no corpus flag | 186 | 12 | 102 | 56 | 46 |
| empty corpus | **14** | **8** | **9** | **3** | **6** |

The old run reported eleven times as much work as the new one, in both
directions at once.

## What this does and does not invalidate

**Sound.** The pinned corpus row counts — the 1,470 recipe-less `reads` rows
and every figure derived from `benchmarks/ecosystem/report.json` — come from
the ecosystem benchmark, which has always passed `--probe-recipe-corpus`. The
`motion-utils` diagnosis is sound for a different reason: it was settled by a
focused producer fixture, not by this harness.

**Suspect.** Every decidable/refused split produced by the two-pass scratch
census, which is the instrument that ranked the premises:
`2026-09-14-tier-a-pass-2-census.md`'s per-case table,
`2026-09-14-tier-b-remaining-census.md`'s decidability claims, and
`2026-09-14-tier-c-class-census.md`'s "pass 1 emitted 12 modules; pass 2
reported every one unserviceable". None of those have been re-run. They are
not known to be wrong — they are known to have been measured with an
instrument that was.

## The fix, in three parts

1. `emptyCorpusManifest()` is exported from `scripts/probe-recipe-scaffold.mjs`
   and is what `readManifest` returns for an absent manifest, so a harness can
   write a loadable empty corpus from one definition instead of approximating
   one by passing nothing.
2. Both certification audits record `probeCorpus`: the configured corpus, or
   `null`. It qualifies every `no recipe in corpus` record in the same
   document, so the two conditions can never again be read off the same shape.
3. `SOLID_CHECKER_EXPECT_PROBE_CORPUS=1` makes a missing `--probe-recipe-corpus`
   an argument error. This is the twin of `SOLID_CHECKER_EXPECT_PROBE_PINS`
   that `AGENTS.md` already describes, for the same failure: a run that
   silently proves less than its reader believes. A measuring harness sets it;
   ordinary certification is untouched.

An empty corpus is a configuration, not an absence. It arms the probe
machinery, runs the census, and offers synthesis — so a candidate it still
withholds for want of a recipe was withheld on the merits.
