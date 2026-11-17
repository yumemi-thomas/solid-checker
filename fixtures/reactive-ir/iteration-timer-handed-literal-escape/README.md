# iteration-timer-handed-literal-escape

**Claim (ADR 1056), the escape twin.** `createHealth` hands its parameter whole to `keep`, so `schedule` may be called anywhere with anything: `run` is not proven, and `set(2)` stays uncertifiable (`tuple-iteration-written`). Chrome reports the write (covered-uncertifiable).
