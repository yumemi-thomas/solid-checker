# A prop call in an apply closure (ADR 1229)

The prop's accessor is called in a closure the apply calls once, in place: the read is the apply's, an untracked read of a signal (rc.21 warns STRICT_READ_UNTRACKED). A violation.

Chrome: `fixtures/runtime-oracle/apply-closure-prop`.
