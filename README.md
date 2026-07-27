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
| `--format <default\|text\|json>` | Output format. `default` prints framed source excerpts, `text` is compact, `json` is machine-readable. |
| `--certify` | Exit non-zero unless the project is fully certified. Use this in CI. |
| `--check-contracts` | Report imported Solid packages that ship no reactivity contract. |
| `-h`, `--help` | Print help. |

Authoring a package contract (see [Publishing a Solid library?](#publishing-a-solid-library)):

| Option | Description |
| --- | --- |
| `--emit-contract <PATH>` | Write a generated `solid-reactivity.json` contract. |
| `--package-name <NAME>` | Package name recorded in the emitted contract. |
| `--package-version <VERSION>` | Optional package version for the contract. |
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

List which of your dependencies are missing a contract:

```sh
solid-checker --project tsconfig.json --check-contracts
```

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

Ship a `solid-reactivity.json` contract describing the reactive behavior of your
exports so downstream projects stay certifiable without analyzing your source:

```sh
solid-checker --project tsconfig.json \
  --emit-contract solid-reactivity.json \
  --package-name my-package \
  --package-version 1.0.0
```

Published at your package root as `solid-reactivity.json`, it's discovered
automatically from `node_modules`. See
[package contracts](docs/package-contracts.md) for hashing artifacts and the
trust boundary.

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
