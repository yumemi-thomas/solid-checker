# Inferred host: workspace

Copy of `inferred-host-spa` as a workspace member (shared `tsconfig.base.json`): browser admission is given, which shows as status `uncertifiable` with no finding (`inferred-host-workspace-published` is the refused twin, `certified`). `startClosed(): void` is opaque since package contracts were removed (ADR 0312), so the admission is not pinned by a finding.
