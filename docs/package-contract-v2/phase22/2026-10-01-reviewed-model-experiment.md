# Reviewed behavior models produce useful warnings

**Follow-up correction:** the original 30/30 count includes an incorrectly
labeled media ownership case. Browser runtime samples show conditional cleanup,
so the current model removes that unconditional owner premise. The remaining
target set has 29 cases. All 20 affected observations are rerun and pass their
corrected expectations. Automatic extraction and application-demand results
are in the [follow-up report](2026-10-01-source-model-followup.md). The counts
below describe the original selected expectations and are retained as history;
they do not establish that every original model premise was correct.

The experimental adapter detects **30/30 selected browser misuse cases across
eight packages**, compared with **two matching baseline diagnostics**: one
proven violation and one uncertifiable result. Across **98 negative
observations**, including correct-use twins, inert server branches, shadowing,
rebinding and deferred callbacks, it produces **zero model warnings**.

These are warnings based on reviewed source assumptions. They are not newly
certified violations, complete package contracts, or evidence that the checker
now covers all Solid 2 applications. The production analyzer and accepted tier
are unchanged. The result supports investing in a model consumption interface
and measuring real applications before expanding certification further.

## Measured scope

The prototype is [`benchmarks/reviewed-package-models/`](../../../benchmarks/reviewed-package-models/README.md).
It reviews the exact published runtime implementations, binds model inputs to
their package/dependency bytes, resolves consumer imports with TypeScript, and
expands the stated behaviors into core operations in an analysis-only copy.
The existing analyzer supplies ownership and compiler-aware execution context.
Warnings project back to the original caller with an explicit assumed-model
basis. No receipt is issued and no model enters `AcceptedContract`.

| Package | Version |
| --- | --- |
| utils | 7.0.0-next.4 |
| raf | 4.0.0-next.2 |
| event-listener | 3.0.0-next.5 |
| timer | 1.4.5-next.1 |
| media | 4.0.0-next.2 |
| date | 3.0.0-next.3 |
| lifecycle | 1.0.0-next.2 |
| memo | 2.0.0-next.2 |

All names are under `@solid-primitives`. Solid, signals and web are pinned to
2.0.0-rc.9. Node is 24.21.0 and TypeScript is 5.9.3. Strict consumer checking
uses real published declarations and renderer-owned JSX; `skipLibCheck`
excludes published declaration-file defects without skipping consumer errors.
Both original and analysis copies pass their typing checks in all observations.

The source-reviewed catalog describes **19 exports** with reusable behavior
families: lifetime requirements, direct/tuple accessor returns, immediate
argument invocation, and tracked callbacks. It is 64,684 bytes including the
released source references and dependency pins. The prototype contains no
package-specific Rust implementation. The memo extension did require adding
the general tracked-callback behavior; this was not a blinded held-out study.

The broad run has 37 cases and 124 host/twin observations. A subsequent focused
run adds two deferred-callback controls and four observations. Final totals
are **39 cases, 128 observations, and 256 native analyses** (baseline plus
modeled). The final warning adapter replays and validates all saved results.
The baseline matching diagnostics are the lazy-memo read, reported
uncertifiable, and the tracked-callback write, reported as SC2001 violation.

| Group | Targeted misuse observations | Detected | Negative observations | Model warnings on negatives |
| --- | ---: | ---: | ---: | ---: |
| utils / raf / event-listener / timer, including added controls | 18 | 18 | 58 | 0 |
| media / date / lifecycle / memo | 12 | 12 | 40 | 0 |
| Total | 30 | 30 | 98 | 0 |

The counts are selected diagnostic tests, not unique exports, complete packages,
or an application import metric. Missing contract findings remain visible in
the raw native observations. A clean model warning list is not certification.

## What the experiment taught

The first separate-module model produced an ownership finding inside the model
helper and missed the returned tuple accessor. Expanding modeled operations at
the original call site recovered both useful feedback and caller attribution.
That is a prototype implementation choice; a production interface should hand
the same premises to semantic analysis without rewriting user source.

The initial broad attempt exposed a false positive in a lazy memo's correct
twin. Its callback needs an explicit tracked compute premise. Another control
showed that a native memo's refresh brand can make the synthesized return type
stricter than the package's declared accessor. The final expansion preserves
the resolved published return type, including generic instantiation. Mutable
return bindings remain unsupported rather than inheriting a stale accessor
identity.

Deferred RAF/timer callbacks exposed a separate reporting defect: a lifetime
premise was being extended to reads inside copied callback arguments. The
final adapter projects only generated operations anchored at the modeled call,
or reads semantically rooted in its modeled return binding. An unmodeled
callback body gains no execution premise. Focused controls and a unit test
pin this restriction; final replay preserves all 124 broad observations.

Source review also corrected one experimental expectation. The existing
`lifecycle-createIsMounted-module-scope` ledger row claims a missing owner.
Its published body uses `onSettled` without returning cleanup. In an independent
runtime sample the unowned call settles to `true` and emits no diagnostic.
The model supplies no owner requirement and this row is a negative control.
The tracked ledger is unchanged; its correction remains separate work. Passing
that ledger's original owner expectation is not counted as a recovered misuse.

Independent runtime controls execute published RAF and lifecycle code with
browser/development resolution in Node. RAF reproduces NO_OWNER_CLEANUP for
unowned setup and STRICT_READ_UNTRACKED for a component-body accessor read;
its owned tracked twin is clean. RAF cancellation is mocked. These are finite
samples, not real-browser conformance or behavioral proof authority.

## Cost and evidence

In the broad sequential run, median native analysis is **938.75 ms baseline**
and **937.82 ms modeled**. Median preparation, including the first consumer
typing check, input checks and expansion, is **122.32 ms**. The second typing
check is additional harness work. No certification transaction runs. These are
small cold-project measurements, not editor latency, a statistical speedup,
or an estimate for full applications. Model authoring effort has not been
measured independently.

Retained evidence in the checkout:

- `rust/target/reviewed-models-final/results.json`: 124 observations;
- `rust/target/reviewed-models-deferred-final/results.json`: four controls;
- `rust/target/reviewed-models-runtime/results.json`: independent runtime samples;
- earlier attempts remain in sibling `reviewed-models-*` directories, including
  failed callback and typing trials. They are not included in final totals.

Each static run records checker, producer and model digests, Node/TypeScript
versions, both native outputs, source copies, span mappings, assumptions and
unsupported cases. `check-results.mjs` validates the saved native observations
against the final warning projection without rerunning unchanged analyses.
The README gives offline reproduction commands and refuses missing artifacts.

## Remaining limitations and verification

Supported return uses need immutable direct bindings; tuple support covers
only the first accessor. Mutable bindings, wrapper returns, arbitrary placement,
computed namespace dispatch, callable/object members and unmodeled hosts remain
unsupported. `access` needs an unescaped native zero-arity accessor: positive
arity, a changed `length` getter or an escape remains unknown. Tracked callbacks
need one inline function argument. Async readiness, stores, options-dependent
behavior, callback cardinality and arbitrary disposal protocols are unmodeled.
The adapter does not infer safety or completeness from an unknown answer.

The warning premises remain fallible source reviews. Exact symbol and byte
checks prevent silent substitution but do not certify those premises. The
experiment does not establish results across full applications or future
versions. It also does not demonstrate automatic model generation: the models
were authored from source during this investigation.

Verification passes: all 128 static observations and both typing checks per
observation, four focused adapter tests, saved-result replay, both independent
runtime controls, Rust formatting, workspace/all-target Clippy with
certification pins through `make verify-fast`, schema JSON validation, dialect
manifest validation and whitespace checks. Full `make verify`, coverage,
ownership, contract corpus, tier regeneration and app sweeps were deferred:
this changes experimental scripts and documentation, with no production Rust,
CLI, fixture expectation, public schema or accepted artifact changes.
