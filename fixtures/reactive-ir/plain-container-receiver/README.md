# plain-container-receiver

**Claim (ADR 0693).** A project subclass that overrides a `Map` member withholds that member for a receiver that may be its instance (ADR 0683). A `new Map()` of the global, on the spot or held by a binding nothing rebinds, is none, and its `forEach` runs in place: a violation. A subclass, an alias of it, a shadowing `Map` and a rebound `let` stay uncertifiable. Chrome: the plain receivers warn, the others are quiet.
