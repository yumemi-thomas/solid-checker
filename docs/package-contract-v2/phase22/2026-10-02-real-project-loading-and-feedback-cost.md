# Real project loading and the cost of package feedback

This slice moves beyond authored consumer snippets: it inspects **986 configured
source files in eight retained real projects** and executes a full portfolio-app
flow in a browser. It finds and fixes a practical integration defect: the
prototype's isolated TypeScript programs lose app aliases and ambient
declarations, causing false typing exclusions on otherwise valid files.

The project-aware loader corrects **11 false exclusions among 28 sampled files**.
A necessary syntax gate then avoids expensive analysis for **461/473 eligible
TSX files**, while preserving all five admitted reads in type-valid projects.
The original three async targets remain visible and their three controls stay
quiet. No newly discovered async defect in a real app is claimed.

## Real source inventory

`async-read-app-inventory-v1.mjs` uses each project's actual TypeScript
configuration, retained published declarations and resolved Solid runtime.
It inspects the nine configurations historically recorded as rc.9. Eight load
successfully; the ninth currently lacks `@solidjs/web` and is refused. Earlier
prereleases are not silently substituted or analyzed under the rc.9 profile.

| Retained project | Configured source files | Published-type errors | Native callback-read candidates |
| --- | --- | --- | --- |
| oscartbeaumont-website | 20 | 0 | 0 |
| viviana-ui-web | 335 | 403 | 2 |
| sefer | 353 | 0 | 4 |
| solid-validation-site | 14 | 14 | 0 |
| lutra-console | 11 | 2 | 0 |
| finds-team | 160 | 0 | 0 |
| helge-dev | 18 | 0 | 0 |
| expenses-app | 75 | 0 | 1 |
| jandibat-web | Refused: missing installed `@solidjs/web` | — | — |

The source count includes configured tests, tools and workspace files under
the retained app roots. It excludes declarations and `node_modules`.
Historical corpus import counts are not a feedback score.

All seven candidates are ordinary synchronous array callbacks: `find`, `map`
and `filter`. Four are Sefer inventory filters, one is the expenses form's
currency selection, and two are Viviana example screens. The two Viviana
sites are excluded under its actual project typing errors. The other five
are eligible for observation; enrollment does not mean a defect exists.
Their runtime contexts have not been observed in this slice.

The inventory records configuration reads, loaded sources and declarations,
exact installed runtime package digests, source bytes and timings. It checks
those retained bytes again after measurement. No installs or package patches
are used. All artifacts retain `authority: false` and `certification: false`.

## Correct project loading and cache invalidation

The previous transform constructs a separate program for each TSX file with
generic compiler options. On real app files, this loses configuration paths
such as `#app/*` and `@/*`, and ambient declarations such as Vite's import
types. The resulting errors do not occur in the same files under their app's
actual configuration.

`project-read-session-v1.mjs` owns one TypeScript program per project. It loads
the real configuration and its included declarations, and reuses the program
while observed inputs agree. Source/configuration bytes, module-resolution
existence checks, resolved paths and included-file listings participate in
invalidation. Changed alias targets, ambient declarations, consumer edits,
inherited configurations and newly appearing imports or source files rebuild
the program. A served consumer with different bytes is refused.

`async-read-transform-v3.mjs` uses that program with the unchanged V2 semantic
selector and informational projector. A sampled comparison covers 28 files
across eight projects. It corrects 11 false per-file exclusions while retaining
11 sampled exclusions caused by actual project typing errors. Each sampled
project builds its program once. Type errors produce no instrumentation or
checker hint. This fixes admission; it creates no new rule for a type-system
defect.

Referenced TypeScript projects need an explicit session and are refused.
Consumers outside the session or absent from its configured program are also
refused. Projects without a configuration use the existing pinned generic
options; an additional source root rebuilds that fallback program. Full editor
integration, watcher-driven invalidation and changes racing an analysis remain
open. This prototype is not a Type Facts replacement or certification authority.

## Avoiding unnecessary analysis

The first real app replay exposes cost even when there is no read to enroll.
V3 constructs a program once and validates its inputs for each of 13 loaded
TSX files. Its initial program takes about 322 ms, with another 10–13 ms of
input validation per reused request.

`async-read-prefilter-v1.mjs` recognizes a necessary syntax condition for the
existing selector: an import from the Solid core and a zero-argument identifier
call inside a callback nested in a function supplied as the first argument of
another call. Exact native symbol resolution still decides enrollment. This
gate cannot supply a semantic premise or classify a package by name.

`async-read-transform-v4.mjs` leaves source unchanged when that necessary
condition is absent. On the complete eligible-source study:

- **473 TSX files** fall within the current plugin's `src/` scope.
- **461 files skip** TypeScript program construction and validation.
- The remaining 12 files require **four project programs**.
- The five valid enrolled sites match the earlier whole-program inventory.
- Both candidates in the type-invalid Viviana project remain excluded.

The aggregate transformation time is **8.26 seconds**, mainly initial program
construction in four projects. Individual project totals are approximately
2.14 s for Viviana, 3.01 s for Sefer, 1.51 s for Finds and 1.58 s for Expenses.
The other four projects need no program for this channel. Initial program
analysis and byte validation are still substantial costs on complex apps.

These are local measurements with retained dependencies and filesystem caches.
They do not establish cold-machine startup or editor latency. The first sampled
study also undercounts old per-file timing for four additional files by taking
that timestamp before typing; no speed ratio uses those observations. V2 of the
app study measures the complete eligible transform path and avoids that flawed
comparison.

## Full app behavior and existing feedback

The retained Helge portfolio app runs from fresh copies. The only source
adaptation exposes the existing render disposer for the experiment. A full-tree
auditor compares **41 copied source/asset files**, original entry bytes, package
pins, real typings, native feedback, exceptions and nine executed steps:
mount, home, three route navigations, modal open/close, mobile navigation,
history back and disposal.

| Browser profile | Mount time | Project programs for the new channel | Async-read observations |
| --- | --- | --- | --- |
| Plain baseline | 843 ms | 0 | 0 |
| Project-aware V3 | 1,332 ms | 1 | 0 |
| Syntax-gated V4 | 816 ms | 0 | 0 |

All flows pass; both full-tree comparisons report no behavioral or existing
feedback changes. V4 skips all 13 loaded TSX files. This is one execution per
variant, so the mount times do not establish a statistically reliable speedup
or zero overhead.

All three profiles retain one `UNSTABLE_MEMO_OUTPUT` performance advisory under
the router owner path. It has no mapped app source location. There are no
escaping exceptions or execution-error feedback. The advisory is not counted
as a proven application defect or a new async detection; its lack of an
actionable consumer location remains a product limitation.

An initial comparison attempted to use the old single-consumer auditor, which
assumes `src/main.tsx`, and failed on this app's `src/index.tsx`. The new
`project-read-browser-audit-v1.mjs` compares the complete copied source tree.
The old auditor and earlier observations are preserved.

## Positive and negative runtime regressions

The exact original six RxJS, Neverthrow and Zod consumers run again with V3 and
V4. Both keep **3/3 target hints and 3/3 quiet, working controls**. Each replay
has four independently audited read witnesses. Original source, published
types, packages, displayed values, callback counts, native diagnostics and
caught exceptions match the earlier execution.

The hints remain `info`, `intent-open` and `staticDispatch: open`. Package
callback timing, arbitrary returned value flow and user intention stay open.
These are regressions on known cases, not a new fresh-case coverage score.
The earlier fresh result of 5/8 targets and 16/17 quiet controls remains
retained; that population is not replayed in this slice. Its three misses and
referenced-inspection noise remain recorded.

## Reproduction

Use fresh output names; evaluators refuse to overwrite earlier observations.

```sh
node benchmarks/reviewed-package-models/async-read-app-inventory-v1.mjs \
  rust/target/app-import-metric/metric.json \
  rust/target/async-read-real-app-inventory-replay.json

node benchmarks/reviewed-package-models/project-read-app-study-v2.mjs \
  rust/target/async-read-real-app-inventory-replay.json \
  rust/target/project-read-real-app-study-replay.json
```

For the real app, `project-read-browser-run-v2.mjs` accepts a fresh output
directory, Chromium executable and `plain`, `v3` or `v4`. Passing an optional
case-module path runs those consumers instead of the built-in portfolio flow.
The browser blocks external requests. Run `project-read-browser-audit-v1.mjs`
on plain and instrumented `browser/results.json` paths and a fresh output JSON.
The six-consumer study and audits reuse `async-read-study-v2.mjs`,
`async-read-audit-v1.mjs` and `async-read-parity-v1.mjs` with the original frozen
population. Real retained installations and browser tooling are required.

## Handoff and remaining work

Verification passes:

- **212/212 full prototype tests**, plus **4/4 additional session-boundary
  tests**; no skips. Twenty-two tests added in this slice cover project loading,
  invalidation, syntax gating and explicit refusal paths.
- Syntax checks for **269 modules**; 263 unique pins across 14 historical seals
  remain unchanged.
- Two full-app tree/behavior audits, two six-consumer behavior comparisons and
  two independent four-witness read audits.
- `make verify-fast`: producer freshness, Rust formatting and pinned workspace
  Clippy; schema parsing, dialect-manifest validation and `git diff --check`.

Full `make verify`, production coverage, ownership, contract corpus and
certification gates are deferred for this isolated prototype. No production
Rust source, contract, schema, public manifest or finding snapshot changes.
Research modules, README, this report and the precision backlog are the source
changes. Generated inventories, browser copies, measurements, audits and logs
remain under `rust/target`.

The shared loader and cheap gate improve the route to practical integration.
Actual real-app precision is still unproven: only one full app is exercised,
none of the five enrolled real-app reads has a runtime context in this slice,
and source enrollment finds no new async dependency loss. Cross-file native
accessors, package-returned accessors, callbacks created outside a memo, restored
owners, unknown argument flow and intentional snapshots remain open. Finding
and explaining bugs in more real app flows is the next substantive evidence;
an unchanged hint count cannot establish broad package coverage.
