# compound-member-tag

**Claim (ADR 0823).** A member tag of a same-file `const` holder that ADR 0342 accounts renders the stored function: its callers are enumerated through the tags, so `Card`'s body read of a signal-backed prop is a proved SC1001, `Badge`'s literal prop is proved clean, and `Panel`'s tracked-effect read of a pending getter is SC9012 `async-prop-getter`, as a plain tag. A holder that escapes resolves no tag: `Loose`'s tracked-effect read is SC9012 `prop-pending-unproven`, never silent; `Derived`'s memo read is not asked.
