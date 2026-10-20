# tuple-slot-context

**Claim (ADR 0282).** A tuple slot is followed only from the fresh array a factory call returns; a context provider's value is one array shared by every consumer, any of which can replace a slot, so `const [, peek] = useContext(TupleContext); peek()` claims nothing (an array pattern of a context value also blocks that context in ADR 0273's census), while `ObjectConsumer` (the same census admitting a returned object's member) is an `SC1001` violation. Closed program (`"private": true`); stubs from `context-member-identity` (published rc.13).
