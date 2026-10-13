# Automatic feedback, shared hooks and intent limits

This pass adds two automatic lifecycle findings and three optional performance
warnings, recovers the registration site of a late package error, and tests a
limit that additional package source analysis cannot remove: identical code can
be correct or incorrect depending on an intended update or lifetime that the
program never declares.

The source inventory finds exact core calls in **88 of 97 retained primitives
packages**. Shared runtime hooks therefore have substantial reach in this
corpus. That count is potential observability, not evidence that 88 packages
have complete misuse coverage. The existing broader corpus still has **15/24**
targets detected directly at runtime and **4/23** detected by the frozen static
path. The new cases expand the kinds of feedback, not those original counts.

## Additional automatic channels

`automatic-feedback-cases.mjs` defines 13 channel specimens and four intent
specimens. One channel target is rejected by the real published typings before
execution; 12 valid consumers run against unchanged package code and again with
optional origin tracing. The following findings need no test assertion about
the desired displayed value:

| Case | Observed code | Meaning |
| --- | --- | --- |
| `autofocus()` called in an event; its settled callback returns cleanup | `SETTLED_CLEANUP_UNOWNED`, error | The runtime cannot attach that cleanup to a lifecycle |
| Error boundary created in an event around a package memo read | `NO_OWNER_BOUNDARY`, warning | The boundary has no parent lifecycle to dispose it |
| Effect apply emits a bus value that changes its own tracked input | `EFFECT_WRITES_OWN_SOURCE`, warning | The effect needs another flush because of its own write |
| Effect apply dispatches to a listener that changes its tracked input | `EFFECT_WRITES_OWN_SOURCE`, warning | The same feedback pattern crosses a different package API |
| Lazy memo repeatedly returns an equivalent fresh object | `UNSTABLE_MEMO_OUTPUT`, warning | The memo keeps notifying subscribers for an equivalent output |

All seven valid channel controls are quiet. The autofocus control with no
`autofocus` attribute is important: the unowned call itself does not inevitably
return a cleanup. Its executed branch remains valid and quiet. The experiment
does not turn a package name or a call location into unconditional proof.

The first two findings are lifecycle diagnostics from the installed runtime.
The three graph findings are explicitly advisory. Both effect twins eventually
render the same value, and both memo twins render the same parity. A performance
warning is useful feedback without proving that the final result is incorrect.
The extended collector preserves the runtime's severity and labels advisory
records separately; all observations retain `certification: false`.

### A candidate belonging to TypeScript

The plain store transaction candidate is excluded:

```text
TS2345: Argument of type '(draft: { count: number; }) => Promise<void>'
is not assignable to parameter of type
'(state: { count: number; }) => void | { count: number; }'.
Type 'Promise<void>' is not assignable to type 'void | { count: number; }'.
```

The actual `setStore(async draft => …)` consumer fails published typing. It
never executes and does not contribute a checker finding. `ASYNC_STORE_SETTER`
is not added to this collector. This case does not establish that TypeScript
can reject every dynamically concealed async callback; it establishes the
boundary for this exact ordinary typed consumer.

## Runtime identity is a required input

The first attribution run requested the engine but obtained zero records.
The live probe showed `OBSERVE.attribution.installed === null`. Vite's optimizer
had bundled Solid and web together, while the separately imported attribution
entry used a different copy of their shared internal state. Package versions
alone did not reveal the split.

Excluding core runtime entries from prebundling in this experimental profile
lets the engine attach to the actual app runtime. The bus/listener trials then
record two runs each and emit their warnings; the memo trial records 12 runs
and emits its warning at the runtime's default threshold. No threshold was
lowered to manufacture a finding. The runtime capability probe is checked in
the validator; a requested option alone cannot make the run count as observed.

This is a demonstrated harness configuration. A production integration needs
to work with ordinary dependency optimization while preserving one runtime
identity, or expose supported hooks through the already loaded core. Shipping
an observer without checking that identity can silently lose entire channels.
Prebundled/minified artifacts also hide the original files from the bounded
source instrumentation.

## Recover a late error's registration site

An unowned autofocus call queues `onSettled`. Its error is thrown later, after
the original consumer and package call have left the stack. The unchanged-code
run detects the error but has no consumer location.

`origin-trace.mjs` resolves exact imported `onSettled` symbols in dependency
JavaScript and wraps their callbacks to retain registration frames. A callback
wrapper alone is insufficient: the cleanup error is thrown by the runtime
*after the callback returns*. A narrow optional trace in the published rc.9
runtime's out-of-band settled branch keeps the origin active through that
validation. It resolves the actual callback parameter and the diagnostic site,
retains the original return/throw behavior, and records original source digests.
Retained dependency files are unchanged; the server transforms executed bytes.

The late error now maps to the exact `autofocus()` registration in the original
consumer. The trace also retains the thrown failure with that origin. All 12
channel twins preserve observed DOM values, diagnostic codes, severities and
exception messages/counts. Four focused tests cover aliases/shadowing, argument
evaluation, callback receiver and return value, thrown identity, and restoration
of origin after a returned cleanup is rejected.

This is a bounded trace for one synchronous scheduler branch, not a general
async provenance implementation. Origins do not follow `await` or unrelated
promises. Namespace/computed dispatch and independently tracing nested callback
registrations are unsupported. The core trace
is coupled to the audited rc.9 source shape and must not be assumed valid for
another release. Original consumer maps are used; rewritten dependency columns
are not provided with a new map. Guard and origin source profiles currently run
separately to avoid interpreting a prior transform as installed source.

## An intent limit demonstrated with identical code

The four intent trials form two pairs. Each pair has byte-identical application
source, identical input changes and disposal actions, identical actual output,
and identical guard observations. Only the externally declared requirement
differs:

| Same executed program | Actual result | Requirement making it a defect | Requirement making it valid |
| --- | --- | --- | --- |
| Capture static-store member before rendering, then change it | Display remains 0 | Display should follow the update to 1 | Display should retain initial snapshot 0 |
| Create/schedule debounce in a callback, then dispose component | Callback fires once | Work should stop at component disposal | Work should finish in the background |

More source, types and execution traces cannot distinguish these pairs without
some additional statement of intent. A blanket warning would also flag the
valid twin. Keeping every guard note informational would miss the unwanted
behavior unless a test or declared policy supplies the expectation.

This does not mean tracking and ownership diagnostics are generally ambiguous.
Many constraints are explicit in the runtime's execution model or a package's
actual checks. It means universal correctness for arbitrary snapshots and
resource lifetimes cannot be inferred from these available inputs alone.

A project could opt into conventions such as “UI-derived values stay live
unless explicitly sampled” and “component work stops at disposal unless
explicitly detached.” Those conventions reduce repeated expectations to a few
intent markers. This pass has not implemented or measured those policy warnings;
the observations must not be promoted to proven violations by assumption.

## Breadth replay and error delivery

The expanded collector, shared core and origin trace also execute all 50
existing breadth consumers. Their observed values and sets of exception messages
agree with the original run. All 48 rc.9 consumers have a live attribution
engine. Query's rc.4 pair has no diagnostic subscription and is excluded from
the rc.9 tracing capability. No new advisory appears on this corpus; none of the
26 controls gets an execution finding, exception or advisory. Direct target
detection remains 15/24. The nine previously quiet failures remain quiet in
these automatic channels.

**Full error delivery parity is not established.** Playwright reports the same
dialog context exception twice instead of once in the two failing wide cases.
The validator records this failed dimension explicitly. Six focused executions
separate unbundled-core, attribution-only and origin-only profiles. Each captures
one browser `window.error` event, while Playwright delivers one or two records.
That is evidence of transport/reporting duplication in these focused cases,
not a second distinct violation. The exact cause of the delivery variation
remains open. A product should retain error identity and source provenance;
deduplicating every equal message would incorrectly merge distinct failures.

## What the source inventory establishes

`mechanism-inventory.mjs` follows the browser root entry's local ESM module graph
and resolves exact core import symbols, including aliases. It counts direct call
sites in parsed source, including calls in exports that may never execute.

| Inventory dimension | Packages |
| --- | ---: |
| Retained primitives artifacts | 97 |
| Resolved browser root module graphs | 94 |
| At least one exact direct core call | 88 |
| Signal/memo/store/projection producers | 77 |
| Effect/cleanup/settled/root/owner operations | 65 |
| Owner/observer guard calls | 14 |
| Direct `onSettled` calls | 9 |

The groups overlap. Six resolved graphs have no direct core call, including map
and media, which delegate to other packages. They are not classified as
nonreactive or safe. Animation, controlled-props and virtual refuse because the
retained root entry is missing; no empty claim is supplied. Subpaths, external
dependency graphs, dynamic loading and executed-export coverage are outside
this inventory. It supports the reach of shared mechanisms within these retained
primitives artifacts, not a statement about most npm packages.

## Feasibility picture

| Feedback | What is possible now | What remains unavailable |
| --- | --- | --- |
| Core execution/ownership constraints | Automatic feedback on exercised calls, even through package wrappers | Paths that never execute; wrong/missing runtime instance |
| Package context exceptions | Capture actual thrown failures without a package contract | Missing consumer origins in some stacks; exceptions caught privately by packages |
| Reactive graph inefficiency | Optional shared engine detects selected cycles and unstable outputs | A proof that every advisory is an application defect; default production integration |
| Skipped tracking or cleanup guards | Generic source instrumentation gives exact executed branch facts | Intended update/lifetime; arbitrary or prebundled source forms |
| Static package misuse | Bounded exact source models and current analyzer can diagnose supported semantics | Broad object/class/callback/context semantics; current corpus is only 4/23 |
| Desired output or disposal | Explicit behavioral tests expose quiet failures | Automatic inference of arbitrary application requirements |
| Every misuse in arbitrary packages | No evidence supports this guarantee | Undeclared intent, unexecuted paths and package-specific semantics remain missing inputs |

The strongest practical direction remains a shared development observer with
source attribution, exact static facts for unexecuted code, and explicit intent
where update/lifetime conventions matter. Package certification should not be
the prerequisite for the runtime path. Wider static semantics and the cost and
precision of project policies remain investigation work.

## Existing development tooling to reuse

The retained `@solidjs/vite-plugin` 3.0.0-next.44 already implements a diagnostics
bridge integration. Its source enables the bridge automatically when the app
declares `@solidjs/diagnostics`, supports an explicit `diagnostics: true` override,
injects that app's browser bridge, and exposes `/__solid/diagnostics` for session
control, `whyDidRun` and costs. It also explicitly prebundles the bridge entries
to avoid discovering them after the page has loaded.

This is inspected publisher implementation, not an executed bridge trial. No
`@solidjs/diagnostics` installation is available in the retained app/primitives
roots checked here. That external artifact remains an unmeasured integration
input; no substitute bridge was presented as the publisher's implementation.
The inspected plugin file has digest
`sha256:e08f985b473fd2e5baecb9a20648628cba7db4a573245d2695f4519106ef5d5c`.

The first product step should evaluate this existing bridge with authenticated
matching artifacts. The experimental collectors establish feedback behavior
and gaps; they are not a reason to recreate session transport already supplied
by the publisher. Whether the existing bridge handles every diagnostic category,
module-identity condition and source origin measured here remains to be tested.

## Evidence and handoff

The final validator covers 84 fresh browser executions: 24 channel twins, four
intent trials, 50 breadth cases and six delivery-isolation trials. Two additional
specimens were rejected by typing before execution. The earlier inactive-engine
trials and the incomplete browser-path launch are retained as failed/debugging
attempts and do not contribute coverage.

Key ignored evidence paths under `rust/target/`:

- `automatic-feedback-study-final.json`: pre-execution source/input digests and
  labels for all 17 specimens;
- `automatic-feedback-browser-shared-original/results.json` and
  `automatic-feedback-browser-shared-core/results.json`: unchanged/traced channel
  twins, typing results and actual engine capability;
- `automatic-feedback-browser-intent/results.json`: identical-code witnesses;
- `automatic-feedback-mechanism-inventory.json`: 97 artifact inventory;
- `automatic-feedback-breadth-shared-core/results.json`: broader replay;
- `automatic-feedback-error-delivery-*/results.json`: focused delivery evidence;
- `automatic-feedback-validated-final.json`: source authentication, reconstructed
  origin sites, channel controls, intent witnesses and explicit parity limits.

Changes are confined to the isolated benchmark prototype and documentation.
Production rules, accepted contracts, public schema and fixture snapshots remain
unchanged. Verification passed:

- all 30 prototype tests, including four new origin/diagnostic regressions;
- the final source, typing, channel, intent and breadth validator, with failed
  error delivery parity retained explicitly in its output;
- syntax checks for all 42 experiment modules and whitespace checks for the
  untracked experiment files and reports;
- `make verify-fast`: current Type Facts stamp, Rust formatting and workspace
  Clippy with certification pins;
- schema parsing, dialect manifest validation and tracked diff whitespace.

Full `make verify`, contract certification, fixture coverage and ownership gates
are deferred because this slice changes isolated experiments and reports.
