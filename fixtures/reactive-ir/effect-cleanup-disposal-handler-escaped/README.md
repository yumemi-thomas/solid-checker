# effect-cleanup-disposal-handler-escaped

**Claim (ADR 1230, its control).** The root's result is called from a button's `onClick` and also handed to a helper that may call it anywhere: not every disposal runs from a handler, so the cleanup's read stays SC1001 uncertifiable. Stub: `effect-cleanup-disposal-only`'s.
