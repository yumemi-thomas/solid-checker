# Inferred host: workspace-published

Copy of `inferred-host-spa` (`workspace-published`): browser admission is refused and the browser upgrade of `startClosed()` stays withheld (explicit browser is the control). Package contracts are removed (ADR 0312), so `startClosed(): void` carries no ownership requirement and the snapshot is certified with no findings either way: the fixture no longer pins the admission outcome through a finding. No published signature is loosened.
