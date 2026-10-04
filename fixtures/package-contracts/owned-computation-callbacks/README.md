# owned-computation-callbacks

ADR 0183: a caller's function that is the exact argument of an eager owned
computation slot -- `createMemo(fn)`, `createEffect(compute, effect)`'s
compute -- in a call the export's own body makes is invoked tracked, during
the call, under an owner the operation creates. So is one the export calls on
every completion of a synchronous function literal written as such a slot's
whole argument. The generator publishes that
owner, and `min: 1` where the call covers every normal completion; the census
proves both from the producer's facts or withdraws them.

The `solid-js` stub is `owner-call-cover`'s.

- `derive`: created owner, `min: 1`.
- `deriveMaybe`: created owner, `min: 0` (the call is conditional).
- `watch`: the compute has a created owner and `min: 1`; the effect function
  is `queued` and `ambient-at-execution`, with no owner claim.
- `deriveWrapped` and `deriveArrow`: created owner, `min: 1`. `fn` is called
  on every completion of a synchronous compute the export writes, and that
  compute is the memo's whole argument.
- `deriveWrappedMaybe`: no owner claim (the call in the compute is
  conditional).
- `deriveWrappedAsync`: no owner claim. The compute is async, so the call runs
  after an `await`, with no owner.
