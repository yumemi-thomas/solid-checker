# getter-parameter-read

**Claim (ADR 0283).** A project function Getting `p.key` of its parameter on every completion runs the `key` getter of the exact literal a call passes, on the caller's stack, so when the parameter is only read that way the getter's accessor reads are proven at the call (violation, getter read as origin); anything unproven adds nothing and the getter keeps its uncertifiable read. Conditional, closure, async, `this`, escaped, written, spread or inexact-receiver (ADR 0225) shapes are uncertifiable. Scaffolding from `summary-completion-read`.

Variant `eval`: `Eval` and `eval-helper.ts`, moved here from this project. ADR 0803 (item 3): a direct `eval` anywhere in a project may patch array iteration, which withdraws every primitive's tuple slots in it; the eval cases live in `variants/eval` so the cases here keep their proofs, and the variant pins the withdrawal.
