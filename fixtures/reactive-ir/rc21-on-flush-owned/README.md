# SC3002: `onFlush` in an owned scope

rc.21 warns `ON_FLUSH_IN_OWNED_SCOPE` when `onFlush` runs under an owner that is
no leaf: a component body, a computation, a root. An effect's apply, `onSettled`,
`createTrackedEffect` and an event handler have no such owner (ADR 0625).
