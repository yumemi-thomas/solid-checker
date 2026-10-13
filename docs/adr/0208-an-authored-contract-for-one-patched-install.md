# ADR 0208: An authored contract for one patched install

- Status: accepted and implemented (2026-10-06). Extends ADR 0198 (the
  authored tier) and ADR 0207.
- Owners:
  - `AuthoredEntry::patched_install` and the authored identity
    (`solid-facts-backend/src/authored_contracts.rs`);
  - `ArtifactAcceptance::patched_install` and the installed-bytes check
    (`accepted_bundles.rs`, `installed_artifact_bytes` in `diagnostics.rs`);
  - `patchedInstall` specs and the probe's patch guard
    (`scripts/author-contracts.mjs`).

## Context

ADR 0198 admits an authored contract only where "the installed files
reproduce the snapshot root, and nothing is patched". A certified receipt is
about the published archive, so a patched install is another artifact, and
refusing it is right.

`openbot` patches `@kobalte/core@2.0.0-alpha.0` (`bun.lock`
`patchedDependencies`) to adapt it to Solid 2's release candidates. Its ~31
`Button` sites are ADR 0207's main target. They stayed uncertifiable even
though its patch leaves the claim's behaviour alone.

A contract can be honest about patched bytes when it is about exactly those
bytes: the claim probed on them, and admission keyed to them.

## Decision

1. **An authored entry may state `patchedInstall: true`.** Its
   `snapshotRoot` is then the root of a patched install's files, the files
   its claims were probed on.
2. **Admission compares that root despite a recorded patch.** For such an
   entry only, a patch the consumer's tree records for the package does not
   refuse the bytes comparison. The root must still match byte for byte, so
   only an install with exactly those files is admitted; another patch, or
   the published files, is not.
3. **Everything else is unchanged.**
   - Certified acceptances and unpatched authored entries still refuse a
     patched package.
   - The Solid runtime environment keeps its patch check.
4. **A patched entry is its own artifact.** Its identity includes its
   snapshot root (`authored-patched:<root>:…`), so it never collides with
   the published entry of the same version and case.
5. **The tier's specs name the patch.**
   - A spec `<package>@<version>+<label>` states `patchedInstall`:
     - `label`;
     - the patch file and its `sha256`;
     - `why`: a review of what the patch changes in the code the claim is
       about.
   - Its probe runs only on an install that applies exactly that patch file.
   - A spec about the published bytes refuses a probe install that records
     any patch for the package. The first ADR 0207 probe had run on
     openbot's patched files; this guard would have stopped it.

## Consequences

- A project's own patch can carry an authored contract, at the price of one
  spec per patch, probed on that project's install.
- A patched install with any other bytes gets nothing, as before.
- **Corpus repair.** The rc.13 corpus upgrade left openbot's `bun.lock`
  recording `@solidjs/signals@2.0.0-rc.13` and `@solidjs/web@2.0.0-rc.13` as
  patched with rc.0 patch files. Their installed files are byte-identical to
  an unpatched rc.13 install (compared against probus-hk's), so bun never
  applied them. Those two records were removed from the corpus copy. The
  copy before the change is `bun.lock.before-patch-fix`.

## Evidence

- **Unit test** `a_patched_install_is_admitted_only_by_an_entry_probed_on_it`:
  - a tree that records a patch is refused for an entry about the published
    bytes;
  - it is admitted for the entry probed on it;
  - it is refused when its root differs.
- **Spec** `@kobalte+core@2.0.0-alpha.0+openbot-patch`:
  - The patch's only change in the button chunk reads `mergedProps.ref`
    inside `untrack`. `Polymorphic` and `Dynamic` handle every other prop as
    in the published code.
  - The pairs pass in Chrome on openbot's install.
- **openbot** (rc.13 corpus, browser host):
  - 37 uncertifiable `SC1001` reads and 2 `SC9012` obligations leave; no
    violation moves.
  - The reads are `onClick` literals of:
    - `Button` (29), `IconButton` (2) and `SettingsLinkRow` (1);
    - three tags a text scan of the source could not name.

    Two more are props that a project component calls from inside a
    `Button` `onClick` literal: `AttachmentDownloadAll`'s `onDownload` and
    `ChatMessageRow`'s `onRemoveReaction`.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-j-browser.json`:
  - violations unchanged at 285 (none added, none removed);
  - uncertifiable 3,421 to 3,382: 40 removed and 1 moved, all in openbot.
    The moved one is `ui/button.tsx`'s contract message, which now names the
    domains the claim leaves open.
