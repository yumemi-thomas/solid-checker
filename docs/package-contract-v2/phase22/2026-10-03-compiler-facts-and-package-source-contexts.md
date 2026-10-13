# Compiler facts and package source contexts

## Outcome

Solid compiler execution facts can supply a useful part of package feedback:
the execution context created by JSX lowering. This experiment collects those
facts from unchanged application and published package sources, then joins
exact component invocation spans to real TypeScript declarations. It uses the
existing compiler adapter; no compiler lowering or production analyzer changes.

The experiment makes 96 DOM/SSR runs over 48 source paths, representing 37
distinct source contents. Twenty-two paths belong to published Kobalte source.
All runs produce normalized facts and preserve trace-on versus trace-off output.
The normalized model contains 1,069 source operations and 1,090 positive
generated operations. These are execution-context counts, not defect detections.

The application's RC.9 compiler produces identical JavaScript in 53 runs and
different bytes in 43. The declaration-binding experiment refuses those 43
contexts. It binds 84 component invocations in the matching subset, including
28 Kobalte declaration bindings across 18 distinct source uses, and attaches
101 exact expression contexts. Runtime provider, callback invocation and causal
responsibility remain open. No new warning or violation is emitted.

## Experiment inputs and boundary

The source population comes from the exact maps validated in the
[existing hydration trial](2026-10-03-existing-package-hydration-failure-and-source-feedback.md).
It contains 35 JSX/TSX paths from the candidate bundle and 13 from the native
control. Shared application files appear at different copied paths, so path
counts are kept separate from distinct source contents. There are no installs,
seeded source defects or application edits.

`compiler-facts-probe-v1` is a standalone Rust research consumer. It calls
`solid-v2-compiler::analyze_with_materialized_output` and serializes the
normalized `solid-facts::compiler::ExecutionMap`. Compiler producer structs stay
inside the existing dialect adapter. That adapter compiles with tracing on and
off, compares output and optional map bytes, and validates the normalized model.
The probe is built offline and its independent lockfile retains the repository's
exact compiler distribution revision `9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4`.

`compiler-source-feedback-v1.mjs` supplies explicit hydratable DOM and SSR
configurations and compares each materialized output with the retained
`@solidjs/compiler@2.0.0-rc.9`. Both direct compiler calls disable source maps
for this comparison. The installed loader, platform binary, probe image,
source files and compiler identity inputs are recorded. Universal and dynamic
requests are refused by the existing fact adapter.

This is a source compilation experiment, not a replay of the full Vite pipeline.
Matching one file's output does not equate compiler identities or establish
full build/runtime fact authority. The existing browser run remains evidence of
its observed failure; these facts do not establish its cause.

## What the compiler contributes

The normalized facts distinguish the following dispositions automatically:

| Disposition | DOM operations | SSR operations |
| --- | ---: | ---: |
| Reactive rerun | 99 | 0 |
| Event triggered | 38 | 0 |
| Eager once | 144 | 0 |
| Component property getter | 159 | 159 |
| Deferred | 63 | 33 |
| Ref application | 26 | 14 |
| Discarded | 5 | 55 |
| SSR evaluation | 0 | 146 |
| SSR render callback | 0 | 128 |

For example, the candidate's `Primitive.Root` has compiler property-getter
contexts for `props.open` and `props.onOpenChange`, with inherited tracking and
an owner ambient at generated invocation. `Primitive.Trigger`'s `props.trigger`
child has a deferred context. Those statements describe evaluation of the prop
or child expression. They do not prove when Kobalte invokes a callback value
obtained from that property, or the owner/tracking of the callback body.

The facts can therefore guide context-sensitive read/write and ownership checks,
preserve discarded regions and avoid treating every JSX expression as immediate
tracked execution. They cannot alone classify arbitrary package helpers,
Promise adoption, timers, callback retention, cleanup, result relevance or user
intent. Those questions still need Type Facts, runtime observations, explicit
expectations or exact package behavior evidence.

Twenty compiler runs have no source or generated operations: a `.jsx` extension
does not imply compiler-controlled JSX behavior. Source-operation completeness
is the normalized producer census claim, not a measured coverage rate for all
source behavior. Generated-operation completeness remains false in every run;
an absent generated operation never proves that behavior is absent.

## Exact package declaration joins

`compiler-package-bindings-v2.mjs` matches a generated component invocation's
whole source span to exactly one JSX element. It then resolves that element's
tag through the real configured TypeScript program, follows symbol aliases and
requires a unique declaration. Direct property/child expressions join to source
operations only when their UTF-8 spans match exactly. There is no smallest-span
symbol selection, guessed member dispatch or package-name trust.

The 84 bindings include 28 Kobalte declaration bindings to 18 distinct source
uses in the existing alpha wrappers. All 101 direct expression contexts match
one compiler source operation. This measured subset contains component getters,
deferred expressions, eager values and SSR evaluations. It does not add a
behavioral contract for Kobalte or treat its declaration as its runtime provider.

The remaining boundaries are explicit:

- 43 per-file/mode outputs differ from the installed compiler;
- 32 matching per-file/mode runs are outside the configured TypeScript source
  programs, including published package implementation sources;
- one component declaration is non-unique and remains unresolved;
- no whole-build compiler identity, runtime dispatch, callback scheduling,
  causal diagnosis or repair is established.

Output drift includes changes to generated getter syntax and renderer helpers.
In particular, the hidden-select DOM output differs, so its old-producer facts
cannot close the source attribution gap for the preceding hydration failure.
No whitespace normalization or semantic-equivalence guess bypasses that boundary.
A future fact-producer update must follow the repository's semantic-only fork
procedure and preserve ordinary compiler output.

The first binding runner holds and diagnoses both projects and terminates with
exit code 137 before saving a result. Its cause is unobserved and it is not
counted as success. V2 handles one project per child with a 2 GiB V8 heap cap and
a 60-second timeout. It authenticates and reuses the existing two clean typing
CLI runs rather than recomputing project diagnostics. Both children complete.
This change is not a total-memory bound or performance benchmark.

## Verification and next direction

`compiler-source-audit-v1.mjs` independently checks source/output hashes,
producer identity, operation spans/IDs/references, counts, package ownership,
declaration bytes, exact context joins and the output-drift refusal partition.
TypeScript symbol acquisition belongs to the binding runner; the independent
artifact audit does not claim a second semantic resolver. Four modified reports
with fabricated callback/runtime claims, wrong package ownership or a drifting
output binding are refused.

The compiler identity gate passes. The standalone probe passes formatting and
Clippy. Fast workspace verification, schema/manifest validation, whitespace,
research-module syntax checks and preservation of the prior handoff seal are
the handoff checks. The existing 823 prototype tests remain unchanged and are
not rerun or counted as new compiler evidence. Full production verification,
coverage, ownership, contract and release gates remain deferred. No production
code, compiler fork, snapshot, contract, schema or manifest changes.

Ignored evidence is under `rust/target/compiler-source-feedback-v1/`,
`compiler-package-bindings-v2.json` and its two worker/process reports,
`compiler-source-audit-v2.json`, `compiler-source-negative-audit-v1.json`, and
the probe build, Clippy, identity and fast-verification logs. The unsuccessful
V1 binding process has a separate termination record.

The useful system direction is to combine these compiler contexts with exact
Type Facts bindings and runtime/test evidence. Compiler facts supply execution
semantics created by lowering; runtime observations supply executed package
behavior; assertions supply expectations. This division supports scalable
feedback while keeping missing behavior open. It does not establish feedback
for all packages or all rules.
