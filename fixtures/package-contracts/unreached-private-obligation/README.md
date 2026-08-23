# A narrowing to zero exports is a decision, and it is on the review plan

`unreached` is private and nothing calls it, so the call graph enumerates its
entry set exactly and finds no export of the entrypoint among them. The
attribution ladder resolves the `.getThing` obligation inside it to *no export*,
and `Steady` is correctly published as certified: this is the answer the
reachability rung exists to give.

It is also indistinguishable, from the contract's bytes alone, from an analyzer
that never saw the obligation. Emission used to return early whenever nothing
was marked, so the narrowing left no trace anywhere — the one decision most
worth checking was the only one nobody could see.

The emitter now reports the zero-export decision on the same stderr marker as
every other attribution, and `generate-package-contract.mjs` turns it into a
review-plan note under **contract artifact binding** — the section for facts
about generation that no contract byte can confirm or deny:

```
.: the ReactiveDispatchUnresolved obligation at channel.js:7-66
(exported-parameter-member-dispatch) was attributed to no export by
`reachability`, so no claim was marked unknown for it; check that no export of
this entrypoint can reach it
```

The note is what makes the residuals in docs/precision-backlog.md visible in
practice: a reach enumeration that is silently truncated produces exactly this
line, and a reviewer can then ask the question the analyzer could not.

The contract itself must stay unchanged by the note — `Steady` carries no
unknown claim — which is what `expected.json` pins.
