# Development feedback

`solid-checker feedback` runs the native checker on an application and, on
request, admits recorded runtime observations against those exact inputs. A
runtime failure or a Solid dev diagnostic is shown without any package
certification; unknown package behavior stays uncertifiable in native analysis.
Recorded observations never become static violations and never fail CI.

## Commands

```sh
export SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust"
export SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"
node packages/cli/bin/solid-checker.mjs feedback --project /abs/tsconfig.json
node packages/cli/bin/solid-checker.mjs feedback manifest --project /abs/tsconfig.json > capture.json
node packages/cli/bin/solid-checker.mjs feedback --project /abs/tsconfig.json --capture capture.json
node packages/cli/bin/solid-checker.mjs feedback run --project /abs/app/tsconfig.json \
  --scenario /abs/scenario.json --browser /abs/chromium
```

- `feedback` analyzes statically and prints the report.
- `feedback manifest` lists what a capture must pin (installed declaration
  inputs). It needs no native binary.
- `--capture` admits a supplied capture file (below).
- `feedback run` also executes the application in Chromium under a scenario and
  collects what ran.

## Native analysis

**Binary.** In order: `--native-bin` (or `SOLID_CHECKER_NATIVE_BIN`), the
installed platform package, then a checkout's `bin/solid-checker-rust`
(`make build-rust` if absent). An explicit path that does not exist is refused.
`native.binary` records the path and a `kind` (`debug`, `release`, `unknown`).
Use a release build: debug is roughly 9-19x slower, and a debug build on 50 or
more source files adds a `warnings` entry.

**Time limit.** `--native-timeout <seconds>` or
`SOLID_CHECKER_FEEDBACK_NATIVE_TIMEOUT` (default 600 s, `0` disables; applies
to `feedback run` too). A timeout is never a clean result.

**Failures are structured.** A timeout, crash, signal, spawn error or malformed
output prints a `status: "native-failed"` document with a `failure` object
(`kind`, `message`, `binary`, `timeoutMs`, `wallMs`, `exitStatus`, `signal`,
`typingErrorCount`, retained `stderr`, `phase`), the message on stderr, and exit
status 2. `phase.lastCompletedStage` is the last Reactive IR stage the child
finished (`null`: still in native setup). The child always runs with
`SOLID_CHECKER_TIMINGS=1` for this; with the variable also set in the caller's
environment, `native.timings` carries the per-stage list.

**Typing errors.** When the input has TypeScript errors against the installed
declarations, additional capture guidance is suppressed.

**Project references.** Referenced projects contribute sources as an editor
would (no TS6305 for unbuilt outputs) and their `tsconfig` files are pinned. A
solution-style tsconfig (only `references`) is expanded to its leaf projects, each
analyzed in its own native session; the report carries `solution.referencedProjects`
and per-leaf `projects[]`. An unresolvable reference refuses. `feedback manifest`
and `--capture` require a leaf.

## Report

Native `analysis.findings` are preserved. Top-level `findings` holds proven
violations, `gaps` the uncertifiable native results, `observations` the recorded
runtime events, `assertionFailures` failed scenario assertions, and
`automatic.notes` and `automatic.open` the guidance below. The exit status
preserves the native analysis status.

Inputs are checked before and after native analysis. Changed source, resolved
declarations, directory listings, missing resolution candidates or recorded
runtime bytes refuse a stale report. There is no atomic shared Type Facts and
runtime session, and browser conditions are not bound.

## Capture format

Keep the generated `project`, `inputId`, `manifest`, `authority: false` and
`certification: false`. Fill `runtimeInputs` with absolute paths and `sha256:`
digests of the executed runtime bytes, and `events` with unique ids and exact
original-source locations:

```json
{ "id": "read-1", "kind": "untracked-read", "message": "Recorded read", "tracking": "untracked",
  "runtimeInput": "/abs/instrumented-runtime.mjs", "sourceSha256": "sha256:...",
  "location": { "path": "/abs/src/App.tsx", "startByte": 120, "endByte": 128 } }
```

`runtime-exception` and `assertion-failure` are also admitted (message, source
digest and location, no tracking state). Locations must lie in a configured
source file on valid UTF-8 byte boundaries. Digests detect changed inputs; they do
not authenticate that an event occurred, so collectors own the truth of the
record. Repeated equivalent observations keep all ids and an occurrence count.
Untracked reads and observer queries have severity `info`; recorded exceptions
and assertion failures have severity `error`.

## Running an application

The application needs its own HTML and client module entry at its Vite root. The
collector uses the installed Vite, `@solidjs/vite-plugin`, Playwright and
`@jridgewell/trace-mapping` there (`--tooling <dir>` selects another
installation), starts a local Vite server and executes the app in Chromium. An
existing Vite configuration owns plugins and aliases and is recorded as a runtime
input; without one the collector supplies the Solid plugin. `--root` and
`--config` select runtime inputs independently of `--project` and must stay inside
the app tree (nearest repository, workspace or package boundary); they are
refused with `--compare-project`.

What the run pins and refuses:

- **Entry and documents.** The effective `server.config.root` must stay inside the
  app tree and hold a physical `index.html`; every module entry must resolve
  through Vite's client environment (inline, foreign and unresolved entries
  refuse) and gets a bootstrap import before its dependencies run. Every
  top-level HTML response is read through CDP (bounded in size and count) and
  must contain exactly the selected entries; a document without a telemetry
  binding, or an unavailable body, refuses. `execution.entry` and `transforms`
  record URLs, resolved ids and code digests.
- **Module identity.** The collector assumes the dev server serves each module URL
  as that module. A `server.proxy` entry with a `bypass` function refuses;
  rewriting middleware and concealed configuration are outside the premise.
- **Solid runtime.** `solid-js`, `@solidjs/signals`, `@solidjs/web` and their
  subpaths are excluded from Vite's optimizer and deduplicated so their installed
  bytes pass through the reviewed reader hook. Separate installed roots for one
  runtime package, an optimizer bundle inlining them, changed reader bytes,
  missing optimizer inventories and changed served output all refuse rather than
  becoming empty evidence. `execution.optimizedDependencies.generations` records
  each optimizer generation's metadata, hashes and output digests.
- **Network.** Requests to other origins are blocked (`execution.blockedRequests`)
  unless the scenario supplies a response for that exact URL. SSR needs a separate
  adapter. Runtime failures publish `status: "runtime-failed"`; the CLI exits 2 on
  failure and the native status otherwise.

### Scenario

A scenario supplies explicit interactions and optional assertions; it never
derives expected values from a proposed change.

```json
{
  "schemaVersion": 1,
  "viewport": { "width": 390, "height": 844, "deviceScaleFactor": 2, "isMobile": true },
  "steps": [
    { "action": "wait-for-text", "selector": "#value", "text": "1" },
    { "action": "click", "selector": "#update" },
    { "action": "assert-text", "id": "updated-value", "selector": "#value", "text": "2" }
  ],
  "responses": [
    { "url": "https://api.example.test/articles?user=a", "body": "articles.json" },
    { "url": "/api/items", "method": "POST", "requestBody": "{\"name\":\"example\"}",
      "body": "created.json", "status": 201 }
  ]
}
```

Steps use CSS selectors (locator-engine spellings such as `:has-text` are
refused). Actions: `wait-for-text`, `wait-for-selector` (readiness), `assert-text`
(records the current value without waiting), `click`, `fill`, `type`, `press`,
`select`, `check`, `uncheck` and `submit`. A failed action stops dependent steps;
a readiness timeout names its step in `execution.failure` and keeps earlier
evidence.

`responses` match one exact normalized URL (GET by default; POST needs an exact
`requestBody`); wildcards, fragments, credentials and duplicates refuse. Same-origin
matches apply only to `fetch` and `xhr` (a matching script or document is aborted
and listed in `execution.refusedResponses`). `body` is a file relative to the
scenario, pinned by digest. Supplied responses are scenario input, not observed
network behavior (`execution.suppliedResponses`).

### Observations

- **Reads and queries.** The read hook is tied to the reviewed
  `@solidjs/signals@2.0.0-rc.9` `dev-shared.js` digest (a different runtime
  profile refuses instrumentation). It records reads made without an observer or
  owner, suppresses explicit `untrack`, and records a `getObserver` query that
  returns null on a modeled result path. Source maps place them in original source.
  At most 256 records and 1 MiB per document are kept; dropped records are
  reported and coverage is always labeled incomplete. A full page load starts a
  new document; `execution.coverage.documents` counts `loaded`, `reported` and
  `lost`.
- **Solid dev diagnostics.** The collector subscribes to the dev build's
  `OBSERVE.diagnostics` channel (`STRICT_READ_UNTRACKED`, `NO_OWNER_CLEANUP`,
  `REACTIVE_WRITE_IN_OWNED_SCOPE`, ...). `execution.diagnostics` lists each with
  code, severity, message, `operation.in` (`application`, or `package` with its
  name) and `siteKind` (`package-call` or `value-access`), plus a `site` when a
  frame maps to configured source. These are the runtime's own reports
  (`channel: "runtime-diagnostic"`), at most 256 per document, not inferences.
- **Console and server.** Browser console warnings and errors, and Vite warnings
  and failed same-origin script bodies, are kept (bounded) in
  `execution.consoleDiagnostics` and `execution.serverDiagnostics`. A failure alone
  does not prove an instrumentation or compiler defect.
- **Unmapped records.** A read or exception with no mapped frame lands in
  `execution.unmapped` with an `attribution`: `application-frame-unmapped` (lost
  application attribution), `package-frames-only` (the whole stack is installed
  packages; names where the code lives, not who is responsible),
  `stack-incomplete` or `no-attributable-frame`.
- **Coverage.** `execution.coverage.candidateScopes` joins the native candidates
  to what ran (modeled, instrumented, entered; `notEntered` lists up to 64 with
  `file-not-loaded`, `not-instrumented` or `not-entered`). A quiet run whose
  derived origins were never entered says nothing about warning precision; extend
  the scenario to reach them.

### Automatic guidance

`automatic.notes` holds conditional warnings from executed reads joined to native
source models, plus informational observer-query notes; no assertion or comparison
application is needed. The native `--feedback-facts` request emits source
digests, exact function and call spans and derived origins, and the adapter
instruments only those spans. A warning is possible when a read's authored site
belongs to the final modeled operation of a retained callback lineage: "if this
derived result should follow the value, capture it while tracking and pass it to
the async work". It does not prove stale output, settlement or repair safety.
Discarded reads and competing return paths stay under `automatic.open`. An
observer query alone is informational (it proves neither a read nor a skipped
subscription) and is suppressed where the same origin already has a read warning.
Generator bodies, calls with suspending arguments, direct `eval` and unmatched
spans carry no instrumentation.

### Comparison

`--compare-project /abs/comparison/tsconfig.json` runs the same scenario against a
separate checkout; neither tree is edited. An informational debugging note appears
only when an original assertion fails and its matching assertion passes in the
comparison. A comparison that breaks a passing assertion, is missing, or still
fails stays under `guidance.open`. The note describes a measured comparison; it
does not claim causation, a safe repair, or that other assertions pass.

## Not implemented

Editor and watch integration, automatic comparison planning, wider result-flow
models, package ownership tracing and further runtime profiles.
