# Independent entrypoint recovery

The opt-in recovery lane now separates unproved additions from independently
certifiable selections. It preserves the original expected case coordinates
and explicit refusals, never changes the coverage denominator, and does not
reuse trial receipts in a different context.

## Implementation

For at most 32 artifact cases, graph preparation first tries the complete set.
If that fails, it prepares all retained generated cases, then adds each missing
case individually. Each preparation has a separate scratch directory; all
workers finish before a failed preparation returns. A final preparation
rebuilds the successful selection. The audit retains `expectedCases`,
`preparationRefusals`, and the number of preparation transactions.

Native certification similarly tries the complete prepared set first. On a
proof refusal it independently certifies the mandatory retained cases in a
private catalog, then each proposed addition together with the accepted
selection. Finally it freshly certifies the combined successful selection into
the requested catalog through the existing atomic native publication and
ordinary consumer verification. Private receipts are not copied into the
final catalog or used as proof inputs. `caseRefusals` retain the exact entrypoint,
conditions, proof demand, family and reason; `publishedCases` name only the
final accepted selection. Native duplicate, conflict and trust checks remain
authoritative. Non-proof execution failures propagate.

If a retained graph case cannot certify, the original generated proposal is
freshly certified through its original lane. This fallback must itself succeed;
an existing receipt or an earlier trial is never treated as success. The audit
records the graph refusal and the actual fallback lane. Above the 32-case
bound, the complete transaction remains all-or-nothing.

## Measured first blockers

- Pacer: `@tanstack/pacer@0.22.0 ./types` has no runtime ESM exports. Isolating
  it permits other roots to prepare. `@tanstack/solid-pacer@0.22.0 ./utils`
  then certifies alongside retained `./provider`. Most other roots reach the
  unproved generic input shape of `@tanstack/store@0.11.1 shallow`.
- Solid 1 Router: development-condition roots reach a dependency graph cycle;
  `./ssr/server` also requires a runtime policy for `node:stream`.
- Solid 2 Router: `./ssr/server` requires the same runtime policy; prepared
  roots reach `@tanstack/router-core ./isServer:loadServerRoute`, whose proposed
  non-callable export shape is not proved. The retained-proposal fallback is
  required to preserve existing certification.
- Table: isolating additions preserves both `./flex-render` condition cases.
  Root and worker-plugin additions reach the `store.shallow` blocker; static
  functions retain the separately documented generic `grouping.get` refusal.
- SolidStart: even its retained graph roots can reach `solid-start:app`, so
  the original proposal is freshly certified when graph preparation fails.

These are entrypoint investigations, not creates-refusal reduction.

## Full-corpus checkpoint before protocol 39

`rust/target/ecosystem-investigations/2026-09-07-independent-recovery-full.json`
finished successfully in 625.033 seconds. Published catalog pointers and ordinary
consumer verification establish **320 complete, 48 partial, 30 refused and 20
not advanced**, unchanged from the preceding recovery full run. The runner's
generation headline is not this certification metric.

Compared with `2026-09-07-entrypoint-recovery-full.json`, Pacer adds `./utils`
beside `./provider`, and each of three SSE rows adds `.` beside `./worker`.
The SSE additions were already measured in scoped probes; Pacer's case is new
in this slice. These are four additional accepted artifact cases and zero
complete-row transitions. All preexisting accepted runtime/declaration hashes
and resolution branches are preserved; all nontarget coverage and installed
versions match. The exact before/after case selections, receipts and published
pointers are in [the measurement](2026-09-07-independent-recovery-measurement.json).
No denominator was changed.

At this checkpoint 183 CLI tests (plus TypeScript checking), 63 runner tests and
full `make verify` passed. Full verification exited 0 with `TOTAL 129.62s` and
no `FAILED during step` marker. Protocol 39 requires its own subsequent check.

## Proof extensions: prerequisites not yet implemented

The subsequent bounded proof slice is ADR 0056's unwritten parameter input
identity, motivated by `store.shallow`. It does not implement the broader
extensions below or imply their certification.

### Table conditional member

Existing `GuardAtom::Property` syntax is not sufficient authority. The verifier
currently refuses guard partitions not bound to an exact guard ordinal
(`type_facts.rs`, `GuardPartition`). Supporting Table requires a producer-owned
branch transcript tied to the exact optional member call, receiver path,
parameter identity, branch reach and source hash; a verified exhaustive
otherwise branch; and consumer selection under the same callable-property
premise. Optional chaining tests nullishness, not callability. A generic missing
member cannot become callable by treating the declaration's absence as evidence.
Positive, nullish, non-callable, shadowed, getter/proxy and mismatched-path cases
must travel through producer, verifier and ordinary consumer tests together.
No conditional-member proof has been added in this slice.

The [subsequent protocol 39 measurement](2026-09-07-unwritten-parameter-recovery.md)
recovers Table's root and worker plugin, preserving both flex-render cases;
the conditional-member refusal remains explicit for static functions.

### SSE module evaluation

The authenticated `worker-handler.js` has top-level `Map` construction and
`self.addEventListener` registrations for dedicated and shared workers. It is
not an empty or declaration-only module. Current proposal validation requires
runtime exports and has no module-evaluation subject. Removing that check would
not prove worker execution or effects.

A supported extension needs a distinct module-evaluation subject bound to the
runtime artifact, complete module closure, worker realm, evaluation trigger,
and registration/handler identities. Event execution must remain distinct from
module evaluation. Unknown worker/EventSource behavior stays explicitly open
until an exact runtime model or dependency contract supplies it. The schema,
normalized model, proof schedule, receipt, consumer and veto harness must agree
on that subject. No synthetic export or empty summary has been substituted.
This model is not implemented or measured as a certification.

### SolidStart virtual input

The retained package's `dist/config/index.js` resolves `solid-start:app` to
`appEntryPath`, discovered from the application's configured root and app root.
It is consumer application code, not a missing published package archive.
An actual application/build configuration is required and has been requested
from the user. A future adapter must bind importer, conditions, resolver and
plugin configuration, source/output hashes, transitive closure and transform
identity; the native verifier must authenticate those inputs before accepting
any virtual edge. A fabricated placeholder application cannot establish
coverage for the current package probe. This dependency remains pending.
