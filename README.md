# solid-checker

`solid-checker` catches [Solid](https://docs.solidjs.com) runtime bugs before they
ship. Your code can compile, type-check, and still misbehave at runtime — these
failures are invisible to the TypeScript compiler:

- **UI that silently goes stale** — a signal read that never registered a
  dependency (an untracked read, a read after `await`, a destructured prop), so
  the computation never re-runs.
- **Feedback loops** — a signal write or action fired inside a tracked scope,
  corrupting the update graph or looping forever.
- **Async that escapes its boundary** — a pending async read outside a
  suspendable region, or async work rendered without a `Loading` boundary.
- **Leaks** — effects, cleanups, and boundaries created with no owner, so they
  are never disposed.

`solid-checker` analyzes your whole TypeScript project, proves where these bugs
happen, and reports each one with the evidence and a fix hint.

## How it works

Solid's runtime has precise rules: tracking is synchronous, props stay live,
writes are forbidden in tracked scopes, effects and cleanups must run under an
owner. No single tool can check those rules from source text alone, so
`solid-checker` cross-references four sources of evidence:

- **Syntax** — the real parse tree of your code (Oxc).
- **Compiler semantics** — how the Solid compiler will actually execute your
  JSX: which scopes are tracked, where boundaries are, what runs once vs. on
  every update.
- **Type facts** — what TypeScript knows about every symbol: where an accessor
  came from, which calls a given `await` dominates, what a function returns.
- **Dependency source** — selected installed runtime files analyzed alongside
  project code; unavailable behavior stays uncertifiable.

Combining them lets the analyzer prove defects rather than pattern-match
risky-looking syntax. Every finding you see carries a stable code (`SCxxxx`),
the evidence that proves it, and a fix hint:

- **violation** — the analyzer proved the code misbehaves at runtime.
- **notice** — the analysis did not apply to this project as stated: the
  installed Solid runtime is unsupported, or its release is unaudited.

Code the analyzer could neither prove correct nor prove wrong is not reported:
those *uncertifiable* results are an internal measure of what the analysis
cannot decide yet (`--format json` and `--internal` show them;
[ADR 0328](docs/decisions.md#adr-0328)).

Reports state their runtime premises. The [standard runtime configuration
premise](docs/decisions.md#adr-0266), amended by
ADR 0268, assumes Solid's standard configuration and hydration host. Dependency
source mode also uses the [package integrity premise](docs/decisions.md#adr-0300):
code outside the analyzed program neither provides selected packages' exported
contexts nor rewrites objects those packages create. Visible providers and
member or descriptor writes still withhold a proof. These assumptions are
conditions of certification; an absent visible veto does not establish them.

For example, `SC1002` [reactive-read-after-await](docs/rules/reactive-read-after-await.md):

```tsx
const profile = createMemo(async () => {
  const posts = await fetchPosts();
  // Tracking ended at the await: changing userId() never re-runs this memo.
  return posts.filter((post) => post.author === userId());
});
```

The rules cover tracking and component semantics, writes and actions, cleanup
and ownership, async boundaries, directives, and API shapes. See the
[full rule index](docs/rules/README.md) for every code with examples and fixes.

## Quick start

```sh
bun add --dev solid-checker
bunx --bun --no-install solid-checker --project tsconfig.json
```

Diagnostics print as framed source excerpts with severity markers, evidence
labels, and a fix hint. In CI, add `--certify` to fail the build on any
violation or project notice:

```sh
bunx --bun --no-install solid-checker --project tsconfig.json --certify
```

Linux (x64, arm64), macOS (arm64), and Windows (x64) are supported; Bun
downloads only the binary matching your platform.

## Solid compatibility

**This build analyzes Solid 2.0 only.**

| | Solid 2.0 |
| --- | --- |
| Audited runtime | `solid-js@2.0.0-rc.17`, `@solidjs/signals@2.0.0-rc.17` and `@solidjs/web@2.0.0-rc.17` |
| Dialect id | `solid-v2` |
| Rule names | Unprefixed |
| Catalog | 32 rules |
| Effect model | `createEffect(compute, apply)` |
| Async boundary | `Loading` / async computations |
| Lifecycle | `onSettled`; leaf cleanup returned from callbacks |
| Directives | `ref` directive factories |
| Props helpers | `merge` / `omit` |

Solid 1.x was retired in 2026-09 ([ADR 0110](docs/decisions.md#adr-0110)).
A project whose resolved `solid-js` is a major this build has no vocabulary for
is **refused outright** — one `SC9013 unsupported-solid-runtime` result naming
the `package.json` that decided it, and no other findings — rather than analyzed
under a language it does not run. An absent or unclassifiable install still gets
the Solid 2.0 default, because that is an absence rather than a contradicted
answer.

The checker detects the installed `solid-js` major automatically. Use
`--dialect solid-v2` only when a package manager or fixture prevents version
discovery; passing it also overrides the refusal above, so it is appropriate
only when the resolved manifest misreports what will actually be installed.
Solid 2.0 is still a release candidate; release-dependent vocabulary is
reviewed against exact audited versions. Unreviewed behavior stays open.

See [the workspace architecture](rust/ARCHITECTURE.md#version-ownership-at-a-glance)
for where version-specific code belongs and [the rule index](docs/rules/README.md)
for the two catalogs.

On macOS and Linux, optimized release checks retain one project actor for up
to two idle minutes. It owns the TypeScript program and analysis caches,
resynchronizes source and runtime inputs before every answer, and makes
repeated CLI and editor checks incremental. Set `SOLID_CHECKER_DAEMON=0` for a
strict one-shot process, or `SOLID_CHECKER_DAEMON_IDLE_SECS=<seconds>` to tune
the idle lifetime. The actor and its Type Facts child are evicted when their
combined resident memory exceeds 15% of physical memory (at least 2 GiB, at
most 8 GiB); tune that with
`SOLID_CHECKER_DAEMON_MAX_RSS_MB=<MiB>`, or set it to `0` to disable the memory
ceiling. `SOLID_CHECKER_TIMINGS=1` emits retained cache-hit, generation,
analysis, round-trip, and payload measurements as JSON on stderr. Debug builds
remain one-shot unless `SOLID_CHECKER_DAEMON=1` is set explicitly.

For projects with at least 1,000 source files, the retained actor releases its
largest derived Reactive IR indexes after each materialized answer while
keeping the current coherent result. Set
`SOLID_CHECKER_CACHE_RETENTION=performance`, `balanced`, or `compact` to
override that automatic policy. `performance` keeps every edit cache;
`balanced` is the large-project default; `compact` keeps only the current
result and rebuilds all derived indexes after an edit.

## Use it with ESLint or Oxlint

The same plugin, `solid-checker/eslint`, works in both linters. It runs the
project analysis once per lint run and reports the findings — including safe
autofixes — through your existing lint pipeline.

With ESLint (flat config):

```js
// eslint.config.js
import solidChecker from "solid-checker/eslint";

export default [solidChecker.configs.recommended];
```

The `prefer-*` rules are enabled by default. Set an individual ESLint rule to
`"off"`, or set `enabled: false` in `.solid-checker/rule-options.json` for
standalone and certification-mode checks. The legacy dialect-specific
`preferences-v1` and `preferences-v2` configs remain composable but are now
redundant.

With Oxlint:

```json
// .oxlintrc.json
{
  "jsPlugins": ["solid-checker/eslint"],
  "rules": {
    "solid-checker/certification": "error",
    "solid-checker/package-note": "warn"
  }
}
```

The plugin finds the nearest `tsconfig.json` automatically (in ESLint it also
reuses `parserOptions.project`). Set `settings.solidChecker.project` if your
config has a nonstandard name or is a solution-style root config. Removed
contract settings are refused with a migration message.

> The plugin analyzes the project once per lint run and reports from that
> snapshot, so it fits lint commands, editor-on-save, and CI.

## CLI options

Run `solid-checker --help` for the full list. The options you'll reach for most:

| Option | Description |
| --- | --- |
| `--project <PATH>` | TypeScript project to analyze (default: `tsconfig.json`). |
| `--dialect solid-v2` | Override automatic Solid major-version detection. Also overrides the unsupported-runtime refusal. |
| `--format <default\|full\|text\|json>` | Output format. `default` prints violations, then project notices, as framed source excerpts. `full` frames every shown finding in place, `text` is compact, `json` is the complete machine-readable report, uncertifiable results included. |
| `--certify` | Exit non-zero on any violation or project notice. Use this in CI. |
| `--internal` | Maintainer view: the human formats also show uncertifiable results (findings to review, then analysis-coverage gaps), and `--certify` fails unless the project is fully certified. |
| `--preset <NAME>` | Enable a catalog preset (repeatable; the compatibility `preferences` preset is currently available). |
| `--enable-rule <NAME>` | Explicitly enable one rule (repeatable). |
| `--runtime-target <browser\|node>`, `--runtime-build <development\|production>` | Declare the runtime host for dependency source selection under Vite with the Solid plugin. Unavailable selection keeps explicit uncertifiable package obligations. |
| `--dependency-source <SPECIFIER>` | Analyze only the named installed packages from source (repeatable; `*` is every Solid package, the default under a declared target and build). Requires both host flags. |
| `-h`, `--help` | Print help. |

## Dependencies

Package behavior is analyzed from installed runtime source under the declared
host and source-selection premises. An unavailable or unsupported package
boundary stays explicitly uncertifiable (SC9005). Contract generation,
certification, catalogs and receipts have been removed; see
[ADR 0312](docs/decisions.md#adr-0312).

## WASM

In StackBlitz, WebContainers, or a browser worker — anywhere a native process
can't be spawned — import the process-free WASM API from the same package:

```js
import { checkSync } from "solid-checker";
```

## Documentation

- [Rule index](docs/rules/README.md) — every diagnostic, with examples and fixes
- [Documentation index](docs/README.md) — decisions, open work, protocols
- [Contributing](CONTRIBUTING.md) — building and developing solid-checker
