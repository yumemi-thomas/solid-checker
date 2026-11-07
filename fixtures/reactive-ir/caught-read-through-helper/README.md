# caught-read-through-helper

**Claim.** rc.21 reports `CAUGHT_REACTIVE_READ` when a read's throw lands in a pass and the body completes, whether the read sits in the `try` or in a helper the `try` calls. A helper call in a `try` that reaches an accessor which may throw into the pass is SC5008's obligation, as the direct read is.

Obligations (`caught-read-unproven`, at the call in the `try`): `ThroughHelper`, `ThroughTwoHelpers`. Silent: `SignalThroughHelper`, `PlainHelper`.

Runtime: rc.21 Chrome, probe `cases/mv/caller-try`; fuzz variants `caught-throw-helper`, `caught-throw-helper-2`. A memo that holds no error fires nothing, so these are obligations, not violations. Premises: the closed program is the `private` manifest.
