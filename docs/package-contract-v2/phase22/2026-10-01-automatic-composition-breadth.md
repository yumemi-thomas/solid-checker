# Automatic composition: broader evidence and limits

Date: 2026-10-01. Implementation remains isolated in
`.claude/worktrees/codex-composition-callables` on `codex/composition-callables`.
The delivery branch's verifier and accepted tier have not changed.
Verified implementation commit: `ba83b3a9a`, on top of `2726e7f94` in the
isolated worktree. No experiment commit has been pushed.

The experiment now reaches a complete cursor node case with **zero authored
semantic proposals**, including its utils dependency. The same generator and
verifier improve four more packages' node exports. Across the seven runnable
packages below, node clean exports rise from 4/37 in the compiled tier to 14/37
in the experimental accepted graphs. Browser remains 0/37. This is evidence of
reusable composition, not evidence that every package or host can be certified.

| Published package | Node compiled tier | Node automatic graph | Browser automatic graph |
| --- | ---: | ---: | ---: |
| cursor 1.0.0-next.2 | 3/6 | 6/6 | 0/6 |
| connectivity 1.0.0-next.2 | 0/6 | 3/6 | 0/6 |
| media 4.0.0-next.2 | 0/6 | 0/6 | 0/6 |
| mouse 4.0.0-next.3 | 0/8 | 2/8 | 0/8 |
| orientation 1.0.0-next.2 | 0/2 | 1/2 | 0/2 |
| page-utilities 3.0.0-next.2 | 1/4 | 2/4 | 0/4 |
| interaction 1.0.0-next.4 | 0/5 | 0/5 | 0/5 |

Drag-drop 0.1.0-next.0 additionally refuses graph preparation on both hosts:
its published `solid-js/web` import is not exported by Solid rc.9. No consumer
finding or clean export is counted for that artifact blocker.

## What changed

The native generator requests exact callable facts for whole returned named
imports, only during contract generation. It proposes the existing inert
described-callable shape for exact callable references and offers a bounded
empty factory callbacks proposal where direct and parameter callback forms are
absent. Independent source censuses, mandatory runtime vetoes, exact graph edges
and all four authenticated child domains still decide whether any proposal
survives. No public claim form, value shape or Type Facts protocol changed.

The first automatic cursor trial reached 4/6 because the actual generation path
did not request returned-import callability. The corrected path reaches 6/6.
Both observations remain retained. Unknown callability, namespace/default/member
imports, written or shadowed bindings, missing identity and ambiguous spans
remain refused. Broader generic or captured-callback composition is not covered.

## Real consumers and controls

Automatic cursor passes six strict published-type checks and twelve consumer
analyses. Its three node consumers certify with accepted package evidence and
no findings. Its browser samples preserve exactly four existing SC4001 owner
violations, with none in the rooted twin. This adds no new misuse finding; it
shows that the generation gain does not erase those findings.

The breadth trial passes another 36 published-type checks and 72 consumer
analyses. Every runnable package has an unowned sample, a rooted twin and a
callback sample, using TypeScript 5.9.3 and the real package declarations with
Solid/signals/web rc.9. Published declaration defects are excluded using
skipLibCheck; strict checking still applies to each snippet. Connectivity,
mouse, orientation and page utilities certify all three selected node consumers
where baseline reported incomplete contracts. This is selected-use evidence;
the table accounts separately for every export, including incomplete ones.

Every breadth graph input is hashed before execution and checked unchanged
afterwards. No script supplies semantic edits to a dependency proposal. Native
outputs, withheld closures/operations, receipts, trust configuration and raw
consumer outputs remain in the evidence directory.

Six node call-time runtime controls pass with throwing DOM/element guards and
throwing supplied callbacks: zero callback executions and no guard reached.
The module is imported first under ordinary Node globals. Earlier observations
installed the guard before import, making utils' legitimate `typeof window`
environment check throw; those are harness defects and are **not package
counterexamples**. They remain preserved, with corrected observations in a
separate directory. Corrected modules are bound by source hash to their
accepted artifact case. Finite observations have no proof authority.

## Remaining walls

- Media's `makeMediaQueryListener` closes callbacks, creates and described
  returns, but reads remains withheld as `no recipe in corpus`. The same
  reason affects three other media exports. A passing external runtime sample
  cannot replace that mandatory gate. The exact synthesis blocker remains
  under investigation; a union-parameter explanation alone would not explain
  the zero-argument export.
  A separate authored control removes only `makeMediaQueryListener`'s two
  accessor bounds from the proposal. Native certification succeeds as a partial
  graph but emits the same incomplete accepted document; the export gains no
  closure. Removing these bounds therefore supplies no certification shortcut.
  Evidence is retained at `rust/target/package-composition-media-bounds-1/`.
- Interaction's `makeInteractOutside` keeps uncensused property reads,
  including nested parameter and local-binding derivations. It remains
  uncertifiable even though the sampled node branch is inert.
- Browser gains none. DOM/member behavior, callback attribution and remaining
  Solid reads rows still need proof. Returning a dependency's inert function
  does not prove its browser factory's other behavior.
- Complete all-host packages, criterion-3 coverage and app-import certification
  are unchanged. The ten additional node exports are experimental gains,
  not a promoted global checkpoint result.

## Reproducibility and verification

Evidence:

- `rust/target/package-composition-cursor-automatic-1/results.json`: initial
  generation refusal, 4/6 node.
- `rust/target/package-composition-cursor-automatic-2/results.json`: final
  automatic cursor, 6/6 node.
- `rust/target/package-composition-breadth-1/results.json`: all seven additional
  package attempts and both hosts; its original runtime guards are invalid.
- `rust/target/package-composition-breadth-call-time-1/results.json`: corrected
  source-bound runtime observations.

The four focused Rust regression tests pass. The first complete verification
run passed Rust/Go, coverage, ownership, performance and CLI before exposing
three intentional proposal snapshot changes. A diagnostic comparison inspected
all 120 fixtures before updating only those three: escaping-private-helper,
returned-callback-descendant and typefacts-implementation-transcript. Their
contracts and proposal plans now record the new candidates, with no accepted
authority. The full contract corpus passes: 170 artifact cases, 778 possible
operations, 2,246 proof candidates and 8,595 local open claims. No consumer
finding snapshots or accepted-tier artifacts changed.

The existing descendant-callback test was adapted to isolate its original
positive execution claim from the added negative factory candidates. A stronger
source control submits those new candidates together with their described
returns and requires a native census refusal for unsupported nested execution.
The focused test passes, including its original forged execution control.

Final `make verify` passes in the isolated worktree in **724.04 seconds**, exit
0, with `TOTAL` present and no `FAILED during step` marker. It passes 1,572 Rust
tests, Go race checks, Clippy/fmt, performance, 276 CLI tests, 375 scripts tests,
the 102-case TypeScript/checker oracle, six obligations and ten discharges.
Coverage remains 142 projects / 737 findings; ownership remains 41 cases / 465
ledger rows. Bundle conformance has zero active receipt-issued cases, so the
experiment's own graph and consumer admission tests remain its receipt evidence.
Log: `/private/tmp/solid-checker-automatic-composition-verify-green.log`.

Both cursor dependency refusal controls pass again against the new generator:
an open callbacks domain and a falsely proposed empty child return each leave
the two dependent exports incomplete. Together with automatic cursor and the
breadth trial, this turn passes **48 published-type checks and 96 consumer
analyses**. These control proposals are authored and excluded from the
automatic-generation gain. Their evidence is retained in
`rust/target/package-composition-cursor-automatic-open-control/` and
`rust/target/package-composition-cursor-automatic-withheld-control/`.

Promotion still requires the retained census/environment/
checkpoint/bundle generation chain and authentication of embedded bundles.
