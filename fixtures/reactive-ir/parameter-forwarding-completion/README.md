# Parameter forwarding on every completion (ADR 0285)

`function f(cb) { return g(cb) }` calls `cb` during the call when the forward is on `f`'s every-completion path, the argument is `f`'s own parameter unchanged at an exactly mapped position, and `g` calls that position on every completion (chains compose to a fixpoint): SC1001 at the call; deferred, replaced, guarded, `Maybe`, timer, after-pattern, `const`-arrow-target (TDZ, ADR 0218) and direct-`eval` cases are clean/silent.

Variant `eval`: `Evaluated` and `evaluated.ts`, moved here from this project. ADR 0803 (item 3): a direct `eval` anywhere in a project may patch array iteration, which withdraws every primitive's tuple slots in it; the eval cases live in `variants/eval` so the cases here keep their proofs, and the variant pins the withdrawal.
