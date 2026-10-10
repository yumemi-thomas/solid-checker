# Native automatic development feedback

Date: 2026-10-03

## Outcome

`feedback run` now produces automatic development guidance without supplied
assertions, expected values or comparison code. Native source models connect
executed untracked reads to candidate result paths. The guidance is conditional;
it does not certify a stale result, Promise settlement or package behavior.

Fresh execution of 41 previously authored consumers produced:

| Population | Result |
| --- | --- |
| 21 intended misuse targets | 18 warning cases, one informational case, two open |
| 20 controls | No automatic warnings or informational guidance |
| Two fresh callback-lineage targets | Both warned |
| Four fresh callback-lineage controls | All quiet |
| Four runtime/typing boundary checks | All passed |

The informational case observes the map package querying the native observer.
A null observer does not prove a reactive read or skipped subscription. The two
open targets are serial and concurrent object-identity results. Their controls
share the same code and differ in intended identity, so automatic selection
cannot resolve them without application intent.

The independent scorer uses expectations to measure behavior; the detector
receives none. The 41 consumers had zero TypeScript errors, runtime exceptions
or proven native findings. Instrumentation preserved their measured results.
There were no evidence retention losses. This is a bounded authored corpus,
not a universal accuracy or zero-noise claim. Exact rows and implementation
digests are retained in the
[evidence](../../../benchmarks/reviewed-package-models/development/automatic-feedback-evidence.json).

## Implementation and semantic ownership

`solid-reactive-ir/src/development_feedback.rs` owns optional native models:
exact primitive identities, source SHA-256, function and operation spans,
derived origins, and candidate result/allocation relevance. Return expressions,
exact const references and distinct numeric branch results can establish a
candidate. Competing paths remain open. This does not establish adoption or
consumption of an asynchronous result.

The CLI source hook performs syntax transformation using those native models.
It preserves callable identity, names, receivers and directive prologues. Exact
body-span joins handle methods whose parser function spans differ. Runtime
tokens carry modeled origins through deferred callbacks and await continuations.
The selector requires the observed source site to belong to the final modeled
operation and requires a relevant allocation path for a late callback.

Two fresh controls exposed false warnings during development: a discarded
reaction result and a discarded method read. Allocation relevance and the exact
method-body join fixed them. The final six-case lineage run is quiet on all four
controls, including discarded inline and named callbacks.

The runtime collector records bounded native read and observer-query evidence.
Native `untrack` intent suppresses guidance. Source revisions, unknown models,
missing spans, incomplete lineage and retention loss stay open. Observer-query
guidance is informational and is suppressed when a read warning already covers
the same origin. Ordinary analysis omits these optional models.

SC5001 also preserves an invocation-context hole: storing a callback inside a
component does not prove it executes during the strict-read window. The focused
fixture now has four proven pending reads and three uncertifiable reads, plus a
retained-local negative control. The ordinary button boundary produces a native
gap; the explicit strict-component boundary still records
`PENDING_ASYNC_UNTRACKED_READ` as an error. Published queue typing errors suppress
extra development guidance. Failed explicit assertions remain errors even
without comparison code.

## Versions and reproduction

The final browser corpus used the exact installed packages
`@solid-primitives/queue@1.0.0-next.3`,
`@solid-primitives/map@1.0.0-next.2`, and
`@solid-primitives/controlled-signal@1.0.0-next.3`.
The reviewed runtime remains Solid RC.9; its native hook refuses different bytes.
Analysis used the isolated RC.13 compiler candidate, distribution commit
`3ad4bbec37ae30f325a803cdb4271a71c86a2a2d`, based on upstream
`5efaf260becb32293f2bcb4d32f8be72be6de674`. Production compiler pins are unchanged.
The fork remains semantic-facts-only.

The updated complete local overlay and restoration instructions live in
[the RC.13 reproduction directory](../../../benchmarks/compiler-facts/rc13/README.md).
The historical overlay and verification evidence remain intact. CLI usage and
exact limitations are in [development feedback](../../development-feedback.md).

Benchmark entry points under `benchmarks/reviewed-package-models/development/`:

- `live-feedback-automatic.mjs`: replay the 41 consumers without assertions or comparisons.
- `automatic-feedback-lineage.mjs`: six fresh callback and method cases.
- `automatic-feedback-boundaries.mjs`: pending, strict-runtime, typing and assertion boundaries.
- `audit-automatic-feedback.mjs`: independently validate pins, source digests, observations and scores.

## Validation

The full `make verify` passed in 841.21 seconds, including coverage, ownership,
contract conformance, the typing oracle and the feature matrix. That run preceded
the final allocation, observer-query and method precision refinements.

Final follow-up on those refinements passed:

- 301 IR library tests and the armed native source-model process test.
- 330 CLI tests plus declaration type checking.
- Fresh coverage: 142 fixtures and 746 findings.
- Real published RC.9 typing checks for the new pending and feedback cases: zero errors.
- Rust formatting, diff checks, schema and dialect validation, and workspace/all-target Clippy with certification pins.
- The independent 41-consumer audit, six fresh lineage cases and four boundary checks.

Only the callback-timing finding snapshot changed for this semantic slice. The
void-JSX fixture/snapshot change was already present. No contracts, public
contract schema, producer protocol or production compiler pins changed.

## Remaining limits

Feedback requires executing the relevant application path. Coverage is always
incomplete; a quiet run does not establish correct package use. Result relevance
is a candidate model rather than a general interprocedural result-flow proof.
Object identity expectations, ambiguous return paths, unknown package behavior,
unmodeled source operations and incomplete evidence remain open. The native
contract proof boundary still fails closed for unknown packages. This command
does not yet provide editor/watch integration or automatic source fixes.
