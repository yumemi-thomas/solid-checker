# 0021 — Materialize graph-authenticated dependencies in the probe workspace

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

After independent census composition succeeds, Kobalte's gate refuses because
the probe workspace has no authenticated snapshot for
`@solid-primitives/event-listener`. The graph transaction authenticated it, but
the harness receives only the parent plan's compiler-source snapshots. Graph
dependency plans are not forwarded to materialization.

Pass the graph's exact transitive dependency plans to the existing private-copy
materializer after dependency receipt authentication. Merge their snapshots
and compiler-source snapshots with the parent's authenticated source set.
Deduplicate equal snapshot roots, refuse conflicting roots for any package
name, and never read installed node_modules. A same-package subpath may reuse
the already copied root only when its snapshot root agrees exactly. Check each
forwarded graph plan's runtime dependency edges against the resulting set.
Replay every static graph dependency's selected runtime target against the
conditions observed from the pinned Node as well as checking the root case.
An extra Node condition selecting a different dependency target must refuse.

The workspace remains 0700, uses env_clear plus the existing allowlist, and
retains process-group cleanup, single startup/run frames, primordial capture,
prototype freezing, exact reported resolution and detect-and-refuse isolation.
No source transformation or runtime substitution is introduced.

Bind the canonical package-name/snapshot-root materialization manifest into
the probe receipt root. Add the policy field
`snapshot:dependency-materialization-manifest-bound-to-probe-root` and advance
sandbox scheme 6 to 7. The new field makes the actual copied dependency set
explicit in probe identity, including graph inputs; the accepted graph and
source roots continue to authenticate where those snapshots came from.

Copying installed dependencies or trusting caller paths is rejected. Ignoring
the missing edge would run a partial closure. Forwarding already authenticated
snapshots through the same copying, collision and watch rules preserves the
published-byte claim and the missing-dependency refusal.
