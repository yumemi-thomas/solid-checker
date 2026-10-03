# Active package-feedback experiment

For evaluating the reusable CLI on unchanged retained applications, use
`evaluate-existing-applications.mjs`, `run-existing-client-tests.mjs`,
`review-existing-app-boundaries.mjs` and `audit-existing-app-evaluation.mjs`.
The [evaluation report](../../../docs/package-contract-v2/phase22/2026-10-03-existing-application-cli-evaluation.md)
records commands, exact inputs, reviewed findings, timeouts and the isolated
configuration-adapted collector profile. Those results are separate from the
authored browser populations below.

For the two completed applications, pass `focused` as the evaluator's third
argument. `audit-feedback-precision-fixes.mjs <before-results> <after-results>
<fresh-output>` independently checks the fifteen previously reviewed sites and
the original-config client scenario. See the
[precision/configuration milestone](../../../docs/package-contract-v2/phase22/2026-10-03-feedback-precision-and-application-config.md)
for its measured results and remaining source-mapping/SSR limits.

Use this directory for new development. Edit `selector.mjs` for feedback
projection, `readiness.mjs` for the authored app protocol, and `profile.mjs` for
the default focused selection. Keep the historical versioned scripts intact.

## Browser cycle

From the repository root:

```sh
node benchmarks/reviewed-package-models/development/run.mjs --plan
node benchmarks/reviewed-package-models/development/run.mjs \
  rust/target/development-cycle-N /absolute/path/to/chromium
```

The command snapshots the current working tools, runs focused tests, executes
observed and plain apps, and runs the independent source/behavior audit. It
records phase times and sampled memory for the owned browser process tree.
Every invocation needs a fresh output directory. Configurations must state a
`hypothesis` and `success` criterion. These are recorded intentions; the command
does not infer that passing checks proves every stated hypothesis.

An optional third argument names a JSON configuration. Start with the output
of `--plan`. A fourth argument selects `chromium` or `lightpanda`. Chromium is
the default and remains the final validation browser. An unchanged plain
baseline can be reused by setting `baseline` to its `results.json`; the audit
still checks its original inputs and behavior. A custom `freeze` must match
the working tools. With `freeze: null`, a fresh snapshot is created in the run.

The readiness adapter adds a pending probe reading the same local memo/field
the authored view reads. `Solid.isPending` supplies the status. Deliberately
pending cases instead expose an authored body counter. This protocol requires
application cooperation. It does not prove completion of arbitrary package
work, Promise adoption, effects or network activity. Unsupported protocols and
timeouts fail the run. Added probe reads remain visible in runtime counts.

## Selector edits without a browser

Create a replay configuration with `hypothesis`, `success`, and `observation`:

```json
{
  "hypothesis": "The edited selector preserves the admitted observations.",
  "success": "Review changed hints and run an independent audit before handoff.",
  "observation": {"path": "/absolute/path/to/observed/results.json", "sha256": "sha256:..."},
  "requireOriginalEquality": true
}
```

Run:

```sh
node --expose-gc benchmarks/reviewed-package-models/development/replay.mjs \
  /absolute/path/to/replay-config.json rust/target/development-replay-N
```

Replay authenticates the observation, original archived profile, package
closures and complete final-stage source/resolution inputs. It rebuilds the
source program and reproduces the original projection first. Then it uses that
same program for the working selector. Set `requireOriginalEquality: false`
only when a selector change is intentional, and review the changed projection.
Replay archives the working tool bytes separately from the observed profile;
it never claims that the browser executed the edited selector.

`workspace.mjs` also exposes a resident `replayWorkspace` interface for repeated
projections of the same case. Its pool holds two programs by default. Keys
include complete input manifests and source/TypeScript producer identity;
timestamps are not trust inputs. Missing files, directory listings, real paths,
configuration and content changes invalidate reuse. A synchronous projection
validates its full inputs before and after running and closes its session
facade afterwards. It rejects async work and retired revisions.

Only final stages still matching disk can be replayed. Historical stages need
an explicit historical filesystem view, which this tool does not provide.
Changes to instrumentation, runtime hooks, source models or execution behavior
need new browser observations. Selector changes still need meaningful tests
and an independent audit. These files and hashes are user-writable research
evidence, with `authority: false` and `certification: false` throughout.
