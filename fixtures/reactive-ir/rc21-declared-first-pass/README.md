# SC1001, not SC5001: a declared `loadingValue` in its creating pass

A declared source is born committed, and a body runs once, so the pass that creates it
reads the served value: a plain read warns `STRICT_READ_UNTRACKED`, an `untrack` is
silent, and a leaf read warns only on a re-ask, an obligation (ADR 0632). Chrome, rc.21.
