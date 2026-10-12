# Development feedback

`solid-checker feedback` is the first implementation boundary for the package
feedback experiment. It runs the native checker on the original application,
then admits optional recorded application observations against those exact
inputs. Package certification is not required to show an independent compiler
finding or a recorded runtime failure. Unknown package behavior stays
uncertifiable in native analysis.

## Use from this checkout

```sh
export SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust"
export SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"
node packages/cli/bin/solid-checker.mjs feedback --project /absolute/path/tsconfig.json
node packages/cli/bin/solid-checker.mjs feedback manifest --project /absolute/path/tsconfig.json > capture.json
node packages/cli/bin/solid-checker.mjs feedback --project /absolute/path/tsconfig.json --capture capture.json
```

Build the main checkout with `make build-checker-debug` first. To use the
locally integrated RC.13 build, set
`SOLID_CHECKER_NATIVE_BIN` to
`rust/target/local-rc13-checker/rust/target/debug/solid-checker-rust` and
`SOLID_TYPEFACTS_BIN` to this checkout's `bin/solid-typefacts`, both as absolute
paths. The compiler candidate has not been published. Production Git pins
remain on their prior compiler revision. The integration patch and restoration
instructions are in `benchmarks/compiler-facts/rc13/`.

The manifest command works without a native binary. TypeScript checks the installed
package declarations; it supplies no behavioral package inference here. When
the input has TypeScript errors, additional capture guidance is suppressed.
Existing native findings retain their own classification.

## Native analysis: binary, time limit, failures

**Binary selection.** The native checker is, in order: `--native-bin <path>`
(or `SOLID_CHECKER_NATIVE_BIN`), the installed platform package, then
`bin/solid-checker-rust` of a repository checkout (which runs `make build-rust`
if absent). An explicit path that does not exist is refused; it never falls back
to a build. The report's `native.binary` records the path and a `kind`
(`debug` / `release` from the cargo profile directory, otherwise `unknown`).
Use a **release** build: debug is roughly 9-19x slower. A debug build on a
project with 50 or more source files adds a `warnings` entry (also printed to
stderr). The repository's `bin/solid-checker-rust` can be a debug build and is
reported as `unknown`.

**Time limit.** Each native analysis is bounded by `--native-timeout <seconds>`
or `SOLID_CHECKER_FEEDBACK_NATIVE_TIMEOUT` (default 600 s, `0` disables the
limit; applies to `feedback run` too). The adapter itself had no limit; the
120 s limit seen in the existing-application evaluation was the evaluation
runner's own `spawnSync` timeout (`run()` in
`benchmarks/reviewed-package-models/development/evaluate-existing-applications.mjs`),
which expired on a debug binary (`finds-team/frontend`: 25 s release, 228 s
debug).

**Failures are structured.** A timeout, crash, signal, spawn error or malformed
output prints a `status: "native-failed"` document on stdout with a `failure`
object (`kind`, `message`, `binary`, `timeoutMs`, `wallMs`, `exitStatus`,
`signal`, `typingErrorCount`, the retained native `stderr`, and `phase`), prints
the message to stderr and exits 2. `phase.lastCompletedStage` is the last
reactive-IR stage the child reported (the adapter always runs the child with
`SOLID_CHECKER_TIMINGS=1` for this); `null` means the child never finished a
stage, i.e. it was still in native setup (project contract admission, source and
Type Facts acquisition, compiler facts). A timeout is never a clean result.

**Measured cost.** A release build analysed all 48 tsconfig projects of 38 real
Solid 2 applications with no adapter failure (records in
`rust/target/adapter-eval/`). Most take 2-30 s. The slow tail is native:
`openbot` 249 s, `solid-groove` 127 s, `app-game` 85 s and `derp-media-server`
77 s are almost entirely the reactive-IR `static-prepass` stage, and
`finds-team/frontend` spends ~24 of 25 s before the first IR stage (a stack
sample shows project contract admission parsing the pnpm lockfile). The default
limit is sized for such projects under a debug build, not for them to be fast.

**Native stderr and timings.** `native.stderr` retains the child's non-timing
stderr (compiler warnings and the like; the last 16 KiB). With
`SOLID_CHECKER_TIMINGS=1` in the caller's environment, `native.timings` also
carries the per-stage list and the native summary object. Native stdout of any
size is accepted.

**Project references.** A project that has files of its own and lists
`references` is analysed as itself. TypeScript checks it as an editor does:
referenced projects contribute their sources, so unbuilt outputs do not raise
TS6305 (plain `tsc -p` on a kui package reports 241 such errors; this check
reports 0). Errors inside a referenced project belong to that project's own
check. The referenced `tsconfig` files are pinned as inputs and listed in
`projectReferences`. A solution-style tsconfig (no files of its own, only
`references`) is expanded recursively to its leaf projects; each is analysed in
its own native session and the report carries `solution.referencedProjects`,
per-leaf `projects[]` (a `report` or a `failure`) and aggregated `findings` /
`gaps`. An unresolvable reference refuses. `feedback manifest` and `--capture`
still require a leaf. `feedback run` expands a solution's leaves for static
analysis, pins the whole reference graph, and executes one shared browser app.
Root and intermediate config parsing retain the same observations as leaf
snapshots: extends chains, directory listings and negative resolutions are
rechecked after execution, including those that decide solution versus leaf.
The report retains `solution.referencedProjects` and each leaf's `projects[].report`.
Source instrumentation/guidance is withheld for a file with conflicting leaf
models (`execution.coverage.conflictingLeafModels`); a changed source digest
refuses. Typing errors in any leaf retain the existing typing exclusion for the
shared capture.

## Capture format

Keep the generated `project`, `inputId`, `manifest`, `authority: false` and
`certification: false`. Fill `runtimeInputs` with canonical absolute file paths
and SHA-256 digests (`sha256:…`) of the executed runtime bytes. Fill `events`
with unique IDs and exact original-source locations:

```json
{
  "id": "read-1",
  "kind": "untracked-read",
  "message": "Recorded read while tracking was off",
  "tracking": "untracked",
  "runtimeInput": "/absolute/path/instrumented-runtime.mjs",
  "sourceSha256": "sha256:…",
  "location": {
    "path": "/absolute/path/src/App.tsx",
    "startByte": 120,
    "endByte": 128
  }
}
```

`runtime-exception` and `assertion-failure` are also admitted, with a message,
source digest and location. They do not require a tracking state. Locations
must belong to a source file in the configured project and use valid UTF-8
byte boundaries. This is a supplied-record interface: digests detect changed
inputs, but do not authenticate that an event occurred. Collectors and callers
remain responsible for the observation's truth and original-source mapping.

The report preserves native `analysis.findings`. Top-level `findings` contains
proven violations; `gaps` contains uncertifiable native results, and
`observations` contains recorded runtime events.
Repeated equivalent observations retain all event IDs and an occurrence count.
Untracked reads and observer queries have severity `info` and
`reactiveIntent: "open"`. Recorded
exceptions/assertion failures have severity `error` in their own channel. No
observation becomes a static violation or a package contract. The exit status
preserves native analysis status; supplied observations do not fail CI.

Inputs are checked before and after native analysis. Changed source, resolved
declarations, directory listings, missing resolution candidates or recorded
runtime bytes refuse a stale report. This boundary does not yet provide an
atomic shared Type Facts/runtime session or bind browser conditions.

## Run an application

The command now includes a live browser collector and the experiment's
automatic read/result guidance and assertion-assisted selection:

```sh
node packages/cli/bin/solid-checker.mjs feedback run \
  --project /absolute/path/app/tsconfig.json \
  --scenario /absolute/path/scenario.json \
  --browser /absolute/path/chromium
```

The application needs its own HTML and client module entry at its Vite root.
The collector uses installed Vite, `@solidjs/vite-plugin`, Playwright and
`@jridgewell/trace-mapping` from that directory. `--tooling /absolute/path/tools`
can select a separate existing tooling installation. The command starts a
local server and executes the application in Chromium. An existing Vite
configuration owns its compiler plugins, aliases, public assets and root; its
configuration files and imported dependencies are recorded as runtime inputs.
`--root /absolute/path/app/runtime-root` and `--config /absolute/path/app/vite.config.ts`
select runtime inputs independently of `--project`; neither changes the static
source selection. `--root` also selects where automatic Vite config lookup
starts when `--config` is omitted. Both must exist, have the appropriate directory/file kind,
and remain inside the app tree after resolving symlinks. The nearest repository
or package-workspace boundary defines that tree, otherwise the nearest package
directory (or tsconfig directory) does. Boundary manifests, config dependencies
and executed source files are pinned; canonical root/config selections appear
in `execution.configuration.options`, with file pins in `execution.runtimeInputs`.
A leaf such as `apps/web/tsconfig.json`
can therefore use its workspace's config with `--root .../apps/web --config .../vite.config.ts`.
Explicit runtime selections with `--compare-project` refuse: collect each app
separately so a path for one app cannot select the other's runtime.

The canonical effective `server.config.root`, after Vite config hooks run,
must remain inside the app tree and equal the reported root; a hook changing
that selection refuses. That effective root must have a physical `index.html`
whose canonical path is an app-tree file. Missing indexes and symlink escapes
refuse. `execution.entry` records its path and `execution.runtimeInputs` pins
its bytes. The browser parses that physical HTML in an inert document, using
its real base-URL and HTML attribute rules. Every external module entry (apart
from Vite's client) must resolve through Vite's client environment; inline,
foreign and unresolved module entries refuse. Each selected entry receives a
first static bootstrap import after application client transforms, before its dependencies execute, with an exact
identity source map. This also binds captured application HTML whose middleware
bypasses Vite's HTML transforms. Unselected virtual helper/CSS modules and server transforms
receive no entry prefix. `execution.entry.entries` retains the exact URLs and
resolved IDs. Request normalization mirrors Vite's HTTP base stripping, timestamp
removal, `decodeURI` and `/@id/`/null-marker unwrapping, preserving encoded reserved
query characters. The served transform's module ID must equal the selected ID.
It does not follow other request rewriting ahead of Vite's transform middleware —
a `server.proxy` `bypass` that returns a new URL, or application middleware that
rewrites `req.url` for a module entry. Such an entry either fails to resolve or
reaches the transform under a different request; both refuse explicitly (an
unresolved entry, or a selected entry with no binding or completed output), so
the run is not measured rather than mis-measured.
`transforms` pins the entry code before and after the prefix and the completed
Vite output (`completedSha256`); completed output must start with the collector import.
If Vite import analysis or a hook with late transform ordering prepends a helper,
the completed-output boundary reasserts the resolved collector import first and
shifts the existing source map by one line. A repeated static import of that same
resolved module evaluates the bootstrap only once.
A twelve-second binding wait precedes scenario actions and final collection, because streamed
server markup can become visible before its client module executes.
A loaded document without a telemetry binding, a selected entry without a
bootstrap transform, or a top-level navigation missing from the bound-document
records refuses. Each top-level loader must have its own default execution-context
binding; iframe starts neither satisfy that requirement nor contribute telemetry.
Every served top-level HTML response is obtained through CDP Network observation,
bounded to two seconds per body, 1 MiB per response, 8 MiB total and 64 documents.
Missing, failed, oversized or unavailable bodies refuse explicitly. Each response
must contain the selected module URLs and no extra module entries beyond Vite's
client and the collector's own HTML bootstrap, even if startup removes script
elements or a later navigation serves different entries. `execution.entry.documents`
retains each response's digest, byte count, entries, frame/loader and bootstrap
execution-context identity. The final DOM cannot substitute for that history.
Missing binding failures retain bounded page-error and failed-
module summaries. `execution.coverage.documents.topLevelNavigations` counts
distinct CDP document loader IDs, excluding same-document route changes.
The collector supplies no generated HTML entry or mount function.
Without an application config, the collector supplies the Solid plugin.

Vite dependency discovery and CommonJS prebundling remain enabled. Only
`solid-js`, `@solidjs/signals`, `@solidjs/web` and their subpaths are excluded
and deduplicated, so their installed files still pass through the byte-reviewed
reader hook. Runtime resolution removes Vite's optimizer `v` query and uses the
canonical installed path: discovering another dependency must not instantiate
the same runtime again under a different browser hash. Resolving separate
installed roots for one runtime package refuses. Application includes for other
packages are retained. Vite keeps
excluded bare imports external even inside included dependency bundles; optimizer
input hooks additionally refuse any of those runtime packages entering a bundle,
including through relative paths or aliases. Changed or unreviewed reader bytes
still refuse. Available optimizer source maps are checked for inlined runtime
files and for source-content disagreement with the installed files.
`execution.optimizedDependencies.generations` records each observed optimizer
generation and environment (`client`, and any server environment used by the
application middleware): the
metadata digest, lockfile/config/browser hashes, dependency and chunk lists,
output/map file digests, and observed optimizer inputs. Physical inputs and
package identity manifests are pinned in `execution.runtimeInputs`; generated
optimizer inputs retain IDs and transformed-code digests, or explicitly mark
unavailable source bytes for an optimizer API that exposes only the load ID.
The collector checks loaded optimizer bytes against that generation's inventory,
then validates the final inventories before removing its temporary cache.
Environment names and committed cache directories come from resolved Vite
configuration, using Vite's exact `deps`/`deps_<environment>` convention. Arbitrary
cache children never create environments. Processing and retired `deps_temp_*`
and `deps_<environment>_temp_*` directories are ignored, including during their
asynchronous deletion; earlier observed generations retain their existing pins.
Legitimate reoptimization retains both old and new generation pins; changed bytes
under unchanged metadata refuse. Ephemeral output pins remain in the report and
are not replayed after replacement or deletion. `transforms` records Vite module
transforms, which can include warmup or attribution loads in addition to HTTP
requests; it does not assert that every inventoried output executed in the browser.
This records optimizer inputs and output; it does
not authenticate arbitrary application/config/plugin side effects or certify
server execution. Missing inventories, inlined runtimes and changed served output
refuse rather than becoming empty runtime evidence.
SSR needs a separate adapter. Requests to other origins are blocked unless the
scenario supplies a response for that exact URL (below); config loading executes
the application's existing config.
The collector closes the browser context, Vite server and browser before
returning JSON. Each resource close has a two-second deadline, and later
resources are attempted even if an earlier close rejects or hangs. The server
is registered in the collector's first configure hook so later startup failures
can close it. A creation failure before Vite exposes a server refuses with an
explicit note that partial resources may remain until CLI exit. Runtime failures
publish a `status: "runtime-failed"` document; native failures retain their
structured `native-failed` document. A cleanup failure after another failure is
attached to that primary failure instead of replacing it, and is published in
its `failure.cleanupErrors`; it is the reported failure only when collection
itself succeeded. The dedicated CLI drains stdout and stderr
in its finalizer and exits with status 2 on failure or the native status on
success, including when application config leaves a process-level timer alive.

A scenario supplies explicit interactions and optional assertions. It never derives
expected values from a proposed change:

```json
{
  "schemaVersion": 1,
  "steps": [
    { "action": "wait-for-text", "selector": "#value", "text": "1" },
    { "action": "click", "selector": "#update" },
    { "action": "wait-for-selector", "selector": "[data-ready=true]" },
    { "action": "assert-text", "id": "updated-value", "selector": "#value", "text": "2" }
  ]
}
```

A scenario can supply exact responses for other origins or same-origin APIs:

```json
"responses": [
  { "url": "https://api.example.test/articles?user=a", "body": "articles.json",
    "status": 200, "contentType": "application/json" },
  { "url": "/api/user?mode=offline", "body": "user.json" },
  { "url": "/api/items", "method": "POST", "requestBody": "{\"name\":\"example\"}",
    "body": "created.json", "status": 201 }
]
```

Each entry matches one exact, normalized `http(s)` URL or `/path?query` for
`GET` by default. `POST` requires an explicit `requestBody` string, matched
against the complete request buffer's UTF-8 bytes (including an explicitly
empty buffer). An unavailable buffer never matches. Wildcards, fragments,
credentials, other methods and duplicate URL/method/body matches refuse.
Same-origin matches are admitted only for Playwright `fetch`/`xhr` requests.
A matching script, document, stylesheet or other resource type is aborted and
recorded in `execution.refusedResponses`, so scenario bytes cannot replace an
executable source or document. Data matches are fulfilled before Vite's proxy; unmatched same-origin
requests keep the application's existing server behavior. `body` is a
file path relative to the scenario, pinned by digest and rechecked after the
run. `status` defaults to 200 and `contentType` to `application/json`. A
response is a scenario input, not observed network behaviour:
`execution.suppliedResponses` lists each URL, method, POST request-body string
and expected digest where present, the actual `matchedRequestBodySha256` (null
if never matched), response status/content-type, body pin and how often it was
served (`channel: "scenario-input"`). Every other cross-origin request stays
blocked and is listed in `execution.blockedRequests`.

Input steps use CSS selectors, as do the `document.querySelector` checkpoints:
`fill` replaces an input's text (`text`), `type` appends text with keyboard
events (`text`), `press` sends a key/chord (`key`, e.g. `Enter`), `select` chooses
an option (`value`), and `check`/`uncheck` set a checkbox or radio's state.
`submit` requires a form selector and calls the browser's `requestSubmit`,
preserving validation and submit events. All steps and their arguments are
pinned by the scenario-file digest; a failed action stops dependent steps.
Playwright locator-engine spellings, chained selectors and CSS extensions
such as `:has-text` are refused; text inside quoted attribute values and
escaped identifiers is CSS, not engine syntax. Before each action, the page validates the
selector with native `document.querySelector`; invalid CSS stops that step
before mutation and appears in `execution.failure`.

Optional scenario `viewport: {"width":390,"height":844,"deviceScaleFactor":2,"isMobile":true}`
is passed to `browser.newContext` and retained in `execution.viewport`, bound
by the scenario digest. Width/height must be integers in 1–16384; optional
deviceScaleFactor must be finite, positive and at most 8; isMobile is boolean.
Omitting the field preserves the browser's default context settings.

`wait-for-text` and `wait-for-selector` establish readiness; `assert-text`
records the current value without waiting for the expected answer. A failing
assertion appears as an error under `assertionFailures`, with its selector,
expected value and measured value. Read observations are separate information.
Runtime exceptions are mapped to original source where possible; unmapped
failures remain visible in `execution.unmapped` and `execution.pageErrors`.
Browser console warnings and errors are retained in `execution.consoleDiagnostics`
with their original browser locations; these are not inferred causal findings.
`execution.serverDiagnostics` retains Vite warnings/errors and failed same-origin
script/module response bodies: at most 64 rows per channel, 16 KiB per message/body,
with byte lengths, truncation flags and dropped-row counters. Bodies are captured
from the original outgoing Vite response as chunks are written, copying only
the first 16 KiB; acquisition stops after two seconds. Body rows record
`timedOut` and `incomplete`; `bytes` counts bytes observed before capture stops,
and a missing bounded capture is explicit as `bodyUnavailable`. A capture carries
no request identity, so a body is attached only when one failed capture and one
failed script response share its URL and status, and no capture was dropped
(`capturesDropped`); otherwise the row says why it is unavailable. No second
request or unbounded Playwright body read is made. These preserve
transform evidence that silent Vite logging previously hid; a failure alone
does not prove an instrumentation or compiler defect.
If a readiness step times out, `execution.failure` identifies the step and
selector. Earlier assertions, browser errors and read evidence remain in the
result, and later interactions stop. Unmapped reads retain their runtime
frames, classified as described under "Explain what the run covered", without
claiming authored source attribution.
Native findings keep their original proof classification.

The read hook is tied to the reviewed `@solidjs/signals@2.0.0-rc.9`
`dev-shared.js` digest. It records normal native reads without an observer or
owner, suppresses explicit native `untrack`, and leaves thrown reads to runtime
error collection. It also records a native `getObserver` query that returns
null on a modeled result path. Queries by the collector itself are excluded.
Source maps locate reads and queries in configured original source.
It covers reads that reach this shared reader, including calls from installed
packages. Store-specific serving paths, owner-present reads, unexecuted paths,
missing source frames and other runtime artifacts remain open. The collector
retains at most 256 records and 1 MiB of event data per document; it reports
dropped records and always labels coverage incomplete. A full page load (an
anchor the router does not intercept, a reload) starts a new document, so each
document reports its records from `beforeunload` over a CDP binding, and the
report merges them in load order (`document` on each record).
`execution.coverage.documents` counts documents `loaded`, `reported` and
`lost`; a lost document also appears under `automatic.open`. Reads during
unload itself are not reported. A different runtime profile refuses
instrumentation. The RC.13 **analysis compiler** does not expand this runtime
profile to RC.13.

### Explain what the run covered

A read or exception with no mapped frame lands in `execution.unmapped` with an
`attribution` class. Each of its frames also records the `outcome` that stopped
mapping (`foreign-origin`, `virtual-module`, `collector`, `no-source-map`,
`no-original-position`, `outside-configured-sources`, `position-out-of-range`,
…). The frame also records the installed package that serves it, and, where
the package's own map resolves it, the original location:

| `attribution` | Meaning |
| --- | --- |
| `application-frame-unmapped` | A frame served from an application-owned file failed to map: lost application attribution. |
| `package-frames-only` | The complete synchronous stack holds only installed-package frames (e.g. a router computation run by the scheduler). `firstPackageFrame` is the first frame outside `@solidjs/signals`. This names where the code lives, not who is responsible. |
| `stack-incomplete` | No application frame, but the stack reached the collector's 40-frame limit (`stackTruncated`) or its depth is the page's own (exceptions), so no frame can be ruled out. |
| `no-attributable-frame` | No frame came from the application server. |

`execution.coverage.unmapped` counts records per class.
`execution.coverage.candidateScopes` joins the native candidates to what ran:
derived-origin functions and operations with an accepted result relationship,
counted as modeled, instrumented and entered (operations also report whether
they returned or threw synchronously). `notEntered` lists up to 64 candidates
with path, span, line and status: `file-not-loaded`, `not-instrumented` or
`not-entered`. An async body's settlement is not observed. A quiet run whose
derived origins were never entered says nothing about warning precision. In
that case, extend the scenario to reach them.

On the unchanged helge-dev replay, all six retained records are
`package-frames-only`, made of `@solidjs/router@2.0.0-next.26` computations
reading through the scheduler. None of the three derived-origin candidates (in
`Blog.tsx` and `Article.tsx`, pages the scenario never visits) was entered;
9 of 32 result operations were.

The evaluation runner's scenario
(`benchmarks/reviewed-package-models/development/evaluate-existing-applications.mjs`)
now also visits Blog and opens an article. It supplies synthetic dev.to
responses, including a blacklisted id that the application filters out. The
article link carries `target="_self"`, which the router does not intercept,
so opening it is a full page load. Before per-document reporting, that load
silently discarded every Blog record. Now:

- all six assertions pass, with no page errors, console diagnostics or blocked
  requests;
- both documents report, and all three derived-origin candidates are entered,
  along with 26 of 32 result operations;
- the run records 1,288 reads and 380 observer queries, and retains 18 records
  with none dropped.

Sixteen of those records are `package-frames-only`: eight router computations,
and eight reads internal to `@solidjs/signals` with no other package frame. The
other two map to the top-level `render()` call in `index.tsx` and carry no
derived-origin lineage, so they stay open. No automatic note is emitted. The
candidates read their reactive inputs before `await` or inside tracked memos,
so the quiet result is consistent with the source. That is one application's
three candidates, not a precision rate.

### Solid dev diagnostics

The collector subscribes to the dev build's `OBSERVE.diagnostics` channel. Solid
emits each diagnostic synchronously at the operation that triggered it
(`STRICT_READ_UNTRACKED`, `NO_OWNER_CLEANUP`, `NO_OWNER_EFFECT`,
`REACTIVE_WRITE_IN_OWNED_SCOPE`, `CLEANUP_IN_FORBIDDEN_SCOPE`, …), so the
stack captured there locates it. `execution.diagnostics` lists each one with
its code, severity and message. `operation.in` says which code performed the
diagnosed operation: `application`, or `package` with the installed package,
skipping Solid's own runtime frames. `siteKind` says what the authored
expression at the site is: `package-call` (the export's own call, so a read it
makes is the package's behaviour) or `value-access` (a value the package
returned, read by the application). It has a `site` when a frame maps to configured
source; otherwise it keeps the same `attribution` class and
`firstPackageFrame` as an unmapped read. These are the runtime's own reports
(`channel: "runtime-diagnostic"`), not inferences. The collector keeps at most
256 per document.

The misuse ledgers measure this channel against seeded pairs. Each runs the
misuse and the correct twin as small client apps on published packages
(`benchmarks/reviewed-package-models/development/misuse-runtime-ledger.mjs`,
`fixtures/primitives-misuse/cases.json` and
`misuse-extra-cases.json`). A case is detected when the misuse twin gets an
expected diagnostic, or an uncaught exception, at the case file and the correct
twin gets none.

### Measure a supplied comparison

Add `--compare-project /absolute/path/comparison/tsconfig.json` to execute the
same scenario against a separate application checkout. Neither source tree is
edited. The selector gives an informational debugging note only when an
original assertion fails and its matching assertion passes in the comparison.
Passing original assertions stay quiet. A comparison that breaks one is
reported under `guidance.open`. Missing comparisons and comparisons that still
fail also stay open. TypeScript errors suppress assertions and guidance for
that input, using installed published declarations.

This ports the decision boundary from `replay-assertion-feedback-v1.mjs` in the
experiment. The message describes the measured supplied comparison. It does
not claim that a read caused the failure, that the comparison is a safe repair,
or that unrelated assertions and side effects pass. Source proposals are not
generated automatically. `observations`, `assertions` and `guidance` remain
distinct; the exit status continues to preserve the original native result.

### Automatic guidance without assertions or comparison code

`automatic.notes` now contains conditional warnings from executed reads and
native source models, plus informational observer-query notes. A scenario can
contain just readiness steps and clicks;
no assertion or comparison application is required.

The native `--feedback-facts` request emits source digests, exact function and
call spans, and derived origins resolved by the normal Type Facts lookup and
dialect interface. It selects bounded candidate result paths: a call in a
returned expression, an exact const initialization referenced by the returned
value, or a read controlling distinct numeric field results. Multiple competing
return paths remain open. These facts are absent from ordinary diagnostic output.

The adapter instruments only those exact spans. Function tokens keep an origin
across native `await` continuations and nested callbacks. A callback invoked
later must also have a native allocation path related to its parent's result;
lexical nesting alone is insufficient. Synchronous operation
scopes restore the previous caller even when a call throws. Runtime values and
function identities remain intact. TypeScript parses and prints syntax; it
does not infer package behavior. Methods join through exact body spans when
the parsers use different function spans. Receiver behavior and callable names
are preserved. Generator bodies, calls with suspending arguments, direct `eval`
and unmatched spans have no instrumentation of their own.

Selection joins mapped executed reads to current native models. It receives no
expected result, benchmark label or comparison code. The observed authored
site must belong to the final modeled operation. A retained callback lineage
with candidate result relationships can produce a warning: **if** this derived
result should follow the value, capture that value while tracking and pass it
to the async work. The warning does not prove stale output, Promise adoption,
settlement, or repair safety. Intentional snapshots and object identity can
need application expectations. Discarded reads and competing returns remain
visible under `automatic.open`; incomplete collection cannot establish safety.

An observer query alone produces an informational note, because it does not
prove a reactive read or a skipped subscription. This surfaces APIs such as
the installed map's trigger cache, which returns early when no observer exists
and consequently emits no native read. Plain deferred callbacks can receive
read guidance without being declared `async`. Query information is suppressed
when the same derived origin already has an observed-read warning.

The native analysis and published typing check run once per project capture.
Their retained inputs are validated again after execution, including negative
resolution candidates and directory listings. This avoids repeating unchanged
type checks without weakening stale-input refusal.

## Remaining implementation

Editor/watch integration, automatic comparison planning, wider result-flow
models, package ownership tracing and additional runtime profiles remain work.
The native models above implement a bounded slice of the experiment's automatic
read selector. They do not implement its complete behavioral model chain.
