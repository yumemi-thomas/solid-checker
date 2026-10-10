# ADR 0270 script-tool audit — NOTES (2026-10-10)

`host_invocation.rs::ignored` requires an exact, case-sensitive first word
from the positive allowlist below. Every word must use the existing plain-word
grammar; quotes, escapes, globs, shell operators, `$`, backticks and newlines
refuse. Any case-insensitive `vite` substring also refuses. An empty command,
a path-qualified tool, an unknown tool or a wrapper refuses. All app and
enclosing scripts, including hooks, are inspected; only the app's manifest
may additionally use the exact conventional Vite command grammar.

This audits the tools' documented command dispatch, under the same
documented-tool interpretation used for literal `vite`. It is not executable
identity authentication or a sandbox for injected JavaScript in configuration,
plugins or replaced binaries. A configurable *command runner* is never ignored.
Config/plugin code in linters and formatters can have arbitrary JavaScript
side effects; this policy does not prove those programs side-effect-free.

| First word | Ignored | Documented behavior; Vite/configured command dispatch |
| --- | --- | --- |
| `tsc` | Yes | Type-checks/emits TypeScript, including project builds/watch. No app execution, Vite build/server or command hooks. [CLI](https://www.typescriptlang.org/docs/handbook/compiler-options.html). |
| `eslint` | Yes | Lints/fixes files; CLI arguments select files/options/config/plugins, not commands. No Vite/app server or shell-command hook. Config, rules and formatters execute JS. [CLI](https://eslint.org/docs/latest/use/command-line-interface). |
| `prettier` | Yes | Formats/checks files; no Vite/app server or configured shell-command hooks. JS config/plugins execute code. [CLI](https://prettier.io/docs/cli). |
| `oxlint` | Yes | Lints/fixes files, optionally type-checks; no Vite/app build/server or command hooks. JS plugins are executable extensions. [CLI](https://oxc.rs/docs/guide/usage/linter/cli.html). |
| `oxfmt` | Yes | Formats/checks files; no Vite/app server or arbitrary configured command dispatch. Formatter integration is not app execution. [CLI](https://oxc.rs/docs/guide/usage/formatter/cli.html). |
| `biome` | Yes | Checks/lints/formats files and maintains its own analysis daemon; the daemon does not serve the app. No Vite build or command hooks. [CLI](https://biomejs.dev/reference/cli/). |
| `stylelint` | Yes | Lints/fixes stylesheet files; no Vite/app server or command hooks. JS configs/plugins/custom syntax execute code. [CLI](https://stylelint.io/user-guide/cli/). |
| `rimraf` | Yes | Removes paths. CLI flags select deletion/globbing/prompt strategy; no configured commands or app build/server. Globs are nevertheless refused by our grammar. [CLI](https://github.com/isaacs/rimraf#cli). |
| `rm` | Yes | Removes paths; cannot dispatch configured commands or Vite. Audited local `man rm`. |
| `mkdir` | Yes | Creates directories; cannot dispatch configured commands or Vite. Audited local `man mkdir`. |
| `cp` | Yes | Copies files/directories; cannot dispatch configured commands or Vite. Audited local `man cp`. |
| `echo` | Yes | Prints arguments; cannot dispatch configured commands or Vite without shell syntax, which refuses. Audited local `man echo`. |
| `true` | Yes | Returns success; cannot dispatch configured commands or Vite. Audited local `man true`. |
| `openapi-typescript` | Yes | Reads OpenAPI schemas and emits declarations, including remote schema fetches and Redocly processing; no app build/server or shell-command hook in the CLI. Library transform hooks and custom JS plugins are executable code, not proof of purity. [CLI](https://openapi-ts.dev/cli). |
| `depcheck` | Yes | Analyzes dependency use; CLI selects directory, parsers/detectors/specials/config, not commands. No Vite/app server or shell-command hook. Config modules/extensions execute JS. [Usage](https://github.com/depcheck/depcheck#usage). |
| `syncpack` | Yes | Lists/lints/fixes/formats/updates manifest dependency data; no app build/server or command hooks in its documented command surface. JS config is executable. [Commands](https://syncpack.dev/command/list/), [update](https://syncpack.dev/command/update/). |
| `husky` | No | Configures Git hooks which run arbitrary project shell commands. Even though current bare/init CLI installs hooks, no whole-tool admission across hook behavior/versions is granted. [Hook setup/execution](https://typicode.github.io/husky/how-to.html). |
| `lint-staged` | No | Runs arbitrary configured tasks; these can invoke Vite. [Configuration](https://github.com/lint-staged/lint-staged#configuration). |
| `graphql-codegen` | No | Explicit lifecycle command hooks can launch Vite; refuse all forms. [Hooks](https://the-guild.dev/graphql/codegen/docs/config-reference/lifecycle-hooks). |
| `tsx`, `ts-node` | No | Execute arbitrary authored TS/JS, including Vite calls; no script-body simulation. |
| `knip` | No | Custom compiler functions can invoke arbitrary build behavior; whole-tool non-launch behavior is unproved. [Compilers](https://knip.dev/features/compilers). |
| `changeset` | No | `publish` invokes package publishing (and lifecycle scripts); commit/version integrations can run configured hooks. Not safe as a whole-tool allowlist entry. [Commands](https://github.com/changesets/changesets/blob/main/docs/command-line-options.md). |
| `playwright` | No | Test `webServer.command` can launch Vite even without a `vite` spelling in the script. [Web server](https://playwright.dev/docs/test-webserver). |
| `vp` | No | Vite+ `build`/`dev`/`preview` launch Vite; task dispatch can run arbitrary commands. Bare `vp` also refuses. [Guide](https://viteplus.dev/guide/); installed 0.3.2 dispatcher was audited in review 8. |
| `cypress`, `vitest`, `storybook`, `astro` | No | Test/app execution, server or bundler behavior; never unrelated tooling for this proof. |
| `nx`, `turbo`, `concurrently`, `node`, `bun`, `deno`, `npx`, `pnpm`, `npm`, `yarn` | No | Arbitrary task/script/package execution can launch Vite. |
| Any other first word | No | Not audited; absence of a recognized launcher or `vite` spelling is not evidence of unrelated behavior. |

Adding a tool requires this audit and focused positive/refusal controls.
Do not widen the list to retain a candidate app. `echo node` is harmless data
for an admitted tool; `node echo` remains an unknown invocation. Enclosing
literal Vite still refuses even without a parent config. Parent `vp build`
refuses before config selection, including unchanged-manifest parent-config
mutation with retained daemon and fresh one-shot checks.
