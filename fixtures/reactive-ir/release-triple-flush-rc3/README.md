# release-triple-flush-rc3

`flush()` inside action bodies on the audited `2.0.0-rc.3` triple does not throw (`@solidjs/signals` gained the `FLUSH_IN_ACTION` guard in rc.8; rc.3's `flush` drains inside an action step). The typings are identical, so `tsc` is clean against the stub and published rc.3 and only the runtime differs; the throw follows the resolved signals (`release-triple-flush-rc8`, `rc9-flush-in-action`). Expected: no finding, no SC9014.
