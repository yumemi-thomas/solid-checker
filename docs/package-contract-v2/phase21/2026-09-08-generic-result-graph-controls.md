# Generic-result graph controls after Until

The same explicit graph preparation used to investigate Until was requested
for three exact retained root cases from `2026-09-08-defined-input-full.json`.
Each request used fresh private output and issuer directories, the current
pinned binaries, and cache-only acquisition. No production source or binary
changed during the running full-corpus measurement.

| Probe | Graph nodes | Duration | Native result |
| --- | ---: | ---: | --- |
| Local Store 1.1.4, Solid 1 | 2 | 1.417s | refused generic result path |
| Flux Store 1.0.0-next.2, Solid 2 head | 3 | 2.483s | refused generic result path |
| Flux Store 1.0.0-next.2, Solid 2 floor | 3 | 1.997s | refused generic result path |

All three actual exits were 2. Every refusal names `recursive-value-shape`,
required presence, non-callability and `unresolvedGeneric` in the target
package's returned value. All acquisitions had zero cache misses. The
[machine-readable evidence](2026-09-08-generic-result-graph-controls.json)
records exact requests, importer paths, native graph/demand identities,
binary digests, preparation censuses and full refusal text.

These controls establish that merely extending the callback retry to the
generic-result family does not recover these rows. The production retry
remains limited to its measured callback families. Local Store still needs
runtime-bound Proxy result evidence; Flux Store still needs an exact
dependency returned-value premise and returned-property provenance. There
are zero new certified entrypoints or complete-row transitions here, and no
partial child artifacts are counted as root certification.

The separate CommonJS frontier also requires more than an export-name rule.
The retained `partial-json@0.1.7` root used by TanStack AI Solid has mutable
`exports` assignments, `require("./options")`, getter-based re-exports,
`__exportStar` and `__createBinding`. A supported recovery must bind those
loader and export identities and their complete write/re-export behavior;
declarations alone cannot establish them. No CommonJS proof rule was added.
