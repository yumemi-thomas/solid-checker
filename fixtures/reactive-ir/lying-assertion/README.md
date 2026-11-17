# lying-assertion

**Claim (ADR 0781).** A type assertion does not make a value the standard container it names. `(t as unknown as number[]).forEach(cb)` runs `t`'s own method, so nothing the standard method does is proved: a read in `cb` is open. An array literal under the assertion, or a value whose own type is that container alone, keeps the proof.

ADR 1023: `Lies`' receiver is a closed `const` literal, so its call dispatches to the literal's own `forEach`, which hands `cb` to a microtask: the call has no impostor row any more, and the read keeps its own obligation (`callee-callback-timing`). `Held` (an alias) is not followed and stays withheld.
