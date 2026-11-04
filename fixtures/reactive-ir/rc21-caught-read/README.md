# SC5008: a computation catches a read's throw

rc.21 `CAUGHT_REACTIVE_READ`. Proven for an async source the body created, read first in
the block of a sync compute whose `catch` has no `throw`; a memo that may throw its error is
open, and a signal cannot throw (ADR 0637). Chrome, rc.21.
