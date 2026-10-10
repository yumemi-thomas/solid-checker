# Compiler-source retry recovery: scoped measurement

After [ADR 0061](../../adr/0061-compiler-source-retry-isolation.md), Kobalte Core
0.13.13's accepted set grows from 503 to 576 artifact cases and from 441 to 507
entrypoints. All 503 previous cases are preserved, including hashes, branches
and multiplicity. Both distribution root cases now certify. The row remains
partial because its wildcard denominator cannot establish completeness.

The [exact before/after case sets](2026-09-07-retry-sources-kobalte-scoped-measurement.json)
record every entrypoint, runtime/declaration path and hash, resolution branch,
importer binding, published pointer/catalog/receipt digest and receipt payload.
The scoped probe exited 0 after 863.041 seconds. Ordinary consumer analysis
authenticated the receipts and selected exact cases. No trial receipts were
reused and no denominator definition changed.

This is 73 additional certifications over the preceding 503-case scoped run,
not 576 additional cases over that run. Against the latest completed full
baseline, Kobalte's accepted set was empty and its state was refused; its
current scoped state is partial with 576 cases. Together with the separately
measured 68 default-export cases, the four scoped probes add 644 cases over
that full baseline. They establish no complete-row gain. Full-corpus counts
and preservation of unrelated rows require the subsequent full run.

Of Kobalte's 577 generated candidates, only `./src/index.tsx` remains refused:
`Accordion`'s recursive-value-shape demand lacks proof of callability or
constructability. Its source says `export * as Accordion from "./accordion"`,
while the target module also exports a callable named `Accordion`. The export
entity traversal in the generator merits investigation: a namespace export
must not acquire a same-named member's identity. The native refusal is retained;
neither a spelling match nor absence of an entity can certify the namespace.

The retry regression failed before the patch and passes afterward. All 83
contract-workflow tests pass, including genuine acquisition failures, exact
identity checks and final union refusal controls. Full `make verify` exited 0,
printed `TOTAL 81.04s`, and contained no `FAILED during step` marker in
`/private/tmp/retry-sources-verify.log`. No fixture snapshots, protocol, receipts
or bundled contracts changed in these CLI slices. No commit or push was made.
