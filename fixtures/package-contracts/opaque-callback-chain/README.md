# A local helper's callback timing needs an enclosing execution proof

`invoke` calls its callback synchronously. `Direct` calls that helper in its
own body, so its callback proposal retains the same-stack operation. `Opaque`
puts the helper call inside an arrow that `retain` stores and nobody invokes;
the helper's own inline behavior cannot establish an invocation by `Opaque`.
`Stored` returns an object containing the arrow; that likewise does not prove
the callback runs before the export returns. Their callback domains stay
unknown until the enclosing execution can be represented and verified.

The generator corpus pins exactly one positive callback operation, for
`Direct`, and explicit unresolved callback claims for `Opaque` and `Stored`.
All creates entries are proposals requiring their normal census and recipe
veto; this fixture's snapshots are not accepted certification receipts.

The native `opaque_callback_chains_stay_open_and_forged_direct_callbacks_refuse`
test authenticates these exact runtime and declaration bytes, verifies the
ordinary proposal, and checks that transplanting `Direct`'s callback claim onto
either sibling still refuses during Type Facts witness verification. ADR 0014
records the decision and real-package measurement.
