# Pinned `solid-js@1.9.14` archive

Written by `scripts/audit-solid-1x.mjs` from the npm registry document and the
tarball it names, after verifying the tarball's SRI and SHA-1 against that
document and the extracted file count and unpacked size against `dist`. The
`package.json` here is the archive's own bytes; its `sha256`
(`52ee61ea826e59a5ec36f6ce91bafb7d38bf99ab04b3a6217993fb17af6037c0`) and the
tarball's `integrity`
(`sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==`)
are the `AuditedArchive` tuple in `rust/crates/solid-dialect/src/solid_1x.rs`,
and they equal the `package` block of every `pkg/contracts/bundled/solid-v1/*.json`
document.

`files.json` is the per-file manifest the citation test in `solid_1x.rs` reads:
each `AuditedCitation::Implementation` names a path here, its `sha256`, and a
byte range inside its `bytes`. The cited ranges themselves are checked in under
`rust/crates/solid-dialect/audited-slices/solid-v1/`.
