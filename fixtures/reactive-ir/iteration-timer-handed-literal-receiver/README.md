# iteration-timer-handed-literal-receiver

**Claim (ADR 1056), the receiver twin.** `createHealth` calls `options.now()`, and the literal's `now` is a method that reads `this` (the literal), so it may call `schedule` with anything: `run` is not proven, and `set(2)` stays uncertifiable (`tuple-iteration-written`). Chrome reports the write (covered-uncertifiable).
