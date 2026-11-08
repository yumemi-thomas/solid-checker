# walk-bounds

**Claim (ADR 0695).** A walk that stops at its bound proved nothing. Past each bound the answer is open: a callback handed on by helpers, an async read five helpers from a leaf owner, a read after an await behind a sync helper. A cleanup keeps its silence only while nothing here can dispose its owner on a chosen stack. `LongChainInLoop` reaches its read through five helpers after an await in a loop: uncertifiable, where the walk stopped silently at three.

ADR 0835: `reads0`'s own read, reached only through helpers past the call-attributed walk's bound of four, is SC5001 and SC5003 uncertifiable at the read (the build before placed no row there).
