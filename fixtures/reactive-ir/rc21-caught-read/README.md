# SC5008: a computation catches a read's throw

rc.21 `CAUGHT_REACTIVE_READ`, judged per pass: an uncaught throw leaves it, so a read outside
the `try` reports nothing. Proven for an async source the body created, read on a sync compute's
stack in a `try` that keeps it; a memo that may throw its error is open, and a signal cannot
throw (ADR 0637). Chrome, rc.21.
