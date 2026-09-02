# A custom export condition naming a target the tarball deliberately omits

This is the published shape of `@solid-devtools/debugger@0.28.1`: a private
condition (`"@solid-devtools/source"`) points at `./src/index.ts` while `files`
ships only `dist`, so the source tree is absent from the tarball on purpose. The
condition census enumerates every custom condition it finds, so the partition
that activates one selects a target the artifact does not contain.

`./source` pins that case. The partition that activates
`unpublished-conditional-target/source` selects `./src/source.ts`, which is not
published. That is **not** a refusal: the artifact itself proves the target
unpublished, and no consumer reaches it without opting into a private condition
name, so there is nothing about certifiable behavior to assert. It is recorded
in the refusal sidecar's `inapplicable` array as
`unpublished-conditional-target`, and the sibling `./source` case under the
empty partition still certifies `track`.

`./browser-gap` pins the boundary that keeps refusal semantics. Its missing
target sits behind `browser`, a standard environment condition every browser
consumer activates, so a real consumer really does fail there — a defective
publish. That case stays an ordinary `target-not-found` refusal with
`unavailable-published-target` applicability.

The `.` entrypoint has no conditional arm at all and certifies under every
partition; the "`.` under the empty/default partition" boundary — a missing
target reached through standard conditions only — is pinned by the generator
unit test `a fully refused proposal writes every artifact-case refusal before
throwing`, where `{".": "./missing.js"}` stays a full refusal with an empty
`inapplicable` census.

`wildcard-asset-entrypoints` pins the other inapplicable class.
