# Solid 1.x bundled-contract consumers

This fixture pins the consumer effects of two exact reviewed contracts.

- `solid-js@1.9.14` states `requestCallback.callbacks[0]=deferred`; the signal
  read in its callback is attributed to the real deferred execution path
  rather than a certified-negative "never invoked" callback claim.
- `@solid-primitives/debounce@1.3.0` states that both its named and default
  factories require a cleanup-capable caller owner and invoke callback
  parameter 0 later without an owner. The component supplies the former; the
  callback's signal read proves the latter.

Both components are explicitly typed with Solid's published `Component` type,
so the cleanup-capable owner is proven rather than guessed from capitalization.
The expected result is fully certified with no findings: deferred callbacks are
legitimate here, and the cleanup is registered in a component owner. The
declarations reproduce the published signatures involved in these proofs, and
`tsc --noEmit` accepts the project.
