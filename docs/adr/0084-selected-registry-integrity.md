# Require archive integrity on the selected registry release

SolidStart's retained compiler sources include `error-stack-parser@2.1.4`.
Its authenticated archive cache has exact SHA-512 integrity for that release,
but the metadata also contains historical version 0.1.0 with only a `shasum`.
The private native registry parser previously required `dist.integrity` on
every record before selecting the requested version. This refused the graph
without considering the selected release's positive integrity evidence.

Allow integrity to be absent while decoding a distribution record. After
exact version selection and name/version agreement, require a non-null,
canonical SHA-512 integrity value. The existing archive hash, registry-origin,
lock-selection and artifact checks continue to consume that exact value.
Missing integrity is never inferred from a neighboring version, `shasum`,
caller data or the downloaded archive. The full original metadata bytes remain
bound to provenance.

Keep the original-byte structural pass and typed deserialization, including
duplicate version and distribution-field rejection. Do not round-trip through
a JSON value that collapses duplicate fields. The new regression accepts a
valid selected release beside a historical record without integrity, and
refuses missing, null or SHA-1 selected integrity and duplicate historical
integrity fields. All three focused registry tests pass through the pinned
`make test-focused TEST=registry_` target.

This changes private acquisition validation only. There is no Type Facts
protocol, receipt schema, trust policy or coverage-metric change. Publication
and ordinary consumer verification still determine any coverage gain.

The cached 2.1.4 archive hashes exactly to its selected registry SHA-512 value;
0.1.0 is the only historical record missing integrity. After this repair the
combined SolidStart publication in ADR 0083 adds `./serialization` and retains
all ten previous cases and claims. Full `make verify` exits 0, TOTAL 125.07
seconds, with no failed-step marker. Missing/null/SHA-1 selected integrity and
duplicate fields continue to refuse.
