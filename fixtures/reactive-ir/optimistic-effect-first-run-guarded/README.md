# SC2009: a guarded call in a child effect apply (ADR 1224)

The child's apply calls the prop only on a later run (`if (v > 1)`), after the answer, and Chrome is silent: uncertifiable. The walk takes an apply step only where every position below it runs on every completion of the first run.
