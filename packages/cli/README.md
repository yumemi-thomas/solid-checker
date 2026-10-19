# solid-checker

Project-level reactivity checker and language server for Solid. The package
ships native `solid-checker` and `solid-typefacts` executables
for supported platforms.

Install it as a development dependency:

```sh
bun add --dev solid-checker
```

Then run `solid-checker --certify`. It reports proven violations and project
notices only; results the analysis could not decide are internal (`--internal`
or `--format json` shows them).

## Dependency analysis

Declare `--runtime-target browser|node` and `--runtime-build development|production`
to select installed dependency source using the Vite and Solid plugin premises.
`--dependency-source <specifier>` narrows selection. Unavailable behavior stays
an explicit SC9005 uncertifiable obligation. Without a declared host, the
checker does not invent package behavior.

Package contracts, catalogs, receipts, generation and certification commands
have been removed. ESLint’s `acceptedContracts`, `receiptTrustConfiguration`
and `dependencySource` settings are refused. `--no-package-contracts` remains
a compatibility no-op. See [ADR 0312](../../docs/adr/0312-package-contracts-are-removed.md).

## Development feedback

The experimental feedback command combines native findings with recorded
application observations:

```sh
solid-checker feedback --project tsconfig.json
solid-checker feedback manifest --project tsconfig.json > capture.json
solid-checker feedback --project tsconfig.json --capture capture.json
solid-checker feedback run --project tsconfig.json --scenario scenario.json --browser /absolute/path/chromium
```

The manifest starts an input-bound capture. A collector fills its `events` and
`runtimeInputs` before the final command. Source, configuration, resolution and
runtime changes refuse stale captures. Native findings keep their original
violation/uncertifiable classification; recorded untracked reads are information
because snapshot intent is open. Runtime exceptions and failed application
assertions can be supplied as separate observations. `feedback run` now collects
live native read observations and conditional automatic warnings from native
source models. Scenarios can contain interactions without assertions or
comparison code. Discarded reads, competing returns and unresolved result
relevance stay open. The automatic notes do not prove stale output or intent.
The command also executes supplied browser assertions. Add
`--compare-project /absolute/path/comparison/tsconfig.json` to measure the same
assertions in a separate checkout: failed assertions that pass there receive
informational debugging guidance; passing originals stay quiet. The collector
uses a reviewed RC.9 native-reader profile. Automatic source proposals and
package behavior inference remain unavailable.

See [the capture format and current limits](../../docs/development-feedback.md).
