# Published JavaScript probe unlock

Date: 2026-09-04. Branch: codex/phase19a-authenticated-proof-policy.
Starting HEAD: 80d2a81e97fcdf38fe329f7896ebeb6cc9cd5501. No commit or push.

Five creates domains in Kobalte 0.9.2's explicit browser root JavaScript case
now certify: clamp, isArray, isFunction, isNumber and noop. Each has its own
complete implementation census and completed mandatory contradiction veto.
This does not certify the package as a whole or replace a TypeScript-source
artifact case. All blockers are not resolved.

## Decision and guarantee

ADR 0009 measures 40/40 source candidates with same-named published JS
siblings, but 0/40 with an equivalence proof permitting substitution. Keep those
source gates refused. Certify independent JS cases with their own identities.

| Option | What remains proved | What is lost / cost | Disposition |
| --- | --- | --- | --- |
| Keep source refusal | A completed veto observes the selected published bytes | 40 source candidates stay unavailable | Retained |
| Census-only closure, veto withheld | Only the census's static conclusion | Independent runtime veto; requires a distinct consumer-visible authenticated policy | Rejected |
| Substitute a published JS sibling | Behavior observed in that sibling | Any statement about the source case; 40/40 names, 0/40 admissible substitutions | Rejected as substitution |
| Pinned, digest-bound transpilation | Identity of derived bytes and transformer | Erasure/lowering/bundling can hide initialization, resolution and execution contradictions; digest is not equivalence | Rejected |
| Independently certify published JS | Same published-byte veto guarantee for that exact JS case | No claim about sibling source or other conditions | Implemented; five new closures |

The veto still provides a finite opportunity to contradict an independently
proved claim in the exact authenticated runtime case. Non-observation never
proves completeness. Recipes observe own-global-key additions during finite
calls; they do not cover arbitrary allocation, pre-recipe initialization,
every input, DOM behavior or OS-level isolation. No weaker transformed-byte or
census-only certification mode was introduced.

Sandbox scheme 6 becomes 7 in ADR 0021 because the canonical copied dependency
manifest now participates in probe identity. Graph receipts authenticate the
source of those snapshots. Each dependency's selected target is checked against
the pinned Node's real condition set. The earlier default-condition graph
correctly refuses because Node selects solid-js dist/server.js where that
transaction analyzed dist/solid.js. Explicit browser selects the matching bytes.
No interpreter/harness pins, 0700 permissions, env_clear/allowlist, process-group
cleanup, framing, primordials, prototype freezing, reported-resolution check or
detect-and-refuse write protection was relaxed.

## Checker corrections, in decision order

| ADR | Correction | Measured next result |
| --- | --- | --- |
| 0015 | Exact returned callable identity, without treating descendants as returned | Passes indexArray; reaches onCleanup |
| 0016 | Symbol-bound unchanged parameter return fact; protocol 16 | Passes onCleanup; reaches onMount |
| 0017 | Withhold self-bootstrapped callback proposals | First accepted open graph: 20 nodes, 13 archives |
| 0018 | Acquire static runtime imports as well as re-exports | 78 nodes, 13 archives; 32 root creates candidates across two cases |
| 0019 | Forward recipe configuration into both graph request shapes | Gates actually scheduled; createGenerateId census refuses coercion |
| 0020 | Compose live-verified independent creates census while authenticating dependency receipts | Passes parent/dependency claim-ID mismatch; reaches workspace |
| 0021 | Materialize authenticated graph snapshots and bind them in scheme 7 | Explicit browser graph: 39 nodes, 13 archives, five completed gates |
| 0022 | Backtrack declaration conditions after missing declaration candidate | Node graph reaches 42 nodes, 15 archives; seroval createReference refuses |
| 0023 | Separate retained values from deferred invocation | Passes createReference; reaches seroval-plugins AbortSignalPlugin shape refusal |
| 0024 | Withhold false mutated/defaulted parameter identities; preserve initializer facts | Restores alpha certification without weakening the identity verifier |

Returned identity refuses aliases, writes, defaults/rest/destructuring,
async/generator wrapping, duplicate parameters, eval and arguments (including
parameter initializers). It never closes generic callability. Independent
creates composition retains dependency receipt, graph, trust and verifier
authentication; only the live-verified exact parent census can supply the
missing independent premise. Receipt-only and transplanted-evidence paths
continue to refuse.

## Measurements

Scratch root:
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/`.
All graph measurements use the retained exact installation, an authenticated
registry cache, and a fetch function that throws on a cache miss. Every run
recorded zero misses. No package installation or benchmark repin was needed.

The original `before.json` was produced with the requested recipe corpus
before any edit. Its exact Kobalte 0.9.2 refusal is:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

The checked-in benchmarks/ecosystem/report.json omitted recipes and is not an
apples-to-apples baseline. The prior-turn recipe-bearing baseline is
`opaque-callback-verified.json` (SHA-256
`7a7959e1d64c73a0ae04829898ef469ba9a119b1458a61c3822683c9edcf6997`).
It contains 49 candidates: the original 43 plus six alpha JS candidates.
Nine are structurally loadable JS; two alpha clamp gates complete, no runtime
contradiction occurs, and exportsProven is 0.

The new browser case is
`artifact-case:cfd221a2e518e2d436990d5970d754d6c7c53e6ef8a627e0911476165ed24564`.
Its runtime SHA-256 is
`2117628a30cfb91851cf8751e3a35147776e3671eee52bd790dc74607d5a3eb8`.
Use `contract certify --entrypoint . --conditions browser --dependency-graph-lane`
with the exact package, issuer and checked-in recipe corpus. Five independent
creates closures succeed; zero real contradictions were observed. These are
not five of the original source gates becoming probeable: that count stays 0.
Closing creates alone does not make exportsProven positive; that metric requires
all measured domains and recursive shape to be known.

Each of the other eleven browser candidates was addressed independently with a
scratch recipe, preventing an earlier refusal from hiding a later candidate:

| Export | Outcome before its gate |
| --- | --- |
| createGenerateId | Census refuses TemplateExpression coercion |
| getDocument | Census refuses unknown property accessor |
| getEventPoint | Census refuses unknown property accessor |
| getScrollParent | Census refuses unknown property accessor |
| getWindow | Census refuses unknown property accessor |
| isFrame | Census refuses unknown property accessor |
| isPointInPolygon | Census refuses ArrayBindingPattern iteration |
| isString | Census refuses CallableFunction.call control transfer |
| isVirtualPointerEvent | Census refuses unknown property accessor |
| removeItemFromArray | Census refuses SpreadElement iteration |
| runAfterTransition | Census completes; runtime gate is incomplete in the Node realm |

The last gate is
`sha256:96c21b89060271e70162a24b74bdd214e1b521fb358bffbfdd8d90f52a43ffd7`.
The published function requires requestAnimationFrame; this harness supplies
neither it nor a DOM. No shim was added. Audits are
`092-frontier/remaining-<export>.audit.json`. The ten census refusals are not
runtime contradictions. None of those eleven gates completed.

The Node lane's current exact remaining demand is
`sha256:ac240ae553ee628e4b0c4b52a2f1885ff387d0e3f5538eac93d4bb5913212b05`:

```text
recursive-value-shape (artifact-case:6630d6444a26cac49a19937c40f5bf01108f652e26ee152d441732139cf802f4:AbortSignalPlugin): export root is not compiler-proved non-callable and non-constructable
```

seroval-plugins publishes a declaration typed any. More .d.ts files alone do
not establish runtime behavior; this path needs exact runtime/declaration
shape evidence or a sound proposal withholding rule. That work is still open.

## Fixtures and reviewed snapshots

The source-only/JS sibling disposition remains pinned by probe-source-disposition.
The new native independent-census-composition fixture certifies its JS positive,
rejects an opaque imported call, and deliberately emits a contradiction that
blocks closure. That veto is a successful negative control, not a regression:
the refusal has the form `probe contradiction at <gate> for <semantic claim>`.
It is an injected harness control, not a discovered package defect.

This continuation adds four generated-main fixtures: returned-callback-descendant,
late-types-condition, retained-value-callback and mutated-parameter-return.
Phase19's tracked-main count moves 181 to 185. Returned-parameter-identity and independent-census-composition
are native-only inputs. All new fixture directories are staged as required.

Non-updating gates and exact output inspection preceded each snapshot update:

- deferred-returned-callback: remove unsupported nested callback claims;
  direct/debounce/decorated/throughIdentity siblings remain unchanged.
- dialect-defining-archive/@solidjs/signals: withhold onSettled bootstrap callback;
  router-shaped sibling remains unchanged.
- conditional-targets: one additional development/solid missing-runtime refusal;
  main and proposal unchanged.
- runtime-semantics: remove retainArray/retainMap/retainSet invocation claims;
  callback knowledge becomes unknown, scheduler/direct cases remain unchanged.
- reactive-ir/package-return-producer coverage: createDeferredProxy gains one
  SC9005 uncertifiable callback-timing finding at the retained constructor input;
  retention no longer certifies queued invocation. The other 93 projects are
  unchanged. The full finding was reviewed before updating its snapshot.

Earlier-turn fixture/snapshot changes remain in the worktree and are described
in ADRs 0009–0014. No benchmarks or phase20/21 ledgers were regenerated.

## Final verification and repeated three-row run

The final requested run is `published-js-verified.json`, SHA-256
`792bcb4a2c4282956c96bb01fa7fbe80fc38ba5749ad0fa02d0de34185bdd9b3`.
It uses the requested three-row command, adding only --keep-temp. Both this
run and the original before.json use a recipe corpus; the recipe additions are
part of the change under measurement. The original before.json SHA-256 is
`1ae364bc20978b7c6ee2f90a4140fd518ccb0a71b4361797aecc0718d300a04c`.

| Required row | Original before | Final after |
| --- | --- | --- |
| Kobalte 0.9.2 | 33 TS candidates; noop gate incomplete; other 32 do not execute | Same 33 and same exact incomplete gate |
| Kobalte alpha | 7 TS candidates withheld; no completed gate | 13 candidates (7 TS, 6 JS); two clamp gates complete; 11 withheld |
| i18n 2.2.1 | 3 JS candidates; all gates unexecuted after chainedTranslator accessor-census refusal | Same 3 and same accessor form/span; all gates unexecuted |

Totals for the requested rows: **43 → 49 candidates; 3 → 9 structurally
loadable JS candidates; 0 → 2 completed gates; 0 runtime contradictions;
exportsProven 0 → 0**. Relative to the immediately preceding turn's own
recipe-bearing baseline, totals stay **49 / 9 / 2 / 0 / 0**. Of the original
43 candidates, **zero became newly probeable**. The five browser JS closures
are additional independent claims, measured separately, so the combined two
lanes have seven completed gates rather than seven repaired source gates.

i18n's final census demand is
`sha256:3da8a07cf0cd148afc9153685ee75053ba323a7e207e672f8134e9a9384847f1`.
It still refuses property-access-unknown-accessor (SpreadAssignment) at
dist/index.js:3471..3483. No accessor rule was changed.

The intermediate `published-js-final.json` recorded alpha's new identity
refusal, which led to ADR 0024. It is not the final outcome. Alpha's two final
main digests are
`sha256:8ae46d4b89f467b606ff7511cf8a6f6fb32a0be8d7f503ab270eb044bbca6678`
and `sha256:d24ae8fc912dc0387aa1f7b3f483a2e7a6c999cd3a395486900271717e1899cc`.
Compared with the preceding accepted mains, only roundToStepPrecision's false
parameter-return operation disappears. Both clamp closures remain. Their
nonempty probe roots are
`sha256:2ba8184e66fb9655b7c9a510620084544a4ca88ce2e5226e0823fba35bd06b3d`
and `sha256:9553b0df05da256f13f4c8808a796b8c12a903dfde4cd8bce1d8d85388d537e9`.

The final browser run uses the checked-in five recipes and is recorded in
`092-frontier/browser-verified.audit.json`, SHA-256
`c397053785047a31692ff6d53800f1edc5e3980a5d6f3c14c4ab3b25acfe31c3`.
Its accepted root main is
`sha256:549f62e687cddbeda6bda8c1a142c8143a83f53e83fdaebb1c298f2d253f69fa`,
receipt `sha256:b15970f67d341c89a8084194c7c8f543693e860d72e08620b2809274d2d43b00`,
and probe root
`sha256:c95554b3ff2d6ec3704e3f8c3893ecab79397df27af9c4dcfddb5a0bf2fb7ba8`.
The accepted main contains exactly the five creates closures named above.
The benchmark's own summarizer reports exportsProven **0 of 59** for this main.

All required checks pass:

- Makefile builds with certification pins; probe harness 97 tests; backend
  library 376; IR library 234; facts library 73; Type Facts client library 43.
- Armed contracts_process 7, diagnostics_process 15 and dialects_process 37.
- Full local Go producer suite, including 17 parameter-identity controls.
- Non-updating final contract corpus: 94 fixtures, 5 exact refusals, 20 local
  refusals, 10 inapplicable cases, 15 withheld claims, 87 declined closures,
  136 artifact cases, 192 possible operations, 904 proof candidates and 4485
  local open claims.
- Coverage: 94 projects, 547 findings, matching the reviewed snapshot.
  Ownership: 289 cases and all 465 ledger rows, zero pending.
- Vitest scripts: 153 tests, including phase19's tracked-main count 185.
  CLI: 175 tests plus its TypeScript tests.
- Pinned TypeScript 5.9.3 resolves the late-types fixture to index.d.ts;
  the isolated retained Proxy example passes strict noEmit against real libs.
- cargo fmt --all then --check; workspace Clippy --all-targets -D warnings;
  schema JSON validation; dialect manifest validation; git diff --check.

One Cargo process ran at a time through the Makefile. A separate debug build
restored pins after the final Clippy run, before final measurements. No
make verify, full ecosystem benchmark, benchmark/ledger repin, commit or push
was performed. The lead still owns the final make verify. The remaining
refusal boundaries above are explicitly unresolved.
