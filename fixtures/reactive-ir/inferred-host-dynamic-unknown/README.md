# Inferred host: dynamic-unknown

Copy of `inferred-host-spa` (`dynamic-unknown`): browser admission is refused and the browser upgrade of `startClosed()` stays withheld (explicit browser is the control; `expected-claims.json` is hand-stated and unrun). Package contracts are removed (ADR 0312), so `startClosed(): void` carries no ownership requirement and the snapshot is certified with no findings either way: the fixture no longer pins the admission outcome through a finding. No published signature is loosened. The `solid-reactivity.json` beside the stub is inert.
