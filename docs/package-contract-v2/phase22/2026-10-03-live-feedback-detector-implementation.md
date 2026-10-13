# Live collector and assertion-assisted detector implementation

## Result

The reusable CLI now executes an application, collects native read observations
and runtime exceptions, evaluates supplied assertions, and runs the
experiment's assertion-assisted selection boundary against a supplied
comparison application. This is an implemented detector path, beyond the
earlier capture-admission interface.

Fresh browser executions through the real command reproduced **21/21 failing
target assertions with comparison guidance and 20/20 quiet controls** across
41 authored consumers. Quiet means no failed original assertion, no comparison
guidance and no native proven violation. Informational reads and native gaps
remain visible. This measures supplied assertions and comparison code; it is
not an automatic package-warning accuracy score or a held-out study.

| Installed package | Cases | Targets with failures and guidance | Quiet controls |
| --- | ---: | ---: | ---: |
| `@solid-primitives/queue@1.0.0-next.3` | 36 | 18/18 | 18/18 |
| `@solid-primitives/map@1.0.0-next.2` | 2 | 1/1 | 1/1 |
| `@solid-primitives/controlled-signal@1.0.0-next.3` | 3 | 2/2 | 1/1 |
| Total | 41 | 21/21 | 20/20 |

The 41 cases ran as 82 original/comparison application executions. They cover
serial and concurrent queues, adopted and awaited child results, Promise
reactions, thenables, returned functions, constant objects, rejected adoption,
throwing `then` getters, branches, discarded reads, finally/catch returns,
identity-sensitive results, namespace imports, reexports and delayed helpers.
Every consumer and comparison passes its real installed package declarations.
No fixture stub supplies the typing result.

The assertions are the experiment's authored primary-value expectations. The
comparison code is supplied by the validation harness: captured reads for the
queue consumers and explicit tracked/snapshot variants for the other packages.
The production command does not generate these changes or choose them from
package source. Case roles are used only by the validation scorer. The CLI
receives interactions, assertions and separate application paths.

## Implemented boundary

- `packages/cli/scripts/feedback-browser.mjs` starts a local Vite/Solid server,
  runs the application in installed Chromium, executes a bounded scenario,
  maps browser stacks to configured original source, binds executed runtime
  inputs and composes the result with native analysis.
- `feedback-native-hook.mjs` edits only a byte-reviewed RC.9 shared reader and
  native `untrack` implementation. TypeScript parses and prints that exact
  JavaScript; it does not infer user or package behavior.
- `feedback-read-runtime.mjs` observes normal unowned/untracked native reads.
  Node identities are weak; retained evidence is bounded to 256 records and
  1 MiB. Thrown reads retain their exception behavior. Explicit `untrack`
  suppresses read observations. No values, Promises or thenables are traversed
  to infer result flow.
- `feedback-assertion-selector.mjs` ports the decision boundary of
  `replay-assertion-feedback-v1.mjs`: an assertion that fails in the original
  and passes in its measured comparison receives informational guidance.
  Passing originals stay quiet; a broken passing assertion remains open.
- The CLI exposes this through `feedback run`. The Node adapter awaits the
  live operation and preserves the original native exit status.

The original source is what native Type Facts and compiler execution facts
analyze. Browser observations never promote package contracts or turn missing
static facts into proven violations. Read occurrence does not establish stale
result causality. Comparison success does not establish repair safety or the
correctness of unasserted side effects.

The selector's message therefore describes the supplied comparison, rather
than claiming that an automatically generated captured-read repair is safe.
The runtime hook works across packages that reach the reviewed shared native
reader; it contains no queue/map/controlled-signal behavior models.

## Additional boundaries

Three additional checks exercised behavior outside the scored population:

1. A failing application assertion without a comparison remains an error;
   no comparison guidance is invented.
2. `queue.enqueue('not a callback')` in an uncalled function produces one
   diagnostic against the real published queue declaration. Extra captured
   observations, assertions and guidance are suppressed for that project.
3. Reading a pending memo through a native component strict-read context
   produces a real `PENDING_ASYNC_UNTRACKED_READ` browser exception. The
   collector records an error at the mapped original source. The application
   later reaches its expected settled value, so its passing assertion does
   not hide the runtime exception.

The plain-handler contrast found a remaining native precision issue. A
`button.onclick` assignment reading a memo after an input update produced an
SC5001 proven finding, but the measured execution raised no exception.
Wrapping that read in `createComponent` enters the strict-read context and
does reproduce the exception. The static callback/strict-window premise still
needs refinement; the live collector does not rewrite native classifications.
This is retained as an unresolved native overclaim, not scored as a detector
success. Its original report is
`rust/target/development-live-cli-boundaries-v1/native-pending-exception.json`.

## Validation and retained evidence

- Full CLI suite: **313 tests pass**, including 19 new focused tests for the
  collector, scenario admission, exact-byte refusal and assertion selection.
  The declaration type check also passes.
- Universal checks pass: Rust formatting and workspace Clippy with the
  certification environment, schema parsing, dialect-manifest validation and
  whitespace checks.
- `audit-live-feedback.mjs` independently reconstructs selector decisions
  from scenario assertions and measured values without importing the selector.
  It verifies current source digests, reviewed runtime bytes, original typing
  results, informational read classification and absence of native proven
  findings in all 41 scored consumers. Result: **21/21 targets, 20/20 controls**.
- All retained runtime counters in the 36 queue cases report zero dropped
  records. Coverage remains explicitly incomplete.
- Full `make verify` was not rerun for this adapter slice. Native source,
  fixtures, compiler pins, contracts and public schemas have not changed in
  this slice; the previous full RC.13 candidate verification still describes
  that native implementation. No new fixture snapshots or contracts were
  generated.

Retained summary and source/check digests:
`benchmarks/reviewed-package-models/development/live-feedback-evidence.json`.
Detailed generated applications and reports remain under:

```text
rust/target/development-live-cli-v6/
rust/target/development-live-cli-broad-v1/
rust/target/development-live-cli-packages-v1/
rust/target/development-live-cli-boundaries-v1/
rust/target/development-live-cli-boundaries-v2/
rust/target/development-live-cli-audit.json
```

Browser execution uses the retained published RC.9 runtime and compiler.
Original-source analysis uses the locally integrated RC.13 native checker.
The runtime audit is not silently upgraded by that compiler integration.
No package publication is required to use this checkout.

## Use and remaining work

See [development feedback](../../development-feedback.md) for the command,
scenario format and native-binary override. An application needs an HTML entry,
the controlled Vite/Solid setup and explicit readiness/assertion steps.

Automatic package source extraction, the `native-read-feedback-v28` source
warning selector, automatic comparison planning, editor/watch integration,
store-specific serving paths and other native runtime profiles remain
unimplemented. Owner-present reads and unexecuted paths are outside this read
collector. Unmapped errors remain visible without an invented source location.
The native strict-window issue above remains open.

The implemented path establishes that useful, quiet assertion-assisted feedback
can reach the actual CLI across several package shapes. It does not establish
feedback for every package rule without application intent or comparison code.
