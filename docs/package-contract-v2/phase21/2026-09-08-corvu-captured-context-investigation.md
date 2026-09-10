# Corvu graph compiler-context collision

A fresh diagnostic of `@corvu/popover@0.2.0|solid1|only` reproduces the exact
`@corvu/utils@0.4.2` `contains` original-input refusal, with no registry cache
misses. The native result is
`/private/tmp/retained-graph-native-kSeRjf/result.json`. No case certified.

An external watcher copied the private Type Facts project while that native
transaction was live. It did not alter the native project, producer or checker.
The diagnostic copy rebases only the project-root references in its config and
harness. The four queried runtime files have hashes matching their corresponding
installations retained by the baseline report. Capture and observations are
recorded in the [evidence](2026-09-08-corvu-captured-context-evidence.json).

With the captured shared compiler roots, `contains` for the copy beneath
`@corvu/dialog` resolves to its own implementation and has two unwritten
parameter bindings. Queries for the copies beneath `solid-dismissible`,
`solid-focus-trap`, and `solid-prevent-scroll` resolve to the dialog copy instead;
all three correctly withhold those bindings. This reproduces the earlier
synthetic duplicate-package observation in the failing package's actual graph
layout. It is not evidence that a foreign declaration is acceptable.

Keeping the same materialized bytes and compiler options, but opening a
separate program for each query with that copy's runtime chunk and its own
importing harness as roots, restores the exact own declaration and both
bindings for all four copies. Both producer diagnostics exit 0. These are
producer-only observations, not receipt verification or new certification.

The next implementation should separate acquisition programs when multiple
installed roots share a compiler package identity. Preserve the authenticated
materialization, original installed resolution topology, complete schedule-owner
lookup, each request's dependency authority, and exact source census. Limit
program roots to the request being acquired; do not delete the other installed
bytes or redirect a query to whichever copy the shared checker chose. Reacquire
the entire request, including local census transcripts, in one pinned session;
never mix answers from the shared and isolated sessions. A dependency closure
that still forces an ambiguous identity must remain refused.

Focused tests must include two copies of the same package/version with distinct
resolution contexts, exact own declaration and parameter identities, a mutated
parameter negative case, and foreign/missing dependency controls. Final graph
publication and ordinary consumer verification must establish any recovered
case. No implementation or coverage gain is claimed here. The full retained
corpus for ADR 0078 is running with source and binaries fixed, so implementation
of this next slice was deferred until that measurement became terminal. It has
now finished with exit 0; see the
[full comparison](2026-09-08-verified-retained-floor-full-recovery.md).

## Implementation seams inspected

`acquire_and_verify_graph_export_values` already distinguishes importer
variants by installed package root in `importer_invariant_request_key`; keep
that identity. Its shared project and schedule currently root every graph
plan, which causes the compiler collision even though materialization itself
preserves all installations correctly.

Detect duplicate `(package name, version)` identities at distinct materialized
roots, including authenticated source packages. For a request belonging to
such an identity, acquire a separate program. Keep the shared path for other
requests initially; a remaining dependency-context refusal is not permission
to broaden this experiment without inspecting it.

Two internal separations are required. `materialize_with_source_refs` currently
adds both materialized dependencies and explicit program plans to the `files`
array; an isolated variant must retain the former bytes while rooting only the
requested plan's authenticated runtime closure. `derive_export_value_schedules`
uses the same plan list for harness subjects and implementation-owner lookup;
an isolated variant must build only this request's subjects while retaining
the complete owner list for foreign runtime bindings. Do not confuse either
list reduction with reducing the verifier's dependency or source census.

Share the existing acquire/local-transcript/verify sequence between both paths
to prevent verification drift. Discard no verifier requirement, transport no
trial receipt, and change no public protocol or receipt interface. Test exact
root selection as well as the armed native graph publication: a producer-only
test cannot establish a recovered catalog case.
