# A consumer of a `merged-props` return

Pins the contract claim that depends on the caller's argument position (ADR 0109's `merged-props`, the `no-destructure` arm in `binding_initializes_reactive_store`): props at the reached position (`withDefaults` at 0, `withOverrides` at 1) give SC1003; plain objects and `withOverrides` with props at 0 (a merge does carry argument 0's reactivity but the census cannot certify it) stay clean, a recorded under-report. Ships `.solid-checker/authorize-contract.json` (`fixture_authorization.rs`); manifest byte-identical to `package-return-consumer`'s.
