# Parameter-read correction: measured coverage regression

The nine-probe run finished at 2026-09-08 00:01:01 JST with actual exit 0.
The [exact measurement](2026-09-07-read-origin-scoped-measurement.json) follows
published case-set pointers and named catalogs, checks their digests, and
records the ordinary consumer's receipt authentication and exact case selection.
Its baseline is the same nine rows from `implementation-owner-full`.

| Probe | Accepted entrypoints before | Accepted entrypoints after | Row transition |
| --- | --- | --- | --- |
| Motion 0.7.0-beta.4, Solid 2 floor | `.`, `./m`, `./v2` | `./m` | complete → partial |
| Motion 0.7.0-beta.4, Solid 2 head | `.`, `./m`, `./v2` | `./m` | complete → partial |
| Motion 0.6.0, Solid 1 | `.`, `./v1` | none | complete → refused |
| Solid Primitives Utils 6.4.1 | `.`, `./immutable` | `./immutable` | complete → partial |
| TanStack Solid Pacer 0.22.0 | 13 entrypoints | same 13 | partial → partial |
| TanStack Solid Table 9.1.2 | 3 entrypoints, 6 artifact cases | same 6 cases | partial → partial |
| Corvu Next Popover, Corvu Popover, Corvu | none | none | refused → refused |

Accepted artifact selections fall from 29 to 22: seven lost selections, zero
new selections. Scoped row counts change from 4 complete / 2 partial / 3 refused
to 0 complete / 5 partial / 4 refused. Existing accepted coverage is therefore
**not preserved**, although both original Motion `./m` selections survive.
The linked JSON contains full before/after artifact hashes, closure hashes,
declarations, resolution branches, importer coordinates, receipts and refusals.
This is a precision correction, not a coverage improvement or metric correction.

All four regressing rows now stop at `handleDiffArray` in exact Utils 6.4.1 or
7.0.0-next.4 dependency contexts. Its source first reads the caller arrays,
then assigns `prev = prev.slice(i)` and `current = current.slice(i)`, and then
calls `includes()` on those resulting values. The existing affirmative
`unwrittenParameters` premise cannot prove either binding unchanged across
that implementation. Declaration member shape does not establish the origin
of the value held at a later read.

The next bounded implementation must distinguish reads before a write from
reads after it. A per-site source-origin premise must bind the exact parameter
declaration, read/call location and path, execution reach, and complete relevant
write/control-flow census. Captured writes, loops, argument aliasing and dynamic
evaluation cannot be omitted. A later assignment must not invalidate an earlier
proved read; an unconditional local replacement must never become caller-input
evidence. Mixed caller/local origins need their own positive premise. This is
not permission to drop unproved read claims silently or treat an unknown method
result as the original input. The accepted replacement counterexample remains
the negative control.

Corvu Popover now stops earlier at `contains`, and Corvu at
`sortByDocumentPosition`, with the same missing original-input premise. Corvu
Next Popover still reaches Floating UI `getOverflowAncestors`. These are
distinct source cases to inspect, not permission to generalize the Motion fix.

Both focused tests and full `make verify` pass for the correction (actual exit
0, `TOTAL 171.95s`, no failed-step marker). The full corpus has not been rerun
after this correction: the targeted run already establishes an unresolved
regression, which should be addressed before another full measurement. The
prior 418-row aggregate must not be presented as current coverage. No snapshots,
bundled contracts, metric definitions, commits or pushes changed in this audit.
The coverage ceiling has not been reached.
