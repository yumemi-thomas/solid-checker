# Compiler and Type Facts bootstrap

This work happens immediately after the legacy baseline and before the package-
contract semantic model is implemented. It changes source ownership and build
wiring, but it must not change checker findings until an explicitly reviewed
fact-protocol improvement lands.

## Outcome

- Solid 2 compiler facts come from a revision-pinned branch of the compiler now
  owned by `solidjs/solid` under `packages/compiler`.
- The fork branch contains semantic-fact code only. It does not carry compiler
  behavior, lowering, runtime, feature, or performance changes.
- The semantic trace currently carried by the `dom-expressions` fork is ported
  to that compiler and proven output-neutral before it is extended.
- Solid 1.x continues to use the separate `solid-1x-compiler` fork.
- The Type Facts Go producer and Rust client return to this repository so one
  change can update producer, consumer, fixtures, and proof obligations.
- Physical colocation does not collapse the Type Facts, compiler-facts, Reactive
  IR, or package-contract module interfaces.

## Verified starting point

On 2026-08-27, the official [`solidjs/solid` `next` compiler][solid-compiler]
was present as `packages/compiler`, with crate name `solidjs-compiler`. The
observed `next` head was [`2f01f23e30d2`][solid-head]. The implementation has
already diverged from the former DOM Expressions tree, including newer server-
function lowering, so copying the old compiler directory or blindly applying a
whole branch is unsafe.

The checker currently consumes
[`yumemi-thomas/dom-expressions@26e744fb`][current-dom-pin]. That revision
exposes semantic trace version 2 and includes the total execution-site census,
ownership and callback wrapper facts, reconciliation fixes, and output-neutral
trace corrections used by the checker. The fork's observed `next` head was
[`46fe53df6bbe`][dom-next-head]. Treat the fork head as the port inventory and
the checker pin as the behavior-preservation baseline; reconcile their delta
explicitly.

The checker and the external Type Facts repository currently agree on
[`solid-ts-facts@92c53392`][typefacts-pin]. That exact revision is the import
baseline.

These hashes document the review observation. At execution time, record fresh
heads and create branches from exact commits rather than from a moving name.

## Ordering rule

Do not combine the source move, semantic-trace port, trace improvements, and
checker-protocol migration in one unreviewable change.

1. Capture the current checker and corpus baseline.
2. Port existing compiler facts to the Solid compiler with no semantic delta.
3. Switch the checker to the pinned Solid fork and prove finding parity.
4. Import Type Facts into this repository with no protocol delta.
5. Switch the build to local Type Facts and prove binary/protocol parity.
6. Add Type Facts and compiler-fact improvements in small producer-consumer
   slices.

The work belongs to one early bootstrap milestone, but parity and improvement
remain separate commits and separate gates.

## Semantic-only fork invariant

The Solid fork exists only to observe and report semantics the upstream
compiler already implements. Allowed changes are:

- semantic trace model and version definitions;
- trace recorders and output-neutral calls at existing lowering decisions;
- trace validation, reconciliation, deterministic serialization, and optional
  host-independent access;
- facts-only configuration that enables trace production;
- semantic-fact unit, mutation, and output-neutrality tests;
- documentation of the semantic-fact interface.

Forbidden changes are:

- generated JavaScript or source-map changes;
- changes to lowering decisions, ordering, optimization, or diagnostics;
- DOM, SSR, hydration, server-function, server-component, runtime, or package
  behavior fixes;
- unrelated compiler refactors or dependency upgrades;
- checker-specific runtime helpers or hard-coded Solid API behavior.

Instrumentation may add a recorder call inside a lowering module, but it may not
change the lowering branch, data, control flow, or result. If a correct fact
cannot be emitted without changing compiler behavior, leave that fact open and
raise the compiler change independently against upstream. After upstream lands
the behavior change, rebase the semantic branch and add facts for the new
decision. Never use the checker fork to carry the fix.

## Solid compiler fork procedure

### Establish the branch

1. Use the existing `yumemi-thomas/solid` fork.
2. Fetch `solidjs/solid` and record the exact `upstream/next` commit.
3. Create a clean branch such as `solid-checker/compiler-facts-v2` from that
   exact commit. Do not branch from a dirty local checkout or a stale local
   `next`.
4. Record the upstream base in the fork branch, checker notice, and compiler
   conformance report.
5. Keep the branch free of ordinary Solid compiler changes. Upstream compiler
   changes arrive only by rebasing or merging the recorded `upstream/next`
   base, never as checker-fork patches.

### Build a port ledger

Inventory every semantic change between the official compiler and
`yumemi-thomas/dom-expressions#next`. Classify each as:

- already present in `solidjs/solid`;
- trace-only and still required;
- an emitted-code correction already present upstream and therefore not ported;
- an emitted-code correction still absent upstream and therefore reported
  upstream, not carried by the semantic branch;
- obsolete because the new Solid compiler changed the lowering primitive;
- unresolved and therefore fail-closed.

The ledger must include the original total-trace and host-independent compiler
work, execution-site terminal decisions, fragment and folded-attribute census
fixes, owner establishment, component-render and deferred-callback facts,
nested children/text-content fixes, shadowed/discarded JSX retraction, void and
`noscript` decisions, ref/event handling, and every later trace-only correction
through the selected fork head.

Do not use cherry-pick success as evidence of semantic correctness. The code has
moved repositories and lowering has continued to evolve; port facts at the new
decision sites and compare observable traces and generated output.

The port ledger is an inventory, not a patch list. Any historical commit that
changes emitted behavior is excluded even when it originally accompanied a
trace fix. Extract only its semantic observation and test after the upstream
compiler independently owns the behavior being observed.

### Parity port

The first compiler branch milestone carries the current semantic meaning only:

- a total census of compiler-controlled source sites;
- exactly one terminal disposition per site;
- explicit discarded/elided decisions;
- callback roles for events, refs, component render, control-flow render, and
  deferred invocation;
- compiler-established owner relations;
- source spans and grouping needed by the current adapter;
- trace format validation and an independent consumer version assertion.

With trace collection disabled and enabled, generated JavaScript, source maps,
diagnostics, and compiler side effects must be identical. Trace production may
allocate and serialize facts, but it must not alter lowering.

### Improvements after parity

After the checker passes unchanged against the parity port, introduce semantic
trace version 3 and checker compiler execution-facts protocol 2. They are
different version namespaces and must not share a version constant.

Recommended additions, in priority order:

1. A reconciliation envelope containing censused-site count, terminal-site
   count, unsupported-site count, source hash, transform configuration digest,
   compiler revision, output hash, and mode.
2. Stable operation identities within one source/configuration generation,
   plus explicit source-to-generated operation relations.
3. Separate trigger, scheduling, tracking, invocation cardinality, owner
   requirement, owner production, and cleanup ownership fields.
4. Explicit generated callbacks and causal edges for multi-stage lowering.
5. Compiler-created resource/lifetime facts where emitted code establishes
   them.
6. Server-function directive, source export, client reference, server
   registration, wrapper preservation, and build-face identities.
7. Explicit unsupported or opaque lowering markers that open only the affected
   compiler fact domain.
8. Deterministic canonical serialization for proof transcripts.

Do not emit runtime-library semantics from the compiler. For example, the
compiler may prove that a generated callback is invoked by a renderer helper,
but the helper's later scheduling and cleanup behavior remains a package-
contract fact unless the emitted code itself establishes it.

Do not improve the compiler while improving its facts. A discovered lowering or
runtime defect is an upstream work item and a temporary open fact domain, not a
reason to widen this fork.

### Compiler gates

- compiler unit tests for every trace decision site;
- trace-on versus trace-off output identity;
- differential generated-output comparison against the exact upstream base;
- a zero-diff assertion for generated JavaScript, source maps, and diagnostics
  across the complete compiler corpus;
- a fork-scope audit rejecting non-semantic source changes;
- total-census mutation tests that delete or duplicate one terminal decision;
- DOM, hydration, SSR, universal, server-function, ref, event, control-flow,
  discarded, and server-component fixtures;
- current-checker finding parity before protocol improvements;
- focused checker fixtures for each new fact;
- full compiler and checker verification on every upstream-base move.

### Checker consumption

Replace the Solid 2 dependency with package `solidjs-compiler` from the exact
revision of `yumemi-thomas/solid`. Keep `default-features = false` and the
host-independent Rust interface. Update the adapter, notice, Cargo lock, cache
identity, trace-version assertion, compiler-revision report, and conformance
fixtures atomically.

Never pin the moving `next` branch. Never silently fall back to the old DOM
Expressions compiler when the Solid compiler trace is unavailable. A missing or
incompatible trace is an explicit compiler-facts failure.

## Type Facts repatriation

### Target layout

Use the earlier monorepo separation, updated for the current producer:

```text
go.mod / go.sum                         Go workspace identity
apps/solid-typefacts/                   Go command and private producer modules
shims/                                  audited TypeScript-Go shim modules
rust/crates/typefacts/                  Rust process/session client
schema/typefacts-*.json                 wire schemas and codec limits
docs/typefacts/                         Type Facts decisions and producer docs
benchmarks/typefacts/                   protocol and performance goldens
```

The Go producer and Rust client remain one deep Type Facts module. Their
external interface is the versioned process/session protocol; Reactive IR does
not learn Go, TypeScript-Go, CBOR-table, or retained-session implementation
details.

### History-preserving import

1. Freeze the external repository at the exact checker pin and require a clean
   external worktree.
2. Import that commit with its history under a temporary prefix. Do not copy an
   arbitrary working directory.
3. Relocate files with `git mv` into the target layout in a follow-up commit so
   provenance and rename detection remain useful.
4. Reconcile the external `CONTEXT.md` terms into the root glossary.
5. Preserve Type Facts ADRs under `docs/typefacts/adr/`; do not renumber them
   into the checker ADR namespace.
6. Preserve licenses, TypeScript-Go revision audits, schema goldens, memory and
   lifecycle benchmarks, and all adversarial retained-session tests.
7. Remove the temporary import prefix after every file has an owner.

The import commit may be mechanically large, but the behavior-changing commits
after it must remain small and independently green.

### Build and identity migration

1. Add `rust/crates/typefacts` to the main Rust workspace and replace the git
   dependency with a path dependency.
2. Build `apps/solid-typefacts` directly; remove clone, fetch, detached-checkout,
   and revision-extraction behavior from `scripts/build-typefacts.sh`.
3. Replace the external-revision stamp with a source manifest digest over the
   Go producer, Rust client, shims, schemas, `go.mod`, `go.sum`, TypeScript-Go
   pin, and build id.
4. Keep the startup handshake over protocol version, schema digest, producer
   build identity, and codec limits.
5. Update gate-cache inputs so any Type Facts-owned source or toolchain change
   invalidates producer-dependent results.
6. Build and test the producer in the same CI change as every Rust client or
   demand-model modification.
7. Remove external Type Facts checkout/cache restore assumptions from CI,
   release packaging, notices, and contributor instructions.

### Parity gate before new facts

Against identical request transcripts, the imported producer and client must
produce byte-identical responses, handshake identities, lifecycle behavior,
memory bounds, cancellation behavior, and checker findings. If build identity
must change because the source location changed, compare decoded semantic
responses separately and document the identity-only delta.

Only after parity may Phase 3 add resolved-invocation, callable-path, finite-
domain, parameter-use-census, and control-flow-census facts.

### Retiring the external repository

After two clean CI runs and one release build from the monorepo:

- make the external repository read-only or archive it;
- replace its README with the final imported commit and new source locations if
  archival policy permits;
- close no historical record and delete no release tags;
- accept no further producer changes there;
- require all Type Facts changes to land with their checker consumers.

## Combined bootstrap exit criteria

- Solid 2 compiles through a pinned `solidjs/solid` fork revision.
- The old and new compiler paths produce identical checker findings on the
  frozen baseline.
- Trace-on and trace-off compiler output is byte-identical.
- Every old semantic-trace regression has a new-compiler fixture or an explicit
  obsolete/superseded ruling.
- Type Facts builds from this repository without cloning another repository.
- The local producer/client pair passes every imported Go, Rust, protocol,
  lifecycle, memory, and adversarial test.
- No external Type Facts PR or pin move is required for a new fact.
- Source ownership changed without weakening any fail-closed behavior.
- Protocol improvements begin only after both parity gates are green.

[solid-compiler]: https://github.com/solidjs/solid/tree/next/packages/compiler
[solid-head]: https://github.com/solidjs/solid/commit/2f01f23e30d2840139dcbfbed79b270c676a09ad
[current-dom-pin]: https://github.com/yumemi-thomas/dom-expressions/commit/26e744fb4feb973a3652bfc45a8c3938ece667f0
[dom-next-head]: https://github.com/yumemi-thomas/dom-expressions/commit/46fe53df6bbe1bbc5fdcf96f35fc4305df09936b
[typefacts-pin]: https://github.com/yumemi-thomas/solid-ts-facts/commit/92c53392388518d69ef27220729f5c061479deed
