# Explicit strict-read clearing (proposal, not executed)

Install this directory as `fixtures/reactive-ir/package-strict-read-cleared-consumer/`.
It mirrors `package-own-tracked-read-consumer`: the synthetic package manifest
is byte-identical, so its authorization identities remain valid. The vocabulary
addition is hand-stated under fixture authorization, not inferred or certified.
The rc.0 package markers copied from the harness select Solid 2; this synthetic
fixture does not claim to audit rc.0 runtime behavior. The real source premise
is rc.13 `untrack(fn, strictReadLabel)` in
`rust/target/primitives-checkpoint/misuse/keyboard-createKeyHold-top-level-read/node_modules/@solidjs/signals/dist/dev-shared.js:5863-5887`.
The matching unlabelled use is
`rust/target/primitives-checkpoint/misuse/scroll-createScrollPosition-module-scope/node_modules/@solid-primitives/static-store/dist/index.js:98`.
The two synthetic calls are identical except for the explicit strictRead fact.

| Case | Required finding |
| --- | --- |
| ClearedAtCall / watchStatus | no SC1001; no reactiveReads completeness obligation |
| NoClearingFact / peekStatus | SC1001, kind uncertifiable, package-internal |
| ClearedThroughWrapper | no SC1001 |
| ClearedThroughModule | no SC1001 |
| tracked-read.refused-contract | decoder/normalizer refusal at strictRead, not silent omission |

The refusal document is intentionally outside the authorized project document.
Use it in a focused decoder/process refusal check; do not authorize it alongside
the positive fixture, which would refuse the entire project. The included wire
unit tests already check the invalid tracked operation. Do not infer a finding
snapshot from this plan: run non-updating coverage on the patched fresh binary,
review the exact diff, then record only this fixture's snapshot. Add `.gitignore`
exceptions for all node_modules files. Phase19 stable-main count increases by
one; the `.refused-contract` file is deliberately not another inventoried JSON.
The void signatures cannot manufacture a callback return-type error; verify
the fixture with TypeScript when implementation is allowed. No tests ran here.
