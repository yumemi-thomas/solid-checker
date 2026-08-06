# solid-checker

Project-level reactivity checker and language server for Solid. The package
ships native `solid-checker` and `solid-typefacts` executables
for supported platforms.

Install it as a development dependency:

```sh
npm install --save-dev solid-checker
```

Then run `solid-checker --certify`.

Library maintainers can generate an inferred contract for every exact and
wildcard package export without writing JSON:

```sh
solid-checker contract generate --package-root .
```

Application developers can generate the same contract into their local
override directory:

```sh
solid-checker contract generate \
  --package-root node_modules/solid-dnd \
  --output .solid-checker/contracts/solid-dnd/solid-reactivity.json
```

Use `--conditions browser,import` for a specific conditional export
environment. Generation uses implementation facts plus published declaration
call signatures, merges compatible conditional targets conservatively, and
does not execute package code. Generated contracts deduplicate effect summaries
and identical subpath surfaces while the checker expands them internally for
analysis.

The CLI uses the Oxc graphical reporter for framed terminal diagnostics:

```sh
solid-checker --project tsconfig.json
```

The `default` format prints the same style of source frames, severity markers,
evidence labels, and error summary used by Oxlint. Use `--format json` for
machine-readable findings or `--format text` for compact output.

Optimized macOS and Linux checks retain a per-project TypeScript and analysis
session for up to two idle minutes, so repeated invocations reuse coherent
facts instead of rebuilding the project. Set `SOLID_CHECKER_DAEMON=0` to force
a one-shot check or `SOLID_CHECKER_DAEMON_IDLE_SECS=<seconds>` to change the
idle lifetime. The default process-tree resident-memory ceiling is 2048 MiB;
change it with `SOLID_CHECKER_DAEMON_MAX_RSS_MB=<MiB>` (`0` disables it).
`SOLID_CHECKER_TIMINGS=1` writes cache-hit, generation, latency, and payload
telemetry as JSON on stderr.

Projects with at least 1,000 source files automatically use balanced cache
retention: the current result stays hot, while the largest derived indexes are
released between changed generations. Override it with
`SOLID_CHECKER_CACHE_RETENTION=performance`, `balanced`, or `compact`.

To report project findings through Oxlint, load the bundled JavaScript adapter:

```json
{
  "jsPlugins": ["solid-checker/eslint"],
  "rules": {
    "solid-checker/certification": "error"
  }
}
```

The adapter discovers the nearest `tsconfig.json`, runs native project analysis
once, caches its snapshot, and projects matching findings into Oxlint. Set
`settings.solidChecker.project` when the project uses a nonstandard config name
or a solution-style root config that only references application configs.

Per-rule options (for example `v1/no-innerhtml`'s `allowStatic`) live in the
project's `.solid-checker/rule-options.json`, which the native analysis
discovers itself — not in ESLint rule configuration. The adapter runs one
analysis per project, so a single discovered file is what keeps ESLint, the
standalone CLI, and every editor integration reading the same options. See
`docs/rules/README.md` in the repository for the format.

In StackBlitz, WebContainers, or a browser worker, import the process-free
WASM API from the same package:

```js
import { checkSync } from "solid-checker";
```

Supported targets are Linux (x64 and arm64), macOS (x64 and arm64), and
Windows (x64). npm installs only the matching
`@solid-checker/binding-<target>` optional dependency; the portable package
contains the launchers. The launcher forwards arguments, stdio, signals, and
exit status. While running from this monorepo, it builds missing development
binaries with `make build-rust`.
