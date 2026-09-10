# Returned-callable identity: measured next blocker

The protocol-37 full run remains at **462 withheld candidates**: **442
creates** (405 census refusals and 37 veto failures), plus **20 returns**.
All 418 probes, installedVersions fields and row statuses match the preceding
run: **368 certified / 30 refused**, with 20 probes not advanced.

ADR 0054 clears six first refusals on the `isCSSVariableName` factory result,
but **closes zero complete candidates**. The affected export is
`motion-dom@12.43.0`'s `buildHTMLStyles`, six occurrences under motion-solidjs.
Returned-callable first refusals fall **52 → 46**, while template-coercion
first refusals rise **7 → 13**. There are no removed or added candidate
identities. All other residual reasons are unchanged.

The newly exposed blocker is precise:

```
build-transform.mjs:1755..1790 — coercion (TemplateExpression)
parameterPremiseRefusal at build-transform.mjs:623..2739 (depth 1):
parameter 2 types differ: twin "any", premised "TransformTemplate | undefined"
```

The factory mechanism therefore fires in the corpus. Its next prerequisite
is a caller-to-helper type premise that remains resolvable in the helper's
module. `spellableTypeReferenceLocked` currently spells one exported type
alias/symbol as an import type. It does not spell a union with an optional
constituent, so the plain helper's `TransformTemplate | undefined` annotation
does not resolve and the twin refuses the whole premise. A complete union
spelling must preserve every constituent and still pass the existing
byte-for-byte type/identity echo checks. This is the next fact fix to
prototype; clearing it does not yet prove the template has no later blocker.

[The evidence JSON](2026-09-07-factory-census-measurement.json) contains all
462 candidate occurrences, all audits and hashes, the six exact before/after
reason pairs, every refused/unattempted row, and the raw report and executable
identities. No acceptance is inferred from the first-refusal movement.

The anonymous-callable producer test passed, including a shifted-span
refusal. The packed native factory fixture passed five outcomes: a literal
primitive capture closes, while writes to the factory, result or capture and
an object capture remain refused. Full **make verify passed in 216.74
seconds**, with exit 0, TOTAL present and no failure marker. No snapshots or
public package contracts changed, and nothing was committed or pushed.

The goal remains active. The other 46 returned-callable cases require broader
factory/capture facts, and the remaining census and veto limitations are not
being reclassified as package defects.
