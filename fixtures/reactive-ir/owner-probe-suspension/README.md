# An owner probe holds up to a suspension (ADR 0866)

`if (getOwner())` proves the owner only on the probed stack. An `await`, a `for await`
or a `yield` in the guarded block before the operation, or in a loop inside the block
that repeats it, leaves that stack: rc.21 raises NO_OWNER_CLEANUP there. An operation
before the suspension, and one in a loop that probes again, keep the proof.
`otherArm`'s operation runs before any suspension: the await is in the other arm, written after it.
