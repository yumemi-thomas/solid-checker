# Solid Devtools after production-root recovery

A fresh pinned release transaction requested `.`, `./vite`, `./setup`, and
`./babel` under `import` for the baseline's retained `solid-devtools@0.34.5`
project. It exited 0 and published only `.` and `./vite`; ordinary consumer
verification authenticated both. The retained audit is
`/private/tmp/inert-entrypoints-CsySLM/audit.json`. This expands the attempted
case census, not the previously measured accepted set. No complete row is
established and no coverage denominator changed.

The Babel case is generated but refuses the recursive value shape of
`namePlugin`: its root is not compiler-proved non-callable and
non-constructable. Published `dist/chunk-RDZMZMK7.js` initializes it with an
object literal; `dist/babel/babel.d.ts` declares `babel.PluginObj<any>` through
`@babel/core`. These are distinct source and declaration premises. The
existing imported-factory initializer transcript does not prove a local
object-literal export. Supporting that source shape would need an exact,
unwritten binding proof and consumer replay, including the returned object's
callable members; simply discarding the declaration refusal is not evidence.

Graph preparation separately reports a missing local closure module
`./babel-7-helpers.cjs` from Babel Core's `lib/transformation/file/file.js`.
The named runtime file **does exist in the retained installation**. Therefore
this message does not establish an absent published runtime artifact, and
fetching another copy is not the next action. The closure's resolution axis
and declaration fallback were then checked with the public
`resolvePackageExport` API: both axes select `lib/index.js` through
`legacy:main`, digest
`sha256:dba868255b447218cd421b30fb5289c118f265817088e7106053cf962057dd26`.
The declaration traversal accepts `.js` source fallback but
`localModuleTarget` substitutes `.cts` and `.d.cts` for the helper's `.cjs`
path. Both companions are absent; the runtime helper exists. Thus the error
is a declaration-axis limitation, not evidence that the published runtime is
broken. Adding `.cjs` fallback alone would still not prove the refused
`namePlugin` shape or supply CommonJS runtime authority. Babel Core's
missing `@types/babel__core` in that retained project's direct node_modules
also does not establish a positive type premise.

The production `./setup` exports two destructured no-op methods; its
declarations re-export debugger setup bindings. It did not enter this
transaction's generated expected cases. The development root imports another
chunk and calls the external `warn`, so the inert initialization rule cannot
be used for that condition.

The next investigation must distinguish source-shape support, declaration
resolution, and CommonJS runtime authority. None is cleared by the successful
empty production root, and none justifies removing an existing accepted case.

## Bounded source-shape extension point

Inspection of `exportImplementationTranscriptLocked` confirms that it refuses
before recording an implementation unless the queried value has exactly one
call signature. It therefore cannot currently describe this object initializer.
`require_export_recursive_subject` checks the declaration-side root, then
`factory_exports::require` can supply an exact imported-factory return premise.
The factory transcript requires a call initializer and does not cover the
local object literal. Neither existing fallback establishes this case.

A candidate extension must positively identify the runtime export's exact
binding and object initializer in authenticated JavaScript, use resolved-symbol
write references to exclude every reassignment (including nested writes and
destructuring), and reject direct eval or an ambiguous binding. Matching a
name, scanning assignment spelling, or relying on an absent type is insufficient.
The runtime binding's snapshot owner, source path, and exact span must equal
the verifier's accepted export binding; a fact from a sibling version or
declaration file cannot substitute. The parser fact should live in solid-facts,
with certification applicability and witness recording in the backend.

This premise would establish only that the export root is non-callable and
non-constructable after normal initialization. Object members, getters,
callback effects, and module initialization effects retain their independent
obligations. Negative controls must include later and nested assignment,
shadowed same-name bindings, non-object initializers, changed bytes/spans,
re-export ownership mismatch, and direct eval. Existing declaration-based and
factory-based paths must remain unchanged.

The local fact recognizer now exists in
`rust/crates/solid-facts/src/ast/object_binding.rs`. It parses JavaScript ESM,
selects an exact top-level declaration-name span, requires an object literal,
rejects symbol redeclarations and resolved write references, and binds its
opaque result to the entire source hash and selected span. Two focused Rust
tests pass, covering changed bytes/spans, shadowed names, member writes,
nested/destructuring/iteration writes, redeclaration, eval, non-object values,
nested declarations and TypeScript ambient syntax. This fact is not yet wired
into certification in that initial test pass.

The subsequent backend integration uses the verified export's snapshot owner,
runtime path and exact query span. Export-list references are joined to their
declaration through the exact Oxc binder edge. A direct declaration must itself
match the fact recognizer; no name or containing-span search is used. The
fallback applies only to an explicitly demanded non-callable export root in
the same authenticated package, without a transform. It records both query and
declaration spans with the snapshot owner in the witness site. Dependency-owned
roots and member paths retain their existing proof requirements.

The receipt-level test passes for direct and export-list bindings declared
`unknown`, rejects reassigned and callable values, and verifies ordinary
consumer loading plus rejection under another importer. A release diagnostic
transaction at `/private/tmp/inert-entrypoints-IscIEF` publishes `.`, `./babel`,
and `./vite`; its final matching-binary measurement and full validation remain
pending. This preliminary transaction preceded the explicit same-owner and
no-transform guards, so it is not the final acceptance evidence.
The existing full corpus continues against frozen earlier binaries and cannot
measure this later object-root change.

The [final scoped measurement](2026-09-09-object-export-devtools-scoped-measurement.json)
uses the matching pinned verification binary, archived at
`/private/tmp/object-export-final-measured-native`, and publication
`/private/tmp/inert-entrypoints-wVLJVM`. The transaction exits 0, follows the
published case-set pointer to all three cases, and passes ordinary receipt
authentication and exact-case selection. It adds `./babel` under `import`,
selecting `./dist/babel.js` and `./dist/babel.d.ts`; both previous scoped main
documents (`.` and `./vite`) are byte-identical. The JSON records exact artifact
selections, document and receipt digests, and binary identities.

The row remains partial: `./setup` and package.json remain outside this
accepted set, and the development root condition is not certified by the inert
production case. No denominator correction was made. The final probe's recorded
stages total about 8.5 seconds; this is not a whole-corpus timing estimate.
Full `make verify` exited 2 in `/private/tmp/object-export-verify.log` at
`verify-performance`: one-file incremental analysis measured 122,482,167 ns
against the 100,000,000 ns ceiling. Clippy, workspace and Go race tests,
coverage and ownership passed before that failure; later checks did not run.
There is no successful TOTAL line for this run. Process inspection found an
orphaned corpus checker consuming about 948% CPU, followed by a second orphan
after its worker exited. Both task-owned orphans were terminated; the live
corpus runner was preserved. This is observed contention, not proof that it
fully explains the performance result. The full gate must pass after the
machine is no longer running the old corpus measurement.

That old debug corpus run has now been deliberately stopped (exit 143) after
more than 75 minutes, with retained artifacts preserved and no final report.
The clean verification rerun is `/private/tmp/object-export-clean-verify.log`.
It does not turn the cancelled corpus into a measured coverage result.

The clean full verification completed with actual exit 0, `TOTAL 89.98`, and
no `FAILED during step` marker. It includes the performance gate, contract
corpus, ecosystem runner tests and the remaining conformance checks. No
unrelated snapshots were changed. The earlier failed run remains recorded;
the successful rerun establishes the current worktree's verification result,
not a completed ecosystem coverage measurement.

## Isolated setup graph

The setup-only transaction (`/private/tmp/inert-entrypoints-hBAhBy`, terminal
exit 1) removes both the inert root and Vite/Babel entrypoints from the request.
It still refuses `@solid-devtools/debugger/setup`'s exact declaration binding
for `setElementInterface`. The installed debugger is exactly 0.28.1; its
`./*` export maps `import/types` to `./dist/*.d.ts` and `import/default` to
`./dist/*.js`. The named `dist/setup.d.ts` positively declares that function.

`staticRuntimeDependencies` in the CLI graph preparation filters edges to
`axis === "runtime"` and static import/reexport kinds. The parent setup runtime
has local no-op implementations, while its declaration file re-exports the
debugger's declarations. Therefore this dependency is not supplied through
that runtime-only graph edge set. The next design needs authenticated
declaration-axis resolution and exact importer context without inventing a
runtime import. Removing the axis filter is not an established fix: the native
graph, receipt composition and ordinary consumer must agree on what the
dependency proves. No declaration receipt was synthesized or copied.

Native inspection identifies an existing foundation and its current limit.
`PublishedGraphSourceRequest` and `VerifiedGraphSourcePackage` authenticate
archive bytes, the exact lock selection, and the installed node_modules
coordinate without granting runtime semantics. However,
`plan_contract_document_with_sources` currently performs ordinary planning
before attaching those source packages. `ExportReplay::external_binding`
accepts bindings only from dependency certification plans. Thus acquiring the
archive alone cannot satisfy this earlier export-binding obligation.

The bounded integration should authenticate source packages before export
planning, expose them only on the declaration axis, and replay package export
resolution from the exact declaration importer with its conditions and
installed topology. Runtime edges must continue to require contract authority.
Conflicting or unauthenticated versions must keep the existing whole-name
withholding behavior, so an absent nested copy cannot silently select a
hoisted version. Root planning, graph planning, generation inputs and ordinary
consumer reconstruction must agree on these identities. The existing source
set identity and closure digests may carry the evidence, but that sufficiency
has not yet been proven by integration tests. This inspection does not change
the public protocol, receipt interface, or accepted cases.

The focused native regression
`declaration_sources_do_not_authorize_a_misattributed_reexport_binding` now
authenticates the exact source archive and lock selection independently, then
supplies a parent resolution that wrongly attributes the re-exported value to
the parent's declaration file. Planning must reject specifically with an
export-binding error, rather than pass merely because valid source bytes were
provided. The test passes. This is a negative control for the upcoming
declaration-axis integration, not a new certificate or a completed fix.

The installed-location premise also needs strengthening before this source
channel can authorize an export binding. Native `plan_graph_source_package`
checks an absolute path with no dot components and the exact
`/node_modules/<package-name>` suffix, then hashes the supplied coordinate with
the lock selection and snapshot. It does not prove this importing declaration
selects that copy. The CLI derives a Bun locator from the lockfile-relative
installed path, but that derivation is acquisition input, not native authority.
An opaque `VerifiedGraphSourcePackage` therefore must not be promoted directly
to an external binding merely because its name and bytes match.

An alternative avoids widening source-only authority: request a full dependency
contract for the exact refused declaration re-export. Native graph ordering
already accepts a dependency importer that is a replayed declaration-role
closure module, and both closure implementations record an accepted dependency
edge on that axis. Thus a full receipt can support the declaration binding
without asserting that the parent executes the imported package at runtime.

The CLI now requests this route only for the root case's exact entrypoint and
conditions when a declaration-binding refusal names a specifier present as a
declaration re-export in its module census. The refusal text is only a discovery
hint; native verification remains mandatory. Runtime dependency discovery is
unchanged, and absent, differently conditioned, import-only or out-of-census
declaration requests do not expand the graph. The focused routing test passes.

The setup-only probe with this route remains refused, but advances past the
missing acquisition binding. Its next error is emitted during candidate
construction: the external declaration target has no planned target binding.
`main.rs` populates candidate `external_targets` only after
`accepted_reexport_summary_for_name` finds a runtime re-export. A local runtime
function with an external declaration re-export cannot pass that condition.
The next candidate-generation change must match the declaration importer,
specifier, exported name and exact target against the supplied dependency
catalog independently of the runtime re-export summary. This affects an
unaccepted proposal, not receipt authority; final graph planning must still
replay the exact external target and dependency receipt. No setup case has
certified and the previous three-case publication remains untouched.

Before enabling the positive path, native replay must bind the lockfile-relative
coordinate and the declaration import's selected dependency, including a nested
copy when one shadows a hoisted copy. A complete selection census or equivalent
positive resolution premise is needed; absence of a supplied nested source must
not imply permission to use another version. This is an additional prerequisite
to the earlier ordering change, and no existing source-only behavior was widened.

### Stop point requested by the user

The bounded declaration dependency discovery change and its negative routing
tests are retained. The setup-only experiment remains a refusal during native
candidate generation; it is not a newly certified case. Completing native
declaration target selection is outstanding work, not an implemented proof rule.
No source-only authority, receipt interface or trust policy was widened.

For this change, the published accepted entrypoint set before and after is
`{ ".", "./babel", "./vite" }` under `import`, with the exact artifact cases
unchanged in `2026-09-09-object-export-devtools-scoped-measurement.json`.
The separate failed setup experiment did not rewrite that publication. The row
remains partial, with zero new certifications and zero complete-row transitions
from this declaration-routing change. No denominator correction was made.
The cancelled full-corpus run supplies no coverage totals. Further recovery work
is deferred at the user's request to finish this bounded slice and stop.

Final validation: `GOCACHE=/private/tmp/solid-checker-go-cache make verify`
exited 0 with `TOTAL 124.58` seconds and no `FAILED during step` marker
(`/private/tmp/devtools-declaration-final-verify.log`). This includes the new
routing regression and native declaration-source rejection regression. The
initial formatting-only failure was corrected before this successful run.
No snapshots were regenerated for the declaration-routing slice.

## Resumed declaration re-export fix: certified setup

The user subsequently requested completion of this fix. Candidate generation
now resolves a direct named declaration re-export independently of runtime
re-exports. It queries the dependency catalog with the exact declaration
importer, specifier and original exported name, and checks the target's export
name, module path and digest against that selected contract. The declaration
source bytes and dependency manifest bytes must match their recorded digests;
the nearest package manifest must match the selected package name and version.
This only normalizes an unaccepted proposal. Native snapshot export replay,
lockfile selection, dependency certification and receipt trust remain mandatory.

The first full runtime dependency graph reached a real refusal in
`@solid-devtools/shared ./utils` for `formatTime`'s defaulted parameter input.
That behavior was not weakened. A dependency requested only for a declaration
re-export instead uses an independent proposal with authenticated compiler
sources for its runtime imports. Any claim needing a behavioral dependency
receipt still refuses. The preparation key distinguishes this attempt from a
runtime dependency graph; existing runtime discovery is unchanged. Debugger's
setup case certifies anew for the parent declaration importer, without copying
the baseline receipt into this context.

The final combined publication is `/private/tmp/inert-entrypoints-nwiTUG`.
Its pointer names exactly the accepted parent entrypoints
`{ ".", "./babel", "./setup", "./vite" }` under `import`. All three previous
scoped main documents are byte-identical. The added case selects
`./dist/setup_noop.js` (SHA-256
`597b725df5e79ac95e3f80a01e829062c7983bd4019aaf87cacdea9ac8c98986`)
and `./dist/setup.d.ts` (SHA-256
`3dd599e998a8fdd7d3e9bbd3755c6f5e2b5abffcae2a0c067d427673e2606938`).
It covers `setElementInterface` and `setLocatorOptions`; ordinary analysis
reports both receipt authentication and exact-case selection successful.

[Exact case and receipt evidence](2026-09-09-declaration-reexport-devtools-scoped-measurement.json)
records the before/after documents, resolution records, source and binary
digests, and consumer-bound dependency receipt/trust roots. The new native
candidate test covers aliases and rejection of missing catalog selection,
wrong importer, export, path, digest, package version, changed target bytes,
type-only exports and wildcard exports. Existing native source-only rejection
and exact receipt-context tests remain applicable.

Measured gain: one executable entrypoint/artifact case, partial to partial,
zero complete-row transitions. `./package.json` remains inapplicable and the
browser/development root condition is not established by this import case.
No metric correction, full-corpus estimate, protocol change or snapshot update
is part of this fix.

Final acceptance: the focused native candidate regression passed (1 test), the
combined package probe exited 0 and its pointer/digest/preservation audit passed.
`GOCACHE=/private/tmp/solid-checker-go-cache make verify` exited 0 with
`TOTAL 95.58` seconds and no `FAILED during step` marker, recorded at
`/private/tmp/devtools-declaration-complete-verify.log`. The full ecosystem
benchmark was not rerun; these are scoped certification results. Work stops
after this fix, with no commit or push.
