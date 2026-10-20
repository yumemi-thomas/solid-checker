# Real lockfiles, trimmed

Lockfiles that real Solid 2 applications commit, trimmed to a few entries so the
lockfile readers are tested on the shapes package managers write rather than
on hand-built approximations. Each app is pinned in
`scripts/ecosystem-benchmark/app-import-corpus.json`. Every line that was kept
is byte-for-byte the original's. Trimming removed whole `packages`/`snapshots`
entries, importer dependencies and workspace dependency lines, and nothing
else. A trimmed entry can therefore name a dependency whose own entry was
removed. The readers never follow those references.

| file | source (repository @ commit, path) | original sha256 | shape |
| --- | --- | --- | --- |
| `finds-team.pnpm-lock.yaml` | `moreal/finds.team` @ `3c223e92`, `pnpm-lock.yaml` | `076bcc5b…a36f` | pnpm 12.5.1: an env document (`packageManagerDependencies: pnpm`), then the project document with `patchedDependencies` |
| `readingroom.pnpm-lock.yaml` | `Sleeping-Donut/readingroom` @ `d53aa31e`, `frontend/pnpm-lock.yaml` | `8ce1080a…dc97` | pnpm 11.17.0: an env document whose `packages` holds `detect-libc@2.1.2`, which the project document also holds, with the same integrity |
| `civil.bun.lock` | `civilnetwork-dev/Civil` @ `3a081157`, `bun.lock` | `0c7a099f…50f4` | `bun.lock` `lockfileVersion: 1`, `configVersion: 1`: `@solidjs/meta` hoisted at `1.0.0-next.2` and nested under `@tanstack/solid-router` at `0.29.4`, plus `patchedDependencies` |

The negative cases are built in the tests from these bytes, each with one
change: a third document, a moved marker, an env document that names a project
importer, a conflicting duplicate key, an unknown `lockfileVersion`, a missing
or non-SRI integrity.
