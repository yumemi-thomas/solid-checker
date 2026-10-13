# ADR 0102: Pinned images are hashed once and re-asserted by fingerprint

- Status: accepted and implemented (2026-09-14); written with the
  implementation
- Date: 2026-09-14
- Owners: probe harness (`probe_harness.rs` `WatchedInput::PinnedImage`,
  `verify_node_executable`, `watch_digests`), Type Facts
  (`type_facts.rs` `shared_from_pin`), the shared memo
  (`contract_certification/pinned_bytes.rs`)
- Relation: narrows what docs/precision-backlog.md § "What was measured and
  left alone" (2026-09-06) named as "a design decision for an ADR, not a
  performance patch". The census labels, the policy digest and every refusal
  direction are unchanged; what changes is how the bytes behind three labels
  are re-read. Handshake protocol stays 56.

## Context

The watched-input census of a probe-gate batch re-hashes every watched path
before the first launch, between launches, and on every exit path. Three of
those paths are pinned images outside every private tree: the Node executable
(117 MB), the verifier's own image (25 MB) and the Type Facts producer image
(29 MB). The Type Facts side separately re-hashes the shared producer image
before every producer launch.

Measured on 2026-09-14 on `solid-js@1.9.14`, isolated, release binary: 2,190
censuses over 292 batches; of about 230 CPU-seconds of census label work,
122 s were the Node executable and 59 s the two other images. Corpus-wide the
same three labels are the largest single CPU line of the certified run, and
every one of those reads returned the digest the build had pinned.

The census exists to prove that one session did not tamper with what the next
one reads. For the private tree that proof needs the bytes: the worker writes
there by design (`TMPDIR`, `HOME`, the cwd). For the three pinned images it
needs only that the bytes did not change, and an unprivileged worker cannot
change a file's bytes without moving its ctime, nor replace it without
changing its inode.

## Decision

1. A pinned image is a distinct census input, `WatchedInput::PinnedImage`.
   Its digest is taken from the bytes once per process and served from a
   process-wide memo while the file's device, inode, size, mtime and ctime are
   the ones the bytes were hashed under (`pinned_bytes::fingerprinted_digest`).
   Fingerprint and bytes are read from the same open descriptor, so a path
   swapped between the two is not a window; a fingerprint that moved during
   the read is not memoized.
2. `verify_node_executable` and the shared producer image check in
   `shared_from_pin` use the same memo, so a transaction hashes each pinned
   image once rather than once per batch and once per acquisition.
3. The census labels (`node-executable`, `type-facts-image`,
   `verifier-image`), their order, the pin comparison of `node-executable`
   against the build's configured digest on every census, and every refusal
   message are unchanged. A rewritten or replaced image still produces a fresh
   digest and refuses exactly as before; the harness tests that swap the Node
   stand-in mid-run pin this against the new variant.

## What this gives up

A writer that can rewrite bytes *and* restore ctime — root, or control of the
system clock — could substitute an image without a re-hash. The pinned images
are outside every private tree, the worker runs as the certifying user without
that privilege, and the same writer could already replace the verifier that
does the hashing. The residual is the host's, not the probe's, and it is
stated here rather than in a comment so a later reader does not mistake the
memo for a correctness shortcut.

## Extended the same day to the private trees

The first cut kept the private `recipe-modules` and `private-node-modules`
trees on bytes, because the worker can write there. Measured on `corvu@0.7.2`
(616 graph nodes, 247 batches, 2,556 sessions, 3,050 censuses) that left the
census as 60% of all batch time, and fourteen batches side by side re-reading
their private trees between every session thrashed the disk: summed batch time
1,332 s for 107 s of gate wall.

The fingerprint argument does not depend on who writes. A file's ctime moves on
every write and cannot be set back by an unprivileged process, and a replaced
file is a new inode; the worker runs as the certifying user with no more
privilege than that. So the tree census (`collect_tree`) now takes each file's
digest through the same fingerprint memo: a census between sessions is a stat
walk that re-hashes only files that moved, and additions and removals remain
the walk's own detection. What a census proves is unchanged — every regular
file under the tree, by root-relative path, digest and length — and every
refusal direction is unchanged. The memo is evicted when a workspace is
removed, so it tracks live files only.

Two other changes landed beside this ADR without changing what is proved: only
the recipes a batch schedules are copied into its workspace (the corpus is
still loaded and identified whole, and its manifest and modules are read
through the same fingerprint memo), and the `private-dependency:*` digests are
derived from the single `private-node-modules` walk rather than read a second
time — the derived digest is byte-for-byte `hash_tree` of the copy.
