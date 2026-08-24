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

Check which dependencies still need one, and which have a contract that no
longer matches the installed version:

```sh
solid-checker contract check
```

Packages are reported as `bundled`, `published`, `local`, `explicit`,
`unverified`, `stale`, `unbound`, or `missing`; every status that cannot certify
prints the command that resolves it, and the command exits non-zero when any
package needs action. `unbound` is a usable contract that describes no import in
the project, because every specifier carrying its name resolves somewhere the
contract's package is not. `stale` means the contract was generated against a different
version of the package than the installed one — regenerate and re-review it
after an upgrade.

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

By default the analysis picks its dialect from the `solid-js` version the
project resolves. Set `settings.solidChecker.dialect` to `"solid-v1"` or
`"solid-v2"` to override detection for every rule the adapter runs.
When package contracts or rendering proofs depend on deployment conditions,
set `settings.solidChecker.runtime` with explicit `target`, `build`,
`rendering`, `conditions`, and `frameworkTransforms` fields. Incomplete or
contradictory selections remain uncertifiable; the adapter includes the full
selection in its analysis cache identity.

Every catalog rule is also its own ESLint rule, so a project can disable one
finding without losing the rest: unprefixed names
(`solid-checker/strict-read-untracked`) come from the Solid 2.0 catalog, and
`v1/`-prefixed names (`solid-checker/v1/no-destructure`) come from the Solid
1.x catalog. A `v1/` rule analyzes with the 1.x dialect on its own when the
configuration has not chosen one. All rules of one dialect share a single
cached analysis run, so enabling an entire catalog still spawns the checker
once per project.

The plugin ships three flat configs. `configs.recommended` enables only
`solid-checker/certification`, which reports every finding through one rule.
`configs.v1` and `configs.v2` enable their catalog's rules at each rule's
native severity and turn `certification` off. The configs compose in either
order: each finding reports exactly once, per rule. Even when a listing such
as `[configs.v1, configs.recommended]` re-enables `certification` (flat
config resolves each rule from the later entry), certification skips every
finding an enabled per-rule rule owns for the linted file and reports only
the rest.

The adapter discovers shipped dialect catalogs by enumerating
`lib/rules-solid-vN.json`. Each generated catalog carries its stable `dialect`
id, compatibility `config` key, and optional rule `namespace`; adding a catalog
does not require a JavaScript registry or version branch.

Project-wide rule enablement and per-rule options (for example
`v1/prefer-classlist`'s `classnames`) live in the project's
`.solid-checker/rule-options.json`, which the native analysis
discovers itself — not in ESLint rule configuration. The adapter runs one
analysis per project, so a single discovered file is what keeps ESLint, the
standalone CLI, and every editor integration reading the same options. See
`docs/rules/README.md` in the repository for the format.

In StackBlitz, WebContainers, or a browser worker, import the process-free
WASM API from the same package:

```js
import { checkSync } from "solid-checker";
```

Supported targets are Linux (x64 and arm64), macOS (arm64), and
Windows (x64). macOS on Intel is not published; build from a checkout with
`make build-rust` to run there. npm installs only the matching
`@solid-checker/binding-<target>` optional dependency; the portable package
contains the launchers. The launcher forwards arguments, stdio, signals, and
exit status. While running from this monorepo, it builds missing development
binaries with `make build-rust`.
