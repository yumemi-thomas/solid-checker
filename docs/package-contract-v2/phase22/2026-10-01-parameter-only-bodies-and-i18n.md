# Parameter-only bodies close three i18n exports per host

The exact published `@solid-primitives/i18n@3.0.0-next.4` graph on Solid
2.0.0-rc.9 now certifies `template`, `identityResolveTemplate` and
`missingKeyAsPath` on none, browser and node. The automatic browser/node
comparison moves **0/12 to 3/12 clean exports**; the additional host-free run
also has 3/12. **The package remains incomplete, with nine other exports open.**
The production tier, global checkpoint, app metric and misuse ledger are
unchanged. These receipts and implementation changes remain experimental.

## A positive body proof instead of missing reachability

All three bodies only return a formal parameter. Their sole blocker was the
package-wide accessor-installation hazard from `proxyTranslator`: its target
and readers remain unbounded. Silence about those readers is not evidence.

ADR 0177 instead proves the complete body of each exact implementation from
authenticated runtime JavaScript. A closed grammar admits plain parameters
without defaults or rest and a single return of one of those same lexical
bindings. Parentheses and concise arrows are transparent. Defaults, getters,
`in`, coercions, calls, writes, captures, async returns, generators and extra
statements refuse. This establishes that the call performs no operation on an
accessor target, regardless of what an unrelated target can do.

The ordinary producer census, closure-hazard/site bindings and mandatory
runtime veto still run. A source-bound witness records the exact function,
parameter index, binding and reference spans. Aliased implementations and
construction invocations remain outside the proof. Nothing is inferred from
an empty invocation-form list; the proxy `in` control demonstrates why that
would be unsound. No public claim form, value shape, schema or producer
protocol changes.

The native synthetic test closes two parameter-only exports with a nonempty
gate root, while aliases, captured values, getter reads, proxy operations,
evaluated defaults, helper calls and async returns stay open. Three parser
tests separately require exact spans and lexical parameter identity.

## Automatic proposals and real consumers

The browser/node before and after runs use byte-identical native-generated
proposals, authenticated exact archives and existing graph/receipt admission.
No proposal was authored or edited. The nine other export surfaces and root
refusals remain unchanged, comparing exact claim addresses and reasons after
normalizing only private producer-project names. Main-document digests change
with the new closures and are deliberately not treated as stable observations.

Fifteen strict TypeScript 5.9.3 checks against published typings precede thirty
consumer analyses across the two-host before/after and host-free extension.
The final nine receipt-consuming specimens certify with no findings. Their
compiled-tier baselines retain three SC9005 findings each. One initial
callback specimen was rejected by the published string signature; it was
corrected before the comparison, and that failed run is preserved separately.

Three additional component twin pairs pass the real typings and exercise a
direct signal read passed to each newly clean export. On every host, the
untracked read reports exactly SC1001 / `strict-read-untracked` / violation;
the JSX twin certifies with no findings. These nine violations are the
existing direct-read rule, preserved as the contract becomes usable. They
are not new findings derived from hidden package behavior.

The unchanged `i18n-translator-argument-read` ledger pair remains uncertifiable
on all three hosts, with two SC9005 findings on both twins. This slice gains
**no criterion-3 case**. The twin replay adds eight strict typings checks and
twenty-four consumer analyses.

The six-package runnable breadth replay remains **8/31 on node and 0/31 on
browser**, with identical consumer findings to ADR 0176. Drag-drop's invalid
published `solid-js/web` graph is still refused on both hosts. This replay
adds thirty-six strict typings checks and seventy-two consumer analyses:
fifty-nine successful typings checks and 126 analyses across this slice's
complete experiments, excluding the initial invalid specimen.

## Evidence and remaining work

Saved evidence is under ignored build directories, preserving every prior run:

- `rust/target/package-composition-passthrough-before-2/results.json`:
  complete browser/node baseline.
- `rust/target/package-composition-passthrough-after-1/results.json`:
  automatic browser/node gains; `comparison.json` checks proposals, surfaces,
  remaining refusals and consumers.
- `rust/target/package-composition-passthrough-none-1/results.json`:
  host-free extension.
- `rust/target/package-composition-i18n-consumers-1/results.json`:
  component twins and unchanged criterion-3 control.
- `rust/target/package-composition-passthrough-breadth-1/results.json`:
  six-package replay and drag-drop refusals; `comparison.json` asserts
  unchanged surfaces and findings.

The comparison scripts validate saved observations and confer no authority.
Certification comes from the native transaction and consumer receipt admission.
No finding snapshot, accepted bundle, embedded tier ledger, public schema or
misuse expectation changes.

This is a reusable proof with a measured browser gain, but its grammar is
deliberately small. It does not solve i18n's callback/coercion/property-reader
implementations or establish a route to every package. Promotion still needs
the complete coverage census, environment runs, checkpoint, accepted-bundle
generation and embedded authentication gate.

Full `make verify` in the isolated implementation worktree passes with exit
0, **TOTAL 860.71 seconds**, and no `FAILED during step`. It includes 1,578
Rust tests, Go race tests, formatting, Clippy, the 142-project coverage
comparison (737 findings), ownership (41 cases / 465 rows), performance, CLI,
ecosystem and scripts tests, 120 contract fixtures, the 102-case TypeScript/
checker oracle (27 keystones), and the obligation audit. The conformance
gate still has zero active receipt-issued legacy bundles; the new graph and
consumer runs supply experimental receipt-admission evidence.
Log: `/private/tmp/solid-checker-passthrough-verify.log`.

The full checkpoint and accepted-tier regeneration were deferred because this
slice evaluates the isolated implementation rather than promoting bundles.
Implementation: `124c3fd58` on `codex/composition-callables`, in
`.claude/worktrees/codex-composition-callables`; no implementation or bundle
promotion to the lead branch and nothing pushed.
