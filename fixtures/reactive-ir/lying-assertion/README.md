# lying-assertion

**Claim (ADR 0781).** A type assertion does not make a value the standard container it names. `(t as unknown as number[]).forEach(cb)` runs `t`'s own method, so nothing the standard method does is proved: a read in `cb` is open. An array literal under the assertion, or a value whose own type is that container alone, keeps the proof.
