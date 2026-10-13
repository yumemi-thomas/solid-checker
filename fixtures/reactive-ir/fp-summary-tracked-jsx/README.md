# fp-summary-tracked-jsx

**Claim.** A helper whose reactive reads are all in its own JSX does not read
them while it is called. The compiler lowers an attribute or child expression
into a tracked effect, and a component property into a getter that its
consumer reads. So a call to such a helper from a strict-read window (here, a
`<For>` render callback) is not a strict-window read. The summary path of
`strict-read-untracked` (`interproc.rs`) now skips a read or call that the
compiler places in the owner's own tracked region or component-property
getter (`execution_role::runs_outside_owner_call`). A function that is an
attribute value inside a tracked region (a handler) keeps its old answer.

Before, all four negatives were proven violations. The real-app sweep's
`sefer` `FindingsPanel.tsx:526` and `:539` are this shape: `aria-expanded` and
`<Show when>` reads in a `line()` helper called from `<For>`.

Positives: a helper that reads while it is called (`label(row)` in the
callback body, and a read before the helper's JSX) stays reported at the call.

`solid-js.d.ts` is copied from `fp-exec-control-flow-callback` and is
byte-faithful to the published 2.0.0-rc.9 declarations the claim rests on
(`createSignal`, `For`, `Show`). The only local addition is the intrinsic
`li` element's `aria-expanded` attribute. `App.tsx` also passes
`tsc --noEmit` against the real rc.9 install.
