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
**unconditionally**; the archive-reading arm (`SOLID_CHECKER_RC3_ARCHIVE_ROOT`)
is a second, stronger check that confirms these bytes are still the archive's
bytes at the cited offsets, and `scripts/verify.sh` always arms it.

## Layout

    solid-v2/<phase0-archive-dir>/<package-relative path>.<start>-<end>.slice

`<phase0-archive-dir>` is the directory name under
`benchmarks/package-contract-v2/phase0/rc3/`, which is where the whole file's
pinned `sha256` and byte length live. `<start>-<end>` is the citation's own
half-open byte range. The path is derived from the citation's fields, so a row
and its slice cannot be named inconsistently.

Two slices may hash to the same value — `@solidjs/signals`' `import` default
and `require` bundles are byte-identical for `createRoot`, `getOwner` and
`onCleanup` — and are still stored twice, once per file, because the claim is
about each file.

## Provenance and licence

These bytes are from `@solidjs/signals@2.0.0-rc.3`
(`sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`),
published by the SolidJS project under the MIT licence. They are reproduced
here as citation evidence only. Do not edit a slice: it is not source, it is a
quotation, and the test that reads it exists to detect exactly such an edit.

Adding a slice is a step of adding an `AuditedCitation::Implementation`, and
both belong in the same commit as the audit section they cite.
