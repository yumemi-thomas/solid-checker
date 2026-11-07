# value-use-escaped

**Claim (ADR 0860).** A read, call or write through a value the census cannot trace to its source has a row of its own, at that site. The census (ADR 0692) answers a position it cannot follow (a carrier, a logical, a member it cannot name, a primitive's callback, a value an export forwards) with an uncertifiable `SC9012` there, and ADR 0831 gives each use the walk ends at its own. The walk used to end at the escape itself, so a read on a later line had no row: Chrome warns there, and that line was clean.

`Carriers` lets a signal or a store escape on one line each, through a `Map`, a `??`, `Object.assign` into a literal and into an array a callback iterates, `untrack`'s returned value, a global object's member, a method that answers `this`, and a hook another module exports, then reads or writes each in the body, its strict window: every read and the write through the `Map` are `SC9012` uncertifiable at their own line, naming the escape. The escape's row stays where it was.

`Tracked` is the negative side: the same escapes, read in a memo, where a read is tracked and legal. No row stands at those reads.

The stub is rc.21's published `solid-js` version; `solid-js.d.ts` is `value-use-census`'s.
