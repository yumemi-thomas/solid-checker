# Controlled import-free TypeScript erasure — 2026-09-05

> Identity amendment: ADR 0030 later moved every controlled profile to receipt
> v5, worker protocol v5 and sandbox scheme 10 so the same protocol can bind an
> authenticated relative module graph. The measurements below predate that
> identity-only strengthening; import-free semantics are unchanged.

> Census amendment: ADR 0029's finite local-recursion fixed point, exact
> exported-helper demand span, and nullish-equality correction subsequently
> admit `getWindow`, `getActiveElement`, alpha `roundToStepPrecision`, and alpha
> `snapValueToStep`. The current import-free result is 18 controlled closures;
> the 14-row table below records the earlier measurement point.

ADR 0028 was written before implementation. The new
`node-strip-import-free-esm-v1` profile extends the checker-owned execution
capability from inert no-ops to directly exported callables in import-free
TypeScript modules. It does not create an ordinary package contract and does
not claim compatibility with an application's compiler or bundler.

## Decision and alternatives

Four dispositions were considered.

1. Keep refusing TypeScript runtime artifacts. This preserves the published-byte
   certificate unchanged, but leaves all 40 source candidates blocked. Closing
   from the census alone would need a new consumer-visible policy saying the
   contradiction veto was withheld; it would no longer be the current certified
   row and was rejected here.
2. Probe a published JavaScript artifact case. All 40 names have a JavaScript
   sibling somewhere in the package, but authenticated behavioral equivalence
   between a source case and a JavaScript case was proved for **0/40**. A sibling
   can certify only its own artifact case. Existing JavaScript closures remain
   useful and unchanged.
3. Bind a transformer and probe derived bytes. The POC established feasibility,
   but its reflection control also proved that Node stripping and TypeScript
   compilation can behave differently. A digest alone cannot make that result
   portable to another consumer.
4. Execute the exact derived interpretation inside a checker-owned transaction.
   This makes applicability concrete without exporting the claim. The checker
   re-derives the bytes, runs the mandatory veto, authenticates a scoped receipt,
   then replays the selected recipe in a fresh worker under the same identity.

The fourth option is implemented. Its cost is deliberate: downstream analyzers
cannot reuse the receipt. The result means only that the native implementation
census closed the selected `creates` domain and the mandatory finite veto found
no contradiction when the exact authenticated source was stripped by pinned
Node 24.11.1 and immediately executed as import-free ESM under this sandbox.
A finite clean run never supplies the positive closure evidence.

## Bound interpretation

The native Oxc validator parses the entire module and permits only direct
callable exports, runtime-import-free ESM, and a finite list of type-only spans.
It blanks those spans with ASCII spaces while preserving byte length and line
terminators, then requires the output to parse as JavaScript. The worker invokes
pinned Node's strip-only transformer independently and requires its output to
equal the native expected bytes and the watched private derived file.

Controlled receipt version 5 binds the source and derived digests, selected
export and semantic claim, empty dependency closure, module format and exact
source URL, parser preservation identity, Node executable/version, complete
harness image, recipe plan, Type Facts producer sessions, sandbox policy and
issuer. Worker protocol v5 reports transform verification, module loading and
consumer completion. Sandbox scheme 10 records the broader profile and recipe
replay while retaining the 0700 private workspace, cleared environment,
allowlist, process group and `killpg`, single startup/run frames, primordial
capture and prototype freezing, exact reported resolution, and detect-and-refuse
write isolation.

Existing policy-2 consumers reject this receipt. Request version 7 requires the
exact profile name and one planning. Profile/proof cross-pairs, source, derived,
retained-output, transformer, Node and receipt mutations refuse. Imports,
extension inference, enums, namespaces, decorators, JSX, dynamic evaluation,
CommonJS consumption and unsupported consumer profiles refuse. Browser globals
are not synthesized.

Transformation can hide a contradiction that depends on erased spelling,
source positions or output another compiler would generate. The reflection
fixture demonstrates this: Node-strip execution is clean while TypeScript
emission adds the tripwire global. The result remains defensible because it is
scoped to the exact Node-strip bytes and cannot be reused across profiles.

## Measurements

The completed POC identified 26 import-free candidates with finite sample
recipes. A fresh production diagnostic ran each through the real planning and
native census. Fourteen reached the mandatory gate; twelve refused before the
probe: one `CallableFunction.call`, one recursive path, five incomplete helper
transcripts, three coercions and two unknown element accesses. The diagnostic
is `/private/tmp/import-free-census/results.json`, SHA-256
`d9a4f03650610ea50130800120f7c190c7201926afebb9b49a3bc3dcf42fe71c`.

All fourteen census-ready candidates then completed the production controlled
profile:

- Kobalte 0.9.2: `addItemToArray`, `removeItemFromArray`, `isArray`,
  `isFunction`, `isNumber`, `contains`, `getDocument`, `isFrame`, `noop`,
  `clamp`, `snapValueToStep`, and `getEventPoint`;
- Kobalte 2.0.0-alpha.0: `clamp` and `getPrecision`.

| population | candidates | loaded | completed gates | real contradictions | controlled accepted closures |
| --- | ---: | ---: | ---: | ---: | ---: |
| census-ready import-free | 14 | 14 | 14 | 0 | 14 |
| import-free census refusals | 12 | 0 | 0 | 0 | 0 |
| original TypeScript population | 40 | 14 | 14 | 0 | 14 |

Against the original blocker baseline, 14 of the 40 TypeScript candidates
become controlled-profile probeable, so the structural population moves from
3/43 to **17/43**. Relative to ADR 0026's immediately preceding inert profile,
this slice adds 13; `noop` was the one already admitted.

The controlled result set is
`/private/tmp/import-free-profile-results.json`, SHA-256
`bb80a49e8b87932697e53ec64b149da1dce3140aabf8374a5c193be73b9caabb`.
These are fresh production
transactions, not the POC's 26 feasibility observations. The deliberate
derived-byte fixture emits `undeclared-alternative`; the gate returns
`ProbeGateError::Contradiction` and no receipt is issued. That contradiction is
a successful veto control and is not counted as a real-package defect.

The requested ordinary three-row runner used the same checked recipe corpus
before and after. Before is the immediately preceding controlled-inert run at
`scratchpad/probe-ts/controlled-inert/after.json`; after is
`/private/tmp/import-free-three-row-after.json`. The checked ecosystem report,
which has no recipe corpus, is not used as a baseline.
The before/after JSON SHA-256 values are
`38b2b0bfe2fea675de6e28f37b6434f8d355856895256e6abe40c918b5bc8555`
and `cec28a8e13912862cf9212192ec60b95659921a8abba89208caf5bc5d789297f`.

| row | ordinary before → after | gate completion | accepted ordinary creates closures | exportsProven |
| --- | --- | ---: | ---: | ---: |
| `@kobalte/utils@0.9.2|solid1|only` | refusal unchanged: gate `sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc` did not complete | 0 → 0 | 0 → 0 | 0 → 0 |
| `@kobalte/utils@2.0.0-alpha.0|solid2|only` | certified with 11 withheld closures, unchanged | 2 → 2 | 2 → 2 | 0 → 0 |
| `@solid-primitives/i18n@2.2.1|solid1|only` | accessor census refusal before any gate, unchanged | 0 → 0 | 0 → 0 | 0 → 0 |

Ordinary behavior remains unchanged because this profile intentionally cannot
publish an accepted catalog. `exportsProven` remains 0 of 99 across these rows;
closing `creates` alone does not close the other export domains.

## Remaining work

Twenty-six of the original 40 do not obtain a controlled closure. Twelve are
the independent census limitations above. Eight candidates need authenticated
extensionless relative-import resolution; that requires a new profile binding
an exact source edge map and has not been inferred. Six require browser APIs;
they need a separate browser execution profile with a real, pinned environment.
These groups come from different feasibility stages and should not be summed as
disjoint certification outcomes without consulting the candidate table.

The next highest-yield work is the five incomplete-helper census transcripts,
then authenticated import resolution. The coercion, recursion,
`CallableFunction.call`, unknown-accessor and browser cases need separate proof
decisions. The accessor-census refusal in `@solid-primitives/i18n` is unchanged
and remains outside this slice.

## Verification

The Makefile-pinned checks pass: 76 facts tests, 99 probe-harness tests, 379
backend tests, 236 IR tests, and the armed contracts/diagnostics/dialects
process suites (11/15/37). The non-updating contract corpus compared 94 stable
fixtures; no update was needed. Coverage compared 94 projects and 547 findings;
ownership passed 289 cases and all 465 ledger rows. The ecosystem script suite
passed 252 tests; the CLI passed 175 tests plus its TypeScript build. Phase 19
remains at 185 stable mains, and the Phase 16 check is unchanged. Rustfmt,
workspace all-target Clippy with `-D warnings`, schema parsing, dialect manifest
validation and `git diff --check` pass. The checker was rebuilt through
`make build-checker-debug` after Clippy to restore the compiled certification
pins.

No contract or finding snapshot moved in this slice, so no updating gate ran.
No ecosystem baseline, phase ledger, commit or push was produced, and
`make verify` was intentionally left for the lead.
