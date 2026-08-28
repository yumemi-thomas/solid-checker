# Solid 1.x scheduled artifact-identity control

The reduced `@solid-primitives/scheduled` package in this fixture does not
match the published `1.5.3` runtime/declaration/closure digests. Its package
name cannot select the receipt-issued first-party contract.

The callback's delayed `query()` read remains locally visible, but without an
accepted exact-artifact operation its tracking behavior is uncertifiable. The
snapshot therefore records one local uncertifiable finding rather than treating
missing evidence as either safety or a proven violation.

The published scheduled bundle is regenerated and receipt-validated by the
contract conformance gate; this fixture exists to prove that authority cannot
cross to different bytes.
