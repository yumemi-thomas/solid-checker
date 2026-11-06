# generator-stepped

**Claim (ADR 0820).** A generator a tracked async computation steps runs on the computation's flight past its first suspension. An async generator's read after an `await`, stepped by a `for await` written in the computation, is proved. A read after a `yield`, an iterator stepped any other way, and a sync generator stepped across an `await` are `generator-stepped` obligations. The generator's first segment under `for await`, and a sync generator drained before the first suspension, stay clean.
