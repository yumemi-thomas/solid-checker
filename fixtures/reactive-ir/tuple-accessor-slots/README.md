# tuple-accessor-slots

**Claim (ADR 0801).** A hook's array literal, destructured at its call, hands the accessor each slot names. `settled-interval` (`clean-interval-hook`'s shape), `settled`, `sync-of-mixed`, `hole` and `covered` stay certified. `async-slot`, `async-of-mixed` and `body-read` are proved rows of the slot's source. `two-returns` and `held-then-written` have no one literal and stay uncertifiable. All warned or stayed quiet in Chrome (rc.21) as claimed.
