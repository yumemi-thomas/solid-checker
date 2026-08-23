# An uncaptured reactive source invalidates the return description too

`observe` comes from a package whose contract describes `described` and says
nothing about `observe`. Handing it a proven accessor as a bare identifier is
`ReactiveSourceUncaptured`: whether the source stays reactive through that
callee is unknowable here.

The claim domains that obligation invalidates were `reactiveReads` only. That
was never *tested*, it was masked. Every shape that reaches this arm during
generation — this fixture included — also raises the package's
missing-contract-export obligation, which already erases all five domains, so
the reads-only claim could not be observed either way. Being covered by another
obligation in the shapes one can build is not a proof that no shape escapes it.

`returns` is now invalidated as well, for the same reason it is for an
unresolved dispatch: what the callee hands back is described from the local
accessor index, which knows nothing about it. `Derived` is that shape —
`const derived = observe(value); return { derived }` — and `derived` could be
the accessor itself, a derivation of it, or a snapshot. Omitting the property
is a certified negative the package cannot support.

`Held` is the neighbouring shape where the returned value *is* locally known
(`return { value }`), and `Steady` neither reads nor hands over anything and
stays certified.

Because both obligations fire on `Derived` and `Held`, the pinned contract
shows all five domains unknown. That is what the fixture records: the arm's
domains cannot be observed in isolation today, so the arm is fail-closed by
construction rather than by proof. See docs/precision-backlog.md.
