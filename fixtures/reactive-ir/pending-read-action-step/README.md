# pending-read-action-step

**Claim (ADR 0614).** A pending read in an action's first step runs in the leaf owner its calls run in. Measured on rc.17: `FromSettled` and `FromTracked` throw `PENDING_ASYNC_FORBIDDEN_SCOPE`; `PastYield` reports nothing.
