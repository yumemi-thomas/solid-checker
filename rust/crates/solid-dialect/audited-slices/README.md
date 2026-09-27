# Audited runtime slices

Each `*.slice` file here is a **verbatim byte range of a published npm
tarball**, checked in so that
`solid_2::tests::every_negative_row_citation_resolves_to_the_bytes_it_claims`
can verify an `AuditedCitation::Implementation` with no package install.

## Why they exist

A `Summary` citation names a byte range inside a contract document this
repository already carries, so the test reads the document and re-derives the
closure. An `Implementation` citation names a byte range inside a *published
archive*, which this repository does not carry — so without these files the
slice half of the check could only run where the archive happened to be
installed, and "green" would have meant "skipped" on most machines. Every
`slice_sha256` in `solid_2.rs` is therefore verified against the bytes here
**unconditionally**, and the cited file against its archive's pinned
`files.json` (digest and byte length). The archive-reading arm is a second,
stronger check that confirms these bytes are still the archive's bytes at the
cited offsets. It is armed per release:

- rc.3, `SOLID_CHECKER_RC3_ARCHIVE_ROOT`: `scripts/verify.sh` always arms it
  from the tsc-oracle install, and `SOLID_CHECKER_EXPECT_PROBE_PINS=1` makes
  its absence a failure.
- rc.6, `SOLID_CHECKER_RC6_ARCHIVE_ROOT`: optional. Nothing provisions an rc.6
  tree today, so a run without it is not a failure, and for rc.6 the
  unconditional slice-and-pin check is what runs. Point it at a directory
  holding `@solidjs/signals/` of the installed `2.0.0-rc.6` to add the
  archive-reading check.
- rc.9, `SOLID_CHECKER_RC9_ARCHIVE_ROOT`: optional, as for rc.6. Point it at a
  directory holding `@solidjs/signals/` of the installed `2.0.0-rc.9`.

## Layout

    solid-v2/<phase0-release-dir>/<phase0-archive-dir>/<package-relative path>.<start>-<end>.slice

`<phase0-release-dir>/<phase0-archive-dir>` is the directory under
`benchmarks/package-contract-v2/phase0/` that pins the archive the citation's
row is about — `rc3/solidjs-signals`, `rc3/solid-js`, `rc6/solidjs-signals`,
`rc9/solidjs-signals` —
which is where the whole file's pinned `sha256` and byte length live. The
release directory is part of the path because two prereleases of one package
cite the same package-relative paths (both `@solidjs/signals` archives cite
`dist/prod/core/owner.js` at `10535-10655`, with different file digests), and a
slice is a claim about one archive's file. `<start>-<end>` is the citation's
own half-open byte range. The path is derived from the row's
`(package, version)` and the citation's fields, so a row and its slice cannot be
named inconsistently.

Two slices may hash to the same value — `@solidjs/signals`' `import` default
and `require` bundles are byte-identical for `createRoot`, `getOwner` and
`onCleanup` — and are still stored twice, once per file, because the claim is
about each file.

## Provenance and licence

The bytes under `solid-v2/rc3/` are from `@solidjs/signals@2.0.0-rc.3`
(`sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`)
and `solid-js@2.0.0-rc.3`; those under `solid-v2/rc6/` are from
`@solidjs/signals@2.0.0-rc.6`
(`sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==`,
`package.json` sha256 `de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c`),
cited by `docs/package-contract-v2/audits/2026-09-25-solid-2-rc6-signals-negative-rows.md`;
those under `solid-v2/rc9/` are from `@solidjs/signals@2.0.0-rc.9`
(`sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==`,
verified against the registry tarball, `package.json` sha256
`c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6`), cited by
`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-signals-negative-rows.md`
and `docs/package-contract-v2/audits/2026-09-27-solid-2-rc9-signals-negative-rows-parity.md`.
rc.9 splits its development build in two, so a development-build citation names
the file that defines the export: `dist/dev-shared.js` for `getOwner`,
`createRoot`, `untrack`, `runWithOwner` and `flush`, which `dist/dev.js`
imports, and `dist/dev.js` for the rest. All are published by the SolidJS project under the MIT licence. They are reproduced
here as citation evidence only. Do not edit a slice: it is not source, it is a
quotation, and the test that reads it exists to detect exactly such an edit.

Adding a slice is a step of adding an `AuditedCitation::Implementation`, and
both belong in the same commit as the audit section they cite.

## `solid-v1/`

    solid-v1/<phase0-archive-dir>/<package-relative path>.<start>-<end>.slice

Solid 1.x had the same layout without the release directory, rooted at
`benchmarks/package-contract-v2/phase0/solid-1x/` instead of `rc3/`, verified by
`solid_1x::tests::every_negative_row_citation_resolves_to_the_bytes_it_claims`
with `SOLID_CHECKER_SOLID1_ARCHIVE_ROOT` as the archive-reading arm. **That
dialect was retired in 2026-09 (ADR 0110)**, and the test and environment
variable went with it; the paragraph is kept because the layout is what a second
dialect's slices would follow.

These bytes are from `solid-js@1.9.14`
(`sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==`),
published by the SolidJS project under the MIT licence. Six runtime bundles are
cited per export — `dist/{solid,dev,server}.{js,cjs}` — because the `exports`
map runs a different one per condition, and the `.js`/`.cjs` twins are
byte-identical for every cited definition, which the equal `slice_sha256`
values state rather than imply. The audit is
`docs/package-contract-v2/audits/2026-09-12-solid-1x-1.9.14-core-primitives-creates.md`.
