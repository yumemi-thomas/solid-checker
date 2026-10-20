# Transported member stability (ADR 0273)

`StableBody` is the positive direct-accessor transport, `StableTracked` stays clean, every other consumer has no violation. Unrelated receiver writes veto only the matching transported name (assignment, update, destructuring, loops, delete, literal computed keys, exact `Object`/`Reflect` descriptor/set/delete calls, literal `Object.assign` sources); local `Object`/`Reflect` homonyms do not; unrelated nonliteral computed writes are outside the premise. Primitive declarations are `accessor-provenance`'s reduced published rc.13 signatures.
