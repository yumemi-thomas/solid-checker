# Callback slot identity and queue feedback

## Assessment

There is more reason for cautious optimism. One shared mechanism closes both
previous queued-task misses without a queue-specific behavior contract. It
also catches eight fresh queue cases with ordinary synchronous callbacks,
including callbacks that return a Promise or an object. This is evidence for
useful development feedback across shared execution patterns. It does not
establish coverage of every package, every rule or real application.

The prototype emits informational, conditional hints. Native execution shows
a read with neither observer nor owner; source and runtime identity connect it
to the original memo callback call. Returned-value flow and developer intent
remain open. No contract certification authority or production rule is added.

## Mechanism

The source matcher admits a fresh ordinary object literal whose static data
field stores an exact parameter or readonly parameter alias. Spreads,
accessors, methods, computed keys, duplicate keys and `__proto__` stay open.
Names such as `fn`, `task` or `enqueue` grant no behavior.

During an admitted consumer call, the runtime records that object's weak
identity and the actual function stored in its field. Later, a source member
call on a readonly receiver supplies invocation provenance. Its original
property call executes unchanged. An ordinary own data descriptor and the
callback's own entry must match the saved function and object identities.
Replaced callbacks, proxy receivers and replacement accessors do not match.

Callback entry activates only observation attribution. It does not restore
Solid ownership or tracking. An explicit normal synchronous callback return
and a normal original registration call are required. An object or Promise
return is recorded as that synchronous return; fulfillment is not claimed.
Throws, implicit completion, revision mismatch, explicit untrack intent and
exhausted callback observation budgets refuse feedback. No Promise reaction,
extra await or thenable inspection is added.

Returned nameless function allocations use a private self-reference that
preserves their empty name. Existing readonly bindings retain their names.
Focused comparisons cover length, prototype presence, lexical `this`,
`arguments`, `new.target`, method receivers, argument order, errors and return
identity. Function source/stack introspection and modified JavaScript
intrinsics remain outside the tested behavior profile.

## Results

| Population | Target hints | Quiet controls | Real typing exclusions |
| --- | ---: | ---: | ---: |
| Fresh queue callbacks, 23 cases | 8/10 | 10/12 | 1 |
| Previous cross-file package population, 26 cases | 9/10, previously 7/10 | 12/14 | 2 |
| Previous constant-result population, 40 cases | 8/9 | 25/29 | 2 |
| Previous async continuation population, 36 cases | 10/15 | 19/19 | 2 |

Fresh callbacks include arrows, named functions, scalar returns, Promise
returns and object returns in both the serial and concurrent published queue.
All eight captured-value fixes and both explicit untrack controls stay quiet.
Both async callbacks remain missed. Both callbacks that discard a read and
return a constant produce a noisy hint.

The 26-case replay closes the serial and concurrent second-task misses. The
object-returning retry callback remains missed. Its two earlier discarded-read
noises remain. The other 76 replayed cases retain their scores. These replays
are regression evidence; only the new 23-case population was authored after
the detector freeze. It was authored with knowledge of the motivating queue
implementation and is not a blinded package sample.

Independent audits compare all 125 variants with uninstrumented applications:
displayed values, getter contexts, counters, errors, native diagnostics and
public typing observations match. Published declarations remain the original
typing gate. A separate actual `tsc --noEmit` run checks eight fresh shapes:
seven are clean and the invalid task is `TS2345`, with no checker hint.

## Implementation and validation

New versions preserve historical frozen modules:

- `callback-slot-sites-v1.mjs` and `callback-slot-runtime-v1.mjs` own exact
  allocation facts and runtime object/function identity.
- `native-read-runtime-v6.mjs` and `async-continuation-transform-v3.mjs` add
  synchronous registered callback scopes to the existing continuation path.
- Plugin V19, browser V13 and projector V14 connect the observation to the
  current issued source revision and mapped registration/invocation/read frames.
- `callback-slot-audit-v2.mjs` independently reconstructs source facts; browser
  audit V10 also rebuilds the published typing and source programs.

The independent audit initially confuses a JavaScript parameter with its
identifier: their exact spans can coincide. Its first process exits 137 without
a completed audit; a second is interrupted. Formatting the failed cyclic AST
assertion is the likely cause. The corrected audit requires the exact node kind and uses small identity
assertions. It releases each stage's programs before explicit collection.
Detector semantics and browser evidence are unchanged. A first replay audit
also rejects an older plain report's different case-file pin; rerunning the
same replay without instrumentation supplies matching pins.

Validation passes:

- 553 prototype tests, including 34 new focused tests, with no skips;
- independent audits of all four populations;
- 19 malformed-provenance refusal checks and one retired-revision rejection;
- parsing all 439 prototype modules;
- authentication of 43 historical/current seals and 449 distinct pinned paths;
- `make verify-fast`, schema JSON validation, dialect manifest validation and
  `git diff --check`.

Full production verification, fixture coverage, ownership and contract corpus
gates are deferred because this slice changes research instrumentation and
documentation only. No Rust rule, public schema, contract, manifest, finding
snapshot or compiler lowering changes. Generated experiment outputs live under
the ignored `rust/target` directory.

## Remaining work

The next semantic gap is an async callback entered through a saved data slot.
Its source entry needs a lexical continuation token; keeping a global observer
scope active across suspension would misattribute unrelated reads. Another gap
is return flow across package storage and completion: a callback's constant
return alone does not prove the consumer's whole result constant.

Only admitted ordinary object storage, readonly named member invocations and
source synchronous entries are covered. Arrays/maps, computed dispatch,
mutable receivers, bound callbacks, methods without a stable self-reference,
implicit completion and other storage forms stay open. The installed-source
loader still admits modules through its async-body profile, so a package's
entirely synchronous callback-storage module can remain unenrolled. CommonJS,
mixed shortcut/source transforms, prebundling, automatic installed-dependency
updates, complete source discovery, long-session memory, large-app cost and
real-app precision remain open. Enrollment is not package-wide rule coverage.

## Evidence

All paths below are relative to the repository:

- Fresh browser reports: `rust/target/callback-slot-browser-{reads,plain}-v1/results.json`.
- Queue replay: `rust/target/callback-slot-detached-browser-{reads,plain}-v1/results.json`.
- Earlier regressions: `rust/target/callback-slot-{constant,continuation}-browser-reads-v1/results.json`.
- Independent audits: `rust/target/callback-slot-audit-v2.json`,
  `rust/target/callback-slot-detached-audit-v3.json`,
  `rust/target/callback-slot-constant-audit-v1.json` and
  `rust/target/callback-slot-continuation-audit-v1.json`.
- Focused/global tests: `rust/target/callback-slot-unit-v2.log` and
  `rust/target/callback-slot-all-tests-v1.log`.
- Refusal checks: `rust/target/callback-slot-projection-check-v1.json`.
- Actual typing checks: `rust/target/callback-slot-tsc-v1/results.json`.
- Frozen inputs: `rust/target/callback-slot-detector-freeze-v1.json` and
  `rust/target/callback-slot-handoff-freeze-v1.json`.
- Handoff checks: `rust/target/callback-slot-verify-fast-v1.log`,
  `rust/target/callback-slot-syntax-v1.json` and
  `rust/target/callback-slot-historical-seals-v1.json`.
