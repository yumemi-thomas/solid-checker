# Package-directed pilot: complete surfaces and consumer limits

Three further offline trials test complete surfaces using existing claim forms:
`@solid-primitives/event-dispatcher@1.0.0-next.2` on node and browser, and
`@solid-primitives/platform@1.0.0-next.2` on node. All six consumer files pass
TypeScript 5.9.3 against published typings and Solid 2.0.0-rc.9. Twelve consumer
analyses compare the compiled tier with independently authenticated proposals.
Evidence remains in `rust/target/package-directed-completion/`.

**No additional complete package or misuse finding is recovered.** The initial
browser RAF ownership improvement remains the experiment's one additional
proven misuse finding. No accepted-tier artifact, schema, value-shape support,
analyzer code, snapshot or misuse-ledger entry changes.

## Complete-surface controls

| Trial | Authenticated surface | Consumer result |
| --- | --- | --- |
| Dispatcher, node | All four domains close; sole export is clean | Plain-handler consumer clean; reactive handler inside a props object remains SC1001 **uncertifiable** |
| Dispatcher, browser | Inert returned-callable claim withheld; export remains incomplete | SC9005 on both consumers; reactive handler also SC1001 **uncertifiable** |
| Platform, node | All 23 exports account for non-callable values | Consumer importing and using all 23 is clean |

These outcomes reproduce existing checkpoint coverage. Dispatcher is already
clean on node; platform is already complete on every host. The custom consumers
are controls, not newly claimed intrinsic misuse cases. The runner retains its
legacy `misuse`/`correct` observation field names, but neither platform specimen
is a misuse and the node dispatcher does not execute its handler.

The dispatcher proposal uses the existing `described-callable` return form
with empty reads and plain returns. Published code starts with
`if (isServer) return () => true`. Browser instead looks up the handler on
`props`, constructs a `CustomEvent`, and invokes the handler. Its refusal is
the necessary negative control: server authority does not generalize to browser.

Two additional runtime controls execute the exact published bytes with node
and browser resolution conditions. The props getter and handler are each called
zero times on node and twice on browser. Node returns `true` for both dispatches;
browser returns `true` for an ordinary event and `false` when the handler prevents
the cancelable event. The source hash matches the certification observation:
`1e370320a12da6b74b88794dd53ee5939373c387cc402e6d735afb0071e4b0ac`.
These samples falsify the browser's proposed inert behavior; runtime samples do
not supply proof authority or establish arbitrary callback behavior.

The node reactive-handler consumer passes types but still receives SC1001
uncertifiable even with all factory domains closed and an inert returned callable.
`callee_callback_timing_within` in `execution_role.rs` only proves external
callback timing through a direct accepted inline callback row or a supported
returned invoker. It does not use this contract to prove that a callback nested
in an object argument never executes. This is a consumer proof limit, not a
proven runtime violation. Closing a package's surface alone therefore does not
guarantee that all semantically safe consumers certify.

## Breadth of the remaining gaps

`inventory.mjs` recounts the retained checkpoint's 97 packages and 721 exports
per host, checks the headline totals, and records each input's SHA-256. It
deduplicates causes by exact package, entrypoint and export. Counts below are
exports with an open domain; one export may appear in several rows.

| Domain | None | Browser | Node |
| --- | ---: | ---: | ---: |
| Callbacks | 551 | 548 | 458 |
| Reads | 522 | 521 | 430 |
| Returns | 544 | 540 | 512 |
| Creates | 499 | 491 | 405 |

All four domains are open together on 465/454/374 exports respectively.
Only 16 browser exports have returns as their sole classified gap, compared
with 68 on node. This is an inventory, not an unlock forecast: a domain includes
many different obligations, and clearing it still requires proving each one.
Each host also has 36 exports with unclassified artifact/graph blockers; these
are retained separately and never treated as domain-only candidates.

Browser refusal families include recursive value shapes on 238 exports across
65 packages, reads through unknown property accessors on 129 across 50 packages,
and uncensused property-access invoking forms on 126 across 50 packages. Families
overlap, and a family label is not evidence that one proof change covers all of
its members. This makes a library of reusable, precise proof steps a better
implementation direction than independently authoring hundreds of proposals.

The next bounded candidate remains the listener's complete-path owner proof
described in the [previous extension](2026-10-01-package-directed-extension.md).
It uses an existing claim form and must distinguish both-branch registration
from missing-else, early-return, throw and unrelated-symbol controls. Returned
ordinary callable members remain a separate owner decision before implementation.

## Reproduction and verification

Use the environment in the pilot [README](../../../benchmarks/package-directed-pilot/README.md)
and append `completion` after a fresh output directory. `check-completion.mjs`
checks a saved record without repeating analysis. `probe-dispatcher.mjs` takes
that directory and requires the pinned probe Node binary. `inventory.mjs` takes
the retained checkpoint directory and a fresh output file.

The three certifications authenticate and select the exact environment; the
completion assertions pass, as do both runtime controls and the retained
inventory checks. Formatting, workspace Clippy, schema/manifest validation and
diff checks pass. Full verification, coverage/ownership reruns, tier regeneration
and checkpoint/app sweeps are deferred because product semantics and accepted
artifacts did not change. Experimental receipts remain outside the tier.
