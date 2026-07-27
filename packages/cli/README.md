# solid-checker

Project-level reactivity checker and language server for Solid. The package
ships native `solid-checker` and `solid-typefacts` executables
for supported platforms.

Install it as a development dependency:

```sh
npm install --save-dev solid-checker
```

Then run `solid-checker --certify`.

The CLI uses the Oxc graphical reporter for framed terminal diagnostics:

```sh
solid-checker --project tsconfig.json
```

The `default` format prints the same style of source frames, severity markers,
evidence labels, and error summary used by Oxlint. Use `--format json` for
machine-readable findings or `--format text` for compact output.

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
