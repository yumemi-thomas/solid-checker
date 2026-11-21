# SC2009: a render effect's apply writes an optimistic store before load (ADR 1175)

A render effect runs its apply at creation, and `createOptimisticStore`'s seed is no answer: a violation. The updater literal
handed to the setter runs after the setter's guard throws, so its member write is not code run before the write.
