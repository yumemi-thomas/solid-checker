# Controlled relative TypeScript graphs — 2026-09-05

ADR 0030 was written before implementation. It admits
`node-strip-relative-ts-graph-esm-v1` only for a fresh checker-owned execution
of a finite package-local `.ts` graph. It does not produce an ordinary accepted
package contract or claim compatibility with an application's compiler,
bundler, Node installation, or browser.

## What the veto proves

The native implementation census closes `creates` over authenticated published
source. Oxc proves that every graph module uses the profile's strip-only grammar.
The existing native snapshot resolver maps each static relative literal to one
member of the already verified runtime closure. Pinned Node 24.11.1 independently
strips every source, and the worker requires each result to equal the native
expected bytes and its watched derived file. Only the exact authenticated edge
map can resolve from a controlled source URL.

The mandatory finite veto then proves that its selected samples found no
contradiction while those derived bytes ran under that exact Node transformer,
module format, edge map, dependency closure, recipe, and sandbox environment.
The clean finite run does not prove the closure; the census does. A real
contradiction still wins and blocks issuance.

The profile stops short of published-byte semantics. Type erasure can hide a
contradiction that depends on source reflection, erased spelling or output a
different transformer would emit. ADR 0028's reflection control demonstrates
that difference. Ordinary policy-2 consumers reject the scoped receipt, so a
digest cannot be mistaken for compatibility with another toolchain.

## Decision across the original population

Four dispositions were measured and weighed.

1. Continue refusing all TypeScript entrypoints. This preserves the ordinary
   published-byte meaning, but leaves 40 gates incomplete. Closing from the
   census while withholding the veto would define a weaker certified row and
   require a new consumer-visible receipt class. It was rejected because the
   current consumer could otherwise mistake an untested closure for a
   veto-completed one.
2. Substitute a published JavaScript case from the same package. JavaScript
   siblings exist, but authenticated behavioral equivalence to the selected
   TypeScript artifact was established for **0/40**. Those cases may certify
   themselves; none can certify the TypeScript case.
3. Transpile under a pinned digest and export the result as an ordinary
   certificate. The reflection control proves that this is unsound: Node
   stripping and TypeScript compilation can expose different behavior. Binding
   the transformer identifies the difference but does not make an application's
   compiler or bundler compatible with it.
4. Execute a checker-owned, explicitly named Node-strip interpretation and
   issue only a profile-scoped receipt. This is implemented for import-free
   modules and exact relative TypeScript graphs. Its cost is that ordinary
   analyzer consumers cannot reuse the result.

The fourth disposition gives the largest defensible gain: 20 controlled
closures with no change to ordinary package-contract meaning. Cases outside
its proven syntax, census, resolution, or runtime premises continue to refuse.

## Identity and controls

Controlled receipt and signature domain move to v5, worker protocol moves to
v5, native execution requests use schema version 8, and sandbox policy moves to
scheme 10. The policy now names all-module transform verification, watched
derived graphs, and the exact authenticated relative edge map. The 0700 private
workspace, `env_clear` allowlist, process group and `killpg`, one startup and one
run frame, primordial capture and prototype freezing before recipe import,
exact reported top-level resolution, authenticated dependency closure, and
detect-and-refuse write census remain in force.

Focused controls cover the TypeScript-only positive graph, an unaffected
published-JavaScript sibling, a contradiction in derived graph bytes, source,
output, retained-output and edge-map mismatches, unsupported syntax, unresolved
imports, transformer pin mismatch, unsupported consumer profiles, and the
reflection counterexample that prevents cross-profile reuse.

## Measurement

All eight original extensionless-import candidates were run through a fresh
production planning, native census, mandatory gate and controlled-profile
attempt. Two completed:

| candidate | native census | graph load and gate | contradiction | scoped closure |
| --- | --- | --- | --- | --- |
| Kobalte 0.9.2 `isVirtualPointerEvent` | complete | complete | none | accepted |
| Kobalte 0.9.2 `hasFocusWithin` | complete | complete | none | accepted |

The six retained refusals occur before profile execution:

| candidates | native refusal |
| --- | --- |
| `scrollIntoView`, `scrollIntoViewport` | reachable indexed property read can invoke an accessor |
| `isElementVisible`, `isFocusable`, `isTabbable` | reachable `instanceof` can invoke `Symbol.hasInstance` |
| `getAllTabbableIn` | `Array.filter` invokes a local callable path that reaches the refusing focusability code |

The result is 2/8 loaded, 2/8 completed gates, zero real contradictions, and two
scoped accepted closures. Together with eighteen import-free completions, 20 of
the original 40 TypeScript candidates now complete a controlled profile. With
the three pre-existing JavaScript cases, structural probeability is 23/43.
Ordinary `exportsProven` remains zero because these scoped `creates` results do
not close every export domain and cannot enter the analyzer catalog.

The complete 20-case remainder is disjoint:

| remaining population | count | disposition |
| --- | ---: | --- |
| import-free native-census refusals | 8 | one exact `Object.prototype.toString.call` path can invoke `Symbol.toStringTag`; five platform reads have no proved inert accessor; two indexed point reads can invoke Proxy/accessor behavior |
| relative-graph native-census refusals | 6 | two indexed reads, three `instanceof` paths, and one callback path into the same `instanceof` code |
| browser-feasibility samples that fail native census | 3 | one `instanceof` path and two reachable getter paths |
| native-census-complete cases needing a browser profile | 3 | both `getScrollParent` versions and Kobalte 0.9.2 `runAfterTransition` |

The first 17 are proof refusals in the independent implementation census. They
are not made safer by another execution harness, and the accessor-census
limitation remains deliberately unchanged. The final three are the only current
population a sound browser profile could advance.

The production graph result is
`/private/tmp/relative-profile-measurement/results.json` (SHA-256
`538c32aa087e78ae5b22c6c869fa8e0ecb54a864036b295c167c2a9764d267c2`).
Browser-census results are `/private/tmp/browser-census/results.json` (SHA-256
`d161af8b923d06f0dcc66d84f1d6ff079b2b5ea0224d7694f16622487fbd9949`).
The refreshed import-free census is
`/private/tmp/import-free-census/results.json` (SHA-256
`93233180f58a92fc993ab2066759088f77a809c080ef750c9570a31139c430d8`),
and all 18 census-ready executions are
`/private/tmp/import-free-profile-results.json` (SHA-256
`846acce460d069c7a13411d074795762c11cd227b8526f467d38a4d43889e344`). These are production
transactions, not the POC's feasibility observations.

## Browser disposition

Three of the six browser-error samples pass native census: `getScrollParent`
in Kobalte 0.9.2 and 2.0.0-alpha.0, and `runAfterTransition` in 0.9.2. The other
three refuse earlier (`focusWithoutScrolling` on `instanceof`, both
`debugPolygon` cases on reachable getters). ADR 0031 withholds a browser
profile because the repository has no compiled browser/driver/module-transform
trust boundary that preserves the harness contract. Vitest browser mode and
ambient Playwright caches do not supply it, and no fake globals were added.
ADR 0032 subsequently measured what such a boundary would be and retained the
refusal; see `docs/2026-09-05-browser-probe-boundary-measurement.md`.

## Ordinary three-row comparison

The same checked recipe corpus was used for the requested three rows before and
after. The checked ecosystem report was produced without that corpus and is not
an apples-to-apples baseline. Controlled execution is intentionally separate
from the ordinary runner, so the ordinary outcomes remain unchanged: Kobalte
0.9.2 refuses on mandatory gate
`sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc`;
the alpha row retains its completed JavaScript gates and withheld source
closures; i18n retains its independent accessor-census refusal. Ordinary
`exportsProven` remains zero.

The exact Kobalte 0.9.2 refusal is:

> `solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete`

The alpha row certifies its existing JavaScript cases with 11 source closures
withheld. The independent i18n refusal is:

> `creates census refuses an uncensused invoking form: property-access-unknown-accessor (SpreadAssignment) .../dist/index.js:3471..3483, reach reachable`

The before file is `/private/tmp/import-free-three-row-after.json` (SHA-256
`cec28a8e13912862cf9212192ec60b95659921a8abba89208caf5bc5d789297f`).
The after file is
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/final/x.json`
(SHA-256
`c3188aa6749b70e3d4c980066638a9a26bafe6327e1adac416da63973626c3af`).

## Snapshot review and verification

The non-updating contract corpus first reported only
`implementation-census-creates` as stale. Manual regeneration showed the
runtime digest moving from `702e9e07...` to `8d8242d4...` and the closure digest
from `c66c26ab...` to `a728e0bb...`; artifact-case, claim and operation
identities rekeyed from those bytes, and three refusal locations shifted by 33
bytes. After that review, the updating run refreshed its stable contract,
proposal plan and refusal sidecar. A second non-updating run compared all 94
fixtures successfully.

The completed checks are:

- `make test-probe-harness`: 102 probe-harness tests;
- `solid-facts --lib`: 77 tests; `solid-facts-backend --lib`: 381 tests;
  `solid-reactive-ir --lib`: 236 tests;
- armed contracts, diagnostics and dialect process suites: 11, 15 and 37
  tests;
- focused Go Type Facts packages;
- contract corpus: 94 fixtures; coverage: 94 projects and 547 findings;
- ownership gate: 289 cases and all 465 ledger rows;
- ecosystem script suite: 252 tests; CLI: 175 tests plus its TypeScript build;
- phase19 audit: 185 stable mains, with no stable-main count change;
- Rustfmt followed by format check, workspace all-target Clippy with
  `-D warnings`, both JSON schemas, dialect manifests, worker/harness syntax,
  and `git diff --check`.

The checker was rebuilt through `make build-checker-debug` after bare Cargo and
Clippy commands so the compiled Type Facts, harness and Node pins are present.
No ecosystem baseline or phase20/21 ledger was refreshed. `make verify` was not
run, and no commit or push was made.
