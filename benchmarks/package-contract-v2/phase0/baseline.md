# Phase 0 package-contract baseline

Captured at `2026-08-26T17:30:14.057Z` from `5950f60aac8a79a7a16eb48db0939f57e7048163` on `codex/package-contract-phase0`.

This is the reproducible comparison authority for the legacy package-contract implementation. Published Solid 2 RC.3 package bytes are authoritative for future behavior, while the currently bundled Solid 2 contracts remain RC.0 inputs; this report does not certify RC.3 semantics.

## Exit result

- 418 verifier rows were classified exactly once.
- 309 verified; 90 refused; 18 generation failures; 1 without a resolvable Solid runtime.
- Input manifest: `6ef08c4a34cc3e218d74cda86e37a274398a142241bb66c88b772718928537af` over 140 files.
- 13 representative legacy fixtures are hash-frozen.
- All three measured commands exited successfully with `SOLID_CHECKER_GATE_CACHE=0`.

## Exact pins

| Input | Identity |
| --- | --- |
| Solid 2 compiler | `26e744fb4feb973a3652bfc45a8c3938ece667f0` (semantic trace 2) |
| Solid 1 compiler | `ca3bbfae7d1e00e28ef73f9af58bdb46e248b512` |
| Type Facts | `92c53392388518d69ef27220729f5c061479deed` |
| Checker binary | `b48a808ce65864dfc77cb8dd2a37b77deceed4bfcc4408c42645dc3a7edf7399` |
| Type Facts binary | `7d8d2ffbc472049660b8b9666da1990edaf86890fd834f45c7e642589abc645b` |
| Published Solid authority | RC.3 at `af6fee86e6dcfbf41869da2c607c82b1fd0939ce` |

## Classification ownership

| Owner | Rows |
| --- | ---: |
| none | 309 |
| schema | 56 |
| probe | 30 |
| resolver | 11 |
| type-facts | 11 |
| generator | 1 |
| compiler-facts | 0 |
| runtime | 0 |
| typescript | 0 |

The machine report contains every row's exact probe ID, outcome, primary and secondary owner, failure class, stable reason, and verifier blocker. Zero rows are currently assigned to compiler-facts or TypeScript ownership; those zeroes are explicit rather than omitted.

## Legacy schema and contract size

The schema is 10,754 pretty bytes and 6,197 minified bytes, with maximum measured object depth 13.

| Contract | Pretty | Minified | Expanded | Evidence delta |
| --- | ---: | ---: | ---: | ---: |
| @solid-primitives/debounce@1.3.0 | 1,083 | 692 | 883 | 195 |
| @solid-primitives/rootless@1.5.4 | 2,958 | 1,917 | 2,489 | 536 |
| @solid-primitives/scheduled@1.5.3 | 4,946 | 2,265 | 3,725 | 1,217 |
| solid-js@1.9.14 | 12,556 | 6,717 | 15,510 | 83 |
| solid-js@2.0.0-rc.0 | 42,983 | 17,368 | 20,461 | 7,281 |
| @solidjs/signals@2.0.0-rc.0 | 546 | 415 | 394 | 75 |
| @solidjs/web@2.0.0-rc.0 | 29,881 | 15,888 | 32,206 | 1,185 |

Across the bundle, minified main documents total 45,262 bytes, expanded documents total 75,668 bytes, and inline evidence accounts for a 10,572-byte serialized delta. Legacy sidecar evidence is not applicable.

## Time and memory

| Measurement | Wall time | Peak process-tree RSS |
| --- | ---: | ---: |
| Ecosystem generation | 627.0 s | 3.598 GiB |
| Ecosystem verification | 141.1 s | 2.104 GiB |
| Legacy contract corpus | 1.04 s | 0.896 GiB |

Legacy JavaScript parse+expand of all 7 bundles: p50 0.105 ms, p95 0.164 ms. Direct normalized lookup: p50 11.796 ns, p95 30.383 ns. The Rust consumer has no isolated query seam, so Rust cost is represented by the end-to-end corpus measurement.

## Frozen fixtures

| Fixture | Purpose | Tree SHA-256 |
| --- | --- | --- |
| unresolved-callee-callback | false-negative guard: unresolved callback reachability | `5b676362611120e52895e86ed5672a26470a62c831da062509e2b36c52254050` |
| conditional-export-absence | false-negative guard: export absent in one artifact condition | `6c9ebdd31975e3c6182eaf609a94382854eecae1685708ef52e2edcdf0610985` |
| conditional-returns-divergence-both | false-negative guard: condition-dependent return semantics | `76566041d661272c8423f8cc7231a1a5293a46d4dd83d4109bafc782982775b4` |
| conditional-callback-conflict | false-negative guard: incompatible conditional callback behavior | `1c3ae9cb5c0ef55d1c57fb118c83d28eeaa682641e996b1f20fbebeac4b0cbe7` |
| declaration-sibling-reach | false-negative guard: declaration sibling reachability | `2738f8ec124651b6754a1f9c984a13d815690ff5dc6c26b2b1590f13f7fe8e2c` |
| escaping-private-helper | false-negative guard: escaping private callback helper | `a5c7ee88083770262a7bd9270e8382aec758defc88fdd423fef2349631ddbe2d` |
| unreached-private-obligation | false-negative guard: private obligation outside export reach | `30a04125dc0fb366c119cf9b35d1de9ca399930a7d2a8e90bddd72adf42c42bf` |
| class-expression-kind | over-refusal guard: class expression export kind | `706759e65ee4ab7ff9d9732545f88e79963b3f3268ea3b8cd6615bf0b31cc103` |
| function-supertype-kind | over-refusal guard: callable supertype export kind | `11b05f016a0aff5cedb54022bf0e852b444132189327de752ea135036824690c` |
| torture-environment-conditions | over-refusal guard: ordered environment conditions | `40153a20ae2a653e183fa56390d9799f3aad617ce583ae371ed6ad72eecf8fd0` |
| attested-record-matches-walk | over-refusal guard: matching attested and walked closure | `cd2cdfa2324f18363c7e63eab226f7f439207bd07c8cb48849e7b46de1a1c9f9` |
| non-literal-dynamic-import | fail-closed frontier: nonliteral dynamic import | `809d182ccd4ad9f1016b74ba6f4a311314a56940111a80b9d257fcca17947f19` |
| torture-runtime-namespace | over-refusal guard: runtime namespace export identity | `472f072b711be377f29c771227b270da3733a823b322083d76cd48213794162c` |

## Known boundaries

- The RC.3 audit proves exact tarball integrity, manifests, package contents, and concrete export-target existence. It does not retain a complete transitive installed dependency/declaration closure; Phase 7 owns that proof.
- The fresh ecosystem generation run observed one 600-second package-install timeout. Independent verification retried the row successfully and classified it as a refusal; both observations are preserved.
- Verification success means the current legacy proof policy accepted the document. It is a comparison baseline, not evidence that the replacement proof model has already been implemented.
- Current bundled Solid 2 contracts are RC.0 structural inputs and must never be cited as RC.3 semantic authority.

## Reproduction

The raw command arrays, timestamps, exit status, sampling method, sample count, and peak RSS are preserved under `benchmarks/package-contract-v2/phase0/measurements/`. Re-run generation and verification with stable checker binaries and `SOLID_CHECKER_GATE_CACHE=0`, then run:

```sh
bun scripts/package-contract-v2-phase0.mjs --capture-current --output-json /tmp/phase0-current.json --output-markdown /tmp/phase0-current.md
bun scripts/package-contract-v2-phase0.mjs --check
```
