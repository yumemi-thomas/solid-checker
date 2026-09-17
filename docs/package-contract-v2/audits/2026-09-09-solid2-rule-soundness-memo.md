# Solid 2 rule soundness and targeted contract certification memo

Date: 2026-09-09

Status: source audit; no behavioral fixes or runtime reproductions performed.
This memo records a side-conversation review of the current working tree, not
an accepted architectural decision or a certification result. The working tree
contains substantial concurrent changes; recheck the named functions before
implementation. Source links are relative to this repository and are not
immutable revision references.

## Requirement

Zero false positives is a non-negotiable design constraint: a violation needs
positive evidence for every premise it asserts. Missing evidence must remain
unknown or produce an explicit uncertifiable result when a relevant check is
blocked. It must not become either a violation or a claim of safety.

No checker finding may duplicate a diagnostic TypeScript already reports
against the real published typings. Warning severity does not excuse speculative
violation findings. This requirement is not a claim that the implementation has
been proved free of bugs.

## Findings and required validation

### 1. SC5003: opaque source options can retain a violation

**Confirmed source-policy mismatch; runtime reproducer still needed.**

`async-outside-loading-boundary` exempts sources with declared first-paint
values, but deliberately retains ordinary reporting when options are opaque.
Those options could contain the very exemption that makes the finding inapplicable.
The projection converts opaque-option findings to uncertifiable for SC5001,
but not ordinary SC5003. Calling the latter informational does not supply the
missing premise.

Evidence: [rule page](../../rules/async-outside-loading-boundary.md),
[`project_findings` and `project_finding`](../../../rust/crates/solid-reactive-ir/src/projection.rs),
[`async_source_options`](../../../rust/crates/solid-reactive-ir/src/source_discovery.rs).

Validate an options identifier that supplies a loading value, a closed options
object without one, an unresolved options value, and store-family seed behavior.
Compare against the exact installed runtime and published typings. Require
positive exemption/absence evidence or an uncertifiable result. Keep ordinary
boundary behavior separate from the client-only SSR variant.

### 2. SC7006: build-transform premise is not bound to the configured build

**Confirmed missing binding in the inspected rule; actual false positive not reproduced.**

`server-function-module-directive` recognizes a module directive and unsupported
export shapes using a documented build-plugin limitation. The inspected path
does not bind that limitation to the actual configured transform or its output.
A runtime package version alone cannot establish how a build plugin transforms
wrapped exports or re-exports.

Evidence: [`server_function_module_directive`](../../../rust/crates/solid-reactive-ir/src/server_rules.rs),
[rule premise](../../rules/server-function-module-directive.md).

Validate the same export forms against a transform exhibiting the limitation,
a transform preserving them, and an unavailable transform. A violation should
require applicable compiler/build evidence; an unknown transform cannot prove
the export is dropped. Do not modify compiler lowering to satisfy the checker.

### 3. SC1004: uncertainty from a governing read may be lost

**Potential false-positive path; needs a focused regression test.**

`component_returns_conditionally` checks whether a structural condition contains
a recorded reactive read, but sets the resulting defect's uncertainty from
component identity. An uncertain prop-backed read must not become a proven
reactive branch merely because the function is definitely a component.

Evidence: [`component_returns_conditionally`](../../../rust/crates/solid-reactive-ir/src/static_rules.rs),
[strict-read uncertainty projection](../../../rust/crates/solid-reactive-ir/src/projection.rs).

Validate a known component with unknown prop callers, a proven-static condition,
a proven-reactive condition, and a conditional inside a tracked JSX attribute.
Establish whether the current pipeline can actually construct the suspected
uncertainty-loss case before reporting it as a reproduced bug.

### 4. Callback timing: partial records can encounter exhaustive consumers

**Partial-contract integration risk; not a reproduced defect.**

`project_callbacks` can preserve known callback records while marking the
callback domain open. `allowed_callback_spans` checks whether all recorded
invocations of a parameter are deferred. All known invocations being deferred
does not establish that every possible invocation is deferred.

Evidence: [`project_callbacks`](../../../rust/crates/solid-reactive-ir/src/contracts.rs),
[`allowed_callback_spans`](../../../rust/crates/solid-reactive-ir/src/execution_role.rs).

Validate partial deferred-only records, closed deferred-only records, mixed
synchronous/deferred records, and unresolved guard selection. Trace both
violation production and suppression: a changed execution classification may
cause a false positive, a missed finding, or an appropriate gap depending on
the consuming path. Require an explicit exclusivity premise wherever needed.

### 5. SC7007: serializer presence and transport execution need stronger scope

**Proof gap identified by inspection; behavioral consequences need tests.**

`server-function-rich-argument` discovers local server-function targets and
uses a project-wide serializer scan. A serializer import/configuration somewhere
in the project does not by itself prove activation before this particular
transport call. Conversely, finding none in an incomplete project does not
prove that no external bootstrap installs one. An arbitrary custom serializer
also needs a supported-value premise, rather than blanket permission.

Evidence: [`server_function_rich_argument` and `project_rich_argument_serializer`](../../../rust/crates/solid-reactive-ir/src/server_rules.rs).

Validate client transport versus in-process server calls, initialization before
versus after the call, unreachable configuration, external bootstrap, dynamic
options, and serializer capability for the actual argument. Preserve runtime
value evidence through TypeScript assertions. Distinguish false-positive risks
from suppression that could hide a real transport failure.

### 6. SC9005: unrelated open domains create broad import-level gaps

**Confirmed excess scope in gap reporting; not itself a misuse false positive.**

`push_unknown_contract_claims` reports open reads, returns, owner requirements,
and async-related knowledge at import bindings before a particular downstream
rule establishes its demand. Its comment explicitly describes the remaining
lack of demand-sensitive consumption.

Evidence: [`push_unknown_contract_claims` and its import callsites](../../../rust/crates/solid-reactive-ir/src/contracts.rs).

Validate an independently proven accessor return with unrelated open creation
knowledge: an accessor-use check should consume the return evidence, while an
ownership check remains blocked only if it needs missing facts. Preserve exact
artifact refusals and prevent SC9005/SC9011/SC9012 from leaving gaps between them.
Do not silence blocked analyses by merely suppressing the import finding.

### 7. Preference findings are not runtime-defect proofs

**Confirmed product distinction.**

`prefer-for` and `prefer-show` are enabled preferences. In particular,
`prefer-show` explicitly recognizes expressions the compiler already handles
correctly. Correctly detecting a preference does not establish incorrect app
behavior. Separate these from a promise that every violation is a runtime misuse;
this memo does not change their configuration or catalog classification.

Evidence: [prefer-for](../../rules/prefer-for.md),
[prefer-show](../../rules/prefer-show.md),
[preference implementation](../../../rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_structure.rs).

## Consequences for reducing contract proof scope

Retain the expressive model and strict verification. Reduce unrelated proof
obligations and all-or-nothing acceptance, not the strength of accepted claims.

- Preserve exact artifacts, importer/resolution conditions, applicable guards,
  dependency receipts, trust, and semantic digest binding.
- Keep independently proven facts usable when sibling claim domains are open.
- Demand completeness for absence/exclusivity claims, scoped to the relevant
  operation, resource, argument, or returned path.
- Distinguish possible behavior from guaranteed behavior and preserve those
  distinctions in compact consumer projections.
- Keep app execution, Loading ancestry, and configured build behavior in their
  owning fact domains; a package contract cannot invent them.
- Retain value/resource identity, callbacks, tracking, ownership, timing, reads,
  writes, invalidations, returns, and relevant cleanup/async capabilities.
  A reactive/nonreactive boolean cannot support all current rules.

The current compact projection does not expose all rich contract semantics.
For example, returned `Action`, `RefApplication`, and
`ServerFunctionReference` shapes are not projected as corresponding values.
Several rule paths still recognize built-in primitives specifically. Describing
equivalent behavior in a contract is therefore not proof that every rule can
consume it today. See [`project_return_shape`](../../../rust/crates/solid-reactive-ir/src/contracts.rs).

## Suggested order and success criteria

1. Reproduce SC5003 and establish the SC7006 build premise.
2. Test SC1004 uncertainty propagation and partial callback consumption.
3. Establish SC7007 transport/configuration scope.
4. Make external-claim demands and gap reporting consistent across consumers.
5. Recover independently certified claims without discarding existing proofs.

For every semantic change, use focused positive, clean, and unresolved cases,
real published typings, exact runtime/compiler evidence where applicable, and
fresh pinned binaries for process measurements. Follow repository fixture and
verification procedures; preserve unrelated snapshots and concurrent work.

Measure proven misuse, discharged checks, and checks blocked by missing evidence
separately. A reduction in irrelevant gap findings is a precision improvement,
not a newly certified entrypoint. Keep complete package-row transitions and
accepted artifact-case coverage as separate measurements. No gain is estimated
or claimed by this memo.

## Validation status

Only this memo was added. Local link existence and whitespace were checked.
No source, contract, receipt, fixture, snapshot, configuration, or binary was
changed. Runtime reproducers, TypeScript oracle runs, focused semantic tests,
and full verification were not run for this documentation-only side task.
All behavioral issues above remain open; nothing was committed or pushed.
