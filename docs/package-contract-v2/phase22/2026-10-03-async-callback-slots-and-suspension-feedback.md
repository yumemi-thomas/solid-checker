# Async callback slots and suspension feedback

## Result

The shared callback-slot mechanism now covers admitted async callbacks. The
original queue challenge improves from 8/10 to **10/10 target hints**, closing
both previous async misses. Its quiet controls remain 10/12: discarded reads
followed by constant results still produce two noisy hints.

A fresh, harder 46-case population gives **12/18 target hints and 24/26 quiet
controls**. All six supported callback shapes work in both serial and concurrent
queues: microtask suspension, timer suspension, named functions, awaited child
helpers, normal finally blocks and explicitly awaited primitive values.
Object results, Promise adoption and observation-budget exhaustion account for
the six misses. These are actual stale results with clean published typings;
the missing feedback is retained as a counterexample.

This is progress toward useful generic development feedback. It is not
evidence of complete package coverage or all-rule coverage. Feedback remains
informational, conditional and without certification authority.

## Attribution across suspension

The runtime first matches the actual saved object, data field and function, as
in the synchronous slot experiment. An async callback captures a lexical token
at its own entry. It does not activate a global observation scope for the
duration of the Promise.

Each admitted source operation temporarily supplies the token while that
operation executes. Reads during unrelated work remain unattributed. Awaited
child helpers carry their actual parent token and operation. Every helper in
the resulting chain must complete with an explicit primitive normal return.
The chain must finish at the exact registered callback. A normal original
registration call is required too. Synchronous completion before that call
returns is buffered; a registration throw revokes it.

Nothing restores a Solid owner or observer. Instrumentation adds no Promise
reaction, extra await or thenable inspection. Nameless function allocations
retain their empty name; readonly named functions retain their binding and
receiver behavior. Explicit untrack intent, replaced functions, replacement
accessors, proxy receivers, retired revisions, throws, implicit completion and
exhausted observation budgets remain closed.

The async projector checks the current source allocation, parameter, receiver,
callback, helper definitions, exact operation declarations, completion kinds
and mapped frames. An independent validator also checks child entry frames
against their parent operation. It reconstructs the public typing program and
the separate source program without importing the detector.

## Populations and regressions

| Population | Target hints | Quiet controls | Typing exclusions |
| --- | ---: | ---: | ---: |
| Fresh async slots, 46 cases | 12/18 | 24/26 | 2 |
| Previous queue challenge, 23 cases | 10/10, previously 8/10 | 10/12 | 1 |
| Previous cross-file packages, 26 cases | 9/10 | 12/14 | 2 |
| Previous constant results, 40 cases | 8/9 | 25/29 | 2 |
| Previous async continuations, 36 cases | 10/15 | 19/19 | 2 |

The fresh population was authored after freezing the detector, using the
known motivating published queue implementations. It is not a blinded sample
of packages. The other populations are adapted regression replays.

Independent audits compare all **171 variants** with their plain applications.
Displayed values, getter contexts, counters, errors, native diagnostics and
published typing observations match. All nine typing exclusions receive no
hints. Actual `tsc --noEmit` checks cover 16 fresh serial shapes: 14 are clean;
the wrong task return is `TS2345` and the wrong Solid argument is `TS2769`.
No declaration stubs or package installs are used.

One existing quiet constant-result control loses its observation: its helper
and consumer have different issued generations despite identical input hashes.
The recorded invalidation is a late `tsconfig.json` event with no source edit.
The existing update handler invalidates a recorded path even when its bytes
have not changed. The runtime refuses the mismatched attribution. The hint/control scores stay
unchanged, but total observations move from the expected 85 to 84 and
suppressions from 20 to 19. Coherent revisions during initial module loading
remain an integration gap; a quiet result here is a refusal, not a constant
proof. The failed-closed trace is retained rather than rerun for a cleaner score.

The two fresh constant-result controls remain noisy. Suppressing a hint merely
because the stored callback returns a constant would skip the missing proof
about the consumer's whole result through the package. That flow remains open.
The implicit-return, terminal-rejection and explicit-untrack controls stay
quiet. Captured-value fixes remain quiet for object and adopted-Promise results
even though the corresponding target cases are still missed.

## Versions and checks

Historical modules stay frozen. New files are:

- `callback-slot-sites-v2.mjs`, admitting stable self-identities for ordinary
  source async functions;
- `callback-slot-runtime-v2.mjs` and `native-read-runtime-v7.mjs`, linking
  registered entries to lexical continuation tokens;
- `async-continuation-transform-v4.mjs`, plugin V20 and browser V14;
- projector V15, preserving original entry frames and checking registration
  separately from ordinary helper attribution;
- independent callback audit V3 and browser audit V11.

The new focused suite passes **53 tests**: it replays 34 synchronous safety
checks and adds 19 async checks. They include concurrent suspension, unrelated
reads, lexical naming, registration failure before/after completion, finally
overrides, exact rejection behavior, child helpers, Promise scheduling,
nonprimitive/adopted results, intent and budget exhaustion. The complete
prototype passes **606 tests with no skips**.

There are 27 malformed-provenance refusal checks and a retired-revision check.
That projector test explicitly rebinds replayed metadata to its own issued
revision; it tests validation, not new runtime observations or authenticity.
Parsing all 451 modules succeeds. Authentication covers 45 historical/current
seals and 461 distinct pinned paths. Fast handoff checks pass: the producer
stamp matches, Rust formatting and pinned workspace Clippy pass, and schema
JSON validation, dialect manifest validation and `git diff --check` pass.

Full production verification, fixture coverage, ownership and contract corpus
gates are deferred for this research-only slice. No production Rust analyzer,
schema, contract, manifest, snapshot or compiler lowering changes. Generated
browser applications, traces and checks remain under ignored `rust/target`.

## Next gaps

The present async completion gate excludes object results and Promise adoption.
Synchronous callbacks can already return these values because that evidence
claims synchronous return rather than fulfillment. A future experiment should
separate normal async body completion from proven primitive completion and
measure whether the weaker claim yields useful, clearly labeled feedback.
It must preserve rejection, adoption, timing and intent distinctions.

The 70-read budget cases repeat one source. Coalescing repeated observations
before applying a budget could retain a useful witness while still bounding
distinct evidence. That must not turn truncated or unresolved evidence into a
certified result. Constant result flow through package queues also needs its
own source/runtime evidence.

Arrays, maps, computed dispatch, mutable receivers, bound callbacks, unsupported
self-identities, synchronous-only installed-source enrollment, generators,
CommonJS, mixed transforms, prebundling, installed-dependency hot updates,
complete discovery, real-app precision and long-session memory/cost remain
open. Initial module-loading revision coherence is also open. No real-app runtime claim is made here. Function source/stack
introspection and modified JavaScript intrinsics remain outside the tested
behavior profile.

## Evidence

Repository-relative paths:

- `rust/target/async-callback-slot-browser-{reads,plain}-v1/results.json`.
- `rust/target/async-callback-slot-prior-queue-browser-reads-v1/results.json`.
- `rust/target/async-callback-slot-package-replay-browser-reads-v1/results.json`.
- `rust/target/async-callback-slot-{constant,continuation}-browser-reads-v1/results.json`.
- `rust/target/async-callback-slot-audit-v1.json`,
  `rust/target/async-callback-slot-prior-queue-audit-v1.json`,
  `rust/target/async-callback-slot-package-replay-audit-v1.json`,
  `rust/target/async-callback-slot-constant-audit-v1.json` and
  `rust/target/async-callback-slot-continuation-audit-v1.json`.
- `rust/target/async-callback-slot-unit-v1.log` and
  `rust/target/async-callback-slot-all-tests-v1.log`.
- `rust/target/async-callback-slot-projection-check-v1.json` and
  `rust/target/async-callback-slot-tsc-v1/results.json`.
- `rust/target/async-callback-slot-detector-freeze-v1.json`,
  `rust/target/async-callback-slot-handoff-freeze-v1.json`,
  `rust/target/async-callback-slot-syntax-v1.json`,
  `rust/target/async-callback-slot-historical-seals-v1.json` and
  `rust/target/async-callback-slot-verify-fast-v1.log`.
