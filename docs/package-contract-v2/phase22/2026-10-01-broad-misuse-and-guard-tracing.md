# Broader package misuse and runtime guard tracing

The broader experiment covers **24 authored defects and 26 valid controls**
across **18 directly exercised packages**, including two mutations in a copied
application. Runtime semantic diagnostics and exceptions identify **15/24**
targets. Generic dependency instrumentation supplies useful guard observations
for **eight more**, but those observations alone do not establish a defect.
Explicit behavioral expectations combined with runtime feedback expose **24/24**.

The frozen static prototype identifies only **4/23 eligible targets**. This
broader sample includes classes, object getters, proxies, callbacks, contexts,
components and asynchronous lifetime errors that the earlier narrow holdout
did not represent. Certification remains unnecessary for useful development
feedback, but the current source extractor cannot provide broad static coverage.

## What ran

`breadth-cases.mjs` defines 24 target/control pairs and two additional intentional
controls. Each pair changes one use and declares its expected update, lifetime
or context behavior. Fourteen Solid Primitives packages, router, Corvu dialog,
meta and query are directly exercised. The copied portfolio also uses
`solid-icons`; that dependency is not counted as a separately tested package API.

All 50 consumers pass strict checking against their actual published typings.
No fixture declaration stubs or type-invalid consumers contribute a target.
Library declaration internals use the oracle's existing `skipLibCheck` setting;
consumer errors remain enabled. The static analysis twins also pass published
typing for all 48 eligible consumers.

The browser ran every consumer twice: once with unchanged package code and
once with optional guard instrumentation. The 100 executions use retained
Chromium 151.0.7922.34, Playwright 1.63.0, Vite 8.3.0 and Solid's compiler plugin
3.0.0-next.44. Requests outside the local servers are blocked. No installations
or package source changes were made in the retained dependency trees.

Forty-eight consumers use Solid/signals/web 2.0.0-rc.9. The two query consumers
use their actual cached rc.4 runtime with `@tanstack/solid-query` 6.0.0-rc.0.
That runtime has no diagnostic subscription API and remains refused by both the
rc.9 static model audit and guard instrumentation. The compiler version is
recorded separately; successful finite query executions do not establish
general compatibility between that compiler and the older runtime.

The app trials copy the adapted cached `helge-dev` source. One mutation captures
`location.pathname` before the navigation memo; another captures `modalOpened()`
before the modal's conditional rendering. Original and mutated source digests
are retained. These are copied-app mutations, not newly discovered defects in
an untouched upstream checkout.

## Results by defect

“Guard note” below means informational evidence from an executed package branch.
The declared behavioral expectation establishes why it matters in that case.
“Static” counts only a finding introduced by the mutation, excluding findings
already present in its control.

| Target use | Direct runtime feedback | Additional quiet-path observation | Static detection |
| --- | --- | --- | --- |
| Capture static-store member | None | Tracking guard skipped | Miss |
| Capture derived-store member | Semantic diagnostic | — | Miss |
| Capture `ReactiveMap.get` result | None | Tracking guard skipped | Miss |
| Capture `ReactiveSet.has` result | None | Tracking guard skipped | Miss |
| Capture combined-props member | Semantic diagnostic | — | Miss |
| Capture element-size width | None | Tracking guard skipped | Miss |
| Capture element-bounds width | Semantic diagnostic | — | Miss |
| Capture mouse-position member | None | Tracking guard skipped | Miss |
| Capture pointer-position accessor | Semantic diagnostic | — | Miss |
| Capture media-query accessor | Semantic diagnostic | — | Source model warning |
| Pass current listener target instead of its accessor | Semantic diagnostic | — | Native finding |
| Create debounce in event handler; expect disposal cancellation | None | Cleanup guard skipped | Miss |
| Create throttle in event handler; expect disposal cancellation | None | Cleanup guard skipped | Miss |
| Create listener in event handler; expect component lifetime | None | Cleanup guard skipped | Miss |
| Create abortable after `await`; expect component cancellation | Semantic diagnostic | — | Miss |
| Write reactive state in lazy-memo callback | Diagnostic and exception | — | Miss |
| Emit bus in memo; listener writes reactive state | Diagnostic and exception | — | Miss |
| Read router context hook in event handler | Exception | — | Miss |
| Read dialog context outside provider | Exception | — | Miss |
| Render dialog trigger outside provider | Exception | — | Miss |
| Capture title text before head component | Semantic diagnostic | — | Native finding |
| Destructure query data before cache update | None | None; rc.4 refused | Refused |
| Capture pathname before app navigation memo | Semantic diagnostic | — | Miss |
| Capture open state before app modal conditional | Semantic diagnostic | — | Native finding |

Twelve targets emit semantic diagnostics and five throw exceptions; two belong
to both groups, giving 15 distinct targets. Twenty-one fail their declared
behavioral expectation. The remaining three are caught by their diagnostic or
exception. None of the 26 controls has a direct runtime diagnostic, exception
or failed behavioral expectation.

This is a paired authored mutation study. The same investigator wrote the cases
and expectations; it does not estimate precision or recall on an independent
package population. Expectations existed in the case source before execution.
The existing extractor, lowerer and specializer stayed frozen; their manifest
was recorded during the initial browser run, before the static pass. The new
guard instrumenter was developed after observing the quiet failures.

## Generic guard observations

`guard-trace.mjs` resolves exact imported symbols from `solid-js` and
`@solidjs/signals`. It instruments bounded source forms where:

- a false `getOwner()` guard bypasses a branch containing `onCleanup`;
- a false `getObserver()` guard leads to an early return in a function that
  also creates a core signal.

The instrumenter has no package-name rules. Aliased imports are supported;
shadowed or unrelated names are refused. It processes installed JavaScript
before other transformations, checks that the input is the actual source file,
and restricts instrumentation to the audited runtime version. An injected
expression evaluates the original guard once and returns its original value.

The observations explain five quiet tracking failures and three quiet lifetime
failures across different APIs. Each carries the original package source digest
and guard location, plus the consuming application's mapped stack location.
All 50 traced executions retain the same observed values, semantic codes and
exception messages as their uninstrumented twins. This checks finite execution
parity; instrumentation overhead and all possible program behavior are unmeasured.

The two extra controls are essential: an intentional static-store snapshot and
a scheduled background callback deliberately outliving a component also produce
guard notes. Those notes therefore have `severity: info`,
`basis: runtime-guard-observation` and `certification: false`. Turning them into
automatic misuse warnings would introduce noise. A false guard does not prove
that a caller wanted tracking, that cleanup was mandatory, or that all package
branches have the same behavior.

## Static precision and source attribution gaps

The fresh static pass performs 96 native analyses over the same browser
consumers, preserving uncertifiable external behavior. Three introduced native
findings and one source model warning detect four targets. No source model was
tuned to the misses. The model path still lacks useful premises for richer
returned objects, class methods, callback phase, contexts and component behavior.
Analysis twins are separate project roots whose cross-file imports retain the
original app graph; this is not a fully rewritten model of the application.

Both app controls already contain native findings. The navigation target adds
no new native finding, so counting any finding in that target would falsely
inflate detection. The validator compares rule, message, file and source bytes
against the control, allowing offsets to move after the mutation.

One existing `strict-read-untracked` claim points to `opened()` inside
`onClick={() => setOpened(!opened())}` in `NavBar.tsx`. The exercised menu flow
works and runtime feedback stays quiet in the control. This is evidence for
investigating a possible callback-phase misclassification, not a completed
precision fix. Another existing finding concerns `articles()` in `Blog.tsx`;
the blog path was not executed here, so this study cannot judge that claim.

All 12 semantic diagnostics in each browser run map to original app source,
as do all ten guard notes in the traced run. Four exception cases retain an
original consumer location. The dialog trigger exception has dependency frames
but no consumer frame; its source attribution remains incomplete. Returning no
invented consumer location is preferable to attaching the exception arbitrarily.

The query mutation remains entirely quiet in runtime findings and guard notes.
Its explicit cache-update expectation catches the stale display. Browser
execution only observes exercised paths; SSR, hydration, non-V8 stacks,
prebundled dependency instrumentation, async origin recovery and broad app
coverage remain open. Dependency columns in rewritten code do not have a new
source map; package premises use original source positions and consumer frames
use the existing app maps.

## Recommended direction

Build the development feedback path in this order:

1. Collect Solid semantic diagnostics and package exceptions during real app
   flows, retaining source attribution and observer capability limits.
2. Add generic guard observations as explanations attached to an observed
   update or disposal failure. Keep standalone guard notes informational.
3. Let application tests declare observable expectations: change input and
   verify the display; dispose a scope and verify its resources stop. Generate
   actions only where exact, isolatable call facts make that safe.
4. Extend static analysis around missing object, callback and lifetime semantics,
   measuring each extension against this corpus and valid intent controls.

This distributes the work across shared Solid mechanisms and application
expectations instead of requiring a complete contract for each package.
It supports broad executed feedback; it does not promise diagnostics for every
misuse in every package. Automatically inferring intended updates and resource
lifetimes remains the central unresolved step for unattended warnings.

## Evidence and verification

The ignored evidence roots are:

- `rust/target/reviewed-models-breadth-study.json`: frozen static input digests,
  corpus digest and authored expectation metadata;
- `rust/target/reviewed-models-breadth-browser-1/results.json`: 50 executions
  against unmodified package code;
- `rust/target/reviewed-models-breadth-browser-traced/results.json`: 50 traced
  executions and guard source premises;
- `rust/target/reviewed-models-breadth-static/results.json`: 96 native outputs,
  source models, unsupported cases and real typing checks;
- `rust/target/reviewed-models-breadth-handoff-validated.json`: final input,
  typing, source location, paired detection and execution parity replay.

Verification passed:

- all 26 prototype tests, including three guard instrumentation regressions;
- the saved breadth validator's input, typing, attribution and execution parity
  replay, using a fresh handoff manifest with the same frozen source digests;
- syntax checks for all 34 experiment modules;
- `make verify-fast`: current Type Facts stamp, Rust formatting and workspace
  Clippy with certification pins;
- schema parsing, dialect manifest validation, tracked diff whitespace and
  whitespace checks covering the untracked experiment files and reports.

The prototype and documentation are the only source changes. Generated browser
copies, analysis twins and raw reports are confined to ignored `rust/target`
outputs. Production rules, public schema, accepted contracts and fixture
snapshots remain unchanged. Full `make verify`, contract certification, fixture
coverage and ownership gates are deferred because this slice changes isolated
experiments and reports.
