# Installed source models and live Solid diagnostics

The further experiment supports two complementary feedback paths: derive small
static models from installed package code, and collect Solid's semantic
diagnostics while the app executes. Package certification is not required for
either experimental path. Static outputs remain assumption-based warnings;
runtime outputs describe observed execution. Neither becomes an accepted
contract or certifies a clean application.

The focused static result is **11/11 target observations**, with **no warnings
on 30 negative or unsupported controls**. Earlier tuple regressions add five
successful targets and 13 clean controls. Seven runtime samples execute real
published package code. The app audit also gives a material limit: this small
extractor still supplies no usable premises for the admitted real apps' used
router, meta and component exports.

## What changed

The experiment lives in `benchmarks/reviewed-package-models/`:

- `demand-models.mjs` derives models from requested exports and installed
  dependency bytes. It follows Solid's own dependency path to resolve signals
  and web, so a pnpm app need not expose transitive dependencies at its root.
- `source-extractor.mjs` handles `typeof` branches and all mandatory accessor
  positions in returned tuples. Concrete array spreads and bounded slices
  retain positions; opaque spreads refuse. Standard Array intrinsics remain
  an explicit assumption.
- Call specialization accepts literals, closed plain objects and inline
  function identity. A numeric timer delay selects cleanup registration; an
  inline function selects effect registration. Unknown identifiers and options
  never select a branch based only on their TypeScript type.
- `lower.mjs` summarizes exact, stable local function symbols with zero
  parameters and one synchronous return. It supports a chain of such wrappers,
  bounded to eight, and models tuple accessor bindings separately. Surrogates
  can project only the finding kinds their premises support: an accessor model
  cannot inherit a memo surrogate's ownership restrictions.
- `runtime-feedback.mjs` collects selected semantic codes from
  `OBSERVE.diagnostics`, with consumer stack locations and dependency frames
  when available. It has no per-package behavior model.

All changes are experimental JavaScript and documentation. The production
Rust analyzer, CLI, stable schema, accepted contracts and fixture snapshots
are unchanged. The earlier media model correction remains part of the prior
experiment, not a new accepted artifact.

## Static results and why the adapter matters

| Focused slice | Target observations | Detected | Negative/control observations | Warnings on controls |
| --- | ---: | ---: | ---: | ---: |
| Numeric timer ownership contexts | 4 | 4 | 12 | 0 |
| Function delay timers | 2 | 2 | 6 | 0 |
| Local media wrapper returns | 2 | 2 | 6 | 0 |
| Pagination tuple members and wrapper | 3 | 3 | 3 | 0 |
| Unknown delay/default tuple controls | 0 | — | 3 | 0 |
| Further experiment | 11 | 11 | 30 | 0 |
| Earlier RAF/date/reducer tuple regressions | 5 | 5 | 13 | 0 |

The existing baseline supplies no matching finding, including uncertifiable
findings, for these selected targets. Both original and generated consumers
pass strict checking against the real published declarations. The 59 static
observations include browser/server twins where a reviewed inert-server
control exists. Model generation itself never accepts the reviewed premise.

Two failed trials are useful evidence about the current consumer interface.
Replacing a package call inside a wrapper did not propagate its returned
reactivity to callers. Creating an array of native memos did not propagate
reactivity through destructuring either. The explicit local wrapper summary
and separate accessor declarations recover those cases. A production model
interface needs to carry these facts directly into the existing semantic
owners, rather than depending on analysis-copy shapes.

The final 41-observation run reuses 35 native modeled outputs only after
matching generated source, compiler options, checker/producer digests and
TypeScript/Node versions. Six changed pagination observations get fresh native
analysis. The earlier trial analyzed all 41 modeled observations; baselines
were initially collected afresh or reused from individually validated saved
observations. All 18 tuple regressions get fresh modeled analysis. Saved raw
outputs retain their original analyzed paths, and projection handles those
paths explicitly. Timings of reused outputs are historical, not new run time.

The revised generic scan examines 97 retained packages in **3.24 seconds**:
151 exports across 56 packages have a positive candidate premise. There are
1,312 export/host observations, 233 with a premise, 144 with a return premise
and 126 with an ownership premise. Three missing runtime entries refuse.
This wall time excludes native consumer analysis, editor work and browser
execution. Extraction has no per-export handwritten models or LLM calls.

These are selected tests authored by the investigator, not a blinded estimate
of precision over arbitrary packages. One previously extracted async ownership
candidate now refuses because its opaque array spread blocks the bounded
interpreter. Candidate counts are not accepted contract counts.

## Live feedback requires no package model

The observer subscribes once to the app's Solid diagnostic channel. It captures
the call stack synchronously, reports a consumer location when that frame is
present, and deduplicates by code/location. Its semantic code allowlist excludes
type/shape diagnostics such as MISSING_EFFECT_FN. It does not assert package
behavior from export names or manufacture a package attribution when a returned
accessor no longer has its factory on the stack.

Seven samples pass published typing and execute exact rc.9 package bytes under
browser/development resolution in Node:

| Real package execution | Runtime observations |
| --- | --- |
| Unowned timer, numeric delay | NO_OWNER_CLEANUP at consumer setup |
| Unowned timer, function delay | NO_OWNER_EFFECT at consumer setup |
| Owned timer disposed | None |
| Pagination through wrapper, eager page read | Two STRICT_READ_UNTRACKED locations |
| Pagination through wrapper, tracked page read | One STRICT_READ_UNTRACKED location |
| Lazy memo callback writes a signal | REACTIVE_WRITE_IN_OWNED_SCOPE at consumer callback |
| Lazy memo callback only reads | None |

Pagination's extra diagnostic comes from the package's own eager
`opts().initialPage` read during setup. The dependency frame identifies the
operation and the consumer frame identifies the wrapper that entered it.
The correctly tracked caller still sees that internal diagnostic. Its later
untracked `page()` read adds a second location; that stack need not contain a
pagination frame. This is observed runtime behavior in the published artifact,
not certification, a fixed upstream issue, or a mislabeled caller defect.

The memo callback write was missed by automatic static extraction because the
extractor lacks the callback-phase premise. The live observer catches it
without authoring that premise. This is the strongest new reason to use the
runtime channel alongside source models.

The observer needs installation before package setup executes and the same
runtime channel instance as the app. It currently handles V8 file/URL stacks;
bundle source maps, a browser overlay, asynchronous attribution and dev-server
integration remain unimplemented. No browser UI flow was exercised. Silent
unexecuted paths provide no evidence of safety. The observer adds development
stack-capture cost; that cost has not been benchmarked.

## Installed models do not yet solve broad app coverage

The cached metric contains 1,850 third-party use sites in 38 projects from 36
apps with such sites; the full corpus has 38 apps. Following actual dependency
paths admits 17 package/project models across eight apps. A root-only lookup
had incorrectly refused most pnpm installs because signals was transitive.

The final audit takes 1.92 seconds and classifies the historical use sites:

| Result | Sites |
| --- | ---: |
| Unsupported package subpath | 642 |
| Installed inputs refused, mostly older runtimes | 860 |
| Admitted inputs, no supported source premise | 348 |
| Installed source supplies a premise | 0 |

The admitted packages include router, meta, TanStack and Corvu packages. Their
used exports do not fit this interpreter's simple accessor/registration
language. This audit performs no fresh native app analysis and diagnoses no
app defect. It preserves the rc.9 audit boundary and installs no dependencies.
On-demand extraction fixes catalog distribution; it does not replace the
semantic work for components, callback phases, stores, contexts and subpaths.

## Recommended next implementation

Make live semantic diagnostics the first development feedback path. Attach
them to original files through dev-server source maps, show both the consumer
entry and the dependency operation when available, and collect them in exercised
UI tests. This can observe misuse across packages that reach the same Solid
runtime without writing a contract for every package.

Keep source-derived warnings as the static complement, cached by installed
bytes, runtime, host, extractor and relevant call argument profile. Grow its
small behavior vocabulary from actual unmet application uses. Use targeted
review or certification for facts whose authority materially changes a
diagnostic. A clean runtime run or an empty model must remain explicitly
unmeasured rather than being used to certify arbitrary external behavior.

## Evidence and verification

Final static evidence is under `rust/target/reviewed-models-further-feedback-split/`
and `rust/target/reviewed-models-further-tuple-regressions/`. Generic extraction
is `rust/target/reviewed-models-further-catalog-final.json`; installed app demand
is `rust/target/reviewed-models-installed-app-demand-final.json`. Runtime evidence
uses the three successful timer observations in
`rust/target/reviewed-models-live-feedback/results.json` and all four observations
in `rust/target/reviewed-models-live-feedback-final/results.json`. Failed trials
remain alongside them; none are presented as successful final assertions.

Twenty-three focused tests pass. All 59 static observations pass both real
typing checks; 219 saved static observations pass warning-projection replay.
The seven runtime samples pass saved executable/input/location validation.
`make verify-fast` passes the producer stamp, Rust format and certification-pinned
workspace Clippy checks. Schema, dialect manifest, JavaScript syntax and
tracked/untracked whitespace checks pass. Full `make verify`,
fixture coverage, ownership, contract corpus and native app sweeps are deferred:
this slice changes only the isolated experiment and documentation. Persistent
model caching, richer wrappers, mutable/object members, loops, callback phase
models, subpaths, older audited runtimes and browser integration remain open.
