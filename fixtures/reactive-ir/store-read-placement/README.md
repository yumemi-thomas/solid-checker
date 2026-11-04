# store-read-placement

**Claim (ADR 0614).** A store or prop member read takes the role an accessor call takes at the same place. An intrinsic `ref` the body applies is a strict-read position (ADR 0520) for members and spreads too, and a named function a standard-library carrier runs places its member reads at the carrier (ADR 0531); a hand-off nothing follows is an obligation, not silence. Measured on rc.17: `RefRead`, `RefSpread`, `RefProp` and `Handed` warn `STRICT_READ_UNTRACKED`; the `untrack`, timer and shadowed cases report nothing.
