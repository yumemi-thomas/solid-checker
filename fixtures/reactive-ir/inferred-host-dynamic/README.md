# Inferred host: dynamic

Copy of `inferred-host-spa` whose Vite config is conditional on `NODE_ENV` with both arms plain plugin-free configs: browser admission is given, which shows as status `uncertifiable` with no finding (`inferred-host-dynamic-unknown` is the refused twin, `certified`). `startClosed(): void` is opaque since package contracts were removed (ADR 0312), so the admission is not pinned by a finding.
