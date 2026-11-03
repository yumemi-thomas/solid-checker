# leaf-carrier-provably-runs

**Claim (ADR 0563).** A leaf owner's forbidden call (SC3001) reached through a standard-library carrier is proven only where the carrier provably runs its callback (`carrier_runs::carrier_provably_runs`); otherwise what the callback reaches is possible, not guaranteed, and the finding is a proof obligation.

Violation: SC3001 in `LeafNonEmpty` (`[1].forEach`).

Uncertifiable, never a violation (both violations on 644bd1a81): SC3001 in `LeafEmpty` and `LeafPropsReceiver`, each with SC4001's obligation (the possible-unowned edge of ADR 0563).

Runtime: rc.17 runs neither obligation callback when the receiver is empty, so it throws nothing there. Stub: `async-unsuspendable-call-attributed`'s. `tsc --noEmit` clean.
