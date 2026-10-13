# First development-feedback implementation

## Outcome

The reusable CLI now exposes `solid-checker feedback` and an input-bound capture
format. It runs native analysis of the original project and separately admits
recorded application reads, exceptions and assertion failures. The native fact
owners retain responsibility for semantics. This is the first implementation
slice; automatic browser collection, package source models and the research
warning selector still need production integration.

The Solid RC.13 candidate also runs through the whole checker in the local
`rust/target/local-rc13-checker` checkout. Its exact compiler revision comes
from a local Git repository. Remote publication is unnecessary for this
development integration. Production compiler pins and the published-runtime
audit remain unchanged.

## What changed

- `packages/cli/scripts/development-feedback.mjs` owns capture admission and
  composition. Top-level `findings`, `gaps` and `observations` separate proven
  native violations, native proof obligations and recorded runtime events.
- The capture binds the configured TypeScript program's source/declaration and
  resolution inputs, including negative file checks and directory listings.
  Inputs are validated before and after native analysis. Runtime inputs bind
  supplied untracked-read records to exact bytes. Changed inputs refuse reuse.
- TypeScript checks installed declarations. Inputs with typing errors receive
  no additional capture guidance. The adapter does not infer package behavior
  from TypeScript syntax. Native findings retain their original classification.
- Repeated equivalent records retain all event IDs and an occurrence count.
  Untracked reads are information because reactive intent remains open.
  Supplied exceptions and assertion failures use their own error channel.
  Captures do not authenticate event occurrence or issue contract authority.

The [usage guide](../../development-feedback.md) explains the command and
capture format. Native analysis owns the exit status; captured observations
do not create a new CI failure policy.

## Precision fix found by the implementation

The eight real installed-package applications all passed their published
typings, but native analysis initially reported a pending-read violation at
their readiness callback:

```ts
h.probePending = () => h.checkPending(() => result());
```

The callee's invocation and pending handling were unresolved. ReactiveRead
already retained callee callback timing uncertainty, but AsyncRead dropped it.
The fix propagates that fact for accessor and async store reads, then projects
the affected SC5001 result as uncertifiable with explicit open-context wording.
Direct component-body reads and callbacks invoked by exact synchronous helpers
remain violations.

The regression also exposed the native `isPending` accessor guard exemption.
The dialect now answers that question explicitly. The exact primitive's direct
callback has no strict pending-read error; a shadowed local function does not
inherit the exemption. Subscription findings retain their own facts. Store
proxy guards and nested callbacks do not inherit the accessor exemption.

The fixture adds two unresolved callbacks, four proven pending reads, direct
and namespace pending probes, and a shadowed function. All new cases and the
cross-file helpers pass TypeScript 5.9.3 against actual audited Solid, signals
and web `2.0.0-rc.9` declarations: zero errors. Reduced fixture stubs do not
supply that evidence.

## Application comparison

The bridge validates the retained Chromium report and each final stage's exact
source/resolution manifest before admitting its native read observations. It
reanalyzes original source with the local RC.13 checker. Browser execution
remains the recorded RC.9 execution; this comparison does not execute browser
code compiled with RC.13.

| Measurement | Result |
| --- | --- |
| Applications | 8 |
| Published typing errors | 0 |
| Native findings | 18 |
| Proven native violations, before the fix | 8 |
| Proven native violations, after the fix | 0 |
| Recorded read observations | 8 informational records |

Each application retains its pending-handler proof obligation and package
contract gaps. The eight raw read records include the four target applications
and four controls. They are not eight defects. The historical selector remains
unchanged: its four targets and two identity controls receive hints, while two
named allocation controls stay quiet. This implementation does not claim an
improvement to that selector's accuracy.

The final replay records the feedback adapter, launcher, native checker and
Type Facts binary digests and checks them throughout execution. Application
times here overlap repository gates and are not controlled speed measurements.

## RC.13 integration compatibility

The candidate needs updated compiler identities in Cargo, the compiler adapter,
dialect manifest, conformance record and the private-child identity test. The
local dependency and all three fork revisions agree. Type Facts stays local.

RC.13 rejects the former `<keygen>` fixture markup before producing facts.
That case was removed from the successful-facts fixture. A separate five-case
check preserves the malformed-markup refusal against both the candidate and
the actual published RC.13 compiler, alongside four successful output matches.
The resulting cleanup span changes from 2497–2506 to 2312–2321. The only other
snapshot update adds the eight findings in the new callback fixture source.

The candidate bundle and rendered checker integration patch survive build-tree
cleanup. Browser replay inputs remain local; restoring the compiler alone does
not restore or refresh historical application observations.

## Verification and remaining scope

Detailed check results and identities are retained in
`benchmarks/compiler-facts/rc13/checker-integration-evidence.json`. The first
RC.13 integration passed full `make verify` before the precision change.
The final full verification passes in 704.25 seconds with 1,565 Rust tests,
292 CLI tests, 102 TypeScript oracle cases, 41 ownership cases and the
performance, contract and obligation gates. Two additional capture tests then
pin successful exception/assertion admission, bringing the CLI suite to 294
tests. Main-checkout build, workspace Clippy, formatting, schema/manifest,
coverage and ownership checks also pass with its production compiler pin.
The CLI's eighteen capture/admission tests and the armed callback process
regression pass. Focused IR tests pass, and coverage compares 142 projects with
745 findings after the new cases.

Remaining open paths include unknown callback context/pending handling, package
contract gaps, stored callback invocation, runtime collector authentication,
browser condition binding and atomic shared Type Facts/runtime sessions.
The compiler still refuses unsupported fact modes and has partial generated
operation enumeration. No contracts, receipts or compiler output semantics
were changed by this implementation. The next slice is the live collector and
test-assisted feedback selection, with fresh positive failures and quiet
snapshot controls.
