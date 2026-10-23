# write-role-entry-census

**Claim (ADR 0378).** A write takes a permissive role from its function's entries only when they are all of its entries; an unfollowed use or an unplaced entry leaves `SC2001` uncertifiable, and a disallowed entry is a violation. `variants/closed` (`"private": true`): an escape ADR 0327 proves event-only stays clean. Each case says why in its comment. Stubs are verbatim rc.17; every case type-checks against the published typings.
