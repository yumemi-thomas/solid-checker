# A prop call in an apply closure (ADR 1229)

The prop's setter is called in a closure the effect's apply calls in place: a write in an effect apply is legal on rc.21. Clean.

Chrome: `fixtures/runtime-oracle/apply-closure-prop`.
