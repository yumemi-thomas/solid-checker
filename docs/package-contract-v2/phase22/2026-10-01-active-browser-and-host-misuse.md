# Active browser behavior and host-specific misuse

The next experiment fixes one scheduling defect and replays the automatic
composition matrix. It also checks three active listener implementations and
four existing misuse-ledger cases against the exact published typings.

**No additional clean export, complete package, or misuse finding is gained.**
The six-package runnable matrix remains 8/31 clean on node and 0/31 on browser;
drag-drop still refuses its invalid published dependency graph on both hosts.
Cursor's earlier 6/6 node result is outside this replay. The production tier,
global checkpoint, app metric and misuse ledger are unchanged.

## A scheduling defect fixed without relaxing proof

The isolated implementation on `codex/composition-callables` adds ADR 0176.
Accessor-bounded reads are first censused, then deferred for their mandatory
runtime veto. A stored missing-recipe record previously took precedence over
the synthesized recipe on every later pass. A matching recipe can now schedule
both the implementation census and veto again. A census refusal or incomplete
veto stays withheld, including when preceded by a missing-recipe record.
No public claim form, value shape, schema or producer protocol changes.

The new scheduling regression fails under the old precedence. A full proof
test closes an unaffected arithmetic export against the audited rc.9 tracking
runtime with a nonempty gate root; its getter-reading sibling stays refused.
The 43 focused reads tests pass.

The published replay performs 36 strict TypeScript checks and 72 consumer
analyses. Surfaces and consumer findings remain identical to the previous
automatic matrix. Six media read refusals now expose their actual census
blocker: four have an unbounded accessor-installation target and two have an
unavailable implementation transcript. The earlier missing-recipe explanation
was insufficient. Supplying a veto cannot repair these proof obligations.

An authored diagnostic additionally removes **all** media accessor bounds,
instead of only the maker's shared summary. It emits the same incomplete main
document as the earlier target-only control. The reads candidates disappear
without those bounds; absence of a withheld record is not closure.

## Active listener samples

Six source-bound observations use Node with simulated browser globals and
native EventTarget, under node/browser export conditions. Each observation
tests explicit cleanup and a root-owned call: twelve scenarios total. They
are runtime samples, not real-browser conformance or certification authority.
The owner's runtime file is selected from the published rc.9 manifest and
its SHA-256 is recorded beside the root artifact's SHA-256.

| Export | Node registration / callback | Browser registration / callback | Browser root disposal |
| --- | --- | --- | --- |
| connectivity.makeConnectivityListener | 0 / 0 | 2 / 1 | removes listeners |
| media.makeMediaQueryListener | 0 / 0 | 1 / 1 | removes listener |
| orientation.makeOrientation | 0 / 0 | 1 / 1 | listener persists until explicit cleanup |

Every explicit cleanup prevents subsequent delivery. An unowned maker with
explicit cleanup is legitimate in these samples. A blanket owner requirement
would misclassify it. Browser callback delivery also falsifies transferring
the node inert-callable behavior to browser.

## Four contradictory node ledger expectations

The exact unchanged ledger snippets are checked with TypeScript 5.9.3 and
the retained published packages on Solid 2.0.0-rc.9. Eight strict typings checks
and sixteen consumer analyses use the same `--runtime-target` option as the
checkpoint, with the new experimental receipts admitted by the consumer.

| Existing case | Node behavior proven by the experimental receipt | Ledger expectation |
| --- | --- | --- |
| connectivity-createConnectivitySignal-top-level-read | returns inert trueFn; no reactive read | strict-read-untracked violation |
| connectivity-createConnectivitySignal-module-scope | no owner registration | missing-owner violation |
| page-utilities-createPageVisibility-top-level-read | returns inert trueFn; no reactive read | strict-read-untracked violation |
| page-utilities-createPageLeaveBlocker-module-scope | returns without registering work | missing-owner violation |

Both labelled twins certify with no findings on node. The ledger marks each
`misuse silent`, because it requires the same violation on every named host.
Reporting these defects would contradict the certified node implementation.
The initial replay used `--conditions`; the final replay uses the checkpoint's
`--runtime-target` and confirms the same result. It adds no new misuse finding.

Browser twins remain incomplete: the first three pairs have SC9005 on both
twins; page leave's purported misuse additionally has two existing SC4001
uncertifiable findings, while its root twin has only SC9005. This does not
satisfy criterion 3. These are genuine browser proof gaps, separate from the
incorrect node expectations.

The corrective benchmark change should restrict a misuse expectation to hosts
where the defect exists, retaining the inert node specimens as explicit clean
controls. Host-free uncertainty also needs its own expected finding kind;
it cannot automatically inherit a browser violation. The experiment leaves
the ledger unchanged and claims no resulting metric improvement.

## Evidence and verification

All inputs are retained locally; no packages were installed for this experiment.

- `rust/target/package-composition-deferred-reads-1/results.json`: automatic
  replay; `comparison.json` asserts unchanged surfaces/findings and records
  the six improved refusal explanations.
- `rust/target/package-composition-media-all-bounds-1/results.json`: authored
  diagnostic only, no automatic gain or proof authority.
- `rust/target/package-composition-active-listeners-2/results.json`: final
  six observations and twelve scenarios, source/runtime digests recorded.
- `rust/target/package-composition-host-misuse-2/results.json`: final eight
  type checks, sixteen consumer analyses and four contrary ledger verdicts.

The replay's original call-time node observations pass with zero callbacks.
The saved comparison reuses those observations rather than rerunning unchanged
runtime commands. Earlier active-listener and host-misuse runs are preserved
as historical evidence; the `-2` records are the final harness versions.

Full verification of ADR 0176 passes in the isolated worktree: exit 0,
TOTAL 846.85 seconds, no `FAILED during step`. It passes 1,574 Rust tests,
Go race tests, formatting, Clippy, the 142-project coverage comparison (737
findings), ownership (41 cases / 465 ledger rows), performance, CLI (276),
ecosystem tests (319 passed / one skipped), scripts (375), the contract corpus
(120 fixtures), and the TypeScript/checker oracle (102 cases / 27 keystones).
The conformance gate still has zero active receipt-issued legacy bundle cases;
the native graph admission and consumer checks supply the experimental receipt
evidence. Log: `/private/tmp/solid-checker-deferred-reads-verify.log`.

No finding snapshot, accepted-tier bundle, schema, misuse-ledger entry or
protocol changes. The implementation remains experimental; tier promotion
still requires the complete census/host/checkpoint/bundle pipeline.
