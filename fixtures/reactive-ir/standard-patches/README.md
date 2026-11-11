# standard-patches

**Claim (ADR 0781).** A write to a standard global or prototype member replaces that member for the whole project: a callback handed to it is open, however the write is spelled. A member nothing wrote, a write of another name, and a symbol key keep the proof.

ADR 0951: a reflective definer put to a literal key is a write of that name (`Object.defineProperty(globalThis, "requestAnimationFrame", …)` withholds `requestAnimationFrame` only): the calls of other members (`queueMicrotask`, `map`, `defineProperty`, `assign`) keep the proof.
