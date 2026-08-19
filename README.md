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
- **Package contracts** — the declared reactive behavior of your dependencies'
  exports, so analysis doesn't stop at `node_modules`.

Combining them lets the analyzer **certify** the project rather than
pattern-match risky-looking syntax. Every finding carries a stable code
(`SCxxxx`), the evidence that proves it, and a fix hint, and is one of two
kinds:

- **violation** — the analyzer proved the code misbehaves at runtime.
- **uncertifiable** — the analyzer could not prove the code correct, and the
  rule page explains how to make it provable.

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
npm install --save-dev solid-checker
npx solid-checker --project tsconfig.json
```

Diagnostics print as framed source excerpts with severity markers, evidence
labels, and a fix hint. In CI, add `--certify` to fail the build unless the
project is fully certified:

```sh
npx solid-checker --project tsconfig.json --certify
```

Linux (x64, arm64), macOS (x64, arm64), and Windows (x64) are supported; npm
downloads only the binary matching your platform.

## Solid compatibility

The release treats Solid 1.x and Solid 2.0 as separate dialects over one shared
analysis engine:

| | Solid 1.x | Solid 2.0 |
| --- | --- | --- |
| Audited runtime | `solid-js@1.9.14` | `solid-js@2.0.0-rc.0` and `@solidjs/web@2.0.0-rc.0` |
| Dialect id | `solid-v1` | `solid-v2` |
| Rule names | `v1/<rule>` | Unprefixed |
| Effect model | `createEffect(fn, initialValue?, options?)` | `createEffect(compute, apply)` |
| Async boundary | `Suspense` / `createResource` | `Loading` / async computations |
| Lifecycle | `onMount`; cleanup via `onCleanup` | `onSettled`; leaf cleanup returned from callbacks |
| Directives | `use:` | `ref` directive factories |
| Props helpers | `mergeProps` / `splitProps` | `merge` / `omit` |

The checker detects the installed `solid-js` major automatically. Use
`--dialect solid-v1` or `--dialect solid-v2` only when a package manager or
fixture prevents version discovery. Solid 2.0 is still a release candidate, so
its bundled contracts intentionally require the exact audited RC; a later RC
must be reviewed before it can certify a project.

See [the workspace architecture](rust/ARCHITECTURE.md#version-ownership-at-a-glance)
for where version-specific code belongs and [the rule index](docs/rules/README.md)
for the two catalogs.

On macOS and Linux, optimized release checks retain one project actor for up
to two idle minutes. It owns the TypeScript program and analysis caches,
resynchronizes source and contract inputs before every answer, and makes
repeated CLI and editor checks incremental. Set `SOLID_CHECKER_DAEMON=0` for a
strict one-shot process, or `SOLID_CHECKER_DAEMON_IDLE_SECS=<seconds>` to tune
the idle lifetime. The actor and its Type Facts child are evicted when their
combined resident memory exceeds 2048 MiB; tune that with
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

With Oxlint:

```json
// .oxlintrc.json
{
  "jsPlugins": ["solid-checker/eslint"],
  "rules": {
    "solid-checker/certification": "error"
  }
}
```

The plugin finds the nearest `tsconfig.json` automatically (in ESLint it also
reuses `parserOptions.project`). Set `settings.solidChecker.project` if your
config has a nonstandard name or is a solution-style root config.

> The plugin analyzes the project once per lint run and reports from that
> snapshot, so it fits lint commands, editor-on-save, and CI.

## CLI options

Run `solid-checker --help` for the full list. The options you'll reach for most:

| Option | Description |
| --- | --- |
| `--project <PATH>` | TypeScript project to analyze (default: `tsconfig.json`). |
| `--dialect <solid-v1\|solid-v2>` | Override automatic Solid major-version detection. |
| `--format <default\|text\|json>` | Output format. `default` prints framed source excerpts, `text` is compact, `json` is machine-readable. |
| `--certify` | Exit non-zero unless the project is fully certified. Use this in CI. |
| `--check-contracts` | Report imported Solid packages whose reactivity contract is missing, unverified, or stale, with the command that fixes each. Also spelled `solid-checker contract check`. |
| `-h`, `--help` | Print help. |

Authoring a package contract (see [Publishing a Solid library?](#publishing-a-solid-library)):

| Option | Description |
| --- | --- |
| `--emit-contract <PATH>` | Write an inferred `solid-reactivity.json` contract candidate. |
| `--package-name <NAME>` | Package name recorded in the emitted contract. |
| `--package-version <VERSION>` | Exact package version recorded in the contract. |
| `--declaration-artifact <PATH>` | Hash a declaration artifact into the contract. |
| `--implementation-artifact <PATH>` | Hash an implementation artifact into the contract. |
| `--contract <PATH>` | Override or discover a package contract (repeatable). |
| `--validate-contract <PATH>` | Validate a contract and its artifact hashes. |

## Using a library that ships no contract

`solid-checker` needs to know how a dependency's exports read reactive values.
When that dependency's source isn't part of your project, it relies on a
`solid-reactivity.json` **contract**. If an imported Solid-dependent package
ships none, the check reports the uncertifiable `SC9005 package-contract-missing`
finding and `--certify` fails.

List which of your dependencies are missing a contract, and which have one
that no longer matches the installed version:

```sh
solid-checker contract check
```

Each package is reported as `bundled`, `published`, `local`, `explicit`,
`unverified`, `stale`, or `missing`, and every status that cannot certify
prints the command that resolves it. The command exits non-zero when any
package needs action, so it also works as a CI gate. A `stale` contract is one
generated against a different version of the package than the one installed —
after an upgrade, regenerate and re-review it. See
[docs/package-contracts.md](docs/package-contracts.md#checking-contract-coverage-and-freshness).

You don't have to wait for the maintainer. You can supply the contract yourself
and `solid-checker` will pick it up automatically from
`.solid-checker/contracts/<package>/solid-reactivity.json` (scoped names keep their
directory, e.g. `.solid-checker/contracts/@scope/pkg/solid-reactivity.json`):

- **Generate it** from the package's source, if you have it checked out with a
  TypeScript project, using the same `--emit-contract` workflow below.
- **Author it by hand** against the
  [contract schema](schema/solid-reactivity.schema.json) and check it with
  `solid-checker --validate-contract <path>`.

A one-off `--contract <path>` on the command line takes precedence over a
discovered contract. See [package contracts](docs/package-contracts.md) for the
full workflow and trust boundary.

## Publishing a Solid library?

Generate every runtime entrypoint directly from the package export map:

```sh
solid-checker contract generate --package-root .
```

This derives the package name and exact version from `package.json`, expands
exact and wildcard export subpaths, analyzes each ESM target independently, and
writes `solid-reactivity.json`. Use `--conditions browser,import` to select one
conditional export environment; without it, all supported ESM variants must
produce compatible summaries. The generated document deduplicates effect
summaries and identical subpath surfaces, so large packages do not repeat the
same export metadata hundreds of times.

Build a `solid-reactivity.json` contract candidate describing the reactive
behavior of your exports:

```sh
solid-checker --project tsconfig.json \
  --emit-contract solid-reactivity.json \
  --package-name my-package \
  --package-version 1.0.0
```

Emission is fail-closed when a parameter escapes into an uncontracted external
call (including unresolved callback execution), and records
`inferred` evidence. Inferred contracts are deliberately not sufficient for
consumer certification: verify them against the exact package artifacts and
behavior, then record `verified`, `reviewed`, or `attested` evidence. Contracts
are entrypoint- and exact-version-aware, so include every published subpath.

Published at your package root as `solid-reactivity.json`, it's discovered
automatically from `node_modules`. See
[package contracts](docs/package-contracts.md) for hashing artifacts and the
trust boundary.

Consumers can run the same generator without modifying the installed package:

```sh
solid-checker contract generate \
  --package-root node_modules/solid-dnd \
  --output .solid-checker/contracts/solid-dnd/solid-reactivity.json
```

## WASM

In StackBlitz, WebContainers, or a browser worker — anywhere a native process
can't be spawned — import the process-free WASM API from the same package:

```js
import { checkSync } from "solid-checker";
```

## Documentation

- [Rule index](docs/rules/README.md) — every diagnostic, with examples and fixes
- [Package contracts](docs/package-contracts.md) — the dependency trust model
- [Documentation index](docs/README.md) — architecture, protocols, glossary
- [Contributing](CONTRIBUTING.md) — building and developing solid-checker
