# Automatic source models and runtime feedback

The next experiment removes per-export authoring for a narrow set of behaviors.
A deterministic source pass scans **97 retained packages in 3.26 seconds** and
extracts candidate premises for **151 exports across 55 packages**. Feeding
those premises to the warning adapter detects **26/35 selected browser misuse
observations across 13 packages**, with no warnings on **105 negative or
unsupported control observations**. All consumer and analysis copies pass the
real published TypeScript declarations.

This is useful evidence for automatic model authoring, not certification or
complete package support. The source pass has a deliberately small language;
its assumptions and finite tests do not prove every extracted candidate.
The production analyzer, public schema and accepted tier remain unchanged.

## What is automated

`benchmarks/reviewed-package-models/source-extractor.mjs` follows released ESM
runtime bodies, local helpers and exact imported symbols. It specializes known
host branches, tracks direct signal/memo return origins, and records mandatory
eager cleanup/effect registrations. It does not walk deferred callbacks in the
factory's execution phase. An owner guard or an owned root removes the ambient
owner premise. Unknown branches must agree before a premise is emitted.

It emits positive premises with source locations and explicit gaps. It never
fills an unobserved behavior domain with an empty claim. Package/dependency
bytes are pinned, including the audited Solid/signals/web rc.9 environment.
No certification transaction, network access, installation or LLM call runs.

The final bounded pass examines 1,312 export/host observations. Of those, 234
have at least one premise: 144 have a return premise and 127 have an ownership
premise, with overlap. There are 149 browser exports with premises. Three
packages refuse because their runtime entry is missing: animation,
controlled-props and virtual. Another 39 packages yield no supported premise.
Neither refusal nor an empty result establishes safety.

The initial pass found more candidates. Controls for mutation, spread options,
object accessors and unknown callback timing narrowed the final set. The
saved-result validator confirms that every premise used in the diagnostic
experiments is unchanged in the final catalog, authenticates its package
inputs, and replays the projected warnings. Unchanged native analyses were not
repeated just to verify that narrower candidate set.

## Does it produce feedback?

| Experiment | Target observations | Model warnings detected | Negative/control observations | Warnings on controls |
| --- | ---: | ---: | ---: | ---: |
| Original eight packages, corrected expectations | 29 | 21 | 99 | 0 |
| Five additional packages | 6 | 5 | 6 | 0 |
| Automatic total | 35 | 26 | 105 | 0 |

The five additional packages are connectivity, active-element, trigger,
pagination and focus. The source extractor contains no branch for their
package names. The pagination page-accessor case is missed because the adapter
supports only tuple member zero; that example reads member one. Its segment
accessor case succeeds. The other four packages' selected cases succeed.

The eight other misses are specific: createTimer's three ownership contexts,
createTimeoutLoop's ownership, createEventListenerMap's loop, createMs's
captured mutable closures, access's conditional invocation, and the write in
createLazyMemo's tracked callback. Generic cleanup/effect branch merging,
callable arity, iteration and callback phase models remain needed. The wrapper
return, mutable consumer binding and computed namespace controls remain
explicitly unsupported. Their silence is not correct-use certification.

The original corpus uses a historical baseline: the original specimens,
compiler options, checker/producer digests and TypeScript/Node versions are
checked before reuse. Four deferred-callback baseline observations and all
12 observations for the additional packages were analyzed afresh. All 140
automatic observations receive fresh modeled analyses and both typing checks.
The existing baseline detects two of the original targets, one violation and
one uncertifiable result, and none of the six additional targets.

These are selected diagnostic tests. The investigator also wrote the extractor
and examined the controls; this is not a blinded generalization study. The
3.26-second extraction wall excludes native consumer analysis and does not
estimate editor latency or full-application performance.

## A correction to the first result

The first reviewed model incorrectly required an owner for createMediaQuery
and createPrefersDark. Their listener helper uses tryOnCleanup: in a browser
development environment it registers cleanup only when an owner exists.
An unowned call can intentionally create a persistent listener. That does not
establish an unconditional missing-owner violation.

Independent samples of the exact published code confirm:

- two unowned media calls add two listeners, remove none and emit no diagnostic;
- owned calls remove their listeners when disposed;
- an eager media accessor read in a component emits STRICT_READ_UNTRACKED;
- unowned scheduled.debounce can be cleared without an ownership diagnostic;
- RAF still emits NO_OWNER_CLEANUP for unowned setup and STRICT_READ_UNTRACKED
  for an eager component read; its tracked twin stays clean;
- lifecycle.createIsMounted still settles to true without an owner diagnostic.

The browser globals must be initialized before dynamically importing media.
An earlier Node sample imported it first, so utils selected its server-mode
initialization and produced misleading ownership diagnostics. That failed
trial is retained separately. Mocked host tests require this environmental
detail; these samples are not real-browser conformance.

The reviewed catalog removes both unconditional media owner premises. All 20
affected static observations are rerun: three selected reads are detected and
17 controls remain without model warnings. The original 30/30 headline had
one incorrectly labeled target, media's module-scope owner case. Its raw
observation is retained as historical evidence, and the corrected target set
has 29. The fixture ledger itself is unchanged, just as with the earlier
createIsMounted correction; those ledger changes remain separate work.

## What real apps demand

The retained import metric contains 38 apps and 1,850 third-party app-use sites.
Only **292 sites (15.8%)** use @solid-primitives. Routers, icons, query libraries
and components dominate the remaining demand. Expanding primitive exports
alone cannot deliver broad application coverage.

The final catalog's package names occur at 255 sites. Only 102 sites have a
matching export name with a browser premise, an upper bound before admission
and caller analysis. **Zero sites match the catalog's full input pins.** The
audit records 1,595 sites in unmodeled packages, 126 without a browser premise,
54 with a package-version mismatch, and 75 with mismatched input bytes. Many
of those input mismatches resolve an older Solid prerelease, such as rc.8.

This is a demand/admission audit of cached observations, not a new application
analysis. The code checks existing project paths and installed input bytes; it
does not substitute rc.9 for the apps' runtimes. The result exposes a design
constraint: a centrally authored catalog for one environment still fails to
serve real installations. Automatic extraction should run against the
consumer's installed bytes and cache those inputs, after the dialect audits
that runtime. It should not require a new package certification transaction
for every dependency combination.

## Design direction supported by the experiment

1. **Extract small models on demand.** Start with the package exports a consumer
   actually uses. Cache positive premises by runtime, package bytes and the
   extractor version. Keep unsupported domains explicit. The 97-package pass
   establishes that this authoring step can be inexpensive for simple code.
2. **Give models a consumer interface.** Pass return, lifetime and callback
   premises into the existing semantic owners. The analysis-copy expansion is
   an experimental adapter; the production interface should preserve original
   source and compiler facts. Source assumptions must stay distinguishable
   from certified external behavior.
3. **Use runtime feedback alongside static feedback.** The published runtime
   already diagnoses these sample misuses through third-party packages.
   Capture those diagnostics during development and exercised UI flows, and
   use them to challenge source models. A clean runtime sample says nothing
   about an unexecuted branch.
4. **Expand by application demand.** Next model families should address callback
   phases, multiple return members, component props, and query/router behavior.
   Review or certify the facts that materially change feedback for real uses.

The current evidence supports a warning mode driven by source models and
runtime observations. It does not support automatically treating every
inferred summary as a proven violation. The remaining unknowns include loops,
classes/member dispatch, dynamic globals, richer returns, wrapper propagation,
arbitrary options, async readiness and unreviewed runtime versions.

## Evidence and verification

Retained outputs under rust/target/:

- reviewed-models-extracted-final-safe.json: final candidate catalog, input
  pins, source references, gaps and extraction timing;
- reviewed-models-automatic-feedback/results.json: 128 automatic observations;
- reviewed-models-automatic-new-packages/results.json: 12 additional observations;
- reviewed-models-media-corrected/results.json: 20 corrected reviewed observations;
- reviewed-models-runtime-followup-browser/results.json: four published-runtime samples;
- reviewed-models-app-demand-safe.json: demand and input-admission audit;
- reviewed-models-final/models-original.json: the exact original catalog,
  reproduced against its recorded digest for historical replay.

The warning adapter labels automatic output source-extracted-assumption and
certification false. Raw native uncertifiable findings are retained. The
extractor/catalog scripts, runtime controls and reproduction commands live in
benchmarks/reviewed-package-models/. The reviewed models.json is regenerated;
no accepted artifact, receipt, public schema or fixture snapshot changes.

Verification: 15 adapter/extractor tests, all 160 follow-up static observations
and their typing checks, saved-result replay with final-premise equivalence,
four independent runtime samples, and the demand audit pass. Source checks use
make verify-fast for formatting and certification-pinned all-target Clippy,
plus schema JSON, dialect manifest and whitespace validation. Full make verify,
coverage, ownership and contract-corpus gates remain deferred because this
slice changes experimental JavaScript and documentation. Fresh full-app
analysis and a browser UI test suite remain unperformed.
